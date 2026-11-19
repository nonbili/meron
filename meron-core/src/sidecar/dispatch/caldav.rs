use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

use meron_core::caldav::service;
use meron_core::engine::Engine;
use meron_core::protocol::Request;

use crate::{Writer, emit};

/// How often calendars are checked when nothing else asks. A check with no
/// changes costs one PROPFIND per account.
const POLL_INTERVAL: Duration = Duration::from_secs(15 * 60);

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
        "caldav.add" | "caldav.setPassword" => {
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
        "caldav.setCalendarEnabled" => {
            spawn_sync(engine.clone(), out.clone(), None);
            emit(out, "caldav.changed", json!({})).await;
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
        }
        "caldav.remove" => {
            emit(out, "caldav.changed", json!({})).await;
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
        }
        "caldav.sync" => {
            emit(out, "caldav.changed", json!({})).await;
            if result["pulled"].as_u64().unwrap_or(0) > 0 {
                emit(out, "calendar.changed", json!({ "method": req.method })).await;
            }
        }
        _ => {}
    }
    Ok(result)
}

/// Sync in the background and tell the UI what came of it.
pub(crate) fn spawn_sync(engine: Arc<Engine>, out: Writer, account: Option<String>) {
    tokio::spawn(async move {
        let engine_for_sync = engine.clone();
        let result = tokio::task::spawn_blocking(move || {
            service::sync(&engine_for_sync, account.as_deref())
        })
        .await;
        match result {
            Ok(Ok(report)) => {
                // What landed before a failure is shown all the same.
                if report.pulled > 0 {
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

/// Sync every account now and then for as long as the sidecar runs.
pub(crate) async fn poll(engine: Arc<Engine>, out: Writer) {
    // Let startup reads go first.
    tokio::time::sleep(Duration::from_secs(5)).await;
    loop {
        let has_accounts = meron_core::caldav::store::accounts(&engine.db.lock().unwrap())
            .is_ok_and(|accounts| !accounts.is_empty());
        if has_accounts {
            spawn_sync(engine.clone(), out.clone(), None);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
