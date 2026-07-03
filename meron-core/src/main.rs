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
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdout};
use tokio::sync::Mutex;

// The binary shares the library crate's modules (rather than recompiling its own
// copies) so the desktop Engine and the mobile FFI operate on identical types.
use meron_core::engine::*;
use meron_core::engine::{Engine, EngineHost};
use meron_core::protocol::{Request, ping_response, ready_event};
use meron_core::{changelog, imap, rss, secrets, smtp, store};

/// Shared, serialized writer so responses and events never interleave on stdout.
type Writer = Arc<Mutex<Stdout>>;

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
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            sync_messages(&engine, &account, &folder, limit),
        )
        .await;
        engine.syncing.lock().unwrap().remove(&key);
        match result {
            Ok(Ok(synced)) => {
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
                if let Some((count, latest)) =
                    new_unread_inbox_summary(&engine, &account, uid_next_before, uid_next_after)
                {
                    let (from, subject, thread_key) =
                        (display_from(&latest), latest.subject, latest.thread_key);
                    emit(
                        &out,
                        "mail.newMessages",
                        json!({
                            "account": account,
                            "accountName": account_label(&engine, &account),
                            "folder": "inbox",
                            "count": count,
                            "muted": engine.is_muted(&account),
                            "from": from,
                            "subject": subject,
                            "threadKey": thread_key,
                        }),
                    )
                    .await;
                    return;
                }
                emit(
                    &out,
                    "mail.synced",
                    json!({ "account": account, "folder": folder, "synced": synced }),
                )
                .await
            }
            Ok(Err(e)) => {
                emit(
                    &out,
                    "mail.syncError",
                    json!({ "account": account, "message": format!("sync {folder}: {e:#}") }),
                )
                .await
            }
            Err(_) => {
                emit(
                    &out,
                    "mail.syncError",
                    json!({ "account": account, "message": format!("sync {folder}: timed out") }),
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
                if new_items > 0 {
                    // New feed entries: notify like fresh mail (toast + reload + OS
                    // notification) instead of a silent refresh.
                    let (from, subject, thread_key) = latest_rss_header(&engine, &account)
                        .unwrap_or_else(|| {
                            (
                                "RSS Feed".to_string(),
                                "New feed entry".to_string(),
                                String::new(),
                            )
                        });
                    emit(
                        &out,
                        "mail.newMessages",
                        json!({
                            "account": account,
                            "accountName": account_label(&engine, &account),
                            "folder": "inbox",
                            "count": new_items,
                            "muted": engine.is_muted(&account),
                            "from": from,
                            "subject": subject,
                            "threadKey": thread_key,
                        }),
                    )
                    .await
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
        let result =
            tokio::time::timeout(Duration::from_secs(30), sync_folders(&engine, &account)).await;
        engine.syncing.lock().unwrap().remove(&key);
        match result {
            Ok(Ok(_)) => {
                emit(
                    &out,
                    "mail.synced",
                    json!({ "account": account, "folders": true }),
                )
                .await
            }
            Ok(Err(e)) => {
                emit(
                    &out,
                    "mail.syncError",
                    json!({ "account": account, "message": format!("folders sync: {e:#}") }),
                )
                .await
            }
            Err(_) => {
                emit(
                    &out,
                    "mail.syncError",
                    json!({ "account": account, "message": "folders sync timed out" }),
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
fn new_unread_inbox_summary(
    engine: &Arc<Engine>,
    account: &str,
    uid_next_before: u32,
    uid_next_after: u32,
) -> Option<(u32, imap::MessageHeader)> {
    if uid_next_before == 0 || uid_next_after <= uid_next_before {
        return None;
    }

    let db = engine.db.lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key
             FROM messages
             WHERE account = ?1 AND folder = 'INBOX'
               AND uid >= ?2 AND uid < ?3 AND seen = 0
             ORDER BY uid DESC",
        )
        .ok()?;
    let rows = stmt
        .query_map(
            rusqlite::params![account, uid_next_before as i64, uid_next_after as i64],
            |row| {
                let uid = row.get(0)?;
                Ok(imap::MessageHeader {
                    uid,
                    subject: row.get(1)?,
                    from_name: row.get(2)?,
                    from_addr: row.get(3)?,
                    date: row.get(4)?,
                    seen: row.get::<_, i64>(5)? != 0,
                    starred: row.get::<_, i64>(6)? != 0,
                    thread_key: row
                        .get::<_, Option<String>>(7)?
                        .filter(|key| !key.is_empty())
                        .unwrap_or_else(|| format!("uid:{uid}")),
                    ..Default::default()
                })
            },
        )
        .ok()?;
    let headers = rows.collect::<rusqlite::Result<Vec<_>>>().ok()?;
    let latest = headers.first()?.clone();
    Some((headers.len() as u32, latest))
}

/// Newest stored RSS item for an account, or None if the store is empty
/// or the query fails. Used to enrich `mail.newMessages` with the latest
/// sender/subject so OS notifications can show something more useful than a
/// bare count.
fn latest_rss_header(engine: &Arc<Engine>, account: &str) -> Option<(String, String, String)> {
    let db = engine.db.lock().unwrap();
    let mut stmt = db
        .prepare(
            "SELECT
                COALESCE(m.subject, '(no subject)'),
                COALESCE(NULLIF(s.title, ''), NULLIF(m.from_name, ''), 'RSS Feed') AS feed_title,
                COALESCE(m.folder, '')
             FROM messages m
             LEFT JOIN subscriptions s ON m.account = s.account AND m.folder = s.id
             WHERE m.account = ?1
             ORDER BY m.date DESC, m.id DESC
             LIMIT 1",
        )
        .ok()?;
    stmt.query_row(rusqlite::params![account], |row| {
        Ok((
            row.get::<_, String>(1)?, // from (feed_title)
            row.get::<_, String>(0)?, // subject
            row.get::<_, String>(2)?, // thread_key (folder / subscription_id)
        ))
    })
    .ok()
}

/// Friendly display name or email address of an account for user-facing notifications.
fn account_label(engine: &Arc<Engine>, account: &str) -> String {
    let db = engine.db.lock().unwrap();
    let stmt = db
        .prepare("SELECT display_name, email FROM accounts WHERE id = ?1")
        .ok();
    if let Some(mut s) = stmt
        && let Ok((display_name, email)) = s.query_row(rusqlite::params![account], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
    {
        let mail = email.trim();
        if !mail.is_empty() {
            return mail.to_string();
        }
        let label = display_name.trim();
        if !label.is_empty() {
            return label.to_string();
        }
    }
    account.to_string()
}

/// Friendly sender label: the From display name if present, else the address,
/// else an empty string. Mirrors how the message list renders the From column.
fn display_from(h: &imap::MessageHeader) -> String {
    let name = h.from_name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    h.from_addr.trim().to_string()
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
            return;
        }
        // Stop checking while paused; account.setPaused respawns us on resume.
        if engine.is_paused(&account) {
            engine.watched.lock().unwrap().remove(&key);
            return;
        }
        if !engine.watched.lock().unwrap().contains(&key) {
            return;
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
        new_unread_inbox_summary(engine, account, uid_next_before, uid_next_after)
    } else {
        None
    };

    if let Some((count, latest)) = new_inbox {
        // New arrivals: warm their bodies so the first open is instant.
        spawn_body_prefetch(engine.clone(), account.to_string(), "INBOX".to_string());
        let (from, subject, thread_key) =
            (display_from(&latest), latest.subject, latest.thread_key);
        emit(
            out,
            "mail.newMessages",
            json!({
                "account": account,
                "accountName": account_label(engine, account),
                "folder": "inbox",
                "count": count,
                "muted": engine.is_muted(account),
                "from": from,
                "subject": subject,
                "threadKey": thread_key,
            }),
        )
        .await;
    } else {
        if !is_inbox {
            spawn_body_prefetch(engine.clone(), account.to_string(), folder.to_string());
        }
        // Flag-only change: refresh the UI silently, no "new mail" toast.
        emit(
            out,
            "mail.synced",
            json!({ "account": account, "folder": folder, "synced": synced }),
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
    let out: Writer = Arc::new(Mutex::new(tokio::io::stdout()));
    let engine = match Engine::new(Box::new(DesktopHost)) {
        Ok(engine) => Arc::new(engine),
        Err(e) => {
            emit(
                &out,
                "error",
                json!({ "message": format!("store init: {e:#}") }),
            )
            .await;
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
            Ok(json!({ "ok": true }))
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
            let host = req_str(p, "host")?;
            let creds = imap::Creds {
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
            };
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
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
            let name = req_str(p, "name")?.trim().to_string();
            if name.is_empty() {
                return Err(anyhow::anyhow!("Folder name is required"));
            }

            engine
                .with_write_session(&account, |session| {
                    let name = name.clone();
                    Box::pin(async move { imap::create_folder(session, &name).await })
                })
                .await?;

            let folder = imap::Folder {
                name,
                delimiter: None,
                ..Default::default()
            };
            {
                let db = engine.db.lock().unwrap();
                store::upsert_folders(&db, &account, std::slice::from_ref(&folder))?;
            }
            Ok(json!({ "folders": serde_json::to_value(vec![folder])? }))
        }

        // RSS returns final thread Message JSON under "threads"; mail returns raw
        // rows under "messages" the bridge groups into threads.
        "messages.recent" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                let query = req_str(p, "query").unwrap_or_default();
                let limit = req_u16(p, "limit").unwrap_or(50) as i64;
                let threads = rss::recent(&engine.db.lock().unwrap(), &account, &query, limit)?;
                if p.get("refresh").and_then(Value::as_bool).unwrap_or(true) {
                    spawn_rss_sync(engine.clone(), out.clone(), account);
                }
                return Ok(json!({ "threads": threads }));
            }
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let limit = req_u16(p, "limit").unwrap_or(50) as u32;
            let before_cursor = p
                .get("before_cursor")
                .and_then(Value::as_str)
                .and_then(parse_mail_cursor);
            let query = req_str(p, "query").unwrap_or_default();
            let query = query.trim();
            let filter = req_str(p, "filter").unwrap_or_default();
            let refresh = p.get("refresh").and_then(Value::as_bool).unwrap_or(true);
            let (mut messages, next_cursor) = if filter == "starred" && query.is_empty() {
                let folders = starred_search_folders(engine, &account, &folder).await;
                (
                    search_starred_mail_messages(engine, &account, &folders, limit, refresh)
                        .await?,
                    None,
                )
            } else if query.is_empty() {
                store::get_recent_page(
                    &engine.db.lock().unwrap(),
                    &account,
                    &folder,
                    limit,
                    before_cursor,
                    filter == "unread",
                )?
            } else {
                // Chat-view search spans the selected folder plus Sent, so a
                // lookup surfaces both received and self-sent mail (and old
                // messages filed under Sent), not just the current mailbox.
                let mut folders = vec![folder.clone()];
                if let Some(sent) = cached_sent_folder(engine, &account, &folder) {
                    folders.push(sent);
                }
                (
                    search_mail_messages(engine, &account, &folders, query, limit).await?,
                    None,
                )
            };
            // Rewrite each card's identity to the correspondent so a thread shows the
            // same person/avatar in every folder (outbound copies show the recipient).
            store::apply_card_identity(&engine.db.lock().unwrap(), &account, &mut messages);
            if before_cursor.is_none() && filter != "starred" && query.is_empty() && refresh {
                spawn_message_sync(engine.clone(), out.clone(), account, folder, limit);
            }
            let mut out = json!({ "messages": serde_json::to_value(messages)? });
            if let Some(cursor) = next_cursor {
                out.as_object_mut()
                    .unwrap()
                    .insert("next_cursor".into(), Value::String(cursor));
            }
            Ok(out)
        }

        // Every starred item across all accounts, local cache only (the
        // IMAP-backed starred filter keeps mail flags fresh; no round-trip
        // here). Mail returns raw rows the bridge shapes into Messages
        // (id minting lives there); RSS returns final Message JSON.
        "starred.items" => {
            let limit = req_u32(p, "limit").unwrap_or(200);
            let db = engine.db.lock().unwrap();
            let mail = store::get_starred_all_accounts(&db, limit)?
                .into_iter()
                .map(|(account, header)| {
                    let mut row = serde_json::to_value(header).unwrap_or_else(|_| json!({}));
                    row["account"] = Value::String(account);
                    row
                })
                .collect::<Vec<_>>();
            let rss = rss::starred_items(&db, limit as i64)?;
            Ok(json!({ "mail": mail, "rss": rss }))
        }

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
            let attachments: Vec<smtp::AttachmentInput> = match p.get("attachments") {
                Some(Value::Array(arr)) => {
                    let mut list = Vec::new();
                    for val in arr {
                        if let Ok(att) =
                            serde_json::from_value::<smtp::AttachmentInput>(val.clone())
                        {
                            list.push(att);
                        }
                    }
                    list
                }
                _ => Vec::new(),
            };
            let requested_from = req_str(p, "from").unwrap_or_default();
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from);
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
            let attachments: Vec<smtp::AttachmentInput> = match p.get("attachments") {
                Some(Value::Array(arr)) => {
                    let mut list = Vec::new();
                    for val in arr {
                        if let Ok(att) =
                            serde_json::from_value::<smtp::AttachmentInput>(val.clone())
                        {
                            list.push(att);
                        }
                    }
                    list
                }
                _ => Vec::new(),
            };
            let requested_from = req_str(p, "from").unwrap_or_default();
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from);
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
            let deleted = engine
                .with_write_session(&account, |session| {
                    let draft_id = draft_id.clone();
                    Box::pin(async move {
                        let drafts = imap::find_drafts_folder(session)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                        imap::discard_draft(session, &drafts, &draft_id).await
                    })
                })
                .await?;
            Ok(json!({ "ok": true, "deleted": deleted, "permanent": true }))
        }

        "messages.read" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let uid = req_u32(p, "uid")?;

            let message = read_cached_or_fetch(engine, &account, &folder, uid).await?;
            Ok(json!({ "message": serde_json::to_value(message)? }))
        }

        "messages.thread" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let thread_key = req_str(p, "thread_key")?;
            // Pagination is opt-in: callers that don't pass `limit` get the
            // full thread (preserves the markRead full-scan path in app.go).
            let limit = p.get("limit").and_then(Value::as_u64).map(|n| n as u32);
            let before_cursor = p
                .get("before_cursor")
                .and_then(Value::as_str)
                .and_then(|s| s.strip_prefix("uid:"))
                .and_then(|s| s.parse::<u32>().ok());

            // Thread view spans folders within the account so the user's own
            // Sent replies appear alongside the inbox messages they thread with.
            // Each header carries its source `folder` for the body fetch.
            //
            // Exception: a synthetic `uid:N` key (a message with no real
            // threading headers — e.g. a freshly-saved draft) is folder-local,
            // because UIDs are folder-scoped. Spanning folders would match an
            // unrelated message that happens to share UID N elsewhere and render
            // the wrong thread, so keep these scoped to the requested folder.
            let headers = if thread_key.starts_with("uid:") {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
            } else {
                // For UI reads (limit present), pull in any referenced ancestor
                // messages missing from the local cache so the reader shows the
                // full conversation instead of just the synced tail or a lone
                // draft. This runs in the background — the gap fill opens an IMAP
                // connection, which used to block the read and made opening any
                // thread with missing ancestors slow. We return the cached thread
                // immediately; if the fill finds anything it emits `mail.synced`
                // and the reader re-reads. The markRead full-scan path (no limit)
                // skips this entirely.
                if limit.is_some() {
                    maybe_spawn_fill_thread_gaps(engine, out, &account, &thread_key);
                }
                let db = engine.db.lock().unwrap();
                store::get_thread_headers_all_folders(&db, &account, &thread_key)?
            };

            // `before_cursor` / `limit` are honored as a date-ordered slice so
            // the cross-folder query stays consistent with the old per-folder
            // pagination contract. The cursor format ("uid:N") is retained for
            // backwards compatibility but now indexes into the cross-folder
            // result list rather than a per-folder UID space.
            let (headers, next_cursor) = if let Some(limit) = limit {
                let mut headers = headers;
                if let Some(cursor) = before_cursor
                    && let Some(idx) = headers.iter().position(|h| h.uid == cursor)
                {
                    headers.truncate(idx);
                }
                let total = headers.len();
                let limit_usize = limit as usize;
                let start = total.saturating_sub(limit_usize);
                let page = headers[start..].to_vec();
                let next_cursor = if start > 0 {
                    page.first().map(|h| format!("uid:{}", h.uid))
                } else {
                    None
                };
                (page, next_cursor)
            } else {
                (headers, None)
            };

            let mut messages = Vec::with_capacity(headers.len());
            let mut seen_message_ids = HashSet::new();
            for header in headers {
                // Use the header's folder when present (cross-folder query
                // populates it); fall back to the requested folder otherwise.
                let msg_folder = if header.folder.is_empty() {
                    folder.as_str()
                } else {
                    header.folder.as_str()
                };
                let message =
                    read_cached_or_fetch(engine, &account, msg_folder, header.uid).await?;
                // Newly synced envelope rows do not have json.message_id yet, so
                // the SQL-level cross-folder dedupe cannot collapse a
                // self-addressed Sent/Inbox pair on the first thread read. Once
                // read_cached_or_fetch has fetched and cached the full message,
                // collapse later copies before returning the response.
                let message_id_key = message.message_id.trim().to_ascii_lowercase();
                if !message_id_key.is_empty() && !seen_message_ids.insert(message_id_key) {
                    continue;
                }
                messages.push(json!({
                    "uid": header.uid,
                    "seen": header.seen,
                    "starred": header.starred,
                    "folder": msg_folder,
                    "message": serde_json::to_value(message)?,
                }));
            }
            let mut out = json!({ "messages": messages });
            if let Some(cursor) = next_cursor {
                out.as_object_mut()
                    .unwrap()
                    .insert("next_cursor".into(), Value::String(cursor));
            }
            Ok(out)
        }

        "messages.threadHeaders" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let thread_key = req_str(p, "thread_key")?;
            let headers = {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
            };
            let headers = headers
                .into_iter()
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
            let thread_key = req_str(p, "thread_key").unwrap_or_default();
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
            } else {
                // Only touch the thread's messages whose flag actually differs.
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
                    .into_iter()
                    .filter(|header| header.seen != seen)
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
            };

            if !uids.is_empty() {
                let _ = engine
                    .with_write_session(&account, |session| {
                        let folder = folder.clone();
                        let uids = uids.clone();
                        Box::pin(async move { imap::set_seen(session, &folder, &uids, seen).await })
                    })
                    .await;
            }

            {
                let db = engine.db.lock().unwrap();
                if thread_key.is_empty() {
                    for marked_uid in &uids {
                        store::update_message_seen(&db, &account, &folder, *marked_uid, seen)?;
                    }
                } else {
                    store::update_thread_seen(&db, &account, &folder, &thread_key, seen)?;
                }
            }
            Ok(json!({ "ok": true }))
        }

        "messages.markStarred" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let thread_key = req_str(p, "thread_key").unwrap_or_default();
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
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
            };

            if !uids.is_empty() {
                let _ = engine
                    .with_write_session(&account, |session| {
                        let folder = folder.clone();
                        let uids = uids.clone();
                        Box::pin(async move {
                            imap::set_starred(session, &folder, &uids, starred).await
                        })
                    })
                    .await;
            }

            {
                let db = engine.db.lock().unwrap();
                if thread_key.is_empty() {
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
            Ok(json!({ "ok": true }))
        }

        "messages.delete" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let thread_key = req_str(p, "thread_key").unwrap_or_default();
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

            let uids = if !explicit_uids.is_empty() {
                explicit_uids
            } else if thread_key.is_empty() {
                uid.into_iter().collect::<Vec<_>>()
            } else {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
                    .into_iter()
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
            };

            if uids.is_empty() {
                return Ok(json!({ "ok": true, "deleted": 0 }));
            }

            // Returns None for a permanent expunge (drafts, or items already in
            // Trash), or Some(trash) when the thread was moved to Trash.
            // Mutating, so it never auto-retries.
            let trashed = engine
                .with_write_session(&account, |session| {
                    let folder = folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        // Drafts are discarded permanently (expunge in place) rather
                        // than moved to Trash: they're unsent and ephemeral, and a
                        // `\Draft` copy in Trash is confusing and handled
                        // inconsistently by servers.
                        let drafts = imap::find_drafts_folder(session).await?;
                        if drafts.as_deref() == Some(folder.as_str()) {
                            imap::expunge_uids(session, &folder, &uids).await?;
                            return anyhow::Ok(None);
                        }
                        let trash = imap::find_trash_folder(session).await?.ok_or_else(|| {
                            anyhow::anyhow!("Trash folder not found for this account")
                        })?;
                        if trash == folder {
                            imap::expunge_uids(session, &folder, &uids).await?;
                            return anyhow::Ok(None);
                        }
                        imap::move_to_folder(session, &folder, &trash, &uids).await?;
                        anyhow::Ok(Some(trash))
                    })
                })
                .await?;

            let deleted = {
                let db = engine.db.lock().unwrap();
                store::delete_messages_by_uid(&db, &account, &folder, &uids)?
            };
            match trashed {
                None => Ok(json!({ "ok": true, "deleted": deleted, "permanent": true })),
                Some(trash) => Ok(json!({ "ok": true, "deleted": deleted, "trash": trash })),
            }
        }

        "messages.move" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let target_folder = canon_folder(&req_str(p, "target_folder")?);
            let thread_key = req_str(p, "thread_key").unwrap_or_default();
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

            if folder == target_folder {
                return Ok(
                    json!({ "ok": true, "moved": 0, "source_folder": folder, "target_folder": target_folder }),
                );
            }

            let uids = if !explicit_uids.is_empty() {
                explicit_uids
            } else if thread_key.is_empty() {
                uid.into_iter().collect::<Vec<_>>()
            } else {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
                    .into_iter()
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
            };

            if uids.is_empty() {
                return Ok(
                    json!({ "ok": true, "moved": 0, "source_folder": folder, "target_folder": target_folder }),
                );
            }

            let target_batch = engine
                .with_write_session(&account, |session| {
                    let folder = folder.clone();
                    let target_folder = target_folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::move_to_folder(session, &folder, &target_folder, &uids).await?;
                        imap::fetch_recent(session, &target_folder, 50.max(uids.len() as u32))
                            .await
                            .context("refresh target folder after move")
                    })
                })
                .await?;

            let moved = {
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
                store::delete_messages_by_uid(&db, &account, &folder, &uids)?
            };
            Ok(
                json!({ "ok": true, "moved": moved, "source_folder": folder, "target_folder": target_folder }),
            )
        }

        "messages.copy" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let target_account = req_str(p, "target_account")?;
            let target_folder = canon_folder(&req_str(p, "target_folder")?);
            let thread_key = req_str(p, "thread_key").unwrap_or_default();
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

            let uids = if !explicit_uids.is_empty() {
                explicit_uids
            } else if thread_key.is_empty() {
                uid.into_iter().collect::<Vec<_>>()
            } else {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
                    .into_iter()
                    .map(|header| header.uid)
                    .collect::<Vec<_>>()
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
            let target_batch = engine
                .with_write_session(&target_account, |session| {
                    let target_folder = target_folder.clone();
                    let raw_messages = raw_messages.clone();
                    Box::pin(async move {
                        for message in &raw_messages {
                            imap::append_copied_message(session, &target_folder, message).await?;
                        }
                        imap::fetch_recent(
                            session,
                            &target_folder,
                            50.max(raw_messages.len() as u32),
                        )
                        .await
                        .context("refresh target folder after copy")
                    })
                })
                .await?;

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
                let _ = engine
                    .with_write_session(&account, |session| {
                        let folder = folder.clone();
                        let uids = uids.clone();
                        Box::pin(async move { imap::set_seen(session, &folder, &uids, true).await })
                    })
                    .await;
            }

            {
                let db = engine.db.lock().unwrap();
                store::mark_folder_seen(&db, &account, &folder, true)?;
            }
            Ok(json!({ "ok": true }))
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

/// Mail list cursor: `date:<epoch>:<uid>` — the (send time, uid) keyset of the
/// last row of the previous page (see `store::get_recent_page`).
fn parse_mail_cursor(raw: &str) -> Option<(i64, u32)> {
    let rest = raw.strip_prefix("date:")?;
    let (date, uid) = rest.split_once(':')?;
    Some((date.parse().ok()?, uid.parse().ok()?))
}

/// Whether an account is RSS-backed (vs mail), per its row in the unified DB.
fn is_rss(engine: &Arc<Engine>, account: &str) -> anyhow::Result<bool> {
    Ok(store::account_engine(&engine.db.lock().unwrap(), account)?.as_deref() == Some("rss"))
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
/// Resolve the outgoing From address + display name for a send/draft. Only a
/// requested address the user actually owns is honored — the primary login or a
/// configured alias — otherwise we fall back to the primary. An alias with its
/// own display name overrides the account's sender name.
fn resolve_send_from(
    engine: &Arc<Engine>,
    account: &str,
    creds: &imap::Creds,
    requested_from: &str,
) -> (String, String) {
    let (sender_name, aliases) = {
        let db = engine.db.lock().unwrap();
        let sender_name = db
            .query_row(
                "SELECT sender_name FROM accounts WHERE id = ?1",
                rusqlite::params![account],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_default();
        let aliases = store::account_aliases(&db, account).unwrap_or_default();
        (sender_name, aliases)
    };
    let requested = requested_from.trim().to_lowercase();
    if requested.is_empty() || requested == creds.user.trim().to_lowercase() {
        return (creds.user.clone(), sender_name);
    }
    if let Some(alias) = aliases
        .iter()
        .find(|a| a.email.trim().to_lowercase() == requested)
    {
        let name = if alias.name.trim().is_empty() {
            sender_name
        } else {
            alias.name.clone()
        };
        return (alias.email.trim().to_string(), name);
    }
    (creds.user.clone(), sender_name)
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
