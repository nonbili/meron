use super::*;
use crate::calendar::events::{EventInput, create_event};

struct TestDir(std::path::PathBuf);
impl TestDir {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn db() -> (TestDir, Connection) {
    let dir =
        TestDir(std::env::temp_dir().join(format!("meron-reminders-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(dir.path()).unwrap();
    let conn = store::open_at(dir.path().join("test.db")).unwrap();
    (dir, conn)
}

#[test]
fn timed_task_midnight_is_distinct_from_date_only_and_survives_undo() {
    let (_dir, conn) = db();
    let midnight = calendar::local_tz()
        .with_ymd_and_hms(2026, 10, 25, 0, 0, 0)
        .single()
        .unwrap()
        .timestamp();
    let result =
        crate::tasks::create_task(&conn, "", "Deadline", "", midnight, "", "", "").unwrap();
    let id = result["task"]["id"].as_str().unwrap();
    let date_only = list(&conn, midnight - DAY, midnight + DAY).unwrap();
    assert_eq!(date_only.len(), 1);
    assert_eq!(date_only[0].at, morning(midnight, false).unwrap());
    let result =
        crate::tasks::apply_due_time(&conn, &json!({"due_has_time": true}), result.clone())
            .unwrap();
    assert_eq!(result["task"]["due_has_time"], true);
    assert_eq!(
        list(&conn, midnight - 1, midnight + DAY).unwrap()[0].at,
        midnight
    );
    let calendar = calendar::events::events_payload(&conn, midnight, midnight + DAY, true).unwrap();
    assert_eq!(calendar["events"][0]["source"], "task");
    assert_eq!(calendar["events"][0]["all_day"], false);
    assert_eq!(calendar["events"][0]["start"], midnight);
    let restore = crate::tasks::delete_task(&conn, id).unwrap();
    crate::tasks::restore(&conn, &restore["restore"]).unwrap();
    assert!(store::task(&conn, id).unwrap().unwrap().due_has_time);
}

#[test]
fn completed_deleted_and_rescheduled_tasks_do_not_fire_and_claim_is_durable() {
    let (dir, conn) = db();
    let at = store::now_unix() + DAY;
    let value = crate::tasks::create_task(&conn, "", "Due", "", at, "", "", "").unwrap();
    let id = value["task"]["id"].as_str().unwrap();
    store::set_task_due_time(&conn, id, true).unwrap();
    let reminder = list(&conn, at - 1, at + 1).unwrap().remove(0);
    assert!(claim(&conn, &reminder.key, at - 1).unwrap().is_none());
    store::set_task_done(&conn, id, true).unwrap();
    assert!(claim(&conn, &reminder.key, at).unwrap().is_none());
    store::set_task_done(&conn, id, false).unwrap();
    assert!(claim(&conn, &reminder.key, at).unwrap().is_some());
    assert!(claim(&conn, &reminder.key, at).unwrap().is_none());
    drop(conn);
    let conn = store::open_at(dir.path().join("test.db")).unwrap();
    assert!(list(&conn, at - 1, at + 1).unwrap().is_empty());
    store::update_task(&conn, id, None, None, Some(at + 60), None).unwrap();
    let next = list(&conn, at, at + 61).unwrap().remove(0);
    assert_ne!(next.key, reminder.key);
    store::delete_task(&conn, id).unwrap();
    assert!(claim(&conn, &next.key, at + 60).unwrap().is_none());
}

#[test]
fn calendar_occurrences_remind_separately_and_removed_events_stop() {
    let (_dir, conn) = db();
    let start = store::now_unix() + DAY;
    let value = create_event(
        &conn,
        &EventInput {
            title: "Standup".into(),
            start,
            end: start + 1800,
            repeat: "daily".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let plan = list(&conn, start - LEAD, start + 3 * DAY).unwrap();
    assert!(plan.len() >= 3);
    assert_eq!(plan[0].at, start - LEAD);
    assert_ne!(plan[0].key, plan[1].key);
    assert!(claim(&conn, &plan[0].key, start - LEAD).unwrap().is_some());
    assert!(
        !list(&conn, start - LEAD, start + 3 * DAY)
            .unwrap()
            .iter()
            .any(|r| r.key == plan[0].key)
    );
    calendar::events::delete_event(&conn, value["event"]["id"].as_str().unwrap()).unwrap();
    assert!(list(&conn, start, start + 3 * DAY).unwrap().is_empty());
}

#[test]
fn all_day_event_uses_local_morning_of_utc_calendar_date() {
    let (_dir, conn) = db();
    let start = 1_792_900_000_i64.div_euclid(DAY) * DAY;
    create_event(
        &conn,
        &EventInput {
            title: "Holiday".into(),
            start,
            end: start + DAY,
            all_day: true,
            ..Default::default()
        },
    )
    .unwrap();
    let plan = list(&conn, start - DAY, start + 2 * DAY).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].at, morning(start, true).unwrap());
}

fn age_items(conn: &Connection, at: i64) {
    for table in ["tasks", "events"] {
        conn.execute(
            &format!(
                "UPDATE {table} SET created_at = ?1, updated_at = ?1,
            json = json_set(json, '$.reminder_after', ?1)"
            ),
            params![at],
        )
        .unwrap();
    }
}

#[test]
fn first_use_skips_history_but_keeps_upcoming_items() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    for at in [now - 60, now + 60] {
        let value = crate::tasks::create_task(&conn, "", "Due", "", at, "", "", "").unwrap();
        store::set_task_due_time(&conn, value["task"]["id"].as_str().unwrap(), true).unwrap();
    }
    age_items(&conn, now - DAY);
    let reminders = plan(&conn, now - DAY, now + DAY, now).unwrap();
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].at, now + 60);
    // The activation boundary survives maintenance even after receipts expire.
    claim(&conn, "missing", now + 61 * DAY).unwrap();
    assert_eq!(started_at(&conn, now + 61 * DAY).unwrap(), now);
}

#[test]
fn finished_events_are_not_replayed_but_ongoing_events_can_catch_up() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    started_at(&conn, now - DAY).unwrap();
    for (title, start, end) in [
        ("Finished", now - 1800, now - 900),
        ("Ongoing", now - 100, now + 100),
    ] {
        create_event(
            &conn,
            &EventInput {
                title: title.into(),
                start,
                end,
                ..Default::default()
            },
        )
        .unwrap();
    }
    age_items(&conn, now - DAY);
    let reminders = plan(&conn, now - DAY, now + 1, now).unwrap();
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].title, "Ongoing");
    let finished = list(&conn, now - DAY, now)
        .unwrap()
        .into_iter()
        .find(|r| r.title == "Finished")
        .unwrap();
    assert!(claim(&conn, &finished.key, now).unwrap().is_none());
}

#[test]
fn past_schedules_are_skipped_without_silencing_reminders_on_notes_edits() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    started_at(&conn, now - DAY).unwrap();
    let value = crate::tasks::create_task(&conn, "", "Due", "", now - 60, "", "", "").unwrap();
    let id = value["task"]["id"].as_str().unwrap();
    store::set_task_due_time(&conn, id, true).unwrap();
    assert!(plan(&conn, now - DAY, now + 1, now).unwrap().is_empty());
    age_items(&conn, now - DAY);
    store::update_task(
        &conn,
        id,
        None,
        Some("Edited after the reminder time"),
        None,
        None,
    )
    .unwrap();
    assert_eq!(plan(&conn, now - DAY, now + 1, now).unwrap().len(), 1);
    store::update_task(&conn, id, None, None, Some(now - 120), None).unwrap();
    assert!(plan(&conn, now - DAY, now + 1, now).unwrap().is_empty());
}

#[test]
fn new_events_inside_the_lead_time_notify_at_their_start() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    create_event(
        &conn,
        &EventInput {
            title: "Soon".into(),
            start: now + 60,
            end: now + 600,
            ..Default::default()
        },
    )
    .unwrap();
    let reminders = plan(&conn, now - DAY, now + DAY, now).unwrap();
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].at, now + 60);
}

#[test]
fn permission_enable_skips_past_and_native_receipts_suppress_replay() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    started_at(&conn, now - DAY).unwrap();
    let value = crate::tasks::create_task(&conn, "", "Due", "", now - 60, "", "", "").unwrap();
    store::set_task_due_time(&conn, value["task"]["id"].as_str().unwrap(), true).unwrap();
    age_items(&conn, now - DAY);
    let reminder = plan(&conn, now - DAY, now + DAY, now).unwrap().remove(0);
    dispatch(
        &conn,
        "reminders.acknowledge",
        &json!({"keys": [&reminder.key]}),
    )
    .unwrap();
    assert!(plan(&conn, now - DAY, now + DAY, now).unwrap().is_empty());
    conn.execute(
        "DELETE FROM reminder_deliveries WHERE key <> ?1",
        params![START_KEY],
    )
    .unwrap();
    assert_eq!(plan(&conn, now - DAY, now + DAY, now).unwrap().len(), 1);
    dispatch(&conn, "reminders.skipPast", &json!({})).unwrap();
    assert!(plan(&conn, now - DAY, now + DAY, now).unwrap().is_empty());
}

#[test]
fn events_rescheduled_inside_the_lead_time_notify_at_their_new_start() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    started_at(&conn, now - DAY).unwrap();
    let value = create_event(
        &conn,
        &EventInput {
            title: "Moved".into(),
            start: now + 3600,
            end: now + 7200,
            ..Default::default()
        },
    )
    .unwrap();
    age_items(&conn, now - DAY);
    calendar::events::update_event(
        &conn,
        value["event"]["id"].as_str().unwrap(),
        &EventInput {
            title: "Moved".into(),
            start: now + 60,
            end: now + 600,
            ..Default::default()
        },
    )
    .unwrap();
    let reminders = plan(&conn, now - DAY, now + DAY, now).unwrap();
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].at, now + 60);
}

#[test]
fn invitation_text_updates_do_not_rearm_a_delivered_reminder() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    started_at(&conn, now - DAY).unwrap();
    let start = now + 60;
    let end = now + 3600;
    let stamp = |at| {
        chrono::DateTime::from_timestamp(at, 0)
            .unwrap()
            .format("%Y%m%dT%H%M%SZ")
            .to_string()
    };
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:one\r\nDTSTART:{}\r\nDTEND:{}\r\nSUMMARY:Original\r\nSEQUENCE:1\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        stamp(start),
        stamp(end)
    );
    store::save_event(
        &conn,
        &store::CalendarEvent {
            id: "invite".into(),
            title: "Original".into(),
            start_at: start,
            end_at: end,
            invite_uid: "one".into(),
            ics,
            ..Default::default()
        },
    )
    .unwrap();
    age_items(&conn, now - DAY);
    let reminder = plan(&conn, now - DAY, now + DAY, now).unwrap().remove(0);
    assert!(claim(&conn, &reminder.key, now).unwrap().is_some());
    let mut event = store::event(&conn, "invite").unwrap().unwrap();
    event.title = "Updated".into();
    event.ics = event
        .ics
        .replace("SUMMARY:Original", "SUMMARY:Updated")
        .replace("SEQUENCE:1", "SEQUENCE:2");
    store::save_event(&conn, &event).unwrap();
    assert!(plan(&conn, now - DAY, now + DAY, now).unwrap().is_empty());
    event.start_at = now + 120;
    event.ics = event.ics.replace(&stamp(start), &stamp(now + 120));
    store::save_event(&conn, &event).unwrap();
    let reminders = plan(&conn, now - DAY, now + DAY, now).unwrap();
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].at, now + 120);
}

#[test]
fn a_repeat_rule_update_does_not_replay_an_already_delivered_occurrence() {
    let (_dir, conn) = db();
    let now = store::now_unix();
    started_at(&conn, now - DAY).unwrap();
    let start = now + 60;
    let value = create_event(
        &conn,
        &EventInput {
            title: "Repeat".into(),
            start,
            end: start + 3600,
            ..Default::default()
        },
    )
    .unwrap();
    age_items(&conn, now - DAY);
    let before = plan(&conn, now - DAY, start + 1, now).unwrap().remove(0);
    assert!(claim(&conn, &before.key, now).unwrap().is_some());
    calendar::events::update_event(
        &conn,
        value["event"]["id"].as_str().unwrap(),
        &EventInput {
            title: "Repeat".into(),
            start,
            end: start + 3600,
            repeat: "daily".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(plan(&conn, now - DAY, start + 1, now).unwrap().is_empty());
    let next = plan(&conn, start + 1, now + 3 * DAY, now).unwrap();
    assert!(!next.is_empty());
}

#[test]
fn event_lead_time_follows_the_setting() {
    let (_dir, conn) = db();
    let start = store::now_unix() + DAY;
    create_event(
        &conn,
        &EventInput {
            title: "Review".into(),
            start,
            end: start + 1800,
            ..Default::default()
        },
    )
    .unwrap();
    let at = |conn: &Connection| list(conn, start - DAY, start + 1).unwrap()[0].at;
    assert_eq!(at(&conn), start - LEAD);
    store::setting_set(&conn, LEAD_SETTING_KEY, &json!(60)).unwrap();
    assert_eq!(at(&conn), start - 3600);
    store::setting_set(&conn, LEAD_SETTING_KEY, &json!(0)).unwrap();
    assert_eq!(at(&conn), start);
    store::setting_set(&conn, LEAD_SETTING_KEY, &json!(-5)).unwrap();
    assert_eq!(at(&conn), start - LEAD);
}

#[test]
fn default_alert_can_be_turned_off() {
    let (_dir, conn) = db();
    let start = store::now_unix() + DAY;
    create_event(
        &conn,
        &EventInput {
            title: "Quiet".into(),
            start,
            end: start + 1800,
            ..Default::default()
        },
    )
    .unwrap();
    store::setting_set(&conn, LEAD_SETTING_KEY, &json!(NO_ALERT)).unwrap();
    assert!(list(&conn, start - DAY, start + 1).unwrap().is_empty());
}

#[test]
fn an_event_with_two_alerts_reminds_at_each_and_receipts_stay_per_alert() {
    let (_dir, conn) = db();
    let start = store::now_unix() + DAY;
    let value = create_event(
        &conn,
        &EventInput {
            title: "Flight".into(),
            start,
            end: start + 3600,
            alerts: Some(Some(vec![60, 5, 60, -3])),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(value["event"]["alerts"], json!([60, 5]));
    let plan = list(&conn, start - DAY, start + 1).unwrap();
    let times: Vec<i64> = plan.iter().map(|r| r.at).collect();
    assert_eq!(times, vec![start - 3600, start - 300]);
    assert!(claim(&conn, &plan[0].key, start - 3600).unwrap().is_some());
    let rest = list(&conn, start - DAY, start + 1).unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].at, start - 300);
}

#[test]
fn event_alerts_override_the_default_and_survive_edits_that_omit_them() {
    let (_dir, conn) = db();
    let start = store::now_unix() + DAY;
    let value = create_event(
        &conn,
        &EventInput {
            title: "Muted".into(),
            start,
            end: start + 1800,
            alerts: Some(Some(vec![])),
            ..Default::default()
        },
    )
    .unwrap();
    let id = value["event"]["id"].as_str().unwrap().to_string();
    assert!(list(&conn, start - DAY, start + 1).unwrap().is_empty());
    let rename = EventInput {
        title: "Still muted".into(),
        start,
        end: start + 1800,
        ..Default::default()
    };
    calendar::events::update_event(&conn, &id, &rename).unwrap();
    assert!(list(&conn, start - DAY, start + 1).unwrap().is_empty());
    let follow_default = EventInput {
        alerts: Some(None),
        ..rename
    };
    let value = calendar::events::update_event(&conn, &id, &follow_default).unwrap();
    assert_eq!(value["event"]["alerts"], Value::Null);
    assert_eq!(
        list(&conn, start - DAY, start + 1).unwrap()[0].at,
        start - LEAD
    );
}

#[test]
fn an_all_day_event_with_alerts_off_stays_quiet() {
    let (_dir, conn) = db();
    let day = (store::now_unix() / DAY + 2) * DAY;
    create_event(
        &conn,
        &EventInput {
            title: "Birthday".into(),
            start: day,
            end: day + DAY,
            all_day: true,
            alerts: Some(Some(vec![])),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(list(&conn, day - DAY, day + 2 * DAY).unwrap().is_empty());
}

#[test]
fn alerts_round_trip_through_undo() {
    let (_dir, conn) = db();
    let start = store::now_unix() + DAY;
    let value = create_event(
        &conn,
        &EventInput {
            title: "Undo".into(),
            start,
            end: start + 1800,
            alerts: Some(Some(vec![30])),
            ..Default::default()
        },
    )
    .unwrap();
    let id = value["event"]["id"].as_str().unwrap();
    let deleted = calendar::events::delete_event(&conn, id).unwrap();
    calendar::events::restore_event(&conn, &deleted["restore"]).unwrap();
    assert_eq!(
        store::event(&conn, id).unwrap().unwrap().alerts,
        Some(vec![30])
    );
}

#[test]
fn all_day_alerts_fire_at_nine_on_the_day_or_days_before() {
    let (_dir, conn) = db();
    let day = (store::now_unix() / DAY + 10) * DAY;
    let value = create_event(
        &conn,
        &EventInput {
            title: "Trip".into(),
            start: day,
            end: day + DAY,
            all_day: true,
            ..Default::default()
        },
    )
    .unwrap();
    let id = value["event"]["id"].as_str().unwrap().to_string();
    let times = |conn: &Connection| {
        list(conn, day - 9 * DAY, day + 2 * DAY)
            .unwrap()
            .iter()
            .map(|r| r.at)
            .collect::<Vec<_>>()
    };
    assert_eq!(times(&conn), vec![morning(day, true).unwrap()]);
    store::setting_set(&conn, ALL_DAY_SETTING_KEY, &json!(1)).unwrap();
    assert_eq!(times(&conn), vec![morning(day - DAY, true).unwrap()]);
    store::setting_set(&conn, ALL_DAY_SETTING_KEY, &json!(NO_ALERT)).unwrap();
    assert!(times(&conn).is_empty());
    let own = EventInput {
        title: "Trip".into(),
        start: day,
        end: day + DAY,
        all_day: true,
        alerts: Some(Some(vec![7, 0, 30])),
        ..Default::default()
    };
    let value = calendar::events::update_event(&conn, &id, &own).unwrap();
    assert_eq!(value["event"]["alerts"], json!([7, 0]));
    assert_eq!(
        times(&conn),
        vec![
            morning(day - 7 * DAY, true).unwrap(),
            morning(day, true).unwrap()
        ]
    );
    // Turning the event into a timed one drops day counts it cannot use.
    let timed = EventInput {
        title: "Trip".into(),
        start: day + 3600,
        end: day + 7200,
        ..Default::default()
    };
    let value = calendar::events::update_event(&conn, &id, &timed).unwrap();
    assert_eq!(value["event"]["alerts"], Value::Null);
}
