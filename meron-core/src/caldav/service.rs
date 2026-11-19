//! CalDAV accounts as both apps use them: add one (discovery doubles as the
//! password check), list them with their calendars, pick which calendars to
//! show, remove one, and sync.
//!
//! Every call here may block on the network; the sidecar runs them on a
//! blocking thread and mobile calls them off the main thread already.

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use super::Client;
use super::store::{self, DavAccount};
use super::sync::{self, Report};
use crate::engine::Engine;
use crate::secrets::Secrets;

/// Run one `caldav.*` request: the vocabulary desktop and mobile share.
pub fn dispatch(engine: &Engine, method: &str, params: &Value) -> Result<Value> {
    let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or_default();
    let required = |key: &str| match text(key) {
        "" => Err(anyhow!("missing {key}")),
        value => Ok(value),
    };
    let flag = |key: &str| params.get(key).and_then(Value::as_bool);
    let conn = || engine.db.lock().map_err(|_| anyhow!("store lock poisoned"));
    match method {
        "caldav.accounts" => Ok(json!({ "accounts": accounts_payload(&*conn()?)? })),
        "caldav.calendars" => calendars_payload(&*conn()?),
        "caldav.add" => add_account(
            engine,
            &NewAccount {
                server_url: text("server_url"),
                username: text("username"),
                password: text("password"),
                display_name: text("display_name"),
            },
        ),
        "caldav.setPassword" => set_password(engine, required("account_id")?, text("password")),
        "caldav.remove" => remove_account(
            engine,
            required("account_id")?,
            flag("discard_unsent").unwrap_or(false),
        ),
        "caldav.setCalendarEnabled" => set_calendar_enabled(
            engine,
            required("calendar_id")?,
            flag("enabled").unwrap_or(true),
            flag("discard_unsent").unwrap_or(false),
        ),
        "caldav.sync" => {
            let account = params.get("account_id").and_then(Value::as_str);
            let report = sync(engine, account)?;
            Ok(json!({
                "pulled": report.pulled,
                "pushed": report.pushed,
                "error": report.error,
            }))
        }
        other => Err(anyhow!("unknown method: {other}")),
    }
}

pub struct NewAccount<'a> {
    pub server_url: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub display_name: &'a str,
}

fn lock(engine: &Engine) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    engine.db.lock().map_err(|_| anyhow!("store lock poisoned"))
}

/// Connect, find the calendars, and keep the account. Fails without keeping
/// anything when the server, the username or the password is wrong. Adding
/// an account that is already here (a retry after the first attempt timed
/// out in the UI) updates its password instead of adding it twice.
pub fn add_account(engine: &Engine, new: &NewAccount<'_>) -> Result<Value> {
    let username = new.username.trim();
    if username.is_empty() || new.password.is_empty() {
        bail!("a username and password are required");
    }
    let server = super::parse_server_url(new.server_url)?;
    let same_account = |conn: &rusqlite::Connection| -> Result<Option<DavAccount>> {
        Ok(store::accounts(conn)?.into_iter().find(|a| {
            a.username == username
                && super::parse_server_url(&a.server_url).is_ok_and(|known| known == server)
        }))
    };
    if let Some(existing) = same_account(&*lock(engine)?)? {
        let mut answer = set_password(engine, &existing.id, new.password)?;
        answer["account_id"] = json!(existing.id);
        return Ok(answer);
    }
    let client = client(server.as_str(), username, new.password)?;
    let found = client.discover()?;
    let id = crate::tasks::new_id("dav");
    let display_name = match new.display_name.trim() {
        "" => server.host_str().unwrap_or(username).to_string(),
        name => name.to_string(),
    };
    let id = {
        let conn = lock(engine)?;
        // Another attempt finished first while this one was discovering:
        // keep that account, with the password just checked.
        let id = match same_account(&conn)? {
            Some(existing) => existing.id,
            None => id,
        };
        engine.host.store_secret(
            &conn,
            &id,
            &Secrets {
                password: new.password.to_string(),
                ..Default::default()
            },
        )?;
        if store::account(&conn, &id)?.is_none() {
            store::save_account(
                &conn,
                &DavAccount {
                    id: id.clone(),
                    server_url: new.server_url.trim().to_string(),
                    username: username.to_string(),
                    display_name,
                    home: found.home.clone(),
                    ..Default::default()
                },
            )?;
            store::reconcile_calendars(&conn, &id, &found.calendars)?;
        }
        id
    };
    remember_password(engine, &id, new.password);
    Ok(json!({ "account_id": id, "accounts": accounts_payload(&*lock(engine)?)? }))
}

/// Replace an account's password, after checking the server takes it.
pub fn set_password(engine: &Engine, account_id: &str, password: &str) -> Result<Value> {
    let account = store::account(&*lock(engine)?, account_id)?
        .ok_or_else(|| anyhow!("no such CalDAV account: {account_id}"))?;
    let client = client(&account.server_url, &account.username, password)?;
    match account.home.as_str() {
        "" => client.discover().map(drop)?,
        home => client.calendars(home).map(drop)?,
    }
    let conn = lock(engine)?;
    engine.host.store_secret(
        &conn,
        account_id,
        &Secrets {
            password: password.to_string(),
            ..Default::default()
        },
    )?;
    store::record_sync(&conn, account_id, "")?;
    remember_password(engine, account_id, password);
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

/// Edits made here that have not reached the server yet go when the
/// calendar is turned off or its account removed. Try to send them first;
/// what still cannot be sent is only thrown away when the caller says so.
/// Returns how many are left unsent.
/// The caller checks again, under the lock it then deletes with, so an edit
/// made meanwhile is not lost either.
fn flush_unsent(engine: &Engine, account_id: &str, calendar_id: Option<&str>, discard: bool) {
    let unsent = || -> Result<i64> { store::unsent(&*lock(engine)?, account_id, calendar_id) };
    if !discard && unsent().is_ok_and(|n| n > 0) {
        let _ = sync(engine, Some(account_id));
    }
}

pub fn remove_account(engine: &Engine, account_id: &str, discard_unsent: bool) -> Result<Value> {
    flush_unsent(engine, account_id, None, discard_unsent);
    // No sync of the account may be halfway through: what it brought back
    // would land on an account that is gone.
    let _sync = SyncGuard::claim(engine, account_id)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    let conn = lock(engine)?;
    let unsent = store::unsent(&conn, account_id, None)?;
    if unsent > 0 && !discard_unsent {
        return Ok(json!({ "unsent": unsent, "accounts": accounts_payload(&conn)? }));
    }
    store::delete_account(&conn, account_id)?;
    engine.host.delete_secret(&conn, account_id)?;
    if let Ok(mut passwords) = engine.dav_passwords.lock() {
        passwords.remove(account_id);
    }
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

pub fn set_calendar_enabled(
    engine: &Engine,
    calendar_id: &str,
    enabled: bool,
    discard_unsent: bool,
) -> Result<Value> {
    let calendar = store::calendar(&*lock(engine)?, calendar_id)?
        .ok_or_else(|| anyhow!("no such calendar: {calendar_id}"))?;
    if !enabled {
        flush_unsent(engine, &calendar.account, Some(calendar_id), discard_unsent);
    }
    // Nor halfway through for a calendar being turned off: an upload that
    // finished after would be taken for a deletion to send.
    let _sync = SyncGuard::claim(engine, &calendar.account)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    let conn = lock(engine)?;
    if !enabled && !discard_unsent {
        let unsent = store::unsent(&conn, &calendar.account, Some(calendar_id))?;
        if unsent > 0 {
            return Ok(json!({ "unsent": unsent, "accounts": accounts_payload(&conn)? }));
        }
    }
    store::set_enabled(&conn, calendar_id, enabled)?;
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

/// Sync one account, or all of them. An account already syncing is waited
/// for and then synced again, so what was changed before this call is sent.
/// The report counts what every account did and carries the first failure.
pub fn sync(engine: &Engine, account_id: Option<&str>) -> Result<Report> {
    let accounts = store::accounts(&*lock(engine)?)?;
    let mut total = Report::default();
    for account in accounts
        .iter()
        .filter(|a| account_id.is_none_or(|id| a.id == id))
    {
        let report = match SyncGuard::claim(engine, &account.id) {
            None => Report {
                error: "a sync of this account is still running".into(),
                ..Default::default()
            },
            Some(_guard) => match password(engine, &account.id)
                .and_then(|password| client(&account.server_url, &account.username, &password))
            {
                Ok(client) => sync::sync_account(&*engine.db, &client, &account.id),
                Err(err) => {
                    let error = format!("{err:#}");
                    let _ = store::record_sync(&*lock(engine)?, &account.id, &error);
                    Report {
                        error,
                        ..Default::default()
                    }
                }
            },
        };
        total.pulled += report.pulled;
        total.pushed += report.pushed;
        if total.error.is_empty() && !report.error.is_empty() {
            total.error = format!("{}: {}", account.display_name, report.error);
        }
    }
    Ok(total)
}

/// Every account with its calendars, for settings.
pub fn accounts_payload(conn: &rusqlite::Connection) -> Result<Value> {
    let mut out = Vec::new();
    for account in store::accounts(conn)? {
        let calendars: Vec<Value> = store::calendars(conn, &account.id)?
            .iter()
            .map(|calendar| {
                json!({
                    "id": calendar.id,
                    "name": calendar.name,
                    "color": calendar.color,
                    "read_only": calendar.read_only,
                    "enabled": calendar.enabled,
                    "events": calendar.supports("VEVENT"),
                    "tasks": calendar.supports("VTODO"),
                })
            })
            .collect();
        out.push(json!({
            "id": account.id,
            "server_url": account.server_url,
            "username": account.username,
            "display_name": account.display_name,
            "last_sync_at": account.last_sync_at,
            "last_error": account.last_error,
            "calendars": calendars,
        }));
    }
    Ok(json!(out))
}

/// The calendars events can be shown from and put in: every enabled event
/// calendar, with `writable` for the editor's picker and `color` for the
/// view.
pub fn calendars_payload(conn: &rusqlite::Connection) -> Result<Value> {
    let accounts = store::accounts(conn)?;
    let calendars: Vec<Value> = store::all_calendars(conn)?
        .iter()
        .filter(|c| c.enabled && c.supports("VEVENT"))
        .map(|calendar| {
            let account = accounts.iter().find(|a| a.id == calendar.account);
            json!({
                "id": calendar.id,
                "name": calendar.name,
                "color": calendar.color,
                "writable": !calendar.read_only,
                "account_id": calendar.account,
                "account_name": account.map(|a| a.display_name.as_str()).unwrap_or_default(),
            })
        })
        .collect();
    Ok(json!({ "calendars": calendars }))
}

fn client(server: &str, username: &str, password: &str) -> Result<Client> {
    Client::new(server, username, password, crate::proxy::global().as_ref())
}

fn remember_password(engine: &Engine, account_id: &str, password: &str) {
    if let Ok(mut passwords) = engine.dav_passwords.lock() {
        passwords.insert(account_id.to_string(), password.to_string());
    }
}

fn password(engine: &Engine, account_id: &str) -> Result<String> {
    if let Some(password) = engine
        .dav_passwords
        .lock()
        .ok()
        .and_then(|p| p.get(account_id).cloned())
    {
        return Ok(password);
    }
    let stored = {
        let conn = engine
            .db
            .lock()
            .map_err(|_| anyhow!("store lock poisoned"))?;
        engine.host.load_secret(&conn, account_id).password
    };
    if stored.is_empty() {
        bail!("no password stored for this calendar account; enter it again");
    }
    remember_password(engine, account_id, &stored);
    Ok(stored)
}

struct SyncGuard<'a> {
    engine: &'a Engine,
    account: String,
}

impl<'a> SyncGuard<'a> {
    /// Wait for any sync of `account` in flight, then claim it. `None` when
    /// it is still running after the longest a sync should take; only a
    /// claimed guard releases the account.
    fn claim(engine: &'a Engine, account: &str) -> Option<Self> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        loop {
            let claimed = engine.dav_syncing.lock().ok()?.insert(account.to_string());
            if claimed {
                return Some(Self {
                    engine,
                    account: account.to_string(),
                });
            }
            if std::time::Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

impl Drop for SyncGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut syncing) = self.engine.dav_syncing.lock() {
            syncing.remove(&self.account);
        }
    }
}
