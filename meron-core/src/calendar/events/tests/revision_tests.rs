use super::*;

const MAYA: &str = "maya@example.com";

fn revision(sequence: i64, start: &str, summary: &str) -> Invite {
    crate::calendar::parse_invite(&format!(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-u\r\n\
         SEQUENCE:{sequence}\r\nDTSTART:{start}\r\nDTEND:{start}\r\nSUMMARY:{summary}\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    ))
    .unwrap()
}

#[test]
fn an_organizer_update_moves_the_filed_event_and_keeps_the_answer() {
    let conn = conn();
    let first = revision(0, "20261008T100000Z", "Review");
    record_invite_event(&conn, "acct", "t", "m1", &first, Response::Tentative).unwrap();
    let filed = store::invite_event(&conn, "acct", "inv-u")
        .unwrap()
        .unwrap();

    // The same revision read again changes nothing.
    apply_invite_update(&conn, "acct", MAYA, "m1", &first).unwrap();
    assert_eq!(
        store::invite_event(&conn, "acct", "inv-u")
            .unwrap()
            .unwrap(),
        filed
    );

    let moved = revision(1, "20261008T140000Z", "Review (moved)");
    apply_invite_update(&conn, "acct", MAYA, "m2", &moved).unwrap();
    let updated = store::invite_event(&conn, "acct", "inv-u")
        .unwrap()
        .unwrap();
    assert_eq!(updated.id, filed.id);
    assert_eq!(updated.title, "Review (moved)");
    assert_eq!(updated.start_at, moved.start);
    assert_eq!(updated.response, "tentative");
    assert_eq!(updated.thread_id, "t");
    assert_eq!(updated.message_id, "m2");

    // An older revision read afterwards does not move it back.
    apply_invite_update(&conn, "acct", MAYA, "m1", &first).unwrap();
    assert_eq!(
        store::invite_event(&conn, "acct", "inv-u")
            .unwrap()
            .unwrap()
            .start_at,
        moved.start
    );
}

#[test]
fn an_organizer_update_files_nothing_that_was_not_filed() {
    let conn = conn();
    apply_invite_update(
        &conn,
        "acct",
        MAYA,
        "m2",
        &revision(1, "20261008T140000Z", "Review"),
    )
    .unwrap();
    assert_eq!(store::invite_event(&conn, "acct", "inv-u").unwrap(), None);
}

#[test]
fn a_late_older_revision_of_an_occurrence_does_not_undo_a_newer_one() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series =
        crate::calendar::parse_invite(&SERIES.replace("RRULE:", "SEQUENCE:0\r\nRRULE:")).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let revision = |sequence: i64, hour: i64| {
        occurrence_at(
            "20261015T170000Z",
            &format!(
                "SEQUENCE:{sequence}\r\nDTSTART:20261015T{hour}0000Z\r\nDTEND:20261015T{}0000Z\r\n",
                hour + 1
            ),
        )
    };
    apply_invite_update(&conn, "acct", MAYA, "m2", &revision(2, 20)).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);

    apply_invite_update(&conn, "acct", MAYA, "m1", &revision(1, 19)).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);
}

#[test]
fn an_update_from_someone_other_than_the_organizer_is_ignored() {
    let conn = conn();
    record_invite_event(
        &conn,
        "acct",
        "t",
        "m1",
        &revision(0, "20261008T100000Z", "Review"),
        Response::Accepted,
    )
    .unwrap();
    let mut forged = revision(1, "20261008T140000Z", "Moved by someone else");
    // The organizer's copy, sent by someone else.
    apply_invite_update(&conn, "acct", "mallory@example.com", "m2", &forged).unwrap();
    // Or naming someone else the organizer.
    forged.organizer.addr = "mallory@example.com".into();
    apply_invite_update(&conn, "acct", "mallory@example.com", "m2", &forged).unwrap();
    let filed = store::invite_event(&conn, "acct", "inv-u")
        .unwrap()
        .unwrap();
    assert_eq!(filed.title, "Review");
}

/// The series with its second week moved to 20:00 at `sequence`, embedded.
fn series_moving_second(sequence: i64, hour: i64) -> Invite {
    crate::calendar::parse_invite(&format!(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\nSEQUENCE:0\r\n\
         DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Review\r\n\
         RRULE:FREQ=WEEKLY;COUNT=3\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:inv-s\r\nRECURRENCE-ID:20261015T170000Z\r\nSEQUENCE:{sequence}\r\n\
         DTSTART:20261015T{hour}0000Z\r\nDTEND:20261015T{}0000Z\r\nSUMMARY:Review\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        hour + 1
    ))
    .unwrap()
}

#[test]
fn a_series_update_that_revises_only_an_embedded_occurrence_is_applied() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series =
        crate::calendar::parse_invite(&SERIES.replace("RRULE:", "SEQUENCE:0\r\nRRULE:")).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    apply_invite_update(&conn, "acct", MAYA, "m2", &series_moving_second(1, 20)).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);
}

#[test]
fn a_series_embedding_an_older_occurrence_keeps_the_newer_one() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    record_invite_event(
        &conn,
        "acct",
        "t",
        "m",
        &series_moving_second(2, 20),
        Response::Accepted,
    )
    .unwrap();
    // The series itself moves on, but its copy of the second week is older.
    let mut later = series_moving_second(1, 19);
    later.ics = later
        .ics
        .replacen("SEQUENCE:0\r\n", "SEQUENCE:3\r\nLOCATION:Room 2\r\n", 1);
    later.sequence = 3;
    record_invite_event(&conn, "acct", "t", "m", &later, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);
    let filed = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert!(filed.ics.contains("Room 2"));
}

#[test]
fn a_newer_occurrence_does_not_carry_an_older_series_with_it() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(
        &SERIES.replace("RRULE:", "SEQUENCE:2\r\nLOCATION:Room 2\r\nRRULE:"),
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    // Series at 0, its second week at 3: the series part is stale.
    apply_invite_update(&conn, "acct", MAYA, "m2", &series_moving_second(3, 20)).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1), week(2)]);
    let filed = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert!(filed.ics.contains("Room 2"));
}

#[test]
fn an_update_downloaded_before_the_answer_is_applied_once_filed() {
    let conn = conn();
    let first = revision(0, "20261008T100000Z", "Review");
    let moved = revision(1, "20261008T140000Z", "Review (moved)");
    store::save_cached_message(
        &conn,
        "acct",
        "INBOX",
        2,
        &crate::parse::Message {
            from_addr: MAYA.into(),
            message_id: "m2".into(),
            invite: Some(moved.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m1", &first, Response::Accepted).unwrap();
    apply_cached_updates(&conn, "acct", "inv-u").unwrap();
    let filed = store::invite_event(&conn, "acct", "inv-u")
        .unwrap()
        .unwrap();
    assert_eq!(filed.title, "Review (moved)");
    assert_eq!(filed.start_at, moved.start);
}

#[test]
fn an_unversioned_embedded_occurrence_counts_as_revision_zero() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    record_invite_event(
        &conn,
        "acct",
        "t",
        "m",
        &series_moving_second(2, 20),
        Response::Accepted,
    )
    .unwrap();
    // A series at 3 whose copy of the second week has no SEQUENCE at all.
    let mut later = series_moving_second(0, 19);
    later.ics = later
        .ics
        .replacen("SEQUENCE:0\r\n", "SEQUENCE:3\r\n", 1)
        .replacen("SEQUENCE:0\r\n", "", 1);
    later.sequence = 3;
    record_invite_event(&conn, "acct", "t", "m", &later, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);
}

#[test]
fn a_series_update_keeps_the_answers_given_to_single_dates() {
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series =
        crate::calendar::parse_invite(&SERIES.replace("RRULE:", "SEQUENCE:0\r\nRRULE:")).unwrap();
    let second = occurrence_at(
        "20261015T170000Z",
        "SEQUENCE:0\r\nDTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );

    // Declined: the organizer's revision of that date does not bring it back.
    let conn = conn();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    store::record_invite_response(&conn, "acct", "inv-s", second.recurrence_id, "declined", 0)
        .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
    apply_invite_update(&conn, "acct", MAYA, "m2", &series_moving_second(1, 20)).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);

    // Tentative: the moved date keeps that answer.
    let conn = self::conn();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    store::record_invite_response(&conn, "acct", "inv-s", second.recurrence_id, "tentative", 0)
        .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Tentative).unwrap();
    apply_invite_update(&conn, "acct", MAYA, "m2", &series_moving_second(1, 20)).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    assert_eq!(payload["events"][1]["response"], "tentative", "{payload}");
    assert_eq!(payload["events"][0]["response"], "accepted");
}

#[test]
fn an_occurrence_update_does_not_pin_the_series_answer_to_that_date() {
    let conn = conn();
    let series =
        crate::calendar::parse_invite(&SERIES.replace("RRULE:", "SEQUENCE:0\r\nRRULE:")).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let moved = occurrence_at(
        "20261015T170000Z",
        "SEQUENCE:1\r\nDTSTART:20261015T200000Z\r\nDTEND:20261015T210000Z\r\n",
    );
    apply_invite_update(&conn, "acct", MAYA, "m2", &moved).unwrap();
    // Answering the series again carries to the moved date too.
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Tentative).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    let responses: Vec<_> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["response"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(responses, vec!["tentative"; 3], "{payload}");
}

#[test]
fn an_invitation_turned_all_day_drops_alerts_counted_in_minutes() {
    let conn = conn();
    let timed = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-2\r\n\
         DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Offsite\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "", "inv@x", &timed, Response::Accepted).unwrap();
    let event = store::invite_event(&conn, "acct", "inv-2")
        .unwrap()
        .unwrap();
    let alerts = EventInput {
        alerts: Some(Some(vec![60])),
        ..input("Ignored", 1, 2)
    };
    let value = update_event(&conn, &event.id, &alerts).unwrap();
    assert_eq!(value["event"]["alerts"], json!([60]));
    let all_day = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-2\r\nSEQUENCE:1\r\n\
         DTSTART;VALUE=DATE:20261008\r\nDTEND;VALUE=DATE:20261009\r\nSUMMARY:Offsite\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    assert!(all_day.all_day);
    record_invite_event(&conn, "acct", "", "inv@x", &all_day, Response::Accepted).unwrap();
    let event = store::invite_event(&conn, "acct", "inv-2")
        .unwrap()
        .unwrap();
    assert!(event.all_day);
    assert_eq!(event.alerts, None);
}
