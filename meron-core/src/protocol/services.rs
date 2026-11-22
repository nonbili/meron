//! Mobile `caldav.*`, `carddav.*` and `googleTasks.*`: the services desktop
//! syncs calendars, address books and task lists with, run on the mobile
//! engine. Each change is announced with the events the desktop sidecar
//! sends, so the open screens re-read and the hosts reschedule reminders.
//!
//! Desktop's sidecar also syncs in the background after some changes (a new
//! account, a calendar switched on) and on each account's interval; on mobile
//! the app asks for those syncs itself, as it is the one that knows whether it
//! is in the foreground.

use super::*;

use crate::engine::Engine;

/// Route one service request; `None` when `method` is none of them.
pub(crate) fn dispatch_mobile_services(
    data_dir: &str,
    method: &str,
    params: &Value,
) -> Option<Result<Value, String>> {
    let run = |work: &dyn Fn(&Engine) -> anyhow::Result<Value>| {
        let engine = crate::ffi::engine_for(data_dir)?;
        work(&engine).map_err(|err| format!("{err:#}"))
    };
    let result = if method == "caldav.sync" && due(params) {
        run(&|engine| sync_due_calendars(engine))
    } else if method.starts_with("caldav.") {
        run(&|engine| crate::caldav::service::dispatch(engine, method, params))
    } else if method.starts_with("carddav.") {
        run(&|engine| crate::carddav::dispatch(engine, method, params))
    } else if method == "googleTasks.pollState" {
        return Some(with_mobile_db(data_dir, |conn| {
            google_tasks_poll_state(&conn).map_err(|err| format!("{err:#}"))
        }));
    } else if method == "googleTasks.sync" && automatic(params) {
        return Some(run(&|engine| {
            sync_google_tasks_automatically(engine, params)
        }));
    } else if method.starts_with("googleTasks.") {
        run(&|engine| crate::google_tasks::dispatch(engine, method, params))
    } else {
        return None;
    };
    if let Ok(value) = &result {
        announce(method, value);
    }
    Some(result)
}

/// Empty answers for the read-only service methods, for the stub dispatcher
/// that runs before a data dir is bound.
pub(crate) fn stub_mobile_services(method: &str) -> Option<Value> {
    Some(match method {
        "caldav.accounts" | "carddav.accounts" | "googleTasks.accounts" => {
            json!({ "accounts": [] })
        }
        "caldav.calendars" => json!({ "calendars": [] }),
        _ => return None,
    })
}

/// A background refresh's: only the accounts whose interval has run out
/// since they last synced, as the foreground poll keeps them.
fn due(params: &Value) -> bool {
    params.get("due").and_then(Value::as_bool).unwrap_or(false)
}

/// Sync the CalDAV accounts due by their own interval: one set to every
/// hour is not synced by each 15-minute background run. One that has not
/// synced cleanly since stays due, its cooldown permitting.
fn sync_due_calendars(engine: &Engine) -> anyhow::Result<Value> {
    let now = store::now_unix();
    let accounts = crate::caldav::store::accounts(
        &*engine
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?,
    )?;
    let (mut pulled, mut pushed) = (0, 0);
    // An account's failure is recorded on it and reported here too, for the
    // background hosts' logs; the others still sync.
    let mut errors = Vec::new();
    for account in accounts {
        if account.sync_interval <= 0 || account.last_sync_at + account.sync_interval * 60 > now {
            continue;
        }
        let report = crate::caldav::service::sync(engine, Some(&account.id), true)?;
        pulled += report.pulled;
        pushed += report.pushed;
        if !report.error.is_empty() {
            errors.push(report.error);
        }
    }
    Ok(json!({ "pulled": pulled, "pushed": pushed, "error": errors.join("; ") }))
}

fn automatic(params: &Value) -> bool {
    params
        .get("automatic")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// What the app's foreground poll needs to decide which Google Tasks
/// accounts are due, as desktop's poll reads it: whether Tasks is on, the
/// accounts with edits waiting to go out, and each account's interval in
/// minutes (0 for only when asked).
fn google_tasks_poll_state(conn: &rusqlite::Connection) -> anyhow::Result<Value> {
    let enabled = store::tasks_enabled(conn)?;
    let pending = crate::google_tasks::pending_emails(conn)?;
    let accounts: Vec<Value> = crate::google_tasks::sync_intervals(conn)?
        .into_iter()
        .map(|(email, minutes)| json!({ "email": email, "minutes": minutes }))
        .collect();
    Ok(json!({ "enabled": enabled, "pending": pending, "accounts": accounts }))
}

/// A sync the app starts on its own (its poll, a background refresh): none
/// while Tasks is off, and none queued behind one already running, as
/// desktop's background sync. `changed` says whether any list moved.
fn sync_google_tasks_automatically(engine: &Engine, params: &Value) -> anyhow::Result<Value> {
    let enabled = store::tasks_enabled(
        &*engine
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("store lock poisoned"))?,
    )?;
    if !enabled {
        return Ok(json!({ "changed": false }));
    }
    let email = params
        .get("email")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_ascii_lowercase);
    // Without an address (a background refresh), only the accounts a poll
    // would sync now; one set to sync only when asked is left alone.
    let changed = match email {
        Some(email) => crate::google_tasks::sync(engine, Some(&email), true)?,
        None => crate::google_tasks::sync_due(engine, store::now_unix())?,
    };
    if changed {
        crate::ffi::emit_event("tasks.changed", json!({ "method": "googleTasks.sync" }));
    }
    crate::ffi::emit_event("googleTasks.changed", json!({}));
    Ok(json!({ "changed": changed }))
}

/// Tell the app what a finished request changed, in the events desktop's
/// sidecar sends for it.
fn announce(method: &str, result: &Value) {
    // Nothing happened yet: the host is off to the browser.
    if result.get("needs_sign_in").is_some() {
        return;
    }
    let emit = |name: &str| {
        let detail = match name {
            "calendar.changed" | "tasks.changed" => json!({ "method": method }),
            _ => json!({}),
        };
        crate::ffi::emit_event(name, detail);
    };
    if method.starts_with("caldav.") {
        let events: &[&str] = match method {
            "caldav.add" | "caldav.addGoogle" | "caldav.setPassword" | "caldav.setSyncInterval" => {
                &["caldav.changed"]
            }
            "caldav.createCalendar"
            | "caldav.updateCalendar"
            | "caldav.deleteCalendar"
            | "caldav.remove" => &["caldav.changed", "calendar.changed", "tasks.changed"],
            "caldav.renameAccount" | "caldav.setCalendarEnabled" => {
                &["caldav.changed", "calendar.changed"]
            }
            "caldav.setTasksEnabled" => &["caldav.changed", "tasks.changed"],
            "caldav.sync" => {
                let moved = ["pulled", "pushed"]
                    .iter()
                    .any(|key| result.get(*key).and_then(Value::as_u64).unwrap_or(0) > 0);
                if moved {
                    &["caldav.changed", "calendar.changed", "tasks.changed"]
                } else {
                    &["caldav.changed"]
                }
            }
            _ => &[],
        };
        events.iter().for_each(|name| emit(name));
        return;
    }
    if method.starts_with("carddav.") {
        let read = matches!(
            method,
            "carddav.accounts" | "carddav.contacts" | "carddav.cardsFor" | "carddav.discoverServer"
        );
        if !read {
            emit("carddav.changed");
        }
        return;
    }
    // Google Tasks answers a change with the accounts as they now are.
    if method != "googleTasks.accounts" && result.get("accounts").is_some() {
        emit("googleTasks.changed");
        emit("tasks.changed");
    }
}

/// Send edits made here to their servers straight away, as desktop's sidecar
/// does after a change: the CalDAV accounts holding [calendars], and Google
/// Tasks when [google] has edits waiting. Runs on its own thread so the edit
/// answers at once; what the sync brings back is announced as it lands. An
/// account set to sync only when asked is still synced: the edit is asking.
pub(crate) fn push_soon(data_dir: &str, calendars: Vec<String>, google: bool) {
    if calendars.is_empty() && !google {
        return;
    }
    let data_dir = data_dir.to_string();
    std::thread::spawn(move || {
        let Ok(engine) = crate::ffi::engine_for(&data_dir) else {
            return;
        };
        let mut accounts: Vec<String> = Vec::new();
        if let Ok(conn) = engine.db.lock() {
            for calendar in &calendars {
                if let Ok(Some(calendar)) = crate::caldav::store::calendar(&conn, calendar)
                    && !accounts.contains(&calendar.account)
                {
                    accounts.push(calendar.account);
                }
            }
        }
        for account in accounts {
            let params = json!({ "account_id": account });
            match crate::caldav::service::dispatch(&engine, "caldav.sync", &params) {
                Ok(result) => announce("caldav.sync", &result),
                Err(err) => crate::mlog!(
                    crate::log::Level::Warn,
                    "caldav",
                    "push after edit failed: {err:#}"
                ),
            }
        }
        // Only the accounts with edits waiting: the others keep their interval.
        // Edits wait, queued, while Tasks is off.
        let tasks_on = engine
            .db
            .lock()
            .ok()
            .and_then(|conn| store::tasks_enabled(&conn).ok())
            .unwrap_or(false);
        if google
            && tasks_on
            && let Ok(changed) = crate::google_tasks::sync_pending(&engine)
        {
            if changed {
                crate::ffi::emit_event("tasks.changed", json!({ "method": "googleTasks.sync" }));
            }
            crate::ffi::emit_event("googleTasks.changed", json!({}));
        }
    });
}

/// After a task edit: whatever it queued for a synced list goes out now.
pub(crate) fn push_task_edits(data_dir: &str) {
    let pending = with_mobile_db(data_dir, |conn| {
        let calendars =
            crate::caldav::todo::calendars_with_pending(&conn).map_err(|err| format!("{err:#}"))?;
        let google = crate::google_tasks::pending(&conn).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "calendars": calendars, "google": google }))
    });
    let Ok(pending) = pending else {
        return;
    };
    let calendars = pending["calendars"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    push_soon(
        data_dir,
        calendars,
        pending["google"].as_bool().unwrap_or(false),
    );
}
