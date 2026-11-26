//! The local calendar: the shape both frontends render for a range of days,
//! and the writes behind its editor.
//!
//! Like Tasks, everything is on this device: events the user typed in, and
//! the invitations they accepted (filed by [`super::respond`]). A month view
//! asks for a range and gets back occurrences — a repeating event expanded,
//! by calcard, from its own rule — with the open tasks due in it alongside.

#[cfg(test)]
mod tests;

mod invitations;
mod recurrence;
pub use invitations::{apply_cached_updates, apply_invite_update, record_invite_event};
pub(crate) use recurrence::{
    date_lines, early_occurrence_start, excludes_occurrence, first_day, keep_floating_times,
    rrule_ics, rule_tz,
};
use recurrence::{occurrences, place_rule, series_zone, settle_rule};

use anyhow::{Result, anyhow};
use calcard::common::timezone::Tz;
use chrono::{Datelike, NaiveDate, TimeZone};
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::caldav;
use crate::store::{self, CalendarEvent, Task};

const DAY: i64 = 86_400;

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
