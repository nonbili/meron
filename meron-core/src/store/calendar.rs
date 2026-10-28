//! Answers to calendar invitations — see [`crate::calendar`].

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

use super::now_unix;

/// The user's recorded answer to one event (or one occurrence of it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InviteResponse {
    pub response: String,
    pub sequence: i64,
    pub responded_at: i64,
}

pub fn invite_response(
    conn: &Connection,
    account: &str,
    uid: &str,
    recurrence_id: i64,
) -> Result<Option<InviteResponse>> {
    Ok(conn
        .query_row(
            "SELECT response, sequence, responded_at FROM invite_responses
             WHERE account = ?1 AND uid = ?2 AND recurrence_id = ?3",
            params![account, uid, recurrence_id],
            |row| {
                Ok(InviteResponse {
                    response: row.get(0)?,
                    sequence: row.get(1)?,
                    responded_at: row.get(2)?,
                })
            },
        )
        .optional()?)
}

/// The occurrences of an invitation the user declined one by one, each with
/// whether the decline covers every later occurrence too.
pub fn declined_occurrences(
    conn: &Connection,
    account: &str,
    uid: &str,
) -> Result<Vec<(i64, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT recurrence_id, this_and_future FROM invite_responses
         WHERE account = ?1 AND uid = ?2 AND recurrence_id <> 0 AND response = 'declined'
         ORDER BY recurrence_id",
    )?;
    let rows = stmt.query_map(params![account, uid], |row| {
        Ok((row.get(0)?, row.get::<_, i64>(1)? != 0))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Mark a recorded answer as covering every later occurrence too.
pub fn mark_this_and_future(
    conn: &Connection,
    account: &str,
    uid: &str,
    recurrence_id: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE invite_responses SET this_and_future = 1
         WHERE account = ?1 AND uid = ?2 AND recurrence_id = ?3",
        params![account, uid, recurrence_id],
    )?;
    Ok(())
}

pub fn record_invite_response(
    conn: &Connection,
    account: &str,
    uid: &str,
    recurrence_id: i64,
    response: &str,
    sequence: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO invite_responses (account, uid, recurrence_id, response, sequence, responded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(account, uid, recurrence_id) DO UPDATE SET
           response = excluded.response,
           sequence = excluded.sequence,
           responded_at = excluded.responded_at,
           this_and_future = 0",
        params![account, uid, recurrence_id, response, sequence, now_unix()],
    )?;
    Ok(())
}

// ---- Events -----------------------------------------------------------------

/// One calendar event: made here, or an invitation the user accepted (then
/// `invite_uid` is set and `ics` holds the organizer's calendar). Times are
/// Unix seconds; an all-day event's are UTC midnights with `end_at` exclusive.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub location: String,
    pub start_at: i64,
    pub end_at: i64,
    pub all_day: bool,
    pub recurring: bool,
    /// An event made here repeats `daily`, `weekly`, `monthly` or `yearly`;
    /// empty for once, and for an invitation, whose rule lives in `ics`.
    pub rrule: String,
    /// The IANA zone a repeating event keeps its wall-clock time in.
    pub tzid: String,
    pub account: String,
    pub thread_id: String,
    pub message_id: String,
    pub invite_uid: String,
    /// An invitation to one occurrence, accepted while its series is not in
    /// the calendar: that occurrence's RECURRENCE-ID. 0 otherwise.
    pub recurrence_id: i64,
    pub response: String,
    pub ics: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// At most two alerts: minutes before a timed start, or for an all-day
    /// event days before it, at 09:00. `None` follows the app-wide default and
    /// an empty list means no alert.
    pub alerts: Option<Vec<i64>>,
    /// Local schedule-change metadata; excluded from backups and undo payloads.
    #[serde(skip)]
    pub reminder_after: i64,
}

const EVENT_COLUMNS: &str = "id, title, notes, location, start_at, end_at, all_day, recurring,
    rrule, tzid, account, thread_id, message_id, invite_uid, response, ics, created_at, updated_at,
    recurrence_id, COALESCE(json_extract(json, '$.reminder_after'), created_at), alerts";

fn read_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEvent> {
    Ok(CalendarEvent {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        location: row.get(3)?,
        start_at: row.get(4)?,
        end_at: row.get(5)?,
        all_day: row.get::<_, i64>(6)? != 0,
        recurring: row.get::<_, i64>(7)? != 0,
        rrule: row.get(8)?,
        tzid: row.get(9)?,
        account: row.get(10)?,
        thread_id: row.get(11)?,
        message_id: row.get(12)?,
        invite_uid: row.get(13)?,
        response: row.get(14)?,
        ics: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
        recurrence_id: row.get(18)?,
        reminder_after: row.get(19)?,
        alerts: row
            .get::<_, Option<String>>(20)?
            .and_then(|alerts| serde_json::from_str(&alerts).ok()),
    })
}

pub fn event(conn: &Connection, id: &str) -> Result<Option<CalendarEvent>> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM events WHERE id = ?1");
    Ok(conn.query_row(&sql, params![id], read_event).optional()?)
}

/// The event an accepted invitation became, if it is in the calendar.
pub fn invite_event(conn: &Connection, account: &str, uid: &str) -> Result<Option<CalendarEvent>> {
    invite_occurrence_event(conn, account, uid, 0)
}

/// An occurrence filed on its own (see [`CalendarEvent::recurrence_id`]), or
/// with `recurrence_id` 0 the whole event.
pub fn invite_occurrence_event(
    conn: &Connection,
    account: &str,
    uid: &str,
    recurrence_id: i64,
) -> Result<Option<CalendarEvent>> {
    if uid.is_empty() {
        return Ok(None);
    }
    let sql = format!(
        "SELECT {EVENT_COLUMNS} FROM events
         WHERE account = ?1 AND invite_uid = ?2 AND recurrence_id = ?3"
    );
    Ok(conn
        .query_row(&sql, params![account, uid, recurrence_id], read_event)
        .optional()?)
}

/// The invitations about `uid` among the account's downloaded messages, as
/// (From address, Message-ID, invitation).
pub fn cached_invites(
    conn: &Connection,
    account: &str,
    uid: &str,
) -> Result<Vec<(String, String, crate::calendar::Invite)>> {
    let mut stmt = conn.prepare(
        "SELECT from_addr, json_extract(json, '$.message_id'), json_extract(json, '$.invite')
         FROM messages
         WHERE account = ?1 AND json_extract(json, '$.invite.uid') = ?2",
    )?;
    let rows = stmt.query_map(params![account, uid], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut invites = Vec::new();
    for row in rows {
        let (from, message_id, invite) = row?;
        if let Ok(invite) = serde_json::from_str(&invite) {
            invites.push((from, message_id, invite));
        }
    }
    Ok(invites)
}

/// Every occurrence of an invitation filed on its own.
pub fn invite_occurrence_events(
    conn: &Connection,
    account: &str,
    uid: &str,
) -> Result<Vec<CalendarEvent>> {
    let sql = format!(
        "SELECT {EVENT_COLUMNS} FROM events
         WHERE account = ?1 AND invite_uid = ?2 AND invite_uid <> '' AND recurrence_id <> 0"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![account, uid], read_event)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Events that can have an occurrence in `[from, to)`: a single one that
/// overlaps it, or any repeating one that has started by `to` — those are
/// expanded by the caller.
pub fn events_between(conn: &Connection, from: i64, to: i64) -> Result<Vec<CalendarEvent>> {
    let sql = format!(
        "SELECT {EVENT_COLUMNS} FROM events
         WHERE start_at < ?2 AND (recurring <> 0 OR end_at > ?1 OR (end_at <= start_at AND start_at >= ?1))
         ORDER BY start_at, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![from, to], read_event)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Write `event` whole: insert, or replace the row with its id. `created_at`
/// survives a replace.
pub fn save_event(conn: &Connection, event: &CalendarEvent) -> Result<CalendarEvent> {
    let now = now_unix();
    let invite_schedule_changed = self::event(conn, &event.id)?.is_some_and(|previous| {
        !crate::calendar::same_reminder_schedule(&previous.ics, &event.ics)
    });
    conn.execute(
        "INSERT INTO events (id, title, notes, location, start_at, end_at, all_day, recurring,
           rrule, tzid, account, thread_id, message_id, invite_uid, response, ics, created_at, updated_at,
           recurrence_id, json, alerts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17, ?18, json_object('reminder_after', ?17), ?20)
         ON CONFLICT(id) DO UPDATE SET
           title = excluded.title, notes = excluded.notes, location = excluded.location,
           start_at = excluded.start_at, end_at = excluded.end_at, all_day = excluded.all_day,
           recurring = excluded.recurring, rrule = excluded.rrule, tzid = excluded.tzid,
           account = excluded.account, thread_id = excluded.thread_id,
           message_id = excluded.message_id, invite_uid = excluded.invite_uid,
           response = excluded.response, ics = excluded.ics, updated_at = excluded.updated_at,
           recurrence_id = excluded.recurrence_id, alerts = excluded.alerts,
           json = CASE WHEN events.start_at <> excluded.start_at OR events.all_day <> excluded.all_day
                         OR events.rrule <> excluded.rrule OR events.tzid <> excluded.tzid OR ?19
                         OR events.alerts IS NOT excluded.alerts
                  THEN json_set(events.json, '$.reminder_after', excluded.updated_at)
                  ELSE events.json END",
        params![
            event.id,
            event.title,
            event.notes,
            event.location,
            event.start_at,
            event.end_at,
            event.all_day as i64,
            event.recurring as i64,
            event.rrule,
            event.tzid,
            event.account,
            event.thread_id,
            event.message_id,
            event.invite_uid,
            event.response,
            event.ics,
            now,
            event.recurrence_id,
            invite_schedule_changed,
            event
                .alerts
                .as_ref()
                .map(|alerts| serde_json::to_string(alerts).unwrap_or_default()),
        ],
    )?;
    Ok(self::event(conn, &event.id)?.unwrap_or_else(|| event.clone()))
}

pub fn all_events(conn: &Connection) -> Result<Vec<CalendarEvent>> {
    let sql = format!("SELECT {EVENT_COLUMNS} FROM events ORDER BY start_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], read_event)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// [`save_event`] for a restore: write `event` only when it is new — by id,
/// and for an invitation by the event it was filed for, which the calendar
/// holds once per account.
pub fn insert_event_if_absent(conn: &Connection, event: &CalendarEvent) -> Result<bool> {
    if self::event(conn, &event.id)?.is_some()
        || invite_occurrence_event(conn, &event.account, &event.invite_uid, event.recurrence_id)?
            .is_some()
    {
        return Ok(false);
    }
    save_event(conn, event)?;
    Ok(true)
}

pub fn delete_event(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.execute("DELETE FROM events WHERE id = ?1", params![id])? > 0)
}
