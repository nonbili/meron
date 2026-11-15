//! Keep SQLite and its synchronous mutex off the async runtime workers.

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub(crate) async fn run<T: Send + 'static>(
    db: &Arc<Mutex<Connection>>,
    operation: &'static str,
    work: impl FnOnce(&mut Connection) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        let mut guard = meron_core::log::timed_db_lock(&db, operation);
        work(&mut guard)
    })
    .await?
}

pub(crate) async fn account_paused(db: &Arc<Mutex<Connection>>, account: &str) -> bool {
    let account = account.to_string();
    run(db, "background_sync.paused", move |db| {
        meron_core::store::account_paused(db, &account)
    })
    .await
    .unwrap_or(false)
}
