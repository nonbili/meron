use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meron_core::{engine::Engine, google_tasks, protocol::Request, store};
use serde_json::{Value, json};

use crate::{Writer, emit};

pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let engine = engine.clone();
    let method = req.method.clone();
    let params = req.params.clone();
    let result =
        tokio::task::spawn_blocking(move || google_tasks::dispatch(&engine, &method, &params))
            .await??;
    if req.method != "googleTasks.accounts" && result.get("accounts").is_some() {
        changed(out, &req.method).await;
    }
    Ok(result)
}

async fn changed(out: &Writer, method: &str) {
    emit(out, "googleTasks.changed", json!({})).await;
    emit(out, "tasks.changed", json!({ "method": method })).await;
}

pub(crate) fn spawn_sync(engine: Arc<Engine>, out: Writer) {
    tokio::spawn(async move {
        background_sync(engine, &out, None).await;
    });
}

/// Every background run starts here, so none goes out while Tasks is off:
/// not the poll's, nor one queued by an edit (say, an undo after switching
/// Tasks off).
async fn background_sync(engine: Arc<Engine>, out: &Writer, email: Option<String>) {
    match tokio::task::spawn_blocking(move || {
        if !store::tasks_enabled(
            &*engine
                .db
                .lock()
                .map_err(|_| anyhow::anyhow!("store lock poisoned"))?,
        )? {
            return Ok(false);
        }
        google_tasks::sync(&engine, email.as_deref(), true)
    })
    .await
    {
        Ok(Ok(true)) => changed(out, "googleTasks.sync").await,
        Ok(Ok(false)) => emit(out, "googleTasks.changed", json!({})).await,
        Ok(Err(err)) => eprintln!("meron-core: Google Tasks sync failed: {err:#}"),
        Err(err) => eprintln!("meron-core: Google Tasks sync panicked: {err}"),
    }
}

/// Sync each account at the interval of its Google calendar account (0 for
/// only when asked) and send queued edits promptly. Paused while Tasks is
/// switched off.
pub(crate) async fn poll(engine: Arc<Engine>, out: Writer) {
    tokio::time::sleep(Duration::from_secs(5)).await;
    // When this loop last synced each account, as in the calendar poll.
    let mut last_run: HashMap<String, Instant> = HashMap::new();
    loop {
        let db = engine.db.clone();
        let state = crate::sidecar::db::run(&db, "googleTasks.poll", |conn| {
            if !store::tasks_enabled(conn)? {
                return Ok(None);
            }
            Ok(Some((
                google_tasks::pending_emails(conn)?,
                google_tasks::sync_intervals(conn)?,
            )))
        })
        .await;
        if let Ok(Some((pending, accounts))) = state {
            for (email, minutes) in accounts {
                // Queued edits go out now; otherwise wait for the interval.
                let due = pending.contains(&email)
                    || (minutes > 0
                        && last_run.get(&email).is_none_or(|at| {
                            at.elapsed() >= Duration::from_secs(minutes as u64 * 60)
                        }));
                if due {
                    last_run.insert(email.clone(), Instant::now());
                    background_sync(engine.clone(), &out, Some(email)).await;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
