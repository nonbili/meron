use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::{rss, store};

use crate::sidecar::idle::*;
use crate::{Writer, emit};

/// Decide whether a thread has *new* ancestor gaps worth fetching, and if so
/// run [`fill_thread_gaps`] in the background so the read it was called from
/// returns immediately. Two guards keep this cheap:
///   - the gap set is computed from the local DB (no network) before spawning;
///   - a per-thread negative cache (`Engine::gap_attempts`) drops ids we've
///     already tried this session, so re-opening a thread whose ancestors will
///     never arrive (the common case) does no network work at all.
/// When the fill actually stores something, it emits `mail.synced` so the open
/// thread re-reads; the re-read sees no new gaps and won't reconnect.
pub(crate) fn maybe_spawn_fill_thread_gaps(
    engine: &Arc<Engine>,
    out: &Writer,
    account: &str,
    thread_key: &str,
) {
    // Synthetic `uid:` keys (drafts / headerless messages) have no References to
    // chase.
    if thread_key.starts_with("uid:") {
        return;
    }

    let gaps = {
        let db = engine.db.lock().unwrap();
        match store::get_thread_reference_gaps(&db, account, thread_key) {
            Ok(gaps) => gaps,
            Err(err) => {
                eprintln!("meron-core: thread reference gaps thread_key={thread_key}: {err:#}");
                return;
            }
        }
    };
    if gaps.is_empty() {
        return;
    }

    // Keep only ids not tried yet this session; record them as tried up front so
    // a second open (or a concurrent one) before this finishes won't re-spawn.
    let cache_key = format!("{account}|{thread_key}");
    let has_new = {
        let mut attempts = engine.gap_attempts.lock().unwrap();
        let tried = attempts.entry(cache_key).or_default();
        let mut has_new = false;
        for id in &gaps {
            if tried.insert(id.clone()) {
                has_new = true;
            }
        }
        has_new
    };
    if !has_new {
        return;
    }

    let engine = engine.clone();
    let out = out.clone();
    let account = account.to_string();
    let thread_key = thread_key.to_string();
    tokio::spawn(async move {
        match fill_thread_gaps(&engine, &account, &thread_key).await {
            Ok(true) => {
                emit(
                    &out,
                    "mail.synced",
                    json!({ "account": account, "folder": "inbox", "synced": 0 }),
                )
                .await;
            }
            Ok(false) => {}
            Err(err) => {
                eprintln!("meron-core: fill thread gaps thread_key={thread_key}: {err:#}");
            }
        }
    });
}

/// Refresh a folder's messages from IMAP in the background (deduped), then emit
/// `mail.synced` so the UI re-reads the now-fresh store. Keeps network I/O off
/// the bridge's synchronous request path (which runs on the app's UI thread).
struct MessageSyncClaim {
    engine: Arc<Engine>,
    key: String,
}

impl Drop for MessageSyncClaim {
    fn drop(&mut self) {
        self.engine.syncing.lock().unwrap().remove(&self.key);
    }
}

pub(crate) fn spawn_message_sync(
    engine: Arc<Engine>,
    out: Writer,
    account: String,
    folder: String,
    limit: u32,
) {
    if engine.is_paused(&account) {
        return;
    }
    let key = format!("msg:{account}/{folder}");
    if !engine.syncing.lock().unwrap().insert(key.clone()) {
        return;
    }
    let claim = MessageSyncClaim {
        engine: engine.clone(),
        key,
    };
    tokio::spawn(async move {
        // sync_messages owns its network-only budget. Never time out its DB
        // phases: their completion must reach notification processing below.
        let result = sync_background_messages(&engine, &account, &folder, limit).await;
        drop(claim);
        match result {
            Ok(synced) => {
                // Warm full bodies for the unread/recent set now that envelopes
                // are fresh. Deduped, and a no-op once everything is cached.
                spawn_body_prefetch(engine.clone(), account.clone(), folder.clone());
                // Piggyback Sent and Drafts syncs so replies sent or drafted
                // from another client thread into conversations straight from
                // the local store (no per-thread network check on read). Runs
                // before the emit so the re-read it triggers already sees them.
                for sync in sync_companion_folders(&engine, &account, &folder, limit).await {
                    if let Err(err) = sync.result {
                        eprintln!("meron-core: sync {} {account}: {err:#}", sync.role);
                    }
                }
                let arrivals = (!synced.arrivals.is_empty()).then_some(synced.arrivals);
                if let Some(headers) = arrivals
                    && let Some(detail) =
                        new_messages_detail(&engine, &account, &folder, &headers).await
                {
                    emit(&out, "mail.newMessages", detail).await;
                    return;
                }
                emit(
                    &out,
                    "mail.synced",
                    json!({ "account": account, "folder": folder, "synced": synced.count }),
                )
                .await
            }
            Err(e) if e.is::<BackgroundSyncCancelled>() => {}
            Err(e) => {
                emit(
                    &out,
                    "mail.syncError",
                    json!({
                        "account": account,
                        "message": format!("sync {folder}: {e:#}"),
                        "outer_timeout": e.is::<BackgroundSyncTimedOut>(),
                    }),
                )
                .await
            }
        }
    });
}

/// Re-fetch an RSS account's feeds in the background (deduped, blocking pool),
/// then emit `mail.synced` so the UI re-reads the refreshed store.
pub(crate) fn spawn_rss_sync(engine: Arc<Engine>, out: Writer, account: String) {
    if engine.is_paused(&account) {
        return;
    }
    let key = format!("rss:{account}");
    if !engine.syncing.lock().unwrap().insert(key.clone()) {
        return;
    }
    tokio::spawn(async move {
        let blocking = {
            let engine = engine.clone();
            let account = account.clone();
            tokio::task::spawn_blocking(move || rss::sync_account(&engine.db, &account))
        };
        let result = tokio::time::timeout(Duration::from_secs(120), blocking).await;
        engine.syncing.lock().unwrap().remove(&key);
        match result {
            Ok(Ok(Ok(new_items))) => {
                // New feed entries: notify like fresh mail (toast + reload + OS
                // notification) instead of a silent refresh. The detail names the
                // entries that actually arrived — not the account's newest-dated
                // stored row, which is a different item whenever a feed publishes
                // with a timestamp older than something already stored.
                if let Some(detail) = rss::new_items_detail(
                    &account,
                    &account_label(&engine, &account),
                    engine.is_muted(&account),
                    &new_items,
                ) {
                    emit(&out, "mail.newMessages", detail).await
                } else {
                    emit(
                        &out,
                        "mail.synced",
                        json!({ "account": account, "folder": "inbox" }),
                    )
                    .await
                }
            }
            Ok(Ok(Err(e))) => {
                emit(
                    &out,
                    "error",
                    json!({ "message": format!("rss sync: {e:#}") }),
                )
                .await
            }
            Ok(Err(e)) => {
                emit(
                    &out,
                    "error",
                    json!({ "message": format!("rss sync task: {e}") }),
                )
                .await
            }
            Err(_) => emit(&out, "error", json!({ "message": "rss sync timed out" })).await,
        }
    });
}

pub(crate) fn spawn_folder_sync(engine: Arc<Engine>, out: Writer, account: String) {
    if engine.is_paused(&account) {
        return;
    }
    let key = format!("folders:{account}");
    if !engine.syncing.lock().unwrap().insert(key.clone()) {
        return;
    }
    tokio::spawn(async move {
        let result = retry_background_sync(
            &format!("folders sync for {account}"),
            || !engine.is_paused(&account),
            || sync_folders(&engine, &account),
        )
        .await;
        engine.syncing.lock().unwrap().remove(&key);
        match result {
            Ok(_) => {
                emit(
                    &out,
                    "mail.synced",
                    json!({ "account": account, "folders": true }),
                )
                .await
            }
            Err(e) if e.is::<BackgroundSyncCancelled>() => {}
            Err(e) => {
                emit(
                    &out,
                    "mail.syncError",
                    json!({
                        "account": account,
                        "message": format!("folders sync: {e:#}"),
                        "outer_timeout": e.is::<BackgroundSyncTimedOut>(),
                    }),
                )
                .await
            }
        }
    });
}
