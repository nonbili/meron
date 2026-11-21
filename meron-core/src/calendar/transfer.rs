//! File-based iCalendar transfer. Each stored object contains one UID and its
//! recurrence overrides, even when the input file contains many series.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Result, anyhow, bail, ensure};
use calcard::icalendar::{
    ICalendar, ICalendarComponentType as Kind, ICalendarEntry, ICalendarParameterName,
    ICalendarParameterValue, ICalendarProperty as Prop, ICalendarStatus, ICalendarValue,
};
use rusqlite::Connection;
use serde::Serialize;

use crate::{
    caldav::ics,
    store::{self, CalendarEvent},
};

use super::{parse_ical, text_of};

pub const MAX_FILE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Default, Serialize)]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
}

pub fn import(conn: &mut Connection, input: &str, preview: bool) -> Result<ImportSummary> {
    ensure!(input.len() <= MAX_FILE_BYTES, "Calendar file exceeds 8 MiB");
    let input = input.trim_start_matches('\u{feff}').trim();
    ensure!(
        input
            .lines()
            .next()
            .is_some_and(|line| line.trim().eq_ignore_ascii_case("BEGIN:VCALENDAR"))
            && input
                .lines()
                .last()
                .is_some_and(|line| line.trim().eq_ignore_ascii_case("END:VCALENDAR")),
        "Expected an iCalendar file"
    );
    let calendar = parse_ical(input).ok_or_else(|| anyhow!("Invalid iCalendar file"))?;

    let tx = conn.transaction_with_behavior(if preview {
        rusqlite::TransactionBehavior::Deferred
    } else {
        rusqlite::TransactionBehavior::Immediate
    })?;
    let known: HashSet<_> = store::all_events(&tx)?
        .iter()
        .filter(|event| event.calendar.is_empty())
        .map(event_uid)
        .collect();
    let mut summary = ImportSummary::default();
    let mut groups = BTreeMap::<String, Vec<u32>>::new();
    let mut zones = HashMap::new();
    for &id in &calendar.components[0].component_ids {
        let component = &calendar.components[id as usize];
        match component.component_type {
            Kind::VEvent => {
                let uid = text_of(component, Prop::Uid);
                if uid.is_empty() {
                    summary.skipped += 1;
                } else {
                    groups.entry(uid).or_default().push(id);
                }
            }
            Kind::VTimezone => {
                zones.insert(text_of(component, Prop::Tzid), id);
            }
            _ => summary.skipped += 1,
        }
    }
    let method = text_of(&calendar.components[0], Prop::Method).to_ascii_uppercase();
    let resolver = calendar.build_tz_resolver();
    let mut stored_bytes = 0;
    for (uid, ids) in groups {
        let mut masters = ids
            .iter()
            .filter(|&&id| !calendar.components[id as usize].is_recurrence_override());
        let master = masters.next().copied();
        let primary = &calendar.components[master.unwrap_or(ids[0]) as usize];
        if !matches!(method.as_str(), "" | "PUBLISH" | "REQUEST")
            || primary.status() == Some(&ICalendarStatus::Cancelled)
            || masters.next().is_some()
            || (master.is_none() && ids.len() > 1)
        {
            summary.skipped += 1;
            continue;
        }
        if known.contains(&uid) {
            summary.duplicates += 1;
            continue;
        }

        let mut object = empty_calendar();
        for id in ids {
            append_component(&mut object, &calendar, id);
        }
        let used_zones = object
            .components
            .iter()
            .flat_map(|component| component.entries.iter().filter_map(|entry| entry.tz_id()))
            .collect::<std::collections::BTreeSet<_>>();
        let zone_ids: Vec<_> = used_zones
            .iter()
            .filter_map(|id| zones.get(*id).copied())
            .collect();
        if used_zones.iter().any(|id| resolver.resolve(id).is_none()) {
            summary.skipped += 1;
            continue;
        }
        for id in zone_ids {
            append_component(&mut object, &calendar, id);
        }
        let Some(mut event) = ics::event_from_ics(&object.to_string()) else {
            summary.skipped += 1;
            continue;
        };
        // Shared timezone definitions can make the stored series larger than
        // the original combined calendar.
        stored_bytes += event.ics.len();
        ensure!(
            stored_bytes <= 4 * MAX_FILE_BYTES,
            "Calendar expands beyond the import size limit"
        );
        if !preview {
            event.id = crate::tasks::new_id("event");
            store::save_event(&tx, &event)?;
        }
        summary.imported += 1;
    }
    tx.commit()?;
    Ok(summary)
}

pub fn export(conn: &Connection, event_id: Option<&str>, calendar_id: &str) -> Result<String> {
    let events = if let Some(id) = event_id {
        vec![store::event(conn, id)?.ok_or_else(|| anyhow!("Event no longer exists"))?]
    } else {
        store::all_events(conn)?
            .into_iter()
            .filter(|event| event.calendar == calendar_id)
            .collect()
    };
    if events.is_empty() {
        bail!("This calendar has no events to export");
    }
    let mut output = empty_calendar();
    let mut zone_names = HashMap::<_, String>::new();
    let mut used_names = HashSet::new();
    for event in events {
        let raw = if event.ics.is_empty() {
            ics::new_ics(&event, &event_uid(&event))
        } else {
            event.ics
        };
        let mut source =
            parse_ical(&raw).ok_or_else(|| anyhow!("Unable to export a stored event"))?;
        let children = source.components[0].component_ids.clone();
        for &id in &children {
            if source.components[id as usize].component_type != Kind::VTimezone {
                continue;
            }
            let name = text_of(&source.components[id as usize], Prop::Tzid);
            if name.is_empty() {
                continue;
            }
            let mut zone = empty_calendar();
            append_component(&mut zone, &source, id);
            let key = (name.clone(), zone.to_string());
            if let Some(existing) = zone_names.get(&key) {
                rename_zone(&mut source, id, &name, existing);
                continue;
            }
            let mut exported_name = name.clone();
            // Independently stored events may have different definitions for
            // the same TZID. Give each definition a distinct name in the file.
            if !used_names.insert(exported_name.clone()) {
                let mut suffix = used_names.len();
                loop {
                    exported_name = format!("Meron/Timezone-{suffix}");
                    if used_names.insert(exported_name.clone()) {
                        break;
                    }
                    suffix += 1;
                }
                rename_zone(&mut source, id, &name, &exported_name);
            }
            append_component(&mut output, &source, id);
            zone_names.insert(key, exported_name);
        }
        for id in children {
            if source.components[id as usize].component_type == Kind::VEvent {
                append_component(&mut output, &source, id);
            }
        }
    }
    Ok(output.to_string())
}

fn event_uid(event: &CalendarEvent) -> String {
    // The local row ID survives edits, undo and backup restores.
    ics::uid_of(&event.ics).unwrap_or_else(|| format!("{}@meron", event.id))
}

fn empty_calendar() -> ICalendar {
    parse_ical("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Meron//Meron//EN\r\nEND:VCALENDAR\r\n")
        .expect("valid calendar template")
}

/// Copy a component and its descendants, remapping the flat tree's indices.
fn append_component(target: &mut ICalendar, source: &ICalendar, root: u32) {
    let mut pending = vec![(0, root)];
    while let Some((parent, index)) = pending.pop() {
        let id = target.components.len() as u32;
        let mut component = source.components[index as usize].clone();
        pending.extend(
            component
                .component_ids
                .drain(..)
                .rev()
                .map(|child| (id as usize, child)),
        );
        target.components[parent].component_ids.push(id);
        target.components.push(component);
    }
}

fn rename_zone(calendar: &mut ICalendar, zone: u32, name: &str, new: &str) {
    if name == new {
        return;
    }
    let component = &mut calendar.components[zone as usize];
    // Keep the IANA association for clients (including Meron) that use it to
    // resolve dates while other clients can use the copied observances.
    if let Ok(tz) = name.parse::<calcard::common::timezone::Tz>() {
        let location = Prop::Other("X-LIC-LOCATION".into());
        component.entries.retain(|entry| entry.name != location);
        component.entries.push(ICalendarEntry {
            name: location,
            params: vec![],
            values: vec![ICalendarValue::Text(
                tz.name()
                    .map_or_else(|| name.to_string(), |name| name.into_owned()),
            )],
        });
    }
    for entry in &mut component.entries {
        if entry.name == Prop::Tzid {
            entry.values = vec![ICalendarValue::Text(new.to_string())];
        }
    }
    for entry in calendar
        .components
        .iter_mut()
        .flat_map(|component| &mut component.entries)
    {
        for param in &mut entry.params {
            if param.name == ICalendarParameterName::Tzid && param.value.as_text() == Some(name) {
                param.value = ICalendarParameterValue::Text(new.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::calendar::events::{CUSTOM_REPEAT, EventInput, events_payload, update_event};

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        store::run_migrations(&conn).unwrap();
        conn
    }

    fn at(value: &str) -> i64 {
        value.parse::<DateTime<Utc>>().unwrap().timestamp()
    }

    const CALENDAR: &str = "BEGIN:VCALENDAR\r
VERSION:2.0\r
PRODID:-//Fictional Calendar//EN\r
METHOD:REQUEST\r
BEGIN:VTIMEZONE\r
TZID:Europe/Berlin\r
BEGIN:STANDARD\r
DTSTART:19701025T030000\r
TZOFFSETFROM:+0200\r
TZOFFSETTO:+0100\r
RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r
END:STANDARD\r
BEGIN:DAYLIGHT\r
DTSTART:19700329T020000\r
TZOFFSETFROM:+0100\r
TZOFFSETTO:+0200\r
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r
END:DAYLIGHT\r
END:VTIMEZONE\r
BEGIN:VEVENT\r
UID:weekly@example.test\r
DTSTAMP:20260301T000000Z\r
DTSTART;TZID=Europe/Berlin:20260323T090000\r
DTEND;TZID=Europe/Berlin:20260323T100000\r
RRULE:FREQ=WEEKLY;COUNT=4\r
EXDATE;TZID=Europe/Berlin:20260406T090000\r
SUMMARY:Studio review\r
ORGANIZER:mailto:organizer@example.test\r
ATTENDEE:mailto:guest@example.test\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:festival@example.test\r
DTSTAMP:20260301T000000Z\r
DTSTART;VALUE=DATE:20260325\r
DTEND;VALUE=DATE:20260327\r
SUMMARY:Festival\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:weekly@example.test\r
DTSTAMP:20260301T000000Z\r
RECURRENCE-ID;TZID=Europe/Berlin:20260413T090000\r
DTSTART;TZID=Europe/Berlin:20260413T110000\r
DTEND;TZID=Europe/Berlin:20260413T120000\r
SUMMARY:Moved review\r
END:VEVENT\r
BEGIN:VTODO\r
UID:todo@example.test\r
SUMMARY:Pick up supplies\r
END:VTODO\r
END:VCALENDAR\r
";

    #[test]
    fn previews_imports_edits_and_round_trips_a_calendar() {
        let mut conn = db();
        let preview = import(&mut conn, CALENDAR, true).unwrap();
        assert_eq!((preview.imported, preview.skipped), (2, 1));
        assert!(store::all_events(&conn).unwrap().is_empty());

        assert_eq!(import(&mut conn, CALENDAR, false).unwrap().imported, 2);
        let again = import(&mut conn, CALENDAR, false).unwrap();
        assert_eq!((again.imported, again.duplicates, again.skipped), (0, 2, 1));

        let stored = store::all_events(&conn).unwrap();
        let series = &stored[0];
        assert!(series.recurring && series.rrule.is_empty());
        assert!(series.invite_uid.is_empty() && series.response.is_empty());
        assert!(series.calendar.is_empty());
        update_event(
            &conn,
            &series.id,
            &EventInput {
                title: "Weekly review".into(),
                start: series.start_at,
                end: series.end_at,
                repeat: CUSTOM_REPEAT.into(),
                ..Default::default()
            },
        )
        .unwrap();

        let single = export(&conn, Some(&series.id), "").unwrap();
        assert_eq!(single.matches("BEGIN:VEVENT").count(), 2);
        assert!(single.contains("RECURRENCE-ID") && single.contains("EXDATE"));
        assert!(!single.contains("METHOD") && !single.contains("Festival"));
        assert!(single.contains("ORGANIZER:mailto:organizer@example.test"));
        let full = export(&conn, None, "").unwrap();
        assert_eq!(full.matches("BEGIN:VCALENDAR").count(), 1);
        assert_eq!(full.matches("BEGIN:VEVENT").count(), 3);
        assert_eq!(full.matches("BEGIN:VTIMEZONE").count(), 1);

        let mut restored = db();
        assert_eq!(import(&mut restored, &full, false).unwrap().imported, 2);
        let result = events_payload(
            &restored,
            at("2026-03-22T00:00:00Z"),
            at("2026-04-15T00:00:00Z"),
            false,
        )
        .unwrap();
        let occurrences = result["events"].as_array().unwrap();
        let entries: Vec<_> = occurrences
            .iter()
            .map(|item| {
                (
                    item["title"].as_str().unwrap(),
                    item["start"].as_i64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            entries,
            [
                ("Weekly review", at("2026-03-23T08:00:00Z")),
                ("Festival", at("2026-03-25T00:00:00Z")),
                ("Weekly review", at("2026-03-30T07:00:00Z")),
                ("Moved review", at("2026-04-13T09:00:00Z")),
            ]
        );
        assert_eq!(occurrences[1]["all_day"], true);
        assert_eq!(occurrences[1]["end"], at("2026-03-27T00:00:00Z"));
    }

    #[test]
    fn gives_native_events_stable_uids_and_skips_them_on_reimport() {
        let mut conn = db();
        let mut event = CalendarEvent {
            id: "event-native".into(),
            title: "Sketch; \u{1f3a8}, revise".into(),
            start_at: at("2026-03-23T08:00:00Z"),
            end_at: at("2026-03-23T09:00:00Z"),
            ..Default::default()
        };
        store::save_event(&conn, &event).unwrap();
        let first = export(&conn, Some(&event.id), "").unwrap();
        assert_eq!(ics::event_from_ics(&first).unwrap().title, event.title);
        event.title = "Sketch again".into();
        store::save_event(&conn, &event).unwrap();
        let second = export(&conn, Some(&event.id), "").unwrap();
        assert_eq!(ics::uid_of(&first), ics::uid_of(&second));
        assert_eq!(import(&mut conn, &first, false).unwrap().duplicates, 1);
        assert_eq!(store::all_events(&conn).unwrap()[0].title, event.title);
    }

    #[test]
    fn ignores_scheduling_cancellations_and_unrecognized_timezones() {
        let mut conn = db();
        let cancelled = CALENDAR.replace("METHOD:REQUEST", "METHOD:CANCEL");
        assert_eq!(import(&mut conn, &cancelled, false).unwrap().skipped, 3);
        let unknown = CALENDAR.replace("Europe/Berlin", "Fictional/Timezone");
        let summary = import(&mut conn, &unknown, false).unwrap();
        assert_eq!((summary.imported, summary.skipped), (1, 2));
    }

    #[test]
    fn shows_an_early_override_and_rdate_without_moving_the_series_start() {
        let file = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:series\r\n\
            DTSTART:20260406T090000Z\r\nDTEND:20260406T100000Z\r\nRRULE:FREQ=WEEKLY;COUNT=3\r\n\
            RDATE;VALUE=PERIOD:20260329T090000Z/PT1H\r\nSUMMARY:Review\r\nEND:VEVENT\r\n\
            BEGIN:VEVENT\r\nUID:series\r\nRECURRENCE-ID:20260406T090000Z\r\n\
            DTSTART:20260330T090000Z\r\nDTEND:20260330T100000Z\r\n\
            SUMMARY:Moved review\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let mut conn = db();
        assert_eq!(import(&mut conn, file, false).unwrap().imported, 1);
        let series = &store::all_events(&conn).unwrap()[0];
        assert_eq!(series.start_at, at("2026-04-06T09:00:00Z"));
        update_event(
            &conn,
            &series.id,
            &EventInput {
                title: "Renamed review".into(),
                start: series.start_at,
                end: series.end_at,
                repeat: CUSTOM_REPEAT.into(),
                ..Default::default()
            },
        )
        .unwrap();
        let march = events_payload(
            &conn,
            at("2026-03-01T00:00:00Z"),
            at("2026-04-01T00:00:00Z"),
            false,
        )
        .unwrap();
        let occurrences = march["events"].as_array().unwrap();
        assert_eq!(occurrences.len(), 2);
        assert_eq!(occurrences[0]["start"], at("2026-03-29T09:00:00Z"));
        assert_eq!(occurrences[1]["start"], at("2026-03-30T09:00:00Z"));
        assert_eq!(occurrences[1]["title"], "Moved review");
        assert_eq!(occurrences[1]["series_date"], "2026-04-06");
    }

    #[test]
    fn expands_this_and_future_overrides_without_rewriting_file_revisions() {
        let file = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:series\r\n\
            SEQUENCE:0\r\nDTSTART:20261008T170000Z\r\nDTEND:20261008T180000Z\r\n\
            RRULE:FREQ=DAILY;COUNT=3\r\nSUMMARY:Review\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:series\r\n\
            RECURRENCE-ID;RANGE=THISANDFUTURE:20261009T170000Z\r\nSEQUENCE:1\r\n\
            DTSTART:20261009T190000Z\r\nDTEND:20261009T200000Z\r\nSUMMARY:Review\r\n\
            END:VEVENT\r\nEND:VCALENDAR\r\n";
        let mut conn = db();
        assert_eq!(import(&mut conn, file, false).unwrap().imported, 1);
        let result = events_payload(
            &conn,
            at("2026-10-08T00:00:00Z"),
            at("2026-10-11T00:00:00Z"),
            false,
        )
        .unwrap();
        let occurrences = result["events"].as_array().unwrap();
        assert_eq!(occurrences.len(), 3);
        assert_eq!(occurrences[0]["start"], at("2026-10-08T17:00:00Z"));
        assert_eq!(occurrences[1]["start"], at("2026-10-09T19:00:00Z"));
        assert_eq!(occurrences[2]["start"], at("2026-10-10T19:00:00Z"));
        let output = export(&conn, None, "").unwrap();
        assert!(output.contains("SEQUENCE:0") && output.contains("SEQUENCE:1"));
    }

    #[test]
    fn an_aliased_timezone_keeps_the_last_day_of_a_series() {
        let conn = db();
        for (id, start, rule) in [
            ("weekly", "2026-03-23T08:00:00Z", "weekly"),
            ("daily", "2028-06-25T21:00:00Z", "FREQ=DAILY;UNTIL=20280630"),
        ] {
            let start_at = at(start);
            store::save_event(
                &conn,
                &CalendarEvent {
                    id: id.into(),
                    title: id.into(),
                    start_at,
                    end_at: start_at + 3600,
                    recurring: true,
                    rrule: rule.into(),
                    tzid: "Europe/Berlin".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }

        let output = export(&conn, None, "").unwrap();
        assert!(output.contains("DTSTART;TZID=Meron/Timezone-"));
        let mut restored = db();
        assert_eq!(import(&mut restored, &output, false).unwrap().imported, 2);
        let result = events_payload(
            &restored,
            at("2028-06-25T00:00:00Z"),
            at("2028-07-02T00:00:00Z"),
            false,
        )
        .unwrap();
        let starts: Vec<_> = result["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["title"] == "daily")
            .map(|item| item["start"].as_i64().unwrap())
            .collect();
        assert_eq!(starts.len(), 6);
        assert_eq!(starts[5], at("2028-06-30T21:00:00Z"));
    }

    #[test]
    fn exports_different_definitions_for_the_same_timezone() {
        let conn = db();
        for (id, date) in [
            ("first", "2026-03-23T08:00:00Z"),
            ("second", "2028-03-23T08:00:00Z"),
        ] {
            let start_at = at(date);
            store::save_event(
                &conn,
                &CalendarEvent {
                    id: id.into(),
                    title: id.into(),
                    start_at,
                    end_at: start_at + 3600,
                    recurring: true,
                    rrule: "weekly".into(),
                    tzid: "Europe/Berlin".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let output = export(&conn, None, "").unwrap();
        assert_eq!(output.matches("BEGIN:VTIMEZONE").count(), 2);
        let mut restored = db();
        assert_eq!(import(&mut restored, &output, false).unwrap().imported, 2);
        let saved = store::all_events(&restored).unwrap();
        assert_eq!(saved[0].start_at, at("2026-03-23T08:00:00Z"));
        let second = &saved[1];
        assert_eq!(second.start_at, at("2028-03-23T08:00:00Z"));
        assert_eq!(second.tzid, "Europe/Berlin");
        assert_eq!(second.rrule, "weekly");

        update_event(
            &restored,
            &second.id,
            &EventInput {
                title: second.title.clone(),
                start: second.start_at + 3600,
                end: second.end_at + 3600,
                repeat: second.rrule.clone(),
                ..Default::default()
            },
        )
        .unwrap();
        let result = events_payload(
            &restored,
            at("2028-03-30T00:00:00Z"),
            at("2028-03-31T00:00:00Z"),
            false,
        )
        .unwrap();
        let occurrence = result["events"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == second.id)
            .unwrap();
        assert_eq!(occurrence["start"], at("2028-03-30T08:00:00Z"));
    }
}
