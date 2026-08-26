//! meron-core: the Meron core engine sidecar (mail, RSS, storage).
//!
//! Speaks a line-delimited JSON protocol over stdio so the desktop bridge can drive
//! it as a single long-lived process. Three message shapes, one JSON object per line:
//!
//!   request   (bridge -> sidecar):  {"id":<u64>,"method":<str>,"params":<json>}
//!   response  (sidecar -> bridge):  {"id":<u64>,"result":<json>}
//!                              or:  {"id":<u64>,"error":{"message":<str>}}
//!   event     (sidecar -> bridge):  {"event":<str>,"detail":<json>}   (no id)
//!
//! Events carry IMAP IDLE notifications to the UI (the bridge distinguishes them
//! by the absent `id`). The request path reuses warm IMAP sessions via a
//! per-account connection pool (see `Engine::with_session`); IDLE watchers hold
//! their own dedicated long-lived connections.

use anyhow::Context as _;
use serde_json::{Value, json};
use std::future::Future;
use std::io::Write as _;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdout};
use tokio::sync::Mutex;

// The binary shares the library crate's modules (rather than recompiling its own
// copies) so the desktop Engine and the mobile FFI operate on identical types.
use meron_core::engine::*;
use meron_core::engine::{Engine, EngineHost};
use meron_core::protocol::{Request, ping_response, ready_event};
use meron_core::{
    backup, changelog, imap, mail_model, parse, proxy, rss, secrets, smtp, store, thread_list,
    thread_read, unified,
};

/// Shared, serialized writer so responses and events never interleave on stdout.
type Writer = Arc<Mutex<Stdout>>;

const BACKGROUND_SYNC_RETRY_DELAY: Duration = if cfg!(test) {
    Duration::ZERO
} else {
    Duration::from_secs(2)
};

fn is_transient_io_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::UnexpectedEof
    )
}

fn is_transient_sync_error(error: &anyhow::Error) -> bool {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<std::io::Error>()
            && is_transient_io_error(io_error)
        {
            return true;
        }
        if let Some(imap_error) = cause.downcast_ref::<async_imap::error::Error>() {
            match imap_error {
                async_imap::error::Error::ConnectionLost => return true,
                async_imap::error::Error::Io(io_error) if is_transient_io_error(io_error) => {
                    return true;
                }
                _ => {}
            }
        }
    }

    // Tokio timeouts and SOCKS reply codes are repository-generated anyhow
    // messages rather than typed sources. Keep this fallback narrow; protocol,
    // authentication, and certificate errors must not retry.
    let message = format!("{error:#}").to_ascii_lowercase();
    [
        "tcp connect",
        "dial tcp",
        "dns lookup",
        "timed out",
        "timeout",
        "network is unreachable",
        "network unreachable",
        "host unreachable",
        "connection refused",
        "connection reset",
        "connection abort",
        "connection closed",
        "connection lost",
        "broken pipe",
        "failed to lookup address",
        "no address associated with hostname",
        "server closed before greeting",
        "unexpected eof",
        "bytes remaining in stream",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

#[derive(Debug)]
struct BackgroundSyncCancelled;

impl std::fmt::Display for BackgroundSyncCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("background sync cancelled")
    }
}

impl std::error::Error for BackgroundSyncCancelled {}

#[derive(Debug)]
struct BackgroundSyncTimedOut(u64);

impl std::fmt::Display for BackgroundSyncTimedOut {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "timed out after {}s", self.0)
    }
}

impl std::error::Error for BackgroundSyncTimedOut {}

/// Retry a background read once when it fails for a recognizable transport
/// reason. Both attempts share the configured per-folder ceiling so a stuck
/// sync does not hold its dedup key indefinitely.
async fn retry_background_sync<T, C, F, Fut>(
    label: &str,
    mut can_attempt: C,
    mut operation: F,
) -> anyhow::Result<T>
where
    C: FnMut() -> bool,
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    if !can_attempt() {
        return Err(anyhow::Error::new(BackgroundSyncCancelled));
    }
    let sync_timeout = background_sync_timeout();
    let deadline = tokio::time::Instant::now() + sync_timeout;
    let first = tokio::time::timeout_at(deadline, operation()).await;
    let first_error = match first {
        Ok(Ok(value)) => return Ok(value),
        Ok(Err(error)) if is_transient_sync_error(&error) => error,
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            return Err(anyhow::Error::new(BackgroundSyncTimedOut(
                sync_timeout.as_secs(),
            )));
        }
    };

    eprintln!("meron-core: {label} failed, retrying: {first_error:#}");
    if tokio::time::timeout_at(deadline, tokio::time::sleep(BACKGROUND_SYNC_RETRY_DELAY))
        .await
        .is_err()
    {
        // Keep the first attempt's cause as context: the budget expiring says
        // only that time ran out, and without this the caller (and the log) get
        // a bare "timed out" with no diagnosis. `Error::is` still finds the
        // typed error through the context chain, so the outer_timeout
        // classification is unaffected.
        return Err(
            anyhow::Error::new(BackgroundSyncTimedOut(sync_timeout.as_secs()))
                .context(format!("first attempt failed: {first_error:#}")),
        );
    }
    if !can_attempt() {
        return Err(anyhow::Error::new(BackgroundSyncCancelled));
    }

    match tokio::time::timeout_at(deadline, operation()).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(retry_error)) => Err(anyhow::anyhow!(
            "retry failed: {retry_error:#}; first attempt: {first_error:#}"
        )),
        Err(_) => Err(
            anyhow::Error::new(BackgroundSyncTimedOut(sync_timeout.as_secs()))
                .context(format!("first attempt failed: {first_error:#}")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{BackgroundSyncCancelled, is_transient_sync_error, retry_background_sync};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn imap_disconnect_errors_are_transient() {
        let lost = anyhow::Error::new(async_imap::error::Error::ConnectionLost);
        assert!(is_transient_sync_error(&lost));

        let buffered_eof = anyhow::Error::new(async_imap::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "bytes remaining in stream",
        )));
        assert!(is_transient_sync_error(&buffered_eof));

        let rejected = anyhow::Error::new(async_imap::error::Error::No(
            "[AUTHENTICATIONFAILED] invalid credentials".to_string(),
        ));
        assert!(!is_transient_sync_error(&rejected));
    }

    #[test]
    fn socks_host_unreachable_is_transient() {
        let error = anyhow::anyhow!("SOCKS5 connect failed: host unreachable");
        assert!(is_transient_sync_error(&error));
    }

    #[tokio::test]
    async fn background_sync_recovers_from_one_transient_failure() {
        let attempts = AtomicUsize::new(0);

        let result = retry_background_sync(
            "test sync",
            || true,
            || {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        anyhow::bail!("network is unreachable")
                    }
                    Ok("synced")
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "synced");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn background_sync_reports_failure_after_retry() {
        let attempts = AtomicUsize::new(0);

        let error = retry_background_sync::<(), _, _, _>(
            "test sync",
            || true,
            || {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        anyhow::bail!("connection reset by peer")
                    }
                    anyhow::bail!("network is unreachable")
                }
            },
        )
        .await
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("network is unreachable"));
        assert!(message.contains("connection reset by peer"));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn background_sync_does_not_retry_permanent_failure() {
        let attempts = AtomicUsize::new(0);

        let error = retry_background_sync(
            "test sync",
            || true,
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                async {
                    anyhow::Result::<()>::Err(anyhow::anyhow!(
                        "login failed: authentication failed"
                    ))
                }
            },
        )
        .await
        .unwrap_err();

        assert_eq!(error.to_string(), "login failed: authentication failed");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn background_sync_does_not_retry_after_pause() {
        let paused = AtomicBool::new(false);
        let attempts = AtomicUsize::new(0);

        let error = retry_background_sync::<(), _, _, _>(
            "test sync",
            || !paused.load(Ordering::SeqCst),
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                paused.store(true, Ordering::SeqCst);
                async { anyhow::Result::<()>::Err(anyhow::anyhow!("connection lost")) }
            },
        )
        .await
        .unwrap_err();

        assert!(error.is::<BackgroundSyncCancelled>());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}

/// Desktop host integration for the shared [`Engine`]: the default on-disk store
/// plus OS-keychain secret storage (with one-time migration of any legacy
/// secrets that older builds wrote into SQLite).
struct DesktopHost;

impl EngineHost for DesktopHost {
    fn open_db(&self) -> anyhow::Result<rusqlite::Connection> {
        store::open()
    }

    fn apply_secret(&self, conn: &rusqlite::Connection, account: &str, creds: &mut imap::Creds) {
        let stored = match secrets::load(account) {
            Ok(stored) => stored,
            Err(err) => {
                eprintln!("meron-core: could not load keychain secret for {account}: {err:#}");
                secrets::Secrets::default()
            }
        };
        if stored.is_empty() {
            // Legacy row from a build that stored secrets in SQLite: migrate
            // whatever's there into the keychain, then scrub the plaintext. The
            // in-memory `creds` already carry the DB-loaded secret, so they stay
            // usable after the scrub.
            let from_db = secrets::Secrets::from_creds(creds);
            if !from_db.is_empty() {
                let _ = secrets::store(account, &from_db);
                let _ = store::scrub_account_secrets(conn, account);
            }
        } else {
            stored.apply_to(creds);
        }
    }

    fn store_secret(
        &self,
        _conn: &rusqlite::Connection,
        account: &str,
        secrets: &secrets::Secrets,
    ) -> anyhow::Result<()> {
        secrets::store(account, secrets)
    }
}

/// Decide whether a thread has *new* ancestor gaps worth fetching, and if so
/// run [`fill_thread_gaps`] in the background so the read it was called from
/// returns immediately. Two guards keep this cheap:
///   - the gap set is computed from the local DB (no network) before spawning;
///   - a per-thread negative cache (`Engine::gap_attempts`) drops ids we've
///     already tried this session, so re-opening a thread whose ancestors will
///     never arrive (the common case) does no network work at all.
/// When the fill actually stores something, it emits `mail.synced` so the open
/// thread re-reads; the re-read sees no new gaps and won't reconnect.
fn maybe_spawn_fill_thread_gaps(
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
fn spawn_message_sync(
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
    tokio::spawn(async move {
        let uid_next_before = if folder.eq_ignore_ascii_case("INBOX") {
            inbox_uid_next(&engine, &account)
        } else {
            0
        };
        let result = retry_background_sync(
            &format!("sync {folder} for {account}"),
            || !engine.is_paused(&account),
            || sync_messages(&engine, &account, &folder, limit),
        )
        .await;
        engine.syncing.lock().unwrap().remove(&key);
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
                let uid_next_after = if folder.eq_ignore_ascii_case("INBOX") {
                    inbox_uid_next(&engine, &account)
                } else {
                    0
                };
                let new_inbox = new_unread_inbox_messages(
                    &engine,
                    &account,
                    uid_next_before,
                    uid_next_after,
                    &synced.messages,
                );
                if let Some(headers) = new_inbox
                    && let Some(detail) = new_messages_detail(&engine, &account, &headers).await
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
fn spawn_rss_sync(engine: Arc<Engine>, out: Writer, account: String) {
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

fn spawn_folder_sync(engine: Arc<Engine>, out: Writer, account: String) {
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

const IDLE_LIMIT: u32 = 50;

/// Unread messages in the UID range that appeared during the last sync.
/// Startup syncs can advance UIDNEXT for messages that were already read on the
/// server; those should refresh the UI without raising a desktop notification.
fn new_unread_inbox_messages(
    engine: &Arc<Engine>,
    account: &str,
    uid_next_before: u32,
    uid_next_after: u32,
    synced_messages: &[imap::MessageHeader],
) -> Option<Vec<imap::MessageHeader>> {
    let db = engine.db.lock().unwrap();
    store::new_unread_inbox_messages(
        &db,
        account,
        uid_next_before,
        uid_next_after,
        synced_messages,
    )
    .ok()
    .flatten()
}

/// Longest a notification waits on the body fetch its snippets need. Past this
/// the event goes out with whatever bodies are cached: a late notification is
/// worse than one showing subjects alone, and the general prefetch fills the
/// rest in anyway.
const NOTIFY_PREVIEW_TIMEOUT: Duration = Duration::from_secs(8);

/// `mail.newMessages` detail for a batch of arrivals, with the arrivals' bodies
/// fetched first so the notification can show the mail itself.
async fn new_messages_detail(
    engine: &Arc<Engine>,
    account: &str,
    headers: &[imap::MessageHeader],
) -> Option<Value> {
    let uids: Vec<u32> = headers
        .iter()
        .take(mail_model::NEW_MESSAGES_DETAIL_MAX)
        .map(|header| header.uid)
        .collect();
    let fetch = fetch_bodies_for_uids(engine, account, "INBOX", &uids, parse::media_root());
    match tokio::time::timeout(NOTIFY_PREVIEW_TIMEOUT, fetch).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => eprintln!("meron-core: notification bodies for {account}: {err:#}"),
        Err(_) => eprintln!("meron-core: notification bodies for {account}: timed out"),
    }
    let account_name = account_label(engine, account);
    let muted = engine.is_muted(account);
    let db = engine.db.lock().unwrap();
    mail_model::new_messages_detail(&db, account, &account_name, muted, headers)
}

/// Friendly display name or email address of an account for user-facing notifications.
fn account_label(engine: &Arc<Engine>, account: &str) -> String {
    let db = engine.db.lock().unwrap();
    store::account_label(&db, account)
}

/// Cached UIDNEXT for an account's INBOX (0 if unknown). Used to detect whether
/// an IDLE wake brought new mail (UIDNEXT advanced) or only a flag change.
fn inbox_uid_next(engine: &Arc<Engine>, account: &str) -> u32 {
    let db = engine.db.lock().unwrap();
    store::get_folder_state(&db, account, "INBOX")
        .ok()
        .flatten()
        .map(|(_, uid_next)| uid_next)
        .unwrap_or(0)
}

fn watch_key(account: &str, folder: &str) -> String {
    format!("{account}\n{folder}")
}

fn start_idle_watch(engine: Arc<Engine>, out: Writer, account: String, folder: String) -> bool {
    let key = watch_key(&account, &folder);
    {
        let mut watched = engine.watched.lock().unwrap();
        if watched.contains(&key) {
            return false;
        }
        watched.insert(key);
    }
    tokio::spawn(idle_watch(engine, out, account, folder));
    true
}

/// Long-lived per-account/folder IDLE watcher. Reconnects with backoff on error
/// so a dropped connection or server timeout resumes pushing updates.
async fn idle_watch(engine: Arc<Engine>, out: Writer, account: String, folder: String) {
    let key = watch_key(&account, &folder);
    loop {
        // Stop cleanly once the account has been removed (account.remove).
        if !engine.accounts.lock().await.contains_key(&account) {
            engine.watched.lock().unwrap().remove(&key);
            break;
        }
        // Stop checking while paused; account.setPaused respawns us on resume.
        if engine.is_paused(&account) {
            engine.watched.lock().unwrap().remove(&key);
            break;
        }
        if !engine.watched.lock().unwrap().contains(&key) {
            break;
        }
        if let Err(e) = idle_once(&engine, &out, &account, &folder).await {
            emit(
                &out,
                "error",
                json!({ "message": format!("idle {account}/{folder}: {e:#}") }),
            )
            .await;
            // Back off before reconnecting on error, but wake immediately on a
            // pause toggle so a just-paused account stops promptly (next
            // iteration sees is_paused). A clean return (pause or OS resume)
            // skips the backoff: pause exits at the top, resume reconnects now.
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(15)) => {}
                _ = engine.pause_signal.notified() => {}
            }
        }
    }
    emit(
        &out,
        "watch.stopped",
        json!({ "account": account, "folder": folder }),
    )
    .await;
}

/// Sync `folder` and surface the result to the UI: a "new mail" toast when
/// INBOX's UIDNEXT advanced (genuine arrivals), otherwise a silent refresh.
/// Shared by the IDLE wake path and the post-connect catch-up so both behave
/// identically.
async fn sync_and_notify(
    engine: &Arc<Engine>,
    out: &Writer,
    account: &str,
    folder: &str,
) -> anyhow::Result<()> {
    // An IDLE wake can mean new mail *or* just a flag change (e.g. a message
    // read on another device). UIDNEXT only advances for new arrivals, so
    // compare it across the refresh to tell them apart.
    let is_inbox = folder.eq_ignore_ascii_case("INBOX");
    let uid_next_before = if is_inbox {
        inbox_uid_next(engine, account)
    } else {
        0
    };
    // Refresh on a separate connection (the IDLE one stays dedicated to IDLE).
    let synced = sync_messages(engine, account, folder, IDLE_LIMIT).await?;
    let uid_next_after = if is_inbox {
        inbox_uid_next(engine, account)
    } else {
        0
    };

    let new_inbox = if is_inbox {
        new_unread_inbox_messages(
            engine,
            account,
            uid_next_before,
            uid_next_after,
            &synced.messages,
        )
    } else {
        None
    };

    if let Some(headers) = new_inbox {
        // Building the detail fetches the arrivals' own bodies (the notification
        // shows a snippet of each); warm the rest of the backlog behind it so the
        // first open of anything else is instant too.
        let detail = new_messages_detail(engine, account, &headers).await;
        spawn_body_prefetch(engine.clone(), account.to_string(), "INBOX".to_string());
        if let Some(detail) = detail {
            emit(out, "mail.newMessages", detail).await;
        }
    } else {
        if !is_inbox {
            spawn_body_prefetch(engine.clone(), account.to_string(), folder.to_string());
        }
        // Flag-only change: refresh the UI silently, no "new mail" toast.
        emit(
            out,
            "mail.synced",
            json!({ "account": account, "folder": folder, "synced": synced.count }),
        )
        .await;
    }
    Ok(())
}

/// One IDLE connection lifecycle: hold a dedicated session on one mailbox, and
/// on each server notification refresh that folder in the store.
async fn idle_once(
    engine: &Arc<Engine>,
    out: &Writer,
    account: &str,
    folder: &str,
) -> anyhow::Result<()> {
    let creds = engine.ensure_valid_creds(account).await?;
    let mut session = imap::connect(&creds).await?;
    session
        .select(folder)
        .await
        .with_context(|| format!("SELECT {folder}"))?;

    // Catch up before parking in IDLE: the server only pushes notifications for
    // mail that arrives *after* IDLE begins, so anything delivered while we were
    // disconnected (startup, error reconnect, or resume from suspend) would
    // otherwise stay invisible until the next push. Cheap because idle_once is
    // only (re)entered on a fresh connection, not on each 15-min IDLE timeout.
    sync_and_notify(engine, out, account, folder).await?;

    loop {
        let mut handle = session.idle();
        handle.init().await.context("IDLE init")?;
        enum Wake<R> {
            /// The IDLE wait completed: new data, a timeout, or an error.
            Data(R),
            /// The account was paused: return so idle_watch sees is_paused.
            Pause,
            /// The system resumed from suspend: the socket is probably dead.
            Resume,
        }
        let wake = {
            let (idle_fut, _stop) = handle.wait_with_timeout(Duration::from_secs(15 * 60));
            // Cancel the wait early on a pause (so idle_watch shuts the watcher
            // down) or an OS resume (so we drop a likely-dead socket and
            // reconnect) instead of blocking up to the IDLE timeout.
            tokio::select! {
                r = idle_fut => Wake::Data(r),
                _ = engine.pause_signal.notified() => Wake::Pause,
                _ = engine.resume_signal.notified() => Wake::Resume,
            }
        };

        // On resume the connection likely died during suspend, and a graceful
        // DONE could block on it until TCP keepalive times out. Drop the handle
        // (closing the socket) without DONE; idle_watch reconnects immediately.
        if let Wake::Resume = wake {
            drop(handle);
            return Ok(());
        }

        session = handle.done().await.context("IDLE done")?;
        let response = match wake {
            Wake::Data(r) => r,
            Wake::Pause => return Ok(()),
            Wake::Resume => unreachable!("handled above"),
        };

        if let async_imap::extensions::idle::IdleResponse::NewData(_) = response.context("IDLE")? {
            sync_and_notify(engine, out, account, folder).await?;
        }
    }
}

#[tokio::main]
async fn main() {
    // Tag panics consistently on stderr, which the desktop bridge copies into
    // meron.log; a panic in a worker task would otherwise be easy to miss.
    meron_core::log::install_panic_hook();
    let out: Writer = Arc::new(Mutex::new(tokio::io::stdout()));
    let engine = match Engine::new(Box::new(DesktopHost)) {
        Ok(engine) => Arc::new(engine),
        Err(e) => {
            // Storage is unusable (an unreachable keychain, a store encrypted
            // with a key we no longer hold). Keep serving stdin anyway: exiting
            // here left the bridge writing into a dead pipe, so every request
            // died of its own timeout and the UI could only report the engine
            // as generically unavailable. Answering each one with the real
            // reason is what makes the failure diagnosable.
            let message = format!("store init: {e:#}");
            emit(&out, "core.fatal", json!({ "message": message })).await;
            run_degraded(&out, &message).await;
            return;
        }
    };

    // Resume IDLE for accounts whose credentials persisted across restarts.
    let known: Vec<String> = engine.accounts.lock().await.keys().cloned().collect();
    for account in known {
        // Paused accounts skip auto-resume; account.setPaused starts them on resume.
        if engine.is_paused(&account) {
            continue;
        }
        // Warm the INBOX backlog (unread + recent) so it's readable offline and
        // opens instantly, without waiting for the UI to request the folder.
        spawn_body_prefetch(engine.clone(), account.clone(), "INBOX".to_string());
        start_idle_watch(engine.clone(), out.clone(), account, "INBOX".to_string());
    }

    emit(&out, "ready", ready_event()).await;

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) | Err(_) => break, // stdin closed: bridge is gone, exit.
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Request>(line) {
            // Handle each request on its own task so a slow IMAP call (sync,
            // thread read) can't block the read loop and stall unrelated
            // requests like account.connect behind it.
            Ok(req) => {
                let engine = engine.clone();
                let out = out.clone();
                tokio::spawn(async move { handle(engine, req, &out).await });
            }
            Err(e) => {
                emit(
                    &out,
                    "error",
                    json!({ "message": format!("bad request: {e}") }),
                )
                .await
            }
        }
    }
}

/// Serve stdin without an engine: answer `ping` (so the bridge can still tell a
/// live process from a dead one) and fail everything else with `reason`. Runs
/// until the bridge closes stdin.
async fn run_degraded(out: &Writer, reason: &str) {
    eprintln!("meron-core: running degraded, storage unavailable: {reason}");
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Request>(line) else {
            continue;
        };
        if req.method == "ping" {
            respond(out, req.id, ping_response()).await;
        } else {
            respond_error(out, req.id, reason).await;
        }
    }
}

async fn handle(engine: Arc<Engine>, req: Request, out: &Writer) {
    match dispatch(&engine, &req, out).await {
        Ok(value) => respond(out, req.id, value).await,
        Err(e) => {
            // Surface failures on stderr (inherited by the app) so swallowed
            // RPC errors are diagnosable.
            eprintln!("meron-core: {} failed: {e:#}", req.method);
            respond_error(out, req.id, &format!("{e:#}")).await;
        }
    }
}

async fn dispatch(engine: &Arc<Engine>, req: &Request, out: &Writer) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        "ping" => Ok(ping_response()),

        // Fetch the in-app changelog from the GitHub releases atom feed. The
        // network call runs on the blocking pool.
        "changelog.fetch" => {
            let variant = changelog::Variant::parse(
                p.get("variant")
                    .and_then(Value::as_str)
                    .unwrap_or("desktop"),
            );
            let releases = tokio::task::spawn_blocking(move || changelog::fetch(variant)).await??;
            Ok(releases)
        }

        "app.prefsGet" => {
            let keys = p
                .get("keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let prefs = store::settings_get(&engine.db.lock().unwrap(), &keys)?;
            Ok(json!({ "prefs": prefs }))
        }

        "app.prefsSet" => {
            let key = req_str(p, "key")?;
            let value = p.get("value").cloned().unwrap_or(Value::Null);
            store::setting_set(&engine.db.lock().unwrap(), &key, &value)?;
            // The proxy lives in a process-global slot that socket code reads
            // without a DB handle, so republish it as soon as it changes.
            if key == proxy::SETTING_KEY {
                proxy::set_global(proxy::parse_global(&value));
            }
            Ok(json!({ "ok": true }))
        }

        // Serialize accounts, prefs, feeds and settings to a backup document
        // (see `backup`). The passphrase-based key derivation is deliberately
        // slow, so this runs on the blocking pool rather than the reactor.
        "backup.export" => {
            let include_secrets = p
                .get("include_secrets")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let passphrase = p
                .get("passphrase")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            // The Go host owns the product version; core only knows its crate's.
            // The platform goes with it because desktop and mobile version
            // independently, so the number alone doesn't identify a build.
            let app_version = p
                .get("app_version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let platform = p
                .get("platform")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let engine = engine.clone();
            let text = tokio::task::spawn_blocking(move || {
                // Secrets are read up front, with the DB lock released, because
                // they come from the OS keychain — the slowest and most
                // failure-prone dependency here (a Flatpak Secret portal with no
                // backend can hang outright). Holding the store lock across that
                // would stall every other request behind a backup.
                let secrets: std::collections::HashMap<String, secrets::Secrets> =
                    if include_secrets {
                        let ids = {
                            let conn = engine.db.lock().unwrap();
                            store::list_accounts(&conn)?
                                .iter()
                                .filter_map(|account| account.get("id").and_then(Value::as_str))
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        };
                        ids.into_iter()
                            // An account whose entry is missing or unreadable exports
                            // without one rather than failing the whole backup.
                            .map(|id| {
                                let loaded = secrets::load(&id).unwrap_or_default();
                                (id, loaded)
                            })
                            .collect()
                    } else {
                        std::collections::HashMap::new()
                    };
                backup::export(
                    &engine.db.lock().unwrap(),
                    include_secrets,
                    Some(passphrase.as_str()),
                    backup::Host {
                        platform: platform.as_str(),
                        app_version: app_version.as_str(),
                    },
                    &|account| secrets.get(account).cloned().unwrap_or_default(),
                )
            })
            .await??;
            Ok(json!({ "backup": text }))
        }

        // Restore a backup document. An encrypted file opened without a
        // passphrase comes back as `needs_passphrase` (not an error) so the UI
        // can prompt and call again instead of showing a failure.
        "backup.import" => {
            let text = req_str(p, "backup")?;
            let passphrase = p
                .get("passphrase")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let engine = engine.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                let data = match backup::parse(&text, Some(passphrase.as_str())) {
                    Ok(data) => data,
                    Err(err) if backup::needs_passphrase(&err.to_string()) => return Ok(None),
                    Err(err) => return Err(err),
                };
                let conn = engine.db.lock().unwrap();
                let summary = backup::apply(&conn, &data, &|conn, account, secrets| {
                    // Mirror DesktopHost::store_secret: the keychain owns the
                    // secret, and the legacy SQLite column stays empty.
                    let _ = conn;
                    secrets::store(account, secrets)
                })?;
                // Restored settings include the app-wide proxy, which socket
                // code reads from a process-global slot.
                proxy::load_global(&conn)?;
                Ok(Some(summary))
            })
            .await??;
            match outcome {
                Some(summary) => Ok(summary.to_json()),
                None => Ok(json!({ "needs_passphrase": true })),
            }
        }

        // All accounts (mail + rss) as bridge-shaped JSON, from the one DB.
        "account.list" => {
            let mut accounts = store::list_accounts(&engine.db.lock().unwrap())?;
            let live_accounts = engine.accounts.lock().await;
            for account in &mut accounts {
                if account
                    .get("auth_type")
                    .and_then(Value::as_str)
                    .is_some_and(|auth_type| auth_type == "rss")
                {
                    continue;
                }
                let Some(id) = account
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                else {
                    continue;
                };
                if let Some(obj) = account.as_object_mut() {
                    let needs_reconnect = live_accounts
                        .get(&id)
                        .is_none_or(|creds| !creds_have_required_secret(creds));
                    obj.insert("needs_reconnect".to_string(), json!(needs_reconnect));
                }
            }
            Ok(json!({ "accounts": accounts }))
        }

        // Add an RSS feed: fetch + parse + persist on the blocking pool (network
        // I/O), returning the bridge Account JSON.
        "account.addRss" => {
            let feed_url = req_str(p, "feed_url")?;
            let display_name = req_str(p, "display_name").unwrap_or_default();
            let engine = engine.clone();
            let account =
                tokio::task::spawn_blocking(move || rss::add(&engine.db, &feed_url, &display_name))
                    .await??;
            Ok(json!({ "account": account }))
        }

        // Add a feed to an existing RSS account (network on the blocking pool).
        "feed.add" => {
            let account = req_str(p, "account")?;
            let feed_url = req_str(p, "feed_url")?;
            let engine = engine.clone();
            let res =
                tokio::task::spawn_blocking(move || rss::add_feed(&engine.db, &account, &feed_url))
                    .await??;
            Ok(res)
        }

        // Remove a single feed (subscription) and its items from an RSS account.
        "feed.remove" => {
            let thread_id = req_str(p, "thread_id")?;
            let res = rss::remove_feed(&engine.db.lock().unwrap(), &thread_id)?;
            Ok(res)
        }

        // Move a feed subscription between RSS accounts without losing cached
        // items or per-item read/starred state.
        "feed.move" => {
            let thread_id = req_str(p, "thread_id")?;
            let target_account = req_str(p, "target_account")?;
            let res = rss::move_feed(&engine.db.lock().unwrap(), &thread_id, &target_account)?;
            Ok(res)
        }

        // Serialize one RSS account's feeds to an OPML 2.0 document.
        "rss.exportOpml" => {
            let account = req_str(p, "account")?;
            let opml = rss::export_opml(&engine.db.lock().unwrap(), &account)?;
            Ok(json!({ "opml": opml }))
        }

        // Import feeds from an OPML document into one RSS account. Returns the
        // number of feeds added; the caller reloads accounts and syncs.
        "rss.importOpml" => {
            let opml = req_str(p, "opml")?;
            let account = req_str(p, "account")?;
            let engine = engine.clone();
            let imported =
                tokio::task::spawn_blocking(move || rss::import_opml(&engine.db, &opml, &account))
                    .await??;
            Ok(json!({ "imported": imported }))
        }

        // RSS thread read: paginated newest-first slice (or full thread when
        // `limit` is omitted), as final Message JSON.
        "rss.thread" => {
            let thread_id = req_str(p, "thread_id")?;
            let limit = p.get("limit").and_then(Value::as_u64).map(|n| n as u32);
            let before_cursor = p
                .get("before_cursor")
                .and_then(Value::as_str)
                .and_then(parse_rss_cursor);
            let (messages, next_cursor) = rss::read_thread_page(
                &engine.db.lock().unwrap(),
                &thread_id,
                before_cursor,
                limit,
            )?;
            let mut out = json!({ "messages": messages });
            if let Some(cursor) = next_cursor {
                out.as_object_mut()
                    .unwrap()
                    .insert("next_cursor".into(), Value::String(cursor));
            }
            Ok(out)
        }

        "rss.markRead" => {
            let thread_id = req_str(p, "thread_id")?;
            // Defaults to read; pass seen:false to mark unread.
            let seen = p.get("seen").and_then(Value::as_bool).unwrap_or(true);
            let item_keys = p
                .get("item_keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if item_keys.is_empty() {
                rss::mark_thread_read(&engine.db.lock().unwrap(), &thread_id, seen)?;
            } else {
                rss::mark_items_read(&engine.db.lock().unwrap(), &thread_id, &item_keys, seen)?;
            }
            Ok(json!({ "ok": true }))
        }

        "rss.markAllRead" => {
            let account = req_str(p, "account")?;
            let updated = rss::mark_account_read(&engine.db.lock().unwrap(), &account)?;
            Ok(json!({
                "ok": true,
                "updated": updated,
                "folder_unreads": { (account): { "inbox": 0 } },
            }))
        }

        "rss.markStarred" => {
            let thread_id = req_str(p, "thread_id")?;
            let starred = p.get("starred").and_then(Value::as_bool).unwrap_or(true);
            let item_keys = p
                .get("item_keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if item_keys.is_empty() {
                rss::mark_thread_starred(&engine.db.lock().unwrap(), &thread_id, starred)?;
            } else {
                rss::mark_items_starred(
                    &engine.db.lock().unwrap(),
                    &thread_id,
                    &item_keys,
                    starred,
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Store (and validate) IMAP credentials for an account.
        "account.connect" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let host = req_str(p, "host")?;
            let mut creds = imap::Creds {
                host: host.clone(),
                port: req_u16(p, "port").unwrap_or(993),
                user: req_str(p, "user")?,
                password: p
                    .get("password")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                tls: p.get("tls").and_then(Value::as_bool).unwrap_or(true),
                starttls: p.get("starttls").and_then(Value::as_bool).unwrap_or(false),
                smtp_host: req_str(p, "smtp_host").unwrap_or(host),
                smtp_port: req_u16(p, "smtp_port").unwrap_or(587),
                smtp_tls: p.get("smtp_tls").and_then(Value::as_bool).unwrap_or(true),
                smtp_starttls: p
                    .get("smtp_starttls")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                auth_type: p
                    .get("auth_type")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "password".to_string()),
                access_token: p
                    .get("access_token")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                refresh_token: p
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                token_expires_at: p
                    .get("token_expires_at")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                oauth_client_id: p
                    .get("oauth_client_id")
                    .or_else(|| p.get("client_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                oauth_client_secret: p
                    .get("oauth_client_secret")
                    .or_else(|| p.get("client_secret"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                oauth_token_url: p
                    .get("oauth_token_url")
                    .or_else(|| p.get("token_url"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                oauth_scope: p
                    .get("oauth_scope")
                    .or_else(|| p.get("scope"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                proxy: proxy::ProxyChoice::from_json(p.get("proxy").unwrap_or(&Value::Null)),
                // Set once the user has inspected and accepted a certificate
                // that webpki rejects (see `account.probeCert`).
                cert_pin: cert_pin_param(p, "cert_pin"),
                smtp_cert_pin: cert_pin_param(p, "smtp_cert_pin"),
            };
            // A reconnect resends the setup form, which has no field for the
            // account's proxy or the certificates it accepted. Carry those over
            // from the stored account so saving credentials again does not
            // silently reset them.
            let omitted = imap::OmittedSettings {
                proxy: p.get("proxy").is_none(),
                cert_pin: p.get("cert_pin").is_none(),
                smtp_cert_pin: p.get("smtp_cert_pin").is_none(),
                password: p.get("password").is_none(),
            };
            if omitted.any() {
                let stored = {
                    let db = engine.db.lock().unwrap();
                    store::load_account(&db, &id)?
                };
                if let Some(mut stored) = stored {
                    // The password lives in the keychain, not the account row,
                    // so the stored creds have to be hydrated before they can
                    // supply one.
                    if omitted.password {
                        let db = engine.db.lock().unwrap();
                        engine.host.apply_secret(&db, &id, &mut stored);
                    }
                    creds.carry_over(&stored, omitted);
                }
            }
            // Password accounts validate before storage. OAuth accounts may be
            // created directly after Google's token exchange; IMAP validation
            // can be slow or network-dependent, and later sync/watch calls will
            // surface any mailbox access failure.
            if p.get("validate").and_then(Value::as_bool).unwrap_or(true) {
                let mut session =
                    tokio::time::timeout(Duration::from_secs(20), imap::connect(&creds))
                        .await
                        .map_err(|_| anyhow::anyhow!("IMAP validation timed out"))??;
                let _ = session.logout().await;
                // The submission server can be a different daemon with a
                // certificate of its own; a save that only validated IMAP would
                // hand the user an account that fails at the first send.
                tokio::time::timeout(Duration::from_secs(20), smtp::check_certificate(&creds))
                    .await
                    .unwrap_or(Ok(()))?;
            }
            let meta = store::AccountMeta {
                engine: "mail".to_string(),
                provider: p
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("custom")
                    .to_string(),
                email: p
                    .get("email")
                    .and_then(Value::as_str)
                    .unwrap_or(&creds.user)
                    .to_string(),
                display_name: p
                    .get("display_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                avatar_url: p
                    .get("avatar_url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                sender_name: p
                    .get("sender_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            };
            {
                let db = engine.db.lock().unwrap();
                store::upsert_account(&db, &id, &meta, &creds)?;
            }
            secrets::store(&id, &secrets::Secrets::from_creds(&creds))?;
            engine.accounts.lock().await.insert(id.clone(), creds);
            // This call also edits an existing account (server settings, a new
            // password, a reconnect). Replacing the cached creds is not enough:
            // warm pooled sessions and the live IDLE watcher are already
            // authenticated against the *old* server, and would keep serving
            // reads and pushes from it indefinitely. Drop the pool and wake the
            // watchers so the next connection is made with what was just saved.
            engine.clear_pool(&id);
            engine.resume_signal.notify_waiters();
            // Start watching the new account right away. Only startup resumed
            // IDLE for known accounts, so an account added mid-session stayed
            // unwatched (and its INBOX unwarmed) until the next launch: mail
            // arrived on the server and nothing pushed it into the store.
            if !engine.is_paused(&id) {
                if start_idle_watch(engine.clone(), out.clone(), id.clone(), "INBOX".to_string()) {
                    spawn_body_prefetch(engine.clone(), id.clone(), "INBOX".to_string());
                }
            }
            Ok(json!({ "ok": true, "account": id }))
        }

        // Cache-only (instant). When refresh != false, also kicks a background
        // sync that emits mail.synced; event-driven reloads pass refresh:false to
        // avoid a sync→event→reload loop.
        // RSS accounts return final Folder JSON (one synthetic Inbox); mail
        // returns raw rows the bridge formats. Routed by the account's engine.
        "folders.list" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                let folders = rss::folders(&engine.db.lock().unwrap(), &account)?;
                return Ok(json!({ "folders": folders }));
            }
            let folders = store::get_folders(&engine.db.lock().unwrap(), &account)?;
            if p.get("refresh").and_then(Value::as_bool).unwrap_or(true) {
                spawn_folder_sync(engine.clone(), out.clone(), account);
            }
            Ok(json!({ "folders": serde_json::to_value(folders)? }))
        }

        "folders.create" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                return Err(anyhow::anyhow!("RSS accounts do not support folders"));
            }
            let display_name = req_str(p, "name")?.trim().to_string();
            if display_name.is_empty() {
                return Err(anyhow::anyhow!("Folder name is required"));
            }
            // The user types UTF-8; servers without UTF8=ACCEPT want modified
            // UTF-7, and the wire form is what we store and address it by.
            let name = meron_core::utf7::encode(&display_name);

            engine
                .with_write_session(&account, |session| {
                    let name = name.clone();
                    Box::pin(async move { imap::create_folder(session, &name).await })
                })
                .await?;

            let folder = imap::Folder {
                name,
                display_name,
                delimiter: None,
                ..Default::default()
            };
            {
                let db = engine.db.lock().unwrap();
                store::upsert_folders(&db, &account, std::slice::from_ref(&folder))?;
            }
            Ok(json!({ "folders": serde_json::to_value(vec![folder])? }))
        }

        // Delete a folder and everything nested under it on the server, then
        // forget the whole subtree's cache. Unrecoverable, so the special-use
        // gate is re-checked here rather than trusted from the caller.
        "folders.delete" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                return Err(anyhow::anyhow!("RSS accounts do not support folders"));
            }
            let folder = canon_folder(&req_str(p, "folder").or_else(|_| req_str(p, "name"))?);
            let targets = {
                let db = engine.db.lock().unwrap();
                mail_model::check_folder_deletable(&db, &account, &folder)
                    .map_err(anyhow::Error::msg)?;
                mail_model::folder_delete_targets(&db, &account, &folder)
                    .map_err(anyhow::Error::msg)?
            };

            // EXAMINE is a read-only preflight and may retry on a stale pooled
            // socket. DELETE itself starts only after that succeeds and is
            // never retried.
            let server_result = engine
                .with_preflighted_write_session(
                    &account,
                    |session| Box::pin(imap::prepare_folder_delete(session)),
                    |session| {
                        let targets = targets.clone();
                        Box::pin(async move { imap::delete_folders(session, &targets).await })
                    },
                )
                .await;
            let (removed, warning) = match server_result {
                Ok(removed) => (removed, None),
                Err(err) => match err.downcast::<imap::PartialFolderDelete>() {
                    Ok(partial) => {
                        let (removed, warning) = partial.into_parts();
                        (removed, Some(warning))
                    }
                    Err(err) => return Err(err),
                },
            };

            let (deleted, folders) = {
                let db = engine.db.lock().unwrap();
                let mut deleted = 0;
                for target in &removed {
                    deleted += store::delete_folder(&db, &account, target)?;
                }
                (deleted, store::get_folders(&db, &account)?)
            };
            Ok(json!({
                "ok": warning.is_none(),
                "folder": folder,
                // Every folder that went with it, so the caller can clear the
                // views and caches keyed on a nested folder as well.
                "removed": removed,
                "deleted": deleted,
                "folders": serde_json::to_value(folders)?,
                "warning": warning,
            }))
        }

        "messages.unifiedRecent" => {
            let cursors = p
                .get("before_cursor")
                .and_then(Value::as_str)
                .and_then(unified::decode_cursor)
                .unwrap_or_default();
            let accounts = store::list_accounts(&engine.db.lock().unwrap())?
                .into_iter()
                .filter(|account| {
                    account
                        .get("included_in_unified")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                })
                .filter_map(|account| {
                    account
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .filter(|account| cursors.is_empty() || cursors.contains_key(account))
                .collect::<Vec<_>>();
            // The unified view switches folders by *role*: each account answers
            // from its own Sent/Archive/Trash/…, and an account whose server has
            // no such mailbox drops out of the merge silently. Surfacing that as
            // a failure would pin a permanent error banner on the view for any
            // account whose provider simply lacks the folder.
            let role = req_str(p, "folder_role").unwrap_or_else(|_| "inbox".to_string());
            let mut folders = Vec::with_capacity(accounts.len());
            {
                let db = engine.db.lock().unwrap();
                for account in accounts {
                    if let Some(folder) = store::folder_for_role(&db, &account, &role)? {
                        folders.push((account, folder));
                    }
                }
            }
            let mut pages = Vec::with_capacity(folders.len());
            for (account, folder) in folders {
                let mut params = json!({
                    "account": account.clone(),
                    "folder": folder,
                    "query": req_str(p, "query").unwrap_or_default(),
                    "filter": req_str(p, "filter").unwrap_or_default(),
                    "limit": req_u16(p, "limit").unwrap_or(50),
                    "refresh": p.get("refresh").and_then(Value::as_bool).unwrap_or(true),
                    "group": true,
                });
                if let Some(cursor) = cursors.get(&account) {
                    params["before_cursor"] = Value::String(cursor.clone());
                }
                let request = Request {
                    id: req.id,
                    method: "messages.recent".to_string(),
                    params,
                };
                let result = Box::pin(dispatch(engine, &request, out))
                    .await
                    .map_err(|err| format!("{err:#}"));
                pages.push((account, result));
            }
            Ok(unified::merge_pages(pages, "threads"))
        }

        // RSS returns final thread Message JSON under "threads"; mail returns raw
        // rows under "messages" the bridge groups into threads.
        "messages.recent" => {
            let account = req_str(p, "account")?;
            let request = thread_list::ThreadListQuery::from_params(p, "folder");
            let refresh = p.get("refresh").and_then(Value::as_bool).unwrap_or(true);
            if is_rss(engine, &account)? {
                let page = thread_list::rss_page(&engine.db.lock().unwrap(), &account, &request)?;
                if refresh {
                    spawn_rss_sync(engine.clone(), out.clone(), account);
                }
                return Ok(page);
            }
            let folder = request.folder.clone();
            let limit = request.limit;
            // Capture this before spawning the background refresh. The returned
            // page was read from the pre-refresh cache, so its empty-state
            // metadata must describe that same snapshot.
            let folder_synced_before =
                store::get_folder_state(&engine.db.lock().unwrap(), &account, &folder)?.is_some();
            // Desktop starred reads are online-first. Search first paints the
            // local index with refresh=false, then repeats with refresh=true;
            // snapshot-backed later pages are local even though they travel
            // through the shared search engine.
            let (messages, next_cursor) = match request.source() {
                thread_list::MailSource::Starred => {
                    let folders = starred_search_folders(engine, &account, &folder).await;
                    (
                        search_starred_mail_messages(engine, &account, &folders, limit, refresh)
                            .await?,
                        None,
                    )
                }
                thread_list::MailSource::Recent { unread_only } => store::get_recent_page(
                    &engine.db.lock().unwrap(),
                    &account,
                    &folder,
                    limit,
                    request.before_cursor,
                    unread_only,
                )?,
                thread_list::MailSource::Search => {
                    // Chat-view search spans the selected folder plus Sent, so a
                    // lookup surfaces both received and self-sent mail (and old
                    // messages filed under Sent), not just the current mailbox.
                    let folders = search_folders(&engine.db.lock().unwrap(), &account, &folder);
                    if refresh || request.search_before_cursor.as_ref().is_some() {
                        let page = search_mail_messages(
                            engine,
                            &account,
                            &folders,
                            &request.query,
                            limit,
                            request.search_before_cursor.as_ref(),
                        )
                        .await?;
                        (page.messages, page.next_cursor)
                    } else {
                        let messages = store::search_messages_in_folders(
                            &engine.db.lock().unwrap(),
                            &account,
                            &folders,
                            &request.query,
                            limit,
                            None,
                        )?;
                        let next_cursor = store::search_next_cursor(&messages, limit, 0);
                        (messages, next_cursor)
                    }
                }
            };
            if refresh && request.wants_background_sync() {
                spawn_message_sync(
                    engine.clone(),
                    out.clone(),
                    account.clone(),
                    folder.clone(),
                    limit,
                );
            }
            let mut page = thread_list::mail_page(
                &engine.db.lock().unwrap(),
                &account,
                &folder,
                messages,
                next_cursor,
                p.get("group").and_then(Value::as_bool).unwrap_or(false),
            )?;
            page.as_object_mut().unwrap().insert(
                "folder_synced".to_string(),
                Value::Bool(folder_synced_before),
            );
            Ok(page)
        }

        // Every starred item across all accounts, local cache only (the
        // IMAP-backed starred filter keeps mail flags fresh; no round-trip
        // here). Core returns one final, searchable, paginated item model for
        // both mail and RSS so transport adapters do not mint ids or reshape it.
        "starred.items" => {
            let limit = req_u32(p, "limit").unwrap_or(200);
            let db = engine.db.lock().unwrap();
            let mut items = mail_model::starred_thread_cards(&db, 2_000)?;
            items.extend(rss::starred_items(&db, 2_000)?);
            Ok(mail_model::starred_page(
                items,
                &req_str(p, "query").unwrap_or_default(),
                &req_str(p, "filter").unwrap_or_else(|_| "all".to_string()),
                limit as usize,
                p.get("before_cursor").and_then(Value::as_str),
            ))
        }

        "identity.allocate" => Ok(json!({
            "message_id": mail_model::allocate_message_id(
                &req_str(p, "account_id").unwrap_or_default(),
                p.get("draft").and_then(Value::as_bool).unwrap_or(false),
            )
        })),

        // Recipient autocomplete: distinct correspondents from cached messages,
        // matched against `query` and ranked by frequency/recency.
        "contacts.suggest" => {
            let account = req_str(p, "account").unwrap_or_default();
            let query = req_str(p, "query").unwrap_or_default();
            let limit = req_u32(p, "limit").unwrap_or(8);
            let contacts =
                store::suggest_contacts(&engine.db.lock().unwrap(), &account, &query, limit)?;
            Ok(json!({ "contacts": contacts }))
        }

        // Fire-and-forget background sync; the result arrives via mail.synced.
        "messages.sync" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                spawn_rss_sync(engine.clone(), out.clone(), account);
                return Ok(json!({ "ok": true, "queued": true }));
            }
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let limit = req_u16(p, "limit").unwrap_or(50) as u32;
            spawn_message_sync(engine.clone(), out.clone(), account, folder, limit);
            Ok(json!({ "ok": true, "queued": true }))
        }

        "send" => {
            let account = req_str(p, "account")?;
            let to = req_str(p, "to")?;
            let cc = req_str(p, "cc").unwrap_or_default();
            let bcc = req_str(p, "bcc").unwrap_or_default();
            let subject = req_str(p, "subject").unwrap_or_default();
            let body = req_str(p, "body").unwrap_or_default();
            let html = req_str(p, "html").unwrap_or_default();
            let in_reply_to = req_str(p, "in_reply_to").unwrap_or_default();
            let references = req_str(p, "references").unwrap_or_default();
            let reply_to = req_str(p, "reply_to").unwrap_or_default();
            // Client-generated Message-ID so the optimistic bubble and a quick
            // follow-up reply share the id the Sent copy will carry.
            let message_id = req_str(p, "message_id").unwrap_or_default();
            let attachments = opt_attachments(p)?;
            let requested_from = req_str(p, "from").unwrap_or_default();
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from)?;
            let raw = smtp::send(
                &creds,
                &from_addr,
                &sender_name,
                &to,
                &cc,
                &bcc,
                &subject,
                &body,
                &html,
                &attachments,
                &in_reply_to,
                &references,
                &reply_to,
                &message_id,
            )
            .await?;
            // Finalize the Sent view. For Gmail/Outlook defaults this only
            // refreshes the provider-created copy; other accounts get Meron's
            // best-effort APPEND plus refresh. The mail already left via SMTP,
            // so Sent-folder issues should not surface as "send failed".
            if let Err(err) = append_to_sent(engine, &account, &raw).await {
                eprintln!("meron-core: APPEND to Sent failed for {account}: {err:#}");
            }
            Ok(json!({ "ok": true }))
        }

        "save_draft" => {
            let account = req_str(p, "account")?;
            let to = req_str(p, "to").unwrap_or_default();
            let cc = req_str(p, "cc").unwrap_or_default();
            let bcc = req_str(p, "bcc").unwrap_or_default();
            let subject = req_str(p, "subject").unwrap_or_default();
            let body = req_str(p, "body").unwrap_or_default();
            let html = req_str(p, "html").unwrap_or_default();
            let in_reply_to = req_str(p, "in_reply_to").unwrap_or_default();
            let references = req_str(p, "references").unwrap_or_default();
            let reply_to = req_str(p, "reply_to").unwrap_or_default();
            let attachments = opt_attachments(p)?;
            let requested_from = req_str(p, "from").unwrap_or_default();
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from)?;
            // Stable per-draft Message-ID: each autosave reuses it so the IMAP
            // layer can find and prune the prior copy instead of piling up dups.
            let draft_id = req_str(p, "draft_id").unwrap_or_default();
            let raw = smtp::build_message(
                &sender_name,
                &from_addr,
                &to,
                &cc,
                &bcc,
                true,
                &subject,
                &body,
                &html,
                &attachments,
                &in_reply_to,
                &references,
                &reply_to,
                &draft_id,
            )?;
            append_to_drafts(engine, &account, &raw, &draft_id).await?;
            Ok(json!({ "ok": true }))
        }

        "discard_draft" => {
            let account = req_str(p, "account")?;
            let draft_id = req_str(p, "draft_id").unwrap_or_default();
            if draft_id.trim().is_empty() {
                return Ok(json!({ "ok": true, "deleted": 0 }));
            }
            // The LIST that finds the folder changes nothing and is where a dead
            // pooled session gives out, so it preflights the delete that follows.
            let drafts_slot: Arc<std::sync::Mutex<Option<String>>> =
                Arc::new(std::sync::Mutex::new(None));
            let (drafts, deleted) = engine
                .with_preflighted_write_session(
                    &account,
                    |session| {
                        let slot = Arc::clone(&drafts_slot);
                        Box::pin(async move {
                            let drafts = imap::find_drafts_folder(session)
                                .await?
                                .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                            *slot.lock().unwrap() = Some(drafts);
                            anyhow::Ok(())
                        })
                    },
                    |session| {
                        let draft_id = draft_id.clone();
                        let slot = Arc::clone(&drafts_slot);
                        Box::pin(async move {
                            let drafts = { slot.lock().unwrap().clone() }
                                .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                            let deleted = imap::discard_draft(session, &drafts, &draft_id).await?;
                            anyhow::Ok((drafts, deleted))
                        })
                    },
                )
                .await?;
            // Drop the locally cached copies too, or the discarded draft keeps
            // showing in the thread view until the next full Drafts sync.
            store::delete_draft_copies(
                &engine.db.lock().unwrap(),
                &account,
                &drafts,
                &draft_id,
                None,
            )?;
            if let Ok(thread_key) = req_str(p, "thread_key") {
                store::delete_quick_reply_drafts_in_thread(
                    &engine.db.lock().unwrap(),
                    &account,
                    &drafts,
                    &thread_key,
                )?;
            }
            Ok(json!({ "ok": true, "deleted": deleted, "permanent": true }))
        }

        // Fetch one message's original RFC822 bytes and write them directly to
        // the path selected by the desktop save dialog. Keeping the bytes out
        // of the JSON response avoids its bounded line size; BODY.PEEK[] keeps
        // an unread message unread.
        "messages.saveRaw" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let uid = req_u32(p, "uid")?;
            let path = req_str(p, "path")?;

            let raw_messages = engine
                .with_read_session(&account, |session| {
                    let folder = folder.clone();
                    Box::pin(async move {
                        imap::fetch_raw_messages_for_copy(session, &folder, &[uid]).await
                    })
                })
                .await?;
            let message = raw_messages
                .into_iter()
                .next()
                .with_context(|| format!("message {uid} not found in {folder}"))?;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            let mut output = options
                .open(&path)
                .with_context(|| format!("open message export {path}"))?;
            output
                .write_all(&message.raw)
                .with_context(|| format!("write message export {path}"))?;
            Ok(json!({ "saved": true, "size": message.raw.len() }))
        }

        "messages.read" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let uid = req_u32(p, "uid")?;

            let message = read_cached_or_fetch(engine, &account, &folder, uid).await?;
            let mine = store::self_addrs(&engine.db.lock().unwrap(), &account);
            let outgoing =
                store::is_outgoing(&mine, &folder, &message.from_addr, message.delivered);
            Ok(json!({ "outgoing": outgoing, "message": serde_json::to_value(message)? }))
        }

        "messages.thread" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            // Card ids carry a branch subject suffix; the store queries use the
            // root key and the branch filter narrows the rows afterwards.
            let (thread_key, subject_filter) = store::split_thread_key(&req_str(p, "thread_key")?);
            // Pagination is opt-in: callers that don't pass `limit` get the
            // full thread (preserves the markRead full-scan path in app.go).
            let limit = p.get("limit").and_then(Value::as_u64).map(|n| n as u32);
            let before_cursor = p.get("before_cursor").and_then(Value::as_str);
            // The bridge passes the frontend's exact thread id so message ids
            // match what the UI keys on; direct callers (tests) may omit it.
            let thread_id = req_str(p, "thread_id")
                .unwrap_or_else(|_| mail_model::format_thread_id(&account, &folder, &thread_key));

            // For UI reads (limit present), pull in any referenced ancestor
            // messages missing from the local cache so the reader shows the
            // full conversation instead of just the synced tail or a lone
            // draft. Runs in the background; if the fill finds anything it
            // emits `mail.synced` and the reader re-reads. The markRead
            // full-scan path (no limit) skips this entirely.
            if limit.is_some() {
                maybe_spawn_fill_thread_gaps(engine, out, &account, &thread_key);
            }

            // The background body fill announces itself with `mail.synced`,
            // which the desktop frontend already answers by re-reading the
            // open thread.
            let on_bodies_fetched: thread_read::BodiesFetchedHook = {
                let out = out.clone();
                let account = account.clone();
                Box::new(move || {
                    let out = out.clone();
                    let account = account.clone();
                    tokio::spawn(async move {
                        emit(
                            &out,
                            "mail.synced",
                            json!({ "account": account, "folder": "inbox", "synced": 0 }),
                        )
                        .await;
                    });
                })
            };
            thread_read::read_thread_page(
                engine,
                thread_read::ThreadReadArgs {
                    account: &account,
                    folder: &folder,
                    thread_id: &thread_id,
                    thread_key: &thread_key,
                    subject_filter: subject_filter.as_deref(),
                    limit,
                    before_cursor,
                    media_root: parse::media_root(),
                    bake_html_policy: true,
                },
                Some(on_bodies_fetched),
            )
            .await
        }

        "messages.threadHeaders" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) = store::split_thread_key(&req_str(p, "thread_key")?);
            let headers = {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
            };
            let headers = headers
                .into_iter()
                .filter(|header| match subject_filter.as_deref() {
                    Some(filter) => store::thread_grouping_subject(&header.subject) == filter,
                    None => true,
                })
                .map(|header| {
                    json!({
                        "uid": header.uid,
                        "folder": folder,
                        "subject": header.subject,
                        "seen": header.seen,
                        "starred": header.starred,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({ "headers": headers }))
        }

        "messages.markRead" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            // Defaults to true (mark read); pass seen:false to mark unread.
            let seen = p.get("seen").and_then(Value::as_bool).unwrap_or(true);

            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let uids = if !explicit_uids.is_empty() {
                explicit_uids
            } else if thread_key.is_empty() {
                uid.into_iter().collect::<Vec<_>>()
            } else if !seen {
                // Marking a whole thread unread flags its newest message only.
                let db = engine.db.lock().unwrap();
                store::newest_thread_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                )?
            } else {
                // Only touch the thread's messages whose flag actually differs.
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
                    .into_iter()
                    .filter(|header| header.seen != seen)
                    .filter(|header| match subject_filter.as_deref() {
                        Some(filter) => store::thread_grouping_subject(&header.subject) == filter,
                        None => true,
                    })
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
            };

            if !uids.is_empty() {
                engine
                    .with_preflighted_write_session(
                        &account,
                        |session| {
                            let folder = folder.clone();
                            Box::pin(
                                async move { imap::prepare_flag_update(session, &folder).await },
                            )
                        },
                        |session| {
                            let uids = uids.clone();
                            Box::pin(async move { imap::store_seen(session, &uids, seen).await })
                        },
                    )
                    .await?;
            }

            {
                let db = engine.db.lock().unwrap();
                if thread_key.is_empty() || subject_filter.is_some() || !seen {
                    // Branch-scoped: a whole-thread update would flip sibling
                    // subject branches sharing the root thread_key. Marking
                    // unread is per-uid for the same reason — only the newest
                    // message was flagged.
                    for marked_uid in &uids {
                        store::update_message_seen(&db, &account, &folder, *marked_uid, seen)?;
                    }
                } else {
                    store::update_thread_seen(&db, &account, &folder, &thread_key, seen)?;
                }
            }
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };
            mail_model::mutation_result(
                json!({ "ok": true }),
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                None,
                Some(!seen),
                None,
                false,
            )
        }

        "messages.markStarred" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            // Defaults to true (mark starred); pass starred:false to unstar.
            let starred = p.get("starred").and_then(Value::as_bool).unwrap_or(true);

            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let uids = if !explicit_uids.is_empty() {
                explicit_uids
            } else if thread_key.is_empty() {
                uid.into_iter().collect::<Vec<_>>()
            } else {
                // Only touch the thread's messages whose flag actually differs.
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
                    .into_iter()
                    .filter(|header| header.starred != starred)
                    .filter(|header| match subject_filter.as_deref() {
                        Some(filter) => store::thread_grouping_subject(&header.subject) == filter,
                        None => true,
                    })
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
            };

            if !uids.is_empty() {
                engine
                    .with_preflighted_write_session(
                        &account,
                        |session| {
                            let folder = folder.clone();
                            Box::pin(
                                async move { imap::prepare_flag_update(session, &folder).await },
                            )
                        },
                        |session| {
                            let uids = uids.clone();
                            Box::pin(
                                async move { imap::store_starred(session, &uids, starred).await },
                            )
                        },
                    )
                    .await?;
            }

            {
                let db = engine.db.lock().unwrap();
                if thread_key.is_empty() || subject_filter.is_some() {
                    // Branch-scoped: a whole-thread update would star sibling
                    // subject branches sharing the root thread_key.
                    for marked_uid in &uids {
                        store::update_message_starred(
                            &db,
                            &account,
                            &folder,
                            *marked_uid,
                            starred,
                        )?;
                    }
                } else {
                    store::update_thread_starred(&db, &account, &folder, &thread_key, starred)?;
                }
            }
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };
            mail_model::mutation_result(
                json!({ "ok": true }),
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                None,
                None,
                Some(starred),
                false,
            )
        }

        "messages.delete" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };

            let uids = {
                let db = engine.db.lock().unwrap();
                store::resolve_message_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &explicit_uids,
                )?
            };

            if uids.is_empty() {
                return mail_model::mutation_result(
                    json!({ "ok": true, "deleted": 0 }),
                    &engine.db.lock().unwrap(),
                    &account,
                    &changed_thread_id,
                    &folder,
                    None,
                    None,
                    None,
                    false,
                );
            }

            // Mutating, so it never auto-retries.
            let trashed = delete_to_trash(engine, &account, &folder, &uids).await?;

            {
                let db = engine.db.lock().unwrap();
                // Discarding a draft must also drop hidden local copies sharing
                // its Message-ID (stale autosaves the pane deduped away), or the
                // thread card keeps its has_draft badge until the next full sync.
                if store::folder_role(&db, &account, &folder)? == "drafts" {
                    store::delete_draft_sibling_copies(&db, &account, &folder, &uids)?;
                }
                store::delete_messages_by_uid(&db, &account, &folder, &uids)?;
            }
            // The server delete/move-to-Trash completed for every resolved UID.
            // A concurrent source refresh may already have pruned the cache rows.
            let deleted = uids.len();
            let result = match trashed.as_deref() {
                None => json!({ "ok": true, "deleted": deleted, "permanent": true }),
                Some(trash) => json!({ "ok": true, "deleted": deleted, "trash": trash }),
            };
            mail_model::mutation_result(
                result,
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                trashed.as_deref(),
                None,
                None,
                true,
            )
        }

        "messages.move" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let target_folder = canon_folder(&req_str(p, "target_folder")?);
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };

            if folder == target_folder {
                return mail_model::mutation_result(
                    json!({ "ok": true, "moved": 0, "source_folder": folder, "target_folder": target_folder }),
                    &engine.db.lock().unwrap(),
                    &account,
                    &changed_thread_id,
                    &folder,
                    Some(&target_folder),
                    None,
                    None,
                    false,
                );
            }

            let uids = {
                let db = engine.db.lock().unwrap();
                store::resolve_message_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &explicit_uids,
                )?
            };

            if uids.is_empty() {
                return mail_model::mutation_result(
                    json!({ "ok": true, "moved": 0, "source_folder": folder, "target_folder": target_folder }),
                    &engine.db.lock().unwrap(),
                    &account,
                    &changed_thread_id,
                    &folder,
                    Some(&target_folder),
                    None,
                    None,
                    false,
                );
            }

            engine
                .with_write_session(&account, |session| {
                    let folder = folder.clone();
                    let target_folder = target_folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::move_to_folder(session, &folder, &target_folder, &uids).await
                    })
                })
                .await?;
            // Read-only refresh, on its own session: the MOVE has landed and
            // must not be retried, and a message the target folder holds that we
            // cannot parse must not sink the whole move.
            let target_batch =
                fetch_recent_resilient(engine, &account, &target_folder, 50.max(uids.len() as u32))
                    .await
                    .context("refresh target folder after move")?;

            {
                let db = engine.db.lock().unwrap();
                store::ensure_folder(&db, &account, &target_folder)?;
                store::upsert_messages(&db, &account, &target_folder, &target_batch.messages)?;
                store::set_folder_state(
                    &db,
                    &account,
                    &target_folder,
                    target_batch.uidvalidity,
                    target_batch.uid_next,
                )?;
                store::delete_messages_by_uid(&db, &account, &folder, &uids)?;
            }
            // The IMAP MOVE above completed for every resolved UID. A concurrent
            // source-folder refresh may already have pruned those rows locally,
            // so the cache DELETE count is not the number moved on the server.
            let moved = uids.len();
            mail_model::mutation_result(
                json!({ "ok": true, "moved": moved, "source_folder": folder, "target_folder": target_folder }),
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                Some(&target_folder),
                None,
                None,
                true,
            )
        }

        "messages.copy" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let target_account = req_str(p, "target_account")?;
            let target_folder = canon_folder(&req_str(p, "target_folder")?);
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let uids = {
                let db = engine.db.lock().unwrap();
                store::resolve_message_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &explicit_uids,
                )?
            };

            if uids.is_empty() {
                return Ok(json!({
                    "ok": true,
                    "copied": 0,
                    "source_folder": folder,
                    "target_account": target_account,
                    "target_folder": target_folder
                }));
            }

            let raw_messages = engine
                .with_read_session(&account, |session| {
                    let folder = folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::fetch_raw_messages_for_copy(session, &folder, &uids).await
                    })
                })
                .await?;

            if raw_messages.is_empty() {
                return Ok(json!({
                    "ok": true,
                    "copied": 0,
                    "source_folder": folder,
                    "target_account": target_account,
                    "target_folder": target_folder
                }));
            }

            let copied = raw_messages.len();
            engine
                .with_write_session(&target_account, |session| {
                    let target_folder = target_folder.clone();
                    let raw_messages = raw_messages.clone();
                    Box::pin(async move {
                        for message in &raw_messages {
                            imap::append_copied_message(session, &target_folder, message).await?;
                        }
                        anyhow::Ok(())
                    })
                })
                .await?;
            // Read-only refresh, on its own session; see the move handler above.
            let target_batch = fetch_recent_resilient(
                engine,
                &target_account,
                &target_folder,
                50.max(raw_messages.len() as u32),
            )
            .await
            .context("refresh target folder after copy")?;

            {
                let db = engine.db.lock().unwrap();
                store::ensure_folder(&db, &target_account, &target_folder)?;
                store::upsert_messages(
                    &db,
                    &target_account,
                    &target_folder,
                    &target_batch.messages,
                )?;
                store::set_folder_state(
                    &db,
                    &target_account,
                    &target_folder,
                    target_batch.uidvalidity,
                    target_batch.uid_next,
                )?;
            }

            Ok(json!({
                "ok": true,
                "copied": copied,
                "source_folder": folder,
                "target_account": target_account,
                "target_folder": target_folder
            }))
        }

        "folders.archive" => {
            let account = req_str(p, "account")?;
            let archive = engine
                .with_read_session(&account, |session| {
                    Box::pin(async move { imap::find_archive_folder(session).await })
                })
                .await?;
            match archive {
                Some(folder) => Ok(json!({ "folder": folder })),
                None => Err(anyhow::anyhow!("Archive folder not found for this account")),
            }
        }

        // Mark every message in a folder as read: set \Seen on the server for the
        // currently-unseen UIDs, then flip the whole folder seen in the store.
        "messages.markAllRead" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));

            let uids = {
                let db = engine.db.lock().unwrap();
                store::get_unseen_uids(&db, &account, &folder)?
            };

            if !uids.is_empty() {
                engine
                    .with_preflighted_write_session(
                        &account,
                        |session| {
                            let folder = folder.clone();
                            Box::pin(
                                async move { imap::prepare_flag_update(session, &folder).await },
                            )
                        },
                        |session| {
                            let uids = uids.clone();
                            Box::pin(async move { imap::store_seen(session, &uids, true).await })
                        },
                    )
                    .await?;
            }

            {
                let db = engine.db.lock().unwrap();
                store::mark_folder_seen(&db, &account, &folder, true)?;
            }
            mail_model::mutation_result(
                json!({ "ok": true, "updated": uids.len(), "folder": folder }),
                &engine.db.lock().unwrap(),
                &account,
                "",
                &folder,
                None,
                Some(false),
                None,
                false,
            )
        }

        "messages.markAllReadUnified" => {
            let role = req_str(p, "folder").unwrap_or_else(|_| "inbox".to_string());
            let account_folders = {
                let db = engine.db.lock().unwrap();
                store::list_accounts(&db)?
                    .into_iter()
                    .filter(|account| {
                        account
                            .get("included_in_unified")
                            .and_then(Value::as_bool)
                            .unwrap_or(true)
                    })
                    .filter(|account| {
                        account.get("auth_type").and_then(Value::as_str) != Some("rss")
                    })
                    .filter_map(|account| {
                        account
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .map(|account| {
                        let folder = store::folder_for_role(&db, &account, &role)?;
                        Ok(folder.map(|folder| (account, folder)))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            };
            let mut updated = 0_u64;
            let mut failures = Vec::new();
            let mut folder_unreads = serde_json::Map::new();
            let mut folder_counts = Vec::new();
            for (account, folder) in account_folders {
                let request = Request {
                    id: req.id,
                    method: "messages.markAllRead".to_string(),
                    params: json!({ "account": account, "folder": folder }),
                };
                match Box::pin(dispatch(engine, &request, out)).await {
                    Ok(result) => {
                        updated += result
                            .get("updated")
                            .and_then(Value::as_u64)
                            .unwrap_or_default();
                        if let Some(counts) = result
                            .get("folder_unreads")
                            .and_then(|all| all.get(&account))
                        {
                            folder_unreads.insert(account.clone(), counts.clone());
                        }
                        folder_counts.extend(
                            result
                                .get("folder_counts")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                    Err(err) => failures
                        .push(json!({ "account_id": account, "message": format!("{err:#}") })),
                }
            }
            Ok(json!({
                "ok": failures.is_empty(),
                "updated": updated,
                "failures": failures,
                "folder_unreads": folder_unreads,
                "folder_counts": folder_counts,
            }))
        }

        // Permanently delete every message in a folder, server side and in the
        // store. Restricted to Trash and Junk: the operation is unrecoverable,
        // so an arbitrary folder must never reach it even if a caller asks.
        "messages.emptyFolder" => {
            let account = req_str(p, "account")?;
            let folder = canon_folder(&req_str(p, "folder")?);
            let role = {
                let db = engine.db.lock().unwrap();
                store::folder_role(&db, &account, &folder)?
            };
            if role != "trash" && role != "junk" {
                return Err(anyhow::anyhow!(
                    "Only Trash and Junk folders can be emptied"
                ));
            }

            // Mutating, so it never auto-retries.
            let expunged = engine
                .with_write_session(&account, |session| {
                    let folder = folder.clone();
                    Box::pin(async move { imap::empty_folder(session, &folder).await })
                })
                .await?;

            let deleted = {
                let db = engine.db.lock().unwrap();
                store::delete_folder_messages(&db, &account, &folder)?
            };
            mail_model::mutation_result(
                json!({
                    "ok": true,
                    "deleted": deleted,
                    "expunged": expunged,
                    "folder": folder,
                    "role": role,
                }),
                &engine.db.lock().unwrap(),
                &account,
                "",
                &folder,
                None,
                None,
                None,
                true,
            )
        }

        // Fetch the certificate a mail server presents, so the account dialog
        // can show it and let the user pin it. Needed for local bridges (Proton
        // Mail Bridge) whose self-signed leaf webpki refuses outright; nothing
        // is sent over the probe connection.
        "account.probeCert" => {
            let host = req_str(p, "host")?;
            let port = req_u16(p, "port").unwrap_or(993);
            let protocol = p
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or("imap")
                .to_string();
            let starttls = p.get("starttls").and_then(Value::as_bool).unwrap_or(false);
            let proxy = proxy::ProxyChoice::from_json(p.get("proxy").unwrap_or(&Value::Null));
            let info =
                meron_core::tls::probe(&host, port, &protocol, starttls, proxy.resolve().as_ref())
                    .await?;
            Ok(json!({ "certificate": info }))
        }

        // Forget an account: drop its in-memory creds, cached state, and the
        // keychain secret. The IDLE watcher notices the account is gone on its
        // next loop and exits.
        "account.remove" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            engine.accounts.lock().await.remove(&id);
            // Drop any warm sessions: their creds are gone and must not be reused.
            engine.clear_pool(&id);
            {
                let db = engine.db.lock().unwrap();
                store::delete_account(&db, &id)?;
            }
            parse::remove_account_media(&parse::media_root(), &id);
            let _ = secrets::delete(&id);
            Ok(json!({ "ok": true }))
        }

        // Set the per-account "load remote images" preference.
        "account.setImages" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_load_remote_images(&engine.db.lock().unwrap(), &id, enabled)?;
            Ok(json!({ "ok": true }))
        }

        // Toggle whether conversation bubbles render original HTML when available.
        "account.setConversationHtml" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(
                &engine.db.lock().unwrap(),
                &id,
                "conversation_html",
                enabled,
            )?;
            Ok(json!({ "ok": true }))
        }

        // Set or clear the per-account chat wallpaper preference. The bridge
        // owns image-file validation and storage; the sidecar validates the
        // persisted JSON shape.
        "account.setChatWallpaper" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let wallpaper = match p.get("wallpaper") {
                Some(Value::Null) | None => None,
                Some(value) => {
                    let obj = value
                        .as_object()
                        .ok_or_else(|| anyhow::anyhow!("wallpaper must be an object"))?;
                    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or_default();
                    match kind {
                        "preset" => {
                            let preset_id = obj
                                .get("presetId")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .trim();
                            if preset_id.is_empty() {
                                anyhow::bail!("preset wallpaper requires presetId");
                            }
                            Some(json!({ "kind": "preset", "presetId": preset_id }))
                        }
                        "custom" => {
                            let url = obj
                                .get("url")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .trim();
                            if !url.starts_with("/media/wallpapers/") {
                                anyhow::bail!("custom wallpaper URL must be a Meron wallpaper");
                            }
                            Some(json!({ "kind": "custom", "url": url }))
                        }
                        _ => anyhow::bail!("unknown wallpaper kind"),
                    }
                }
            };
            store::set_account_pref_json(
                &engine.db.lock().unwrap(),
                &id,
                "chat_wallpaper",
                wallpaper,
            )?;
            Ok(json!({ "ok": true }))
        }

        // Set the account's display name.
        // Point one account at a different proxy than the app-wide setting (or
        // at none). Live sessions keep their sockets; the choice applies as
        // they reconnect.
        "account.setProxy" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let choice = proxy::ProxyChoice::from_json(p.get("proxy").unwrap_or(&Value::Null));
            store::set_account_proxy(&engine.db.lock().unwrap(), &id, &choice)?;
            if let Some(creds) = engine.accounts.lock().await.get_mut(&id) {
                creds.proxy = choice;
            }
            Ok(json!({ "ok": true }))
        }

        // Store certificate pins the user accepted for an account that already
        // exists — a server whose certificate rotated, or one whose failure only
        // showed up on a later sync or send. Omitting a key leaves that server's
        // pin alone; an explicit null clears it.
        "account.setCertPin" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let mut accounts = engine.accounts.lock().await;
            let existing = accounts.get(&id);
            let cert_pin = match p.get("cert_pin") {
                Some(_) => cert_pin_param(p, "cert_pin"),
                None => existing.and_then(|creds| creds.cert_pin.clone()),
            };
            let smtp_cert_pin = match p.get("smtp_cert_pin") {
                Some(_) => cert_pin_param(p, "smtp_cert_pin"),
                None => existing.and_then(|creds| creds.smtp_cert_pin.clone()),
            };
            store::set_account_cert_pins(
                &engine.db.lock().unwrap(),
                &id,
                cert_pin.as_deref(),
                smtp_cert_pin.as_deref(),
            )?;
            if let Some(creds) = accounts.get_mut(&id) {
                creds.cert_pin = cert_pin;
                creds.smtp_cert_pin = smtp_cert_pin;
            }
            drop(accounts);
            // Pooled sessions were built with the old trust decision.
            engine.clear_pool(&id);
            Ok(json!({ "ok": true }))
        }

        "account.setName" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let name = req_str(p, "name")?;
            {
                let db = engine.db.lock().unwrap();
                db.execute(
                    "UPDATE accounts SET display_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
                    rusqlite::params![name.trim(), id],
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Set the account's sender name.
        "account.setSenderName" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let name = req_str(p, "name")?;
            {
                let db = engine.db.lock().unwrap();
                db.execute(
                    "UPDATE accounts SET sender_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
                    rusqlite::params![name.trim(), id],
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Set or clear the account's UI avatar URL/path.
        "account.setAvatar" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let avatar_url = req_str(p, "avatar_url").unwrap_or_default();
            {
                let db = engine.db.lock().unwrap();
                db.execute(
                    "UPDATE accounts SET avatar_url = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
                    rusqlite::params![avatar_url.trim(), id],
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Replace an account's send-as aliases (the whole list). Entries are
        // {email, name?}; we trim, drop blank emails, and dedupe by email.
        "account.setAliases" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let mut aliases: Vec<store::Alias> = match p.get("aliases") {
                Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
                None => Vec::new(),
            };
            let mut seen = std::collections::HashSet::new();
            aliases.retain_mut(|a| {
                a.email = a.email.trim().to_string();
                a.name = a.name.trim().to_string();
                !a.email.is_empty() && seen.insert(a.email.to_lowercase())
            });
            {
                let db = engine.db.lock().unwrap();
                store::set_account_aliases(&db, &id, &aliases)?;
            }
            Ok(json!({ "ok": true }))
        }

        // Set or clear this account's signature override. A null `signature`
        // drops the pref, so the account follows the app-wide signature again.
        "account.setSignature" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let signature = store::AccountSignature::from_param(p.get("signature"))
                .map_err(|err| anyhow::anyhow!(err))?;
            store::set_account_pref_json(
                &engine.db.lock().unwrap(),
                &id,
                "signature",
                signature.map(|sig| json!(sig)),
            )?;
            Ok(json!({ "ok": true }))
        }

        // Toggle whether the account folds into the unified inbox. Purely a stored
        // pref the UI reads via account.list; no engine side effects.
        "account.setUnified" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(
                &engine.db.lock().unwrap(),
                &id,
                "included_in_unified",
                enabled,
            )?;
            Ok(json!({ "ok": true }))
        }

        // Toggle whether new mail/feed items raise a desktop notification. The
        // watcher still runs (mail keeps arriving); the bridge reads the `muted`
        // flag on each mail.newMessages event to decide whether to notify.
        "account.setMuted" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(&engine.db.lock().unwrap(), &id, "muted", enabled)?;
            Ok(json!({ "ok": true }))
        }

        // Pause/resume automatic checking. Pausing stops the IDLE watcher and
        // gates background syncs; resuming restarts the watcher for mail accounts.
        "account.setPaused" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(&engine.db.lock().unwrap(), &id, "paused", enabled)?;
            if enabled {
                // Wake live watchers so the just-paused one shuts down promptly,
                // and drop warm sessions so a paused account holds no connections.
                engine.pause_signal.notify_waiters();
                engine.clear_pool(&id);
            } else if !is_rss(engine, &id)? {
                // Resume: restart the IDLE watcher (deduped) and warm the inbox.
                if start_idle_watch(engine.clone(), out.clone(), id.clone(), "INBOX".to_string()) {
                    spawn_body_prefetch(engine.clone(), id.clone(), "INBOX".to_string());
                }
            }
            Ok(json!({ "ok": true }))
        }

        // Override Sent-copy behavior. Null removes the override so provider
        // defaults apply; true/false force or suppress IMAP APPEND after SMTP.
        "account.setSaveSentCopy" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let value = match p.get("value") {
                Some(Value::Bool(enabled)) => Some(json!(enabled)),
                Some(Value::Null) | None => None,
                _ => return Err(anyhow::anyhow!("value must be true, false, or null")),
            };
            store::set_account_pref_json(&engine.db.lock().unwrap(), &id, "save_sent_copy", value)?;
            Ok(json!({ "ok": true }))
        }

        // The host OS resumed from suspend. Connections held across sleep are
        // likely dead but look fresh (monotonic clock froze), so drop pooled
        // sessions and wake every IDLE watcher to reconnect, rather than waiting
        // out TCP keepalive / the IDLE timeout with no mail being pushed.
        "system.resumed" => {
            engine.clear_all_pools();
            engine.resume_signal.notify_waiters();
            Ok(json!({ "ok": true }))
        }

        // Set the RSS automatic sync interval. Stored in minutes so the UI and
        // scheduler can read it from account.list without sidecar state.
        "account.setRSSSyncInterval" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let minutes = req_u32(p, "minutes")?.clamp(5, 1440) as u64;
            store::set_account_pref_u64(
                &engine.db.lock().unwrap(),
                &id,
                "rss_sync_interval_minutes",
                minutes,
            )?;
            Ok(json!({ "ok": true, "minutes": minutes }))
        }

        // Reorder accounts in the database.
        "account.reorder" => {
            let ids = req_str_array(p, "accounts")?;
            store::reorder_accounts(&engine.db.lock().unwrap(), &ids)?;
            Ok(json!({ "ok": true }))
        }

        // Start watching one account folder over IMAP IDLE. IMAP IDLE is per
        // selected mailbox, so kanban starts visible non-INBOX folders here while
        // account startup keeps INBOX watched.
        "watch.start" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            if engine.is_paused(&account) {
                return Ok(json!({ "ok": true, "paused": true }));
            }
            let started = start_idle_watch(engine.clone(), out.clone(), account, folder);
            Ok(json!({ "ok": true, "already": !started }))
        }

        "watch.stop" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let removed = engine
                .watched
                .lock()
                .unwrap()
                .remove(&watch_key(&account, &folder));
            if removed {
                engine.pause_signal.notify_waiters();
            }
            Ok(json!({ "ok": true, "stopped": removed }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}

/// Decode an opaque RSS pagination cursor `"ts:<i64>:<item_key>"`.
fn parse_rss_cursor(raw: &str) -> Option<(i64, String)> {
    let rest = raw.strip_prefix("ts:")?;
    let (ts, key) = rest.split_once(':')?;
    Some((ts.parse().ok()?, key.to_string()))
}

/// Whether an account is RSS-backed (vs mail), per its row in the unified DB.
fn is_rss(engine: &Arc<Engine>, account: &str) -> anyhow::Result<bool> {
    Ok(store::account_engine(&engine.db.lock().unwrap(), account)?.as_deref() == Some("rss"))
}

/// Parse the optional `attachments` array. An entry that fails to deserialize
/// is a hard error: skipping it would send/save the message without its file
/// while reporting success.
fn opt_attachments(params: &Value) -> anyhow::Result<Vec<smtp::AttachmentInput>> {
    match params.get("attachments") {
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|val| {
                serde_json::from_value::<smtp::AttachmentInput>(val.clone())
                    .map_err(|err| anyhow::anyhow!("invalid attachment: {err}"))
            })
            .collect(),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err(anyhow::anyhow!("attachments must be an array")),
    }
}

fn req_str(params: &Value, key: &str) -> anyhow::Result<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("missing string param: {key}"))
}

fn req_bool(params: &Value, key: &str) -> anyhow::Result<bool> {
    params
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow::anyhow!("missing bool param: {key}"))
}

fn req_u16(params: &Value, key: &str) -> anyhow::Result<u16> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .map(|n| n as u16)
        .ok_or_else(|| anyhow::anyhow!("missing number param: {key}"))
}

fn req_u32(params: &Value, key: &str) -> anyhow::Result<u32> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .map(|n| n as u32)
        .ok_or_else(|| anyhow::anyhow!("missing number param: {key}"))
}

/// A certificate pin parameter: hex, normalized, with blank treated as absent.
fn cert_pin_param(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|pin| !pin.is_empty())
        .map(|pin| pin.to_ascii_lowercase())
}

fn req_str_array(params: &Value, key: &str) -> anyhow::Result<Vec<String>> {
    let arr = params
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing array param: {key}"))?;
    let mut out = Vec::new();
    for v in arr {
        if let Some(s) = v.as_str() {
            out.push(s.to_string());
        } else {
            return Err(anyhow::anyhow!("array element is not a string"));
        }
    }
    Ok(out)
}

/// IMAP APPEND a freshly-sent message to the account's Sent folder, with
/// `\Seen`. Best-effort: callers log and ignore errors so SMTP success doesn't
/// surface as "send failed" when the server's Sent folder is unusual.
///
/// After the APPEND succeeds we also fetch the most recent envelopes from the
/// Sent folder and upsert them into the local store. Without this, the just-
/// sent message would only land in the DB at the next periodic sync — meaning
/// it wouldn't appear in the thread view until the user reconnects or refreshes.
/// Resolve the outgoing From address + display name for a send/draft, deferring
/// to the shared store rule so desktop and mobile accept the same identities.
/// An unowned address is an error, surfaced to the composer rather than sent
/// under a substituted sender.
fn resolve_send_from(
    engine: &Arc<Engine>,
    account: &str,
    creds: &imap::Creds,
    requested_from: &str,
) -> anyhow::Result<(String, String)> {
    let db = engine.db.lock().unwrap();
    store::resolve_send_from(&db, account, &creds.user, requested_from)
}

async fn write_line(out: &Writer, value: Value) {
    let mut line = value.to_string();
    line.push('\n');
    let mut guard = out.lock().await;
    let _ = guard.write_all(line.as_bytes()).await;
    let _ = guard.flush().await;
}

async fn emit(out: &Writer, name: &str, detail: Value) {
    write_line(out, json!({ "event": name, "detail": detail })).await;
}

async fn respond(out: &Writer, id: u64, result: Value) {
    write_line(out, json!({ "id": id, "result": result })).await;
}

async fn respond_error(out: &Writer, id: u64, message: &str) {
    write_line(out, json!({ "id": id, "error": { "message": message } })).await;
}
