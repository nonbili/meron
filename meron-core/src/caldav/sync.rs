//! One sync of a CalDAV account: send what was changed here, then take in
//! what changed on the server.
//!
//! Local edits go first so a sync never fetches over an edit not yet sent.
//! When both sides changed the same event the server wins: the edit here is
//! dropped and the server's version fetched. That loses the local edit, but
//! never the server's, which other devices may already show.
//!
//! The store is locked only between network calls, never across one: a sync
//! can take minutes on a slow server and the UI reads the same tables. So
//! whatever a request's answer is applied to is read again afterwards — the
//! user may have edited, deleted or undone a deletion meanwhile — and a pull
//! lands only in a calendar that is still there and still shown.

use std::collections::HashSet;

use anyhow::{Result, anyhow};
use rusqlite::Connection;

use super::store::{self, CHANGED, CREATED, DELETED, DavCalendar, DavObject};
use super::{
    AuthFailed, Calendar, Client, Entry, PreconditionFailed, SyncTokenInvalid, UidConflict, ics,
};
use crate::store as db;

/// Somewhere to run a closure against the store: the sidecar's shared
/// connection behind its mutex, or a connection mobile opened for the sync.
pub trait Db {
    fn with<R>(&self, f: impl FnOnce(&Connection) -> Result<R>) -> Result<R>;
}

impl Db for std::sync::Mutex<Connection> {
    fn with<R>(&self, f: impl FnOnce(&Connection) -> Result<R>) -> Result<R> {
        let conn = self.lock().map_err(|_| anyhow!("store lock poisoned"))?;
        f(&conn)
    }
}

impl Db for Connection {
    fn with<R>(&self, f: impl FnOnce(&Connection) -> Result<R>) -> Result<R> {
        f(self)
    }
}

/// What a sync did, for the caller to decide whether to tell the UI.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Local events added, changed or removed from server data.
    pub pulled: usize,
    /// Local edits sent.
    pub pushed: usize,
    /// Why some or all of the sync failed; empty when nothing did. What was
    /// pulled or pushed before the failure is counted all the same.
    pub error: String,
}

impl Report {
    fn add(&mut self, other: Report) {
        self.pulled += other.pulled;
        self.pushed += other.pushed;
        if self.error.is_empty() {
            self.error = other.error;
        }
    }

    fn fail(&mut self, err: anyhow::Error) {
        if self.error.is_empty() {
            self.error = format!("{err:#}");
        }
    }
}

/// Sync every enabled calendar of `account`. A calendar that fails does not
/// stop the others. The first failure is in the report and recorded on the
/// account for the UI.
pub fn sync_account(db: &impl Db, client: &Client, account_id: &str) -> Report {
    let mut report = Report::default();
    if let Err(err) = sync_calendars(db, client, account_id, &mut report) {
        report.fail(err);
    }
    if let Err(err) = db.with(|conn| store::record_sync(conn, account_id, &report.error)) {
        report.fail(err);
    }
    report
}

/// How long a calendar goes on sync tokens alone before every object is
/// listed and compared again.
const RELIST_AFTER_SECS: i64 = 24 * 60 * 60;

/// The kinds of local edit [`push`] sends in one pass.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    /// Creates and changes.
    Writes,
    /// Deletions whose move, if any, has landed.
    Deletions,
}

#[derive(Default)]
struct Pushed {
    sent: usize,
    /// Writes the server refused because its copy changed, which was
    /// fetched in their place.
    conflicts: usize,
    /// The first object that could not be sent; the rest were tried.
    error: Option<anyhow::Error>,
}

fn sync_calendars(
    db: &impl Db,
    client: &Client,
    account_id: &str,
    report: &mut Report,
) -> Result<()> {
    let account = db
        .with(|conn| store::account(conn, account_id))?
        .ok_or_else(|| anyhow!("no such CalDAV account: {account_id}"))?;
    let listed = if account.home.is_empty() {
        let found = client.discover()?;
        db.with(|conn| {
            store::save_account(
                conn,
                &store::DavAccount {
                    home: found.home.clone(),
                    ..account.clone()
                },
            )
        })?;
        found.calendars
    } else {
        client.calendars(&account.home)?
    };
    let calendars = db.with(|conn| {
        // Removed meanwhile (the caller holds the account's sync guard, so
        // only before this sync began): nothing to bring back.
        if store::account(conn, account_id)?.is_none() {
            return Ok(None);
        }
        let before = store::event_count(conn, account_id)?;
        let calendars = store::reconcile_calendars(conn, account_id, &listed)?;
        // A calendar gone from the server takes its events with it.
        report.pulled += before.saturating_sub(store::event_count(conn, account_id)?);
        Ok(Some(calendars))
    })?;
    let Some(calendars) = calendars else {
        return Ok(());
    };
    let synced: Vec<&DavCalendar> = calendars
        .iter()
        .filter(|c| c.enabled && c.supports("VEVENT"))
        .collect();

    // A move creates the event in its new calendar before deleting it from
    // the old, so a failed create never leaves it on neither. Creates go
    // first everywhere; a server that refuses one for its UID, still in the
    // old calendar, gets the deletion first and the create retried after.
    let mut uid_conflicts = HashSet::new();
    let mut conflicts = HashSet::new();
    for pass in [Pass::Writes, Pass::Deletions] {
        for calendar in &synced {
            match push(db, client, calendar, pass, &mut uid_conflicts) {
                Ok(pushed) => {
                    report.pushed += pushed.sent;
                    // The server's copy replaced each refused edit here.
                    report.pulled += pushed.conflicts;
                    if pushed.conflicts > 0 {
                        conflicts.insert(calendar.id.clone());
                    }
                    if let Some(err) = pushed.error {
                        report.fail(err.context(format!("calendar {}", calendar.name)));
                    }
                }
                Err(err) if err.is::<AuthFailed>() => return Err(err),
                Err(err) => report.fail(err.context(format!("calendar {}", calendar.name))),
            }
        }
    }
    for calendar in synced {
        let remote = listed.iter().find(|c| c.url == calendar.url);
        let conflicted = conflicts.contains(&calendar.id);
        let mut done = Report::default();
        let result = sync_calendar(db, client, calendar, remote, conflicted, &mut done);
        report.add(done);
        match result {
            Ok(()) => {}
            // A wrong password fails every calendar the same way.
            Err(err) if err.is::<AuthFailed>() => return Err(err),
            Err(err) => report.fail(err.context(format!("calendar {}", calendar.name))),
        }
    }
    Ok(())
}

fn sync_calendar(
    db: &impl Db,
    client: &Client,
    calendar: &DavCalendar,
    remote: Option<&Calendar>,
    conflicted: bool,
    report: &mut Report,
) -> Result<()> {
    // What is still pending: creates refused for a UID until its deletion
    // went, and anything edited since the passes above.
    let pushed = push(db, client, calendar, Pass::Writes, &mut HashSet::new())?;
    report.pushed += pushed.sent;
    report.pulled += pushed.conflicts;
    // An edit the server refuses stays queued; the pull goes ahead.
    if let Some(err) = pushed.error {
        report.fail(err);
    }
    let remote_ctag = remote.map(|r| r.ctag.as_str()).unwrap_or_default();
    // Nothing changed on the server since the last sync, and nothing was sent
    // or refused just now that would have changed it.
    let quiet = pushed.sent == 0 && pushed.conflicts == 0 && !conflicted;
    // Some servers answer a sync token they no longer know with everything
    // they hold, which looks like a delta and so hides deletions. A daily
    // full listing catches those.
    let relist = crate::store::now_unix() - calendar.listed_at > RELIST_AFTER_SECS;
    if quiet && !relist && !remote_ctag.is_empty() && remote_ctag == calendar.ctag {
        return Ok(());
    }

    let known = db.with(|conn| store::objects(conn, &calendar.id))?;
    let supports_sync = remote.is_some_and(|r| !r.sync_token.is_empty());
    let delta = if supports_sync && !relist {
        match client.changes(&calendar.url, &calendar.sync_token) {
            Ok(changes) => Some(changes),
            Err(err) if err.is::<SyncTokenInvalid>() => None,
            Err(err) => return Err(err),
        }
    } else {
        None
    };
    let listed = delta.is_none();
    let (changed, mut removed, token) = match delta {
        Some(delta) => (delta.changed, delta.removed, delta.sync_token),
        None => {
            let listed = client.list(&calendar.url)?;
            let removed = known
                .iter()
                .filter(|k| k.state != CREATED && !listed.iter().any(|e| e.url == k.url))
                .map(|k| k.url.clone())
                .collect();
            let token = remote.map(|r| r.sync_token.clone()).unwrap_or_default();
            (listed, removed, token)
        }
    };
    // Our own writes come back in the delta with the etag we already have.
    let to_fetch: Vec<String> = changed
        .iter()
        .filter(|entry| !is_current(&known, entry))
        .map(|entry| entry.url.clone())
        .collect();
    let fetched = if to_fetch.is_empty() {
        super::Fetched::default()
    } else {
        client.fetch(&calendar.url, &to_fetch)?
    };
    removed.extend(fetched.gone);

    report.pulled += db.with(|conn| {
        // Turned off or removed while the server was being asked: what came
        // back is no longer wanted here.
        if !store::calendar(conn, &calendar.id)?.is_some_and(|c| c.enabled) {
            return Ok(0);
        }
        let mut pulled = 0;
        for object in &fetched.objects {
            pulled += apply_remote(conn, calendar, object)? as usize;
        }
        for url in &removed {
            pulled += apply_removal(conn, calendar, url)? as usize;
        }
        store::set_synced(conn, &calendar.id, remote_ctag, &token, listed)?;
        Ok(pulled)
    })?;
    Ok(())
}

fn is_current(known: &[DavObject], entry: &Entry) -> bool {
    known
        .iter()
        .any(|k| k.url == entry.url && !k.etag.is_empty() && k.etag == entry.etag)
}

/// Send one kind of this calendar's local edits. Creates the server refused
/// for their UID are added to `uid_conflicts` and kept pending.
fn push(
    db: &impl Db,
    client: &Client,
    calendar: &DavCalendar,
    pass: Pass,
    uid_conflicts: &mut HashSet<String>,
) -> Result<Pushed> {
    let pending = db.with(|conn| store::pending(conn, &calendar.id))?;
    let mut pushed = Pushed::default();
    for object in pending {
        let result = match (pass, object.state == DELETED) {
            (Pass::Deletions, true) => push_deletion(db, client, calendar, &object, uid_conflicts),
            (Pass::Writes, false) => push_write(db, client, calendar, &object, uid_conflicts),
            _ => continue,
        };
        match result {
            Ok(Sent::Done) => pushed.sent += 1,
            Ok(Sent::Conflict) => pushed.conflicts += 1,
            Ok(Sent::Waiting) => {}
            Err(err) if err.is::<AuthFailed>() => return Err(err),
            // One object the server refuses does not hold up the others.
            Err(err) => {
                pushed.error.get_or_insert(err);
            }
        }
    }
    Ok(pushed)
}

enum Sent {
    Done,
    /// The server's copy changed since: dropped here, to be fetched.
    Conflict,
    /// Not sent yet, on purpose.
    Waiting,
}

fn push_deletion(
    db: &impl Db,
    client: &Client,
    calendar: &DavCalendar,
    object: &DavObject,
    uid_conflicts: &HashSet<String>,
) -> Result<Sent> {
    // Still to be deleted? An undo since puts the event back instead.
    let Some(object) = db
        .with(|conn| store::object(conn, &object.calendar, &object.url))?
        .filter(|row| row.state == DELETED)
    else {
        return Ok(Sent::Waiting);
    };
    if !object.moved_to.is_empty() {
        // The event left this calendar for another: delete it here only once
        // it is there, or when the server will not take it there until it
        // is gone from here.
        let destination = db.with(|conn| store::object_at(conn, &object.moved_to))?;
        let landed = destination.as_ref().is_none_or(|d| d.state != CREATED);
        if !landed && !uid_conflicts.contains(&object.moved_to) {
            return Ok(Sent::Waiting);
        }
    }
    // Never delete without knowing which version: another client's edit
    // since must not go with it. Only a server that hands out no versions at
    // all is sent an unconditional DELETE.
    let etag = match object.etag.as_str() {
        "" => client.get(&object.url)?.map(|current| current.etag),
        etag => Some(etag.to_string()),
    };
    let outcome = match etag {
        // Gone already.
        None => Sent::Done,
        Some(etag) => {
            match client.delete(&object.url, Some(etag.as_str()).filter(|e| !e.is_empty())) {
                Ok(()) => Sent::Done,
                Err(err) if err.is::<PreconditionFailed>() => Sent::Conflict,
                Err(err) => return Err(err),
            }
        }
    };
    // Changed on the server since: its version comes back here.
    let server = match outcome {
        Sent::Conflict => client.get(&object.url)?,
        _ => None,
    };
    db.with(|conn| {
        match store::object(conn, &object.calendar, &object.url)? {
            // Undone while the DELETE was out: back here, and gone there
            // unless the server kept it, so it goes up again as new.
            Some(current) if current.state != DELETED && !current.local_id.is_empty() => {
                store::save_object(
                    conn,
                    &DavObject {
                        etag: String::new(),
                        state: if server.is_some() { CHANGED } else { CREATED }.into(),
                        moved_to: String::new(),
                        ..current
                    },
                )
            }
            _ => {
                store::delete_object(conn, &object.calendar, &object.url)?;
                match &server {
                    Some(copy) => apply_remote(conn, calendar, copy).map(drop),
                    None => Ok(()),
                }
            }
        }
    })?;
    Ok(outcome)
}

fn push_write(
    db: &impl Db,
    client: &Client,
    calendar: &DavCalendar,
    object: &DavObject,
    uid_conflicts: &mut HashSet<String>,
) -> Result<Sent> {
    // The queue row and its event as they are now, not as `pending` saw
    // them: the user may have deleted or moved the event since.
    let (row, event) = db.with(|conn| {
        let row = store::object(conn, &object.calendar, &object.url)?
            .filter(|r| r.state == CREATED || r.state == CHANGED);
        let event = match &row {
            Some(row) => db::event(conn, &row.local_id)?,
            None => None,
        };
        Ok((row, event))
    })?;
    let Some(row) = row else {
        return Ok(Sent::Waiting);
    };
    let Some(event) = event.filter(|e| !e.ics.is_empty()) else {
        // The event is gone without a deletion recorded; nothing to send.
        db.with(|conn| store::delete_object(conn, &row.calendar, &row.url))?;
        return Ok(Sent::Waiting);
    };
    let mut create = row.state == CREATED;
    let mut etag = row.etag.clone();
    // An update whose version is unknown (its last PUT returned none) asks
    // for it, rather than going up as a create the server must refuse.
    if !create && etag.is_empty() {
        match client.get(&row.url)? {
            Some(current) => etag = current.etag,
            None => create = true,
        }
    }
    let etag = Some(etag.as_str()).filter(|e| !e.is_empty());
    match client.put(&row.url, &event.ics, if create { None } else { etag }) {
        Ok(etag) => {
            // An empty etag (the server sent none) leaves the version to the
            // pull, which fetches the object and its version together.
            db.with(|conn| complete_write(conn, &row, &event.ics, &etag))?;
            Ok(Sent::Done)
        }
        // Server wins: its copy replaces the edit here, now.
        Err(err) if err.is::<PreconditionFailed>() => {
            take_server_copy(db, client, calendar, &row)?;
            Ok(Sent::Conflict)
        }
        Err(err) if create && err.is::<UidConflict>() => {
            uid_conflicts.insert(row.url.clone());
            Ok(Sent::Waiting)
        }
        Err(err) => Err(err),
    }
}

/// Record a PUT that went through, against the queue row as it is now.
fn complete_write(conn: &Connection, sent: &DavObject, ics: &str, etag: &str) -> Result<()> {
    let saved = match store::object(conn, &sent.calendar, &sent.url)? {
        // Deleted, or moved to another calendar, while the create was out:
        // the server has it now, so it has to go from there too, after the
        // copy it moved to has landed.
        None => DavObject {
            calendar: sent.calendar.clone(),
            url: sent.url.clone(),
            state: DELETED.into(),
            moved_to: store::object_for_event(conn, &sent.local_id)?
                .filter(|moved| moved.calendar != sent.calendar)
                .map(|moved| moved.url)
                .unwrap_or_default(),
            ..Default::default()
        },
        // Deleted, moved or edited again while the PUT was out: still to do,
        // now against the version just written.
        Some(current) if current.state != sent.state || current.local_id != sent.local_id => {
            current
        }
        Some(current) if db::event(conn, &current.local_id)?.is_some_and(|e| e.ics != ics) => {
            DavObject {
                state: CHANGED.into(),
                ..current
            }
        }
        Some(current) => DavObject {
            state: String::new(),
            ..current
        },
    };
    store::save_object(
        conn,
        &DavObject {
            etag: etag.to_string(),
            ..saved
        },
    )
}

/// The server refused a write because its copy changed: fetch that copy and
/// let it replace the one here, unless the user has changed it again since.
fn take_server_copy(
    db: &impl Db,
    client: &Client,
    calendar: &DavCalendar,
    sent: &DavObject,
) -> Result<()> {
    let server = client.get(&sent.url)?;
    db.with(|conn| {
        let Some(current) = store::object(conn, &sent.calendar, &sent.url)? else {
            return Ok(());
        };
        if current.state != sent.state || current.local_id != sent.local_id {
            return Ok(());
        }
        store::save_object(
            conn,
            &DavObject {
                etag: String::new(),
                state: String::new(),
                moved_to: String::new(),
                ..current
            },
        )?;
        match &server {
            Some(object) => apply_remote(conn, calendar, object).map(drop),
            None => apply_removal(conn, calendar, &sent.url).map(drop),
        }
    })
}

/// Take in one object from the server. Returns whether the calendar changed.
fn apply_remote(conn: &Connection, calendar: &DavCalendar, object: &super::Object) -> Result<bool> {
    let known = store::object(conn, &calendar.id, &object.url)?;
    // Edited here and not yet sent (its PUT failed): keep the edit.
    if known.as_ref().is_some_and(|k| !k.state.is_empty()) {
        return Ok(false);
    }
    let local_id = known
        .as_ref()
        .map(|k| k.local_id.clone())
        .unwrap_or_default();
    let Some(mut event) = ics::event_from_ics(&object.ics) else {
        // Not an event: remember it so it is not fetched again.
        let removed = !local_id.is_empty() && db::delete_event(conn, &local_id)?;
        store::save_object(
            conn,
            &DavObject {
                calendar: calendar.id.clone(),
                url: object.url.clone(),
                etag: object.etag.clone(),
                ..Default::default()
            },
        )?;
        return Ok(removed);
    };
    let existing = if local_id.is_empty() {
        None
    } else {
        db::event(conn, &local_id)?
    };
    event.id = existing
        .as_ref()
        .map(|e| e.id.clone())
        .unwrap_or_else(|| crate::tasks::new_id("event"));
    event.calendar = calendar.id.clone();
    // Alerts are this device's; the server's copy does not carry them.
    event.alerts = existing.as_ref().and_then(|e| e.alerts.clone());
    let unchanged = existing
        .as_ref()
        .is_some_and(|e| e.ics == event.ics && e.calendar == event.calendar);
    if !unchanged {
        db::save_event(conn, &event)?;
    }
    store::save_object(
        conn,
        &DavObject {
            calendar: calendar.id.clone(),
            url: object.url.clone(),
            etag: object.etag.clone(),
            local_id: event.id,
            ..Default::default()
        },
    )?;
    Ok(!unchanged)
}

/// The server no longer has `url`. Returns whether an event went.
fn apply_removal(conn: &Connection, calendar: &DavCalendar, url: &str) -> Result<bool> {
    let Some(known) = store::object(conn, &calendar.id, url)? else {
        return Ok(false);
    };
    // Created here and not yet sent: the server never had it.
    if known.state == CREATED {
        return Ok(false);
    }
    store::delete_object(conn, &calendar.id, url)?;
    Ok(!known.local_id.is_empty() && db::delete_event(conn, &known.local_id)?)
}
