use super::*;

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
