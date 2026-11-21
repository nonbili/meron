//! What the store keeps for CalDAV: the accounts, the calendars each one has,
//! and for every synced event which server object it is and whether it has
//! local changes still to send.
//!
//! The events themselves are ordinary rows in `events`, marked with the
//! calendar they came from, so the calendar view, reminders and search treat
//! them like any other. The password is not here: it lives where mail
//! passwords do (the OS keychain on desktop, `account_secrets` on mobile).

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

use crate::store::now_unix;

// The tables are created by store migration 0030 (`store/db/migrations`).

/// Local edits waiting to be sent.
pub const CREATED: &str = "created";
pub const CHANGED: &str = "changed";
pub const DELETED: &str = "deleted";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DavAccount {
    pub id: String,
    pub server_url: String,
    pub username: String,
    pub display_name: String,
    pub home: String,
    pub google: bool,
    pub google_calendar_list_disabled: bool,
    pub last_sync_at: i64,
    pub last_error: String,
    /// Minutes between automatic syncs; 0 syncs only when asked.
    pub sync_interval: i64,
    /// Unix deadline for a Google throttle, shared by all sync entry points.
    pub retry_at: i64,
    pub throttle_count: u32,
}

impl DavAccount {
    /// Discard implausible deadlines after a backward system clock jump.
    pub fn cooling_down(&self, now: i64) -> bool {
        let remaining = self.retry_at.saturating_sub(now);
        (1..=super::MAX_RETRY_AFTER_SECS).contains(&remaining)
    }

    pub(super) fn cooldown_error(&self, now: i64) -> Option<String> {
        self.cooling_down(now).then(|| {
            let minutes = (self.retry_at - now + 59) / 60;
            format!("{} Try again in about {minutes} min.", self.last_error)
        })
    }
}

/// Minutes between automatic syncs for an account that has not chosen.
pub const DEFAULT_SYNC_INTERVAL: i64 = 15;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DavCalendar {
    pub id: String,
    pub account: String,
    pub url: String,
    pub name: String,
    pub color: String,
    pub components: Vec<String>,
    pub ctag: String,
    pub sync_token: String,
    pub read_only: bool,
    pub enabled: bool,
    /// When every object was last listed and compared, rather than only
    /// the changes a sync token reported.
    pub listed_at: i64,
    /// A colour chosen here for a calendar whose server would not take it;
    /// empty otherwise. Wins over `color`.
    pub local_color: String,
}

impl DavCalendar {
    /// The colour its events are drawn in, as far as the server and the
    /// user have said; empty leaves it to the apps.
    pub fn shown_color(&self) -> &str {
        if self.local_color.is_empty() {
            &self.color
        } else {
            &self.local_color
        }
    }

    pub fn supports(&self, component: &str) -> bool {
        self.components.is_empty()
            || self
                .components
                .iter()
                .any(|c| c.eq_ignore_ascii_case(component))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DavObject {
    pub calendar: String,
    pub url: String,
    pub etag: String,
    pub local_id: String,
    pub state: String,
    /// For a deletion left by a move: the object the event moved to. The
    /// deletion waits until that one is on its server.
    pub moved_to: String,
}

const ACCOUNT_COLUMNS: &str = "id, server_url, username, display_name, home, last_sync_at,
    last_error, json_extract(json, '$.sync_interval'), json_extract(json, '$.google'),
    json_extract(json, '$.retry_at'), json_extract(json, '$.throttle_count'),
    json_extract(json, '$.google_calendar_list_disabled')";

fn read_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<DavAccount> {
    Ok(DavAccount {
        id: row.get(0)?,
        server_url: row.get(1)?,
        username: row.get(2)?,
        display_name: row.get(3)?,
        home: row.get(4)?,
        google: row.get::<_, Option<bool>>(8)?.unwrap_or(false),
        google_calendar_list_disabled: row.get::<_, Option<bool>>(11)?.unwrap_or(false),
        last_sync_at: row.get(5)?,
        last_error: row.get(6)?,
        sync_interval: row
            .get::<_, Option<i64>>(7)?
            .unwrap_or(DEFAULT_SYNC_INTERVAL),
        retry_at: row.get::<_, Option<i64>>(9)?.unwrap_or(0),
        throttle_count: row.get::<_, Option<u32>>(10)?.unwrap_or(0),
    })
}

pub fn accounts(conn: &Connection) -> Result<Vec<DavAccount>> {
    let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM dav_accounts ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], read_account)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn account(conn: &Connection, id: &str) -> Result<Option<DavAccount>> {
    let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM dav_accounts WHERE id = ?1");
    Ok(conn.query_row(&sql, params![id], read_account).optional()?)
}

pub fn save_account(conn: &Connection, account: &DavAccount) -> Result<()> {
    let now = now_unix();
    conn.execute(
        "INSERT INTO dav_accounts (id, server_url, username, display_name, home, created_at, updated_at, json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, json_object('google', ?7))
         ON CONFLICT(id) DO UPDATE SET
           server_url = excluded.server_url, username = excluded.username,
           display_name = excluded.display_name, home = excluded.home,
           json = json_set(dav_accounts.json, '$.google', ?7),
           updated_at = excluded.updated_at",
        params![
            account.id,
            account.server_url,
            account.username,
            account.display_name,
            account.home,
            now,
            account.google,
        ],
    )?;
    Ok(())
}

pub fn set_sync_interval(conn: &Connection, id: &str, minutes: i64) -> Result<()> {
    conn.execute(
        "UPDATE dav_accounts SET json = json_set(json, '$.sync_interval', ?2) WHERE id = ?1",
        params![id, minutes.max(0)],
    )?;
    Ok(())
}

pub(super) fn set_google_calendar_list_disabled(
    conn: &Connection,
    id: &str,
    disabled: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE dav_accounts SET json = json_set(json, '$.google_calendar_list_disabled', ?2) WHERE id = ?1",
        params![id, disabled],
    )?;
    Ok(())
}

pub fn record_sync(conn: &Connection, id: &str, error: &str) -> Result<()> {
    record_sync_with_retry(conn, id, error, 0, 0)
}

pub(super) fn record_sync_with_retry(
    conn: &Connection,
    id: &str,
    error: &str,
    retry_at: i64,
    throttle_count: u32,
) -> Result<()> {
    conn.execute(
        "UPDATE dav_accounts SET last_error = ?2,
           last_sync_at = CASE WHEN ?2 = '' THEN ?3 ELSE last_sync_at END,
           json = CASE WHEN ?4 > 0 THEN json_set(json, '$.retry_at', ?4, '$.throttle_count', ?5)
                       WHEN ?2 = '' THEN json_remove(json, '$.retry_at', '$.throttle_count')
                       ELSE json_remove(json, '$.retry_at') END
         WHERE id = ?1",
        params![id, error, now_unix(), retry_at, throttle_count],
    )?;
    Ok(())
}

/// Remove an account and everything synced from it. The server keeps its
/// copy; only this device forgets.
pub fn delete_account(conn: &Connection, id: &str) -> Result<()> {
    for calendar in calendars(conn, id)? {
        delete_calendar(conn, &calendar.id)?;
    }
    conn.execute("DELETE FROM dav_accounts WHERE id = ?1", params![id])?;
    Ok(())
}

const CALENDAR_COLUMNS: &str = "id, account, url, name, color, components, ctag, sync_token,
    read_only, enabled, COALESCE(json_extract(json, '$.listed_at'), 0),
    COALESCE(json_extract(json, '$.local_color'), '')";

fn read_calendar(row: &rusqlite::Row<'_>) -> rusqlite::Result<DavCalendar> {
    let components: String = row.get(5)?;
    Ok(DavCalendar {
        id: row.get(0)?,
        account: row.get(1)?,
        url: row.get(2)?,
        name: row.get(3)?,
        color: row.get(4)?,
        components: components
            .split(',')
            .filter(|c| !c.is_empty())
            .map(str::to_string)
            .collect(),
        ctag: row.get(6)?,
        sync_token: row.get(7)?,
        read_only: row.get::<_, i64>(8)? != 0,
        enabled: row.get::<_, i64>(9)? != 0,
        listed_at: row.get(10)?,
        local_color: row.get(11)?,
    })
}

pub fn calendars(conn: &Connection, account: &str) -> Result<Vec<DavCalendar>> {
    let sql = format!(
        "SELECT {CALENDAR_COLUMNS} FROM dav_calendars WHERE account = ?1 ORDER BY sort_order, name, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![account], read_calendar)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn all_calendars(conn: &Connection) -> Result<Vec<DavCalendar>> {
    let sql = format!(
        "SELECT {CALENDAR_COLUMNS} FROM dav_calendars ORDER BY account, sort_order, name, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], read_calendar)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn calendar(conn: &Connection, id: &str) -> Result<Option<DavCalendar>> {
    let sql = format!("SELECT {CALENDAR_COLUMNS} FROM dav_calendars WHERE id = ?1");
    Ok(conn
        .query_row(&sql, params![id], read_calendar)
        .optional()?)
}

/// Bring the account's calendars in line with what the server lists: new ones
/// are added (enabled), known ones renamed or recoloured, and ones the server
/// no longer has removed with their events. Sync state is left alone; it
/// moves only when a sync completes ([`set_synced`]).
pub fn reconcile_calendars(
    conn: &Connection,
    account: &str,
    listed: &[super::Calendar],
) -> Result<Vec<DavCalendar>> {
    let now = now_unix();
    for (order, calendar) in listed.iter().enumerate() {
        conn.execute(
            "INSERT INTO dav_calendars (id, account, url, name, color, components, read_only,
               sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(account, url) DO UPDATE SET
               name = excluded.name, color = excluded.color,
               components = excluded.components, read_only = excluded.read_only,
               sort_order = excluded.sort_order, updated_at = excluded.updated_at",
            params![
                crate::tasks::new_id("cal"),
                account,
                calendar.url,
                calendar.name,
                calendar.color,
                calendar.components.join(","),
                calendar.read_only as i64,
                order as i64,
                now,
            ],
        )?;
    }
    for known in calendars(conn, account)? {
        if !listed.iter().any(|c| c.url == known.url) {
            keep_unsent_here(conn, &known.id)?;
            delete_calendar(conn, &known.id)?;
        }
    }
    calendars(conn, account)
}

/// Record a completed pull. `listed`: it compared every object, not only
/// the reported changes.
/// Add one calendar the server has just made, leaving the others alone
/// (unlike [`reconcile_calendars`], which takes the whole list).
pub fn add_calendar(
    conn: &Connection,
    account: &str,
    calendar: &super::Calendar,
) -> Result<Vec<DavCalendar>> {
    let now = now_unix();
    conn.execute(
        "INSERT INTO dav_calendars (id, account, url, name, color, components, read_only,
           sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0,
           (SELECT COALESCE(MAX(sort_order), -1) + 1 FROM dav_calendars WHERE account = ?2), ?7, ?7)
         ON CONFLICT(account, url) DO NOTHING",
        params![
            crate::tasks::new_id("cal"),
            account,
            calendar.url,
            calendar.name,
            calendar.color,
            calendar.components.join(","),
            now,
        ],
    )?;
    calendars(conn, account)
}

pub fn set_synced(
    conn: &Connection,
    id: &str,
    ctag: &str,
    sync_token: &str,
    listed: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE dav_calendars SET ctag = ?2, sync_token = ?3,
           json = CASE WHEN ?4 THEN json_set(json, '$.listed_at', ?5) ELSE json END
         WHERE id = ?1",
        params![id, ctag, sync_token, listed, now_unix()],
    )?;
    Ok(())
}

pub(super) fn invalidate_ctag(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE dav_calendars SET ctag = '' WHERE id = ?1", [id])?;
    Ok(())
}

/// What the server now says a calendar is called and coloured, after the
/// user changed it here. A colour the server took replaces any kept here.
pub fn set_calendar_props(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    color: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE dav_calendars SET name = COALESCE(?2, name), color = COALESCE(?3, color),
           json = CASE WHEN ?3 IS NULL THEN json ELSE json_remove(json, '$.local_color') END
         WHERE id = ?1",
        params![id, name, color],
    )?;
    Ok(())
}

/// A colour for a calendar kept on this device only; empty clears it.
pub fn set_local_color(conn: &Connection, id: &str, color: &str) -> Result<()> {
    conn.execute(
        "UPDATE dav_calendars SET json = CASE WHEN ?2 = '' THEN json_remove(json, '$.local_color')
           ELSE json_set(json, '$.local_color', ?2) END WHERE id = ?1",
        params![id, color],
    )?;
    Ok(())
}

/// Turn a calendar off or on. Off drops its events from this device and
/// forgets its sync state, so turning it on again starts afresh.
pub fn set_enabled(conn: &Connection, id: &str, enabled: bool) -> Result<()> {
    if !enabled {
        forget_contents(conn, id)?;
    }
    conn.execute(
        "UPDATE dav_calendars SET enabled = ?2 WHERE id = ?1",
        params![id, enabled as i64],
    )?;
    Ok(())
}

/// The calendar is gone from its server: events created or changed here
/// that never reached it stay on this device instead of going with it.
fn keep_unsent_here(conn: &Connection, id: &str) -> Result<()> {
    // A floating series that leaves its object keeps its wall times by
    // taking this device's zone (see `keep_floating_times`).
    let floating: Vec<(String, String)> = conn
        .prepare(
            "SELECT id, ics FROM events
             WHERE calendar = ?1 AND recurring <> 0 AND rrule <> '' AND all_day = 0 AND tzid = ''
               AND id IN (SELECT local_id FROM dav_objects
                          WHERE calendar = ?1 AND state IN (?2, ?3) AND local_id <> '')",
        )?
        .query_map(params![id, CREATED, CHANGED], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let zone = crate::calendar::local_tzid().unwrap_or_default();
    for (event, ics) in floating {
        if super::ics::starts_floating(&ics) {
            conn.execute(
                "UPDATE events SET tzid = ?2 WHERE id = ?1",
                params![event, zone],
            )?;
        }
    }
    conn.execute(
        "UPDATE events SET calendar = '',
           -- A repeat the editor can name is kept by `rrule`; a server's own
           -- rule only by the object itself.
           ics = CASE WHEN recurring <> 0 AND rrule = '' THEN ics ELSE '' END
         WHERE calendar = ?1 AND id IN (
           SELECT local_id FROM dav_objects
           WHERE calendar = ?1 AND state IN (?2, ?3) AND local_id <> '')",
        params![id, CREATED, CHANGED],
    )?;
    Ok(())
}

fn forget_contents(conn: &Connection, id: &str) -> Result<()> {
    // Moves into this calendar that never got here are called off: the
    // calendars they left keep the event, and list everything again on
    // their next sync to show it once more.
    conn.execute(
        "UPDATE dav_calendars SET ctag = '', sync_token = '',
           json = json_set(json, '$.listed_at', 0)
         WHERE id IN (SELECT calendar FROM dav_objects WHERE moved_to IN (
           SELECT url FROM dav_objects WHERE calendar = ?1 AND state = ?2))",
        params![id, CREATED],
    )?;
    conn.execute(
        "DELETE FROM dav_objects WHERE moved_to IN (
           SELECT url FROM dav_objects WHERE calendar = ?1 AND state = ?2)",
        params![id, CREATED],
    )?;
    conn.execute("DELETE FROM events WHERE calendar = ?1", params![id])?;
    conn.execute("DELETE FROM dav_objects WHERE calendar = ?1", params![id])?;
    conn.execute(
        "UPDATE dav_calendars SET ctag = '', sync_token = '' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn delete_calendar(conn: &Connection, id: &str) -> Result<()> {
    forget_contents(conn, id)?;
    conn.execute("DELETE FROM dav_calendars WHERE id = ?1", params![id])?;
    Ok(())
}

const OBJECT_COLUMNS: &str = "calendar, url, etag, local_id, state, moved_to";

fn read_object(row: &rusqlite::Row<'_>) -> rusqlite::Result<DavObject> {
    Ok(DavObject {
        calendar: row.get(0)?,
        url: row.get(1)?,
        etag: row.get(2)?,
        local_id: row.get(3)?,
        state: row.get(4)?,
        moved_to: row.get(5)?,
    })
}

pub fn objects(conn: &Connection, calendar: &str) -> Result<Vec<DavObject>> {
    let sql = format!("SELECT {OBJECT_COLUMNS} FROM dav_objects WHERE calendar = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![calendar], read_object)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn object(conn: &Connection, calendar: &str, url: &str) -> Result<Option<DavObject>> {
    let sql = format!("SELECT {OBJECT_COLUMNS} FROM dav_objects WHERE calendar = ?1 AND url = ?2");
    Ok(conn
        .query_row(&sql, params![calendar, url], read_object)
        .optional()?)
}

pub fn object_for_event(conn: &Connection, event_id: &str) -> Result<Option<DavObject>> {
    let sql = format!("SELECT {OBJECT_COLUMNS} FROM dav_objects WHERE local_id = ?1");
    Ok(conn
        .query_row(&sql, params![event_id], read_object)
        .optional()?)
}

pub fn pending(conn: &Connection, calendar: &str) -> Result<Vec<DavObject>> {
    let sql = format!(
        "SELECT {OBJECT_COLUMNS} FROM dav_objects WHERE calendar = ?1 AND state <> '' ORDER BY url"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![calendar], read_object)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn save_object(conn: &Connection, object: &DavObject) -> Result<()> {
    conn.execute(
        "INSERT INTO dav_objects (calendar, url, etag, local_id, state, moved_to)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(calendar, url) DO UPDATE SET
           etag = excluded.etag, local_id = excluded.local_id, state = excluded.state,
           moved_to = excluded.moved_to",
        params![
            object.calendar,
            object.url,
            object.etag,
            object.local_id,
            object.state,
            object.moved_to
        ],
    )?;
    Ok(())
}

pub fn delete_object(conn: &Connection, calendar: &str, url: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM dav_objects WHERE calendar = ?1 AND url = ?2",
        params![calendar, url],
    )?;
    Ok(())
}

/// Note that a synced event was edited here. One created here and not yet
/// sent stays `created`.
pub fn mark_changed(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE dav_objects SET state = ?2 WHERE local_id = ?1 AND state IN ('', ?3)",
        params![event_id, CHANGED, DELETED],
    )?;
    Ok(())
}

/// Note that a synced event is leaving its calendar for another: like
/// [`mark_deleted`], but the server object stops naming the event, which is
/// about to be another object's.
pub fn detach_for_move(conn: &Connection, event_id: &str) -> Result<Option<DavObject>> {
    conn.execute(
        "DELETE FROM dav_objects WHERE local_id = ?1 AND state = ?2",
        params![event_id, CREATED],
    )?;
    let Some(source) = object_for_event(conn, event_id)? else {
        return Ok(None);
    };
    conn.execute(
        "UPDATE dav_objects SET state = ?3, local_id = '' WHERE calendar = ?1 AND url = ?2",
        params![source.calendar, source.url, DELETED],
    )?;
    Ok(Some(source))
}

/// Make the deletion a move left at `source` wait for `destination`.
pub fn link_move(conn: &Connection, source: &DavObject, destination: &str) -> Result<()> {
    conn.execute(
        "UPDATE dav_objects SET moved_to = ?3 WHERE calendar = ?1 AND url = ?2",
        params![source.calendar, source.url, destination],
    )?;
    Ok(())
}

/// Deletions that waited for a copy at `from` wait for `to` instead, or, with
/// `to` empty, for nothing.
pub fn retarget_moves(conn: &Connection, from: &str, to: &str) -> Result<()> {
    conn.execute(
        "UPDATE dav_objects SET moved_to = ?2 WHERE moved_to = ?1",
        params![from, to],
    )?;
    Ok(())
}

/// The object at `url`, in whichever calendar holds it.
pub fn object_at(conn: &Connection, url: &str) -> Result<Option<DavObject>> {
    let sql = format!("SELECT {OBJECT_COLUMNS} FROM dav_objects WHERE url = ?1 LIMIT 1");
    Ok(conn.query_row(&sql, params![url], read_object).optional()?)
}

/// How many events an account's calendars show here.
pub fn event_count(conn: &Connection, account: &str) -> Result<usize> {
    Ok(conn.query_row(
        "SELECT count(*) FROM events WHERE calendar IN
           (SELECT id FROM dav_calendars WHERE account = ?1)",
        params![account],
        |row| row.get::<_, i64>(0),
    )? as usize)
}

/// Local changes not yet on the server, in one calendar or (`None`) all of
/// an account's.
pub fn unsent(conn: &Connection, account: &str, calendar: Option<&str>) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM dav_objects o JOIN dav_calendars c ON c.id = o.calendar
         WHERE c.account = ?1 AND (?2 IS NULL OR c.id = ?2) AND o.state <> ''",
        params![account, calendar],
        |row| row.get(0),
    )?)
}

/// Note that a synced event was deleted here. One never sent is simply
/// forgotten.
pub fn mark_deleted(conn: &Connection, event_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM dav_objects WHERE local_id = ?1 AND state = ?2",
        params![event_id, CREATED],
    )?;
    conn.execute(
        "UPDATE dav_objects SET state = ?2 WHERE local_id = ?1",
        params![event_id, DELETED],
    )?;
    Ok(())
}
