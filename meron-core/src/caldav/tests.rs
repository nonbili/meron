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
