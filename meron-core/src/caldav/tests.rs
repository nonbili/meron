use super::xml::{self, CALDAV, DAV};
use super::*;

/// A calendar home listing as Nextcloud (sabre/dav) writes it: `d:` / `cal:`
/// prefixes, a 404 propstat for what the collection lacks, a read-only
/// subscription, and an Apple colour with alpha.
const SABRE_HOME: &str = r##"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/" xmlns:x1="http://apple.com/ns/ical/">
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  <d:propstat><d:prop><d:displayname/><cs:getctag/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/personal/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
    <d:displayname>Personal &amp; Work</d:displayname>
    <cs:getctag>http://sabre.io/ns/sync/42</cs:getctag>
    <d:sync-token>http://sabre.io/ns/sync/42</d:sync-token>
    <x1:calendar-color>#0082C9FF</x1:calendar-color>
    <cal:supported-calendar-component-set><cal:comp name="VEVENT"/><cal:comp name="VTODO"/></cal:supported-calendar-component-set>
    <d:current-user-privilege-set><d:privilege><d:read/></d:privilege><d:privilege><d:all/></d:privilege></d:current-user-privilege-set>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/holidays/</d:href>
  <d:propstat>
   <d:prop>
    <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
    <cs:getctag>7</cs:getctag>
    <cal:supported-calendar-component-set><cal:comp name="vevent"/></cal:supported-calendar-component-set>
    <d:current-user-privilege-set><d:privilege><d:read/></d:privilege></d:current-user-privilege-set>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/calendars/alice/inbox/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:schedule-inbox/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
 </d:response>
</d:multistatus>"##;

/// A sync-collection answer as Stalwart writes it: upper-case `D:`, a removed
/// member with a response-level 404, and the new token last.
const STALWART_SYNC: &str = r#"<?xml version="1.0" encoding="UTF-8"?><D:multistatus xmlns:D="DAV:"><D:response><D:href>/dav/cal/alice%40mail.test/default/a.ics</D:href><D:propstat><D:prop><D:getetag>"11"</D:getetag></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response><D:response><D:href>/dav/cal/alice%40mail.test/default/b.ics</D:href><D:status>HTTP/1.1 404 Not Found</D:status></D:response><D:sync-token>https://mail.test/dav/sync/9</D:sync-token></D:multistatus>"#;

#[test]
fn multistatus_keeps_successful_props_only() {
    let ms = xml::parse_multistatus(SABRE_HOME).unwrap();
    assert_eq!(ms.responses.len(), 4);
    let home = &ms.responses[0];
    assert!(home.prop(DAV, "displayname").is_none());
    assert!(home.prop(DAV, "resourcetype").is_some());
    let personal = &ms.responses[1];
    assert_eq!(
        personal.prop_text(DAV, "displayname"),
        Some("Personal & Work")
    );
    assert!(
        personal
            .prop(DAV, "resourcetype")
            .unwrap()
            .child(CALDAV, "calendar")
            .is_some()
    );
}

#[test]
fn calendars_from_a_sabre_home() {
    let base = Url::parse("https://cloud.example.com/remote.php/dav/calendars/alice/").unwrap();
    let ms = xml::parse_multistatus(SABRE_HOME).unwrap();
    let calendars = calendars_in(&base, &ms).unwrap();
    assert_eq!(
        calendars,
        vec![
            Calendar {
                url: "https://cloud.example.com/remote.php/dav/calendars/alice/personal/".into(),
                name: "Personal & Work".into(),
                color: "#0082C9".into(),
                components: vec!["VEVENT".into(), "VTODO".into()],
                ctag: "http://sabre.io/ns/sync/42".into(),
                sync_token: "http://sabre.io/ns/sync/42".into(),
                read_only: false,
            },
            Calendar {
                url: "https://cloud.example.com/remote.php/dav/calendars/alice/holidays/".into(),
                name: "holidays".into(),
                color: String::new(),
                components: vec!["VEVENT".into()],
                ctag: "7".into(),
                sync_token: String::new(),
                read_only: true,
            },
        ]
    );
    assert!(calendars[1].supports("vevent"));
    assert!(!calendars[1].supports("VTODO"));
}

#[test]
fn multistatus_reads_removals_and_sync_token() {
    let ms = xml::parse_multistatus(STALWART_SYNC).unwrap();
    assert_eq!(
        ms.sync_token.as_deref(),
        Some("https://mail.test/dav/sync/9")
    );
    assert_eq!(ms.responses[0].status, None);
    assert_eq!(ms.responses[0].prop_text(DAV, "getetag"), Some("\"11\""));
    assert_eq!(ms.responses[1].status, Some(404));
}

#[test]
fn multistatus_rejects_other_documents() {
    assert!(xml::parse_multistatus("<html><body>Login</body></html>").is_err());
    assert!(xml::parse_multistatus("not xml <").is_err());
}

#[test]
fn xml_matches_by_namespace_not_prefix() {
    let root = xml::parse(
        r#"<multistatus xmlns="DAV:"><response><href> /a/ </href></response><x:href xmlns:x="other">no</x:href></multistatus>"#,
    )
    .unwrap();
    assert!(root.is(DAV, "multistatus"));
    assert_eq!(root.child(DAV, "response").unwrap().href(), Some("/a/"));
    assert_eq!(root.children_named(DAV, "href").count(), 0);
}

#[test]
fn xml_keeps_cdata_calendar_data() {
    let root = xml::parse(
        "<d:multistatus xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><c:calendar-data><![CDATA[BEGIN:VCALENDAR\r\nX:<&>\r\nEND:VCALENDAR]]></c:calendar-data></d:multistatus>",
    )
    .unwrap();
    assert_eq!(
        root.child(CALDAV, "calendar-data").unwrap().text(),
        "BEGIN:VCALENDAR\r\nX:<&>\r\nEND:VCALENDAR"
    );
}

#[test]
fn server_url_defaults_to_https() {
    assert_eq!(
        parse_server_url(" cloud.example.com ").unwrap().as_str(),
        "https://cloud.example.com/"
    );
    assert_eq!(
        parse_server_url("http://127.0.0.1:4080/dav")
            .unwrap()
            .as_str(),
        "http://127.0.0.1:4080/dav"
    );
    assert!(parse_server_url("").is_err());
    assert!(parse_server_url("ftp://example.com").is_err());
}

#[test]
fn propfind_body_declares_each_namespace_once() {
    let body = propfind_body(&[
        (DAV, "displayname"),
        (CALDAV, "calendar-home-set"),
        (CALDAV, "supported-calendar-component-set"),
    ]);
    assert_eq!(body.matches("xmlns:c=").count(), 1);
    let root = xml::parse(&body).unwrap();
    let prop = root.child(DAV, "prop").unwrap();
    assert!(prop.child(CALDAV, "calendar-home-set").is_some());
}

#[test]
fn requests_stay_with_the_server_named() {
    let client = |server: &str| Client::new(server, "u", "p", None).unwrap();
    let trusts = |client: &Client, url: &str| client.trusts(&Url::parse(url).unwrap());
    let icloud = client("https://caldav.icloud.com");
    assert!(trusts(&icloud, "https://caldav.icloud.com/123/principal/"));
    // Per-user hosts under the same domain, as iCloud hands out.
    assert!(trusts(
        &icloud,
        "https://p42-caldav.icloud.com/123/calendars/"
    ));
    assert!(!trusts(&icloud, "http://p42-caldav.icloud.com/"));
    assert!(!trusts(&icloud, "https://evil.example.com/"));
    assert!(!trusts(&icloud, "https://icloud.com.evil.example/"));
    let local = client("http://127.0.0.1:4080");
    assert!(trusts(&local, "http://127.0.0.1:4080/dav/"));
    assert!(trusts(&local, "https://127.0.0.1/dav/"));
    assert!(!trusts(&local, "http://127.0.0.1:4081/dav/"));
    assert!(!trusts(&local, "https://127.0.0.1:8443/dav/"));
    // Hosts of one organisation are not one service.
    let cloud = client("https://cloud.example.com");
    assert!(!trusts(&cloud, "https://wiki.example.com/"));
    assert!(!trusts(&cloud, "https://cloud.example.com:8443/"));
    let github = client("https://alice.github.io");
    assert!(!trusts(&github, "https://mallory.github.io/"));
}

#[test]
fn helpers() {
    assert_eq!(normalize_color("#0082C9FF"), "#0082C9");
    assert_eq!(normalize_color("#0082C9"), "#0082C9");
    let a = Url::parse("https://x.test/cal/").unwrap();
    assert!(same_resource(
        &a,
        &Url::parse("https://x.test/cal").unwrap()
    ));
    assert!(!same_resource(
        &a,
        &Url::parse("https://x.test/cal/a.ics").unwrap()
    ));
    assert_eq!(
        last_segment(&Url::parse("https://x.test/cal/My%20Calendar/").unwrap()),
        "My Calendar"
    );
}

const GOOGLE_COLLECTION: &str = "/caldav/v2/calendar%40example.test/events";

fn google_properties(ctag: &str) -> String {
    format!(
        r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/">
        <d:response><d:href>{GOOGLE_COLLECTION}</d:href><d:propstat><d:prop>
        <d:resourcetype><d:collection/><c:calendar/></d:resourcetype><d:displayname>Primary</d:displayname>
        <cs:getctag>{ctag}</cs:getctag>
        </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#
    )
}

fn google_fixture() -> (Client, std::net::TcpListener) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let mut client = Client::google("calendar@example.test", "fixture-token").unwrap();
    assert!(same_member(
        &client.server,
        &Url::parse(&format!(
            "https://apidata.googleusercontent.com{GOOGLE_COLLECTION}"
        ))
        .unwrap()
    ));
    assert!(!client.trusts(&Url::parse("https://other.example.test/").unwrap()));
    client.server = Url::parse(&format!("{origin}{GOOGLE_COLLECTION}")).unwrap();
    client.google_list_url = None;
    (client, listener)
}

fn serve_google(
    listener: std::net::TcpListener,
    responses: Vec<(&'static str, String, String)>,
) -> std::sync::mpsc::Receiver<String> {
    use std::io::Write;
    let (requests, received) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (status, headers, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let request = crate::test_support::read_http_request(&mut stream);
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer fixture-token")
            );
            requests.send(request).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            ).unwrap();
        }
    });
    received
}

fn google_store(client: &Client, read_only: bool) -> (rusqlite::Connection, store::DavCalendar) {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    store::save_account(
        &conn,
        &store::DavAccount {
            id: "dav-1".into(),
            google: true,
            ..Default::default()
        },
    )
    .unwrap();
    let calendar = store::reconcile_calendars(
        &conn,
        "dav-1",
        &[Calendar {
            url: client.server.to_string(),
            read_only,
            ..Default::default()
        }],
    )
    .unwrap()
    .remove(0);
    (conn, calendar)
}

#[test]
fn google_calendar_list_disabled_does_not_remove_existing_secondary_calendars() {
    let (mut client, listener) = google_fixture();
    client.google_list_url = Some(
        client
            .server
            .join("/calendar/v3/users/me/calendarList")
            .unwrap(),
    );
    let (conn, primary) = google_store(&client, false);
    let secondary = google_collection(client.server.clone(), "shared@group.example.test").unwrap();
    let calendars = store::reconcile_calendars(
        &conn,
        "dav-1",
        &[
            Calendar {
                url: primary.url,
                ..Default::default()
            },
            Calendar {
                url: secondary.to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let _requests = serve_google(
        listener,
        vec![
            (
                "403 Forbidden",
                "".into(),
                r#"{"error":{"details":[{"reason":"SERVICE_DISABLED"}]}}"#.into(),
            ),
            ("207 Multi-Status", "".into(), google_properties("one")),
        ],
    );
    let report = sync::sync_account(&conn, &client, "dav-1");
    assert!(
        report.error.contains("Enable the Google Calendar API"),
        "{report:?}"
    );
    let current = store::calendars(&conn, "dav-1").unwrap();
    assert_eq!(current.len(), 2);
    assert_eq!(current[1].id, calendars[1].id);
}

#[test]
fn google_sync_discovers_multiple_pages_and_writes_to_the_selected_calendar() {
    use crate::calendar::events::{self, EventInput};
    use std::time::Duration;

    let (mut client, listener) = google_fixture();
    client.google_list_url = Some(
        client
            .server
            .join("/calendar/v3/users/me/calendarList")
            .unwrap(),
    );
    let (conn, primary) = google_store(&client, false);
    let work = google_collection(client.server.clone(), "work@group.example.test").unwrap();
    let shared = google_collection(client.server.clone(), "shared@group.example.test").unwrap();
    let page_one = r##"{"kind":"calendar#calendarList","nextPageToken":"next+page","items":[
        {"id":"calendar@example.test","primary":true,"accessRole":"owner","summary":"Primary"},
        {"id":"work@group.example.test","accessRole":"writer","summary":"Work","summaryOverride":"Studio","backgroundColor":"#abcdef"}] }"##;
    let page_two = r#"{"kind":"calendar#calendarList","items":[
        {"id":"shared@group.example.test","accessRole":"reader","summary":"Shared"},
        {"id":"busy@group.example.test","accessRole":"freeBusyReader"}] }"#;
    let delta = |token: &str| {
        format!(
            r#"<d:multistatus xmlns:d="DAV:"><d:sync-token>{token}</d:sync-token></d:multistatus>"#
        )
    };
    let received = serve_google(
        listener,
        vec![
            ("200 OK", "".into(), page_one.into()),
            ("200 OK", "".into(), page_two.into()),
            ("207 Multi-Status", "".into(), delta("primary-1")),
            ("207 Multi-Status", "".into(), delta("work-1")),
            ("207 Multi-Status", "".into(), delta("shared-1")),
            ("200 OK", "".into(), page_one.into()),
            ("200 OK", "".into(), page_two.into()),
            ("404 Not Found", "".into(), "".into()),
            (
                "207 Multi-Status",
                "".into(),
                r#"<d:multistatus xmlns:d="DAV:"/>"#.into(),
            ),
            (
                "201 Created",
                format!("Location: {}/created.ics\r\n", work.path()),
                "".into(),
            ),
            ("207 Multi-Status", "".into(), delta("primary-2")),
            ("207 Multi-Status", "".into(), delta("work-2")),
            ("200 OK", "".into(), page_one.replace("writer", "reader")),
            ("200 OK", "".into(), page_two.into()),
            ("207 Multi-Status", "".into(), delta("primary-3")),
            ("207 Multi-Status", "".into(), delta("work-3")),
        ],
    );
    assert_eq!(synced(&conn, &client), sync::Report::default());
    let calendars = store::calendars(&conn, "dav-1").unwrap();
    assert_eq!(calendars.len(), 3);
    assert_eq!(calendars[0].id, primary.id);
    assert_eq!(calendars[0].sync_token, "primary-1");
    assert_eq!(calendars[1].url, work.as_str());
    assert_eq!(calendars[1].name, "Studio");
    assert_eq!(calendars[1].color, "#abcdef");
    assert_eq!(calendars[1].sync_token, "work-1");
    assert_eq!(calendars[2].url, shared.as_str());
    assert_eq!(calendars[2].sync_token, "shared-1");
    assert!(calendars[2].read_only);
    let input = EventInput {
        title: "Planning".into(),
        start: 1_795_251_600,
        end: 1_795_255_200,
        calendar: Some(calendars[2].id.clone()),
        ..Default::default()
    };
    assert!(events::create_event(&conn, &input).is_err());
    store::set_enabled(&conn, &calendars[2].id, false).unwrap();
    let created = events::create_event(
        &conn,
        &EventInput {
            calendar: Some(calendars[1].id.clone()),
            ..input
        },
    )
    .unwrap();
    let id = created["event"]["id"].as_str().unwrap();
    assert_eq!(synced(&conn, &client).pushed, 1);
    let object = store::object_for_event(&conn, id).unwrap().unwrap();
    assert_eq!(object.url, format!("{work}/created.ics"));
    assert!(
        !store::calendar(&conn, &calendars[2].id)
            .unwrap()
            .unwrap()
            .enabled
    );

    let requests: Vec<_> = (0..12)
        .map(|_| received.recv_timeout(Duration::from_secs(2)).unwrap())
        .collect();
    assert!(requests[0].starts_with("GET /calendar/v3/users/me/calendarList?"));
    assert!(requests[1].contains("pageToken=next%2Bpage"));
    assert!(requests[4].starts_with(&format!("REPORT {} ", shared.path())));
    assert!(requests[7].starts_with(&format!("GET {}/", work.path())));
    assert!(requests[8].starts_with(&format!("REPORT {} ", work.path())));
    assert!(requests[8].contains("calendar-query"));
    assert!(requests[9].starts_with(&format!("PUT {}/", work.path())));
    assert!(requests[11].contains("<d:sync-token>work-1</d:sync-token>"));
    let scoped = client.for_calendar(work.as_str()).unwrap();
    assert!(
        scoped
            .delete(
                &format!("{}/wrong-calendar.ics", client.server),
                Some("etag")
            )
            .is_err()
    );
    assert!(scoped.delete_calendar(work.as_str()).is_err());

    // A write queued while access was still granted must not be silently
    // dropped or sent after the calendar becomes read-only.
    let queued = events::create_event(
        &conn,
        &EventInput {
            title: "Queued before access changed".into(),
            start: 1_795_251_600,
            end: 1_795_255_200,
            calendar: Some(calendars[1].id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let report = sync::sync_account(&conn, &client, "dav-1");
    assert_eq!(report.pushed, 0);
    assert!(
        report.error.contains("read-only; pending edits were kept"),
        "{report:?}"
    );
    let queued = store::object_for_event(&conn, queued["event"]["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(queued.state, store::CREATED);
    let requests: Vec<_> = (0..4)
        .map(|_| received.recv_timeout(Duration::from_secs(2)).unwrap())
        .collect();
    assert!(requests[3].starts_with(&format!("REPORT {} ", work.path())));
}

#[test]
fn google_primary_calendar_imports_and_recovers_from_expired_sync_token() {
    use std::time::Duration;

    let event_url = format!("{GOOGLE_COLLECTION}/event.ics");
    let event_props = r#"<d:propstat><d:prop><d:getetag>"1"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>"#;
    let snapshot = format!(
        r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>{event_url}</d:href>{event_props}</d:response><d:sync-token>first</d:sync-token></d:multistatus>"#
    );
    let ics = "BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
UID:fictional-event
DTSTAMP:20261119T080000Z
DTSTART:20261120T090000Z
DTEND:20261120T100000Z
SUMMARY:Planning
END:VEVENT
END:VCALENDAR
";
    let fetched = format!(
        r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>{event_url}</d:href><d:propstat><d:prop>
        <d:getetag>"1"</d:getetag><c:calendar-data><![CDATA[{ics}]]></c:calendar-data>
        </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response></d:multistatus>"#
    );

    let (client, listener) = google_fixture();
    let received = serve_google(listener, [
        ("207 Multi-Status", google_properties("one")),
        ("207 Multi-Status", snapshot),
        ("207 Multi-Status", fetched),
        ("207 Multi-Status", google_properties("two")),
        ("403 Forbidden", r#"<d:error xmlns:d="DAV:"><d:valid-sync-token/></d:error>"#.into()),
        ("207 Multi-Status", r#"<d:multistatus xmlns:d="DAV:"><d:sync-token>second</d:sync-token></d:multistatus>"#.into()),
    ].into_iter().map(|(status, body)| (status, String::new(), body)).collect());
    let err = client.delete_calendar(client.server.as_str()).unwrap_err();
    assert!(err.to_string().contains("outside the selected calendar"));
    let (conn, previous) = google_store(&client, true);
    assert_eq!(synced(&conn, &client).pulled, 1);
    let calendars = store::calendars(&conn, "dav-1").unwrap();
    assert_eq!(calendars.len(), 1);
    assert_eq!(calendars[0].id, previous.id);
    assert!(!calendars[0].read_only);
    assert_eq!(calendars[0].components, ["VEVENT"]);
    assert_eq!(calendars[0].sync_token, "first");
    assert_eq!(
        crate::store::all_events(&conn).unwrap()[0].title,
        "Planning"
    );

    assert_eq!(synced(&conn, &client).pulled, 1);
    let current = store::calendar(&conn, &calendars[0].id).unwrap().unwrap();
    assert_eq!(current.sync_token, "second");
    assert!(crate::store::all_events(&conn).unwrap().is_empty());

    for (method, body) in [
        ("PROPFIND", "<d:propfind"),
        ("REPORT", "<d:sync-token></d:sync-token>"),
        ("REPORT", "<c:calendar-multiget"),
        ("PROPFIND", "<d:propfind"),
        ("REPORT", "<d:sync-token>first</d:sync-token>"),
        ("REPORT", "<d:sync-token></d:sync-token>"),
    ] {
        let request = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(request.starts_with(method), "{request}");
        assert!(request.contains(body), "{request}");
    }
}

#[test]
fn google_sync_writes_events_to_the_server_assigned_url() {
    use crate::calendar::events::{self, EventInput};
    use std::time::Duration;

    // Google can return a different path and spell @ inconsistently.
    let location = format!("{}/google-id.ics", GOOGLE_COLLECTION.replace("%40", "@"));
    let canonical = format!("{GOOGLE_COLLECTION}/google-id.ics");
    let (client, listener) = google_fixture();
    let (conn, calendar) = google_store(&client, false);
    let mut input = EventInput {
        title: "Planning".into(),
        start: 1_795_251_600,
        end: 1_795_255_200,
        repeat: "weekly".into(),
        calendar: Some(calendar.id.clone()),
        ..Default::default()
    };
    let event = events::create_event(&conn, &input).unwrap();
    let id = event["event"]["id"].as_str().unwrap();
    let queued = store::object_for_event(&conn, id).unwrap().unwrap();
    let ics = crate::store::event(&conn, id).unwrap().unwrap().ics;
    let edited = ics.replace("SUMMARY:Planning", "SUMMARY:Updated planning");
    let remote = ics.replace("SUMMARY:Planning", "SUMMARY:Edited on another device");
    let delta = |token: &str, props: &str| {
        format!(
            r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response>
        <d:href>{canonical}</d:href>{props}</d:response><d:sync-token>{token}</d:sync-token></d:multistatus>"#
        )
    };
    let propstat = |etag: &str, data: &str| {
        format!(
            r#"<d:propstat><d:prop><d:getetag>"{etag}"</d:getetag>{data}</d:prop>
        <d:status>HTTP/1.1 200 OK</d:status></d:propstat>"#
        )
    };
    let received = serve_google(
        listener,
        vec![
            ("207 Multi-Status", "".into(), google_properties("one")),
            ("404 Not Found", "".into(), "".into()),
            (
                "207 Multi-Status",
                "".into(),
                r#"<d:multistatus xmlns:d="DAV:"/>"#.into(),
            ),
            (
                "201 Created",
                format!("Location: {location}\r\n"),
                "".into(),
            ),
            (
                "207 Multi-Status",
                "".into(),
                delta("first", &propstat("1", "")),
            ),
            (
                "207 Multi-Status",
                "".into(),
                delta(
                    "first",
                    &propstat(
                        "1",
                        &format!("<c:calendar-data><![CDATA[{ics}]]></c:calendar-data>"),
                    ),
                ),
            ),
            ("207 Multi-Status", "".into(), google_properties("one")),
            ("204 No Content", "ETag: \"2\"\r\n".into(), "".into()),
            (
                "207 Multi-Status",
                "".into(),
                delta("second", &propstat("2", "")),
            ),
            (
                "207 Multi-Status",
                "".into(),
                delta(
                    "second",
                    &propstat(
                        "2",
                        &format!("<c:calendar-data><![CDATA[{edited}]]></c:calendar-data>"),
                    ),
                ),
            ),
            ("207 Multi-Status", "".into(), google_properties("two")),
            ("412 Precondition Failed", "".into(), "".into()),
            ("200 OK", "ETag: \"3\"\r\n".into(), remote.clone()),
            (
                "207 Multi-Status",
                "".into(),
                delta("third", &propstat("3", "")),
            ),
            (
                "207 Multi-Status",
                "".into(),
                delta(
                    "third",
                    &propstat(
                        "3",
                        &format!("<c:calendar-data><![CDATA[{remote}]]></c:calendar-data>"),
                    ),
                ),
            ),
            ("207 Multi-Status", "".into(), google_properties("three")),
            ("204 No Content", "".into(), "".into()),
            (
                "207 Multi-Status",
                "".into(),
                delta("fourth", "<d:status>HTTP/1.1 404 Not Found</d:status>"),
            ),
        ],
    );

    assert_eq!(synced(&conn, &client).pushed, 1);
    let created = store::object_for_event(&conn, id).unwrap().unwrap();
    assert!(created.url.ends_with(&location));
    assert_eq!(created.etag, "\"1\"");
    assert!(
        store::object(&conn, &calendar.id, &queued.url)
            .unwrap()
            .is_none()
    );
    assert_eq!(crate::store::all_events(&conn).unwrap().len(), 1);

    input.title = "Intermediate planning".into();
    events::update_event(&conn, id, &input).unwrap();
    input.title = "Updated planning".into();
    events::update_event(&conn, id, &input).unwrap();
    assert_eq!(synced(&conn, &client).pushed, 1);
    assert_eq!(
        store::object_for_event(&conn, id).unwrap().unwrap().etag,
        "\"2\""
    );

    input.title = "Local stale edit".into();
    events::update_event(&conn, id, &input).unwrap();
    assert_eq!(synced(&conn, &client).pushed, 0);
    assert_eq!(
        crate::store::event(&conn, id).unwrap().unwrap().title,
        "Edited on another device"
    );

    events::delete_event(&conn, id).unwrap();
    assert_eq!(synced(&conn, &client).pushed, 1);
    assert!(store::objects(&conn, &calendar.id).unwrap().is_empty());
    assert!(crate::store::all_events(&conn).unwrap().is_empty());

    let requests: Vec<_> = (0..18)
        .map(|_| received.recv_timeout(Duration::from_secs(2)).unwrap())
        .collect();
    let creates = &requests[3];
    assert!(creates.starts_with("PUT "));
    assert!(!creates.to_ascii_lowercase().contains("if-none-match:"));
    assert!(requests[2].contains("calendar-query"));
    let edit = &requests[7];
    assert!(edit.starts_with(&format!("PUT {location} ")));
    assert!(edit.to_ascii_lowercase().contains("if-match: \"1\""));
    assert!(edit.contains("SUMMARY:Updated planning"));
    assert!(!edit.contains("SUMMARY:Intermediate planning"));
    assert!(edit.contains("RRULE:FREQ=WEEKLY"));
    let delete = &requests[16];
    assert!(delete.starts_with(&format!("DELETE {location} ")));
    assert!(delete.to_ascii_lowercase().contains("if-match: \"3\""));
}

#[test]
fn google_throttle_pauses_the_queue_until_the_stored_deadline() {
    use crate::calendar::events::{self, EventInput};
    use std::time::Duration;

    let (client, listener) = google_fixture();
    let (conn, calendar) = google_store(&client, false);
    for title in ["a", "b", "c"] {
        let event = events::create_event(
            &conn,
            &EventInput {
                title: title.into(),
                start: 1_795_251_600,
                end: 1_795_255_200,
                calendar: Some(calendar.id.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        let id = event["event"]["id"].as_str().unwrap();
        let mut object = store::object_for_event(&conn, id).unwrap().unwrap();
        store::delete_object(&conn, &calendar.id, &object.url).unwrap();
        object.url = format!("{}/{title}.ics", client.server);
        object.etag = "\"1\"".into();
        object.state = store::CHANGED.into();
        store::save_object(&conn, &object).unwrap();
    }

    let received = serve_google(
        listener,
        vec![
            ("207 Multi-Status", "".into(), google_properties("one")),
            ("204 No Content", "ETag: \"2\"\r\n".into(), "".into()),
            ("429 Too Many Requests", "Retry-After: 120\r\n".into(), "".into()),
            ("207 Multi-Status", "".into(), google_properties("two")),
            ("204 No Content", "ETag: \"2\"\r\n".into(), "".into()),
            ("204 No Content", "ETag: \"2\"\r\n".into(), "".into()),
            (
                "207 Multi-Status", "".into(),
                r#"<d:multistatus xmlns:d="DAV:"><d:sync-token>next</d:sync-token></d:multistatus>"#.into(),
            ),
        ],
    );
    // Start with a sync token so the empty test delta doesn't remove events.
    store::set_synced(&conn, &calendar.id, "one", "previous", false).unwrap();
    let before = crate::store::now_unix();
    let report = sync::sync_account(&conn, &client, "dav-1");
    assert_eq!(report.pushed, 1);
    assert!(report.error.contains("rate limiting"));
    let account = store::account(&conn, "dav-1").unwrap().unwrap();
    assert!(account.retry_at >= before + 120);
    assert_eq!(account.throttle_count, 1);
    assert_eq!(store::pending(&conn, &calendar.id).unwrap().len(), 2);
    let paused = sync::sync_account(&conn, &client, "dav-1");
    assert!(paused.error.starts_with(&report.error));
    assert!(paused.error.contains("Try again in about"));

    store::record_sync_with_retry(&conn, "dav-1", &report.error, before - 1, 1).unwrap();
    store::record_sync(&conn, "dav-1", "Token refresh failed").unwrap();
    let account = store::account(&conn, "dav-1").unwrap().unwrap();
    assert_eq!((account.retry_at, account.throttle_count), (0, 1));
    assert_eq!(synced(&conn, &client).pushed, 2);
    let account = store::account(&conn, "dav-1").unwrap().unwrap();
    assert_eq!((account.retry_at, account.throttle_count), (0, 0));
    assert!(store::pending(&conn, &calendar.id).unwrap().is_empty());
    let requests: Vec<_> = (0..7)
        .map(|_| received.recv_timeout(Duration::from_secs(2)).unwrap())
        .collect();
    assert_eq!(requests.len(), 7);
    assert!(requests[1].starts_with(&format!("PUT {GOOGLE_COLLECTION}/a.ics ")));
    assert!(requests[2].starts_with(&format!("PUT {GOOGLE_COLLECTION}/b.ics ")));
    assert!(requests[3].starts_with("PROPFIND "));
    assert_eq!(requests[2], requests[4]);
    assert!(requests[5].starts_with(&format!("PUT {GOOGLE_COLLECTION}/c.ics ")));
    assert!(requests[6].starts_with("REPORT "));
}

#[test]
fn google_only_treats_quota_forbidden_responses_as_throttled() {
    let (client, listener) = google_fixture();
    let _requests = serve_google(
        listener,
        vec![
            (
                "403 Forbidden",
                "".into(),
                r#"{"error":{"errors":[{"domain":"usageLimits","reason":"userRateLimitExceeded"}]}}"#.into(),
            ),
            (
                "403 Forbidden",
                "".into(),
                r#"{"error":{"errors":[{"domain":"usageLimits","reason":"accessNotConfigured"}]}}"#.into(),
            ),
            (
                "403 Forbidden",
                "".into(),
                r#"<d:error xmlns:d="DAV:"><d:need-privileges/></d:error>"#.into(),
            ),
        ],
    );
    assert!(client.primary_calendar().unwrap_err().is::<RateLimited>());
    assert!(!client.primary_calendar().unwrap_err().is::<RateLimited>());
    assert!(!client.primary_calendar().unwrap_err().is::<RateLimited>());
    assert_eq!(
        retry_after_seconds("Thu, 01 Jan 1970 00:02:00 GMT", 10),
        Some(110)
    );
    assert_eq!(retry_after_seconds("nonsense", 10), None);
    assert_eq!(
        retry_after_seconds("9999999999999999999999", 10),
        Some(86_400)
    );
    assert_eq!(
        retry_after_seconds("Sat, 03 Jan 1970 00:00:00 GMT", 10),
        Some(86_400)
    );
}

#[test]
fn google_create_retry_recovers_an_event_at_another_url() {
    let (client, listener) = google_fixture();
    let ics =
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:fictional-retry\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let canonical = format!("{GOOGLE_COLLECTION}/already-created.ics");
    let received = serve_google(
        listener,
        vec![
            ("404 Not Found", "".into(), "".into()),
            (
                "207 Multi-Status",
                "".into(),
                format!(
                    r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:href>{canonical}</d:href>
            <d:propstat><d:prop><c:calendar-data><![CDATA[{ics}]]></c:calendar-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
            </d:response></d:multistatus>"#
                ),
            ),
        ],
    );
    let queued = format!("{}/fictional-retry.ics", client.server);
    let written = client.put(&queued, ics, None).unwrap();
    assert!(written.url.ends_with(&canonical));
    assert!(written.etag.is_empty());
    assert_eq!(received.into_iter().count(), 2);
}

/// Round trip against a live server, such as the Stalwart in `~/repo/mservers`:
///
/// ```sh
/// MERON_TEST_CALDAV_URL=http://127.0.0.1:4080 \
/// MERON_TEST_CALDAV_USER=user1@mail.localhost MERON_TEST_CALDAV_PASSWORD=user1password \
/// cargo test caldav::tests::live -- --nocapture
/// ```
#[test]
fn live_round_trip() {
    let (Ok(server), Ok(user), Ok(password)) = (
        std::env::var("MERON_TEST_CALDAV_URL"),
        std::env::var("MERON_TEST_CALDAV_USER"),
        std::env::var("MERON_TEST_CALDAV_PASSWORD"),
    ) else {
        return;
    };
    let _live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    let client = Client::new(&server, &user, &password, None).unwrap();
    let wrong = Client::new(&server, &user, "wrong-password", None).unwrap();
    assert!(wrong.discover().unwrap_err().is::<AuthFailed>());

    let found = client.discover().unwrap();
    let calendar = found
        .calendars
        .iter()
        .find(|c| c.supports("VEVENT") && !c.read_only)
        .expect("a writable calendar")
        .clone();
    let start = client.changes(&calendar.url, "").unwrap();

    let uid = format!("meron-test-{}", uuid::Uuid::new_v4());
    let url = Url::parse(&calendar.url)
        .unwrap()
        .join(&format!("{uid}.ics"))
        .unwrap()
        .to_string();
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Meron//Test//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261119T080000Z\r\nDTSTART:20261120T090000Z\r\nDTEND:20261120T100000Z\r\nSUMMARY:Round trip\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    );
    client.put(&url, &ics, None).unwrap();
    assert!(
        client
            .put(&url, &ics, None)
            .unwrap_err()
            .is::<PreconditionFailed>()
    );

    let delta = client.changes(&calendar.url, &start.sync_token).unwrap();
    let entry = delta
        .changed
        .iter()
        .find(|e| e.url == url)
        .expect("new event in changes")
        .clone();
    assert!(client.list(&calendar.url).unwrap().contains(&entry));
    let fetched = client.fetch(&calendar.url, &[url.clone()]).unwrap().objects;
    assert_eq!(fetched.len(), 1);
    assert!(fetched[0].ics.contains("SUMMARY:Round trip"));
    assert_eq!(fetched[0].etag, entry.etag);

    let edited = ics.replace("Round trip", "Edited");
    assert!(
        client
            .put(&url, &edited, Some("\"stale\""))
            .unwrap_err()
            .is::<PreconditionFailed>()
    );
    client.put(&url, &edited, Some(&entry.etag)).unwrap();
    assert!(
        client
            .get(&url)
            .unwrap()
            .unwrap()
            .ics
            .contains("SUMMARY:Edited")
    );

    client.delete(&url, None).unwrap();
    let after = client.changes(&calendar.url, &delta.sync_token).unwrap();
    assert!(after.removed.contains(&url));
    assert!(client.get(&url).unwrap().is_none());
    // A calendar made, renamed and recoloured here shows so on the server,
    // made inside the home even when its URL lacks the trailing slash.
    let made = client
        .make_calendar(found.home.trim_end_matches('/'), "Meron test", "#14B8A6")
        .unwrap();
    assert!(
        made.starts_with(&found.home),
        "{made} not in {}",
        found.home
    );
    client
        .update_calendar(&made, Some("Meron test renamed"), Some("#8B5CF6"))
        .unwrap();
    let listed = client.calendars(&found.home).unwrap();
    let made_calendar = listed
        .iter()
        .find(|c| Url::parse(&c.url).unwrap() == Url::parse(&made).unwrap())
        .expect("new calendar listed");
    assert_eq!(made_calendar.name, "Meron test renamed");
    assert_eq!(made_calendar.color.to_ascii_lowercase(), "#8b5cf6");
    assert!(made_calendar.supports("VEVENT"));
    client.delete_calendar(&made).unwrap();
    assert!(
        client
            .calendars(&found.home)
            .unwrap()
            .iter()
            .all(|c| c.url != made_calendar.url)
    );

    // RFC 6578 wants an error for a token the server does not know; Stalwart
    // answers with a full listing instead. Either is something sync handles.
    if let Err(err) = client.changes(&calendar.url, "bogus-token") {
        assert!(err.is::<SyncTokenInvalid>(), "{err:#}");
    }
}

/// The live tests share one calendar on the server; one at a time.
static LIVE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A sync that must succeed.
fn synced(conn: &rusqlite::Connection, client: &Client) -> sync::Report {
    let report = sync::sync_account(conn, client, "dav-1");
    assert!(report.error.is_empty(), "{}", report.error);
    report
}

/// Full sync both ways against a live server (see [`live_round_trip`]).
#[test]
fn live_sync() {
    use crate::calendar::events::{self, EventInput};
    let (Ok(server), Ok(user), Ok(password)) = (
        std::env::var("MERON_TEST_CALDAV_URL"),
        std::env::var("MERON_TEST_CALDAV_USER"),
        std::env::var("MERON_TEST_CALDAV_PASSWORD"),
    ) else {
        return;
    };
    let _live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    store::save_account(
        &conn,
        &store::DavAccount {
            id: "dav-1".into(),
            server_url: server.clone(),
            username: user.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let client = Client::new(&server, &user, &password, None).unwrap();
    let synced_event = |title: &str| {
        crate::store::all_events(&conn)
            .unwrap()
            .into_iter()
            .find(|e| e.title == title)
    };

    // Something already on the server comes down.
    let calendar_url = client
        .discover()
        .unwrap()
        .calendars
        .into_iter()
        .find(|c| c.supports("VEVENT") && !c.read_only)
        .unwrap()
        .url;
    let tag = uuid::Uuid::new_v4().to_string();
    let a_title = format!("From server {tag}");
    let a_url = format!("{calendar_url}a-{tag}.ics");
    let a_ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Other//EN\r\nBEGIN:VEVENT\r\nUID:a-{tag}\r\nDTSTAMP:20261119T080000Z\r\nDTSTART:20261121T090000Z\r\nDTEND:20261121T100000Z\r\nSUMMARY:{a_title}\r\nX-OTHER-CLIENT:keep\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    );
    client.put(&a_url, &a_ics, None).unwrap();
    let report = synced(&conn, &client);
    assert!(report.pulled >= 1, "{report:?}");
    let a = synced_event(&a_title).expect("server event synced");
    let calendar_id = a.calendar.clone();
    assert!(!calendar_id.is_empty());
    assert_eq!(a.start_at, 1_795_251_600);
    assert_eq!(synced(&conn, &client), sync::Report::default());

    // An event made here goes up.
    let b_title = format!("From Meron {tag}");
    let created = events::create_event(
        &conn,
        &EventInput {
            title: b_title.clone(),
            start: 1_795_338_000,
            end: 1_795_341_600,
            calendar: Some(calendar_id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let b_id = created["event"]["id"].as_str().unwrap().to_string();
    assert_eq!(synced(&conn, &client).pushed, 1);
    let b_object = store::object_for_event(&conn, &b_id).unwrap().unwrap();
    assert!(b_object.state.is_empty());
    let on_server = client.get(&b_object.url).unwrap().expect("pushed");
    assert!(on_server.ics.contains(&b_title));

    // An edit here goes up without losing what another client wrote.
    events::update_event(
        &conn,
        &a.id,
        &EventInput {
            title: format!("{a_title} edited"),
            start: a.start_at,
            end: a.end_at,
            ..Default::default()
        },
    )
    .unwrap();
    synced(&conn, &client);
    let a_server = client.get(&a_url).unwrap().unwrap();
    assert!(a_server.ics.contains(&format!("SUMMARY:{a_title} edited")));
    assert!(a_server.ics.contains("X-OTHER-CLIENT:keep"));

    // An edit on the server comes down.
    client
        .put(
            &a_url,
            &a_server.ics.replace(" edited", " renamed"),
            Some(&a_server.etag),
        )
        .unwrap();
    assert_eq!(synced(&conn, &client).pulled, 1);
    assert!(synced_event(&format!("{a_title} renamed")).is_some());

    // Deletions go both ways.
    events::delete_event(&conn, &b_id).unwrap();
    client.delete(&a_url, None).unwrap();
    let report = synced(&conn, &client);
    assert_eq!(report.pushed, 1);
    assert!(client.get(&b_object.url).unwrap().is_none());
    assert!(synced_event(&format!("{a_title} renamed")).is_none());
    assert!(
        store::objects(&conn, &calendar_id)
            .unwrap()
            .iter()
            .all(|o| o.url != a_url)
    );

    // Turning the calendar off forgets it here only.
    store::set_enabled(&conn, &calendar_id, false).unwrap();
    assert!(
        crate::store::all_events(&conn)
            .unwrap()
            .iter()
            .all(|e| e.calendar.is_empty())
    );
}
