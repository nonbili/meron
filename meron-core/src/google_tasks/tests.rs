use super::*;
use crate::tasks;
use serde_json::json;
use std::io::Write;
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use url::Url;

const EMAIL: &str = "tasks@example.test";
const TASKS_PATH: &str = "/tasks/v1/lists/list-1/tasks";

fn fixture(replies: Vec<(&str, Value)>) -> (Client, Receiver<String>, Mutex<Connection>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = Url::parse(&format!(
        "http://{}/tasks/v1/",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let replies: Vec<_> = replies
        .into_iter()
        .map(|(status, body)| {
            (
                status.to_string(),
                if body.is_null() {
                    String::new()
                } else {
                    body.to_string()
                },
            )
        })
        .collect();
    let (requests, received) = mpsc::channel();
    std::thread::spawn(move || {
        for (status, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let request = crate::test_support::read_http_request(&mut stream);
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-tasks")
            );
            requests.send(request).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            ).unwrap();
        }
    });
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    store::save_account(&conn, EMAIL).unwrap();
    (
        Client::new("test-tasks").unwrap().with_endpoint(endpoint),
        received,
        Mutex::new(conn),
    )
}

fn incoming(id: &str, etag: &str, title: &str, done: bool) -> Value {
    json!({
        "id": id, "etag": etag, "title": title, "notes": "Fictional notes",
        "status": if done { "completed" } else { "needsAction" },
        "due": "2026-11-22T00:00:00.000Z",
        "completed": if done { Some("2026-11-23T12:00:00Z") } else { None },
    })
}

fn lists() -> Value {
    json!({"kind": "tasks#taskLists", "items": [{"id": "list-1", "title": "Errands"}]})
}

fn page(items: &[Value]) -> Value {
    json!({"kind": "tasks#tasks", "items": items})
}

fn refresh(db: &Mutex<Connection>, client: &Client) -> Result<bool> {
    sync_lists(db, client, EMAIL, client.lists()?)
}

fn list_id(db: &Mutex<Connection>) -> String {
    let conn = db.lock().unwrap();
    let result = tasks::lists_payload(&conn).unwrap();
    let list = result["lists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|list| list["provider"] == "google")
        .unwrap();
    // Lists named alike on two Google accounts are told apart by it.
    assert_eq!(list["account"], EMAIL);
    list["id"].as_str().unwrap().to_string()
}

fn create_local(db: &Mutex<Connection>, list: &str) -> String {
    use chrono::TimeZone as _;
    let due = chrono::Local
        .with_ymd_and_hms(2026, 11, 22, 15, 45, 0)
        .earliest()
        .unwrap()
        .timestamp();
    let value = tasks::mutate_with_due_time(
        &db.lock().unwrap(),
        &json!({"due_has_time": true}),
        |conn| {
            tasks::create_task(
                conn,
                list,
                "Made in Meron",
                "Fictional notes",
                due,
                "mail-local",
                "thread-local",
                "",
            )
        },
    )
    .unwrap();
    assert_eq!(value["task"]["due_has_time"], false);
    value["task"]["id"].as_str().unwrap().to_string()
}

#[test]
fn round_trips_tasks_through_the_rest_client() {
    let seed = incoming("server-id", "seed", "From Google", false);
    let created = incoming("created-id", "created", "Made in Meron", false);
    let done = incoming("created-id", "done", "Made in Meron", true);
    let external = incoming("created-id", "external", "Edited in Google", true);
    let (client, requests, db) = fixture(vec![
        (
            "200 OK",
            json!({"kind": "tasks#taskLists", "nextPageToken": "next"}),
        ),
        ("200 OK", lists()),
        ("200 OK", page(std::slice::from_ref(&seed))),
        ("200 OK", lists()),
        ("200 OK", page(std::slice::from_ref(&seed))),
        ("200 OK", created.clone()),
        ("200 OK", lists()),
        ("200 OK", page(&[seed.clone(), created])),
        ("200 OK", done),
        ("200 OK", lists()),
        ("200 OK", page(&[seed.clone(), external.clone()])),
        ("200 OK", lists()),
        ("200 OK", page(&[seed, external])),
        ("204 No Content", Value::Null),
    ]);
    assert!(refresh(&db, &client).unwrap());
    let list = list_id(&db);
    {
        let tasks = db::tasks_for_list(&db.lock().unwrap(), &list, true).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "From Google");
        assert_eq!(
            tasks[0].due_at,
            client::date_at_local_midnight(chrono::NaiveDate::from_ymd_opt(2026, 11, 22).unwrap())
                .unwrap()
        );
    }
    let id = create_local(&db, &list);
    assert!(refresh(&db, &client).unwrap());
    tasks::set_done(&db.lock().unwrap(), &id, true).unwrap();
    assert!(refresh(&db, &client).unwrap());
    assert!(refresh(&db, &client).unwrap());
    let local = db::task(&db.lock().unwrap(), &id).unwrap().unwrap();
    assert_eq!(local.title, "Edited in Google");
    assert_eq!(local.thread_id, "thread-local");
    assert!(local.completed_at > 0);
    tasks::delete_task(&db.lock().unwrap(), &id).unwrap();
    refresh(&db, &client).unwrap();
    assert!(store::mapping(&db.lock().unwrap(), &id).unwrap().is_none());

    let observed: Vec<_> = requests.iter().collect();
    assert!(observed[1].contains("pageToken=next"));
    assert!(observed[2].starts_with(&format!("GET {TASKS_PATH}?")));
    assert!(observed[2].contains("showHidden=true"));
    let post = &observed[5];
    assert!(post.starts_with(&format!("POST {TASKS_PATH} ")));
    assert!(post.contains("\"due\":\"2026-11-22T00:00:00Z\""));
    assert!(!post.contains("thread-local"));
    let patch = &observed[8];
    assert!(patch.starts_with(&format!("PATCH {TASKS_PATH}/created-id ")));
    assert!(patch.to_ascii_lowercase().contains("if-match: \"created\""));
    assert!(patch.contains("\"status\":\"completed\""));
    assert!(observed[13].starts_with(&format!("DELETE {TASKS_PATH}/created-id ")));
}

#[test]
fn an_ambiguous_insert_is_not_retried_automatically() {
    let (client, requests, db) = fixture(vec![
        ("200 OK", lists()),
        ("200 OK", page(&[])),
        ("200 OK", lists()),
        ("200 OK", page(&[])),
        ("503 Service Unavailable", Value::Null),
        ("200 OK", lists()),
        ("200 OK", page(&[])),
    ]);
    refresh(&db, &client).unwrap();
    let id = create_local(&db, &list_id(&db));
    assert!(refresh(&db, &client).is_err());
    let retry = refresh(&db, &client).unwrap_err();
    assert!(retry.to_string().contains("creation was interrupted"));
    assert!(db::task(&db.lock().unwrap(), &id).unwrap().is_some());
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count(),
        1
    );
}

#[test]
fn an_edit_after_acknowledgement_uses_the_acknowledged_version() {
    let created = incoming("created-id", "acknowledged", "Made in Meron", false);
    let edited = incoming("created-id", "edited", "Another edit", false);
    let (client, requests, db) = fixture(vec![
        ("200 OK", lists()),
        ("200 OK", page(&[])),
        ("200 OK", created.clone()),
        ("200 OK", edited),
    ]);
    refresh(&db, &client).unwrap();
    let id = create_local(&db, &list_id(&db));
    let list = RemoteList {
        id: list_id(&db),
        remote_id: "list-1".into(),
    };
    let mapping = || store::mapping(&db.lock().unwrap(), &id).unwrap().unwrap();
    let mut remote = HashMap::new();

    assert!(push(&db, &client, &list, &mapping(), &mut remote).unwrap());
    {
        let conn = db.lock().unwrap();
        tasks::mutate_with_due_time(&conn, &Value::Null, |conn| {
            tasks::update_task(conn, &id, Some("Another edit"), None, None, None)
        })
        .unwrap();
        store::apply_snapshot(&conn, &list, remote.clone()).unwrap();
    }

    assert_eq!(mapping().etag, "acknowledged");
    assert!(push(&db, &client, &list, &mapping(), &mut remote).unwrap());
    let last = requests.iter().last().unwrap();
    assert!(last.starts_with("PATCH "));
    assert!(
        last.to_ascii_lowercase()
            .contains("if-match: \"acknowledged\"")
    );
    assert_eq!(
        db::task(&db.lock().unwrap(), &id).unwrap().unwrap().title,
        "Another edit"
    );
}

#[test]
fn a_refused_connection_leaves_creation_pending_for_retry() {
    let (client, _requests, db) = fixture(vec![("200 OK", lists()), ("200 OK", page(&[]))]);
    refresh(&db, &client).unwrap();
    let list = RemoteList {
        id: list_id(&db),
        remote_id: "list-1".into(),
    };
    let id = create_local(&db, &list.id);
    let mapping = || store::mapping(&db.lock().unwrap(), &id).unwrap().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = Url::parse(&format!(
        "http://{}/tasks/v1/",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    drop(listener);
    let client = Client::new("test-tasks").unwrap().with_endpoint(endpoint);
    assert!(push(&db, &client, &list, &mapping(), &mut HashMap::new()).is_err());
    assert_eq!(mapping().state, store::CREATED);
}

#[test]
fn accounts_follow_their_google_calendar_sync_interval() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    store::save_account(&conn, EMAIL).unwrap();
    store::save_account(&conn, "alone@example.test").unwrap();
    crate::caldav::store::save_account(
        &conn,
        &crate::caldav::store::DavAccount {
            id: "google-calendars".into(),
            username: "Tasks@Example.test".into(),
            google: true,
            ..Default::default()
        },
    )
    .unwrap();
    crate::caldav::store::set_sync_interval(&conn, "google-calendars", 0).unwrap();
    assert_eq!(
        sync_intervals(&conn).unwrap(),
        [
            ("alone@example.test".to_string(), 15),
            (EMAIL.to_string(), 0)
        ]
    );
}
