use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use meron_core::caldav::service;
use meron_core::engine::Engine;
use meron_core::protocol::Request;

use crate::{Writer, emit};

/// How often the poll loop looks for accounts due a sync. Each account's own
/// interval decides when it is; a sync with no changes costs one PROPFIND.
const POLL_TICK: Duration = Duration::from_secs(60);

/// Handle `caldav.*`: the accounts behind synced calendars.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let engine_for_call = engine.clone();
    let method = req.method.clone();
    let params = req.params.clone();
    let result =
        tokio::task::spawn_blocking(move || service::dispatch(&engine_for_call, &method, &params))
            .await??;

    match req.method.as_str() {
        // A new account, or a calendar turned on, has events to fetch.
        "caldav.add" | "caldav.addGoogle" | "caldav.setPassword" => {
            let account = result
                .get("account_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    req.params
                        .get("account_id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            spawn_sync(engine.clone(), out.clone(), account);
            emit(out, "caldav.changed", json!({})).await;
        }
        "caldav.createCalendar" | "caldav.updateCalendar" | "caldav.deleteCalendar" => {
            emit(out, "caldav.changed", json!({})).await;
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
        }
        "caldav.setSyncInterval" => emit(out, "caldav.changed", json!({})).await,
        // The editor's picker names calendars by their account.
        "caldav.renameAccount" => {
            emit(out, "caldav.changed", json!({})).await;
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
        }
        "caldav.setCalendarEnabled" => {
            if let Some(calendar) = req.params.get("calendar_id").and_then(Value::as_str) {
                spawn_sync_for_calendars(engine, out, &[calendar]);
            }
            emit(out, "caldav.changed", json!({})).await;
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
        }
        "caldav.remove" => {
            emit(out, "caldav.changed", json!({})).await;
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
        }
        "caldav.sync" => {
            emit(out, "caldav.changed", json!({})).await;
            if result["pulled"].as_u64().unwrap_or(0) > 0
                || result["pushed"].as_u64().unwrap_or(0) > 0
            {
                emit(out, "calendar.changed", json!({ "method": req.method })).await;
            }
        }
        _ => {}
    }
    Ok(result)
}

/// Sync the accounts these calendars (`dav_calendars.id`) belong to, each
/// once, in the background.
pub(crate) fn spawn_sync_for_calendars(engine: &Arc<Engine>, out: &Writer, calendars: &[&str]) {
    let mut accounts: Vec<String> = Vec::new();
    {
        let conn = engine.db.lock().unwrap();
        for calendar in calendars {
            if let Ok(Some(calendar)) = meron_core::caldav::store::calendar(&conn, calendar) {
                if !accounts.contains(&calendar.account) {
                    accounts.push(calendar.account);
                }
            }
        }
    }
    for account in accounts {
        spawn_sync(engine.clone(), out.clone(), Some(account));
    }
}

/// Sync in the background and tell the UI what came of it.
pub(crate) fn spawn_sync(engine: Arc<Engine>, out: Writer, account: Option<String>) {
    tokio::spawn(async move {
        let engine_for_sync = engine.clone();
        let result = tokio::task::spawn_blocking(move || {
            service::sync(&engine_for_sync, account.as_deref(), false)
        })
        .await;
        match result {
            Ok(Ok(report)) => {
                // Publish local changes made while reconciling uploads too.
                if report.pulled > 0 || report.pushed > 0 {
                    emit(&out, "calendar.changed", json!({ "method": "caldav.sync" })).await;
                }
                if !report.error.is_empty() {
                    eprintln!("meron-core: calendar sync failed: {}", report.error);
                }
            }
            Ok(Err(err)) => eprintln!("meron-core: calendar sync failed: {err:#}"),
            Err(err) => eprintln!("meron-core: calendar sync panicked: {err}"),
        }
        // Each account's last sync time and error changed either way.
        emit(&out, "caldav.changed", json!({})).await;
    });
}

/// Sync each account at its own interval (`sync_interval` minutes; 0 for
/// only when asked) for as long as the sidecar runs. Due accounts are looked
/// for every [`POLL_TICK`]; the first round runs shortly after startup.
pub(crate) async fn poll(engine: Arc<Engine>, out: Writer) {
    // Let startup reads go first.
    tokio::time::sleep(Duration::from_secs(5)).await;
    // When each account was last synced by this loop. Kept here rather than
    // taken from `last_sync_at`, which only moves on success: a failing
    // account would otherwise be retried every tick.
    let mut last_run: HashMap<String, (std::time::Instant, i64)> = HashMap::new();
    loop {
        let accounts =
            meron_core::caldav::store::accounts(&engine.db.lock().unwrap()).unwrap_or_default();
        let now = meron_core::store::now_unix();
        for account in accounts {
            if account.sync_interval <= 0 || account.cooling_down(now) {
                continue;
            }
            let interval = Duration::from_secs(account.sync_interval as u64 * 60);
            let due = last_run.get(&account.id).is_none_or(|(at, timestamp)| {
                at.elapsed() >= interval
                    || (*timestamp < account.retry_at && account.retry_at <= now)
            });
            if due {
                last_run.insert(account.id.clone(), (std::time::Instant::now(), now));
                spawn_sync(engine.clone(), out.clone(), Some(account.id));
            }
        }
        tokio::time::sleep(POLL_TICK).await;
    }
}
