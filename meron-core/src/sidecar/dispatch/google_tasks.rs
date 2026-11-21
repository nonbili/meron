use std::sync::Arc;
use std::time::{Duration, Instant};

use meron_core::{engine::Engine, google_tasks, protocol::Request};
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
        background_sync(engine, &out).await;
    });
}

async fn background_sync(engine: Arc<Engine>, out: &Writer) {
    match tokio::task::spawn_blocking(move || google_tasks::sync(&engine, None, true)).await {
        Ok(Ok(true)) => changed(out, "googleTasks.sync").await,
        Ok(Ok(false)) => emit(out, "googleTasks.changed", json!({})).await,
        Ok(Err(err)) => eprintln!("meron-core: Google Tasks sync failed: {err:#}"),
        Err(err) => eprintln!("meron-core: Google Tasks sync panicked: {err}"),
    }
}

pub(crate) async fn poll(engine: Arc<Engine>, out: Writer) {
    tokio::time::sleep(Duration::from_secs(5)).await;
    let interval = Duration::from_secs(15 * 60);
    let mut refreshed = None::<Instant>;
    loop {
        let db = engine.db.clone();
        let pending = crate::sidecar::db::run(&db, "googleTasks.pending", |conn| {
            google_tasks::pending(conn)
        })
        .await;
        if refreshed.is_none_or(|at| at.elapsed() >= interval) || pending.unwrap_or(false) {
            background_sync(engine.clone(), &out).await;
            refreshed = Some(Instant::now());
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
