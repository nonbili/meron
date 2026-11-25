use super::*;

/// `me`'s primary Google calendar, holding `ics` as a sync would file it.
fn synced_google_event(conn: &Connection, me: &str, ics: &str) {
    synced_google_event_in(conn, me, me, ics);
}

/// The Google calendar `calendar_id` synced by `me`'s account, holding
/// `ics` as a sync would file it.
fn synced_google_event_in(conn: &Connection, me: &str, calendar_id: &str, ics: &str) {
    caldav::store::save_account(
        conn,
        &caldav::store::DavAccount {
            id: "dav-1".into(),
            username: me.into(),
            google: true,
            ..Default::default()
        },
    )
    .unwrap();
    let calendar = caldav::store::reconcile_calendars(
        conn,
        "dav-1",
        &[caldav::Calendar {
            url: format!("https://apidata.googleusercontent.com/caldav/v2/{calendar_id}/events"),
            components: vec!["VEVENT".into()],
            ..Default::default()
        }],
    )
    .unwrap()
    .remove(0);
    let mut event = caldav::ics::event_from_ics(ics).unwrap();
    event.id = crate::tasks::new_id("event");
    event.calendar = calendar.id;
    store::save_event(conn, &event).unwrap();
}

fn google_meeting(my_partstat: &str, extra: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Google Inc//Google Calendar 70.9054//EN\r\n\
         BEGIN:VEVENT\r\nUID:meet-1@google.com\r\nDTSTART:20261008T170000Z\r\n\
         DTEND:20261008T180000Z\r\nSUMMARY:Planning\r\n{extra}\
         ORGANIZER;CN=Maya:mailto:maya@example.com\r\n\
         ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;CN=Maya:mailto:maya@example.com\r\n\
         ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT={my_partstat};CN=Sam:mailto:Sam@Example.test\r\n\
         END:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

#[test]
fn a_synced_event_the_user_declined_is_hidden() {
    let conn = conn();
    // A login typed with capitals is the same address.
    synced_google_event(&conn, "Sam@Example.test", &google_meeting("DECLINED", ""));
    let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert_eq!(keys(&payload), vec![], "{payload}");
}

#[test]
fn a_synced_event_the_user_accepted_or_someone_else_declined_shows() {
    for (me, partstat) in [
        ("sam@example.test", "ACCEPTED"),
        ("sam@example.test", "NEEDS-ACTION"),
        // Another attendee's answer is not the user's.
        ("other@example.test", "DECLINED"),
    ] {
        let conn = conn();
        synced_google_event(&conn, me, &google_meeting(partstat, ""));
        let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
        assert_eq!(
            keys(&payload),
            vec![("Planning".into(), OCT_1 + 7 * DAY + 17 * 3600)],
            "{me} {partstat}: {payload}"
        );
    }
}

#[test]
fn a_synced_series_the_user_declined_is_hidden_but_for_a_date_accepted_since() {
    let conn = conn();
    let mut ics = google_meeting("DECLINED", "RRULE:FREQ=WEEKLY;COUNT=3\r\n");
    // Google keeps one date the user accepted on its own as an override.
    ics = ics.replace(
        "END:VCALENDAR",
        "BEGIN:VEVENT\r\nUID:meet-1@google.com\r\nRECURRENCE-ID:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nSUMMARY:Planning\r\n\
         ORGANIZER;CN=Maya:mailto:maya@example.com\r\n\
         ATTENDEE;PARTSTAT=ACCEPTED:mailto:sam@example.test\r\n\
         END:VEVENT\r\nEND:VCALENDAR",
    );
    synced_google_event(&conn, "sam@example.test", &ics);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    assert_eq!(
        keys(&payload),
        vec![("Planning".into(), OCT_1 + 14 * DAY + 17 * 3600)],
        "{payload}"
    );
}

#[test]
fn a_synced_occurrence_the_user_declined_leaves_the_rest_of_the_series() {
    let conn = conn();
    let mut ics = google_meeting("ACCEPTED", "RRULE:FREQ=WEEKLY;COUNT=3\r\n");
    ics = ics.replace(
        "END:VCALENDAR",
        "BEGIN:VEVENT\r\nUID:meet-1@google.com\r\nRECURRENCE-ID:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nSUMMARY:Planning\r\n\
         ORGANIZER;CN=Maya:mailto:maya@example.com\r\n\
         ATTENDEE;PARTSTAT=DECLINED:mailto:sam@example.test\r\n\
         END:VEVENT\r\nEND:VCALENDAR",
    );
    synced_google_event(&conn, "sam@example.test", &ics);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    assert_eq!(
        keys(&payload),
        vec![
            ("Planning".into(), OCT_1 + 7 * DAY + 17 * 3600),
            ("Planning".into(), OCT_1 + 21 * DAY + 17 * 3600),
        ],
        "{payload}"
    );
}

#[test]
fn a_synced_event_declined_as_a_send_as_alias_is_hidden() {
    let conn = conn();
    conn.execute(
        "INSERT INTO accounts (id, email, prefs) VALUES ('mail-1', 'sam@example.test', ?1)",
        [json!({ "aliases": [{ "email": "Sam@Alias.example", "name": "Sam" }] }).to_string()],
    )
    .unwrap();
    let ics = google_meeting("ACCEPTED", "").replace(
        "PARTSTAT=ACCEPTED;CN=Sam:mailto:Sam@Example.test",
        "PARTSTAT=DECLINED;CN=Sam:mailto:sam@alias.example",
    );
    synced_google_event(&conn, "Sam@Example.test", &ics);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert_eq!(keys(&payload), vec![], "{payload}");
}

#[test]
fn a_synced_series_the_user_has_not_declined_shows_every_date() {
    for partstat in ["NEEDS-ACTION", "TENTATIVE", "ACCEPTED"] {
        let conn = conn();
        // Someone else declining is not the user declining.
        let ics = google_meeting(
            partstat,
            "RRULE:FREQ=WEEKLY;COUNT=3\r\nATTENDEE;PARTSTAT=DECLINED:mailto:lee@example.com\r\n",
        );
        synced_google_event(&conn, "sam@example.test", &ics);
        let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
        assert_eq!(keys(&payload).len(), 3, "{partstat}: {payload}");
    }
}

#[test]
fn a_synced_lone_date_of_a_series_the_user_declined_is_hidden() {
    let conn = conn();
    // Invited to one date only: the object holds that date's override alone.
    let ics = google_meeting("DECLINED", "RECURRENCE-ID:20261008T170000Z\r\n");
    synced_google_event(&conn, "sam@example.test", &ics);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert_eq!(keys(&payload), vec![], "{payload}");
}

#[test]
fn a_synced_event_the_user_declined_in_someone_elses_calendar_shows() {
    // A colleague's calendar shared with the user: the meeting is still on
    // for its owner, whatever the user answered.
    for calendar in ["maya@example.com", "team@group.calendar.google.com"] {
        let conn = conn();
        synced_google_event_in(
            &conn,
            "sam@example.test",
            calendar,
            &google_meeting("DECLINED", ""),
        );
        let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
        assert_eq!(keys(&payload).len(), 1, "{calendar}: {payload}");
    }
}

#[test]
fn a_synced_event_declined_in_a_primary_calendar_with_an_encoded_id_is_hidden() {
    let conn = conn();
    synced_google_event_in(
        &conn,
        "sam@example.test",
        "Sam%40Example.test",
        &google_meeting("DECLINED", ""),
    );
    let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert_eq!(keys(&payload), vec![], "{payload}");
}

#[test]
fn a_synced_event_declined_in_a_send_as_aliases_calendar_shows() {
    // The alias's own calendar, subscribed through the login's account, is
    // not the login's primary: the meeting may still be on for its owner.
    let conn = conn();
    conn.execute(
        "INSERT INTO accounts (id, email, prefs) VALUES ('mail-1', 'sam@example.test', ?1)",
        [json!({ "aliases": [{ "email": "sam@alias.example", "name": "Sam" }] }).to_string()],
    )
    .unwrap();
    let ics = google_meeting("ACCEPTED", "").replace(
        "PARTSTAT=ACCEPTED;CN=Sam:mailto:Sam@Example.test",
        "PARTSTAT=DECLINED;CN=Sam:mailto:sam@alias.example",
    );
    synced_google_event_in(&conn, "sam@example.test", "sam@alias.example", &ics);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert_eq!(keys(&payload).len(), 1, "{payload}");
}
