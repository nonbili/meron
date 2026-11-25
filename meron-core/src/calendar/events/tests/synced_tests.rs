use super::*;

#[test]
fn a_synced_custom_repeat_is_kept_or_dropped_on_purpose() {
    let conn = conn();
    let id = synced_custom_repeat(&conn);
    let event = store::event(&conn, &id).unwrap().unwrap();
    assert!(event.recurring);
    assert_eq!(event.rrule, "");
    let edit = |repeat: &str, title: &str| EventInput {
        title: title.into(),
        start: event.start_at,
        end: event.end_at,
        repeat: repeat.into(),
        ..Default::default()
    };

    update_event(&conn, &id, &edit(CUSTOM_REPEAT, "Daily sync")).unwrap();
    let kept = store::event(&conn, &id).unwrap().unwrap();
    assert!(kept.recurring);
    assert!(kept.ics.contains("RRULE:FREQ=MONTHLY;BYDAY=-1MO,-1WE"));
    assert!(kept.ics.contains("SUMMARY:Daily sync"));
    let object = crate::caldav::store::object_for_event(&conn, &id)
        .unwrap()
        .unwrap();
    assert_eq!(object.state, crate::caldav::store::CHANGED);

    update_event(&conn, &id, &edit("", "Daily sync")).unwrap();
    let once = store::event(&conn, &id).unwrap().unwrap();
    assert!(!once.recurring);
    assert!(!once.ics.contains("RRULE"));
}

#[test]
fn a_read_only_calendar_refuses_edits() {
    let conn = conn();
    let id = synced_custom_repeat(&conn);
    conn.execute("UPDATE dav_calendars SET read_only = 1", [])
        .unwrap();
    let event = store::event(&conn, &id).unwrap().unwrap();
    let input = EventInput {
        title: "Mine now".into(),
        start: event.start_at,
        end: event.end_at,
        repeat: CUSTOM_REPEAT.into(),
        ..Default::default()
    };
    assert!(update_event(&conn, &id, &input).is_err());
    assert!(delete_event(&conn, &id).is_err());
    assert_eq!(store::event(&conn, &id).unwrap().unwrap().title, "Standup");
}

#[test]
fn events_move_between_calendars() {
    let conn = conn();
    let id = synced_custom_repeat(&conn);
    let home = store::event(&conn, &id).unwrap().unwrap().calendar;
    let other = crate::caldav::store::reconcile_calendars(
        &conn,
        "dav-1",
        &[
            crate::caldav::Calendar {
                url: "https://dav.example.com/cal/".into(),
                name: "Work".into(),
                ..Default::default()
            },
            crate::caldav::Calendar {
                url: "https://dav.example.com/home/".into(),
                name: "Home".into(),
                ..Default::default()
            },
        ],
    )
    .unwrap()
    .into_iter()
    .find(|c| c.id != home)
    .unwrap();
    let event = store::event(&conn, &id).unwrap().unwrap();
    let to = |calendar: &str, repeat: &str| EventInput {
        title: "Standup".into(),
        start: event.start_at,
        end: event.end_at,
        repeat: repeat.into(),
        calendar: Some(calendar.into()),
        ..Default::default()
    };

    // A rule this device cannot keep stops it leaving its server.
    assert!(update_event(&conn, &id, &to("", CUSTOM_REPEAT)).is_err());

    // To another synced calendar: deleted from one, created in the other,
    // same UID, rule intact.
    update_event(&conn, &id, &to(&other.id, CUSTOM_REPEAT)).unwrap();
    let moved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(moved.calendar, other.id);
    assert!(moved.ics.contains("UID:standup") && moved.ics.contains("BYDAY=-1MO,-1WE"));
    let left = crate::caldav::store::pending(&conn, &home).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].state, crate::caldav::store::DELETED);
    assert_eq!(left[0].local_id, "");
    let arrived = crate::caldav::store::object_for_event(&conn, &id)
        .unwrap()
        .unwrap();
    assert_eq!(arrived.calendar, other.id);
    assert_eq!(arrived.state, crate::caldav::store::CREATED);
    assert_eq!(arrived.url, "https://dav.example.com/home/standup.ics");

    // To this device, once it repeats by a rule the editor names. Never sent
    // to the second calendar, it simply is not.
    update_event(&conn, &id, &to("", "weekly")).unwrap();
    let local = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!((local.calendar.as_str(), local.ics.as_str()), ("", ""));
    assert_eq!(local.rrule, "weekly");
    assert!(
        crate::caldav::store::object_for_event(&conn, &id)
            .unwrap()
            .is_none()
    );
    assert!(
        crate::caldav::store::pending(&conn, &other.id)
            .unwrap()
            .is_empty()
    );

    // And back out to a synced calendar as a new object.
    update_event(&conn, &id, &to(&home, "weekly")).unwrap();
    let synced = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(synced.calendar, home);
    assert!(synced.ics.contains("RRULE:FREQ=WEEKLY"));
    assert_eq!(
        crate::caldav::store::object_for_event(&conn, &id)
            .unwrap()
            .unwrap()
            .state,
        crate::caldav::store::CREATED
    );
    // An edit that names no calendar leaves it where it is.
    update_event(
        &conn,
        &id,
        &EventInput {
            calendar: None,
            ..to("", "weekly")
        },
    )
    .unwrap();
    assert_eq!(store::event(&conn, &id).unwrap().unwrap().calendar, home);
}

#[test]
fn a_stale_editor_keeps_what_the_server_changed_since() {
    let conn = conn();
    let id = synced_custom_repeat(&conn);
    let opened = store::event(&conn, &id).unwrap().unwrap();
    let shown = EventInput {
        title: opened.title.clone(),
        location: opened.location.clone(),
        start: opened.start_at,
        end: opened.end_at,
        repeat: CUSTOM_REPEAT.into(),
        ..Default::default()
    };
    // A sync brings a new location while the editor is open.
    let mut synced = opened.clone();
    synced.location = "Room 4".into();
    synced.ics = crate::caldav::ics::patch(&opened.ics, &opened, &synced);
    store::save_event(&conn, &synced).unwrap();

    // The user renames it; the location field still shows the old value.
    update_event(
        &conn,
        &id,
        &EventInput {
            title: "Daily sync".into(),
            original: Some(Box::new(shown.clone())),
            ..shown
        },
    )
    .unwrap();
    let saved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(saved.title, "Daily sync");
    assert_eq!(saved.location, "Room 4");
    assert!(saved.ics.contains("LOCATION:Room 4"));
    assert!(saved.recurring);
}

#[test]
fn a_stale_editor_keeps_alerts_a_sync_brought() {
    let conn = conn();
    let id = synced_custom_repeat(&conn);
    let opened = store::event(&conn, &id).unwrap().unwrap();
    let shown = EventInput {
        title: opened.title.clone(),
        start: opened.start_at,
        end: opened.end_at,
        repeat: CUSTOM_REPEAT.into(),
        alerts: Some(None),
        ..Default::default()
    };
    // Another client sets an alert while the editor is open.
    let mut synced = opened.clone();
    synced.alerts = Some(vec![30]);
    store::save_event(&conn, &synced).unwrap();

    update_event(
        &conn,
        &id,
        &EventInput {
            title: "Daily sync".into(),
            original: Some(Box::new(shown.clone())),
            ..shown
        },
    )
    .unwrap();
    let saved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(saved.title, "Daily sync");
    assert_eq!(saved.alerts, Some(vec![30]));
}

#[test]
fn a_rename_keeps_a_utc_series_in_utc() {
    let conn = conn();
    let calendars = crate::caldav::store::reconcile_calendars(
        &conn,
        "dav-1",
        &[crate::caldav::Calendar {
            url: "https://dav.example.com/cal/".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:utc\r\nDTSTART:20261005T090000Z\r\nDTEND:20261005T100000Z\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Sync\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let mut event = crate::caldav::ics::event_from_ics(ics).unwrap();
    event.id = "event-utc".into();
    event.calendar = calendars[0].id.clone();
    store::save_event(&conn, &event).unwrap();
    assert_eq!(event.tzid, "");

    update_event(
        &conn,
        "event-utc",
        &EventInput {
            title: "Weekly sync".into(),
            start: event.start_at,
            end: event.end_at,
            repeat: "weekly".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let saved = store::event(&conn, "event-utc").unwrap().unwrap();
    assert!(
        saved.ics.contains("DTSTART:20261005T090000Z"),
        "{}",
        saved.ics
    );
    assert!(saved.ics.contains("SUMMARY:Weekly sync"));
}

/// Three synced calendars, Work, Home and Club, with the custom-repeat
/// event in Work.
fn two_calendars(conn: &Connection) -> (String, String, String) {
    let id = synced_custom_repeat(conn);
    let calendars = crate::caldav::store::reconcile_calendars(
        conn,
        "dav-1",
        &[
            crate::caldav::Calendar {
                url: "https://dav.example.com/cal/".into(),
                name: "Work".into(),
                ..Default::default()
            },
            crate::caldav::Calendar {
                url: "https://dav.example.com/home/".into(),
                name: "Home".into(),
                ..Default::default()
            },
            crate::caldav::Calendar {
                url: "https://dav.example.com/club/".into(),
                name: "Club".into(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let id_of = |name: &str| {
        calendars
            .iter()
            .find(|c| c.name == name)
            .unwrap()
            .id
            .clone()
    };
    (id, id_of("Work"), id_of("Home"))
}

fn move_to(conn: &Connection, id: &str, calendar: &str) -> Value {
    let event = store::event(conn, id).unwrap().unwrap();
    update_event(
        conn,
        id,
        &EventInput {
            title: event.title.clone(),
            start: event.start_at,
            end: event.end_at,
            repeat: CUSTOM_REPEAT.into(),
            calendar: Some(calendar.into()),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn moving_again_before_a_move_lands_retargets_the_deletion() {
    let conn = conn();
    let (id, work, home) = two_calendars(&conn);
    let result = move_to(&conn, &id, &home);
    assert_eq!(result["moved_from"], work.as_str());
    let club = crate::caldav::store::all_calendars(&conn)
        .unwrap()
        .into_iter()
        .find(|c| c.name == "Club")
        .unwrap()
        .id;
    move_to(&conn, &id, &club);
    let tombstone = &crate::caldav::store::pending(&conn, &work).unwrap()[0];
    assert_eq!(tombstone.state, crate::caldav::store::DELETED);
    // Waits for the copy in Club, not the one in Home that was never sent.
    assert_eq!(
        tombstone.moved_to,
        "https://dav.example.com/club/standup.ics"
    );
    assert!(
        crate::caldav::store::pending(&conn, &home)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_picker_left_as_it_opened_is_not_a_move() {
    let conn = conn();
    let (id, work, home) = two_calendars(&conn);
    let event = store::event(&conn, &id).unwrap().unwrap();
    let shown = EventInput {
        title: event.title.clone(),
        start: event.start_at,
        end: event.end_at,
        repeat: CUSTOM_REPEAT.into(),
        calendar: Some(work.clone()),
        ..Default::default()
    };
    // Moved to Home elsewhere while this editor was open on Work.
    move_to(&conn, &id, &home);
    update_event(
        &conn,
        &id,
        &EventInput {
            title: "Renamed".into(),
            original: Some(Box::new(shown.clone())),
            ..shown
        },
    )
    .unwrap();
    let saved = store::event(&conn, &id).unwrap().unwrap();
    assert_eq!(
        (saved.title.as_str(), saved.calendar.as_str()),
        ("Renamed", home.as_str())
    );
}

#[test]
fn unsent_events_outlive_a_calendar_gone_from_its_server() {
    let conn = conn();
    let (id, _work, home) = two_calendars(&conn);
    let created = create_event(
        &conn,
        &EventInput {
            title: "Offline".into(),
            start: OCT_1,
            end: OCT_1 + 3600,
            calendar: Some(home.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let offline = created["event"]["id"].as_str().unwrap().to_string();
    // The server now lists Work only.
    crate::caldav::store::reconcile_calendars(
        &conn,
        "dav-1",
        &[crate::caldav::Calendar {
            url: "https://dav.example.com/cal/".into(),
            name: "Work".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    let kept = store::event(&conn, &offline)
        .unwrap()
        .expect("unsent event kept");
    assert_eq!((kept.calendar.as_str(), kept.ics.as_str()), ("", ""));
    assert!(store::event(&conn, &id).unwrap().is_some());
}

#[test]
fn moving_back_before_a_sync_updates_the_original() {
    let conn = conn();
    let (id, work, home) = two_calendars(&conn);
    move_to(&conn, &id, &home);
    move_to(&conn, &id, &work);
    let back = crate::caldav::store::object_for_event(&conn, &id)
        .unwrap()
        .unwrap();
    assert_eq!(back.calendar, work);
    assert_eq!(back.state, crate::caldav::store::CHANGED);
    assert_eq!(back.etag, "\"1\"");
    assert!(
        crate::caldav::store::pending(&conn, &home)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn discarding_a_move_keeps_the_original() {
    let conn = conn();
    let (id, work, home) = two_calendars(&conn);
    move_to(&conn, &id, &home);
    crate::caldav::store::set_enabled(&conn, &home, false).unwrap();
    // No deletion is left to send from Work, which lists everything again.
    assert!(
        crate::caldav::store::pending(&conn, &work)
            .unwrap()
            .is_empty()
    );
    let work = crate::caldav::store::calendar(&conn, &work)
        .unwrap()
        .unwrap();
    assert_eq!((work.ctag.as_str(), work.listed_at), ("", 0));
}
