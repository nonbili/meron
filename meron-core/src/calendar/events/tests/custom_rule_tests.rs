use super::*;

#[test]
fn a_custom_repeat_made_here_expands_by_its_rule() {
    let conn = conn();
    let all_day = |repeat: &str| EventInput {
        title: "Review".into(),
        // Thursday 1 October 2026.
        start: OCT_1,
        end: OCT_1 + DAY,
        all_day: true,
        repeat: repeat.into(),
        ..Default::default()
    };
    let starts = |conn: &Connection| -> Vec<i64> {
        let payload = events_payload(conn, OCT_1, OCT_1 + 120 * DAY, false).unwrap();
        keys(&payload).into_iter().map(|(_, start)| start).collect()
    };

    // Every other Thursday and Friday, four times.
    let created = create_event(
        &conn,
        &all_day("freq=weekly;interval=2;byday=fr,th;count=4"),
    )
    .unwrap();
    assert_eq!(
        created["event"]["repeat"],
        "FREQ=WEEKLY;INTERVAL=2;BYDAY=TH,FR;COUNT=4"
    );
    assert_eq!(
        starts(&conn),
        [OCT_1, OCT_1 + DAY, OCT_1 + 14 * DAY, OCT_1 + 15 * DAY]
    );

    // The first Thursday of each month, through 31 December.
    let id = created["event"]["id"].as_str().unwrap().to_string();
    update_event(
        &conn,
        &id,
        &all_day("FREQ=MONTHLY;BYDAY=1TH;UNTIL=20261231"),
    )
    .unwrap();
    assert_eq!(starts(&conn), [OCT_1, OCT_1 + 35 * DAY, OCT_1 + 63 * DAY]);

    let early = update_event(&conn, &id, &all_day("FREQ=DAILY;UNTIL=20260930")).unwrap_err();
    assert!(early.to_string().contains("ends before"), "{early}");
    assert!(update_event(&conn, &id, &all_day("FREQ=HOURLY")).is_err());
}

#[test]
fn a_synced_rule_can_be_replaced_by_a_custom_one_and_brought_here() {
    let conn = conn();
    let id = synced_custom_repeat(&conn);
    let event = store::event(&conn, &id).unwrap().unwrap();
    let edit = |repeat: &str, calendar: Option<&str>| EventInput {
        title: event.title.clone(),
        start: event.start_at,
        end: event.end_at,
        repeat: repeat.into(),
        calendar: calendar.map(str::to_string),
        ..Default::default()
    };

    let rule = "FREQ=WEEKLY;BYDAY=MO,WE;COUNT=6";
    update_event(&conn, &id, &edit(rule, None)).unwrap();
    let synced = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(synced.rrule, rule);
    assert!(
        synced.ics.contains(&format!("RRULE:{rule}\r\n")),
        "{}",
        synced.ics
    );
    assert!(!synced.ics.contains("-1MO"));

    // A rule the editor says can live on this device, unlike the server's.
    update_event(&conn, &id, &edit(rule, Some(""))).unwrap();
    let here = store::event(&conn, &id).unwrap().unwrap();
    assert!(here.calendar.is_empty() && here.ics.is_empty());
    assert_eq!(here.rrule, rule);
    let payload = events_payload(&conn, event.start_at, event.start_at + 60 * DAY, false).unwrap();
    assert_eq!(keys(&payload).len(), 6, "{payload}");
}

#[test]
fn a_utc_end_keeps_the_last_day_of_a_series_in_a_zone_ahead_of_utc() {
    // Daily 18:00 Tokyo (09:00 UTC), ending at the end of 3 December in Tokyo
    // as RFC 5545 has it written: in UTC, 14:59:59.
    let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:p\r\nDTSTART;TZID=Asia/Tokyo:20261201T180000\r\n\
               DTEND;TZID=Asia/Tokyo:20261201T190000\r\nRRULE:FREQ=DAILY;UNTIL=20261203T145959Z\r\n\
               END:VEVENT\r\nEND:VCALENDAR\r\n";
    let event = CalendarEvent {
        id: "p".into(),
        ..crate::caldav::ics::event_from_ics(ics).unwrap()
    };
    let starts: Vec<i64> = occurrences(&event, 0, i64::MAX / 2, &Default::default())
        .iter()
        .map(|o| o.start)
        .collect();
    let first = 1_796_115_600; // 2026-12-01T09:00:00Z
    assert_eq!(starts, [first, first + DAY, first + 2 * DAY]);
}

#[test]
fn a_utc_series_ending_late_in_its_first_day_keeps_its_end_through_an_edit() {
    let conn = conn();
    // 23:00 UTC, already the next day in much of Asia, ending that UTC day.
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:late\r\nDTSTART:20261005T230000Z\r\nDTEND:20261005T233000Z\r\n\
         RRULE:FREQ=DAILY;UNTIL=20261005T235959Z\r\nSUMMARY:Late\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    let event = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(event.rrule, "FREQ=DAILY;UNTIL=20261005");
    let input = EventInput {
        title: "Later".into(),
        start: event.start_at,
        end: event.end_at,
        repeat: event.rrule.clone(),
        ..Default::default()
    };
    update_event(&conn, &id, &input).unwrap();
    assert_eq!(store::event(&conn, &id).unwrap().unwrap().title, "Later");
}

#[test]
fn a_custom_rule_moves_with_its_series_and_never_leaves_out_its_start() {
    let conn = conn();
    // Every Thursday and Friday from Thursday 1 October, all day.
    let rule = "FREQ=WEEKLY;BYDAY=TH,FR";
    let input = |start: i64, repeat: &str| EventInput {
        title: "Review".into(),
        start,
        end: start + DAY,
        all_day: true,
        repeat: repeat.into(),
        ..Default::default()
    };
    let created = create_event(&conn, &input(OCT_1, rule)).unwrap();
    let id = created["event"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["event"]["series_date"], "2026-10-01");

    // Moved to Monday: the days move with it.
    update_event(&conn, &id, &input(OCT_1 + 4 * DAY, rule)).unwrap();
    assert_eq!(
        store::event(&conn, &id).unwrap().unwrap().rrule,
        "FREQ=WEEKLY;BYDAY=MO,TU"
    );

    // A rule that leaves out the first day is not made.
    let refused = create_event(&conn, &input(OCT_1, "FREQ=WEEKLY;BYDAY=MO")).unwrap_err();
    assert!(refused.to_string().contains("leaves out"), "{refused}");
}

#[test]
fn a_floating_series_counts_its_dates_on_this_device() {
    // Monday 00:30, wherever the reader is.
    let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:f\r\nDTSTART:20261005T003000\r\nDTEND:20261005T010000\r\n\
               RRULE:FREQ=WEEKLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let event = crate::caldav::ics::event_from_ics(ics).unwrap();
    assert_eq!(event_json(&event)["series_date"], "2026-10-05");
}

#[test]
fn a_rule_in_a_zone_only_its_object_defines_stays_the_servers() {
    let ics = "BEGIN:VCALENDAR\r\nBEGIN:VTIMEZONE\r\nTZID:Office Time\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\n\
               TZOFFSETFROM:+0900\r\nTZOFFSETTO:+0900\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:z\r\n\
               DTSTART;TZID=Office Time:20261005T090000\r\nDTEND;TZID=Office Time:20261005T100000\r\n\
               RRULE:FREQ=WEEKLY;COUNT=5\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let event = crate::caldav::ics::event_from_ics(ics).unwrap();
    assert!(event.recurring);
    assert_eq!(event.rrule, "");
}

#[test]
fn a_floating_series_brought_here_keeps_its_days_and_time() {
    let conn = conn();
    // Monday 00:30 wherever the reader is: Sunday in UTC for a zone ahead.
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:f\r\nDTSTART:20261005T003000\r\nDTEND:20261005T010000\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=3\r\nSUMMARY:Early\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    let event = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(event.rrule, "FREQ=WEEKLY;BYDAY=MO;COUNT=3");
    let input = EventInput {
        title: event.title.clone(),
        start: event.start_at,
        end: event.end_at,
        repeat: event.rrule.clone(),
        calendar: Some(String::new()),
        ..Default::default()
    };
    update_event(&conn, &id, &input).unwrap();
    let here = store::event(&conn, &id).unwrap().unwrap();
    assert!(here.calendar.is_empty() && here.ics.is_empty());
    let tz = crate::calendar::local_tz();
    let local: Vec<String> = occurrences(&here, 0, i64::MAX / 2, &Default::default())
        .iter()
        .map(|o| {
            tz.timestamp_opt(o.start, 0)
                .unwrap()
                .naive_local()
                .format("%a %H:%M")
                .to_string()
        })
        .collect();
    assert_eq!(local, ["Mon 00:30", "Mon 00:30", "Mon 00:30"]);
}

#[test]
fn a_utc_series_moved_within_its_utc_day_keeps_its_days() {
    let conn = conn();
    // Monday 13:00 UTC, Monday and Wednesday: 22:00 Monday in Tokyo.
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:m\r\nDTSTART:20261005T130000Z\r\nDTEND:20261005T140000Z\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO,WE\r\nSUMMARY:Sync\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    let event = store::event(&conn, &id).unwrap().unwrap();
    // Three hours later: Tuesday in Tokyo, still Monday in UTC.
    let input = EventInput {
        title: event.title.clone(),
        start: event.start_at + 3 * 3600,
        end: event.end_at + 3 * 3600,
        repeat: event.rrule.clone(),
        ..Default::default()
    };
    update_event(&conn, &id, &input).unwrap();
    let moved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(moved.rrule, "FREQ=WEEKLY;BYDAY=MO,WE");
    assert!(
        moved.ics.contains("DTSTART:20261005T160000Z"),
        "{}",
        moved.ics
    );
}

#[test]
fn an_every_other_week_series_moves_whole_with_an_occurrence() {
    let conn = conn();
    let monday = OCT_1 + 4 * DAY;
    // Every other week on Monday and Sunday, from Monday 5 October: the 5th,
    // the 11th, the 19th, the 25th, …
    let input = |start: i64, occurrence_start: i64| EventInput {
        title: "Shift".into(),
        start,
        end: start + DAY,
        all_day: true,
        repeat: "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,SU".into(),
        occurrence_start,
        ..Default::default()
    };
    let created = create_event(&conn, &input(monday, 0)).unwrap();
    let id = created["event"]["id"].as_str().unwrap().to_string();

    // Sunday the 11th moved to Monday the 12th: everything a day later.
    update_event(&conn, &id, &input(monday + 7 * DAY, monday + 6 * DAY)).unwrap();
    let payload = events_payload(&conn, OCT_1, OCT_1 + 30 * DAY, false).unwrap();
    let starts: Vec<i64> = keys(&payload).into_iter().map(|(_, start)| start).collect();
    assert_eq!(
        starts,
        [
            monday + DAY,
            monday + 7 * DAY,
            monday + 15 * DAY,
            monday + 21 * DAY
        ],
        "{}",
        store::event(&conn, &id).unwrap().unwrap().rrule
    );
}

#[test]
fn a_repeat_chosen_with_a_move_is_saved_as_chosen() {
    let conn = conn();
    let monday = OCT_1 + 4 * DAY;
    let made = create_event(
        &conn,
        &EventInput {
            title: "Review".into(),
            start: monday,
            end: monday + DAY,
            all_day: true,
            repeat: "weekly".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let id = made["event"]["id"].as_str().unwrap().to_string();
    let opened = EventInput {
        title: "Review".into(),
        start: monday,
        end: monday + DAY,
        all_day: true,
        repeat: "weekly".into(),
        ..Default::default()
    };
    // Moved to Tuesday, and a rule picked there for Tuesday and Friday.
    let input = EventInput {
        start: monday + DAY,
        end: monday + 2 * DAY,
        repeat: "FREQ=WEEKLY;BYDAY=TU,FR".into(),
        original: Some(Box::new(opened.clone())),
        ..opened
    };
    update_event(&conn, &id, &input).unwrap();
    assert_eq!(
        store::event(&conn, &id).unwrap().unwrap().rrule,
        "FREQ=WEEKLY;BYDAY=TU,FR"
    );
}

#[test]
fn a_rule_left_as_it_was_moves_here_when_the_editor_says_it_did_not() {
    let conn = conn();
    let monday = OCT_1 + 4 * DAY;
    let made = create_event(
        &conn,
        &EventInput {
            title: "Review".into(),
            start: monday,
            end: monday + DAY,
            all_day: true,
            repeat: "FREQ=WEEKLY;BYDAY=MO,TH".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let id = made["event"]["id"].as_str().unwrap().to_string();
    let opened = EventInput {
        title: "Review".into(),
        start: monday,
        end: monday + DAY,
        all_day: true,
        repeat: "FREQ=WEEKLY;BYDAY=MO,TH".into(),
        ..Default::default()
    };
    // Mobile's editor cannot place the series' days in its own zone, so it
    // sends the rule as it opened and leaves moving it to the core.
    let input = EventInput {
        start: monday + DAY,
        end: monday + 2 * DAY,
        original: Some(Box::new(opened.clone())),
        repeat_as_shown: Some(false),
        ..opened
    };
    update_event(&conn, &id, &input).unwrap();
    assert_eq!(
        store::event(&conn, &id).unwrap().unwrap().rrule,
        "FREQ=WEEKLY;BYDAY=TU,FR"
    );
}

#[test]
fn a_rule_left_as_it_was_moves_the_one_a_sync_brought_meanwhile() {
    let conn = conn();
    // The server now says Monday and Friday; the editor opened on Monday
    // and Thursday, before that sync.
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:r\r\nDTSTART;VALUE=DATE:20261005\r\nDTEND;VALUE=DATE:20261006\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO,FR\r\nSUMMARY:Review\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    let event = store::event(&conn, &id).unwrap().unwrap();
    let opened = EventInput {
        title: "Review".into(),
        start: event.start_at,
        end: event.end_at,
        all_day: true,
        repeat: "FREQ=WEEKLY;BYDAY=MO,TH".into(),
        ..Default::default()
    };
    let input = EventInput {
        start: event.start_at + DAY,
        end: event.end_at + DAY,
        original: Some(Box::new(opened.clone())),
        repeat_as_shown: Some(false),
        ..opened
    };
    update_event(&conn, &id, &input).unwrap();
    assert_eq!(
        store::event(&conn, &id).unwrap().unwrap().rrule,
        "FREQ=WEEKLY;BYDAY=TU,SA"
    );
}

#[test]
fn a_rule_chosen_for_one_day_follows_the_series_into_the_next() {
    let conn = conn();
    // Monday 23:00 UTC. An editor further east picks Monday and Wednesday
    // for the series, then moves it two hours later: a time change on its
    // own calendar, but Tuesday in the series' zone.
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:z\r\nDTSTART:20261005T230000Z\r\nDTEND:20261005T233000Z\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO\r\nSUMMARY:Late\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    let event = store::event(&conn, &id).unwrap().unwrap();
    let opened = EventInput {
        title: "Late".into(),
        start: event.start_at,
        end: event.end_at,
        repeat: event.rrule.clone(),
        ..Default::default()
    };
    let input = EventInput {
        start: event.start_at + 2 * 3600,
        end: event.end_at + 2 * 3600,
        repeat: "FREQ=WEEKLY;BYDAY=MO,WE".into(),
        original: Some(Box::new(opened.clone())),
        repeat_as_shown: Some(true),
        repeat_from: Some("2026-10-05".into()),
        ..opened
    };
    update_event(&conn, &id, &input).unwrap();
    assert_eq!(
        store::event(&conn, &id).unwrap().unwrap().rrule,
        "FREQ=WEEKLY;BYDAY=TU,TH"
    );
}

#[test]
fn a_utc_series_brought_here_keeps_its_days_through_an_edit() {
    let conn = conn();
    // Monday 22:00 UTC, Mondays until the 5th: Tuesday morning further east.
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:u\r\nDTSTART:20261005T220000Z\r\nDTEND:20261005T230000Z\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO;UNTIL=20261005T235959Z\r\nSUMMARY:Late\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    let event = store::event(&conn, &id).unwrap().unwrap();
    let edit = |title: &str, calendar: Option<&str>| EventInput {
        title: title.into(),
        start: event.start_at,
        end: event.end_at,
        repeat: event.rrule.clone(),
        calendar: calendar.map(str::to_string),
        ..Default::default()
    };
    update_event(&conn, &id, &edit("Late", Some(""))).unwrap();
    update_event(&conn, &id, &edit("Later", None)).unwrap();
    let here = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(here.title, "Later");
    assert_eq!(here.tzid, "");
    assert_eq!(here.rrule, "FREQ=WEEKLY;BYDAY=MO;UNTIL=20261005");
}

#[test]
fn an_unsent_floating_series_keeps_its_wall_times_when_its_calendar_goes() {
    let conn = conn();
    let id = synced_event(
        &conn,
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:f\r\nDTSTART:20261005T003000\r\nDTEND:20261005T010000\r\n\
         RRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=3\r\nSUMMARY:Early\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    );
    conn.execute(
        "UPDATE dav_objects SET state = ?1",
        [crate::caldav::store::CHANGED],
    )
    .unwrap();
    // The server no longer lists the calendar.
    crate::caldav::store::reconcile_calendars(&conn, "dav-1", &[]).unwrap();
    let here = store::event(&conn, &id).unwrap().unwrap();
    assert!(here.calendar.is_empty() && here.ics.is_empty());
    assert_eq!(Some(here.tzid.clone()), crate::calendar::local_tzid());
}

#[test]
fn a_series_zone_is_named_as_a_browser_knows_it() {
    let event = CalendarEvent {
        recurring: true,
        tzid: "Eastern Standard Time".into(),
        ..Default::default()
    };
    assert_eq!(event_json(&event)["series_zone"], "America/New_York");
    let utc = CalendarEvent {
        recurring: true,
        ..Default::default()
    };
    assert_eq!(event_json(&utc)["series_zone"], "UTC");
}
