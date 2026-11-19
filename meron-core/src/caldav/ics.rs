//! Between a server's calendar object and a row in `events`.
//!
//! Reading uses the same parser as invitations. Writing back does not
//! regenerate the object: a server's event carries attendees, alarms,
//! categories, exceptions and properties no client here knows, and all of
//! them must survive a change of title. So an edit rewrites only the
//! properties it changed, in the series' master VEVENT, and leaves every
//! other line as the server sent it.

use crate::calendar::events::date_lines;
use crate::store::CalendarEvent;

/// The repeat rules the editor offers, as an RRULE says them.
const SIMPLE_FREQS: [(&str, &str); 4] = [
    ("DAILY", "daily"),
    ("WEEKLY", "weekly"),
    ("MONTHLY", "monthly"),
    ("YEARLY", "yearly"),
];

/// What a server object says about the event it holds, in the shape the
/// `events` table keeps. `None` when it holds no VEVENT (a VTODO, a journal).
pub fn event_from_ics(ics: &str) -> Option<CalendarEvent> {
    let invite = crate::calendar::parse_invite(ics)?;
    let lines = unfold(ics);
    let master = master_event(&lines);
    // The editor's repeat choices say how often and nothing else: a series
    // with dates added, skipped or moved is one only its server can keep.
    let has_exceptions = master.is_some_and(|(start, end)| {
        top_level(&lines, start, end).any(|l| matches!(name_of(l).as_str(), "EXDATE" | "RDATE"))
    }) || lines.iter().any(|l| name_of(l) == "RECURRENCE-ID");
    let rrule = master
        .and_then(|(start, end)| top_level(&lines, start, end).find(|l| name_of(l) == "RRULE"))
        .and_then(|line| simple_rrule(value_of(line)))
        .filter(|_| !has_exceptions)
        .unwrap_or_default();
    // A repeat keeps its wall-clock time in the zone DTSTART names, when that
    // is one the editor can use again.
    let tzid = master
        .and_then(|(start, end)| top_level(&lines, start, end).find(|l| name_of(l) == "DTSTART"))
        .and_then(|line| param_of(line, "TZID"))
        .filter(|tzid| tzid.parse::<calcard::common::timezone::Tz>().is_ok())
        .filter(|_| invite.recurring && !invite.all_day)
        .unwrap_or_default();
    Some(CalendarEvent {
        title: invite.summary,
        notes: invite.description,
        location: invite.location,
        start_at: invite.start,
        end_at: invite.end,
        all_day: invite.all_day,
        recurring: invite.recurring,
        rrule: rrule.to_string(),
        tzid,
        ics: ics.to_string(),
        ..Default::default()
    })
}

/// The UID of the object's event (or to-do).
pub fn uid_of(ics: &str) -> Option<String> {
    let lines = unfold(ics);
    lines
        .iter()
        .find(|line| name_of(line) == "UID")
        .map(|line| unescape(value_of(line)))
        .filter(|uid| !uid.is_empty())
}

/// A new object for an event made here in a synced calendar.
pub fn new_ics(event: &CalendarEvent, uid: &str) -> String {
    let skeleton = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Meron//Meron//EN\r\nBEGIN:VEVENT\r\nUID:{}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        escape(uid)
    );
    let blank = CalendarEvent {
        start_at: i64::MIN,
        ..Default::default()
    };
    patch(&skeleton, &blank, event)
}

/// `ics` with the master VEVENT brought in line with `after`, rewriting only
/// what differs from `before`.
pub fn patch(ics: &str, before: &CalendarEvent, after: &CalendarEvent) -> String {
    let lines = unfold(ics);
    let Some((start, end)) = master_event(&lines) else {
        return ics.to_string();
    };

    let mut replace: Vec<&str> = Vec::new();
    let mut add: Vec<String> = Vec::new();
    let mut text = |name: &'static str, old: &str, new: &str, replace: &mut Vec<&str>| {
        if old != new {
            replace.push(name);
            if !new.is_empty() {
                add.push(format!("{name}:{}", escape(new)));
            }
        }
    };
    text("SUMMARY", &before.title, &after.title, &mut replace);
    text("DESCRIPTION", &before.notes, &after.notes, &mut replace);
    text("LOCATION", &before.location, &after.location, &mut replace);

    let start_changed = before.start_at != after.start_at
        || before.all_day != after.all_day
        || before.tzid != after.tzid;
    let timing_changed = start_changed || before.end_at != after.end_at;
    let rule_changed = before.recurring != after.recurring || before.rrule != after.rrule;
    if timing_changed {
        // A floating time (a wall-clock time in no zone, which every device
        // reads as its own) stays floating rather than turning into UTC.
        let floating = !after.all_day
            && after.tzid.is_empty()
            && top_level(&lines, start, end)
                .find(|l| name_of(l) == "DTSTART")
                .is_some_and(|l| {
                    let value = value_of(l).trim();
                    param_of(l, "TZID").is_none() && value.contains('T') && !value.ends_with('Z')
                });
        let (dtstart, dtend) = if floating {
            floating_lines(after)
        } else {
            date_lines(after)
        };
        // Only the end moved: DTSTART, and how it is written, stays.
        if start_changed {
            replace.extend(["DTSTART", "DTEND", "DURATION"]);
            add.push(dtstart);
        } else {
            replace.extend(["DTEND", "DURATION"]);
        }
        add.push(dtend);
    }
    if rule_changed {
        replace.push("RRULE");
        if !after.recurring {
            replace.extend(["RDATE", "EXDATE"]);
        }
        if !after.rrule.is_empty() {
            add.push(format!("RRULE:FREQ={}", after.rrule.to_ascii_uppercase()));
        }
    }
    if replace.is_empty() {
        return ics.to_string();
    }

    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    replace.extend(["DTSTAMP", "LAST-MODIFIED"]);
    add.push(format!("DTSTAMP:{stamp}"));
    add.push(format!("LAST-MODIFIED:{stamp}"));
    // RFC 5545 §3.8.7.4: a change of time or recurrence is a new revision.
    if timing_changed || rule_changed {
        let sequence = top_level(&lines, start, end)
            .find(|line| name_of(line) == "SEQUENCE")
            .and_then(|line| value_of(line).trim().parse::<i64>().ok())
            .unwrap_or(0);
        replace.push("SEQUENCE");
        add.push(format!("SEQUENCE:{}", sequence + 1));
    }

    let mut out = Vec::with_capacity(lines.len() + add.len());
    let mut depth = 0usize;
    let mut inserted = false;
    for (index, line) in lines.iter().enumerate() {
        let inside = index > start && index < end;
        if inside {
            let name = name_of(line);
            if name == "BEGIN" {
                if depth == 0 && !inserted {
                    out.extend(add.iter().cloned());
                    inserted = true;
                }
                depth += 1;
            } else if name == "END" {
                depth = depth.saturating_sub(1);
            } else if depth == 0 && replace.contains(&name.as_str()) {
                continue;
            }
        }
        if index == end && !inserted {
            out.extend(add.iter().cloned());
            inserted = true;
        }
        out.push(line.clone());
    }
    // A TZID the object names must be defined in it (RFC 4791 §4.1).
    if timing_changed && !after.all_day && !after.tzid.is_empty() {
        // TZID as a property line only appears inside a VTIMEZONE.
        let defined = out
            .iter()
            .any(|l| name_of(l) == "TZID" && value_of(l).trim() == after.tzid);
        if !defined {
            if let Some(block) = vtimezone(&after.tzid, after.start_at) {
                let at = out
                    .iter()
                    .position(|l| name_of(l) == "BEGIN" && value_of(l).trim() != "VCALENDAR")
                    .unwrap_or(out.len());
                out.splice(at..at, block);
            }
        }
    }
    let mut folded = String::new();
    for line in out {
        fold_into(&mut folded, &line);
    }
    folded
}

/// DTSTART and DTEND as floating wall-clock times, read in this device's
/// zone as the event was.
fn floating_lines(event: &CalendarEvent) -> (String, String) {
    use chrono::TimeZone as _;
    let local = |at: i64| {
        chrono::Local
            .timestamp_opt(at, 0)
            .single()
            .map(|time| time.naive_local().format("%Y%m%dT%H%M%S").to_string())
            .unwrap_or_default()
    };
    (
        format!("DTSTART:{}", local(event.start_at)),
        format!("DTEND:{}", local(event.end_at)),
    )
}

/// A VTIMEZONE for an IANA zone, covering a series from `start` on, as
/// calcard writes one.
fn vtimezone(tzid: &str, start: i64) -> Option<Vec<String>> {
    const YEAR: i64 = 366 * 86_400;
    let mut ical =
        crate::calendar::parse_ical("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n")?;
    ical.add_timezone(tzid, start - YEAR, start + 30 * YEAR)?;
    let lines = unfold(&ical.to_string());
    let begin = lines
        .iter()
        .position(|l| name_of(l) == "BEGIN" && value_of(l).trim() == "VTIMEZONE")?;
    let end = lines[begin..]
        .iter()
        .position(|l| name_of(l) == "END" && value_of(l).trim() == "VTIMEZONE")?;
    Some(lines[begin..=begin + end].to_vec())
}

/// The editor's name for an RRULE that says nothing more than how often.
fn simple_rrule(rule: &str) -> Option<&'static str> {
    let mut freq = None;
    for part in rule.split(';').filter(|p| !p.is_empty()) {
        let (key, value) = part.split_once('=')?;
        match key.to_ascii_uppercase().as_str() {
            "FREQ" => freq = Some(value.to_ascii_uppercase()),
            "INTERVAL" if value.trim() == "1" => {}
            // WKST changes nothing for a rule without BYDAY.
            "WKST" => {}
            _ => return None,
        }
    }
    let freq = freq?;
    SIMPLE_FREQS
        .iter()
        .find(|(name, _)| *name == freq)
        .map(|(_, simple)| *simple)
}

/// Content lines with folding undone (RFC 5545 §3.1).
fn unfold(ics: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        match raw.chars().next() {
            Some(' ' | '\t') if !lines.is_empty() => {
                lines.last_mut().unwrap().push_str(&raw[1..]);
            }
            _ if raw.is_empty() => {}
            _ => lines.push(raw.to_string()),
        }
    }
    lines
}

/// Fold to 75 octets per line, never inside a UTF-8 character.
fn fold_into(out: &mut String, line: &str) {
    let mut width = 0;
    for ch in line.chars() {
        let len = ch.len_utf8();
        if width + len > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(ch);
        width += len;
    }
    out.push_str("\r\n");
}

/// The property name of a content line, upper-cased.
fn name_of(line: &str) -> String {
    line.split([';', ':'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase()
}

/// The value of a content line: after the first colon outside a quoted
/// parameter.
fn value_of(line: &str) -> &str {
    let mut quoted = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            ':' if !quoted => return &line[index + 1..],
            _ => {}
        }
    }
    ""
}

fn param_of(line: &str, param: &str) -> Option<String> {
    let head = &line[..line.len() - value_of(line).len()];
    head.trim_end_matches(':').split(';').skip(1).find_map(|p| {
        let (key, value) = p.split_once('=')?;
        key.eq_ignore_ascii_case(param)
            .then(|| value.trim_matches('"').to_string())
    })
}

/// Line indexes of the master VEVENT's BEGIN and END: the one without a
/// RECURRENCE-ID, or the first when every one has (an object holding only
/// overrides).
fn master_event(lines: &[String]) -> Option<(usize, usize)> {
    let mut events = Vec::new();
    let mut stack: Vec<(String, usize)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let name = name_of(line);
        if name == "BEGIN" {
            stack.push((value_of(line).trim().to_ascii_uppercase(), index));
        } else if name == "END" {
            if let Some((kind, begin)) = stack.pop() {
                if kind == "VEVENT" && stack.len() == 1 {
                    events.push((begin, index));
                }
            }
        }
    }
    events
        .iter()
        .copied()
        .find(|&(start, end)| !top_level(lines, start, end).any(|l| name_of(l) == "RECURRENCE-ID"))
        .or_else(|| events.first().copied())
}

/// The VEVENT's own property lines, without those of a nested VALARM.
fn top_level(lines: &[String], start: usize, end: usize) -> impl Iterator<Item = &String> {
    let mut depth = 0usize;
    lines[start + 1..end].iter().filter(move |line| {
        match name_of(line).as_str() {
            "BEGIN" => {
                depth += 1;
                return false;
            }
            "END" => {
                depth = depth.saturating_sub(1);
                return false;
            }
            _ => {}
        }
        depth == 0
    })
}

/// TEXT value escaping (RFC 5545 §3.3.11).
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEETING: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Server//EN\r\nBEGIN:VEVENT\r\nUID:abc-1\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261120T090000Z\r\nDTEND:20261120T100000Z\r\nSUMMARY:Planning\r\nORGANIZER;CN=\"Doe, Jane\":mailto:jane@example.com\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:alice@example.com\r\nX-SERVER-THING:keep me\r\nSEQUENCE:2\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:Reminder\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn reads_an_event() {
        let event = event_from_ics(MEETING).unwrap();
        assert_eq!(event.title, "Planning");
        assert_eq!(event.start_at, 1_795_165_200);
        assert_eq!(event.end_at - event.start_at, 3600);
        assert!(!event.recurring);
        assert_eq!(uid_of(MEETING).as_deref(), Some("abc-1"));
        assert!(
            event_from_ics(
                "BEGIN:VCALENDAR\r\nBEGIN:VTODO\r\nUID:t\r\nEND:VTODO\r\nEND:VCALENDAR\r\n"
            )
            .is_none()
        );
    }

    #[test]
    fn a_title_edit_keeps_everything_else() {
        let before = event_from_ics(MEETING).unwrap();
        let after = CalendarEvent {
            title: "Planning; Q4, final".into(),
            ..before.clone()
        };
        let patched = patch(MEETING, &before, &after);
        assert!(patched.contains("SUMMARY:Planning\\; Q4\\, final\r\n"));
        assert!(!patched.contains("SUMMARY:Planning\r\n"));
        for kept in [
            "ORGANIZER;CN=\"Doe, Jane\":mailto:jane@example.com",
            "X-SERVER-THING:keep me",
            "DTSTART:20261120T090000Z",
            "SEQUENCE:2",
            // The alarm's own DESCRIPTION is not the event's.
            "DESCRIPTION:Reminder",
            "TRIGGER:-PT15M",
        ] {
            assert!(patched.contains(kept), "lost {kept}:\n{patched}");
        }
        assert_eq!(
            event_from_ics(&patched).unwrap().title,
            "Planning; Q4, final"
        );
    }

    #[test]
    fn a_move_rewrites_times_and_bumps_the_sequence() {
        let before = event_from_ics(MEETING).unwrap();
        let after = CalendarEvent {
            start_at: before.start_at + 3600,
            end_at: before.end_at + 7200,
            ..before.clone()
        };
        let patched = patch(MEETING, &before, &after);
        assert!(patched.contains("DTSTART:20261120T100000Z"));
        assert!(patched.contains("DTEND:20261120T120000Z"));
        assert!(patched.contains("SEQUENCE:3"));
        assert_eq!(patched.matches("DTSTART").count(), 1);
        let reread = event_from_ics(&patched).unwrap();
        assert_eq!(
            (reread.start_at, reread.end_at),
            (after.start_at, after.end_at)
        );
    }

    #[test]
    fn overrides_are_left_alone() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:r\r\nRECURRENCE-ID:20261127T090000Z\r\nDTSTART:20261127T110000Z\r\nDTEND:20261127T120000Z\r\nSUMMARY:Moved\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:r\r\nDTSTART:20261120T090000Z\r\nDTEND:20261120T100000Z\r\nRRULE:FREQ=WEEKLY;COUNT=4\r\nSUMMARY:Weekly\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let before = event_from_ics(ics).unwrap();
        assert!(before.recurring);
        // COUNT is more than the editor can say, so it shows no simple repeat.
        assert_eq!(before.rrule, "");
        let after = CalendarEvent {
            title: "Standup".into(),
            ..before.clone()
        };
        let patched = patch(ics, &before, &after);
        assert!(patched.contains("SUMMARY:Moved"));
        assert!(patched.contains("SUMMARY:Standup"));
        assert!(patched.contains("RRULE:FREQ=WEEKLY;COUNT=4"));
    }

    #[test]
    fn a_zoned_repeat_carries_its_zone() {
        let event = CalendarEvent {
            title: "Standup".into(),
            start_at: 1_795_165_200,
            end_at: 1_795_166_100,
            recurring: true,
            rrule: "weekly".into(),
            tzid: "Europe/Berlin".into(),
            ..Default::default()
        };
        let ics = new_ics(&event, "z-1");
        assert!(ics.contains("DTSTART;TZID=Europe/Berlin:"));
        assert!(
            ics.contains("BEGIN:VTIMEZONE\r\nTZID:Europe/Berlin"),
            "{ics}"
        );
        assert!(ics.find("BEGIN:VTIMEZONE") < ics.find("BEGIN:VEVENT"));
        let reread = event_from_ics(&ics).unwrap();
        assert_eq!(
            (reread.start_at, reread.rrule.as_str()),
            (event.start_at, "weekly")
        );
        // Moving it again does not add a second definition.
        let moved = patch(
            &ics,
            &reread,
            &CalendarEvent {
                start_at: event.start_at + 3600,
                end_at: event.end_at + 3600,
                ..reread.clone()
            },
        );
        assert_eq!(moved.matches("BEGIN:VTIMEZONE").count(), 1);
    }

    #[test]
    fn a_floating_series_stays_floating() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:f\r\nDTSTART:20261005T090000\r\nDTEND:20261005T100000\r\nRRULE:FREQ=WEEKLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let before = event_from_ics(ics).unwrap();
        // A longer meeting: only DTEND changes.
        let longer = patch(
            ics,
            &before,
            &CalendarEvent {
                end_at: before.end_at + 1800,
                ..before.clone()
            },
        );
        assert!(longer.contains("DTSTART:20261005T090000\r\n"), "{longer}");
        assert!(longer.contains("DTEND:20261005T103000\r\n"), "{longer}");
        // A later one: still floating.
        let later = patch(
            ics,
            &before,
            &CalendarEvent {
                start_at: before.start_at + 3600,
                end_at: before.end_at + 3600,
                ..before.clone()
            },
        );
        assert!(later.contains("DTSTART:20261005T100000\r\n"), "{later}");
        assert!(!later.contains("T100000Z"));
    }

    #[test]
    fn a_weekly_series_with_exceptions_is_not_simple() {
        let skipped = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nDTSTART:20261005T090000Z\r\nDTEND:20261005T100000Z\r\nRRULE:FREQ=WEEKLY\r\nEXDATE:20261012T090000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        assert_eq!(event_from_ics(skipped).unwrap().rrule, "");
        let plain = skipped.replace("EXDATE:20261012T090000Z\r\n", "");
        assert_eq!(event_from_ics(&plain).unwrap().rrule, "weekly");
    }

    #[test]
    fn simple_rules_round_trip() {
        assert_eq!(simple_rrule("FREQ=WEEKLY"), Some("weekly"));
        assert_eq!(simple_rrule("FREQ=DAILY;INTERVAL=1"), Some("daily"));
        assert_eq!(simple_rrule("FREQ=WEEKLY;BYDAY=MO,WE"), None);
        assert_eq!(simple_rrule("FREQ=MONTHLY;INTERVAL=2"), None);
        assert_eq!(simple_rrule("FREQ=HOURLY"), None);
    }

    #[test]
    fn a_new_event_is_complete() {
        let event = CalendarEvent {
            title: "Lunch".into(),
            notes: "Line one\nline two".into(),
            start_at: 1_795_165_200,
            end_at: 1_795_168_800,
            ..Default::default()
        };
        let ics = new_ics(&event, "uid-1");
        assert!(ics.contains("UID:uid-1\r\n"));
        assert!(ics.contains("DESCRIPTION:Line one\\nline two\r\n"));
        assert!(ics.contains("DTSTAMP:"));
        let reread = event_from_ics(&ics).unwrap();
        assert_eq!(reread.title, "Lunch");
        assert_eq!(reread.notes, "Line one\nline two");
        assert_eq!(
            (reread.start_at, reread.end_at),
            (event.start_at, event.end_at)
        );
    }

    #[test]
    fn long_lines_fold_on_character_boundaries() {
        let mut out = String::new();
        fold_into(&mut out, &format!("SUMMARY:{}", "日本語".repeat(20)));
        assert!(out.split("\r\n").all(|line| line.len() <= 75));
        assert_eq!(unfold(&out)[0], format!("SUMMARY:{}", "日本語".repeat(20)));
    }
}
