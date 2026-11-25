use super::*;

mod basic_tests;
mod custom_rule_tests;
mod declined_tests;
mod occurrence_tests;
mod revision_tests;
mod synced_tests;

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

const SERIES: &str = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
DTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\nSUMMARY:Review\r\n\
RRULE:FREQ=WEEKLY;COUNT=3\r\nORGANIZER:mailto:maya@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

fn occurrence_at(rid: &str, extra: &str) -> crate::calendar::Invite {
    crate::calendar::parse_invite(&format!(
        "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:inv-s\r\n\
         RECURRENCE-ID:{rid}\r\n{extra}SUMMARY:Review\r\n\
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

/// An event synced from a CalDAV calendar, repeating by a rule the editor has
/// no name for.
fn synced_custom_repeat(conn: &Connection) -> String {
    synced_event(
        conn,
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:standup\r\nDTSTART:20261005T090000Z\r\nDTEND:20261005T091500Z\r\nRRULE:FREQ=MONTHLY;BYDAY=-1MO,-1WE\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
    )
}

/// An event synced from a CalDAV calendar, as `ics` has it.
fn synced_event(conn: &Connection, ics: &str) -> String {
    let calendars = crate::caldav::store::reconcile_calendars(
        conn,
        "dav-1",
        &[crate::caldav::Calendar {
            url: "https://dav.example.com/cal/".into(),
            name: "Work".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    let mut event = crate::caldav::ics::event_from_ics(ics).unwrap();
    event.id = "event-standup".into();
    event.calendar = calendars[0].id.clone();
    store::save_event(conn, &event).unwrap();
    crate::caldav::store::save_object(
        conn,
        &crate::caldav::store::DavObject {
            calendar: calendars[0].id.clone(),
            url: "https://dav.example.com/cal/standup.ics".into(),
            etag: "\"1\"".into(),
            local_id: event.id.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    event.id
}
