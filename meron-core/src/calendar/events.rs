//! The local calendar: the shape both frontends render for a range of days,
//! and the writes behind its editor.
//!
//! Like Tasks, everything is on this device: events the user typed in, and
//! the invitations they accepted (filed by [`super::respond`]). A month view
//! asks for a range and gets back occurrences — a repeating event expanded,
//! by calcard, from its own rule — with the open tasks due in it alongside.

#[cfg(test)]
mod tests;

use anyhow::{Result, anyhow};
use calcard::common::PartialDateTime;
use calcard::common::timezone::Tz;
use calcard::icalendar::{
    ICalendarComponentType, ICalendarPeriod, ICalendarProperty, ICalendarValue,
};
use chrono::{Datelike, NaiveDate, TimeZone};
use rusqlite::Connection;
use serde_json::{Value, json};

use super::{Invite, Response};
use crate::caldav;
use crate::store::{self, CalendarEvent, Task};

const DAY: i64 = 86_400;

/// How many occurrences of one repeating event are walked from its start.
/// Expansion always begins at DTSTART, so this bounds how far a range can lie
/// past it: a daily event reaches more than fifty years.
const MAX_OCCURRENCES: usize = 20_000;

/// What the editor sends to keep an event's own repeat rule, one the
/// editor cannot say (see [`super::rrule`]).
pub const CUSTOM_REPEAT: &str = "custom";

/// What the editor sends to create or change an event made here.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EventInput {
    pub title: String,
    pub notes: String,
    pub location: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    /// Empty for once, a plain choice (`weekly`), or an RRULE the custom
    /// repeat editor made (see [`super::rrule`]).
    pub repeat: String,
    /// When editing a repeating event, the start of the occurrence the editor
    /// was opened on: `start`/`end` are then that occurrence's new times, and
    /// the series moves by the same amount. 0 for the series itself.
    pub occurrence_start: i64,
    /// `None` keeps the event's alerts; `Some(None)` follows the app-wide
    /// default; otherwise see [`CalendarEvent::alerts`].
    pub alerts: Option<Option<Vec<i64>>>,
    /// The synced calendar (`dav_calendars.id`) the event goes in; empty for
    /// this device. `None` leaves an edited event where it is (and creates on
    /// this device); a different calendar on update moves it there.
    pub calendar: Option<String>,
    /// On update, the fields as the editor showed them when it opened. A
    /// field still equal to them was not edited, so a synced event keeps
    /// whatever its server changed it to meanwhile instead of the stale value.
    pub original: Option<Box<EventInput>>,
    /// Whether `repeat` is the rule as the editor showed it for the start
    /// being saved, the editor having moved it with that start itself. Left
    /// unsaid, an editor that sends `original` is taken to; one that cannot
    /// place a series' days in its own zone (mobile's) says it does not, and
    /// a rule left as it was is then moved here, in the series' zone.
    pub repeat_as_shown: Option<bool>,
    /// The series' first day (`YYYY-MM-DD`) the editor placed a rule it
    /// chose for. The core moves that rule to the first day as saved, in the
    /// series' own zone, where a time change can cross midnight that the
    /// editor's own calendar does not show.
    pub repeat_from: Option<String>,
}

/// Alerts an event can carry on its own, like Apple Calendar's "Alert" and
/// "Second Alert".
pub const MAX_ALERTS: usize = 2;

/// The longest timed alert lead the core accepts, in minutes: one week.
pub const MAX_ALERT_MINUTES: i64 = 7 * 24 * 60;

/// The longest all-day alert lead the core accepts, in days.
pub const MAX_ALL_DAY_ALERT_DAYS: i64 = 7;

/// Keep the alerts a client sent that the core can schedule: in range for the
/// kind of event, no repeats, at most [`MAX_ALERTS`], in the order given.
fn sanitize_alerts(alerts: &[i64], all_day: bool) -> Vec<i64> {
    let max = if all_day {
        MAX_ALL_DAY_ALERT_DAYS
    } else {
        MAX_ALERT_MINUTES
    };
    let mut out = Vec::new();
    for &lead in alerts {
        if (0..=max).contains(&lead) && !out.contains(&lead) {
            out.push(lead);
        }
    }
    out.truncate(MAX_ALERTS);
    out
}

impl EventInput {
    pub fn from_params(params: &Value) -> Self {
        let text = |key: &str| {
            params
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Self {
            title: text("title"),
            notes: text("notes"),
            location: text("location"),
            start: params.get("start").and_then(Value::as_i64).unwrap_or(0),
            end: params.get("end").and_then(Value::as_i64).unwrap_or(0),
            all_day: params
                .get("all_day")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            repeat: text("repeat"),
            occurrence_start: params
                .get("occurrence_start")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            alerts: params.get("alerts").map(|alerts| {
                alerts
                    .as_array()
                    .map(|alerts| alerts.iter().filter_map(Value::as_i64).collect())
            }),
            calendar: params
                .get("calendar")
                .and_then(Value::as_str)
                .map(|calendar| calendar.trim().to_string()),
            original: params
                .get("original")
                .filter(|original| original.is_object())
                .map(|original| Box::new(Self::from_params(original))),
            repeat_as_shown: params.get("repeat_as_shown").and_then(Value::as_bool),
            repeat_from: params
                .get("repeat_from")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
}

/// Apply `input` to `event`: an all-day event covers whole UTC days, an end
/// never precedes its start, and a repeating timed event remembers the zone
/// its wall-clock time belongs to.
fn apply_input(event: &mut CalendarEvent, input: &EventInput) -> Result<()> {
    if input.start <= 0 {
        return Err(anyhow!("an event needs a start"));
    }
    let repeat = input.repeat.trim();
    let (start, end) = if input.all_day {
        let start = input.start.div_euclid(DAY) * DAY;
        let end = (input.end.max(input.start) + DAY - 1).div_euclid(DAY) * DAY;
        (start, end.max(start + DAY))
    } else {
        (input.start, input.end.max(input.start))
    };
    event.title = input.title.trim().to_string();
    event.notes = input.notes.trim().to_string();
    event.location = input.location.trim().to_string();
    event.start_at = start;
    event.end_at = end;
    set_all_day(event, input.all_day);
    event.recurring = !repeat.is_empty();
    // A repeat keeps the zone it was made in: edited after travelling, it must
    // not take on the new zone's DST rules.
    event.tzid = if !event.recurring || input.all_day {
        String::new()
    } else if event.tzid.is_empty() {
        super::local_tzid().unwrap_or_default()
    } else {
        event.tzid.clone()
    };
    event.rrule = super::rrule::from_editor(repeat)?;
    apply_alerts(event, input);
    Ok(())
}

/// Alerts count minutes for a timed event and days for an all-day one, so a
/// switch between the two follows the default until alerts are set again.
fn set_all_day(event: &mut CalendarEvent, all_day: bool) {
    if event.all_day != all_day {
        event.alerts = None;
    }
    event.all_day = all_day;
}

fn apply_alerts(event: &mut CalendarEvent, input: &EventInput) {
    if let Some(alerts) = &input.alerts {
        event.alerts = alerts
            .as_deref()
            .map(|alerts| sanitize_alerts(alerts, event.all_day));
    }
}

pub fn create_event(conn: &Connection, input: &EventInput) -> Result<Value> {
    let mut event = CalendarEvent {
        id: crate::tasks::new_id("event"),
        ..Default::default()
    };
    apply_input(&mut event, input)?;
    settle_rule(&CalendarEvent::default(), &mut event, true)?;
    // The event and its upload together: a sync on another connection (on
    // mobile, one an earlier edit started) must not find one without the other.
    let tx = conn.unchecked_transaction()?;
    if let Some(calendar) = input.calendar.as_deref().filter(|c| !c.is_empty()) {
        let calendar = writable_calendar(&tx, calendar)?;
        caldav::queue_new_event(&tx, &calendar, &mut event)?;
    }
    let saved = store::save_event(&tx, &event)?;
    tx.commit()?;
    Ok(json!({ "event": event_json(&saved) }))
}

/// A synced calendar events can be written to, or why not.
fn writable_calendar(conn: &Connection, id: &str) -> Result<caldav::store::DavCalendar> {
    let calendar = caldav::store::calendar(conn, id)?
        .filter(|c| c.enabled)
        .ok_or_else(|| anyhow!("no such calendar: {id}"))?;
    if calendar.read_only || !calendar.supports("VEVENT") {
        return Err(anyhow!("calendar {} is read-only", calendar.name));
    }
    Ok(calendar)
}

/// Change an event made here. An accepted invitation's time and title are the
/// organizer's, so only its notes and alerts are the user's to edit. Read and
/// written in one transaction: a sync on another connection (mobile's) must
/// not change or remove the event between the two.
pub fn update_event(conn: &Connection, id: &str, input: &EventInput) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    let result = update_event_in(&tx, id, input)?;
    tx.commit()?;
    Ok(result)
}

fn update_event_in(conn: &Connection, id: &str, input: &EventInput) -> Result<Value> {
    let repeat_from = input
        .repeat_from
        .as_deref()
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok());
    let mut event = store::event(conn, id)?.ok_or_else(|| anyhow!("no such event: {id}"))?;
    // An editor that says what it opened with (desktop's) shows a repeat as
    // of the start being saved, and moves it with that start itself; so is
    // one chosen in this edit. Otherwise a series' own rule is moved here.
    // An editor that says outright is taken at its word: one that left the
    // rule as it was wants the latest stored rule moved here, even when a
    // sync changed it while the editor was open.
    let rule_as_shown = input.repeat_as_shown.unwrap_or_else(|| {
        input.original.is_some()
            || super::rrule::from_editor(&input.repeat).is_ok_and(|rule| rule != event.rrule)
    });
    // A picker left as it opened is not a move, even if the event has since
    // moved elsewhere (another window, a sync).
    let opened_in = input
        .original
        .as_deref()
        .and_then(|o| o.calendar.as_deref());
    let target = match input.calendar.as_deref() {
        Some(calendar) if Some(calendar) != opened_in => calendar.to_string(),
        _ => event.calendar.clone(),
    };
    let moving = target != event.calendar;
    if moving && !event.invite_uid.is_empty() {
        return Err(anyhow!(
            "an accepted invitation stays in this device's calendar"
        ));
    }
    let destination = match target.as_str() {
        "" => None,
        id => Some(writable_calendar(conn, id)?),
    };
    let synced = !event.calendar.is_empty();
    if synced || (event.invite_uid.is_empty() && !event.ics.is_empty()) {
        if synced {
            writable_calendar(conn, &event.calendar)?;
        }
        let before = event.clone();
        let mut input = series_input(&event, &unedited_from_store(input, &before));
        // A repeat the editor cannot represent, such as a series with
        // occurrence exceptions, comes back as `custom` to keep it.
        let keep_custom = before.recurring
            && before.rrule.is_empty()
            && input.repeat.trim().eq_ignore_ascii_case(CUSTOM_REPEAT);
        if keep_custom {
            input.repeat.clear();
        }
        apply_input(&mut event, &input)?;
        if keep_custom {
            event.recurring = true;
        }
        // A save that moved nothing keeps how the server times the event: a
        // UTC series must not become one in this device's zone, which DST
        // would then shift.
        let same_timing = event.start_at == before.start_at
            && event.end_at == before.end_at
            && event.all_day == before.all_day
            && event.recurring == before.recurring
            && event.rrule == before.rrule;
        // A series moved to another time keeps its zone too, UTC included;
        // only one that starts repeating here takes this device's.
        let still_a_timed_series =
            before.recurring && !before.all_day && event.recurring && !event.all_day;
        if keep_custom || same_timing || still_a_timed_series {
            event.tzid = before.tzid.clone();
        }
        place_rule(&mut event, repeat_from);
        settle_rule(&before, &mut event, rule_as_shown)?;
        event.ics = caldav::ics::patch(&before.ics, &before, &event);
        if !moving {
            let saved = store::save_event(conn, &event)?;
            if synced {
                caldav::store::mark_changed(conn, &saved.id)?;
            }
            return Ok(json!({ "event": event_json(&saved) }));
        }
        if !synced {
            let calendar = destination
                .as_ref()
                .expect("moving from the local calendar");
            event.calendar = calendar.id.clone();
            caldav::requeue_restored_event(conn, calendar, &mut event)?;
            let saved = store::save_event(conn, &event)?;
            return Ok(json!({ "event": event_json(&saved) }));
        }
        // Moving: deleted from the calendar it leaves, created (with the same
        // UID) in the one it goes to. Moved on again before that copy was
        // sent, deletions waiting for it wait for the newest copy instead.
        let unsent_copy = caldav::store::object_for_event(conn, id)?
            .filter(|object| object.state == caldav::store::CREATED);
        match &destination {
            Some(calendar) => {
                let source = caldav::store::detach_for_move(conn, id)?;
                event.calendar = calendar.id.clone();
                caldav::requeue_restored_event(conn, calendar, &mut event)?;
                let arrival = caldav::store::object_for_event(conn, id)?;
                // The old copy goes only once the new one is on its server.
                if let Some(arrival) = &arrival {
                    if let Some(source) = &source {
                        caldav::store::link_move(conn, source, &arrival.url)?;
                    }
                    if let Some(copy) = &unsent_copy {
                        caldav::store::retarget_moves(conn, &copy.url, &arrival.url)?;
                    }
                }
            }
            // This device repeats only by the rules the editor names.
            None if event.recurring && event.rrule.is_empty() => {
                return Err(anyhow!(
                    "this event repeats by a rule only its calendar server can keep; \
                     choose a repeat before moving it to this device"
                ));
            }
            None => {
                caldav::store::detach_for_move(conn, id)?;
                // Here for good: whatever waited for a copy elsewhere goes.
                if let Some(copy) = &unsent_copy {
                    caldav::store::retarget_moves(conn, &copy.url, "")?;
                }
                keep_floating_times(&mut event);
                event.calendar.clear();
                event.ics.clear();
            }
        }
        let saved = store::save_event(conn, &event)?;
        // The calendar it left has a deletion to send.
        return Ok(json!({ "event": event_json(&saved), "moved_from": before.calendar }));
    }
    if event.invite_uid.is_empty() {
        let before = event.clone();
        let input = series_input(&event, input);
        apply_input(&mut event, &input)?;
        // A timed series keeps the zone it repeats in, UTC (no zone) for one
        // brought here from a calendar that kept it so.
        if before.recurring && !before.all_day && event.recurring && !event.all_day {
            event.tzid = before.tzid.clone();
        }
        place_rule(&mut event, repeat_from);
        settle_rule(&before, &mut event, rule_as_shown)?;
    } else {
        event.notes = input.notes.trim().to_string();
        apply_alerts(&mut event, input);
    }
    if let Some(calendar) = &destination {
        caldav::queue_new_event(conn, calendar, &mut event)?;
    }
    let saved = store::save_event(conn, &event)?;
    Ok(json!({ "event": event_json(&saved) }))
}

/// `input` with every field the editor did not change (it still equals
/// `input.original`) taken from `stored`, the event as it is now: a sync may
/// have brought a newer version since the editor opened.
fn unedited_from_store(input: &EventInput, stored: &CalendarEvent) -> EventInput {
    let Some(original) = input.original.as_deref() else {
        return input.clone();
    };
    let mut out = input.clone();
    if input.title == original.title {
        out.title = stored.title.clone();
    }
    if input.notes == original.notes {
        out.notes = stored.notes.clone();
    }
    if input.location == original.location {
        out.location = stored.location.clone();
    }
    if (input.start, input.end, input.all_day) == (original.start, original.end, original.all_day) {
        out.start = stored.start_at;
        out.end = stored.end_at;
        out.all_day = stored.all_day;
        // The series' own times, not an occurrence's.
        out.occurrence_start = 0;
    }
    // Alerts left as they opened (for the same kind of event) keep what a
    // sync brought meanwhile.
    if input.alerts == original.alerts && input.all_day == original.all_day {
        out.alerts = None;
    }
    // A rule the editor says it chose stays, even one that moved back to
    // the text it opened with.
    if input.repeat == original.repeat && input.repeat_as_shown != Some(true) {
        out.repeat = if stored.recurring && stored.rrule.is_empty() {
            CUSTOM_REPEAT.to_string()
        } else {
            stored.rrule.clone()
        };
    }
    out
}

/// An edit made on one occurrence of a repeating event, restated for the
/// series: the editor shows the occurrence it was opened on, and moving that
/// occurrence moves every one by the same amount, from the series' own start.
/// Without this, saving a later occurrence would make it the first and drop
/// the ones before it.
fn series_input(event: &CalendarEvent, input: &EventInput) -> EventInput {
    if !event.recurring || input.occurrence_start <= 0 || input.start <= 0 {
        return input.clone();
    }
    let duration = (input.end - input.start).max(0);
    let start = shift_series_start(event, input)
        .unwrap_or(event.start_at + (input.start - input.occurrence_start));
    EventInput {
        start,
        end: start + duration,
        occurrence_start: 0,
        ..input.clone()
    }
}

/// The series' new start, moved as far on the calendar and the clock as the
/// edited occurrence moved, all read in the zone the series repeats in.
/// Counting seconds instead would carry a DST change between the two into
/// every occurrence: moving a 09:00 across the change by a day would make the
/// whole series 10:00. An all-day bound is a UTC midnight, not a local time,
/// so when either side is all-day — including a switch between the two —
/// the move is counted in whole dates and the time of day comes from the edit.
fn shift_series_start(event: &CalendarEvent, input: &EventInput) -> Option<i64> {
    // A timed series' own zone, UTC included; an all-day one becoming timed
    // the zone it names, or this device's.
    let tz = match event.tzid.parse::<Tz>() {
        Ok(tz) if event.all_day && !event.tzid.is_empty() => tz,
        _ if event.all_day => super::local_tz(),
        _ => rule_tz(event),
    };
    let local = |at: i64| Some(tz.timestamp_opt(at, 0).single()?.naive_local());
    let utc = |at: i64| Some(chrono::DateTime::from_timestamp(at, 0)?.naive_utc());
    let at = |at: i64, all_day: bool| if all_day { utc(at) } else { local(at) };

    if !event.all_day && !input.all_day {
        let shifted =
            local(event.start_at)? + (local(input.start)? - local(input.occurrence_start)?);
        return Some(tz.from_local_datetime(&shifted).earliest()?.timestamp());
    }
    let days =
        at(input.start, input.all_day)?.date() - at(input.occurrence_start, event.all_day)?.date();
    let date = at(event.start_at, event.all_day)?.date() + days;
    if input.all_day {
        Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp())
    } else {
        let time = local(input.start)?.time();
        Some(
            tz.from_local_datetime(&date.and_time(time))
                .earliest()?
                .timestamp(),
        )
    }
}

/// Remove an event. Returns it whole, so the caller can offer an undo that
/// hands it straight back to [`restore_event`].
pub fn delete_event(conn: &Connection, id: &str) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    let event = store::event(&tx, id)?;
    if let Some(synced) = event.as_ref().filter(|e| !e.calendar.is_empty()) {
        writable_calendar(&tx, &synced.calendar)?;
        caldav::store::mark_deleted(&tx, id)?;
    }
    store::delete_event(&tx, id)?;
    tx.commit()?;
    Ok(json!({
        "ok": true,
        "restore": event.map(|event| serde_json::to_value(event).unwrap_or(Value::Null)),
    }))
}

pub fn restore_event(conn: &Connection, restore: &Value) -> Result<Value> {
    let event: CalendarEvent = serde_json::from_value(restore.clone())
        .map_err(|err| anyhow!("not an event to restore: {err}"))?;
    if event.id.is_empty() {
        return Err(anyhow!("not an event to restore"));
    }
    let mut event = event;
    let tx = conn.unchecked_transaction()?;
    if !event.calendar.is_empty() {
        match caldav::store::calendar(&tx, &event.calendar)?.filter(|c| c.enabled) {
            Some(calendar) => caldav::requeue_restored_event(&tx, &calendar, &mut event)?,
            // Its calendar went away meanwhile: keep the event on this device.
            // Its `ics` still describes it, repeats and all.
            None => event.calendar.clear(),
        }
    }
    let saved = store::save_event(&tx, &event)?;
    tx.commit()?;
    Ok(json!({ "event": event_json(&saved) }))
}

pub fn event_payload(conn: &Connection, id: &str) -> Result<Value> {
    Ok(json!({ "event": store::event(conn, id)?.as_ref().map(event_json) }))
}

/// File the answer to an invitation in the calendar: an accepted or tentative
/// event is put there (or brought up to date), a declined one taken out.
pub fn record_invite_event(
    conn: &Connection,
    account: &str,
    thread_id: &str,
    message_id: &str,
    invite: &Invite,
    response: Response,
) -> Result<()> {
    let series = store::invite_event(conn, account, &invite.uid)?;
    if invite.recurrence_id != 0 {
        let alone =
            store::invite_occurrence_event(conn, account, &invite.uid, invite.recurrence_id)?;
        // An answer about one occurrence of a series already in the calendar
        // changes that occurrence, not the series.
        if let Some(mut series) = series.filter(|event| event.recurring && !event.ics.is_empty()) {
            series.ics = with_occurrence(&series.ics, invite, Some(response))?;
            settle_start(&mut series);
            store::save_event(conn, &series)?;
            if let Some(alone) = alone {
                store::delete_event(conn, &alone.id)?;
            }
            return Ok(());
        }
        // Otherwise the occurrence stands alone, beside any other occurrence
        // of the same event answered the same way.
        if response == Response::Declined {
            if let Some(alone) = alone {
                store::delete_event(conn, &alone.id)?;
            }
            // Declined or cancelled from that date on: the later ones too.
            if invite.this_and_future {
                for later in store::invite_occurrence_events(conn, account, &invite.uid)? {
                    if later.recurrence_id >= invite.recurrence_id {
                        store::delete_event(conn, &later.id)?;
                    }
                }
            }
            return Ok(());
        }
        file_invite(
            conn, alone, account, thread_id, message_id, invite, response,
        )?;
        return Ok(());
    }

    let alone = store::invite_occurrence_events(conn, account, &invite.uid)?;
    if response == Response::Declined {
        for event in series.into_iter().chain(alone) {
            store::delete_event(conn, &event.id)?;
        }
        return Ok(());
    }
    let mut series = file_invite(
        conn, series, account, thread_id, message_id, invite, response,
    )?;
    // Occurrences answered before the series fold into it: accepted ones as
    // its overrides, declined ones (which left nothing filed) as exclusions.
    if series.recurring && !series.ics.is_empty() {
        for (recurrence_id, this_and_future) in
            store::declined_occurrences(conn, account, &invite.uid)?
        {
            series.ics = if this_and_future {
                cut_from(&series.ics, recurrence_id)?
            } else {
                without_occurrence(&series.ics, recurrence_id, series.all_day)?
            };
        }
        for occurrence in alone {
            let answer = Response::parse(&occurrence.response).unwrap_or(Response::Accepted);
            let update = Invite {
                ics: occurrence.ics.clone(),
                recurrence_id: occurrence.recurrence_id,
                ..Default::default()
            };
            series.ics = with_occurrence(&series.ics, &update, Some(answer))?;
            // The series has one set of notes; the user's on this occurrence
            // join them rather than go with its row.
            let notes = occurrence.notes.trim();
            if !notes.is_empty() && !series.notes.contains(notes) {
                series.notes = if series.notes.is_empty() {
                    notes.to_string()
                } else {
                    format!("{}\n\n{notes}", series.notes)
                };
            }
            store::delete_event(conn, &occurrence.id)?;
        }
        settle_start(&mut series);
        store::save_event(conn, &series)?;
    }
    Ok(())
}

/// Bring a filed invitation up to date with the organizer's newer revision of
/// it (a later SEQUENCE: moved, renamed, ...), as soon as that message is read.
/// The calendar would otherwise keep the old time until the user answered
/// again. The answer they gave carries over; the card asks for a new one.
/// `sender` is the message's From address.
pub fn apply_invite_update(
    conn: &Connection,
    account: &str,
    sender: &str,
    message_id: &str,
    invite: &Invite,
) -> Result<()> {
    if invite.method != "request" || invite.cancelled || invite.uid.is_empty() {
        return Ok(());
    }
    let alone = if invite.recurrence_id == 0 {
        None
    } else {
        store::invite_occurrence_event(conn, account, &invite.uid, invite.recurrence_id)?
    };
    let Some(filed) = alone.or(store::invite_event(conn, account, &invite.uid)?) else {
        return Ok(());
    };
    // An occurrence the calendar leaves out stays out.
    if invite.recurrence_id != 0
        && filed.recurrence_id == 0
        && filed.recurring
        && excludes_occurrence(&filed, invite.recurrence_id)
    {
        return Ok(());
    }
    // Only the organizer revises an event, and only in mail from them: anyone
    // else who knows its UID (another attendee, say) must not move it on the
    // user's calendar unasked. Answering their copy still can.
    let organizer = super::parse_invite(&filed.ics)
        .map(|filed| filed.organizer.addr.trim().to_ascii_lowercase())
        .unwrap_or_default();
    if organizer.is_empty()
        || organizer != invite.organizer.addr.trim().to_ascii_lowercase()
        || organizer != sender.trim().to_ascii_lowercase()
    {
        return Ok(());
    }
    // Something in it is newer than what is filed, and the series it brings
    // (if it brings one) is not older: that replaces the filed one whole.
    let revisions = revisions(invite);
    let newer = revisions
        .iter()
        .any(|&(recurrence_id, sequence)| sequence > filed_sequence(&filed, recurrence_id));
    let older_series = invite.recurrence_id == 0
        && revisions.iter().any(|&(recurrence_id, sequence)| {
            recurrence_id == 0 && sequence < filed_sequence(&filed, 0)
        });
    if !newer || older_series {
        return Ok(());
    }
    let recorded = store::invite_response(conn, account, &invite.uid, invite.recurrence_id)?;
    let Some(response) = recorded
        .and_then(|recorded| Response::parse(&recorded.response))
        .or_else(|| Response::parse(&filed.response))
        .filter(|response| *response != Response::Declined)
    else {
        return Ok(());
    };
    // An automatic update is no answer: the dates the user answered one by
    // one keep those answers, and the others stay with the series'.
    if invite.recurrence_id != 0
        && filed.recurrence_id == 0
        && filed.recurring
        && !filed.ics.is_empty()
    {
        let mut series = filed;
        let answer = occurrence_answer(&series.ics, invite.recurrence_id);
        series.ics = with_occurrence(&series.ics, invite, answer)?;
        settle_start(&mut series);
        store::save_event(conn, &series)?;
        return Ok(());
    }
    let invite = if invite.recurrence_id == 0 && filed.recurring {
        &with_occurrence_answers(conn, account, invite, &filed)?
    } else {
        invite
    };
    record_invite_event(
        conn,
        account,
        &filed.thread_id,
        message_id,
        invite,
        response,
    )
}

/// The answer the user gave the date at `recurrence_id` on its own, if any.
fn occurrence_answer(series_ics: &str, recurrence_id: i64) -> Option<Response> {
    let series = super::parse_ical(series_ics)?;
    overrides(&series)
        .filter(|component| recurrence_instant(&series, component) == Some(recurrence_id))
        .find_map(|component| {
            Response::parse(&super::text_of(
                component,
                ICalendarProperty::Other(OCCURRENCE_RESPONSE.to_string()),
            ))
        })
}

/// [`apply_invite_update`] for the revisions of `uid` already downloaded,
/// oldest first: an update can arrive before the user answers the invitation
/// it revises, and then had nothing filed to change.
pub fn apply_cached_updates(conn: &Connection, account: &str, uid: &str) -> Result<()> {
    let mut cached = store::cached_invites(conn, account, uid)?;
    cached.sort_by_key(|(_, _, invite)| invite.sequence);
    for (sender, message_id, invite) in cached {
        apply_invite_update(conn, account, &sender, &message_id, &invite)?;
    }
    Ok(())
}

/// The revised series `invite` with the user's own answers about its dates:
/// the organizer's override of a date they declined goes, so the decline (put
/// back by [`record_invite_event`]) still holds, and an override of a date
/// they answered otherwise than the series takes that answer.
fn with_occurrence_answers(
    conn: &Connection,
    account: &str,
    invite: &Invite,
    filed: &CalendarEvent,
) -> Result<Invite> {
    let (Some(mut incoming), Some(stored)) = (
        super::parse_ical(&invite.ics),
        super::parse_ical(&filed.ics),
    ) else {
        return Ok(invite.clone());
    };
    for (recurrence_id, _) in store::declined_occurrences(conn, account, &invite.uid)? {
        remove_overrides(&mut incoming, recurrence_id);
    }
    let private = ICalendarProperty::Other(OCCURRENCE_RESPONSE.to_string());
    let answers: Vec<(i64, String)> = overrides(&stored)
        .filter_map(|component| {
            let answer = super::text_of(component, private.clone());
            (!answer.is_empty())
                .then(|| Some((recurrence_instant(&stored, component)?, answer)))
                .flatten()
        })
        .collect();
    let at: Vec<(usize, i64)> = incoming
        .components
        .iter()
        .enumerate()
        .filter(|(_, component)| {
            component.component_type == ICalendarComponentType::VEvent
                && component.is_recurrence_override()
        })
        .filter_map(|(index, component)| Some((index, recurrence_instant(&incoming, component)?)))
        .collect();
    for (index, recurrence_id) in at {
        if let Some((_, answer)) = answers.iter().find(|(rid, _)| *rid == recurrence_id) {
            let component = &mut incoming.components[index];
            component.entries.retain(|entry| entry.name != private);
            component.add_property(private.clone(), ICalendarValue::Text(answer.clone()));
        }
    }
    Ok(Invite {
        ics: incoming.to_string(),
        ..invite.clone()
    })
}

/// Each event `invite` carries, as (RECURRENCE-ID, SEQUENCE): a series may
/// bring occurrences the organizer revised on their own, at their own count.
fn revisions(invite: &Invite) -> Vec<(i64, i64)> {
    if invite.recurrence_id != 0 {
        return vec![(invite.recurrence_id, invite.sequence)];
    }
    let Some(ical) = super::parse_ical(&invite.ics) else {
        return vec![(0, invite.sequence)];
    };
    ical.components
        .iter()
        .filter(|component| component.component_type == ICalendarComponentType::VEvent)
        .filter_map(|component| {
            let recurrence_id = if component.is_recurrence_override() {
                recurrence_instant(&ical, component)?
            } else {
                0
            };
            Some((recurrence_id, own_sequence(component)))
        })
        .collect()
}

/// The revision of `filed` an update about `recurrence_id` must be newer than.
/// An occurrence moved within a series keeps its own (see [`OWN_SEQUENCE`]);
/// the series' counts too, as an organizer's SEQUENCE only grows.
fn filed_sequence(filed: &CalendarEvent, recurrence_id: i64) -> i64 {
    if recurrence_id == 0 || filed.recurrence_id != 0 {
        return super::parse_invite(&filed.ics).map_or(0, |filed| filed.sequence);
    }
    let Some(ical) = super::parse_ical(&filed.ics) else {
        return 0;
    };
    ical.components
        .iter()
        .enumerate()
        .filter(|(index, component)| {
            Some(*index) == master_index(&ical)
                || (component.component_type == ICalendarComponentType::VEvent
                    && component.is_recurrence_override()
                    && recurrence_instant(&ical, component) == Some(recurrence_id))
        })
        .map(|(_, component)| own_sequence(component))
        .max()
        .unwrap_or(0)
}

/// Put the event `invite` describes in the calendar, over `existing` (the row
/// an earlier answer filed) when there is one.
fn file_invite(
    conn: &Connection,
    existing: Option<CalendarEvent>,
    account: &str,
    thread_id: &str,
    message_id: &str,
    invite: &Invite,
    response: Response,
) -> Result<CalendarEvent> {
    let mut event = existing.unwrap_or_else(|| CalendarEvent {
        id: crate::tasks::new_id("event"),
        ..Default::default()
    });
    event.title = invite.summary.clone();
    // The user's own notes on an earlier revision outlive the organizer's.
    if event.notes.is_empty() {
        event.notes = invite.description.clone();
    }
    event.location = invite.location.clone();
    event.start_at = invite.start;
    event.end_at = invite.end;
    set_all_day(&mut event, invite.all_day);
    event.recurring = invite.recurring;
    event.account = account.to_string();
    if !thread_id.is_empty() {
        event.thread_id = thread_id.to_string();
    }
    event.message_id = message_id.to_string();
    event.invite_uid = invite.uid.clone();
    event.recurrence_id = invite.recurrence_id;
    event.response = response.as_str().to_string();
    // A series answered again keeps the dates the user already dealt with.
    let incoming = if invite.recurrence_id == 0 {
        with_series_sequence(&invite.ics)?
    } else {
        invite.ics.clone()
    };
    event.ics = if invite.recurrence_id == 0 && event.recurring && !event.ics.is_empty() {
        carry_exceptions(&event.ics, &incoming)?
    } else {
        incoming
    };
    settle_start(&mut event);
    store::save_event(conn, &event)
}

pub fn event_json(event: &CalendarEvent) -> Value {
    json!({
        "id": event.id,
        "title": event.title,
        "notes": event.notes,
        "location": event.location,
        "start": event.start_at,
        "end": event.end_at,
        "all_day": event.all_day,
        "recurring": event.recurring,
        "repeat": event.rrule,
        "account": event.account,
        "thread_id": event.thread_id,
        "message_id": event.message_id,
        "response": event.response,
        "created_at": event.created_at,
        "reminder_after": event.reminder_after,
        "alerts": event.alerts,
        // The kind `alerts` counts in; an occurrence can differ (see
        // `events_payload`).
        "alerts_all_day": event.all_day,
        "source": if event.invite_uid.is_empty() { "event" } else { "invite" },
        // The synced calendar it belongs to; empty for one on this device.
        "calendar": event.calendar,
        // A series' first date, in the zone its repeat's dates are counted in:
        // what a custom repeat is made from, whichever occurrence is open.
        "series_date": if event.recurring {
            first_day(event).format("%Y-%m-%d").to_string()
        } else {
            String::new()
        },
        // The zone those dates are in, for an editor to place an edited start
        // in: an IANA name, `UTC`, or empty for this device's own (a floating
        // series) or for whole days.
        "series_zone": series_zone(event),
    })
}

/// Everything in `[from, to)` for a calendar view: each event's occurrences
/// there and, with `include_tasks`, the open tasks due in it, soonest first.
/// Each item has a `key` unique within the answer, for list identity.
pub fn events_payload(conn: &Connection, from: i64, to: i64, include_tasks: bool) -> Result<Value> {
    if to <= from {
        return Ok(json!({ "events": [] }));
    }
    let mut items: Vec<(i64, Value)> = Vec::new();
    let mut keys = std::collections::HashSet::new();
    // A synced calendar keeps events the user declined (Google shows them
    // only with "Show declined events" on); they are left out here, as a
    // declined invitation is taken out of the calendar.
    let own = caldav::store::own_addresses(conn)?;
    let nobody = std::collections::HashSet::new();
    for event in store::events_between(conn, from, to)? {
        let mine = own.get(&event.calendar).unwrap_or(&nobody);
        // The same for every occurrence: worked out once, the series' first
        // date included, which may mean reading its whole object.
        let series = event_json(&event);
        for occurrence in occurrences(&event, from, to, mine) {
            let start = occurrence.start;
            let mut item = series.clone();
            // Two occurrences can start together (one moved onto another), and
            // clients key their lists by this.
            let base = format!("{}@{start}", event.id);
            let key = (0..)
                .map(|n| {
                    if n == 0 {
                        base.clone()
                    } else {
                        format!("{base}#{n}")
                    }
                })
                .find(|key| !keys.contains(key))
                .unwrap_or(base);
            keys.insert(key.clone());
            item["key"] = json!(key);
            item["start"] = json!(start);
            item["end"] = json!(occurrence.end);
            if let Some(all_day) = occurrence.all_day {
                item["all_day"] = json!(all_day);
            }
            if let Some(title) = occurrence.title {
                item["title"] = json!(title);
            }
            if let Some(location) = occurrence.location {
                item["location"] = json!(location);
            }
            if let Some(response) = occurrence.response {
                item["response"] = json!(response);
            }
            items.push((start, item));
        }
    }
    if include_tasks {
        // A due date is a local day (see `fromDateInputValue`), so look a day
        // either side and keep the ones whose day falls in the range.
        for task in store::tasks_due_between(conn, from - DAY, to + DAY)? {
            let day = local_day_as_utc(task.due_at);
            if task.due_has_time {
                if task.due_at >= from && task.due_at < to {
                    items.push((task.due_at, task_json(&task, day)));
                }
            } else if day < to && day + DAY > from {
                items.push((day, task_json(&task, day)));
            }
        }
    }
    items.sort_by(|(a, left), (b, right)| {
        a.cmp(b)
            .then_with(|| right["all_day"].as_bool().cmp(&left["all_day"].as_bool()))
            .then_with(|| left["key"].as_str().cmp(&right["key"].as_str()))
    });
    Ok(json!({ "events": items.into_iter().map(|(_, item)| item).collect::<Vec<_>>() }))
}

fn task_json(task: &Task, day: i64) -> Value {
    json!({
        "id": task.id,
        "key": format!("task:{}", task.id),
        "source": "task",
        "title": task.title,
        "notes": task.notes,
        "location": "",
        "start": if task.due_has_time { task.due_at } else { day },
        "end": if task.due_has_time { task.due_at } else { day + DAY },
        "all_day": !task.due_has_time,
        "recurring": false,
        "repeat": "",
        "list_id": task.list_id,
        "account": task.account,
        "thread_id": task.thread_id,
        "message_id": task.message_id,
        "response": "",
    })
}

/// The local calendar day `at` falls on, as that date's UTC midnight — how
/// all-day items are placed.
fn local_day_as_utc(at: i64) -> i64 {
    chrono::Local
        .timestamp_opt(at, 0)
        .single()
        .map(|time| {
            let date = time.date_naive();
            NaiveDate::from_ymd_opt(date.year(), date.month(), date.day())
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|midnight| midnight.and_utc().timestamp())
                .unwrap_or(at)
        })
        .unwrap_or(at)
}

/// One time an event happens. An occurrence the organizer changed on its own
/// (an override VEVENT) can carry its own title and location.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Occurrence {
    start: i64,
    end: i64,
    /// The VEVENT it came from, so two occurrences that land on the same
    /// times — one moved onto another — stay two.
    component: u32,
    /// An override that is all-day while its series is timed, or the other
    /// way round.
    all_day: Option<bool>,
    title: Option<String>,
    location: Option<String>,
    /// The user's answer about this occurrence alone, when it differs from
    /// the event's (see [`OCCURRENCE_RESPONSE`]).
    response: Option<String>,
}

/// Where an occurrence's own answer is kept in a stored invitation: a private
/// property on its override VEVENT, which never leaves the device.
const OCCURRENCE_RESPONSE: &str = "X-MERON-RESPONSE";

/// An override's SEQUENCE as the organizer sent it, kept beside the series'
/// one it is given (see [`put_override`]) so a late, older revision of the
/// occurrence is known for one. A private property, like [`OCCURRENCE_RESPONSE`].
const OWN_SEQUENCE: &str = "X-MERON-SEQUENCE";

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
fn own_sequence(component: &calcard::icalendar::ICalendarComponent) -> i64 {
    super::text_of(
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
fn occurrences(
    event: &CalendarEvent,
    from: i64,
    to: i64,
    mine: &std::collections::HashSet<String>,
) -> Vec<Occurrence> {
    let overlaps =
        |start: i64, end: i64| start < to && (end > from || (end <= start && start >= from));
    // Only an object that says DECLINED somewhere is read for it.
    let may_be_declined = !mine.is_empty() && event.ics.to_ascii_uppercase().contains("DECLINED");
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
            && super::parse_ical(&event.ics).is_some_and(|ical| {
                // Its one VEVENT: a lone date of a series may be an override.
                ical.components
                    .iter()
                    .find(|component| component.component_type == ICalendarComponentType::VEvent)
                    .and_then(|component| super::own_partstat(component, mine))
                    .is_some_and(|answer| answer == "declined")
            });
        return if declined { Vec::new() } else { single() };
    }
    let ics = if event.ics.is_empty() {
        repeat_ics(event)
    } else {
        event.ics.clone()
    };
    let Some(mut ical) = super::parse_ical(&caldav::ics::for_expansion(&ics)) else {
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
            && super::own_partstat(component, mine).is_some_and(|answer| answer == "declined")
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
                    .then(|| super::text_of(component, prop))
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
        None => super::local_tz(),
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
fn place_rule(event: &mut CalendarEvent, from: Option<NaiveDate>) {
    let Some(from) = from else { return };
    if !event.recurring || event.rrule.is_empty() {
        return;
    }
    let to = first_day(event);
    if from != to {
        event.rrule = super::rrule::align(&event.rrule, from, to);
    }
}

fn settle_rule(before: &CalendarEvent, event: &mut CalendarEvent, as_shown: bool) -> Result<()> {
    if !event.recurring || event.rrule.is_empty() {
        return Ok(());
    }
    let to = first_day(event);
    let from = before.recurring.then(|| first_day(before));
    if let Some(from) = from.filter(|from| *from != to && !as_shown) {
        event.rrule = super::rrule::align(&event.rrule, from, to);
    }
    if event.rrule == before.rrule && from == Some(to) {
        return Ok(());
    }
    super::rrule::check(&event.rrule, to)
}

/// Before a synced series' object goes: a floating series' times are this
/// device's wall times, so it takes this device's zone to keep them so.
pub(crate) fn keep_floating_times(event: &mut CalendarEvent) {
    if event.recurring
        && !event.all_day
        && event.tzid.is_empty()
        && caldav::ics::starts_floating(&event.ics)
    {
        event.tzid = super::local_tzid().unwrap_or_default();
    }
}

/// The zone [`first_day`] counts in, as `event_json` names it.
fn series_zone(event: &CalendarEvent) -> String {
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
        _ if caldav::ics::starts_floating(&event.ics) => super::local_tz(),
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
    use super::rrule::{Timing, to_ics};
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
fn with_occurrence(
    series_ics: &str,
    invite: &Invite,
    response: Option<Response>,
) -> Result<String> {
    let mut series =
        super::parse_ical(series_ics).ok_or_else(|| anyhow!("stored event is not iCalendar"))?;
    let update =
        super::parse_ical(&invite.ics).ok_or_else(|| anyhow!("invitation is not iCalendar"))?;
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
fn without_occurrence(series_ics: &str, recurrence_id: i64, all_day: bool) -> Result<String> {
    let mut series =
        super::parse_ical(series_ics).ok_or_else(|| anyhow!("stored event is not iCalendar"))?;
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
fn with_series_sequence(ics: &str) -> Result<String> {
    let Some(mut series) = super::parse_ical(ics) else {
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
fn carry_exceptions(stored: &str, incoming: &str) -> Result<String> {
    let mut series =
        super::parse_ical(incoming).ok_or_else(|| anyhow!("invitation is not iCalendar"))?;
    let Some(old) = super::parse_ical(stored) else {
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
fn settle_start(event: &mut CalendarEvent) {
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
    let ical = super::parse_ical(&event.ics)?;
    let resolver = ical.build_tz_resolver().with_default(super::local_tz());
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
        super::text_of(component, ICalendarProperty::Tzid)
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
fn master_index(ical: &calcard::icalendar::ICalendar) -> Option<usize> {
    ical.components.iter().position(|component| {
        component.component_type == ICalendarComponentType::VEvent
            && !component.is_recurrence_override()
    })
}

fn overrides(
    ical: &calcard::icalendar::ICalendar,
) -> impl Iterator<Item = &calcard::icalendar::ICalendarComponent> {
    ical.components.iter().filter(|component| {
        component.component_type == ICalendarComponentType::VEvent
            && component.is_recurrence_override()
    })
}

fn remove_overrides(series: &mut calcard::icalendar::ICalendar, recurrence_id: i64) {
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
fn cut_from(series_ics: &str, at: i64) -> Result<String> {
    let mut series =
        super::parse_ical(series_ics).ok_or_else(|| anyhow!("stored event is not iCalendar"))?;
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
    super::text_of(
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
/// [`super::parse_invite`] reads it.
fn recurrence_instant(
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
            .with_default(super::local_tz())
            .resolve_or_default(entry.tz_id())
    } else {
        Tz::UTC
    };
    Some(value.to_date_time_with_tz(tz)?.timestamp())
}

/// Whether `event` (an invitation's series) already leaves out the occurrence
/// starting at `recurrence_id`.
pub(crate) fn excludes_occurrence(event: &CalendarEvent, recurrence_id: i64) -> bool {
    super::parse_ical(&event.ics).is_some_and(|ical| {
        let accepted_since = overrides(&ical)
            .any(|component| recurrence_instant(&ical, component) == Some(recurrence_id));
        exdates(&ical).contains(&recurrence_id)
            || (!accepted_since && ends_before(&ical).is_some_and(|cut| recurrence_id >= cut))
    })
}
