//! Between a server's calendar object and a row in `events`.
//!
//! Reading uses the same parser as invitations. Writing back does not
//! regenerate the object: a server's event carries attendees, alarms,
//! categories, exceptions and properties no client here knows, and all of
//! them must survive a change of title. So an edit rewrites only the
//! properties it changed, in the series' master VEVENT, and leaves every
//! other line as the server sent it.

use calcard::common::timezone::Tz;

use crate::calendar::events::{date_lines, rrule_ics, rule_tz};
use crate::calendar::rrule::{self, Timing};
use crate::store::CalendarEvent;

/// What a server object says about the event it holds, in the shape the
/// `events` table keeps. `None` when it holds no VEVENT (a VTODO, a journal).
pub fn event_from_ics(ics: &str) -> Option<CalendarEvent> {
    let invite = crate::calendar::parse_invite(ics)?;
    let lines = unfold(ics);
    let master = master_event(&lines);
    // The editor's repeats say a rule and nothing else: a series with dates
    // added, skipped or moved is one only its server can keep.
    let has_exceptions = master.is_some_and(|(start, end)| {
        top_level(&lines, start, end).any(|l| matches!(name_of(l).as_str(), "EXDATE" | "RDATE"))
    }) || lines.iter().any(|l| name_of(l) == "RECURRENCE-ID");
    let dtstart = master
        .and_then(|(start, end)| top_level(&lines, start, end).find(|l| name_of(l) == "DTSTART"));
    // A repeat keeps its wall-clock time in the zone DTSTART names, when that
    // is one the editor can use again.
    let named_zone = dtstart.and_then(|line| param_of(line, "TZID"));
    let editor_zone = named_zone.as_ref().and_then(|name| {
        if name.parse::<Tz>().is_ok() {
            return Some(name.clone());
        }
        let resolved = resolve_timezone_alias(ics, name)?;
        if matches!(resolved, Tz::Tz(_)) {
            resolved.name().map(|name| name.into_owned())
        } else {
            None
        }
    });
    let own_zone = named_zone.is_some() && editor_zone.is_none();
    let tzid = editor_zone
        .filter(|_| invite.recurring && !invite.all_day)
        .unwrap_or_default();
    let series = CalendarEvent {
        all_day: invite.all_day,
        tzid: tzid.clone(),
        ..Default::default()
    };
    let tz = rule_tz(&series);
    let timing = if invite.all_day {
        Timing::AllDay
    } else if dtstart.is_some_and(|l| is_floating(l)) {
        Timing::Floating
    } else {
        Timing::Zoned(&tz)
    };
    let rrule = master
        .and_then(|(start, end)| top_level(&lines, start, end).find(|l| name_of(l) == "RRULE"))
        .filter(|_| !has_exceptions && !own_zone)
        .and_then(|line| rrule::from_ics(value_of(line), timing, invite.start))
        .unwrap_or_default();
    Some(CalendarEvent {
        title: invite.summary,
        notes: invite.description,
        location: invite.location,
        start_at: invite.start,
        end_at: invite.end,
        all_day: invite.all_day,
        recurring: invite.recurring,
        rrule,
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
    // An end date is written as DTSTART says times (a date, a floating time
    // or an instant), so a series whose start changes how, or when in the
    // day, has its end written again with it.
    let rewrite_rule =
        rule_changed || (start_changed && after.recurring && after.rrule.contains("UNTIL="));
    // A floating time (a wall-clock time in no zone, which every device
    // reads as its own) stays floating rather than turning into UTC.
    let floating = !after.all_day
        && after.tzid.is_empty()
        && top_level(&lines, start, end)
            .find(|l| name_of(l) == "DTSTART")
            .is_some_and(|l| is_floating(l));
    if timing_changed {
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
    if rewrite_rule {
        replace.push("RRULE");
        if !after.recurring {
            replace.extend(["RDATE", "EXDATE"]);
        }
        if !after.rrule.is_empty() {
            add.push(format!("RRULE:{}", rrule_ics(after, floating)));
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

/// `ics` as the series' occurrences are worked out from it: an RRULE's UTC
/// end (RFC 5545 asks for one when DTSTART names a zone) restated as a wall
/// time in that zone. calcard compares the end with each occurrence's wall
/// time before placing it in the zone, so a UTC end would cut a series in a
/// zone ahead of UTC short by the occurrences of its last day.
pub fn for_expansion(ics: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let lines = unfold(ics);
    let Some((start, end)) = master_event(&lines) else {
        return Cow::Borrowed(ics);
    };
    let Some(tz) = top_level(&lines, start, end)
        .find(|l| name_of(l) == "DTSTART")
        .and_then(|l| param_of(l, "TZID"))
        .and_then(|name| {
            name.parse::<Tz>()
                .ok()
                .or_else(|| resolve_timezone_alias(ics, &name))
        })
    else {
        return Cow::Borrowed(ics);
    };
    let local_until = |part: &str| -> Option<String> {
        let (key, value) = part.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("UNTIL") {
            return None;
        }
        let instant =
            chrono::NaiveDateTime::parse_from_str(value.trim().strip_suffix('Z')?, "%Y%m%dT%H%M%S")
                .ok()?;
        use chrono::TimeZone as _;
        Some(format!(
            "UNTIL={}",
            tz.from_utc_datetime(&instant)
                .naive_local()
                .format("%Y%m%dT%H%M%S")
        ))
    };
    let mut changed = false;
    let mut depth = 0usize;
    let mut out = lines.clone();
    for line in out.iter_mut().take(end).skip(start + 1) {
        match name_of(line).as_str() {
            "BEGIN" => depth += 1,
            "END" => depth = depth.saturating_sub(1),
            "RRULE" if depth == 0 => {
                let value = value_of(line);
                let rule: Vec<String> = value
                    .split(';')
                    .map(|part| local_until(part).unwrap_or_else(|| part.to_string()))
                    .collect();
                let rule = rule.join(";");
                if rule != value {
                    let head = line[..line.len() - value.len()].to_string();
                    *line = format!("{head}{rule}");
                    changed = true;
                }
            }
            _ => {}
        }
    }
    if !changed {
        return Cow::Borrowed(ics);
    }
    let mut folded = String::new();
    for line in out {
        fold_into(&mut folded, &line);
    }
    Cow::Owned(folded)
}

/// A VTIMEZONE can name the IANA zone it represents in X-LIC-LOCATION.
fn resolve_timezone_alias(ics: &str, name: &str) -> Option<Tz> {
    crate::calendar::parse_ical(ics)?
        .build_tz_resolver()
        .resolve(name)
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

/// Whether an object's series starts at a wall-clock time in no zone.
pub(crate) fn starts_floating(ics: &str) -> bool {
    let lines = unfold(ics);
    master_event(&lines).is_some_and(|(start, end)| {
        top_level(&lines, start, end)
            .find(|l| name_of(l) == "DTSTART")
            .is_some_and(|l| is_floating(l))
    })
}

/// A DTSTART naming a wall-clock time in no zone.
fn is_floating(line: &str) -> bool {
    let value = value_of(line).trim();
    param_of(line, "TZID").is_none() && value.contains('T') && !value.ends_with('Z')
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
    fn rules_the_editor_can_say_round_trip_and_others_stay_the_servers() {
        let ics = |rule: &str| {
            format!(
                "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:r\r\nDTSTART;TZID=Asia/Tokyo:20261123T090000\r\n\
                 DTEND;TZID=Asia/Tokyo:20261123T100000\r\nRRULE:{rule}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
            )
        };
        assert_eq!(event_from_ics(&ics("FREQ=WEEKLY")).unwrap().rrule, "weekly");
        assert_eq!(
            event_from_ics(&ics("FREQ=DAILY;INTERVAL=1")).unwrap().rrule,
            "daily"
        );
        assert_eq!(event_from_ics(&ics("FREQ=HOURLY")).unwrap().rrule, "");
        assert_eq!(
            event_from_ics(&ics("FREQ=MONTHLY;BYDAY=MO,TU;BYSETPOS=-1"))
                .unwrap()
                .rrule,
            ""
        );

        // Every other Monday and Wednesday until 30 June, 09:00 Tokyo.
        let server = ics("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;UNTIL=20270630T000000Z");
        let before = event_from_ics(&server).unwrap();
        assert_eq!(
            before.rrule,
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;UNTIL=20270630"
        );
        // Unchanged, the server's own line stays as it was.
        assert_eq!(patch(&server, &before, &before), server);

        let after = CalendarEvent {
            rrule: "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=10".into(),
            ..before.clone()
        };
        let patched = patch(&server, &before, &after);
        assert!(
            patched.contains("RRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=10\r\n"),
            "{patched}"
        );
        let after = CalendarEvent {
            rrule: "FREQ=WEEKLY;BYDAY=MO;UNTIL=20270701".into(),
            ..before.clone()
        };
        let patched = patch(&server, &before, &after);
        // The end of 1 July in Tokyo, as an instant (RFC 5545 §3.3.10).
        assert!(
            patched.contains("RRULE:FREQ=WEEKLY;BYDAY=MO;UNTIL=20270701T145959Z\r\n"),
            "{patched}"
        );
        assert_eq!(event_from_ics(&patched).unwrap().rrule, after.rrule);
    }

    #[test]
    fn an_end_date_follows_the_start_when_it_changes_form() {
        // Weekly 09:00 Tokyo, ending with 30 June's occurrence itself.
        let server = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:r\r\nDTSTART;TZID=Asia/Tokyo:20261123T090000\r\n\
                      DTEND;TZID=Asia/Tokyo:20261123T100000\r\nRRULE:FREQ=WEEKLY;UNTIL=20270630T000000Z\r\n\
                      END:VEVENT\r\nEND:VCALENDAR\r\n";
        let before = event_from_ics(server).unwrap();
        assert_eq!(before.rrule, "FREQ=WEEKLY;UNTIL=20270630");

        // All day now: the end becomes a date, as DTSTART is.
        let all_day = CalendarEvent {
            all_day: true,
            tzid: String::new(),
            start_at: before.start_at.div_euclid(86_400) * 86_400,
            end_at: before.start_at.div_euclid(86_400) * 86_400 + 86_400,
            ..before.clone()
        };
        let patched = patch(server, &before, &all_day);
        assert!(
            patched.contains("RRULE:FREQ=WEEKLY;UNTIL=20270630\r\n"),
            "{patched}"
        );

        // Later in the day: the end is the end of 30 June still, not 09:00.
        let later = CalendarEvent {
            start_at: before.start_at + 3 * 3600,
            end_at: before.end_at + 3 * 3600,
            ..before.clone()
        };
        let patched = patch(server, &before, &later);
        assert!(
            patched.contains("RRULE:FREQ=WEEKLY;UNTIL=20270630T145959Z\r\n"),
            "{patched}"
        );
    }

    #[test]
    fn a_utc_series_counts_its_end_date_in_utc() {
        // Daily 13:00 UTC, ending with 30 December's occurrence, read the same
        // on any device.
        let server = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:u\r\nDTSTART:20260701T130000Z\r\n\
                      DTEND:20260701T140000Z\r\nRRULE:FREQ=DAILY;UNTIL=20261230T130000Z\r\n\
                      END:VEVENT\r\nEND:VCALENDAR\r\n";
        let event = event_from_ics(server).unwrap();
        assert_eq!(event.rrule, "FREQ=DAILY;UNTIL=20261230");
        let changed = CalendarEvent {
            rrule: "FREQ=DAILY;UNTIL=20261231".into(),
            ..event.clone()
        };
        let patched = patch(server, &event, &changed);
        assert!(
            patched.contains("RRULE:FREQ=DAILY;UNTIL=20261231T235959Z\r\n"),
            "{patched}"
        );
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
