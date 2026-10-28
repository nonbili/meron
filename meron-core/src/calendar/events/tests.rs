use super::*;

fn conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn
}

fn input(title: &str, start: i64, end: i64) -> EventInput {
    EventInput {
        title: title.into(),
        start,
        end,
        ..Default::default()
    }
}

fn keys(payload: &Value) -> Vec<(String, i64)> {
    payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["title"].as_str().unwrap().to_string(),
                item["start"].as_i64().unwrap(),
            )
        })
        .collect()
}

// 2026-10-01T00:00Z, a Thursday.
const OCT_1: i64 = 1_790_812_800;

#[test]
fn a_range_holds_the_events_that_overlap_it() {
    let conn = conn();
    create_event(&conn, &input("Before", OCT_1 - 7200, OCT_1 - 3600)).unwrap();
    create_event(&conn, &input("Across", OCT_1 - 3600, OCT_1 + 3600)).unwrap();
    create_event(&conn, &input("Inside", OCT_1 + 7200, OCT_1 + 9000)).unwrap();
    create_event(&conn, &input("Instant", OCT_1 + 10_000, 0)).unwrap();
    create_event(
        &conn,
        &input("After", OCT_1 + 31 * DAY, OCT_1 + 31 * DAY + 60),
    )
    .unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert_eq!(
        keys(&payload),
        vec![
            ("Across".into(), OCT_1 - 3600),
            ("Inside".into(), OCT_1 + 7200),
            ("Instant".into(), OCT_1 + 10_000),
        ]
    );
    assert_eq!(payload["events"][0]["source"], "event");
}

#[test]
fn an_all_day_event_covers_whole_utc_days() {
    let conn = conn();
    let created = create_event(
        &conn,
        &EventInput {
            title: "Trip".into(),
            start: OCT_1 + 3 * 3600,
            end: OCT_1 + DAY + 60,
            all_day: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(created["event"]["start"], OCT_1);
    assert_eq!(created["event"]["end"], OCT_1 + 2 * DAY);
    // No end: one day.
    let single = create_event(
        &conn,
        &EventInput {
            title: "Holiday".into(),
            start: OCT_1,
            all_day: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(single["event"]["end"], OCT_1 + DAY);
    assert!(create_event(&conn, &input("No start", 0, 0)).is_err());
}

#[test]
fn a_repeating_event_is_expanded_into_the_range() {
    let conn = conn();
    // Weekly from a Thursday a month before the range.
    create_event(
        &conn,
        &EventInput {
            title: "Sync".into(),
            start: OCT_1 - 28 * DAY + 3600,
            end: OCT_1 - 28 * DAY + 5400,
            repeat: "weekly".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 14 * DAY, false).unwrap();
    let starts: Vec<i64> = keys(&payload).into_iter().map(|(_, start)| start).collect();
    // Wall-clock time is kept in the device's zone, so a DST change between
    // the first week and these can move the instant by an hour.
    assert_eq!(starts.len(), 2, "{payload}");
    for (week, start) in starts.iter().enumerate() {
        let expected = OCT_1 + week as i64 * 7 * DAY + 3600;
        assert!((start - expected).abs() <= 3600, "{start} vs {expected}");
    }
    let items = payload["events"].as_array().unwrap();
    assert_eq!(items[0]["end"].as_i64().unwrap() - starts[0], 1800);
    assert_ne!(items[0]["key"], items[1]["key"]);
    assert_eq!(items[0]["repeat"], "weekly");

    let daily = create_event(
        &conn,
        &EventInput {
            title: "Standup".into(),
            start: OCT_1 + DAY,
            end: OCT_1 + DAY,
            all_day: true,
            repeat: "daily".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(daily["event"]["recurring"], true);
    let payload = events_payload(&conn, OCT_1, OCT_1 + 4 * DAY, false).unwrap();
    let standups: Vec<i64> = keys(&payload)
        .into_iter()
        .filter(|(title, _)| title == "Standup")
        .map(|(_, start)| start)
        .collect();
    assert_eq!(
        standups,
        vec![OCT_1 + DAY, OCT_1 + 2 * DAY, OCT_1 + 3 * DAY]
    );

    assert!(
        create_event(
            &conn,
            &EventInput {
                repeat: "hourly".into(),
                ..input("Bad", OCT_1, OCT_1)
            }
        )
        .is_err()
    );
}

#[test]
fn an_edit_and_a_delete_round_trip_through_restore() {
    let conn = conn();
    let created = create_event(&conn, &input("Draft", OCT_1, OCT_1 + 3600)).unwrap();
    let id = created["event"]["id"].as_str().unwrap().to_string();
    let updated = update_event(
        &conn,
        &id,
        &EventInput {
            notes: "Bring slides".into(),
            ..input("Review", OCT_1 + 3600, OCT_1 + 7200)
        },
    )
    .unwrap();
    assert_eq!(updated["event"]["title"], "Review");
    assert_eq!(updated["event"]["notes"], "Bring slides");
    assert_eq!(updated["event"]["start"], OCT_1 + 3600);

    let deleted = delete_event(&conn, &id).unwrap();
    assert_eq!(event_payload(&conn, &id).unwrap()["event"], Value::Null);
    restore_event(&conn, &deleted["restore"]).unwrap();
    assert_eq!(
        event_payload(&conn, &id).unwrap()["event"]["title"],
        "Review"
    );
}

#[test]
fn open_tasks_due_in_the_range_sit_on_their_day() {
    let conn = conn();
    let list = crate::tasks::create_list(&conn, "Work").unwrap();
    let list_id = list["list"]["id"].as_str().unwrap().to_string();
    // A due date is local midnight of its day.
    let due = chrono::Local
        .with_ymd_and_hms(2026, 10, 5, 0, 0, 0)
        .single()
        .unwrap()
        .timestamp();
    crate::tasks::create_task(&conn, &list_id, "File report", "", due, "", "", "").unwrap();
    let done = crate::tasks::create_task(&conn, &list_id, "Done", "", due, "", "", "").unwrap();
    crate::tasks::set_done(&conn, done["task"]["id"].as_str().unwrap(), true).unwrap();

    let without = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, false).unwrap();
    assert!(without["events"].as_array().unwrap().is_empty());
    let with = events_payload(&conn, OCT_1, OCT_1 + 31 * DAY, true).unwrap();
    let items = with["events"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["source"], "task");
    assert_eq!(items[0]["title"], "File report");
    assert_eq!(items[0]["all_day"], true);
    assert_eq!(items[0]["start"], OCT_1 + 4 * DAY);
    assert_eq!(items[0]["list_id"], list_id);
}

#[test]
fn an_accepted_invitation_is_filed_updated_and_declined_away() {
    let conn = conn();
    let mut invite = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-1\r\n\
         DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Review\r\n\
         RRULE:FREQ=WEEKLY;COUNT=3\r\nORGANIZER:mailto:maya@example.com\r\n\
         END:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(
        &conn,
        "acct",
        "acct#INBOX#t.x",
        "inv@x",
        &invite,
        Response::Tentative,
    )
    .unwrap();
    let event = store::invite_event(&conn, "acct", "inv-1")
        .unwrap()
        .unwrap();
    assert_eq!(event.response, "tentative");
    assert_eq!(event.thread_id, "acct#INBOX#t.x");
    assert!(event.recurring);

    // The invitation's own rule: three weeks, then nothing.
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    let items = payload["events"].as_array().unwrap();
    assert_eq!(items.len(), 3, "{payload}");
    assert_eq!(items[0]["source"], "invite");
    assert_eq!(items[0]["start"], OCT_1 + 7 * DAY + 17 * 3600);

    // Accepting an update keeps one event, and the user's notes on it.
    update_event(
        &conn,
        &event.id,
        &EventInput {
            notes: "Mine".into(),
            ..input("Ignored", 1, 2)
        },
    )
    .unwrap();
    invite.summary = "Review (moved)".into();
    record_invite_event(&conn, "acct", "", "inv@x", &invite, Response::Accepted).unwrap();
    let updated = store::invite_event(&conn, "acct", "inv-1")
        .unwrap()
        .unwrap();
    assert_eq!(updated.id, event.id);
    assert_eq!(updated.title, "Review (moved)");
    assert_eq!(updated.notes, "Mine");
    assert_eq!(updated.response, "accepted");
    assert_eq!(updated.thread_id, "acct#INBOX#t.x");

    record_invite_event(&conn, "acct", "", "inv@x", &invite, Response::Declined).unwrap();
    assert_eq!(store::invite_event(&conn, "acct", "inv-1").unwrap(), None);
}

#[test]
fn deleting_an_account_takes_its_invitations_and_unlinks_the_rest() {
    let conn = conn();
    let invite = crate::calendar::Invite {
        uid: "inv-2".into(),
        start: OCT_1,
        end: OCT_1 + 3600,
        ..Default::default()
    };
    record_invite_event(
        &conn,
        "acct",
        "acct#INBOX#7",
        "m@x",
        &invite,
        Response::Accepted,
    )
    .unwrap();
    let mine = create_event(&conn, &input("Mine", OCT_1, OCT_1 + 60)).unwrap();
    let id = mine["event"]["id"].as_str().unwrap().to_string();
    conn.execute(
        "UPDATE events SET account = 'acct', thread_id = 'acct#INBOX#8' WHERE id = ?1",
        [&id],
    )
    .unwrap();

    store::delete_account(&conn, "acct").unwrap();
    assert_eq!(store::invite_event(&conn, "acct", "inv-2").unwrap(), None);
    let kept = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!((kept.account.as_str(), kept.thread_id.as_str()), ("", ""));
}

#[test]
fn editing_a_later_occurrence_moves_the_series_rather_than_restarting_it() {
    let conn = conn();
    let first = OCT_1 + 3600;
    let created = create_event(
        &conn,
        &EventInput {
            repeat: "daily".into(),
            all_day: false,
            ..input("Standup", first, first + 900)
        },
    )
    .unwrap();
    let id = created["event"]["id"].as_str().unwrap().to_string();

    // Opened on the third day: unchanged times save as they were.
    let third = first + 2 * DAY;
    update_event(
        &conn,
        &id,
        &EventInput {
            notes: "Bring updates".into(),
            repeat: "daily".into(),
            occurrence_start: third,
            ..input("Standup", third, third + 900)
        },
    )
    .unwrap();
    let saved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(saved.start_at, first, "earlier occurrences are kept");
    assert_eq!(saved.notes, "Bring updates");

    // Moved half an hour later and made longer: every day follows.
    update_event(
        &conn,
        &id,
        &EventInput {
            repeat: "daily".into(),
            occurrence_start: third,
            ..input("Standup", third + 1800, third + 1800 + 1800)
        },
    )
    .unwrap();
    let saved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(saved.start_at, first + 1800);
    assert_eq!(saved.end_at - saved.start_at, 1800);
}

const SERIES: &str = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Review\r\n\
RRULE:FREQ=WEEKLY;COUNT=3\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

fn occurrence(extra: &str) -> crate::calendar::Invite {
    crate::calendar::parse_invite(&format!(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID:20261015T170000Z\r\n{extra}SUMMARY:Review\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    ))
    .unwrap()
}

fn starts(conn: &Connection) -> Vec<i64> {
    events_payload(conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap()["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["start"].as_i64().unwrap())
        .collect()
}

#[test]
fn an_answer_about_one_occurrence_leaves_the_rest_of_the_series() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    assert_eq!(starts(&conn), vec![week(0), week(1), week(2)]);

    // The second week moved an hour later, and accepted.
    let moved = occurrence("DTSTART:20261015T180000Z\r\nDTEND:20261015T190000Z\r\n");
    assert_eq!(moved.recurrence_id, week(1));
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3600, week(2)]);

    // Then declined: that week alone goes.
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
    let event = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert_eq!(event.response, "accepted", "the series' own answer stands");
}

#[test]
fn a_changed_occurrence_shows_its_own_title_and_location() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let moved =
        occurrence("DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nLOCATION:Room 9\r\n");
    let mut moved_ics = moved.clone();
    moved_ics.ics = moved
        .ics
        .replace("SUMMARY:Review", "SUMMARY:Review (offsite)");
    record_invite_event(&conn, "acct", "t", "m", &moved_ics, Response::Accepted).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    let items = payload["events"].as_array().unwrap();
    let details: Vec<(&str, &str)> = items
        .iter()
        .map(|item| {
            (
                item["title"].as_str().unwrap(),
                item["location"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        details,
        vec![
            ("Review", ""),
            ("Review (offsite)", "Room 9"),
            ("Review", "")
        ]
    );
}

#[test]
fn removing_a_cancelled_occurrence_keeps_the_rest_and_stops_offering_it() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let cancel = crate::calendar::parse_invite(&format!(
        "BEGIN:VCALENDAR\r\nMETHOD:CANCEL\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID:20261015T170000Z\r\nDTSTART:20261015T170000Z\r\nSTATUS:CANCELLED\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    ))
    .unwrap();
    crate::store::save_cached_message(
        &conn,
        "acct",
        "INBOX",
        7,
        &crate::parse::Message {
            body: "Cancelled".into(),
            invite: Some(cancel.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let mine = std::collections::HashSet::new();
    let offered = crate::calendar::invite_json_for(&conn, "acct", &cancel, &mine);
    assert_ne!(offered["event_id"], "");

    crate::calendar::remove_from_calendar(
        &conn,
        "acct#INBOX#7",
        std::path::Path::new("/nonexistent"),
    )
    .unwrap();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
    let after = crate::calendar::invite_json_for(&conn, "acct", &cancel, &mine);
    assert_eq!(after["event_id"], "", "the occurrence is already out");
}

fn occurrence_at(rid: &str, extra: &str) -> crate::calendar::Invite {
    crate::calendar::parse_invite(&format!(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID:{rid}\r\n{extra}SUMMARY:Review\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    ))
    .unwrap()
}

#[test]
fn occurrences_answered_without_their_series_stand_alone_and_fold_in_later() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nLOCATION:Room 9\r\n",
    );
    let third = occurrence_at(
        "20261022T170000Z",
        "DTSTART:20261022T170000Z\r\nDTEND:20261022T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &third, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(1), week(2)]);

    // Declining one date leaves the other.
    record_invite_event(&conn, "acct", "t", "m", &third, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(1)]);

    // Accepting the series takes the lone occurrence in, keeping its details.
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1), week(2)]);
    assert!(
        store::invite_occurrence_events(&conn, "acct", "inv-s")
            .unwrap()
            .is_empty()
    );
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    assert_eq!(payload["events"][1]["location"], "Room 9");

    // Declining the series takes everything.
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Declined).unwrap();
    assert!(starts(&conn).is_empty());
}

#[test]
fn a_moved_occurrence_at_a_newer_sequence_replaces_the_original() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series =
        crate::calendar::parse_invite(&SERIES.replace("RRULE:", "SEQUENCE:0\r\nRRULE:")).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let moved = occurrence_at(
        "20261015T170000Z",
        "SEQUENCE:1\r\nDTSTART:20261015T200000Z\r\nDTEND:20261015T210000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3 * 3600, week(2)]);
}

#[test]
fn moving_an_occurrence_across_a_dst_change_keeps_the_series_wall_clock_time() {
    let conn = conn();
    let tz: Tz = "America/New_York".parse().unwrap();
    let at = |day: u32, hour: u32| {
        tz.with_ymd_and_hms(2026, 10, day, hour, 0, 0)
            .single()
            .unwrap()
            .timestamp()
    };
    let nov = |day: u32, hour: u32| {
        tz.with_ymd_and_hms(2026, 11, day, hour, 0, 0)
            .single()
            .unwrap()
            .timestamp()
    };
    let event = store::save_event(
        &conn,
        &CalendarEvent {
            id: "event-dst".into(),
            title: "Run".into(),
            start_at: at(30, 9),
            end_at: at(30, 10),
            recurring: true,
            rrule: "daily".into(),
            tzid: "America/New_York".into(),
            ..Default::default()
        },
    )
    .unwrap();
    // The Oct 31 09:00 occurrence moved a day on, to Nov 1 09:00 (after the
    // clocks go back).
    let input = EventInput {
        occurrence_start: at(31, 9),
        repeat: "daily".into(),
        ..input("Run", nov(1, 9), nov(1, 10))
    };
    let moved = series_input(&event, &input);
    assert_eq!(moved.start, at(31, 9), "still 09:00, a day later");
    assert_eq!(moved.end - moved.start, 3600);
}

#[test]
fn an_occurrence_declined_then_accepted_is_back_and_can_be_cancelled() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Declined).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1), week(2)]);
    let event = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert!(!excludes_occurrence(&event, week(1)));
}

#[test]
fn an_occurrence_keeps_its_own_answer() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Tentative).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    let answers: Vec<&str> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["response"].as_str().unwrap())
        .collect();
    assert_eq!(answers, vec!["accepted", "tentative", "accepted"]);
}

#[test]
fn editing_a_repeating_event_keeps_the_zone_it_was_made_in() {
    let conn = conn();
    store::save_event(
        &conn,
        &CalendarEvent {
            id: "event-tz".into(),
            title: "Run".into(),
            start_at: OCT_1,
            end_at: OCT_1 + 3600,
            recurring: true,
            rrule: "daily".into(),
            tzid: "Pacific/Chatham".into(),
            ..Default::default()
        },
    )
    .unwrap();
    update_event(
        &conn,
        "event-tz",
        &EventInput {
            repeat: "daily".into(),
            ..input("Long run", OCT_1, OCT_1 + 3600)
        },
    )
    .unwrap();
    let saved = store::event(&conn, "event-tz").unwrap().unwrap();
    assert_eq!(saved.title, "Long run");
    assert_eq!(saved.tzid, "Pacific/Chatham");
}

#[test]
fn answering_the_series_again_keeps_the_dates_already_dealt_with() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    // The second week declined, the third moved an hour and accepted.
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    let third = occurrence_at(
        "20261022T170000Z",
        "DTSTART:20261022T180000Z\r\nDTEND:20261022T190000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Declined).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &third, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2) + 3600]);

    record_invite_event(&conn, "acct", "t", "m", &series, Response::Tentative).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2) + 3600]);
}

#[test]
fn accepting_one_date_of_a_listed_exdate_keeps_the_others_excluded() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(&SERIES.replace(
        "RRULE:FREQ=WEEKLY;COUNT=3\r\n",
        "RRULE:FREQ=WEEKLY;COUNT=4\r\nEXDATE:20261015T170000Z,20261022T170000Z\r\n",
    ))
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(3)]);
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1), week(3)]);
}

#[test]
fn a_declined_rdate_occurrence_goes() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(&SERIES.replace(
        "RRULE:FREQ=WEEKLY;COUNT=3\r\n",
        "RDATE:20261015T170000Z\r\n",
    ))
    .unwrap();
    assert!(series.recurring);
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1)]);
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(0)]);
}

#[test]
fn an_occurrence_moved_before_the_series_start_is_still_found() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    // The first week, moved back into September.
    let early = occurrence_at(
        "20261008T170000Z",
        "DTSTART:20260924T170000Z\r\nDTEND:20260924T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &early, Response::Accepted).unwrap();
    let sept = OCT_1 - 30 * DAY;
    let payload = events_payload(&conn, sept, OCT_1, false).unwrap();
    let found: Vec<i64> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["start"].as_i64().unwrap())
        .collect();
    assert_eq!(found, vec![OCT_1 - 7 * DAY + 17 * 3600]);
}

#[test]
fn a_date_declined_before_the_series_stays_out_once_it_is_accepted() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    // What `respond` records for a decline about one date.
    store::record_invite_response(&conn, "acct", "inv-s", week(1), "declined", 0).unwrap();
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Declined).unwrap();

    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
}

#[test]
fn turning_an_occurrence_all_day_moves_the_series_by_whole_dates() {
    let conn = conn();
    let tz: Tz = "America/New_York".parse().unwrap();
    let local = |month: u32, day: u32, hour: u32| {
        tz.with_ymd_and_hms(2026, month, day, hour, 0, 0)
            .single()
            .unwrap()
            .timestamp()
    };
    let utc_date = |month: u32, day: u32| {
        chrono::NaiveDate::from_ymd_opt(2026, month, day)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp()
    };
    let event = store::save_event(
        &conn,
        &CalendarEvent {
            id: "event-convert".into(),
            title: "Run".into(),
            start_at: local(10, 1, 9),
            end_at: local(10, 1, 10),
            recurring: true,
            rrule: "daily".into(),
            tzid: "America/New_York".into(),
            ..Default::default()
        },
    )
    .unwrap();

    // The Nov 5 occurrence (after the clocks went back) made all-day, same date.
    let all_day = series_input(
        &event,
        &EventInput {
            all_day: true,
            occurrence_start: local(11, 5, 9),
            repeat: "daily".into(),
            ..input("Run", utc_date(11, 5), utc_date(11, 6))
        },
    );
    assert_eq!(all_day.start, utc_date(10, 1));
    assert_eq!(all_day.end, utc_date(10, 2));

    // And back: an all-day series made timed on one occurrence, a day later.
    let all_day_event = CalendarEvent {
        start_at: utc_date(10, 1),
        end_at: utc_date(10, 2),
        all_day: true,
        tzid: "America/New_York".into(),
        ..event
    };
    let timed = series_input(
        &all_day_event,
        &EventInput {
            occurrence_start: utc_date(11, 5),
            repeat: "daily".into(),
            ..input("Run", local(11, 6, 9), local(11, 6, 10))
        },
    );
    assert_eq!(timed.start, local(10, 2, 9));
    assert_eq!(timed.end - timed.start, 3600);
}

#[test]
fn an_update_that_excludes_an_answered_occurrence_keeps_it_out() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T180000Z\r\nDTEND:20261015T190000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 3600, week(2)]);

    // The organizer then drops that date from the series.
    let update = crate::calendar::parse_invite(&SERIES.replace(
        "RRULE:FREQ=WEEKLY;COUNT=3\r\n",
        "RRULE:FREQ=WEEKLY;COUNT=3\r\nEXDATE:20261015T170000Z\r\nSEQUENCE:1\r\n",
    ))
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &update, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
}

#[test]
fn a_cancelled_override_is_not_shown() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(&SERIES.replace(
        "END:VEVENT\r\nEND:VCALENDAR",
        "END:VEVENT\r\nBEGIN:VEVENT\r\nUID:inv-s\r\nRECURRENCE-ID:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nSTATUS:CANCELLED\r\n\
         SUMMARY:Review\r\nEND:VEVENT\r\nEND:VCALENDAR",
    ))
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
}

#[test]
fn a_moved_rdate_occurrence_replaces_the_original() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(&SERIES.replace(
        "RRULE:FREQ=WEEKLY;COUNT=3\r\n",
        "RDATE:20261015T170000Z\r\n",
    ))
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let moved = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T190000Z\r\nDTEND:20261015T200000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1) + 2 * 3600]);
}

#[test]
fn an_occurrence_update_keeps_the_zone_it_defines() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    // 12:00 in a zone the update alone defines, the way Thunderbird names
    // one: its own TZID, pointing at São Paulo (UTC-3, no DST).
    let moved = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VTIMEZONE\r\nTZID:Custom Three\r\n\
         X-LIC-LOCATION:America/Sao_Paulo\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\n\
         TZOFFSETFROM:-0300\r\nTZOFFSETTO:-0300\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID:20261015T170000Z\r\nDTSTART;TZID=Custom Three:20261016T120000\r\n\
         DTEND;TZID=Custom Three:20261016T130000\r\nSUMMARY:Review\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    // 12:00 -03:00 on the 16th is 15:00Z, 22 hours after the original 17:00Z.
    assert_eq!(moved.start, week(1) + 22 * 3600);
    assert_eq!(starts(&conn), vec![week(0), week(1) + 22 * 3600, week(2)]);
}

#[test]
fn a_cancelled_lone_occurrence_stays_out_when_the_series_is_accepted() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    let cancel = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:CANCEL\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID:20261015T170000Z\r\nDTSTART:20261015T170000Z\r\nSTATUS:CANCELLED\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    crate::store::save_cached_message(
        &conn,
        "acct",
        "INBOX",
        8,
        &crate::parse::Message {
            body: "Cancelled".into(),
            invite: Some(cancel),
            ..Default::default()
        },
    )
    .unwrap();
    crate::calendar::remove_from_calendar(
        &conn,
        "acct#INBOX#8",
        std::path::Path::new("/nonexistent"),
    )
    .unwrap();
    assert!(starts(&conn).is_empty());

    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
}

#[test]
fn an_occurrence_made_all_day_is_all_day_on_its_date() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let all_day = occurrence_at(
        "20261015T170000Z",
        "DTSTART;VALUE=DATE:20261015\r\nDTEND;VALUE=DATE:20261016\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &all_day, Response::Accepted).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    let items = payload["events"].as_array().unwrap();
    assert_eq!(items.len(), 3, "{payload}");
    let day = items.iter().find(|item| item["all_day"] == true).unwrap();
    assert_eq!(day["start"], OCT_1 + 14 * DAY);
    assert_eq!(day["end"], OCT_1 + 15 * DAY);
    assert_eq!(
        items.iter().filter(|item| item["all_day"] == false).count(),
        2
    );
}

#[test]
fn a_this_and_future_override_keeps_every_later_occurrence() {
    let conn = conn();
    let series = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-f\r\n\
         DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Daily\r\n\
         RRULE:FREQ=DAILY;COUNT=5\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:inv-f\r\nRECURRENCE-ID;RANGE=THISANDFUTURE:20261009T170000Z\r\n\
         DTSTART:20261009T190000Z\r\nDTEND:20261009T200000Z\r\nSUMMARY:Daily\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn).len(), 5);
}

#[test]
fn an_all_day_this_and_future_override_makes_every_later_occurrence_all_day() {
    let conn = conn();
    let series = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-d\r\n\
         DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Daily\r\n\
         RRULE:FREQ=DAILY;COUNT=5\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:inv-d\r\nRECURRENCE-ID;RANGE=THISANDFUTURE:20261009T170000Z\r\n\
         DTSTART;VALUE=DATE:20261009\r\nDTEND;VALUE=DATE:20261010\r\nSUMMARY:Daily\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 30 * DAY, false).unwrap();
    let items: Vec<(i64, i64, bool)> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["start"].as_i64().unwrap(),
                item["end"].as_i64().unwrap(),
                item["all_day"].as_bool().unwrap(),
            )
        })
        .collect();
    let day = |n: i64| OCT_1 + (7 + n) * DAY;
    assert_eq!(
        items,
        vec![
            (day(0) + 17 * 3600, day(0) + 18 * 3600, false),
            (day(1), day(2), true),
            (day(2), day(3), true),
            (day(3), day(4), true),
            (day(4), day(5), true),
        ]
    );
}

#[test]
fn a_this_and_future_override_lends_its_length_to_every_later_occurrence() {
    let conn = conn();
    let series = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-l\r\n\
         DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Daily\r\n\
         RRULE:FREQ=DAILY;COUNT=4\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:inv-l\r\nRECURRENCE-ID;RANGE=THISANDFUTURE:20261009T170000Z\r\n\
         DTSTART:20261009T170000Z\r\nDTEND:20261009T190000Z\r\nSUMMARY:Daily\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 30 * DAY, false).unwrap();
    let lengths: Vec<i64> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["end"].as_i64().unwrap() - item["start"].as_i64().unwrap())
        .collect();
    assert_eq!(lengths, vec![3600, 7200, 7200, 7200]);
}

#[test]
fn occurrences_starting_together_get_their_own_keys() {
    let conn = conn();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    // The second week moved onto the third's start, for longer.
    let moved = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261022T170000Z\r\nDTEND:20261022T190000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    let keys: Vec<&str> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys.len(), 3);
    let unique: std::collections::HashSet<&&str> = keys.iter().collect();
    assert_eq!(unique.len(), 3, "{keys:?}");
}

#[test]
fn an_occurrence_moved_onto_another_keeps_both() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let moved = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261022T170000Z\r\nDTEND:20261022T180000Z\r\nLOCATION:Room 9\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2), week(2)]);
}

#[test]
fn an_all_day_this_and_future_override_counts_dates_across_spring_forward() {
    let conn = conn();
    let series = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-dst\r\n\
         DTSTART;TZID=America/New_York:20260305T090000\r\n\
         DTEND;TZID=America/New_York:20260305T100000\r\nSUMMARY:Daily\r\n\
         RRULE:FREQ=DAILY;COUNT=5\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:inv-dst\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE;TZID=America/New_York:20260307T090000\r\n\
         DTSTART;VALUE=DATE:20260307\r\nDTEND;VALUE=DATE:20260308\r\nSUMMARY:Daily\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let march = |day: u32| {
        chrono::NaiveDate::from_ymd_opt(2026, 3, day)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp()
    };
    let payload = events_payload(&conn, march(1), march(20), false).unwrap();
    let all_day: Vec<i64> = payload["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["all_day"] == true)
        .map(|item| item["start"].as_i64().unwrap())
        .collect();
    assert_eq!(all_day, vec![march(7), march(8), march(9)]);
}

#[test]
fn cancelling_this_and_future_takes_out_the_rest_of_the_series() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let cancel = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:CANCEL\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nSTATUS:CANCELLED\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &cancel, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(0)]);
    let event = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert!(excludes_occurrence(&event, week(1)));
    assert!(excludes_occurrence(&event, week(2)));
    assert!(!excludes_occurrence(&event, week(0)));
}

#[test]
fn a_series_cut_short_stays_cut_when_answered_again() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let cancel = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:CANCEL\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nSTATUS:CANCELLED\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &cancel, Response::Declined).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Tentative).unwrap();
    assert_eq!(starts(&conn), vec![week(0)]);
}

#[test]
fn cancelling_this_and_future_takes_out_later_lone_occurrences() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    for (rid, start) in [
        ("20261008T170000Z", "20261008"),
        ("20261015T170000Z", "20261015"),
        ("20261022T170000Z", "20261022"),
    ] {
        let occurrence = occurrence_at(
            rid,
            &format!("DTSTART:{start}T170000Z\r\nDTEND:{start}T180000Z\r\n"),
        );
        record_invite_event(&conn, "acct", "t", "m", &occurrence, Response::Accepted).unwrap();
    }
    assert_eq!(starts(&conn), vec![week(0), week(1), week(2)]);
    let cancel = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:CANCEL\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nSTATUS:CANCELLED\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    assert!(cancel.this_and_future);
    record_invite_event(&conn, "acct", "t", "m", &cancel, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(0)]);
}

#[test]
fn a_this_and_future_cancellation_before_the_series_keeps_the_rest_out() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    let cancel = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:CANCEL\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nSTATUS:CANCELLED\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    crate::store::save_cached_message(
        &conn,
        "acct",
        "INBOX",
        9,
        &crate::parse::Message {
            body: "Cancelled".into(),
            invite: Some(cancel),
            ..Default::default()
        },
    )
    .unwrap();
    crate::calendar::remove_from_calendar(
        &conn,
        "acct#INBOX#9",
        std::path::Path::new("/nonexistent"),
    )
    .unwrap();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0)]);
}

#[test]
fn an_answered_invitation_can_be_put_back_without_answering_again() {
    let conn = conn();
    let invite = crate::calendar::parse_invite(SERIES).unwrap();
    crate::store::save_cached_message(
        &conn,
        "acct",
        "INBOX",
        10,
        &crate::parse::Message {
            body: "Invitation".into(),
            message_id: "inv@x".into(),
            invite: Some(invite.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let missing = std::path::Path::new("/nonexistent");
    // Not answered yet: nothing to put back.
    assert!(crate::calendar::add_to_calendar(&conn, "acct#INBOX#10", "t", missing).is_err());

    store::record_invite_response(&conn, "acct", "inv-s", 0, "tentative", 0).unwrap();
    let id = crate::calendar::add_to_calendar(&conn, "acct#INBOX#10", "t", missing).unwrap();
    let event = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert_eq!(event.id, id);
    assert_eq!(event.response, "tentative");
    assert_eq!(event.message_id, "inv@x");
}

const MOVED_ON: &str = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-m\r\n\
     SEQUENCE:0\r\nDTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Daily\r\n\
     RRULE:FREQ=DAILY;COUNT=5\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\n\
     BEGIN:VEVENT\r\nUID:inv-m\r\nSEQUENCE:1\r\n\
     RECURRENCE-ID;RANGE=THISANDFUTURE:20261009T170000Z\r\n\
     DTSTART:20261009T190000Z\r\nDTEND:20261009T200000Z\r\nSUMMARY:Daily\r\n\
     ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

#[test]
fn an_embedded_this_and_future_override_moves_every_later_occurrence() {
    let conn = conn();
    let series = crate::calendar::parse_invite(MOVED_ON).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let day = |n: i64| OCT_1 + (7 + n) * DAY;
    assert_eq!(
        starts(&conn),
        vec![
            day(0) + 17 * 3600,
            day(1) + 19 * 3600,
            day(2) + 19 * 3600,
            day(3) + 19 * 3600,
            day(4) + 19 * 3600,
        ]
    );
}

#[test]
fn a_declined_date_after_a_this_and_future_move_goes() {
    let conn = conn();
    let series = crate::calendar::parse_invite(MOVED_ON).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let third = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-m\r\n\
         RECURRENCE-ID:20261010T170000Z\r\nDTSTART:20261010T190000Z\r\n\
         DTEND:20261010T200000Z\r\nSUMMARY:Daily\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &third, Response::Declined).unwrap();
    let day = |n: i64| OCT_1 + (7 + n) * DAY;
    assert_eq!(
        starts(&conn),
        vec![
            day(0) + 17 * 3600,
            day(1) + 19 * 3600,
            day(3) + 19 * 3600,
            day(4) + 19 * 3600,
        ]
    );
}

#[test]
fn accepting_again_after_a_this_and_future_decline_brings_it_back() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let from_second = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nSUMMARY:Review\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    record_invite_event(&conn, "acct", "t", "m", &from_second, Response::Declined).unwrap();
    assert_eq!(starts(&conn), vec![week(0)]);

    // Just the third accepted on its own: back, with the second still out.
    let third = occurrence_at(
        "20261022T170000Z",
        "DTSTART:20261022T170000Z\r\nDTEND:20261022T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &third, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
    let event = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert!(!excludes_occurrence(&event, week(2)));
    assert!(excludes_occurrence(&event, week(1)));

    // The whole range accepted again: everything is back.
    record_invite_event(&conn, "acct", "t", "m", &from_second, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(1), week(2)]);
}

#[test]
fn notes_on_a_lone_occurrence_join_the_series_it_folds_into() {
    let conn = conn();
    let second = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &second, Response::Accepted).unwrap();
    let alone = store::invite_occurrence_event(&conn, "acct", "inv-s", second.recurrence_id)
        .unwrap()
        .unwrap();
    update_event(
        &conn,
        &alone.id,
        &EventInput {
            notes: "Bring the Q3 numbers".into(),
            ..input("", 1, 2)
        },
    )
    .unwrap();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let event = store::invite_event(&conn, "acct", "inv-s")
        .unwrap()
        .unwrap();
    assert_eq!(event.notes, "Bring the Q3 numbers");
}

#[test]
fn a_repeating_invitation_without_an_end_lasts_an_instant() {
    let conn = conn();
    let series =
        crate::calendar::parse_invite(&SERIES.replace("DTEND:20261008T180000Z\r\n", "")).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 60 * DAY, false).unwrap();
    for item in payload["events"].as_array().unwrap() {
        assert_eq!(item["end"], item["start"], "{payload}");
    }
}

#[test]
fn a_floating_override_in_a_utc_series_is_device_local() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let local_tz = crate::calendar::local_tz();
    let floating = local_tz
        .with_ymd_and_hms(2026, 10, 15, 20, 0, 0)
        .single()
        .unwrap()
        .timestamp();
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let moved = occurrence_at(
        "20261015T170000Z",
        "DTSTART:20261015T200000\r\nDTEND:20261015T210000\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &moved, Response::Accepted).unwrap();
    let mut expected = vec![week(0), floating, week(2)];
    expected.sort();
    assert_eq!(starts(&conn), expected);
}

#[test]
fn a_date_accepted_again_past_a_cut_survives_the_series_being_answered_again() {
    let conn = conn();
    let week = |n: i64| OCT_1 + 7 * DAY + 17 * 3600 + n * 7 * DAY;
    let series = crate::calendar::parse_invite(SERIES).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &series, Response::Accepted).unwrap();
    let from_second = crate::calendar::parse_invite(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID;RANGE=THISANDFUTURE:20261015T170000Z\r\n\
         DTSTART:20261015T170000Z\r\nDTEND:20261015T180000Z\r\nSUMMARY:Review\r\n\
         ORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
    .unwrap();
    store::record_invite_response(&conn, "acct", "inv-s", week(1), "declined", 0).unwrap();
    store::mark_this_and_future(&conn, "acct", "inv-s", week(1)).unwrap();
    record_invite_event(&conn, "acct", "t", "m", &from_second, Response::Declined).unwrap();
    let third = occurrence_at(
        "20261022T170000Z",
        "DTSTART:20261022T170000Z\r\nDTEND:20261022T180000Z\r\n",
    );
    record_invite_event(&conn, "acct", "t", "m", &third, Response::Accepted).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);

    record_invite_event(&conn, "acct", "t", "m", &series, Response::Tentative).unwrap();
    assert_eq!(starts(&conn), vec![week(0), week(2)]);
}

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
