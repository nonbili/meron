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

use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdout};
use tokio::sync::Mutex;

mod sidecar;

// The binary shares the library crate's modules (rather than recompiling its own
// copies) so the desktop Engine and the mobile FFI operate on identical types.
use meron_core::engine::*;
use meron_core::engine::{Engine, EngineHost};
use meron_core::protocol::{Request, ping_response, ready_event};
use meron_core::{imap, secrets, store};
use sidecar::dispatch::dispatch;
use sidecar::idle::{poll_notify_folders, start_idle_watch, start_notify_folder_watches};
use sidecar::prefs::activate_pref_session;

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

    // List cards name attachments from a column older caches lack; fill it
    // behind the UI rather than in the migration that added it.
    spawn_files_backfill(engine.clone());

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
        start_notify_folder_watches(&engine, &out, &account);
        start_idle_watch(engine.clone(), out.clone(), account, "INBOX".to_string());
    }

    tokio::spawn(poll_notify_folders(engine.clone(), out.clone()));

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
                // Claiming write ordering happens here, in arrival order, and
                // not on the spawned task: two boot reads racing as tasks could
                // otherwise activate out of order, leaving the reloaded window's
                // writes rejected in favour of the session it replaced.
                if req.method == "app.prefsGet" {
                    activate_pref_session(
                        req.params
                            .get("session")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    );
                }
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

async fn write_line(out: &Writer, value: Value) {
    let started = std::time::Instant::now();
    let id = value.get("id").and_then(Value::as_u64);
    let event = value.get("event").and_then(Value::as_str).unwrap_or("");
    let mut line = value.to_string();
    line.push('\n');
    let serialized = started.elapsed();
    let lock_started = std::time::Instant::now();
    let mut guard = out.lock().await;
    let lock_wait = lock_started.elapsed();
    let write_started = std::time::Instant::now();
    let write_result = guard.write_all(line.as_bytes()).await;
    let write_time = write_started.elapsed();
    let flush_started = std::time::Instant::now();
    let flush_result = guard.flush().await;
    let flush_time = flush_started.elapsed();
    drop(guard);
    if started.elapsed().as_millis() >= 100 {
        eprintln!(
            "meron-core: response timing: id={id:?} event={event} bytes={} serialize_ms={} output_lock_wait_ms={} write_ms={} flush_ms={} total_ms={} failed={}",
            line.len(),
            serialized.as_millis(),
            lock_wait.as_millis(),
            write_time.as_millis(),
            flush_time.as_millis(),
            started.elapsed().as_millis(),
            write_result.is_err() || flush_result.is_err()
        );
    }
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
