//! Shared mail Engine: per-account IMAP session pool, OAuth token refresh, and
//! the sync/search/fetch operations used by both the desktop sidecar and the
//! mobile FFI host. Platform differences (where the SQLite DB lives, where
//! per-account secrets are stored) are injected via the [`EngineHost`] trait.

use anyhow::Context as _;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};

use crate::{imap, parse, secrets, store};

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
    pub accounts: Mutex<HashMap<String, imap::Creds>>,
    pub db: std::sync::Mutex<rusqlite::Connection>,
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
    /// Platform integration: opens the SQLite store and loads/stores per-account
    /// secrets (OS keychain on desktop, the keyed DB on mobile).
    pub host: Box<dyn EngineHost>,
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

pub fn creds_have_required_secret(creds: &imap::Creds) -> bool {
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

impl Engine {
    pub fn new(host: Box<dyn EngineHost>) -> anyhow::Result<Self> {
        let conn = host.open_db()?;
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
            accounts: Mutex::new(accounts),
            db: std::sync::Mutex::new(conn),
            watched: std::sync::Mutex::new(HashSet::new()),
            syncing: std::sync::Mutex::new(HashSet::new()),
            gap_attempts: std::sync::Mutex::new(HashMap::new()),
            pause_signal: Notify::new(),
            resume_signal: Notify::new(),
            pool: std::sync::Mutex::new(HashMap::new()),
            host,
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

    pub async fn ensure_valid_creds(&self, account: &str) -> anyhow::Result<imap::Creds> {
        let mut accounts = self.accounts.lock().await;
        // Lazily hydrate accounts added out-of-band (the mobile host adds/edits
        // accounts through the stateless command path, not this Engine, so a
        // long-lived foreground Engine can miss them). A no-op on desktop, where
        // the dispatch loop keeps `accounts` authoritative.
        if !accounts.contains_key(account) {
            let loaded = store::load_accounts(&self.db.lock().unwrap())
                .ok()
                .and_then(|rows| rows.into_iter().find(|(id, _)| id == account));
            if let Some((id, mut creds)) = loaded {
                self.host
                    .apply_secret(&self.db.lock().unwrap(), &id, &mut creds);
                accounts.insert(id, creds);
            }
        }
        let creds = accounts
            .get_mut(account)
            .ok_or_else(|| anyhow::anyhow!("account needs reconnect: {account}"))?;

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
                let (new_access, expires_in) = imap::refresh_oauth_token(
                    &token_url,
                    &client_id,
                    &client_secret,
                    refresh_token,
                    scope,
                )
                .await?;
                creds.access_token = Some(new_access);
                creds.token_expires_at = now + expires_in;

                // Persist: token_expires_at to SQLite, the new access token to
                // the host's secret store (keychain on desktop, keyed DB on mobile).
                {
                    let db = self.db.lock().unwrap();
                    store::save_account_config(&db, account, creds)?;
                    self.host
                        .store_secret(&db, account, &secrets::Secrets::from_creds(creds))?;
                }
            }
        }

        Ok(creds.clone())
    }

    /// Pop the hottest non-expired pooled session for `account`, discarding any
    /// that have been idle past `MAX_IDLE` (likely dropped by the server).
    pub fn take_pooled(&self, account: &str) -> Option<imap::Session> {
        pool_take(
            &mut self.pool.lock().unwrap(),
            account,
            std::time::Instant::now(),
            MAX_IDLE,
        )
    }

    /// Return a healthy session to the pool, or drop it if the account is
    /// already at `MAX_POOLED` (the transient-overflow case).
    pub fn return_pooled(&self, account: &str, session: imap::Session) {
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
    pub async fn with_session<T, F>(
        &self,
        account: &str,
        retry: bool,
        mut f: F,
    ) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        if let Some(mut session) = self.take_pooled(account) {
            match f(&mut session).await {
                Ok(val) => {
                    pool_debug(account, "reuse");
                    self.return_pooled(account, session);
                    return Ok(val);
                }
                Err(err) => {
                    // Discard the suspect session (do not pool it).
                    drop(session);
                    if !retry {
                        return Err(err);
                    }
                    pool_debug(account, "stale-retry");
                    // Fall through to a fresh connection and try once more.
                }
            }
        }

        pool_debug(account, "fresh-connect");
        let creds = self.ensure_valid_creds(account).await?;
        let mut session = imap::connect(&creds).await?;
        match f(&mut session).await {
            Ok(val) => {
                self.return_pooled(account, session);
                Ok(val)
            }
            Err(err) => Err(err),
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

    /// Run a mutating operation against a pooled (or fresh) session. Never
    /// auto-retries, so a connection dropped mid-command can't double-apply.
    pub async fn with_write_session<T, F>(&self, account: &str, f: F) -> anyhow::Result<T>
    where
        F: FnMut(&mut imap::Session) -> SessionOp<'_, T> + Send,
        T: Send,
    {
        self.with_session(account, false, f).await
    }
}

/// A boxed, `Send` future produced by a session-op closure. The `'a` lifetime
/// ties it to the borrowed `&mut Session`; closures must move any other data
/// they need (clone owned copies) into the future so it borrows only the
/// session — letting the closure be re-invoked for the stale-retry path.
pub type SessionOp<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 'a>>;

/// Reconnect to IMAP, list folders, and refresh the store.
pub async fn sync_folders(
    engine: &Arc<Engine>,
    account: &str,
) -> anyhow::Result<Vec<imap::Folder>> {
    let folders = engine
        .with_read_session(account, |session| {
            Box::pin(async move { imap::list_folders(session).await })
        })
        .await?;
    let db = engine.db.lock().unwrap();
    store::upsert_folders(&db, account, &folders)?;
    Ok(folders)
}

/// Reconnect to IMAP, fetch the most recent `limit` messages of a folder into
/// the store, resetting cached messages if the server's UIDVALIDITY changed.
pub async fn sync_messages(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> anyhow::Result<usize> {
    // Read the prior sync position before any network I/O so we can ask the
    // server for only the flag changes since then (CONDSTORE CHANGEDSINCE).
    let (prior_modseq, prior_validity) = {
        let db = engine.db.lock().unwrap();
        let modseq = store::get_folder_modseq(&db, account, folder)?;
        let validity = store::get_folder_state(&db, account, folder)?
            .map(|(v, _)| v)
            .unwrap_or(0);
        (modseq, validity)
    };

    let (batch, flag_sync, server_uids) = engine
        .with_read_session(account, |session| {
            let folder = folder.to_string();
            Box::pin(async move {
                let batch = imap::fetch_recent(session, &folder, limit).await?;
                // Reconcile \Seen across the whole folder (catches reads on other
                // devices, even for messages older than the recent window).
                // Best-effort: a no-op when there's no baseline, UIDVALIDITY changed,
                // or the server lacks CONDSTORE.
                let validity_matches = prior_validity != 0 && prior_validity == batch.uidvalidity;
                let flag_sync = imap::sync_flags(session, &folder, prior_modseq, validity_matches)
                    .await
                    .ok();
                // Server-side UID set so we can drop locally cached messages another
                // client moved or deleted. Best-effort: a failure here skips the prune.
                let server_uids = imap::list_all_uids(session, &folder).await.ok();
                anyhow::Ok((batch, flag_sync, server_uids))
            })
        })
        .await?;

    let count = batch.messages.len();
    let db = engine.db.lock().unwrap();
    if prior_validity != 0 && prior_validity != batch.uidvalidity {
        store::clear_folder_messages(&db, account, folder)?;
    }
    store::upsert_messages(&db, account, folder, &batch.messages)?;
    // Make sure the folder is represented in the folders table so its unread
    // count surfaces (tray dot / badges) even before a full folder LIST sync —
    // which, in the unified view, may never run for this account.
    store::ensure_folder(&db, account, folder)?;
    if let Some(uids) = server_uids.as_ref() {
        let validity_ok = prior_validity == 0 || prior_validity == batch.uidvalidity;
        if validity_ok {
            store::prune_missing_messages(&db, account, folder, uids)?;
        }
    }
    if let Some(fs) = flag_sync {
        for &(uid, seen, starred) in &fs.changes {
            store::update_message_seen(&db, account, folder, uid, seen)?;
            store::update_message_starred(&db, account, folder, uid, starred)?;
        }
        if fs.highest_modseq > 0 {
            store::set_folder_modseq(&db, account, folder, fs.highest_modseq)?;
        }
    }
    store::set_folder_state(&db, account, folder, batch.uidvalidity, batch.uid_next)?;
    Ok(count)
}

/// Pick the folder filling a special-use role from a cached folder list. The
/// server-advertised RFC 6154 attribute (recorded by the folder LIST sync)
/// wins when present; folders synced before the attribute was recorded — or
/// servers without the extension — fall back to the name heuristic. `exclude`
/// (the folder currently being synced/viewed) never matches, so callers don't
/// handle the same folder twice.
fn find_role_folder(
    folders: Vec<imap::Folder>,
    special_use: &str,
    looks_like: impl Fn(&str) -> bool,
    exclude: &str,
) -> Option<String> {
    folders
        .iter()
        .find(|folder| {
            folder.special_use.as_deref() == Some(special_use)
                && !folder.name.eq_ignore_ascii_case(exclude)
        })
        .map(|folder| folder.name.clone())
        .or_else(|| {
            folders
                .into_iter()
                .map(|folder| folder.name)
                .find(|name| looks_like(name) && !name.eq_ignore_ascii_case(exclude))
        })
}

/// The cached Sent mailbox name for `account`, if any — used so a chat-view
/// search reaches the user's own replies (and old mail filed under Sent), not
/// just the inbox. `inbox` is excluded so we never list it twice.
pub fn cached_sent_folder(engine: &Arc<Engine>, account: &str, inbox: &str) -> Option<String> {
    let folders = store::get_folders(&engine.db.lock().unwrap(), account).ok()?;
    find_role_folder(folders, "sent", imap::looks_like_sent, inbox)
}

/// The cached Drafts mailbox name for `account`, if any — used so the regular
/// mailbox sync also pulls drafts and replies saved from another client thread
/// into the conversation view straight from the local store. `current` is
/// excluded so we never sync the same folder twice in one pass.
pub fn cached_drafts_folder(engine: &Arc<Engine>, account: &str, current: &str) -> Option<String> {
    let folders = store::get_folders(&engine.db.lock().unwrap(), account).ok()?;
    find_role_folder(folders, "drafts", imap::looks_like_drafts, current)
}

fn push_unique_folder(folders: &mut Vec<String>, folder: Option<String>) {
    if let Some(folder) = folder
        && !folders
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&folder))
    {
        folders.push(folder);
    }
}

fn thread_gap_search_folders(
    sent: Option<String>,
    drafts: Option<String>,
    archive: Option<String>,
) -> Vec<String> {
    let mut folders = vec!["INBOX".to_string()];
    push_unique_folder(&mut folders, sent);
    push_unique_folder(&mut folders, drafts);
    push_unique_folder(&mut folders, archive);
    folders
}

/// Search `account` for `query` across each of `folders` (typically Inbox +
/// Sent), merging the cached and live-IMAP hits. UIDs are folder-scoped, so
/// results are keyed by (folder, uid) and ordered newest-first by date — the
/// only ordering comparable across mailboxes. Each returned header carries its
/// source `folder` so the bridge can build per-message thread IDs correctly.
pub async fn search_mail_messages(
    engine: &Arc<Engine>,
    account: &str,
    folders: &[String],
    query: &str,
    limit: u32,
) -> anyhow::Result<Vec<imap::MessageHeader>> {
    let mut by_key: HashMap<(String, u32), imap::MessageHeader> = HashMap::new();
    {
        let db = engine.db.lock().unwrap();
        for folder in folders {
            for mut message in store::search_messages(&db, account, folder, query, limit)? {
                message.folder = folder.clone();
                by_key.insert((folder.clone(), message.uid), message);
            }
        }
    }

    // Live search, one SELECT + UID SEARCH per folder on a shared session.
    // Best-effort: any failure falls back to the cached hits already collected.
    let server_headers = engine
        .with_read_session(account, |session| {
            let folders = folders.to_vec();
            let query = query.to_string();
            Box::pin(async move {
                let mut all = Vec::new();
                for folder in &folders {
                    let uids = imap::search_uids(session, folder, &query, limit).await?;
                    let mut headers = imap::fetch_headers_by_uid(session, folder, &uids).await?;
                    for header in &mut headers {
                        header.folder = folder.clone();
                    }
                    all.push((folder.clone(), headers));
                }
                anyhow::Ok(all)
            })
        })
        .await
        .ok();

    if let Some(per_folder) = server_headers {
        {
            let db = engine.db.lock().unwrap();
            for (folder, headers) in &per_folder {
                store::upsert_messages(&db, account, folder, headers)?;
            }
        }
        for (folder, headers) in per_folder {
            for message in headers {
                by_key.insert((folder.clone(), message.uid), message);
            }
        }
    }

    let mut messages = by_key.into_values().collect::<Vec<_>>();
    // Newest first by epoch send time; unknown dates (0) sort last.
    messages.sort_unstable_by(|a, b| b.date.cmp(&a.date).then_with(|| b.uid.cmp(&a.uid)));
    messages.truncate(limit as usize);
    Ok(messages)
}

pub async fn starred_search_folders(
    engine: &Arc<Engine>,
    account: &str,
    requested: &str,
) -> Vec<String> {
    let is_gmail = {
        let accounts = engine.accounts.lock().await;
        accounts
            .get(account)
            .map(|creds| creds.auth_type == "gmail_oauth")
            .unwrap_or(false)
    };
    eprintln!(
        "meron-core: starred folders account={account} requested={requested} gmail={is_gmail}"
    );
    if !is_gmail {
        return vec![requested.to_string()];
    }
    let mut folders = store::get_folders(&engine.db.lock().unwrap(), account).unwrap_or_default();
    let find_starred = |folders: &[imap::Folder]| {
        folders
            .iter()
            .map(|folder| folder.name.as_str())
            .find(|name| {
                name.eq_ignore_ascii_case("starred")
                    || name.eq_ignore_ascii_case("[gmail]/starred")
                    || name.eq_ignore_ascii_case("[google mail]/starred")
            })
            .map(str::to_string)
    };
    eprintln!(
        "meron-core: starred folders cached_count={} account={account}",
        folders.len()
    );
    if let Some(folder) = find_starred(&folders) {
        eprintln!("meron-core: starred folders using_starred_mailbox={folder}");
        return vec![folder];
    }
    let fresh = engine
        .with_read_session(account, |session| {
            Box::pin(async move { imap::list_folders(session).await })
        })
        .await;
    match fresh {
        Ok(fresh) => {
            eprintln!(
                "meron-core: starred folders fresh_count={} account={account}",
                fresh.len()
            );
            if let Ok(db) = engine.db.lock() {
                let _ = store::upsert_folders(&db, account, &fresh);
            }
            folders = fresh;
            if let Some(folder) = find_starred(&folders) {
                eprintln!("meron-core: starred folders using_starred_mailbox={folder}");
                return vec![folder];
            }
        }
        Err(err) => {
            eprintln!("meron-core: starred folders LIST/connect failed account={account}: {err:#}");
        }
    }
    let mut names = folders
        .into_iter()
        .map(|folder| folder.name)
        .collect::<Vec<_>>();
    if names.is_empty() {
        names.push(requested.to_string());
    }
    eprintln!(
        "meron-core: starred folders scan_count={} names={}",
        names.len(),
        names.join(", ")
    );
    names
}

pub async fn search_starred_mail_messages(
    engine: &Arc<Engine>,
    account: &str,
    folders: &[String],
    limit: u32,
    refresh: bool,
) -> anyhow::Result<Vec<imap::MessageHeader>> {
    let mut by_uid = HashMap::new();
    let mut cached_count = 0usize;
    for folder in folders {
        let cached = store::get_starred(&engine.db.lock().unwrap(), account, folder, limit)?;
        cached_count += cached.len();
        for message in cached {
            by_uid.insert(format!("{folder}:{}", message.uid), message);
        }
    }
    eprintln!(
        "meron-core: starred search account={account} folders={} cached_hits={} refresh={refresh}",
        folders.len(),
        cached_count
    );

    if refresh {
        // Best-effort per folder: a single folder's failure is logged and
        // skipped, so the closure always succeeds (no stale-retry needed here).
        let server_headers = engine
            .with_read_session(account, |session| {
                let folders = folders.to_vec();
                Box::pin(async move {
                    let mut headers = Vec::new();
                    for folder in &folders {
                        let result = async {
                            let uids = imap::search_starred_uids(session, folder, limit).await?;
                            imap::fetch_headers_by_uid(session, folder, &uids).await
                        }
                        .await;
                        if let Ok(mut found) = result {
                            eprintln!(
                                "meron-core: starred search folder={folder} hits={}",
                                found.len()
                            );
                            for header in &mut found {
                                header.folder = folder.to_string();
                                header.starred = true;
                            }
                            headers.extend(found);
                        } else if let Err(err) = result {
                            eprintln!("meron-core: starred search folder={folder} failed: {err:#}");
                        }
                    }
                    anyhow::Ok(headers)
                })
            })
            .await
            .map_err(|err| {
                eprintln!("meron-core: starred search connect failed account={account}: {err:#}");
                err
            })
            .ok();

        if let Some(headers) = server_headers {
            {
                let db = engine.db.lock().unwrap();
                let mut by_folder: HashMap<String, Vec<imap::MessageHeader>> = HashMap::new();
                for header in &headers {
                    by_folder
                        .entry(header.folder.clone())
                        .or_default()
                        .push(header.clone());
                }
                for (folder, headers) in by_folder {
                    store::upsert_messages(&db, account, &folder, &headers)?;
                }
            }
            for message in headers {
                by_uid.insert(format!("{}:{}", message.folder, message.uid), message);
            }
        }
    }

    let mut messages = by_uid.into_values().collect::<Vec<_>>();
    messages.sort_unstable_by(|a, b| b.date.cmp(&a.date).then_with(|| b.uid.cmp(&a.uid)));
    messages.truncate(limit as usize);
    eprintln!(
        "meron-core: starred search account={account} returning={}",
        messages.len()
    );
    Ok(messages)
}

/// How far back the body prefetcher reaches: unread mail of any age, plus
/// everything received within this many days.
pub const PREFETCH_DAYS: u32 = 14;

#[derive(Clone, Debug)]
pub struct BodyPrefetchOptions {
    pub days: u32,
    pub max_count: Option<usize>,
    pub media_root: PathBuf,
}

impl Default for BodyPrefetchOptions {
    fn default() -> Self {
        Self {
            days: PREFETCH_DAYS,
            max_count: None,
            media_root: parse::media_root(),
        }
    }
}

fn limit_prefetch_uids(mut pending: Vec<u32>, max_count: Option<usize>) -> Vec<u32> {
    if let Some(max_count) = max_count {
        // SEARCH returns ascending UIDs. When mobile has a cap, spend the budget
        // on the newest messages first; older unread mail remains available via
        // the on-demand reader.
        pending.reverse();
        pending.truncate(max_count);
    }
    pending
}

/// Download full message bodies (RFC822, attachments included) for a folder's
/// unread + recent messages into the store, so opening them is instant and they
/// read offline. Reuses one connection: SELECT + SEARCH once, then UID FETCH
/// each pending UID. Skips messages whose body is already cached, so repeat runs
/// converge to a cheap SEARCH; a run cut short (timeout) resumes on the next
/// trigger since saved bodies persist. Layered over the on-demand reader, which
/// still handles anything opened before the prefetcher reaches it.
pub async fn prefetch_bodies(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
) -> anyhow::Result<usize> {
    prefetch_bodies_with_options(engine, account, folder, BodyPrefetchOptions::default()).await
}

pub async fn prefetch_bodies_with_options(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    options: BodyPrefetchOptions,
) -> anyhow::Result<usize> {
    engine
        .with_read_session(account, |session| {
            let engine = engine.clone();
            let account = account.to_string();
            let folder = folder.to_string();
            let options = options.clone();
            Box::pin(async move {
                let uids = imap::search_prefetch_uids(session, &folder, options.days).await?;

                let pending: Vec<u32> = {
                    let db = engine.db.lock().unwrap();
                    uids.into_iter()
                        .filter(|uid| {
                            store::has_message(&db, &account, &folder, *uid).unwrap_or(false)
                                && !store::has_cached_body(&db, &account, &folder, *uid)
                                    .unwrap_or(false)
                        })
                        .collect()
                };
                let pending = limit_prefetch_uids(pending, options.max_count);

                let mut fetched = 0usize;
                for uid in pending {
                    let media = parse::MediaCtx {
                        root: options.media_root.clone(),
                        account: account.to_string(),
                        folder: folder.to_string(),
                        uid,
                    };
                    // peek = true: warming bodies must not flip unread mail to read.
                    match imap::fetch_full_message(session, uid, &media, true).await {
                        Ok(Some(message)) => {
                            let db = engine.db.lock().unwrap();
                            let _ =
                                store::save_cached_message(&db, &account, &folder, uid, &message);
                            fetched += 1;
                        }
                        // Vanished between SEARCH and FETCH (e.g. moved/expunged): skip.
                        Ok(None) => {}
                        // Background warming: log and keep going so one bad message
                        // doesn't abort the whole backlog.
                        Err(e) => eprintln!("meron-core: prefetch {folder} uid {uid}: {e:#}"),
                    }
                }
                anyhow::Ok(fetched)
            })
        })
        .await
}

/// Warm a folder's bodies in the background (deduped per account/folder). Stays
/// silent: this is an optimization, so failures go to stderr rather than
/// surfacing as UI error toasts. The stored data it fills is observed lazily when the
/// user opens a message.
pub fn spawn_body_prefetch(engine: Arc<Engine>, account: String, folder: String) {
    if engine.is_paused(&account) {
        return;
    }
    let key = format!("body:{account}/{folder}");
    if !engine.syncing.lock().unwrap().insert(key.clone()) {
        return;
    }
    tokio::spawn(async move {
        let result = tokio::time::timeout(
            Duration::from_secs(600),
            prefetch_bodies(&engine, &account, &folder),
        )
        .await;
        engine.syncing.lock().unwrap().remove(&key);
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => eprintln!("meron-core: prefetch {folder}: {e:#}"),
            // Partial progress persisted; the next trigger resumes the rest.
            Err(_) => eprintln!("meron-core: prefetch {folder}: timed out (will resume)"),
        }
    });
}

/// Canonicalize folder names so "inbox"/"INBOX" map to one store key + mailbox.
pub fn canon_folder(folder: &str) -> String {
    if folder.eq_ignore_ascii_case("inbox") {
        "INBOX".to_string()
    } else {
        folder.to_string()
    }
}

pub fn should_append_sent_copy(
    auth_type: &str,
    smtp_host: &str,
    override_pref: Option<bool>,
) -> bool {
    override_pref.unwrap_or_else(|| {
        let host = smtp_host.trim_end_matches('.').to_ascii_lowercase();
        let provider_saves_sent = matches!(auth_type, "gmail_oauth" | "outlook_oauth")
            || matches!(
                host.as_str(),
                "smtp.gmail.com"
                    | "smtp.googlemail.com"
                    | "smtp-mail.outlook.com"
                    | "smtp.office365.com"
                    | "smtp.live.com"
                    | "smtp.hotmail.com"
            );
        !provider_saves_sent
    })
}

pub async fn append_to_sent(engine: &Arc<Engine>, account: &str, raw: &[u8]) -> anyhow::Result<()> {
    let (auth_type, smtp_host, override_pref) = {
        let (auth_type, smtp_host) = engine
            .accounts
            .lock()
            .await
            .get(account)
            .map(|creds| (creds.auth_type.clone(), creds.smtp_host.clone()))
            .unwrap_or_else(|| ("password".to_string(), String::new()));
        let override_pref = store::save_sent_copy_pref(&engine.db.lock().unwrap(), account)?;
        (auth_type, smtp_host, override_pref)
    };
    let should_append = should_append_sent_copy(&auth_type, &smtp_host, override_pref);

    // APPEND is mutating, so this never auto-retries (a drop after the server
    // accepted the message must not re-APPEND a duplicate copy).
    let (sent, batch) = engine
        .with_write_session(account, |session| {
            let raw = raw.to_vec();
            Box::pin(async move {
                let sent = imap::find_sent_folder(session)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("no Sent folder found"))?;
                if should_append {
                    imap::append_to_sent(session, &sent, &raw).await?;
                }
                // Refresh local Sent-folder envelopes so the new row is queryable
                // by the cross-folder thread view immediately. For Gmail/Outlook
                // defaults, this picks up the provider-created Sent copy instead
                // of uploading a duplicate.
                let batch = imap::fetch_recent(session, &sent, 20).await?;
                anyhow::Ok((sent, batch))
            })
        })
        .await?;
    {
        let db = engine.db.lock().unwrap();
        store::upsert_messages(&db, account, &sent, &batch.messages)?;
        store::set_folder_state(&db, account, &sent, batch.uidvalidity, batch.uid_next)?;
    }

    Ok(())
}

pub async fn append_to_drafts(
    engine: &Arc<Engine>,
    account: &str,
    raw: &[u8],
    message_id: &str,
) -> anyhow::Result<()> {
    // replace_draft APPENDs the new draft and expunges the prior copy; mutating,
    // so it never auto-retries.
    let (drafts, batch) = engine
        .with_write_session(account, |session| {
            let raw = raw.to_vec();
            let message_id = message_id.to_string();
            Box::pin(async move {
                let drafts = imap::find_drafts_folder(session)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                imap::replace_draft(session, &drafts, &raw, &message_id).await?;
                // Refresh local Drafts envelopes so an autosaved reply appears in
                // the existing cross-folder thread view immediately.
                let batch = imap::fetch_recent(session, &drafts, 20).await?;
                anyhow::Ok((drafts, batch))
            })
        })
        .await?;
    {
        let db = engine.db.lock().unwrap();
        store::upsert_messages(&db, account, &drafts, &batch.messages)?;
        store::set_folder_state(&db, account, &drafts, batch.uidvalidity, batch.uid_next)?;
    }
    Ok(())
}

/// Fetch a thread's referenced-but-unsynced ancestor messages from the server
/// and cache them, so the reader shows the whole conversation rather than the
/// locally-synced tail. No-op when nothing is missing (the common case), so a
/// normal thread open pays no network cost. Searches INBOX and Sent first —
/// where the recent-sync assigns stable UIDs, so re-finding an already-cached
/// message updates it in place rather than duplicating it — then Drafts for
/// remote saved replies, then the all-mail / archive folder for messages that
/// live only there.
/// Fetch any referenced-but-missing ancestor messages for a thread over IMAP and
/// persist them. Returns `true` if at least one message was newly stored, so the
/// caller can refresh the open thread. Opens a network connection — call it off
/// the read path (see [`maybe_spawn_fill_thread_gaps`]).
pub async fn fill_thread_gaps(
    engine: &Arc<Engine>,
    account: &str,
    thread_key: &str,
) -> anyhow::Result<bool> {
    let remaining = {
        let db = engine.db.lock().unwrap();
        store::get_thread_reference_gaps(&db, account, thread_key)?
    };
    if remaining.is_empty() {
        return Ok(false);
    }

    // IMAP-read-only (SEARCH + FETCH); upserts are idempotent, so the whole loop
    // is safe to retry on a stale pooled connection. `remaining` is cloned per
    // invocation so a retry starts from the full gap set.
    engine
        .with_read_session(account, |session| {
            let engine = engine.clone();
            let account = account.to_string();
            let mut remaining = remaining.clone();
            Box::pin(async move {
                let mut persisted = false;
                let sent = imap::find_sent_folder(session).await.ok().flatten();
                let drafts = imap::find_drafts_folder(session).await.ok().flatten();
                let archive = imap::find_archive_folder(session).await.ok().flatten();
                let folders = thread_gap_search_folders(sent, drafts, archive);

                let media_root = parse::media_root();
                for folder in folders {
                    if remaining.is_empty() {
                        break;
                    }
                    let found = match imap::fetch_by_message_ids(
                        session,
                        &folder,
                        &remaining,
                        &media_root,
                        &account,
                    )
                    .await
                    {
                        Ok(found) => found,
                        Err(err) => {
                            eprintln!("meron-core: fetch_by_message_ids folder={folder}: {err:#}");
                            continue;
                        }
                    };
                    if found.is_empty() {
                        continue;
                    }
                    let mut found_ids: HashSet<String> = HashSet::new();
                    {
                        let db = engine.db.lock().unwrap();
                        for fm in &found {
                            // Header row first (writes thread_key etc.), then the
                            // body row (writes body/json) — both keyed on
                            // (account, folder, uid).
                            let _ = store::upsert_messages(
                                &db,
                                &account,
                                &folder,
                                std::slice::from_ref(&fm.header),
                            );
                            let _ = store::save_cached_message(
                                &db,
                                &account,
                                &folder,
                                fm.header.uid,
                                &fm.message,
                            );
                            persisted = true;
                            let mid = fm.message.message_id.trim().to_ascii_lowercase();
                            if !mid.is_empty() {
                                found_ids.insert(mid);
                            }
                        }
                    }
                    remaining.retain(|id| !found_ids.contains(id));
                }
                anyhow::Ok(persisted)
            })
        })
        .await
}

pub async fn read_cached_or_fetch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uid: u32,
) -> anyhow::Result<parse::Message> {
    let media_root = parse::media_root();
    let (cached, load_remote_images) = {
        let db = engine.db.lock().unwrap();
        let cached = store::get_cached_message(&db, account, folder, uid);
        let load_remote = store::load_remote_images(&db, account).unwrap_or(false);
        (cached, load_remote)
    };

    if let Ok(Some(mut msg)) = cached {
        // Rows cached before the threading-header extraction landed have an
        // empty `message_id`; refetch them so reply_to/cc/references populate.
        // Real-world mail almost always carries a Message-ID, so emptiness is a
        // reliable "pre-extraction cache" signal.
        let needs_header_refetch = msg.message_id.is_empty();
        if !needs_header_refetch && parse::cached_media_available(&media_root, &msg) {
            attach_html(&mut msg, load_remote_images);
            return Ok(msg);
        }
    }

    let mut message = engine
        .with_read_session(account, |session| {
            let account = account.to_string();
            let folder = folder.to_string();
            let media_root = media_root.clone();
            Box::pin(async move {
                let media = parse::MediaCtx {
                    root: media_root,
                    account,
                    folder: folder.clone(),
                    uid,
                };
                imap::read_message(session, &folder, uid, &media).await
            })
        })
        .await?;

    {
        let db = engine.db.lock().unwrap();
        let _ = store::save_cached_message(&db, account, folder, uid, &message);
    }

    attach_html(&mut message, load_remote_images);
    Ok(message)
}

/// Turn the stored HTML source into the iframe-ready `body_html` the reader's HTML
/// mode renders: inject the remote-image CSP gated on the account setting. Plain
/// messages have no HTML source, so this is a no-op for them.
pub fn attach_html(message: &mut parse::Message, load_remote_images: bool) {
    if let Some(html) = message.body_html.take() {
        message.body_html = Some(parse::prepare_html(&html, load_remote_images));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Pooled, find_role_folder, limit_prefetch_uids, pool_return, pool_take,
        should_append_sent_copy, thread_gap_search_folders,
    };
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    const MAX_IDLE: Duration = Duration::from_secs(120);
    const MAX_POOLED: usize = 3;

    #[test]
    pub fn take_returns_none_for_unknown_account() {
        let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
        assert_eq!(pool_take(&mut map, "a", Instant::now(), MAX_IDLE), None);
    }

    #[test]
    pub fn return_then_take_round_trips_lifo() {
        let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
        let now = Instant::now();
        pool_return(&mut map, "a", 1, now, MAX_POOLED);
        pool_return(&mut map, "a", 2, now, MAX_POOLED);
        // LIFO: the hottest (last returned) comes back first.
        assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), Some(2));
        assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), Some(1));
        assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), None);
    }

    #[test]
    pub fn return_drops_session_when_at_capacity() {
        let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
        let now = Instant::now();
        for s in 0..(MAX_POOLED as u32 + 2) {
            pool_return(&mut map, "a", s, now, MAX_POOLED);
        }
        assert_eq!(map["a"].len(), MAX_POOLED);
    }

    #[test]
    pub fn take_evicts_sessions_idle_past_max_idle() {
        let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
        let now = Instant::now();
        let stale = now.checked_sub(MAX_IDLE + Duration::from_secs(1)).unwrap();
        // One stale entry, then a fresh one on top.
        pool_return(&mut map, "a", 1, stale, MAX_POOLED);
        pool_return(&mut map, "a", 2, now, MAX_POOLED);
        // Fresh one is taken; the stale one underneath is evicted on the next take.
        assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), Some(2));
        assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), None);
    }

    fn role_folder(name: &str, special_use: Option<&str>) -> crate::imap::Folder {
        crate::imap::Folder {
            name: name.to_string(),
            special_use: special_use.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    pub fn find_role_folder_prefers_server_attribute_over_name() {
        // A localized drafts folder carries the \Drafts attribute; a folder
        // that merely *looks* like drafts by name must lose to it.
        let folders = vec![
            role_folder("Drafts", None),
            role_folder("Mail/Entwürfe", Some("drafts")),
        ];
        assert_eq!(
            find_role_folder(folders, "drafts", crate::imap::looks_like_drafts, "INBOX"),
            Some("Mail/Entwürfe".to_string())
        );
    }

    #[test]
    pub fn find_role_folder_falls_back_to_name_heuristic() {
        // No special-use attributes recorded (server without RFC 6154, or rows
        // synced before the column existed): the name heuristic still works.
        let folders = vec![role_folder("INBOX", None), role_folder("Drafts", None)];
        assert_eq!(
            find_role_folder(folders, "drafts", crate::imap::looks_like_drafts, "INBOX"),
            Some("Drafts".to_string())
        );
    }

    #[test]
    pub fn find_role_folder_never_returns_the_excluded_folder() {
        let folders = vec![role_folder("Drafts", Some("drafts"))];
        assert_eq!(
            find_role_folder(folders, "drafts", crate::imap::looks_like_drafts, "drafts"),
            None
        );
    }

    #[test]
    pub fn thread_gap_search_folders_include_drafts_once_before_archive() {
        let folders = thread_gap_search_folders(
            Some("Sent".to_string()),
            Some("Drafts".to_string()),
            Some("[Gmail]/All Mail".to_string()),
        );
        assert_eq!(folders, vec!["INBOX", "Sent", "Drafts", "[Gmail]/All Mail"]);
    }

    #[test]
    pub fn thread_gap_search_folders_dedup_case_insensitively() {
        let folders = thread_gap_search_folders(
            Some("inbox".to_string()),
            Some("Drafts".to_string()),
            Some("drafts".to_string()),
        );
        assert_eq!(folders, vec!["INBOX", "Drafts"]);
    }

    #[test]
    pub fn prefetch_limit_keeps_all_uids_when_uncapped() {
        assert_eq!(
            limit_prefetch_uids(vec![1, 2, 3, 4], None),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    pub fn prefetch_limit_spends_mobile_budget_on_newest_uids() {
        assert_eq!(limit_prefetch_uids(vec![1, 2, 3, 4], Some(2)), vec![4, 3]);
    }

    #[test]
    pub fn prefetch_limit_zero_fetches_nothing() {
        assert_eq!(
            limit_prefetch_uids(vec![1, 2, 3, 4], Some(0)),
            Vec::<u32>::new()
        );
    }

    #[test]
    pub fn sent_copy_policy_uses_provider_defaults_and_overrides() {
        assert!(!should_append_sent_copy("gmail_oauth", "", None));
        assert!(!should_append_sent_copy("outlook_oauth", "", None));
        assert!(!should_append_sent_copy("password", "smtp.gmail.com", None));
        assert!(!should_append_sent_copy(
            "custom",
            "smtp.office365.com",
            None
        ));
        assert!(should_append_sent_copy(
            "password",
            "smtp.example.com",
            None
        ));
        assert!(should_append_sent_copy("custom", "", None));

        assert!(should_append_sent_copy("gmail_oauth", "", Some(true)));
        assert!(should_append_sent_copy(
            "password",
            "smtp.gmail.com",
            Some(true)
        ));
        assert!(!should_append_sent_copy(
            "password",
            "smtp.example.com",
            Some(false)
        ));
    }
}
