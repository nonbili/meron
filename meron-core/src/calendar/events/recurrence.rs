use anyhow::{Result, anyhow};
use calcard::common::{PartialDateTime, timezone::Tz};
use calcard::icalendar::{
    ICalendarComponentType, ICalendarPeriod, ICalendarProperty, ICalendarValue,
};
use chrono::{NaiveDate, TimeZone};

use super::DAY;
use crate::calendar::{self, Invite, Response};
use crate::{caldav, store::CalendarEvent};

/// How many occurrences of one repeating event are walked from its start.
/// Expansion always begins at DTSTART, so this bounds how far a range can lie
/// past it: a daily event reaches more than fifty years.
const MAX_OCCURRENCES: usize = 20_000;

/// One time an event happens. An occurrence the organizer changed on its own
/// (an override VEVENT) can carry its own title and location.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Occurrence {
    pub(super) start: i64,
    pub(super) end: i64,
    /// The VEVENT it came from, so two occurrences that land on the same
    /// times — one moved onto another — stay two.
    component: u32,
    /// An override that is all-day while its series is timed, or the other
    /// way round.
    pub(super) all_day: Option<bool>,
    pub(super) title: Option<String>,
    pub(super) location: Option<String>,
    /// The user's answer about this occurrence alone, when it differs from
    /// the event's (see [`OCCURRENCE_RESPONSE`]).
    pub(super) response: Option<String>,
}

/// Where an occurrence's own answer is kept in a stored invitation: a private
/// property on its override VEVENT, which never leaves the device.
pub(super) const OCCURRENCE_RESPONSE: &str = "X-MERON-RESPONSE";

/// An override's SEQUENCE as the organizer sent it, kept beside the series'
/// one it is given (see [`put_override`]) so a late, older revision of the
/// occurrence is known for one. A private property, like [`OCCURRENCE_RESPONSE`].
pub(super) const OWN_SEQUENCE: &str = "X-MERON-SEQUENCE";

/// Note an override's own SEQUENCE before it takes the series', once: an
/// override filed again keeps what was first noted.
fn keep_own_sequence(component: &mut calcard::icalendar::ICalendarComponent) {
    let name = ICalendarProperty::Other(OWN_SEQUENCE.to_string());
    if component.property(&name).is_some() {
        return;
    }
    // None given is 0 (RFC 5545), not the series' count it is about to take.
    let sequence = component
        .property(&ICalendarProperty::Sequence)
        .and_then(|entry| entry.values.first())
        .and_then(ICalendarValue::as_integer)
        .unwrap_or(0);
    component.add_property(name, ICalendarValue::Text(sequence.to_string()));
}

/// An event's SEQUENCE as the organizer sent it.
pub(super) fn own_sequence(component: &calcard::icalendar::ICalendarComponent) -> i64 {
    calendar::text_of(
        component,
        ICalendarProperty::Other(OWN_SEQUENCE.to_string()),
    )
    .parse()
    .ok()
    .or_else(|| {
        component
            .property(&ICalendarProperty::Sequence)
            .and_then(|entry| entry.values.first())
            .and_then(ICalendarValue::as_integer)
    })
    .unwrap_or(0)
}

/// An event's occurrences that overlap `[from, to)`, less those the user,
/// by one of `mine`, declined.
pub(super) fn occurrences(
    event: &CalendarEvent,
    from: i64,
    to: i64,
    mine: &std::collections::HashSet<String>,
) -> Vec<Occurrence> {
    let overlaps =
        |start: i64, end: i64| start < to && (end > from || (end <= start && start >= from));
    // Only an object that says DECLINED somewhere, in any case, is read for
    // it: the word compared only where a `d` or `D` is found.
    let may_be_declined = !mine.is_empty() && {
        let bytes = event.ics.as_bytes();
        memchr::memchr2_iter(b'd', b'D', bytes).any(|at| {
            bytes
                .get(at..at + b"DECLINED".len())
                .is_some_and(|word| word.eq_ignore_ascii_case(b"DECLINED"))
        })
    };
    let single = || {
        overlaps(event.start_at, event.end_at)
            .then(|| Occurrence {
                start: event.start_at,
                end: event.end_at,
                component: 0,
                all_day: None,
                title: None,
                location: None,
                response: None,
            })
            .into_iter()
            .collect()
    };
    if !event.recurring {
        let declined = may_be_declined
            && calendar::parse_ical(&event.ics).is_some_and(|ical| {
                // Its one VEVENT: a lone date of a series may be an override.
                ical.components
                    .iter()
                    .find(|component| component.component_type == ICalendarComponentType::VEvent)
                    .and_then(|component| calendar::own_partstat(component, mine))
                    .is_some_and(|answer| answer == "declined")
            });
        return if declined { Vec::new() } else { single() };
    }
    let ics = if event.ics.is_empty() {
        repeat_ics(event)
    } else {
        event.ics.clone()
    };
    let Some(mut ical) = calendar::parse_ical(&caldav::ics::for_expansion(&ics)) else {
        return single();
    };
    // calcard pairs overrides with their series by SEQUENCE. Imported files
    // can give the two different revisions. Align the in-memory copy for
    // expansion so the file's original revisions survive editing and export.
    align_override_sequences(&mut ical);
    let expanded = ical.expand_dates(expansion_tz(&ical, event.all_day), MAX_OCCURRENCES);
    // calcard leaves out EXDATEs, and dates an override replaces, from the
    // RRULE's dates but not from RDATE's, and a THISANDFUTURE override's later
    // dates are checked against their moved times rather than their original
    // ones, so every occurrence's original time is checked here.
    let excluded = exdates(&ical);
    let overridden: Vec<i64> = overrides(&ical)
        .filter_map(|component| recurrence_instant(&ical, component))
        .collect();
    let ends_before = ends_before(&ical);
    let endless = !event.all_day
        && master_index(&ical).is_some_and(|master| {
            let master = &ical.components[master];
            !master.has_property(&ICalendarProperty::Dtend)
                && !master.has_property(&ICalendarProperty::Duration)
        });
    // Each override's first expanded start, which its own DTSTART stands for:
    // with RANGE=THISANDFUTURE calcard gives the same override every later
    // occurrence too, each its original time moved by the override's own move.
    let mut override_origins = std::collections::HashMap::new();
    // Each date by the answer on its own VEVENT: the series', or an
    // override's (one date declined, or accepted, on its own).
    let declined = |component: &calcard::icalendar::ICalendarComponent| {
        may_be_declined
            && calendar::own_partstat(component, mine).is_some_and(|answer| answer == "declined")
    };
    let mut out: Vec<Occurrence> = expanded
        .events
        .iter()
        .filter_map(|occurrence| {
            let component = ical.components.get(occurrence.comp_id as usize)?;
            if component.component_type != ICalendarComponentType::VEvent {
                return None;
            }
            let expanded = occurrence.timestamps().0;
            let rid = component
                .is_recurrence_override()
                .then(|| recurrence_instant(&ical, component))
                .flatten();
            let origin = rid.map(|_| {
                *override_origins
                    .entry(occurrence.comp_id)
                    .or_insert(expanded)
            });
            // The time this occurrence had before any override moved it.
            let original = match (rid, origin) {
                (Some(rid), Some(origin)) => expanded - (origin - rid),
                _ => expanded,
            };
            // Cancelled from a date on (RANGE=THISANDFUTURE) — except an
            // occurrence accepted on its own since, which the cut removed any
            // earlier override of.
            let accepted_since = rid == Some(original);
            if !accepted_since && ends_before.is_some_and(|cut| original >= cut) {
                return None;
            }
            // An override the organizer made all-day (or timed) on its own is
            // read from its own DTSTART and DTEND: calcard reads its dates in
            // the series' zone. With RANGE=THISANDFUTURE calcard gives the
            // same override every later occurrence too, so each of those is
            // the override's own bounds moved by how far its expanded start
            // is from the first one's — whole days for an all-day one.
            let own_bounds = component
                .is_recurrence_override()
                .then(|| override_bounds(&ical, component))
                .flatten()
                .map(|(start, end, all_day)| {
                    // One of the same type is its own bounds, and a later
                    // occurrence of a THISANDFUTURE one is its expanded start
                    // moved as far as the first one's was from them — calcard
                    // reads a floating time in the series' zone rather than
                    // the device's — with the override's length, where calcard
                    // keeps the series'.
                    if all_day == event.all_day {
                        let moved = expanded + (start - origin.unwrap_or(expanded));
                        return (moved, moved + (end - start), all_day);
                    }
                    // Counted from the first occurrence it covers: for an
                    // all-day override in whole dates between the original and
                    // the overridden one in the series' zone — across a DST
                    // change they are 23 or 25 hours apart.
                    let origin = origin.unwrap_or(expanded);
                    let offset = if !all_day {
                        expanded - origin
                    } else if let Some(rid) = rid {
                        let zone = occurrence.start.timezone();
                        let date =
                            |at: i64| zone.timestamp_opt(at, 0).single().map(|t| t.date_naive());
                        match (date(original), date(rid)) {
                            (Some(to), Some(from)) => (to - from).num_days() * DAY,
                            _ => (expanded - origin).div_euclid(DAY) * DAY,
                        }
                    } else {
                        (expanded - origin).div_euclid(DAY) * DAY
                    };
                    (start + offset, end + offset, all_day)
                });
            let (start, end) = own_bounds
                .map(|(start, end, _)| (start, end))
                .unwrap_or_else(|| {
                    let (start, end) = occurrence.timestamps();
                    // A timed series with no end lasts an instant (RFC 5545
                    // 3.6.1); calcard runs each occurrence to the end of its day.
                    (start, if endless { start } else { end })
                });
            // A cancelled occurrence (an override saying so) is not on.
            if !overlaps(start, end)
                || declined(component)
                || excluded.contains(&original)
                || (!component.is_recurrence_override() && overridden.contains(&original))
                || component.status() == Some(&calcard::icalendar::ICalendarStatus::Cancelled)
            {
                return None;
            }
            let own = |prop: ICalendarProperty| {
                component
                    .is_recurrence_override()
                    .then(|| calendar::text_of(component, prop))
                    .filter(|text| !text.is_empty())
            };
            Some(Occurrence {
                start,
                end,
                component: occurrence.comp_id,
                all_day: own_bounds
                    .map(|(_, _, all_day)| all_day)
                    .filter(|all_day| *all_day != event.all_day),
                title: own(ICalendarProperty::Summary),
                location: own(ICalendarProperty::Location),
                response: own(ICalendarProperty::Other(OCCURRENCE_RESPONSE.to_string())),
            })
        })
        .collect();
    out.sort();
    // calcard can give one date twice (an RDATE on the DTSTART).
    out.dedup_by(|a, b| a.start == b.start && a.end == b.end && a.component == b.component);
    out
}

/// An override's own start, end and whether it is all-day: a date at its UTC
/// midnight (end exclusive, a day when absent), a time in its TZID's zone.
fn override_bounds(
    ical: &calcard::icalendar::ICalendar,
    component: &calcard::icalendar::ICalendarComponent,
) -> Option<(i64, i64, bool)> {
    let entry = component.property(&ICalendarProperty::Dtstart)?;
    let value = entry.values.first()?;
    let all_day = !value.as_partial_date_time()?.has_time();
    let start = value_instant(ical, entry, value)?;
    let end = component
        .property(&ICalendarProperty::Dtend)
        .and_then(|entry| value_instant(ical, entry, entry.values.first()?))
        .or_else(|| {
            match component
                .property(&ICalendarProperty::Duration)?
                .values
                .first()?
            {
                ICalendarValue::Duration(duration) => Some(start + duration.as_seconds()),
                _ => None,
            }
        })
        .unwrap_or(if all_day { start + DAY } else { start })
        .max(start);
    Some((start, end, all_day))
}

/// The zone a calendar's times without a TZID are expanded in. Dates without
/// a time are UTC midnights here, and times without a zone the device's. A
/// time in UTC (`…Z`) or at an offset is in that: calcard expands a repeat in
/// the TZID's zone or the default, not the start's own offset, so a weekly
/// 17:00Z would otherwise land at 17:00 local.
fn expansion_tz(ical: &calcard::icalendar::ICalendar, all_day: bool) -> Tz {
    if all_day {
        return Tz::UTC;
    }
    let offset = ical
        .components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VEvent)
        .filter_map(|component| component.property(&ICalendarProperty::Dtstart))
        .find(|entry| entry.tz_id().is_none())
        .and_then(|entry| entry.values.first()?.as_partial_date_time())
        .and_then(|start| {
            let hours = i32::from(start.tz_hour?);
            let minutes = i32::from(start.tz_minute.unwrap_or(0));
            let seconds = (hours * 3600 + minutes * 60) * if start.tz_minus { -1 } else { 1 };
            chrono::FixedOffset::east_opt(seconds)
        });
    match offset {
        Some(offset) if offset.local_minus_utc() == 0 => Tz::UTC,
        Some(offset) => Tz::Fixed(offset),
        None => calendar::local_tz(),
    }
}

/// The calendar an event made here repeats by: its first occurrence and the
/// rule, timed in the zone it was made in so 09:00 stays 09:00 across DST.
fn repeat_ics(event: &CalendarEvent) -> String {
    let rule = rrule_ics(event, false);
    let (dtstart, dtend) = date_lines(event);
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Meron//Meron//EN\r\nBEGIN:VEVENT\r\n\
         UID:{}\r\n{dtstart}\r\n{dtend}\r\nRRULE:{rule}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
        event.id
    )
}

/// A repeat rule fit for the series as saved, its zone now settled. One not
/// `as_shown` (the series' own, from an editor that leaves moving it here)
/// moves with a series whose first day moved; either is refused when it ends
/// before that day or leaves it out. A rule kept as it was on a series that
/// has not moved is left as it is, the server's own reading of it included.
/// Move a rule the editor placed for the series starting on `from` to the
/// first day as saved, in the series' zone. No-op without one.
pub(super) fn place_rule(event: &mut CalendarEvent, from: Option<NaiveDate>) {
    let Some(from) = from else { return };
    if !event.recurring || event.rrule.is_empty() {
        return;
    }
    let to = first_day(event);
    if from != to {
        event.rrule = calendar::rrule::align(&event.rrule, from, to);
    }
}

pub(super) fn settle_rule(
    before: &CalendarEvent,
    event: &mut CalendarEvent,
    as_shown: bool,
) -> Result<()> {
    if !event.recurring || event.rrule.is_empty() {
        return Ok(());
    }
    let to = first_day(event);
    let from = before.recurring.then(|| first_day(before));
    if let Some(from) = from.filter(|from| *from != to && !as_shown) {
        event.rrule = calendar::rrule::align(&event.rrule, from, to);
    }
    if event.rrule == before.rrule && from == Some(to) {
        return Ok(());
    }
    calendar::rrule::check(&event.rrule, to)
}

/// Before a synced series' object goes: a floating series' times are this
/// device's wall times, so it takes this device's zone to keep them so.
pub(crate) fn keep_floating_times(event: &mut CalendarEvent) {
    if event.recurring
        && !event.all_day
        && event.tzid.is_empty()
        && caldav::ics::starts_floating(&event.ics)
    {
        event.tzid = calendar::local_tzid().unwrap_or_default();
    }
}

/// The zone [`first_day`] counts in, as `event_json` names it.
pub(super) fn series_zone(event: &CalendarEvent) -> String {
    if !event.recurring || event.all_day {
        return String::new();
    }
    match event.tzid.parse::<Tz>() {
        // By its IANA name, which a browser knows, rather than as the object
        // names it ("Eastern Standard Time").
        Ok(tz) if !event.tzid.is_empty() => {
            tz.name().map(|name| name.into_owned()).unwrap_or_default()
        }
        _ if caldav::ics::starts_floating(&event.ics) => String::new(),
        _ => "UTC".into(),
    }
}

/// The zone a series' repeat end date is a date in: its own, UTC for one
/// written in UTC, and this device's for a floating one.
pub(crate) fn rule_tz(event: &CalendarEvent) -> Tz {
    if event.all_day {
        return Tz::UTC;
    }
    match event.tzid.parse::<Tz>() {
        Ok(tz) if !event.tzid.is_empty() => tz,
        // A floating start is read in this device's zone, as are its dates.
        _ if caldav::ics::starts_floating(&event.ics) => calendar::local_tz(),
        _ => Tz::UTC,
    }
}

/// The date a series starts on, as its repeat's end date is counted.
pub(crate) fn first_day(event: &CalendarEvent) -> NaiveDate {
    rule_tz(event)
        .timestamp_opt(event.start_at, 0)
        .single()
        .map(|t| t.date_naive())
        .unwrap_or_default()
}

/// The RRULE value for an event's stored repeat, as its DTSTART is written:
/// `floating` when that is a wall-clock time in no zone.
pub(crate) fn rrule_ics(event: &CalendarEvent, floating: bool) -> String {
    use calendar::rrule::{Timing, to_ics};
    if event.all_day {
        return to_ics(&event.rrule, Timing::AllDay);
    }
    if floating {
        return to_ics(&event.rrule, Timing::Floating);
    }
    to_ics(&event.rrule, Timing::Zoned(&rule_tz(event)))
}

/// The DTSTART and DTEND lines for an event: dates for an all-day one, wall
/// time in its zone for a repeating one, UTC otherwise.
pub(crate) fn date_lines(event: &CalendarEvent) -> (String, String) {
    if event.all_day {
        let date = |at: i64| {
            chrono::DateTime::from_timestamp(at, 0)
                .map(|time| time.format("%Y%m%d").to_string())
                .unwrap_or_default()
        };
        (
            format!("DTSTART;VALUE=DATE:{}", date(event.start_at)),
            format!("DTEND;VALUE=DATE:{}", date(event.end_at)),
        )
    } else {
        match event
            .tzid
            .parse::<Tz>()
            .ok()
            .filter(|_| !event.tzid.is_empty())
        {
            Some(tz) => {
                let local = |at: i64| {
                    tz.timestamp_opt(at, 0)
                        .single()
                        .map(|time| time.naive_local().format("%Y%m%dT%H%M%S").to_string())
                        .unwrap_or_default()
                };
                (
                    format!("DTSTART;TZID={}:{}", event.tzid, local(event.start_at)),
                    format!("DTEND;TZID={}:{}", event.tzid, local(event.end_at)),
                )
            }
            None => {
                let utc = |at: i64| {
                    chrono::DateTime::from_timestamp(at, 0)
                        .map(|time| time.format("%Y%m%dT%H%M%SZ").to_string())
                        .unwrap_or_default()
                };
                (
                    format!("DTSTART:{}", utc(event.start_at)),
                    format!("DTEND:{}", utc(event.end_at)),
                )
            }
        }
    }
}

/// `series_ics` with the occurrence `invite` is about answered: declined, it
/// is excluded (EXDATE); accepted, the organizer's version of it replaces any
/// earlier one (an override VEVENT with its RECURRENCE-ID), which the
/// expansion then shows in place of the series' own. With no `response`, an
/// organizer's update the user has not answered, it takes the series' answer.
pub(super) fn with_occurrence(
    series_ics: &str,
    invite: &Invite,
    response: Option<Response>,
) -> Result<String> {
    let mut series =
        calendar::parse_ical(series_ics).ok_or_else(|| anyhow!("stored event is not iCalendar"))?;
    let update =
        calendar::parse_ical(&invite.ics).ok_or_else(|| anyhow!("invitation is not iCalendar"))?;
    let occurrence = update
        .components
        .iter()
        .find(|component| {
            component.component_type == ICalendarComponentType::VEvent
                && recurrence_instant(&update, component) == Some(invite.recurrence_id)
        })
        .ok_or_else(|| anyhow!("invitation names no occurrence"))?;

    // The update's times may name a zone only it defines.
    merge_timezones(&mut series, &update);
    // An earlier answer about the same occurrence gives way to this one.
    remove_overrides(&mut series, invite.recurrence_id);
    remove_exdate(&mut series, invite.recurrence_id);
    if response == Some(Response::Declined) {
        let rid = occurrence
            .property(&ICalendarProperty::RecurrenceId)
            .cloned()
            .ok_or_else(|| anyhow!("invitation names no occurrence"))?;
        let this_and_future = matches!(
            rid.parameter(&calcard::icalendar::ICalendarParameterName::Range),
            Some(calcard::icalendar::ICalendarParameterValue::Bool(true))
        );
        if this_and_future {
            end_before(&mut series, invite.recurrence_id)?;
        } else {
            add_exdate(&mut series, rid)?;
        }
    } else {
        // Accepted again from the date the series was cut at: it runs on.
        if invite.this_and_future
            && ends_before(&series).is_some_and(|cut| cut >= invite.recurrence_id)
        {
            lift_end(&mut series);
        }
        put_override(&mut series, occurrence.clone(), response);
    }
    Ok(series.to_string())
}

/// `series_ics` leaving out the occurrence at `recurrence_id`, unless it
/// already does or has an answer of its own (an override).
pub(super) fn without_occurrence(
    series_ics: &str,
    recurrence_id: i64,
    all_day: bool,
) -> Result<String> {
    let mut series =
        calendar::parse_ical(series_ics).ok_or_else(|| anyhow!("stored event is not iCalendar"))?;
    let overridden = overrides(&series)
        .any(|component| recurrence_instant(&series, component) == Some(recurrence_id));
    if overridden || exdates(&series).contains(&recurrence_id) {
        return Ok(series_ics.to_string());
    }
    let date = if all_day {
        PartialDateTime::from_date_timestamp(recurrence_id)
    } else {
        PartialDateTime::from_utc_timestamp(recurrence_id)
    };
    add_exdate(
        &mut series,
        calcard::icalendar::ICalendarEntry {
            name: ICalendarProperty::Exdate,
            params: vec![],
            values: vec![ICalendarValue::PartialDateTime(Box::new(date))],
        },
    )?;
    Ok(series.to_string())
}

/// An organizer's series calendar with each override at the series' own
/// SEQUENCE, which calcard needs to pair them (see [`put_override`]).
pub(super) fn with_series_sequence(ics: &str) -> Result<String> {
    let Some(mut series) = calendar::parse_ical(ics) else {
        return Ok(ics.to_string());
    };
    Ok(if align_override_sequences(&mut series) {
        series.to_string()
    } else {
        ics.to_string()
    })
}

fn align_override_sequences(series: &mut calcard::icalendar::ICalendar) -> bool {
    let Some(master) = master_index(series) else {
        return false;
    };
    let sequence = series.components[master]
        .property(&ICalendarProperty::Sequence)
        .cloned();
    let mut changed = false;
    for component in series.components.iter_mut().filter(|component| {
        component.component_type == ICalendarComponentType::VEvent
            && component.is_recurrence_override()
    }) {
        if component.property(&ICalendarProperty::Sequence) != sequence.as_ref() {
            keep_own_sequence(component);
            component
                .entries
                .retain(|entry| entry.name != ICalendarProperty::Sequence);
            component.entries.extend(sequence.clone());
            changed = true;
        }
    }
    changed
}

/// A newly answered `incoming` series, with the exceptions the user already
/// had on the `stored` one carried over: dates declined or cancelled away,
/// and occurrences answered on their own. Re-filing the series must not bring
/// back a meeting the user took out, nor undo a moved one they accepted. What
/// the organizer's new calendar says about a date itself wins.
pub(super) fn carry_exceptions(stored: &str, incoming: &str) -> Result<String> {
    let mut series =
        calendar::parse_ical(incoming).ok_or_else(|| anyhow!("invitation is not iCalendar"))?;
    let Some(old) = calendar::parse_ical(stored) else {
        return Ok(incoming.to_string());
    };
    // What is carried over may name a zone only the stored calendar defines.
    merge_timezones(&mut series, &old);
    // A series cut short stays cut: the organizer's own occurrences past the
    // cut go, while the user's own answers carried below stay.
    if let Some(cut) = ends_before(&old) {
        end_before(&mut series, cut)?;
    }
    // An occurrence the user already has at a newer revision than the one
    // this calendar embeds (a late copy of the series) keeps the newer.
    let stored_revision = |rid: i64| {
        overrides(&old)
            .filter(|component| recurrence_instant(&old, component) == Some(rid))
            .map(own_sequence)
            .max()
    };
    let older: Vec<i64> = overrides(&series)
        .filter_map(|component| {
            let rid = recurrence_instant(&series, component)?;
            stored_revision(rid)
                .is_some_and(|stored| stored > own_sequence(component))
                .then_some(rid)
        })
        .collect();
    for rid in &older {
        remove_overrides(&mut series, *rid);
    }
    let overridden: Vec<i64> = overrides(&series)
        .filter_map(|component| recurrence_instant(&series, component))
        .collect();
    // The organizer's own exclusions count first: an override is shown
    // whatever the EXDATEs say, so carrying one over would undo them.
    let excluded = exdates(&series);
    for component in overrides(&old) {
        if recurrence_instant(&old, component)
            .is_some_and(|rid| !overridden.contains(&rid) && !excluded.contains(&rid))
        {
            put_override(&mut series, component.clone(), None);
        }
    }
    if let Some(master) = master_index(&old) {
        for entry in old.components[master].properties(&ICalendarProperty::Exdate) {
            for value in &entry.values {
                let Some(at) = value_instant(&old, entry, value) else {
                    continue;
                };
                if !excluded.contains(&at) && !overridden.contains(&at) {
                    add_exdate(
                        &mut series,
                        calcard::icalendar::ICalendarEntry {
                            values: vec![value.clone()],
                            ..entry.clone()
                        },
                    )?;
                }
            }
        }
    }
    Ok(series.to_string())
}

/// Keep a stored invitation's `start_at` at or before its earliest occurrence.
/// A range query takes a repeating row only once it has started, and an
/// organizer can move one occurrence before the series' first.
pub(super) fn settle_start(event: &mut CalendarEvent) {
    if let Some(earliest) = early_occurrence_start(event) {
        event.start_at = earliest;
    }
}

/// A recurring object may add or move an occurrence before its master
/// DTSTART. Keep the range-search bound separate from the editor's start.
pub(crate) fn early_occurrence_start(event: &CalendarEvent) -> Option<i64> {
    if !event.recurring || event.ics.is_empty() {
        return None;
    }
    let ical = calendar::parse_ical(&event.ics)?;
    let resolver = ical.build_tz_resolver().with_default(calendar::local_tz());
    ical.components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VEvent)
        .flat_map(|component| &component.entries)
        .filter(|entry| {
            matches!(
                entry.name,
                ICalendarProperty::Dtstart | ICalendarProperty::Rdate
            )
        })
        .flat_map(|entry| {
            let zone = resolver.resolve_or_default(entry.tz_id());
            entry.values.iter().filter_map(move |value| {
                let date = match value {
                    ICalendarValue::Period(
                        ICalendarPeriod::Range { start, .. }
                        | ICalendarPeriod::Duration { start, .. },
                    ) => start,
                    value => value.as_partial_date_time()?,
                };
                let zone = if date.has_time() { zone } else { Tz::UTC };
                Some(date.to_date_time_with_tz(zone)?.timestamp())
            })
        })
        .filter(|start| *start < event.start_at)
        .min()
}

/// Copy into `series` the VTIMEZONEs `other` defines and it does not, so an
/// override or EXDATE taken from `other` keeps the zone its TZID names.
fn merge_timezones(
    series: &mut calcard::icalendar::ICalendar,
    other: &calcard::icalendar::ICalendar,
) {
    let tzid = |component: &calcard::icalendar::ICalendarComponent| {
        calendar::text_of(component, ICalendarProperty::Tzid)
    };
    let known: Vec<String> = series
        .components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VTimezone)
        .map(tzid)
        .collect();
    let Some(calendar) = series
        .components
        .iter()
        .position(|component| component.component_type == ICalendarComponentType::VCalendar)
    else {
        return;
    };
    for zone in other.components.iter().filter(|component| {
        component.component_type == ICalendarComponentType::VTimezone
            && !known.contains(&tzid(component))
    }) {
        let id = series.components.len() as u32;
        series
            .components
            .push(calcard::icalendar::ICalendarComponent {
                component_ids: vec![],
                ..zone.clone()
            });
        series.components[calendar].component_ids.push(id);
        // Its STANDARD and DAYLIGHT rules.
        for child in &zone.component_ids {
            if let Some(rule) = other.components.get(*child as usize) {
                let child_id = series.components.len() as u32;
                series.components.push(rule.clone());
                series.components[id as usize].component_ids.push(child_id);
            }
        }
    }
}

/// The series' own VEVENT: the one that is not an occurrence override.
pub(super) fn master_index(ical: &calcard::icalendar::ICalendar) -> Option<usize> {
    ical.components.iter().position(|component| {
        component.component_type == ICalendarComponentType::VEvent
            && !component.is_recurrence_override()
    })
}

pub(super) fn overrides(
    ical: &calcard::icalendar::ICalendar,
) -> impl Iterator<Item = &calcard::icalendar::ICalendarComponent> {
    ical.components.iter().filter(|component| {
        component.component_type == ICalendarComponentType::VEvent
            && component.is_recurrence_override()
    })
}

pub(super) fn remove_overrides(series: &mut calcard::icalendar::ICalendar, recurrence_id: i64) {
    let stale: Vec<u32> = series
        .components
        .iter()
        .enumerate()
        .filter(|(_, component)| {
            component.component_type == ICalendarComponentType::VEvent
                && component.is_recurrence_override()
                && recurrence_instant(series, component) == Some(recurrence_id)
        })
        .map(|(index, _)| index as u32)
        .collect();
    series.remove_component_ids(&stale);
}

/// Add an occurrence's own VEVENT to `series`. calcard pairs an override with
/// its series by RECURRENCE-ID and SEQUENCE together, and an organizer who
/// moves one occurrence bumps the SEQUENCE of that occurrence alone, so it
/// takes the series' own — or the moved meeting would show beside the one it
/// replaces. `response`, when given, is the user's answer about it.
fn put_override(
    series: &mut calcard::icalendar::ICalendar,
    mut component: calcard::icalendar::ICalendarComponent,
    response: Option<Response>,
) {
    // Alarms and the like hang off the update's own numbering.
    component.component_ids.clear();
    let master_sequence = master_index(series)
        .and_then(|master| series.components[master].property(&ICalendarProperty::Sequence))
        .cloned();
    keep_own_sequence(&mut component);
    let private = ICalendarProperty::Other(OCCURRENCE_RESPONSE.to_string());
    component.entries.retain(|entry| {
        entry.name != ICalendarProperty::Sequence && (response.is_none() || entry.name != private)
    });
    component.entries.extend(master_sequence);
    if let Some(response) = response {
        component.add_property(private, ICalendarValue::Text(response.as_str().to_string()));
    }
    let id = series.components.len() as u32;
    series.components.push(component);
    if let Some(calendar) = series
        .components
        .iter_mut()
        .find(|component| component.component_type == ICalendarComponentType::VCalendar)
    {
        calendar.component_ids.push(id);
    }
}

/// Exclude a date from the series: `entry` names it (a RECURRENCE-ID or an
/// EXDATE, whichever it came as), and is filed as an EXDATE.
fn add_exdate(
    series: &mut calcard::icalendar::ICalendar,
    entry: calcard::icalendar::ICalendarEntry,
) -> Result<()> {
    let master = master_index(series).ok_or_else(|| anyhow!("stored event has no series"))?;
    series.components[master]
        .entries
        .push(calcard::icalendar::ICalendarEntry {
            name: ICalendarProperty::Exdate,
            ..entry
        });
    Ok(())
}

/// Where a series' end is kept once it has been cut short: declined or
/// cancelled with RANGE=THISANDFUTURE, every occurrence from that time on goes.
/// A private property on the series' VEVENT, holding Unix seconds.
const ENDS_BEFORE: &str = "X-MERON-ENDS-BEFORE";

/// Cut `series` short before `at`: nothing from then on is shown, its
/// overrides included.
fn end_before(series: &mut calcard::icalendar::ICalendar, at: i64) -> Result<()> {
    let cut = ends_before(series).map_or(at, |earlier| earlier.min(at));
    drop_overrides_from(series, cut);
    set_end(series, cut)
}

/// Record that `series` ends before `at`, leaving its overrides: any past the
/// cut were accepted on their own afterwards, and stay (see `occurrences`).
fn set_end(series: &mut calcard::icalendar::ICalendar, at: i64) -> Result<()> {
    let cut = ends_before(series).map_or(at, |earlier| earlier.min(at));
    let master = master_index(series).ok_or_else(|| anyhow!("stored event has no series"))?;
    let name = ICalendarProperty::Other(ENDS_BEFORE.to_string());
    let component = &mut series.components[master];
    component.entries.retain(|entry| entry.name != name);
    component.add_property(name, ICalendarValue::Text(cut.to_string()));
    Ok(())
}

fn drop_overrides_from(series: &mut calcard::icalendar::ICalendar, cut: i64) {
    let later: Vec<u32> = series
        .components
        .iter()
        .enumerate()
        .filter(|(_, component)| {
            component.component_type == ICalendarComponentType::VEvent
                && component.is_recurrence_override()
                && recurrence_instant(series, component).is_some_and(|rid| rid >= cut)
        })
        .map(|(index, _)| index as u32)
        .collect();
    series.remove_component_ids(&later);
}

/// [`set_end`] on stored calendar text, for a decline replayed when its series
/// is filed: overrides there by then were accepted on their own afterwards.
pub(super) fn cut_from(series_ics: &str, at: i64) -> Result<String> {
    let mut series =
        calendar::parse_ical(series_ics).ok_or_else(|| anyhow!("stored event is not iCalendar"))?;
    set_end(&mut series, at)?;
    Ok(series.to_string())
}

/// Undo [`end_before`].
fn lift_end(series: &mut calcard::icalendar::ICalendar) {
    if let Some(master) = master_index(series) {
        let name = ICalendarProperty::Other(ENDS_BEFORE.to_string());
        series.components[master]
            .entries
            .retain(|entry| entry.name != name);
    }
}

/// The time a series was cut short before, if it was.
fn ends_before(ical: &calcard::icalendar::ICalendar) -> Option<i64> {
    let master = master_index(ical)?;
    calendar::text_of(
        &ical.components[master],
        ICalendarProperty::Other(ENDS_BEFORE.to_string()),
    )
    .parse()
    .ok()
}

/// Bring a date back into the series. An EXDATE may list several dates;
/// only the one is dropped, and the property only when nothing is left of it.
fn remove_exdate(series: &mut calcard::icalendar::ICalendar, recurrence_id: i64) {
    let Some(master) = master_index(series) else {
        return;
    };
    let kept: Vec<Option<Vec<ICalendarValue>>> = series.components[master]
        .entries
        .iter()
        .map(|entry| {
            (entry.name == ICalendarProperty::Exdate).then(|| {
                entry
                    .values
                    .iter()
                    .filter(|value| value_instant(series, entry, value) != Some(recurrence_id))
                    .cloned()
                    .collect()
            })
        })
        .collect();
    let mut kept = kept.into_iter();
    series.components[master]
        .entries
        .retain_mut(|entry| match kept.next().flatten() {
            Some(values) => {
                entry.values = values;
                !entry.values.is_empty()
            }
            None => true,
        });
}

/// Every date the series' own VEVENT excludes.
fn exdates(ical: &calcard::icalendar::ICalendar) -> Vec<i64> {
    master_index(ical)
        .map(|master| {
            ical.components[master]
                .properties(&ICalendarProperty::Exdate)
                .flat_map(|entry| {
                    entry
                        .values
                        .iter()
                        .filter_map(|value| value_instant(ical, entry, value))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The instant a component's RECURRENCE-ID names, read the way
/// [`calendar::parse_invite`] reads it.
pub(super) fn recurrence_instant(
    ical: &calcard::icalendar::ICalendar,
    component: &calcard::icalendar::ICalendarComponent,
) -> Option<i64> {
    let entry = component.property(&ICalendarProperty::RecurrenceId)?;
    value_instant(ical, entry, entry.values.first()?)
}

/// The instant one value of a date or date-time property names: in its
/// TZID's zone (or the device's, when floating), a bare date at its UTC
/// midnight.
fn value_instant(
    ical: &calcard::icalendar::ICalendar,
    entry: &calcard::icalendar::ICalendarEntry,
    value: &ICalendarValue,
) -> Option<i64> {
    let value = value.as_partial_date_time()?;
    let tz = if value.has_time() {
        ical.build_tz_resolver()
            .with_default(calendar::local_tz())
            .resolve_or_default(entry.tz_id())
    } else {
        Tz::UTC
    };
    Some(value.to_date_time_with_tz(tz)?.timestamp())
}

/// Whether `event` (an invitation's series) already leaves out the occurrence
/// starting at `recurrence_id`.
pub(crate) fn excludes_occurrence(event: &CalendarEvent, recurrence_id: i64) -> bool {
    calendar::parse_ical(&event.ics).is_some_and(|ical| {
        let accepted_since = overrides(&ical)
            .any(|component| recurrence_instant(&ical, component) == Some(recurrence_id));
        exdates(&ical).contains(&recurrence_id)
            || (!accepted_since && ends_before(&ical).is_some_and(|cut| recurrence_id >= cut))
    })
}
