use super::*;
use rusqlite::Connection;

fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn
}

fn titles(conn: &Connection, list_id: &str, include_completed: bool) -> Vec<String> {
    store::tasks_for_list(conn, list_id, include_completed)
        .unwrap()
        .into_iter()
        .map(|task| task.title)
        .collect()
}

fn add(conn: &Connection, list_id: &str, title: &str) -> String {
    create_task(conn, list_id, title, "", 0, "", "", "")
        .unwrap()
        .pointer("/task/id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

#[test]
fn first_read_creates_a_list_and_later_reads_reuse_it() {
    let conn = test_conn();
    let payload = lists_payload(&conn).unwrap();
    let lists = payload["lists"].as_array().unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0]["title"], DEFAULT_LIST_TITLE);
    assert_eq!(payload["default_list_id"], lists[0]["id"]);

    let again = lists_payload(&conn).unwrap();
    assert_eq!(again["lists"].as_array().unwrap().len(), 1, "no duplicate");
    assert_eq!(again["default_list_id"], payload["default_list_id"]);
}

#[test]
fn a_task_with_no_list_lands_in_the_default_one() {
    let conn = test_conn();
    let created = create_task(&conn, "", "Reply to the landlord", "", 0, "", "", "").unwrap();
    let default_id = lists_payload(&conn).unwrap()["default_list_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(created["task"]["list_id"], default_id);
}

#[test]
fn new_tasks_stack_on_top() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    add(&conn, &list, "first");
    add(&conn, &list, "second");
    add(&conn, &list, "third");
    assert_eq!(titles(&conn, &list, false), ["third", "second", "first"]);
}

#[test]
fn completed_tasks_sink_and_can_be_hidden() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    add(&conn, &list, "keep");
    let done = add(&conn, &list, "tick me");
    set_done(&conn, &done, true).unwrap();

    assert_eq!(titles(&conn, &list, false), ["keep"]);
    assert_eq!(titles(&conn, &list, true), ["keep", "tick me"]);

    set_done(&conn, &done, false).unwrap();
    assert_eq!(
        titles(&conn, &list, false).len(),
        2,
        "unticking restores it"
    );
}

#[test]
fn clearing_completed_removes_only_ticked_tasks() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    add(&conn, &list, "keep");
    let done = add(&conn, &list, "gone");
    set_done(&conn, &done, true).unwrap();

    let result = clear_completed(&conn, &list).unwrap();
    assert_eq!(result["removed"], 1);
    assert_eq!(titles(&conn, &list, true), ["keep"]);
}

#[test]
fn reorder_follows_the_client_and_keeps_unlisted_tasks() {
    let mut conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    let a = add(&conn, &list, "a");
    let b = add(&conn, &list, "b");
    let c = add(&conn, &list, "c");

    reorder_tasks(&mut conn, &list, &[a.clone(), b.clone(), c.clone()]).unwrap();
    assert_eq!(titles(&conn, &list, false), ["a", "b", "c"]);

    // A task the client never saw (created between its read and its drop) must
    // survive the reorder rather than fight for position 0.
    let late = add(&conn, &list, "late");
    reorder_tasks(&mut conn, &list, &[c, b, a]).unwrap();
    assert_eq!(titles(&conn, &list, false), ["c", "b", "a", "late"]);
    assert!(store::task(&conn, &late).unwrap().is_some());
}

#[test]
fn deleting_a_list_takes_its_tasks_with_it() {
    let mut conn = test_conn();
    let keep = ensure_default_list(&conn).unwrap();
    let doomed = create_list(&conn, "Groceries").unwrap()["list"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let kept = add(&conn, &keep, "survivor");
    add(&conn, &doomed, "milk");

    delete_list(&mut conn, &doomed).unwrap();
    assert!(store::task_list(&conn, &doomed).unwrap().is_none());
    assert!(
        store::tasks_for_list(&conn, &doomed, true)
            .unwrap()
            .is_empty()
    );
    assert!(store::task(&conn, &kept).unwrap().is_some());
}

#[test]
fn a_mail_linked_task_is_findable_from_its_thread() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    create_task(
        &conn,
        &list,
        "Send the invoice",
        "",
        0,
        "acct1",
        "acct1#thread#9",
        "<msg@example.com>",
    )
    .unwrap();

    let payload = thread_tasks_payload(&conn, "acct1#thread#9").unwrap();
    let tasks = payload["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["account"], "acct1");
    assert_eq!(tasks[0]["message_id"], "<msg@example.com>");
    assert!(
        thread_tasks_payload(&conn, "acct1#thread#other").unwrap()["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_edit_never_blanks_a_title() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    let id = add(&conn, &list, "Pay rent");

    let updated = update_task(&conn, &id, Some("   "), None, None, None).unwrap();
    assert_eq!(updated["task"]["title"], "Pay rent");

    let updated = update_task(&conn, &id, Some("  Pay the rent  "), None, None, None).unwrap();
    assert_eq!(updated["task"]["title"], "Pay the rent");
}

#[test]
fn moving_a_task_between_lists_puts_it_on_top() {
    let conn = test_conn();
    let from = ensure_default_list(&conn).unwrap();
    let to = create_list(&conn, "Later").unwrap()["list"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    add(&conn, &to, "already here");
    let moved = add(&conn, &from, "moving");

    update_task(&conn, &moved, None, None, None, Some(&to)).unwrap();
    assert_eq!(titles(&conn, &to, false), ["moving", "already here"]);
    assert!(titles(&conn, &from, false).is_empty());
}

#[test]
fn ids_do_not_repeat() {
    let ids: std::collections::HashSet<String> = (0..256).map(|_| new_id("task")).collect();
    assert_eq!(ids.len(), 256);
    assert!(ids.iter().all(|id| id.starts_with("task-")));
}

#[test]
fn deleting_a_task_hands_back_enough_to_undo_it() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    add(&conn, &list, "first");
    let id = add(&conn, &list, "second");
    add(&conn, &list, "third");
    let before = store::task(&conn, &id).unwrap().unwrap();

    let deleted = delete_task(&conn, &id).unwrap();
    assert_eq!(titles(&conn, &list, false), ["third", "first"]);

    restore(&conn, &deleted["restore"]).unwrap();

    // Identity and position both survive: undo is not "add it again at the top".
    let after = store::task(&conn, &id).unwrap().unwrap();
    assert_eq!(after, before);
    assert_eq!(titles(&conn, &list, false), ["third", "second", "first"]);
}

#[test]
fn deleting_a_list_hands_back_the_list_and_everything_in_it() {
    let mut conn = test_conn();
    ensure_default_list(&conn).unwrap();
    let doomed = create_list(&conn, "Groceries").unwrap()["list"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    add(&conn, &doomed, "milk");
    let ticked = add(&conn, &doomed, "bread");
    set_done(&conn, &ticked, true).unwrap();

    let deleted = delete_list(&mut conn, &doomed).unwrap();
    assert!(store::task_list(&conn, &doomed).unwrap().is_none());

    restore(&conn, &deleted["restore"]).unwrap();

    let restored = store::task_list(&conn, &doomed).unwrap().unwrap();
    assert_eq!(restored.title, "Groceries");
    // Completed tasks come back completed, not as fresh work to do again.
    assert_eq!(titles(&conn, &doomed, true), ["milk", "bread"]);
    assert!(store::task(&conn, &ticked).unwrap().unwrap().completed_at > 0);
}

#[test]
fn clearing_completed_can_be_undone() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    add(&conn, &list, "keep");
    let a = add(&conn, &list, "done a");
    let b = add(&conn, &list, "done b");
    set_done(&conn, &a, true).unwrap();
    set_done(&conn, &b, true).unwrap();

    let cleared = clear_completed(&conn, &list).unwrap();
    assert_eq!(cleared["removed"], 2);
    assert_eq!(titles(&conn, &list, true), ["keep"]);

    restore(&conn, &cleared["restore"]).unwrap();
    assert_eq!(store::tasks_for_list(&conn, &list, true).unwrap().len(), 3);
}

/// A double-click on Undo must not fail, and must not duplicate anything.
#[test]
fn restoring_twice_changes_nothing_the_second_time() {
    let conn = test_conn();
    let list = ensure_default_list(&conn).unwrap();
    let id = add(&conn, &list, "once");
    let deleted = delete_task(&conn, &id).unwrap();

    restore(&conn, &deleted["restore"]).unwrap();
    restore(&conn, &deleted["restore"]).unwrap();
    assert_eq!(titles(&conn, &list, true), ["once"]);
}

#[test]
fn restoring_junk_is_ignored_rather_than_an_error() {
    let conn = test_conn();
    ensure_default_list(&conn).unwrap();
    assert!(restore(&conn, &Value::Null).is_ok());
    assert!(restore(&conn, &json!({})).is_ok());
    // A task with no list has nowhere to go, so it is skipped, not inserted.
    let result = restore(
        &conn,
        &json!({ "tasks": [{ "id": "t1", "title": "orphan" }] }),
    )
    .unwrap();
    assert_eq!(result["restored"], 0);
}
