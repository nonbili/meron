//! Shared mail Engine: per-account IMAP session pool, OAuth token refresh, and
//! the sync/search/fetch operations used by both the desktop sidecar and the
//! mobile FFI host. Platform differences (where the SQLite DB lives, where
//! per-account secrets are stored) are injected via the [`EngineHost`] trait.

use anyhow::Context as _;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};

use crate::{imap, secrets, store};

mod append;
mod background_sync;
mod folders;
mod prefetch;
#[cfg(test)]
pub(crate) use prefetch::transfer_clock_tests::slow_fetch_engine;
mod read;
mod search;
mod sync;

pub use append::*;
pub use background_sync::{BackgroundSyncCancelled, BackgroundSyncTimedOut, retry_background_sync};
pub use folders::*;
pub use prefetch::*;
pub use read::*;
pub use search::*;
pub use sync::*;

pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub const OUTLOOK_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

pub const OUTLOOK_SCOPES: &str = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid email profile";

#[derive(Clone, Default)]
pub struct OAuthDefaults {
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_token_url: String,
    pub outlook_client_id: String,
}

static OAUTH_DEFAULTS: LazyLock<RwLock<OAuthDefaults>> =
    LazyLock::new(|| RwLock::new(OAuthDefaults::default()));

pub fn set_oauth_defaults(defaults: OAuthDefaults) {
    if let Ok(mut current) = OAUTH_DEFAULTS.write() {
        *current = defaults;
    }
}

fn oauth_defaults() -> OAuthDefaults {
    OAUTH_DEFAULTS
        .read()
        .map(|defaults| defaults.clone())
        .unwrap_or_default()
}

/// Engine state: per-account credentials plus the on-disk store.
/// Reads serve from SQLite; syncs reconnect to IMAP and refresh stored rows.
pub struct Engine {
    pub accounts: Arc<Mutex<HashMap<String, imap::Creds>>>,
    pub db: Arc<std::sync::Mutex<rusqlite::Connection>>,
    /// Accounts with a live IDLE task, to avoid spawning duplicates.
    /// Keyed by `account\nfolder` because IMAP IDLE watches one selected mailbox
    /// per connection.
    pub watched: std::sync::Mutex<HashSet<String>>,
    /// In-flight background sync keys, to dedupe concurrent refreshes.
    pub syncing: std::sync::Mutex<HashSet<String>>,
    /// Per-thread set of referenced-but-missing message-ids we've already tried
    /// to fetch this session, keyed by `account|thread_key`. A negative cache:
    /// most gaps are permanent (ancestors that were never delivered to this
    /// mailbox, e.g. GitHub notification threads), so without this every thread
    /// open would re-open an IMAP connection to re-search for them. Cleared on
    /// restart, which lets a genuinely-late ancestor be retried.
    pub gap_attempts: std::sync::Mutex<HashMap<String, HashSet<String>>>,
    /// Messages with an in-flight background body fetch, keyed by
    /// [`message_key`], so re-reads cannot duplicate a download while other
    /// pages and new messages in the thread remain independently fetchable.
    pub body_fetches: std::sync::Mutex<HashSet<String>>,
    /// Uncached-body background failures and when each happened, consumed by
    /// the next read so its UI can show an error without starting an automatic
    /// fetch/notification loop. Only answers for [`BODY_FAILURE_TTL`]: an old
    /// failure says nothing about the connection now.
    pub body_fetch_failures: std::sync::Mutex<HashMap<String, (String, std::time::Instant)>>,
    /// Pulsed when a background body fetch ends, so a read waiting on one of
    /// its messages looks at the cache again.
    pub body_fetch_done: Notify,
    /// Messages ([`message_key`]) whose attachment files are not to be
    /// fetched again yet. `None`: a download arrived and the files were still
    /// missing, or a successful FETCH omitted the UID (expunged). Another
    /// download is not known to help, and an open thread would
    /// otherwise refetch, notify and re-read without end. `Some(until)`: the
    /// download itself failed, so readers stop waiting on it until then.
    /// Cleared for a message once its files are on disk.
    pub media_recovery_hold: std::sync::Mutex<HashMap<String, Option<std::time::Instant>>>,
    /// Messages ([`message_key`]) the prefetcher leaves to the on-demand
    /// reader for the rest of this run: each failed to download, then failed
    /// again straight after other mail had come down on the same connection.
    pub prefetch_skipped: std::sync::Mutex<HashSet<String>>,
    /// Pulsed when an account is paused so live IDLE watchers wake and re-check
    /// their paused state (and stop) instead of blocking up to the IDLE timeout.
    pub pause_signal: Notify,
    /// Pulsed on OS resume (`system.resumed`) so IDLE watchers abandon sockets
    /// that died during suspend and reconnect, instead of blocking up to the
    /// IDLE timeout / TCP keepalive while no new mail is pushed.
    pub resume_signal: Notify,
    /// Warm, authenticated IMAP sessions reused by the request path so each
    /// thread open / search / sync doesn't pay a fresh TLS + LOGIN/XOAUTH2
    /// handshake. Keyed by account; per-account `Vec` is a LIFO free-list
    /// (reuse the hottest session first). IDLE watcher connections are *not*
    /// pooled — they have a different, long-lived lifecycle. See `with_session`.
    pub pool: std::sync::Mutex<HashMap<String, Vec<Pooled<imap::Session>>>>,
    /// Serialize fresh connection setup per account. Without this, startup's
    /// folder, inbox, companion and body tasks can all observe an empty pool and
    /// perform concurrent TLS and authentication handshakes. The lock covers the
    /// handshake only and is dropped before the caller's IMAP operation runs, so
    /// a long prefetch cannot block interactive work behind it — and the wait
    /// for the lock is itself bounded (see `connect_coordination_timeout`), so
    /// the worst an interactive fresh connect pays is that bound.
    connect_locks: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// Platform integration: opens the SQLite store and loads/stores per-account
    /// secrets (OS keychain on desktop, the keyed DB on mobile).
    pub host: Arc<dyn EngineHost>,
}

/// Platform hooks the [`Engine`] needs but that differ between the desktop
/// sidecar and the mobile FFI host: where the SQLite store lives and how
/// per-account secrets are persisted.
pub trait EngineHost: Send + Sync {
    /// Open the engine's SQLite connection (path and cipher key are host-chosen).
    fn open_db(&self) -> anyhow::Result<rusqlite::Connection>;
    /// Apply the account's stored secret onto `creds` (keychain on desktop, keyed
    /// DB on mobile). `conn` is the engine's open store, for hosts that read or
    /// migrate secrets there.
    fn apply_secret(&self, conn: &rusqlite::Connection, account: &str, creds: &mut imap::Creds);
    /// Persist a refreshed secret after an OAuth token refresh.
    fn store_secret(
        &self,
        conn: &rusqlite::Connection,
        account: &str,
        secrets: &secrets::Secrets,
    ) -> anyhow::Result<()>;
}

/// One idle, reusable session plus when it was last returned to the pool, so
/// stale connections (silently dropped by the server) can be evicted on acquire
/// instead of failing an operation. Generic over the session type so the
/// free-list policy ([`pool_take`]/[`pool_return`]) is unit-testable without a
/// live IMAP `Session`.
pub struct Pooled<S> {
    pub session: S,
    pub last_used: std::time::Instant,
}

/// Pop the hottest still-fresh session for `account`, discarding any idle longer
/// than `max_idle`. Pure (caller supplies `now`) so it can be tested directly.
pub fn pool_take<S>(
    map: &mut HashMap<String, Vec<Pooled<S>>>,
    account: &str,
    now: std::time::Instant,
    max_idle: Duration,
) -> Option<S> {
    let list = map.get_mut(account)?;
    while let Some(p) = list.pop() {
        if now.saturating_duration_since(p.last_used) < max_idle {
            return Some(p.session);
        }
        // else: too old to trust — drop it and try the next.
    }
    None
}

/// Trace connection-pool decisions when `MERON_POOL_DEBUG` is set. Off by
/// default so production runs stay quiet. Routes through the logger so the trace
/// is visible on mobile (os_log / Logcat), not just desktop stderr.
pub fn pool_debug(account: &str, what: &str) {
    if std::env::var_os("MERON_POOL_DEBUG").is_some() {
        crate::mlog!(
            crate::log::Level::Debug,
            "engine.pool",
            "{what} account={account}"
        );
    }
}

/// Whether `err` is core's "this account must sign in again" failure, from any
/// of the paths that refuse to use missing or rejected credentials. Hosts get
/// it as an `account.needsReconnect` event; see [`needs_reconnect_event`].
pub fn is_needs_reconnect_error(err: &anyhow::Error) -> bool {
    format!("{err:#}").contains("account needs reconnect")
}

/// The event that tells a host `account` has to be reconnected, when `err` is
/// that failure. Carries the account id itself so hosts never have to recover
/// it from the message text.
pub fn needs_reconnect_event(
    account: &str,
    err: &anyhow::Error,
) -> Option<(&'static str, serde_json::Value)> {
    is_needs_reconnect_error(err).then(|| {
        (
            "account.needsReconnect",
            serde_json::json!({ "account": account }),
        )
    })
}

pub fn creds_have_required_secret(creds: &imap::Creds) -> bool {
    // A secret the provider has refused is as good as none.
    if creds.auth_rejected() {
        return false;
    }
    if creds.is_oauth() {
        creds
            .refresh_token
            .as_deref()
            .is_some_and(|s| !s.is_empty())
            || creds.access_token.as_deref().is_some_and(|s| !s.is_empty())
    } else {
        !creds.password.is_empty()
    }
}

/// Return a session to `account`'s free-list, or drop it if already at
/// `max_pooled` (the transient-overflow case). Pure for testability.
pub fn pool_return<S>(
    map: &mut HashMap<String, Vec<Pooled<S>>>,
    account: &str,
    session: S,
    now: std::time::Instant,
    max_pooled: usize,
) {
    let list = map.entry(account.to_string()).or_default();
    if list.len() < max_pooled {
        list.push(Pooled {
            session,
            last_used: now,
        });
    }
    // else: drop `session` (closes the socket) instead of pooling it.
}

/// Max warm sessions kept per account; extra concurrent requests open a
/// transient connection that is closed (dropped) after use rather than pooled.
pub const MAX_POOLED: usize = 3;

/// Drop a pooled session rather than reuse it once it's been idle this long.
/// Comfortably under typical server idle timeouts (Gmail ~30 min), so a reused
/// session is almost always still alive.
pub const MAX_IDLE: Duration = Duration::from_secs(120);

/// Cap on a single command run against a *pooled* session in a retryable
/// (read-only) op. A pooled connection the server or a NAT silently dropped
/// doesn't error — it hangs until TCP keepalive gives up (60s+), well past the
/// desktop bridge's 30s call timeout, so the user sees a dead UI. Failing the
/// reuse attempt at 10s leaves room for the stale-retry fresh connection to
/// still answer within the bridge budget. Write ops are exempt: cutting off a
/// slow but progressing APPEND/SEND mid-command is worse than waiting.
pub const POOLED_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a pooled session gets to answer the NOOP that precedes a transfer.
/// A live server answers in a round trip; a socket that died over a sleep or a
/// network change says nothing, and every second spent finding that out comes
/// off the download's budget.
const TRANSFER_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// How long a recorded body download failure stands in for a new attempt.
/// Long enough for the re-read its notification triggers; short enough that
/// reconnecting or retrying by hand reaches the server again.
const BODY_FAILURE_TTL: Duration = Duration::from_secs(60);

impl Engine {
    /// Note that a background fetch could not deliver `key`'s body.
    pub(crate) fn record_body_failure(&self, key: String, error: String) {
        let now = std::time::Instant::now();
        let mut failures = self.body_fetch_failures.lock().unwrap();
        failures.retain(|_, (_, at)| now.duration_since(*at) < BODY_FAILURE_TTL);
        failures.insert(key, (error, now));
    }

    /// The recent failure recorded for `key`, if any. Taking it is what lets
    /// the read after this one try the server again.
    pub(crate) fn take_body_failure(&self, key: &str) -> Option<String> {
        let (error, at) = self.body_fetch_failures.lock().unwrap().remove(key)?;
        (at.elapsed() < BODY_FAILURE_TTL).then_some(error)
    }
}

/// The key the engine's per-message bookkeeping files a message under.
pub(crate) fn message_key(account: &str, folder: &str, uid: u32) -> String {
    format!("{}{uid}", folder_key_prefix(account, folder))
}

/// What every [`message_key`] of one folder starts with.
pub(crate) fn folder_key_prefix(account: &str, folder: &str) -> String {
    format!("{account}|{folder}|")
}

/// A body download ran out of its budget. Typed so callers can tell a message
/// that is too slow to fetch from a connection that went away.
#[derive(Debug)]
pub struct TransferTimedOut(pub Duration);

impl std::fmt::Display for TransferTimedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IMAP transfer timed out after {}s", self.0.as_secs())
    }
}

impl std::error::Error for TransferTimedOut {}

/// How [`Engine::run_session`] treats the session it runs `f` on.
#[derive(Clone, Copy)]
enum SessionUse {
    /// Mutating. Never interrupted and never run twice, so a connection
    /// dropped mid-command can't double-apply.
    Write,
    /// Read-only. A pooled session that fails, or stalls past
    /// [`POOLED_READ_TIMEOUT`], is replaced by a fresh one and `f` runs once
    /// more.
    Read,
    /// A read that transfers message bytes, due by `deadline` however many
    /// connections it takes. See [`Engine::with_transfer_session`].
    Transfer {
        deadline: tokio::time::Instant,
        budget: Duration,
    },
}

/// Do not let a wedged fresh handshake make every later operation wait for the
/// per-account connection coordinator indefinitely. Normal startup handshakes
/// serialize; after this, availability wins and the caller connects independently.
/// The bound is deliberately short: a handshake slower than this is exactly the
/// case where an interactive open must not queue behind background prefetch, so
/// coordination is traded away rather than latency. Waiters re-check the pool
/// after the wait, so a leader that finished in time is still reused.
fn connect_coordination_timeout() -> Duration {
    std::cmp::min(
        Duration::from_secs(2),
        background_sync_timeout().mul_f32(0.1),
    )
}

impl Engine {
    pub fn new(host: Box<dyn EngineHost>) -> anyhow::Result<Self> {
        let conn = host.open_db()?;
        // Publish the app-wide proxy before anything can open a socket.
        if let Err(err) = crate::proxy::load_global(&conn) {
            eprintln!("meron-core: could not load the proxy setting: {err:#}");
        }
        let mut accounts: HashMap<String, imap::Creds> = HashMap::new();
        for (id, mut creds) in store::load_accounts(&conn)? {
            host.apply_secret(&conn, &id, &mut creds);
            if !creds_have_required_secret(&creds) {
                eprintln!("meron-core: account {id} needs reconnect; no stored secret found");
                continue;
            }
            accounts.insert(id, creds);
        }
        Ok(Self {
            accounts: Arc::new(Mutex::new(accounts)),
            db: Arc::new(std::sync::Mutex::new(conn)),
            watched: std::sync::Mutex::new(HashSet::new()),
            syncing: std::sync::Mutex::new(HashSet::new()),
            gap_attempts: std::sync::Mutex::new(HashMap::new()),
            body_fetches: std::sync::Mutex::new(HashSet::new()),
            body_fetch_failures: std::sync::Mutex::new(HashMap::new()),
            body_fetch_done: Notify::new(),
            media_recovery_hold: std::sync::Mutex::new(HashMap::new()),
            prefetch_skipped: std::sync::Mutex::new(HashSet::new()),
            pause_signal: Notify::new(),
            resume_signal: Notify::new(),
            pool: std::sync::Mutex::new(HashMap::new()),
            connect_locks: std::sync::Mutex::new(HashMap::new()),
            host: Arc::from(host),
        })
    }

    /// Whether automatic checking is paused for an account (per its stored pref).
    pub fn is_paused(&self, account: &str) -> bool {
        store::account_paused(&self.db.lock().unwrap(), account).unwrap_or(false)
    }

    /// Whether desktop notifications are suppressed for an account.
    pub fn is_muted(&self, account: &str) -> bool {
        store::account_muted(&self.db.lock().unwrap(), account).unwrap_or(false)
    }

    /// Refresh and persist credentials in an owned task. Dropping a read or
    /// transfer must not cancel a refresh after the provider accepted it.
    pub async fn ensure_valid_creds(&self, account: &str) -> anyhow::Result<imap::Creds> {
        let accounts = self.accounts.clone();
        let db = self.db.clone();
        let host = self.host.clone();
        let account = account.to_string();
        tokio::spawn(
            async move { Self::refresh_creds(&accounts, &db, host.as_ref(), &account).await },
        )
        .await?
    }

    async fn refresh_creds(
        accounts: &Mutex<HashMap<String, imap::Creds>>,
        db: &std::sync::Mutex<rusqlite::Connection>,
        host: &dyn EngineHost,
        account: &str,
    ) -> anyhow::Result<imap::Creds> {
        let mut accounts = accounts.lock().await;
        // Lazily hydrate accounts added out-of-band (the mobile host adds/edits
        // accounts through the stateless command path, not this Engine, so a
        // long-lived foreground Engine can miss them). A no-op on desktop, where
        // the dispatch loop keeps `accounts` authoritative.
        if !accounts.contains_key(account) {
            let loaded = store::load_accounts(&db.lock().unwrap())
                .ok()
                .and_then(|rows| rows.into_iter().find(|(id, _)| id == account));
            if let Some((id, mut creds)) = loaded {
                host.apply_secret(&db.lock().unwrap(), &id, &mut creds);
                accounts.insert(id, creds);
            }
        }
        let creds = accounts
            .get_mut(account)
            .ok_or_else(|| anyhow::anyhow!("account needs reconnect: {account}"))?;
        // Already refused once: asking again only repeats the answer.
        if creds.auth_rejected() {
            anyhow::bail!("account needs reconnect: {account}");
        }

        if creds.is_oauth() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            // If token is expired or expires in less than 5 minutes (300s)
            if creds.token_expires_at <= now + 300 {
                let refresh_token = creds.refresh_token.as_deref().unwrap_or("");
                if refresh_token.is_empty() {
                    if creds.access_token.as_deref().is_some_and(|s| !s.is_empty()) {
                        return Ok(creds.clone());
                    }
                    anyhow::bail!("account needs reconnect: {account}");
                }
                // Provider-specific endpoint / credentials. Google needs a client
                // secret; Microsoft is a public client (PKCE) with no secret but
                // must request the resource scopes on refresh.
                let (
                    default_token_url,
                    default_client_id_env,
                    default_client_secret_env,
                    default_scope,
                ): (&str, &str, Option<&str>, Option<&str>) = match creds.auth_type.as_str() {
                    "outlook_oauth" => (
                        OUTLOOK_TOKEN_URL,
                        "MERON_OUTLOOK_CLIENT_ID",
                        None,
                        Some(OUTLOOK_SCOPES),
                    ),
                    _ => (
                        GOOGLE_TOKEN_URL,
                        "MERON_GOOGLE_CLIENT_ID",
                        Some("MERON_GOOGLE_CLIENT_SECRET"),
                        None,
                    ),
                };
                let oauth_defaults = oauth_defaults();
                let default_google_token_url = oauth_defaults.google_token_url.trim();
                let token_url = if !creds.oauth_token_url.trim().is_empty() {
                    creds.oauth_token_url.trim().to_string()
                } else if creds.auth_type != "outlook_oauth" && !default_google_token_url.is_empty()
                {
                    default_google_token_url.to_string()
                } else {
                    default_token_url.to_string()
                };
                let client_id = if creds.oauth_client_id.trim().is_empty() {
                    let configured = match creds.auth_type.as_str() {
                        "outlook_oauth" => oauth_defaults.outlook_client_id.trim(),
                        _ => oauth_defaults.google_client_id.trim(),
                    };
                    if configured.is_empty() {
                        std::env::var(default_client_id_env).with_context(|| {
                            match creds.auth_type.as_str() {
                                "outlook_oauth" => {
                                    "MERON_OUTLOOK_CLIENT_ID is required for Outlook OAuth refresh"
                                }
                                _ => "MERON_GOOGLE_CLIENT_ID is required for Gmail OAuth refresh",
                            }
                        })?
                    } else {
                        configured.to_string()
                    }
                } else {
                    creds.oauth_client_id.trim().to_string()
                };
                let client_secret = if creds.oauth_client_secret.trim().is_empty() {
                    let configured = oauth_defaults.google_client_secret.trim();
                    if !configured.is_empty() {
                        configured.to_string()
                    } else {
                        // No secret anywhere is a valid configuration: mobile's
                        // native Google clients are public (installed-app)
                        // clients that refresh with client_id only, and
                        // `refresh_oauth_token` omits the empty field.
                        match default_client_secret_env {
                            Some(env_name) if token_url == GOOGLE_TOKEN_URL => {
                                std::env::var(env_name).unwrap_or_default()
                            }
                            _ => String::new(),
                        }
                    }
                } else {
                    creds.oauth_client_secret.trim().to_string()
                };
                let owned_scope = creds.oauth_scope.trim().to_string();
                let scope = if owned_scope.is_empty() {
                    default_scope
                } else {
                    Some(owned_scope.as_str())
                };
                let refreshed = imap::refresh_oauth_token(
                    &token_url,
                    &client_id,
                    &client_secret,
                    refresh_token,
                    scope,
                    creds.proxy.resolve(),
                )
                .await;
                let refreshed = match refreshed {
                    Ok(tokens) => tokens,
                    // The grant is dead, not the network: remember it, so the
                    // account shows as needing a reconnect and every later sync
                    // stops here instead of asking the provider again.
                    //
                    // The verdict is filed under the token that earned it, so
                    // an account reconnected while we were asking is unaffected.
                    Err(err) if imap::oauth_grant_rejected(&err) => {
                        let fingerprint = imap::grant_fingerprint(refresh_token);
                        if let Err(save_err) = store::set_account_rejected_grant(
                            &db.lock().unwrap(),
                            account,
                            &fingerprint,
                        ) {
                            eprintln!(
                                "meron-core: could not record rejected sign-in for {account}: {save_err:#}"
                            );
                        }
                        creds.rejected_grant = Some(fingerprint);
                        return Err(err.context(format!("account needs reconnect: {account}")));
                    }
                    Err(err) => return Err(err),
                };
                creds.access_token = Some(refreshed.access_token);
                creds.token_expires_at = now + refreshed.expires_in;
                // A rotated refresh token replaces the one just used; without
                // it the old one stays, as for providers that never rotate.
                if let Some(rotated) = refreshed.refresh_token {
                    creds.refresh_token = Some(rotated);
                }

                // Persist: token_expires_at to SQLite, the new tokens to the
                // host's secret store (keychain on desktop, keyed DB on mobile).
                {
                    let db = db.lock().unwrap();
                    store::save_account_config(&db, account, creds)?;
                    host.store_secret(&db, account, &secrets::Secrets::from_creds(creds))?;
                }
            }
        }

        Ok(creds.clone())
    }

    /// Pop the hottest non-expired pooled session for `account`, discarding any
    /// that have been idle past `MAX_IDLE` or whose socket the server has
    /// already closed.
    pub fn take_pooled(&self, account: &str) -> Option<imap::Session> {
        loop {
            let session = pool_take(
                &mut self.pool.lock().unwrap(),
                account,
                std::time::Instant::now(),
                MAX_IDLE,
            )?;
            if imap::session_looks_open(&session) {
                return Some(session);
            }
            pool_debug(account, "closed-evict");
        }
    }

    /// Return a healthy session to the pool, or drop it if the account is
    /// paused or already at `MAX_POOLED` (the transient-overflow case).
    ///
    /// The paused check must happen here, not only when `account.setPaused`
    /// clears the pool: a background sync can already have a session checked
    /// out when the account is paused and otherwise return that old session
    /// after the clear.
    pub fn return_pooled(&self, account: &str, session: imap::Session) {
        if self.is_paused(account) {
            return;
        }
        pool_return(
            &mut self.pool.lock().unwrap(),
            account,
            session,
            std::time::Instant::now(),
            MAX_POOLED,
        );
    }

    /// Drop every pooled session for `account`. Called when credentials become
    /// invalid (account removed) or checking is paused, so we never reuse a
    /// session built from stale creds.
    pub fn clear_pool(&self, account: &str) {
        self.pool.lock().unwrap().remove(account);
    }

    /// Drop every warm pooled session. Used on OS resume: a session's freshness
    /// is judged by elapsed `Instant`, but the monotonic clock is frozen across
    /// suspend, so a connection dead for hours still looks recently used and
    /// dodges the `MAX_IDLE` eviction. Clearing forces the next op to reconnect.
    pub fn clear_all_pools(&self) {
        self.pool.lock().unwrap().clear();
    }

    fn connect_lock(&self, account: &str) -> Arc<Mutex<()>> {
        self.connect_locks
            .lock()
            .unwrap()
            .entry(account.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Run `f` against a warm pooled session when one is available, otherwise a
    /// freshly connected one. On success the session is returned to the pool.
    ///
    /// `retry`: when a *pooled* session fails (the likely cause being that the
    /// server silently dropped it), reconnect fresh and run `f` once more. Only
    /// safe for read-only operations — a stale pooled connection fails on its
    /// first command (before any mutation), but a connection that drops *after*
    /// a mutating command reached the server must not be retried. Use
    /// [`with_read_session`](Self::with_read_session) /
    /// [`with_write_session`](Self::with_write_session) instead of calling this
    /// directly.
    pub async fn with_session<T, F>(&self, account: &str, retry: bool, f: F) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        let usage = if retry {
            SessionUse::Read
        } else {
            SessionUse::Write
        };
        self.run_session(account, usage, f).await
    }

    async fn run_session<T, F>(
        &self,
        account: &str,
        usage: SessionUse,
        mut f: F,
    ) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        let mut command_attempted = false;
        if let Some(session) = self.take_pooled(account) {
            match self
                .run_pooled(account, usage, session, &mut f, &mut command_attempted)
                .await
            {
                Some(result) => return result,
                // Fall through to a fresh connection and try once more.
                None => pool_debug(account, "stale-retry"),
            }
        }

        pool_debug(account, "fresh-connect");
        let connect_lock = self.connect_lock(account);
        let mut connect_guard = tokio::time::timeout(
            match usage {
                SessionUse::Transfer { deadline, .. } => connect_coordination_timeout()
                    .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
                _ => connect_coordination_timeout(),
            },
            connect_lock.lock(),
        )
        .await
        .ok();
        if connect_guard.is_none() {
            crate::mlog!(
                crate::log::Level::Warn,
                "net",
                "fresh-connect coordination timed out for {account}; connecting independently"
            );
        }
        // The task ahead of us may have completed its operation and returned the
        // session while we waited for its handshake coordinator.
        if let Some(session) = self.take_pooled(account) {
            drop(connect_guard.take());
            match self
                .run_pooled(account, usage, session, &mut f, &mut command_attempted)
                .await
            {
                Some(result) => return result,
                None => {
                    pool_debug(account, "stale-retry");
                    connect_guard = tokio::time::timeout(
                        match usage {
                            SessionUse::Transfer { deadline, .. } => connect_coordination_timeout()
                                .min(
                                    deadline.saturating_duration_since(tokio::time::Instant::now()),
                                ),
                            _ => connect_coordination_timeout(),
                        },
                        connect_lock.lock(),
                    )
                    .await
                    .ok();
                }
            }
        }
        // The caller's wait is bounded. ensure_valid_creds owns a spawned task,
        // so cancellation still lets a rotated token reach durable storage.
        let creds = match usage {
            SessionUse::Transfer { deadline, .. } => {
                tokio::time::timeout_at(deadline, self.ensure_valid_creds(account))
                    .await
                    .map_err(|_| anyhow::anyhow!("IMAP session preparation timed out"))??
            }
            _ => self.ensure_valid_creds(account).await?,
        };
        let connect_started = std::time::Instant::now();
        let mut session = match usage {
            // Not `TransferTimedOut`: no transfer started, and callers read
            // that one as the message being too slow to download.
            SessionUse::Transfer { deadline, .. } => {
                tokio::time::timeout_at(deadline, imap::connect(&creds))
                    .await
                    .map_err(|_| anyhow::anyhow!("IMAP connect timed out"))??
            }
            SessionUse::Write | SessionUse::Read => imap::connect(&creds).await?,
        };
        let connect_ms = connect_started.elapsed().as_millis();
        if connect_ms > 2_000 {
            crate::mlog!(
                crate::log::Level::Warn,
                "net",
                "slow IMAP connect for {account}: {connect_ms}ms"
            );
        }
        drop(connect_guard);
        let result = match usage {
            SessionUse::Transfer { deadline, budget } => {
                match tokio::time::timeout_at(deadline, f(&mut session)).await {
                    Ok(result) => result,
                    Err(_) => Err(TransferTimedOut(budget).into()),
                }
            }
            SessionUse::Write | SessionUse::Read => f(&mut session).await,
        };
        // A failed or timed-out command leaves the socket mid-command. Drop it
        // instead of returning it to the pool.
        let val = result?;
        self.return_pooled(account, session);
        Ok(val)
    }

    /// Run `f` on a pooled session. `None` means the session was stale (it has
    /// been dropped) and the caller should connect fresh and run `f` again.
    async fn run_pooled<T, F>(
        &self,
        account: &str,
        usage: SessionUse,
        mut session: imap::Session,
        f: &mut F,
        command_attempted: &mut bool,
    ) -> Option<anyhow::Result<T>>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        let may_retry = !*command_attempted;
        let result = match usage {
            // Writes must not be interrupted mid-command.
            SessionUse::Write => f(&mut session).await,
            // A bounded reuse attempt, so a dead pooled connection fails over
            // to a fresh one instead of hanging until TCP keepalive notices.
            SessionUse::Read => {
                // A second pooled checkout may also be stale after a network
                // change. Reject it before spending the command's only retry.
                if !may_retry {
                    let probe = tokio::time::timeout(
                        TRANSFER_PROBE_TIMEOUT,
                        session.run_command_and_check_ok("NOOP"),
                    )
                    .await;
                    if !matches!(probe, Ok(Ok(()))) {
                        return None;
                    }
                }
                *command_attempted = true;
                match tokio::time::timeout(POOLED_READ_TIMEOUT, f(&mut session)).await {
                    Ok(result) => result,
                    Err(_) if may_retry => return None,
                    Err(_) => return Some(Err(anyhow::anyhow!("pooled IMAP read timed out"))),
                }
            }
            // The short timeout covers a NOOP only: enough to notice a dead
            // socket without cutting a slow download short.
            SessionUse::Transfer { deadline, budget } => {
                let probe = tokio::time::timeout_at(
                    deadline,
                    tokio::time::timeout(
                        TRANSFER_PROBE_TIMEOUT,
                        session.run_command_and_check_ok("NOOP"),
                    ),
                )
                .await;
                if !matches!(probe, Ok(Ok(Ok(())))) {
                    return None;
                }
                *command_attempted = true;
                match tokio::time::timeout_at(deadline, f(&mut session)).await {
                    Ok(result) => result,
                    // Alive, only slow: starting over would not be faster.
                    Err(_) => return Some(Err(TransferTimedOut(budget).into())),
                }
            }
        };
        match result {
            Ok(val) => {
                pool_debug(account, "reuse");
                self.return_pooled(account, session);
                Some(Ok(val))
            }
            // Discard the suspect session (do not pool it).
            Err(err) => match usage {
                SessionUse::Write => Some(Err(err)),
                SessionUse::Read | SessionUse::Transfer { .. } if may_retry => None,
                // A session returned during coordination can serve the retry,
                // but its command failure must not trigger a third execution.
                SessionUse::Read | SessionUse::Transfer { .. } => Some(Err(err)),
            },
        }
    }

    /// Run a read-only operation against a pooled (or fresh) session, retrying
    /// once on a stale-connection failure.
    pub async fn with_read_session<T, F>(&self, account: &str, f: F) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        self.with_session(account, true, f).await
    }

    /// Run a read that transfers message bytes (a body FETCH).
    ///
    /// A reused socket is only given a short NOOP to prove it is alive.
    /// Wrapping the FETCH itself in [`POOLED_READ_TIMEOUT`] aborts a
    /// slow-but-alive transfer and starts it over, which is how a large or
    /// high-latency message ends up taking twice as long to appear.
    ///
    /// `budget` is for the whole call: the probe, the wait for a reconnect,
    /// the handshake, credential wait and a second run of `f` all come out of
    /// it. A spawned credential refresh continues if this caller times out.
    /// Running out mid-transfer fails with [`TransferTimedOut`]. `f` runs
    /// at most twice on pooled command errors. A session returned during
    /// coordination can serve the retry; otherwise a fresh connection does.
    pub async fn with_transfer_session<T, F>(
        &self,
        account: &str,
        budget: Duration,
        f: F,
    ) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        let deadline = tokio::time::Instant::now() + budget;
        self.run_session(account, SessionUse::Transfer { deadline, budget }, f)
            .await
    }

    /// Run a mutating operation against a pooled (or fresh) session. Never
    /// auto-retries, so a connection dropped mid-command can't double-apply.
    pub async fn with_write_session<T, F>(&self, account: &str, f: F) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        self.with_session(account, false, f).await
    }

    /// Run a mutating operation after a read-only preflight on the same
    /// session. If the preflight fails on a pooled session, discard that
    /// session and retry the preflight on a fresh connection. Once the
    /// mutating closure starts it is never retried.
    ///
    /// This is for commands such as folder deletion, which must first move a
    /// reused session away from the selected mailbox. Keeping the phases
    /// separate lets a dead pooled socket recover without risking a replay of
    /// a mutation whose server-side outcome is unknown.
    pub async fn with_preflighted_write_session<T, P, F>(
        &self,
        account: &str,
        preflight: P,
        f: F,
    ) -> anyhow::Result<T>
    where
        P: FnMut(&mut imap::Session) -> SessionOp<'_, ()> + Send,
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        self.preflighted_write(account, false, preflight, f).await
    }

    /// Like [`with_preflighted_write_session`](Self::with_preflighted_write_session),
    /// for idempotent writes such as `+FLAGS`/`-FLAGS` stores. The server can
    /// close a pooled connection between the preflight and the write, so when
    /// the write fails on a pooled session with a dropped connection it is run
    /// again, preflight included, on a fresh one. Applying it twice is harmless.
    pub async fn with_flag_write_session<T, P, F>(
        &self,
        account: &str,
        preflight: P,
        f: F,
    ) -> anyhow::Result<T>
    where
        P: FnMut(&mut imap::Session) -> SessionOp<'_, ()> + Send,
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        self.preflighted_write(account, true, preflight, f).await
    }

    async fn preflighted_write<T, P, F>(
        &self,
        account: &str,
        replay_dropped: bool,
        mut preflight: P,
        mut f: F,
    ) -> anyhow::Result<T>
    where
        P: FnMut(&mut imap::Session) -> SessionOp<'_, ()> + Send,
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        let replay = |err: &anyhow::Error| {
            let dropped = replay_dropped && background_sync::is_transient_sync_error(err);
            if dropped {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "net",
                    "pooled connection dropped during write for {account}, replaying on a fresh one: {err:#}"
                );
            }
            dropped
        };
        if let Some(mut session) = self.take_pooled(account) {
            let ready =
                match tokio::time::timeout(POOLED_READ_TIMEOUT, preflight(&mut session)).await {
                    Ok(result) => result,
                    Err(_) => Err(anyhow::anyhow!(
                        "pooled session preflight timed out after {}s",
                        POOLED_READ_TIMEOUT.as_secs()
                    )),
                };
            match ready {
                Ok(()) => {
                    let result = f(&mut session).await;
                    match result {
                        Ok(val) => {
                            pool_debug(account, "reuse");
                            self.return_pooled(account, session);
                            return Ok(val);
                        }
                        Err(err) => {
                            // The mutation may have reached the server. Drop
                            // the connection, and report the result as-is
                            // unless the write is safe to replay.
                            drop(session);
                            if !replay(&err) {
                                return Err(err);
                            }
                            pool_debug(account, "stale-replay");
                        }
                    }
                }
                Err(_) => {
                    // No mutation has run, so replacing this dead pooled
                    // connection and repeating the preflight is safe.
                    drop(session);
                    pool_debug(account, "stale-retry");
                }
            }
        }

        pool_debug(account, "fresh-connect");
        let connect_lock = self.connect_lock(account);
        let mut connect_guard =
            tokio::time::timeout(connect_coordination_timeout(), connect_lock.lock())
                .await
                .ok();
        if connect_guard.is_none() {
            crate::mlog!(
                crate::log::Level::Warn,
                "net",
                "fresh-connect coordination timed out for {account}; connecting independently"
            );
        }
        let pooled_after_wait = self.take_pooled(account);
        if let Some(mut session) = pooled_after_wait {
            drop(connect_guard.take());
            let ready =
                match tokio::time::timeout(POOLED_READ_TIMEOUT, preflight(&mut session)).await {
                    Ok(result) => result,
                    Err(_) => Err(anyhow::anyhow!(
                        "pooled session preflight timed out after {}s",
                        POOLED_READ_TIMEOUT.as_secs()
                    )),
                };
            match ready {
                Ok(()) => {
                    let result = f(&mut session).await;
                    match result {
                        Ok(value) => {
                            self.return_pooled(account, session);
                            return Ok(value);
                        }
                        Err(error) => {
                            drop(session);
                            if !replay(&error) {
                                return Err(error);
                            }
                            pool_debug(account, "stale-replay");
                            connect_guard = tokio::time::timeout(
                                connect_coordination_timeout(),
                                connect_lock.lock(),
                            )
                            .await
                            .ok();
                        }
                    }
                }
                Err(_) => {
                    drop(session);
                    pool_debug(account, "stale-retry");
                    connect_guard =
                        tokio::time::timeout(connect_coordination_timeout(), connect_lock.lock())
                            .await
                            .ok();
                }
            }
        }
        let creds = self.ensure_valid_creds(account).await?;
        let connect_started = std::time::Instant::now();
        let mut session = imap::connect(&creds).await?;
        let connect_ms = connect_started.elapsed().as_millis();
        if connect_ms > 2_000 {
            crate::mlog!(
                crate::log::Level::Warn,
                "net",
                "slow IMAP connect for {account}: {connect_ms}ms"
            );
        }
        drop(connect_guard);
        preflight(&mut session).await?;
        match f(&mut session).await {
            Ok(val) => {
                self.return_pooled(account, session);
                Ok(val)
            }
            Err(err) => Err(err),
        }
    }

    /// Set or clear `\Seen` on UIDs spread over several of the account's
    /// mailboxes — a thread spans folders, and UIDs are mailbox-local — with
    /// one preflighted write per mailbox.
    pub async fn store_seen_by_folder(
        &self,
        account: &str,
        by_folder: &BTreeMap<String, Vec<u32>>,
        seen: bool,
    ) -> anyhow::Result<()> {
        for (folder, uids) in by_folder {
            if uids.is_empty() {
                continue;
            }
            self.with_flag_write_session(
                account,
                |session| {
                    let folder = folder.clone();
                    Box::pin(async move { imap::prepare_flag_update(session, &folder).await })
                },
                |session| {
                    let uids = uids.clone();
                    Box::pin(async move { imap::store_seen(session, &uids, seen).await })
                },
            )
            .await?;
        }
        Ok(())
    }
}

/// A boxed, `Send` future produced by a session-op closure. The `'a` lifetime
/// ties it to the borrowed `&mut Session`; closures must move any other data
/// they need (clone owned copies) into the future so it borrows only the
/// session — letting the closure be re-invoked for the stale-retry path.
pub type SessionOp<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 'a>>;

#[cfg(test)]
mod tests;
