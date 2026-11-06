use anyhow::{Context, Result};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(super) const ACCOUNTS_DDL: &str = "
CREATE TABLE IF NOT EXISTS accounts (
  id           TEXT PRIMARY KEY,
  engine       TEXT NOT NULL DEFAULT 'mail',     -- 'mail' | 'rss'
  provider     TEXT NOT NULL DEFAULT 'custom',   -- 'custom' | 'gmail' | 'rss'
  email        TEXT NOT NULL DEFAULT '',
  display_name TEXT NOT NULL DEFAULT '',
  avatar_url   TEXT NOT NULL DEFAULT '',
  sender_name TEXT NOT NULL DEFAULT '',         -- sender name
  config       TEXT NOT NULL DEFAULT '{}',       -- JSON connection metadata (mail)
  prefs        TEXT NOT NULL DEFAULT '{}',       -- user preferences (see AccountPrefs)
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL DEFAULT 0
);
";

pub(super) const MESSAGES_DDL: &str = "
CREATE TABLE IF NOT EXISTS messages (
  id               INTEGER PRIMARY KEY,   -- stable surrogate rowid; survives VACUUM, is the FTS docid
  account          TEXT NOT NULL,
  folder           TEXT NOT NULL,   -- mail: IMAP folder; rss: subscription id
  msg_id           TEXT NOT NULL,   -- mail: uid as string; rss: item key
  uid              INTEGER NOT NULL DEFAULT 0,   -- mail uid; 0 for rss
  subject          TEXT,
  from_name        TEXT,
  from_addr        TEXT,
  date             INTEGER NOT NULL DEFAULT 0,   -- send time, epoch seconds (0 = unknown)
  seen             INTEGER NOT NULL DEFAULT 0,
  starred          INTEGER NOT NULL DEFAULT 0,
  thread_key       TEXT,
  body             TEXT,
  json             TEXT NOT NULL DEFAULT '{}',   -- JSON catch-all (recipients, body_html, rss fields)
  UNIQUE (account, folder, msg_id)   -- real message identity (mail UID / rss item key)
);
-- (account, folder) lookups are covered by the UNIQUE index prefix.
-- These add the uid ordering / unread / starred access paths the hot queries need.
-- Message lists order by send time (date), with uid as the keyset tiebreaker.
-- `messages_list_idx` keeps uid indexed for the per-uid POINT lookups (mark
-- seen/starred, delete, body-cache write) — those match on uid, not date.
CREATE INDEX IF NOT EXISTS messages_list_idx    ON messages(account, folder, uid);
CREATE INDEX IF NOT EXISTS messages_unread_idx  ON messages(account, folder, date, uid) WHERE seen = 0;
CREATE INDEX IF NOT EXISTS messages_date_idx    ON messages(account, folder, date, uid);
CREATE INDEX IF NOT EXISTS messages_starred_idx ON messages(account, folder, date, uid) WHERE starred <> 0;
-- Cross-account starred view (starred.items) filters only on starred and orders
-- by send time, so it needs a date-leading index independent of account/folder.
CREATE INDEX IF NOT EXISTS messages_starred_all_idx ON messages(date, uid) WHERE starred <> 0;

-- Full-text search over the typed text columns, keyed to messages.id and kept in
-- sync by triggers on the base table so every write path (envelope upsert,
-- body-cache write, prune) is covered. `trigram` does true substring matching
-- (incl. CJK, which has no word breaks); it requires >= 3 codepoints, so shorter
-- queries fall back to LIKE in search_messages. Because `id` is an INTEGER PRIMARY
-- KEY it is stable across VACUUM, so the FTS docid mapping never drifts.
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
  subject, from_name, from_addr, body,
  content='messages', content_rowid='id',
  tokenize='trigram'
);
CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
  VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
END;
CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
  VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
END;
-- Scoped to the indexed text columns so frequent seen/starred toggles (incl.
-- mark-all-read) don't pointlessly reindex the row in FTS; body-cache writes
-- still touch `body` and so still reindex.
CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE OF subject, from_name, from_addr, body ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
  VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
  INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
  VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
END;
";

/// Thread-key inheritance (migration v8) looks a message up by its cached
/// Message-ID, and looks up the rows whose thread key names one — both compared
/// case-insensitively, and neither a plain column, since the id lives in the
/// `json` catch-all. Without these expression indexes every reply upsert scanned
/// the account's whole message cache, `json_extract`ing each row. The index
/// expressions must stay character-identical to the ones in
/// `store::resolve_message_thread_key` / `store::reconcile_thread_keys_from`, or
/// SQLite won't match a query to them.
const MESSAGES_THREAD_KEY_INDEXES_DDL: &str = "
CREATE INDEX IF NOT EXISTS messages_message_id_idx
  ON messages(account, lower(COALESCE(json_extract(json, '$.message_id'), '')));
CREATE INDEX IF NOT EXISTS messages_thread_key_idx
  ON messages(account, lower(COALESCE(thread_key, '')));
";

/// Recipient search index (migration v6). `To`/`Cc` live in the `json` catch-all,
/// which FTS can't index, so `messages.recipients` mirrors them as flat text
/// (see `store::recipients_index_text`). It gets its own FTS table rather than a
/// fifth column on `messages_fts`: adding a column there would force a full
/// rebuild of the body trigram index — minutes of startup for a large mailbox —
/// while this one indexes a few dozen bytes per row.
const MESSAGES_RECIPIENTS_FTS_DDL: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS messages_recipients_fts USING fts5(
  recipients,
  content='messages', content_rowid='id',
  tokenize='trigram'
);
CREATE TRIGGER IF NOT EXISTS messages_recipients_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
END;
CREATE TRIGGER IF NOT EXISTS messages_recipients_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
  VALUES ('delete', old.id, old.recipients);
END;
CREATE TRIGGER IF NOT EXISTS messages_recipients_au AFTER UPDATE OF recipients ON messages BEGIN
  INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
  VALUES ('delete', old.id, old.recipients);
  INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
END;
";

const OBSERVED_MAIL_IDENTITIES_DDL: &str = "
CREATE TABLE IF NOT EXISTS observed_mail_identities (
  account       TEXT NOT NULL,
  identity      TEXT NOT NULL,
  first_seen_at INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, identity)
);
";

const MAIL_SEARCH_HITS_DDL: &str = "
CREATE TABLE IF NOT EXISTS mail_search_hits (
  token      TEXT NOT NULL,
  account    TEXT NOT NULL,
  query      TEXT NOT NULL,
  scope      TEXT NOT NULL,
  position   INTEGER NOT NULL,
  folder     TEXT NOT NULL,
  uid        INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (token, position)
);
CREATE INDEX IF NOT EXISTS mail_search_hits_created_idx
  ON mail_search_hits(account, created_at);
";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS folders (
  account   TEXT NOT NULL,
  name      TEXT NOT NULL,
  delimiter TEXT,
  PRIMARY KEY (account, name)
);
CREATE TABLE IF NOT EXISTS folder_state (
  account        TEXT NOT NULL,
  folder         TEXT NOT NULL,
  uidvalidity    INTEGER,
  uid_next       INTEGER,
  highest_modseq INTEGER,
  PRIMARY KEY (account, folder)
);
CREATE TABLE IF NOT EXISTS subscriptions (
  id            TEXT PRIMARY KEY,
  account       TEXT NOT NULL,
  url           TEXT NOT NULL UNIQUE,
  title         TEXT NOT NULL DEFAULT '',
  site_url      TEXT NOT NULL DEFAULT '',
  feed_title    TEXT NOT NULL DEFAULT '',
  enabled       INTEGER NOT NULL DEFAULT 1,
  last_sync_at  INTEGER NOT NULL DEFAULT 0,
  last_error    TEXT NOT NULL DEFAULT '',
  etag          TEXT NOT NULL DEFAULT '',
  last_modified TEXT NOT NULL DEFAULT '',
  created_at    INTEGER NOT NULL DEFAULT 0,
  updated_at    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS subscriptions_account_idx ON subscriptions(account);
CREATE TABLE IF NOT EXISTS account_secrets (
  account_id TEXT PRIMARY KEY,
  blob       TEXT NOT NULL
);
";

/// Optional Tasks feature. Two tables rather than a JSON blob in `settings`:
/// tasks are user-authored data that grows without bound and is queried by list
/// and by linked thread, which a blob can serve neither cheaply nor safely.
/// `json` is the usual catch-all, so subtasks or recurrence can arrive later
/// without a migration.
pub(super) const TASKS_DDL: &str = "
CREATE TABLE IF NOT EXISTS task_lists (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS tasks (
  id           TEXT PRIMARY KEY,
  list_id      TEXT NOT NULL,
  title        TEXT NOT NULL DEFAULT '',
  notes        TEXT NOT NULL DEFAULT '',
  due_at       INTEGER NOT NULL DEFAULT 0,   -- 0 = no due date
  completed_at INTEGER NOT NULL DEFAULT 0,   -- 0 = not done
  sort_order   INTEGER NOT NULL DEFAULT 0,
  account      TEXT NOT NULL DEFAULT '',     -- mail link; empty for a plain task
  thread_id    TEXT NOT NULL DEFAULT '',
  message_id   TEXT NOT NULL DEFAULT '',
  json         TEXT NOT NULL DEFAULT '{}',
  created_at   INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS tasks_list_idx ON tasks(list_id, sort_order);
CREATE INDEX IF NOT EXISTS tasks_thread_idx ON tasks(thread_id);
";

const BODY_CACHE_VERSION: &str = "1";

pub fn open() -> Result<Connection> {
    open_notifying(|| {})
}

/// [`open`], calling `on_upgrade` first if this open has to rewrite an existing
/// store, which can take a while on a large cache. A brand-new store and an
/// up-to-date one stay silent.
pub fn open_notifying(on_upgrade: impl FnOnce()) -> Result<Connection> {
    let path = db_path();
    // The key comes from the OS keychain, historically the slowest and most
    // failure-prone step of startup (a Flatpak Secret portal with no backend
    // used to hang here forever). Bracket it in the log so a stall is visible.
    crate::mlog!(crate::log::Level::Warn, "store", "resolving store key");
    let started = std::time::Instant::now();
    let key = crate::secrets::db_key();
    crate::mlog!(
        crate::log::Level::Warn,
        "store",
        "store key resolved in {:?}: {}",
        started.elapsed(),
        match &key {
            Ok(Some(_)) => "encrypted".to_string(),
            Ok(None) => "plaintext (keychain disabled)".to_string(),
            Err(error) => format!("failed: {error:#}"),
        }
    );
    let key = key?;
    let _ = STORE_KEY.set(key.clone());
    match key {
        Some(key) => open_inner(path.as_ref(), Some(&key), on_upgrade),
        // Keyring disabled (tests/headless): keep the store plaintext as before.
        None => open_inner(path.as_ref(), None, on_upgrade),
    }
}

/// [`open_notifying`] for a store at a host-chosen path and key.
pub fn open_at_notifying(
    path: impl AsRef<Path>,
    key: Option<&str>,
    on_upgrade: impl FnOnce(),
) -> Result<Connection> {
    open_inner(path.as_ref(), key, on_upgrade)
}

/// The key [`open`] resolved, so the local-folder archive opens with the same
/// one without asking the keychain a second time.
static STORE_KEY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Open the local-folder archive (`archive.db`, beside the store), encrypted
/// with the store's key. See `store::archive`.
pub fn open_archive() -> Result<Connection> {
    open_archive_at(archive_path(), archive_key()?.as_deref())
}

/// The key `archive.db` is encrypted with: the store's.
fn archive_key() -> Result<Option<String>> {
    match STORE_KEY.get() {
        Some(key) => Ok(key.clone()),
        None => crate::secrets::db_key(),
    }
}

/// Open an archive at `path`, keyed when `key` is given. The archive shares
/// the store's schema, so every message-list and thread read works on it
/// unchanged, plus the `archive_raw` table holding the original messages.
pub fn open_archive_at(path: impl AsRef<Path>, key: Option<&str>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let fresh = !path.exists();
    let conn = open_keyed_connection(path, key, || {})?;
    // Deleting archived mail should give the disk space back. auto_vacuum only
    // takes effect when set before the first table exists.
    if fresh {
        conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")
            .context("set archive auto_vacuum")?;
    }
    configure_connection(&conn)?;
    // The archive holds the only copy of its mail: a commit must reach the
    // disk before the server copy is removed.
    conn.execute_batch("PRAGMA synchronous = FULL;")
        .context("set archive synchronous")?;
    with_anyhow_busy_retry(|| run_migrations(&conn)).context("run migrations")?;
    conn.execute_batch(super::archive::ARCHIVE_DDL)
        .context("create archive tables")?;
    Ok(conn)
}

/// Write a consistent copy of the open archive to `dest`, encrypted with
/// `passphrase` rather than the keychain's key, so it can be restored on a
/// computer that never had that key (see `store::restore_local_backup`).
/// Copying `archive.db` itself while Meron runs is not a backup: recent
/// commits live in its WAL.
pub fn backup_archive_to(conn: &Connection, dest: &Path, passphrase: &str) -> Result<()> {
    if passphrase.is_empty() {
        anyhow::bail!("A passphrase is required");
    }
    // Renaming the snapshot over a database in use (or its WAL) would corrupt it.
    let live = [
        conn.path().map(PathBuf::from),
        Some(archive_path()),
        Some(db_path()),
    ];
    for database in live.into_iter().flatten() {
        for suffix in ["", "-wal", "-shm"] {
            if same_file_path(dest, &sibling_with_suffix(&database, suffix)) {
                anyhow::bail!("Choose another file: {} is in use by Meron", dest.display());
            }
        }
    }
    let file_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.db");
    let partial = dest.with_file_name(format!("{file_name}.partial"));
    let _ = std::fs::remove_file(&partial);
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let exported = conn
        .execute(
            "ATTACH DATABASE ?1 AS snapshot KEY ?2",
            params![partial.to_string_lossy(), passphrase],
        )
        .map_err(anyhow::Error::from)
        .and_then(|_| {
            conn.execute_batch(&format!(
                "SELECT sqlcipher_export('snapshot');
                 PRAGMA snapshot.user_version = {user_version};"
            ))
            .map_err(anyhow::Error::from)
        });
    let _ = conn.execute_batch("DETACH DATABASE snapshot;");
    if let Err(err) = exported {
        let _ = std::fs::remove_file(&partial);
        return Err(err).context("export archive snapshot");
    }
    std::fs::rename(&partial, dest).context("move archive snapshot into place")?;
    Ok(())
}

/// Whether two paths name the same file, comparing resolved directories so a
/// path spelled through a symlink or `..` still matches.
fn same_file_path(a: &Path, b: &Path) -> bool {
    let resolve = |path: &Path| {
        let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty());
        let dir = dir
            .map(|dir| dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        path.file_name().map(|name| dir.join(name))
    };
    matches!((resolve(a), resolve(b)), (Some(a), Some(b)) if a == b)
}

fn archive_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_ARCHIVE_DB") {
        return PathBuf::from(path);
    }
    app_dir().join("archive.db")
}

/// Open an unencrypted store. SQLCipher leaves a database untouched until a key
/// is set, so this stays byte-compatible with pre-encryption databases and is
/// what tests and the headless/`MERON_KEYRING=off` path use.
pub fn open_at(path: impl AsRef<Path>) -> Result<Connection> {
    open_inner(path.as_ref(), None, || {})
}

/// Open an encrypted store, keying the connection with `key` (64 hex chars = a
/// raw 32-byte SQLCipher key). A legacy plaintext database at `path` is migrated
/// in place to an encrypted one on first keyed open.
pub fn open_at_keyed(path: impl AsRef<Path>, key: &str) -> Result<Connection> {
    open_inner(path.as_ref(), Some(key), || {})
}

fn open_inner(path: &Path, key: Option<&str>, on_upgrade: impl FnOnce()) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut on_upgrade = Some(on_upgrade);
    let mut announce_upgrade = || {
        if let Some(on_upgrade) = on_upgrade.take() {
            on_upgrade();
        }
    };
    let conn = open_keyed_connection(path, key, &mut announce_upgrade)?;
    configure_connection(&conn)?;
    if upgrade_pending(&conn) {
        announce_upgrade();
    }
    with_anyhow_busy_retry(|| run_migrations(&conn)).context("run migrations")?;
    with_anyhow_busy_retry(|| invalidate_body_cache_if_needed(&conn))
        .context("invalidate body cache if needed")?;
    Ok(conn)
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_millis(5000))
        .context("set busy timeout")?;
    // WAL + synchronous=NORMAL is the durable-but-fast desktop combo; busy_timeout
    // avoids spurious SQLITE_BUSY now that a reader can overlap a writer under WAL.
    with_busy_retry(|| {
        conn.execute_batch(
            // foreign_keys is per-connection and off by default. No table declares a
            // FOREIGN KEY today, so this is a no-op for now — it's set so that any FK
            // constraints added in a future migration are enforced automatically.
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA foreign_keys = ON;",
        )
    })
    .context("set connection pragmas")?;
    Ok(())
}

/// Opens `path` and applies the SQLCipher `key` (if any). When the key is set
/// but the file turns out to be an unencrypted legacy database, it is migrated
/// in place to an encrypted database before returning, calling `on_encrypt`
/// first because that exports the whole file.
fn open_keyed_connection(
    path: &Path,
    key: Option<&str>,
    on_encrypt: impl FnOnce(),
) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("open db {}", path.display()))?;
    let Some(key) = key else {
        return Ok(conn);
    };
    apply_key(&conn, key)?;
    if database_readable(&conn) {
        return Ok(conn);
    }
    // The key didn't unlock the file. Either it predates encryption and is
    // plaintext (the one-time upgrade below), or we were handed the wrong key —
    // which happens when the keychain lost our entry and minted a fresh one.
    // Refuse the migration in that case: it would fail anyway, and the explicit
    // error names the real problem instead of blaming the schema.
    if is_encrypted_database(path) {
        anyhow::bail!(
            "{} is encrypted with a different key — the keychain entry holding \
             the store key is missing or was replaced",
            path.display()
        );
    }
    drop(conn);
    on_encrypt();
    encrypt_plaintext_db(path, key)
        .with_context(|| format!("encrypt legacy plaintext db {}", path.display()))?;
    let conn = Connection::open(path).with_context(|| format!("reopen db {}", path.display()))?;
    apply_key(&conn, key)?;
    if !database_readable(&conn) {
        anyhow::bail!(
            "database still unreadable after encryption migration: {}",
            path.display()
        );
    }
    Ok(conn)
}

/// Apply the raw 32-byte key (as 64 hex chars). Using the raw-key form makes
/// SQLCipher skip PBKDF2 derivation, so opening is cheap and deterministic.
fn apply_key(conn: &Connection, key: &str) -> Result<()> {
    conn.execute_batch(&format!("PRAGMA key = \"x'{key}'\";"))
        .context("apply sqlcipher key")
}

/// Whether `path` holds an existing *encrypted* database, i.e. a non-empty file
/// that does not carry SQLite's plaintext header. Used to tell "wrong key" from
/// "legacy plaintext database" without guessing.
fn is_encrypted_database(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 16];
    match file.read_exact(&mut header) {
        Ok(()) => &header != b"SQLite format 3\0",
        // Shorter than a header: a freshly created or empty file, not encrypted.
        Err(_) => false,
    }
}

/// A cheap probe that succeeds only when the page cipher matches the file: a
/// plaintext file opened with a key (or a wrong key) fails to decrypt here.
fn database_readable(conn: &Connection) -> bool {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .is_ok()
}

/// One-time upgrade of a plaintext database to an encrypted one. Exports the
/// plaintext contents into a fresh encrypted sibling via `sqlcipher_export`,
/// then atomically replaces the original and drops its now-stale WAL/SHM files.
fn encrypt_plaintext_db(path: &Path, key: &str) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("meron.db");
    let tmp = path.with_file_name(format!("{file_name}.sqlcipher-migrating"));
    let _ = std::fs::remove_file(&tmp);
    {
        let plain =
            Connection::open(path).with_context(|| format!("open plaintext {}", path.display()))?;
        // Fold any WAL frames into the main file so the export sees committed data.
        let _ = plain.pragma_update(None, "journal_mode", "DELETE");
        // `sqlcipher_export` copies the schema and rows but not `user_version`,
        // so carry it across explicitly — otherwise migrations re-run on the
        // encrypted copy and trip over the already-migrated schema.
        let user_version: i64 = plain.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        plain
            .execute_batch(&format!(
                "ATTACH DATABASE '{}' AS encrypted KEY \"x'{key}'\";
                 SELECT sqlcipher_export('encrypted');
                 PRAGMA encrypted.user_version = {user_version};
                 DETACH DATABASE encrypted;",
                sql_single_quote(&tmp)
            ))
            .context("sqlcipher_export to encrypted db")?;
    }
    std::fs::rename(&tmp, path).context("replace plaintext db with encrypted db")?;
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(sibling_with_suffix(path, suffix));
    }
    Ok(())
}

fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn sql_single_quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn with_busy_retry<T>(mut f: impl FnMut() -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut delay = Duration::from_millis(10);
    loop {
        match f() {
            Err(err) if is_database_locked(&err) && Instant::now() < deadline => {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

fn with_anyhow_busy_retry<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut delay = Duration::from_millis(10);
    loop {
        match f() {
            Err(err) if anyhow_error_is_database_locked(&err) && Instant::now() < deadline => {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

fn anyhow_error_is_database_locked(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<rusqlite::Error>()
            .is_some_and(is_database_locked)
    })
}

fn is_database_locked(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            )
    )
}

fn db_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_CORE_DB") {
        return PathBuf::from(path);
    }
    config_dir().join("meron.db")
}

/// The directory holding the store, and the natural home for anything else that
/// belongs to this app profile (the local keyring, for one). Follows
/// `MERON_CORE_DB`, so dev and production profiles stay separate.
pub fn app_dir() -> PathBuf {
    db_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(config_dir)
}

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/meron")
}

// ---- Migrations -------------------------------------------------------------

/// The `user_version` [`run_migrations`] leaves behind.
pub(super) const SCHEMA_VERSION: i64 = 29;

/// Whether opening this store will rewrite existing data: a schema behind
/// [`SCHEMA_VERSION`], or bodies rendered by an older version. Only decides
/// whether to tell the user, so a failed read just answers "no".
fn upgrade_pending(conn: &Connection) -> bool {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if version == 0 {
        return false;
    }
    version < SCHEMA_VERSION
        || meta_get(conn, "body_cache_version")
            .is_ok_and(|current| current.as_deref() != Some(BODY_CACHE_VERSION))
}

/// Apply schema migrations in order, tracked by SQLite's `PRAGMA user_version`.
///
/// Pending steps run inside one IMMEDIATE transaction that also bumps
/// `user_version`, so concurrent first-open callers serialize before reading
/// the version. A crash mid-migration rolls back cleanly and the step re-runs
/// next launch. Append-only: to evolve the schema, add a new `if version < N`
/// block; never edit or reorder a shipped one. (Cache-only invalidation lives in
/// `invalidate_body_cache_if_needed`, kept off this counter so a render change
/// doesn't look like a schema change.)
///
/// The local-folder archive runs these too (`open_archive_at`), and there the
/// `messages` rows are not a cache: a migration may rebuild them, but never
/// drop cached mail on the assumption that a sync will bring it back.
pub(super) fn run_migrations(conn: &Connection) -> Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version < 1 {
        migrate_v1(&tx)?;
    }
    if version < 2 {
        migrate_v2(&tx)?;
    }
    if version < 3 {
        migrate_v3(&tx)?;
    }
    if version < 4 {
        migrate_v4(&tx)?;
    }
    if version < 5 {
        migrate_v5(&tx)?;
    }
    if version < 6 {
        migrate_v6(&tx)?;
    }
    if version < 7 {
        migrate_v7(&tx)?;
    }
    if version < 8 {
        migrate_v8(&tx)?;
    }
    if version < 9 {
        migrate_v9(&tx)?;
    }
    if version < 10 {
        migrate_v10(&tx)?;
    }
    if version < 11 {
        migrate_v11(&tx)?;
    }
    if version < 12 {
        migrate_v12(&tx)?;
    }
    if version < 13 {
        migrate_v13(&tx)?;
    }
    if version < 14 {
        migrate_v14(&tx)?;
    }
    if version < 15 {
        migrate_v15(&tx)?;
    }
    if version < 16 {
        migrate_v16(&tx)?;
    }
    if version < 17 {
        migrate_v17(&tx)?;
    }
    if version < 18 {
        migrate_v18(&tx)?;
    }
    if version < 19 {
        migrate_v19(&tx)?;
    }

    if version < 20 {
        migrate_v20(&tx)?;
    }
    if version < 21 {
        migrate_v21(&tx)?;
    }
    if version < 22 {
        migrate_v22(&tx)?;
    }
    if version < 23 {
        migrate_v23(&tx)?;
    }
    if version < 24 {
        migrate_v24(&tx)?;
    }
    if version < 25 {
        migrate_v25(&tx)?;
    }
    if version < 26 {
        migrate_v26(&tx)?;
    }
    if version < 27 {
        migrate_v27(&tx)?;
    }
    if version < 28 {
        migrate_v28(&tx)?;
    }
    if version < 29 {
        migrate_v29(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

/// A database as v5 left it, for tests that need to migrate an *existing*
/// install rather than a fresh one.
#[cfg(test)]
pub(super) fn migrate_to_v5(conn: &Connection) -> Result<()> {
    migrate_v1(conn)?;
    migrate_v2(conn)?;
    migrate_v3(conn)?;
    migrate_v4(conn)?;
    migrate_v5(conn)
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(ACCOUNTS_DDL)?;
    conn.execute_batch(MESSAGES_DDL)?;
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch("PRAGMA user_version = 1;")?;
    Ok(())
}

/// Per-subscription extra metadata that doesn't warrant its own typed column
/// (feed icon/logo, …), stored as a JSON object. Mirrors the `messages.json`
/// approach; defaults to an empty object so existing rows stay valid.
fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE subscriptions ADD COLUMN json TEXT NOT NULL DEFAULT '{}';")?;
    conn.execute_batch("PRAGMA user_version = 2;")?;
    Ok(())
}

/// Per-account secret blob (IMAP password / OAuth tokens) for platforms without
/// an OS keychain (Android, iOS sandbox). Desktop continues to use the keychain
/// via the `secrets` module; only the mobile FFI path reads/writes this table.
fn migrate_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS account_secrets (
           account_id TEXT PRIMARY KEY,
           blob       TEXT NOT NULL
         );",
    )?;
    conn.execute_batch("PRAGMA user_version = 3;")?;
    Ok(())
}

/// RFC 6154 special-use role reported by LIST for a folder ("drafts", "sent",
/// …), NULL when the server doesn't advertise one. Lets role lookups (which
/// folder holds drafts?) trust the server over name heuristics.
fn migrate_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE folders ADD COLUMN special_use TEXT;")?;
    conn.execute_batch("PRAGMA user_version = 4;")?;
    Ok(())
}

/// Stable message identities we've already seen, used to suppress "new mail"
/// notifications when Gmail restores an older message with a fresh INBOX UID.
fn migrate_v5(conn: &Connection) -> Result<()> {
    conn.execute_batch(OBSERVED_MAIL_IDENTITIES_DDL)?;
    conn.execute_batch("PRAGMA user_version = 5;")?;
    Ok(())
}

/// Searchable `To`/`Cc` text plus the FTS index over it, so a lookup by
/// recipient finds cached mail the way a lookup by sender already does.
/// Existing rows are backfilled from `json` through the same formatter the write
/// path uses, then indexed in one `rebuild` — cheaper than letting the triggers
/// fire per row.
fn migrate_v6(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE messages ADD COLUMN recipients TEXT;")?;
    backfill_recipients(conn)?;
    conn.execute_batch(MESSAGES_RECIPIENTS_FTS_DDL)?;
    conn.execute_batch(
        "INSERT INTO messages_recipients_fts(messages_recipients_fts) VALUES('rebuild');",
    )?;
    conn.execute_batch("PRAGMA user_version = 6;")?;
    Ok(())
}

/// Stable server-search snapshots. IMAP SEARCH returns an unordered UID set,
/// while the UI pages by message date; retaining the resolved order prevents a
/// later page from losing messages whose Date header does not follow UID order.
fn migrate_v7(conn: &Connection) -> Result<()> {
    conn.execute_batch(MAIL_SEARCH_HITS_DDL)?;
    conn.execute_batch("PRAGMA user_version = 7;")?;
    Ok(())
}

/// Access paths for thread-key inheritance, which until now scanned the whole
/// account cache per upserted reply. See `MESSAGES_THREAD_KEY_INDEXES_DDL`.
fn migrate_v8(conn: &Connection) -> Result<()> {
    conn.execute_batch(MESSAGES_THREAD_KEY_INDEXES_DDL)?;
    conn.execute_batch("PRAGMA user_version = 8;")?;
    Ok(())
}

/// Avoid reindexing unchanged headers and observe cached identities immediately.
/// Sync classifies arrivals before writing its fetched batch.
fn migrate_v9(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP TRIGGER messages_au;
         CREATE TRIGGER messages_au AFTER UPDATE OF subject, from_name, from_addr, body ON messages
         WHEN old.subject IS NOT new.subject OR old.from_name IS NOT new.from_name
           OR old.from_addr IS NOT new.from_addr OR old.body IS NOT new.body
         BEGIN
           INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
           VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
           INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
           VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
         END;
         DROP TRIGGER messages_recipients_au;
         CREATE TRIGGER messages_recipients_au AFTER UPDATE OF recipients ON messages
         WHEN old.recipients IS NOT new.recipients
         BEGIN
           INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
           VALUES ('delete', old.id, old.recipients);
           INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
         END;",
    )?;
    conn.execute_batch(
        "INSERT OR IGNORE INTO observed_mail_identities(account, identity, first_seen_at)
         SELECT account, CASE
           WHEN json_extract(json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(json, '$.gmail_msg_id')
           WHEN COALESCE(json_extract(json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(json, '$.message_id'))
           ELSE 'uid:' || lower(folder) || ':' || uid END, unixepoch()
         FROM messages WHERE uid <> 0;
         DROP TRIGGER IF EXISTS pending_mail_identity_ai;
         DROP TRIGGER IF EXISTS pending_mail_identity_au;
         DROP TRIGGER IF EXISTS pending_mail_identity_ad;
         DROP TABLE IF EXISTS pending_mail_identities;
         DROP TRIGGER IF EXISTS observe_mail_identity_ai;
         DROP TRIGGER IF EXISTS observe_mail_identity_au;",
    )?;
    for (name, event) in [
        ("ai", "INSERT"),
        ("au", "UPDATE OF json, uid, folder, account"),
    ] {
        let changed = if name == "au" {
            " AND (old.json IS NOT new.json OR old.uid IS NOT new.uid OR old.folder IS NOT new.folder OR old.account IS NOT new.account)"
        } else {
            ""
        };
        conn.execute_batch(&format!(
            "CREATE TRIGGER observe_mail_identity_{name} AFTER {event} ON messages WHEN new.uid <> 0{changed} BEGIN
             INSERT INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(new.account, CASE
               WHEN json_extract(new.json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(new.json, '$.gmail_msg_id')
               WHEN COALESCE(json_extract(new.json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(new.json, '$.message_id'))
               ELSE 'uid:' || lower(new.folder) || ':' || new.uid END, unixepoch())
             ON CONFLICT DO NOTHING;
             END;"
        ))?;
    }
    conn.execute_batch("PRAGMA user_version = 9;")?;
    Ok(())
}

/// Tasks: lists and their items, for the optional Tasks surface. Local-only —
/// nothing here is synced to a server, so the rows are also carried by the
/// config backup.
fn migrate_v10(conn: &Connection) -> Result<()> {
    conn.execute_batch(TASKS_DDL)?;
    conn.execute_batch("PRAGMA user_version = 10;")?;
    Ok(())
}

/// `messages.files`: the cached body's real attachments as a JSON array of
/// `{filename, mime}`, NULL when it has none, so list cards can name them and
/// the attachments filter can page without parsing every row's `json`. Written
/// with header BODYSTRUCTURE metadata and refined by `save_cached_message`.
///
/// Rows cached before this are filled by [`super::backfill_files_batch`] after
/// startup, not here: deriving them parses every row's `json` (HTML bodies
/// included), which on a large cache holds the store long enough for the
/// frontend's first requests to time out.
fn migrate_v11(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE messages ADD COLUMN files TEXT;")?;
    conn.execute_batch("PRAGMA user_version = 11;")?;
    Ok(())
}

/// One row per live search: the identity a snapshot page is validated against.
/// A first page fetches only the newest hits; the rest wait in
/// `mail_search_pending`, one row per server hit with its Date header resolved
/// up front, so batches are taken newest-first by date across every folder
/// rather than by UID, which orders mailbox insertion and not send time.
/// `cache_resume` is set when a batch cut off its cached-only hits: the keyset
/// cursor of the last one it took, from which paging past the snapshot
/// continues through the cache.
const MAIL_SEARCH_SNAPSHOTS_DDL: &str = "
CREATE TABLE IF NOT EXISTS mail_search_snapshots (
  token        TEXT PRIMARY KEY,
  account      TEXT NOT NULL,
  query        TEXT NOT NULL,
  scope        TEXT NOT NULL,
  cache_resume TEXT,
  created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS mail_search_snapshots_created_idx
  ON mail_search_snapshots(account, created_at);
CREATE TABLE IF NOT EXISTS mail_search_pending (
  token   TEXT NOT NULL,
  account TEXT NOT NULL,
  folder  TEXT NOT NULL,
  uid     INTEGER NOT NULL,
  date    INTEGER NOT NULL,
  PRIMARY KEY (token, folder, uid)
);
CREATE INDEX IF NOT EXISTS mail_search_pending_order_idx
  ON mail_search_pending(token, date DESC, uid DESC, folder DESC);
CREATE INDEX IF NOT EXISTS mail_search_pending_account_idx
  ON mail_search_pending(account);
";

/// Resumable live searches; see `MAIL_SEARCH_SNAPSHOTS_DDL`. Hit rows saved by
/// earlier versions have no snapshot row, so their cursors fall back to cached
/// keyset paging, as an expired snapshot does.
fn migrate_v12(conn: &Connection) -> Result<()> {
    conn.execute_batch(MAIL_SEARCH_SNAPSHOTS_DDL)?;
    conn.execute_batch("PRAGMA user_version = 12;")?;
    Ok(())
}

/// Unseen server UIDs that the last sync found outside the cached window, so a
/// folder's unread total covers mail older than what the cache holds. A row is
/// dropped once its message is cached there, whether inserted or moved in from
/// another folder: the cached row
/// then carries the unread state, and deleting or moving it later can't bring
/// the stale row back into the count.
fn migrate_v13(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS uncached_unseen (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;
         CREATE TRIGGER IF NOT EXISTS messages_uncached_unseen_ai AFTER INSERT ON messages
         WHEN new.uid <> 0 BEGIN
           DELETE FROM uncached_unseen
            WHERE account = new.account AND folder = new.folder AND uid = new.uid;
         END;
         CREATE TRIGGER IF NOT EXISTS messages_uncached_unseen_au
         AFTER UPDATE OF account, folder, uid ON messages
         WHEN new.uid <> 0 BEGIN
           DELETE FROM uncached_unseen
            WHERE account = new.account AND folder = new.folder AND uid = new.uid;
         END;",
    )?;
    conn.execute_batch("PRAGMA user_version = 13;")?;
    Ok(())
}

/// UIDs this process removed from a folder. A folder sync that fetched its
/// snapshot before the removal must not write those rows back, or an archived
/// thread reappears in the mailbox it just left.
fn migrate_v14(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS removed_message_uids (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           epoch   INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;",
    )?;
    conn.execute_batch("PRAGMA user_version = 14;")?;
    Ok(())
}

/// Folder syncs with a snapshot in flight, shared by every connection so one
/// engine does not prune removal markers another engine's sync still needs.
fn migrate_v15(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS active_message_syncs (
           id    INTEGER PRIMARY KEY AUTOINCREMENT,
           epoch INTEGER NOT NULL,
           owner TEXT NOT NULL
         );",
    )?;
    conn.execute_batch("PRAGMA user_version = 15;")?;
    Ok(())
}

/// Per-folder opt-in to live sync and new-mail notifications. INBOX is always
/// watched and never consults the flag.
fn migrate_v16(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE folders ADD COLUMN notify INTEGER NOT NULL DEFAULT 0;")?;
    conn.execute_batch("PRAGMA user_version = 16;")?;
    Ok(())
}

/// Recipient autocomplete's directory (migration v17): one row per account,
/// address and display name seen as a mail message's sender, To or Cc,
/// counting the messages it appears on. Keeping each name apart means a name
/// leaves with the last message carrying it. Suggesting from the messages
/// themselves meant reading and parsing every cached row's recipient lists per
/// keystroke. Triggers on `messages` keep it in step with every write path;
/// the update trigger only fires when the addresses could have changed, as an
/// envelope upsert rewrites `json` on every resync.
fn contacts_ddl() -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS contacts (
           account  TEXT NOT NULL,
           addr_key TEXT NOT NULL,   -- lower(trim(addr))
           name     TEXT NOT NULL,   -- '' when the address came without one
           addr     TEXT NOT NULL,
           count    INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (account, addr_key, name)
         ) WITHOUT ROWID;
         {}",
        contacts_triggers(false)
    )
}

/// The `messages` triggers that keep `contacts` in step. With `track_date`
/// (migration v23) they also keep `last_date`, the newest message an entry was
/// seen on. Removing a message can't lower it without rescanning that
/// address's mail, so it only ever moves forward until the entry goes.
fn contacts_triggers(track_date: bool) -> String {
    let add = |row: &str| {
        let (date_col, date_val, date_set) = if track_date {
            (
                ", last_date",
                format!(", {row}.date"),
                ", last_date = max(last_date, excluded.last_date)",
            )
        } else {
            ("", String::new(), "")
        };
        format!(
            "INSERT INTO contacts(account, addr_key, name, addr, count{date_col})
               SELECT {row}.account, lower(trim(addr)), trim(COALESCE(name, '')), max(trim(addr)), 1{date_val}
               FROM ({occurrences}) WHERE {row}.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
               GROUP BY lower(trim(addr)), trim(COALESCE(name, ''))
             ON CONFLICT(account, addr_key, name) DO UPDATE SET count = count + 1{date_set};",
            occurrences = contact_occurrences(row),
        )
    };
    let remove = |row: &str| {
        let keys = format!(
            "SELECT lower(trim(addr)), trim(COALESCE(name, '')) FROM ({})
               WHERE trim(COALESCE(addr, '')) <> ''",
            contact_occurrences(row)
        );
        format!(
            "UPDATE contacts SET count = count - 1
               WHERE account = {row}.account AND {row}.uid <> 0 AND (addr_key, name) IN ({keys});
             DELETE FROM contacts
               WHERE account = {row}.account AND count <= 0 AND (addr_key, name) IN ({keys});"
        )
    };
    let addressees = |row: &str| format!("json_extract({row}.json, '$.to', '$.cc')");
    // The date only matters to `last_date`, so without it a date change
    // alone doesn't rewrite the directory.
    let date_changed = if track_date {
        "\n             OR old.date IS NOT new.date"
    } else {
        ""
    };
    let date_column = if track_date { ", date" } else { "" };
    format!(
        "CREATE TRIGGER IF NOT EXISTS contacts_ai AFTER INSERT ON messages BEGIN
           {add_new}
         END;
         CREATE TRIGGER IF NOT EXISTS contacts_ad AFTER DELETE ON messages BEGIN
           {remove_old}
         END;
         CREATE TRIGGER IF NOT EXISTS contacts_au
           AFTER UPDATE OF account, uid, from_name, from_addr, json{date_column} ON messages
           WHEN old.account IS NOT new.account
             OR (old.uid <> 0) IS NOT (new.uid <> 0)
             OR old.from_name IS NOT new.from_name
             OR old.from_addr IS NOT new.from_addr
             OR {old_addressees} IS NOT {new_addressees}{date_changed}
         BEGIN
           {remove_old}
           {add_new}
         END;",
        add_new = add("new"),
        remove_old = remove("old"),
        old_addressees = addressees("old"),
        new_addressees = addressees("new"),
    )
}

/// `row`'s sender and its To and Cc entries, as `(name, addr)` rows.
fn contact_occurrences(row: &str) -> String {
    let list = |path: &str| {
        format!(
            "SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each({row}.json, '{path}') WHERE type = 'object'"
        )
    };
    format!(
        "SELECT {row}.from_name AS name, {row}.from_addr AS addr
         UNION ALL {} UNION ALL {}",
        list("$.to"),
        list("$.cc"),
    )
}

fn migrate_v17(conn: &Connection) -> Result<()> {
    conn.execute_batch(&contacts_ddl())?;
    let list = |path: &str| {
        format!(
            "SELECT m.id, m.account, json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
               FROM messages m, json_each(m.json, '{path}') e
               WHERE m.uid <> 0 AND e.type = 'object'"
        )
    };
    conn.execute_batch(&format!(
        "INSERT INTO contacts(account, addr_key, name, addr, count)
           SELECT account, addr_key, name, max(addr), count(*) FROM (
             SELECT id, account, lower(trim(addr)) AS addr_key, trim(COALESCE(name, '')) AS name,
                    max(trim(addr)) AS addr
             FROM (
               SELECT id, account, from_name AS name, from_addr AS addr FROM messages WHERE uid <> 0
               UNION ALL {} UNION ALL {}
             )
             WHERE trim(COALESCE(addr, '')) <> ''
             GROUP BY id, lower(trim(addr)), trim(COALESCE(name, ''))
           )
           WHERE true
           GROUP BY account, addr_key, name;
         PRAGMA user_version = 17;",
        list("$.to"),
        list("$.cc"),
    ))?;
    Ok(())
}

/// Calendar invitations the user answered from the card (migration v18), keyed
/// by the event rather than the message so every copy of the invite — the
/// original, an update, the one in another folder — shows the same answer.
/// `sequence` is the revision answered: a later one asks again.
fn migrate_v18(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS invite_responses (
           account       TEXT NOT NULL,
           uid           TEXT NOT NULL,     -- iCalendar UID
           recurrence_id INTEGER NOT NULL DEFAULT 0,   -- 0 = the whole series
           response      TEXT NOT NULL,     -- accepted | tentative | declined
           sequence      INTEGER NOT NULL DEFAULT 0,
           responded_at  INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (account, uid, recurrence_id)
         );
         PRAGMA user_version = 18;",
    )?;
    Ok(())
}

/// The local calendar (migration v19): events the user made, and invitations
/// they accepted. Like tasks, a table rather than a blob, queried by time.
/// `recurring` rows are expanded on read from `rrule` (an event made here) or
/// `ics` (an invitation, whose own rules and exceptions are kept whole), so a
/// range query takes every recurring row that starts before its end.
fn migrate_v19(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
           id          TEXT PRIMARY KEY,
           title       TEXT NOT NULL DEFAULT '',
           notes       TEXT NOT NULL DEFAULT '',
           location    TEXT NOT NULL DEFAULT '',
           start_at    INTEGER NOT NULL DEFAULT 0,   -- all-day: UTC midnight
           end_at      INTEGER NOT NULL DEFAULT 0,   -- exclusive
           all_day     INTEGER NOT NULL DEFAULT 0,
           recurring   INTEGER NOT NULL DEFAULT 0,
           rrule       TEXT NOT NULL DEFAULT '',     -- daily | weekly | monthly | yearly
           tzid        TEXT NOT NULL DEFAULT '',     -- the zone a repeat keeps its time in
           account     TEXT NOT NULL DEFAULT '',     -- mail link; empty for an event made here
           thread_id   TEXT NOT NULL DEFAULT '',
           message_id  TEXT NOT NULL DEFAULT '',
           invite_uid  TEXT NOT NULL DEFAULT '',     -- iCalendar UID of an accepted invitation
           recurrence_id INTEGER NOT NULL DEFAULT 0, -- an invitation to one occurrence alone; 0 = the event
           response    TEXT NOT NULL DEFAULT '',     -- accepted | tentative
           ics         TEXT NOT NULL DEFAULT '',
           json        TEXT NOT NULL DEFAULT '{}',
           created_at  INTEGER NOT NULL DEFAULT 0,
           updated_at  INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS events_start_idx ON events(start_at);
         -- A decline (or cancellation) of an occurrence that stands for every
         -- later one too (RANGE=THISANDFUTURE).
         ALTER TABLE invite_responses ADD COLUMN this_and_future INTEGER NOT NULL DEFAULT 0;
         CREATE UNIQUE INDEX IF NOT EXISTS events_invite_idx
           ON events(account, invite_uid, recurrence_id) WHERE invite_uid <> '';
         PRAGMA user_version = 19;",
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn migrate_to_v8(conn: &Connection) -> Result<()> {
    migrate_to_v5(conn)?;
    migrate_v6(conn)?;
    migrate_v7(conn)?;
    migrate_v8(conn)
}

fn backfill_recipients(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, json_extract(json, '$.to'), json_extract(json, '$.cc') FROM messages
         WHERE json_extract(json, '$.to') IS NOT NULL OR json_extract(json, '$.cc') IS NOT NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                super::parse_recipients_json(row.get::<_, Option<String>>(1)?),
                super::parse_recipients_json(row.get::<_, Option<String>>(2)?),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, to, cc) in rows {
        conn.execute(
            "UPDATE messages SET recipients = ?2 WHERE id = ?1",
            params![id, super::recipients_index_text(&to, &cc)],
        )?;
    }
    Ok(())
}

pub(super) fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

pub(super) fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Track rendered-body cache format separately from schema version.
///
/// When the render format changes, re-render every HTML-derived body in place
/// (`body_is_rendered = 1`) from its stored `body_html`. Re-rendering rather than
/// nulling keeps the FTS `body` column populated, so full-text search over these
/// messages keeps working across a version bump instead of degrading until each
/// message is reopened.
pub(super) fn invalidate_body_cache_if_needed(conn: &Connection) -> Result<()> {
    let current = meta_get(conn, "body_cache_version")?;
    if current.as_deref() == Some(BODY_CACHE_VERSION) {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "SELECT id, json_extract(json, '$.body_html') FROM messages
             WHERE json_extract(json, '$.body_is_rendered') = 1",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, html) in rows {
            // No HTML source to re-render from: drop the stale body and let the
            // read path fall back if a source ever reappears.
            let body = html.as_deref().map(crate::parse::render_body);
            tx.execute(
                "UPDATE messages SET body = ?2 WHERE id = ?1",
                params![id, body],
            )?;
        }
    }
    meta_set(&tx, "body_cache_version", BODY_CACHE_VERSION)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    fn tmp_db_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meron-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("meron.db")
    }

    #[test]
    fn keyed_open_round_trips() {
        let path = tmp_db_path();
        {
            let conn = open_at_keyed(&path, TEST_KEY).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
                .unwrap();
        }
        // Reopening with the key sees the data.
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "v");
        // The on-disk file is encrypted: a plaintext open can't read the schema.
        assert!(!database_readable(&Connection::open(&path).unwrap()));
    }

    #[test]
    fn legacy_plaintext_is_migrated_to_encrypted() {
        let path = tmp_db_path();
        // Simulate a pre-encryption store: create it plaintext with some data.
        {
            let conn = open_at(&path).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'legacy')", [])
                .unwrap();
        }
        assert!(database_readable(&Connection::open(&path).unwrap()));

        // First keyed open migrates the plaintext file in place, preserving data.
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "legacy");
        drop(conn);

        // The file is now encrypted and still readable with the key on reopen.
        assert!(!database_readable(&Connection::open(&path).unwrap()));
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "legacy");
    }

    #[test]
    fn wrong_key_on_an_encrypted_store_is_named_not_migrated() {
        let path = tmp_db_path();
        {
            let conn = open_at_keyed(&path, TEST_KEY).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
                .unwrap();
        }
        assert!(is_encrypted_database(&path));

        // A keychain that lost our entry hands back a freshly minted key. Opening
        // with it must report that, not run the plaintext migration over an
        // encrypted file.
        let other = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";
        let error = open_at_keyed(&path, other).unwrap_err().to_string();
        assert!(error.contains("encrypted with a different key"), "{error}");

        // The original file is untouched: the real key still opens it.
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "v");
    }

    #[test]
    fn plaintext_and_missing_files_are_not_taken_for_encrypted() {
        let path = tmp_db_path();
        assert!(!is_encrypted_database(&path), "missing file");
        drop(open_at(&path).unwrap());
        assert!(!is_encrypted_database(&path), "plaintext file");
    }

    fn contact_message(conn: &Connection, id: i64, folder: &str, date: i64) {
        conn.execute(
            "INSERT INTO messages(id, account, folder, msg_id, uid, from_name, from_addr, date, json)
             VALUES(?1, 'a', ?2, ?1, ?1, 'Person', 'person@example.com', ?3,
               '{\"to\":[{\"name\":\"Person\",\"addr\":\"person@example.com\"}],\"cc\":[]}')",
            params![id, folder, date],
        )
        .unwrap();
    }

    #[test]
    fn contact_participants_track_moves_deletes_and_lower_dates() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        contact_message(&conn, 1, "INBOX", 10);
        contact_message(&conn, 2, "INBOX", 20);
        let tally = |folder: &str| {
            conn.query_row(
                "SELECT count, last_date FROM contacts WHERE account = 'a' AND folder = ?1",
                [folder],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .unwrap()
        };
        // A sender repeated in To counts only once on each message.
        assert_eq!(tally("INBOX"), Some((2, 20)));
        conn.execute("UPDATE messages SET folder = 'Trash' WHERE id = 2", [])
            .unwrap();
        assert_eq!(tally("INBOX"), Some((1, 10)));
        assert_eq!(tally("Trash"), Some((1, 20)));
        conn.execute(
            "UPDATE messages SET folder = 'INBOX', date = 5 WHERE id = 2",
            [],
        )
        .unwrap();
        assert_eq!(tally("Trash"), None);
        assert_eq!(tally("INBOX"), Some((2, 10)));
        conn.execute("DELETE FROM messages WHERE id = 1", [])
            .unwrap();
        assert_eq!(tally("INBOX"), Some((1, 5)));
        conn.execute("UPDATE messages SET uid = 0 WHERE id = 2", [])
            .unwrap();
        assert_eq!(tally("INBOX"), None);
    }

    #[test]
    fn contact_exclusion_uses_current_metadata_and_name_fallback() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        contact_message(&conn, 1, "Odd mailbox", 10);
        let visible = || {
            super::super::list_contacts(
                &conn,
                "a",
                "",
                20,
                super::super::ContactSort::Frequent,
                false,
            )
            .unwrap()
            .len()
        };
        assert_eq!(visible(), 1);
        conn.execute(
            "INSERT INTO folders(account, name, special_use) VALUES('a', 'Odd mailbox', 'junk')",
            [],
        )
        .unwrap();
        assert_eq!(visible(), 0);
        conn.execute("UPDATE folders SET special_use = 'trash'", [])
            .unwrap();
        assert_eq!(visible(), 0);
        conn.execute("UPDATE folders SET special_use = NULL", [])
            .unwrap();
        assert_eq!(visible(), 1);
        conn.execute("UPDATE messages SET folder = 'Trash'", [])
            .unwrap();
        assert_eq!(visible(), 0);
        conn.execute(
            "INSERT INTO folders(account, name, special_use) VALUES('a', 'Trash', ?1)",
            [super::super::LOCAL_SPECIAL_USE],
        )
        .unwrap();
        assert_eq!(visible(), 1);
    }

    #[test]
    fn contact_v24_backfills_existing_cache_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        // Restore the v23 shape without changing the message cache.
        conn.execute_batch(
            "DROP TRIGGER rule_tag_evaluations_ai; DROP TRIGGER rule_tag_evaluations_au; DROP TRIGGER rule_tag_evaluations_ad;
             DROP TRIGGER rule_evaluations_au;
             DROP TRIGGER rule_arrival_au;
             DROP TABLE rule_activation; DROP TABLE rule_evaluations; DROP TABLE rule_event_sequence;
             DROP TRIGGER rule_events_ai; DROP TRIGGER rule_events_au; DROP TRIGGER rule_events_ad;
             DROP TABLE rule_events; DROP TABLE rule_firings;
             DROP TRIGGER contacts_ai; DROP TRIGGER contacts_ad; DROP TRIGGER contacts_au;
             DROP TABLE contact_participants; DROP TABLE contacts;
             CREATE TABLE contacts(account TEXT, addr_key TEXT, name TEXT, addr TEXT,
               count INTEGER, last_date INTEGER);
             PRAGMA user_version = 23;",
        )
        .unwrap();
        contact_message(&conn, 1, "INBOX", 10);
        contact_message(&conn, 2, "Trash", 20);
        run_migrations(&conn).unwrap();
        run_migrations(&conn).unwrap();
        let entries: i64 = conn
            .query_row("SELECT count(*) FROM contact_participants", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(entries, 2);
        let contacts = super::super::suggest_contacts(&conn, "a", "person", 10).unwrap();
        assert_eq!(contacts.len(), 1);
        conn.execute("DELETE FROM messages WHERE folder = 'INBOX'", [])
            .unwrap();
        assert!(
            super::super::suggest_contacts(&conn, "a", "person", 10)
                .unwrap()
                .is_empty()
        );
    }
}

/// Local notification delivery receipts; deliberately excluded from backups.
fn migrate_v20(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reminder_deliveries (
            key TEXT PRIMARY KEY,
            delivered_at INTEGER NOT NULL
         );
         PRAGMA user_version = 20;",
    )?;
    Ok(())
}

/// An event's own alerts (JSON minutes before start); NULL follows the
/// app-wide default.
fn migrate_v21(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE events ADD COLUMN alerts TEXT;
         PRAGMA user_version = 21;",
    )?;
    Ok(())
}

/// Tags (see `store::tags`). `server_tags` is what a mailbox copy last showed,
/// NULL until it has been looked at. The key lives in an expression index
/// rather than a column so every path that writes a message row is covered.
/// Rules (see `store::rules`): `rule_actions` is what they still have to do on
/// the server, so a failed sync keeps it, and `rule_log` what they did.
fn migrate_v22(conn: &Connection) -> Result<()> {
    conn.execute_batch(&format!(
        "ALTER TABLE messages ADD COLUMN server_tags TEXT;
         CREATE INDEX messages_tag_key_idx ON messages(account, {key}) WHERE uid <> 0;
         CREATE TABLE message_tags (
            account    TEXT NOT NULL,
            tag_key    TEXT NOT NULL,
            tag        TEXT NOT NULL,
            state      INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account, tag_key, tag)
         ) WITHOUT ROWID;
         CREATE INDEX message_tags_tag_idx ON message_tags(account, tag);
         CREATE TABLE rule_actions (
            account     TEXT NOT NULL,
            folder      TEXT NOT NULL,
            uid         INTEGER NOT NULL,
            uidvalidity INTEGER NOT NULL,
            mark_read   INTEGER NOT NULL DEFAULT 0,
            star        INTEGER NOT NULL DEFAULT 0,
            trash       INTEGER NOT NULL DEFAULT 0,
            target      TEXT NOT NULL DEFAULT '',
            copies      TEXT NOT NULL DEFAULT '[]',
            rule_ids    TEXT NOT NULL DEFAULT '[]',
            run         INTEGER NOT NULL DEFAULT 0,
            attempts    INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account, folder, uid, run)
         ) WITHOUT ROWID;
         CREATE TABLE rule_log (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            at      INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            account TEXT NOT NULL,
            subject TEXT NOT NULL DEFAULT '',
            count   INTEGER NOT NULL DEFAULT 1,
            failed  INTEGER NOT NULL DEFAULT 0,
            detail  TEXT NOT NULL DEFAULT ''
         );
         PRAGMA user_version = 22;",
        key = super::tags::TAG_KEY_SQL,
    ))?;
    Ok(())
}

/// When each contact was last seen (migration v23), so the Contacts panel can
/// list the most recent correspondents first. The triggers are replaced with
/// ones that keep `last_date`, and the directory is rebuilt the way v17 built
/// it, now with each entry's newest message date.
fn migrate_v23(conn: &Connection) -> Result<()> {
    let list = |path: &str| {
        format!(
            "SELECT m.id, m.account, m.date, json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
               FROM messages m, json_each(m.json, '{path}') e
               WHERE m.uid <> 0 AND e.type = 'object'"
        )
    };
    conn.execute_batch(&format!(
        "DROP TRIGGER IF EXISTS contacts_ai;
         DROP TRIGGER IF EXISTS contacts_ad;
         DROP TRIGGER IF EXISTS contacts_au;
         ALTER TABLE contacts ADD COLUMN last_date INTEGER NOT NULL DEFAULT 0;
         {triggers}
         DELETE FROM contacts;
         INSERT INTO contacts(account, addr_key, name, addr, count, last_date)
           SELECT account, addr_key, name, max(addr), count(*), max(date) FROM (
             SELECT id, account, max(date) AS date, lower(trim(addr)) AS addr_key,
                    trim(COALESCE(name, '')) AS name, max(trim(addr)) AS addr
             FROM (
               SELECT id, account, date, from_name AS name, from_addr AS addr FROM messages WHERE uid <> 0
               UNION ALL {to} UNION ALL {cc}
             )
             WHERE trim(COALESCE(addr, '')) <> ''
             GROUP BY id, lower(trim(addr)), trim(COALESCE(name, ''))
           )
           WHERE true
           GROUP BY account, addr_key, name;
         PRAGMA user_version = 23;",
        triggers = contacts_triggers(true),
        to = list("$.to"),
        cc = list("$.cc"),
    ))?;
    Ok(())
}

/// Folder-scoped contacts let readers apply the *current* folder roles, even
/// when LIST metadata arrives after the mail. Participant rows are a compact
/// reverse index: removing the newest message can find the next date with an
/// indexed lookup rather than scanning/parsing that correspondent's mail.
fn migrate_v24(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS contacts_ai;
         DROP TRIGGER IF EXISTS contacts_ad;
         DROP TRIGGER IF EXISTS contacts_au;
         DROP TABLE contacts;
         CREATE TABLE contacts (
           account   TEXT NOT NULL,
           folder    TEXT NOT NULL,
           addr_key  TEXT NOT NULL,
           name      TEXT NOT NULL,
           addr      TEXT NOT NULL,
           count     INTEGER NOT NULL,
           last_date INTEGER NOT NULL,
           PRIMARY KEY (account, folder, addr_key, name)
         ) WITHOUT ROWID;
         CREATE TABLE contact_participants (
           message_id INTEGER NOT NULL,
           account    TEXT NOT NULL,
           folder     TEXT NOT NULL,
           addr_key   TEXT NOT NULL,
           name       TEXT NOT NULL,
           addr       TEXT NOT NULL,
           date       INTEGER NOT NULL,
           PRIMARY KEY (message_id, addr_key, name)
         ) WITHOUT ROWID;",
    )?;
    let list = |path: &str| {
        format!(
            "SELECT m.id, m.account, m.folder, m.date,
                    json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
             FROM messages m, json_each(m.json, '{path}') e
             WHERE m.uid <> 0 AND e.type = 'object'"
        )
    };
    // Build once from cached headers, before installing maintenance triggers.
    conn.execute_batch(&format!(
        "INSERT INTO contact_participants(message_id, account, folder, addr_key, name, addr, date)
           SELECT id, account, folder, lower(trim(addr)), trim(COALESCE(name, '')),
                  max(trim(addr)), date
           FROM (
             SELECT id, account, folder, date, from_name AS name, from_addr AS addr
             FROM messages WHERE uid <> 0
             UNION ALL {to} UNION ALL {cc}
           )
           WHERE trim(COALESCE(addr, '')) <> ''
           GROUP BY id, lower(trim(addr)), trim(COALESCE(name, ''));
         CREATE INDEX contact_participants_date_idx
           ON contact_participants(account, folder, addr_key, name, date DESC);
         INSERT INTO contacts(account, folder, addr_key, name, addr, count, last_date)
           SELECT account, folder, addr_key, name, max(addr), count(*), max(date)
           FROM contact_participants GROUP BY account, folder, addr_key, name;",
        to = list("$.to"),
        cc = list("$.cc"),
    ))?;
    let add = format!(
        "INSERT INTO contact_participants(message_id, account, folder, addr_key, name, addr, date)
           SELECT new.id, new.account, new.folder, lower(trim(addr)), trim(COALESCE(name, '')),
                  max(trim(addr)), new.date
           FROM ({}) WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
           GROUP BY lower(trim(addr)), trim(COALESCE(name, ''));",
        contact_occurrences("new"),
    );
    conn.execute_batch(&format!(
        "CREATE TRIGGER contact_participants_ai AFTER INSERT ON contact_participants BEGIN
           INSERT INTO contacts(account, folder, addr_key, name, addr, count, last_date)
           VALUES(new.account, new.folder, new.addr_key, new.name, new.addr, 1, new.date)
           ON CONFLICT(account, folder, addr_key, name) DO UPDATE SET
             count = count + 1, addr = max(addr, excluded.addr),
             last_date = max(last_date, excluded.last_date);
         END;
         CREATE TRIGGER contact_participants_ad AFTER DELETE ON contact_participants BEGIN
           DELETE FROM contacts
             WHERE account = old.account AND folder = old.folder
               AND addr_key = old.addr_key AND name = old.name AND count = 1;
           UPDATE contacts SET count = count - 1,
             last_date = CASE WHEN last_date = old.date THEN (
               SELECT date FROM contact_participants
               WHERE account = old.account AND folder = old.folder
                 AND addr_key = old.addr_key AND name = old.name
               ORDER BY date DESC LIMIT 1
             ) ELSE last_date END
             WHERE account = old.account AND folder = old.folder
               AND addr_key = old.addr_key AND name = old.name;
         END;
         CREATE TRIGGER contacts_ai AFTER INSERT ON messages BEGIN
           {add}
         END;
         CREATE TRIGGER contacts_ad AFTER DELETE ON messages BEGIN
           DELETE FROM contact_participants WHERE message_id = old.id;
         END;
         CREATE TRIGGER contacts_au
           AFTER UPDATE OF id, account, folder, uid, from_name, from_addr, json, date ON messages
           WHEN old.id IS NOT new.id OR old.account IS NOT new.account
             OR old.folder IS NOT new.folder OR (old.uid <> 0) IS NOT (new.uid <> 0)
             OR old.from_name IS NOT new.from_name OR old.from_addr IS NOT new.from_addr
             OR json_extract(old.json, '$.to', '$.cc') IS NOT json_extract(new.json, '$.to', '$.cc')
             OR old.date IS NOT new.date
         BEGIN
           DELETE FROM contact_participants WHERE message_id = old.id;
           {add}
         END;
         PRAGMA user_version = 24;"
    ))?;
    Ok(())
}

/// Event clocks are independent of envelope JSON, which is replaced on sync.
/// Read transitions include bulk updates and changes observed from other clients.
fn migrate_v25(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE rule_events (
            message INTEGER PRIMARY KEY,
            arrived_at INTEGER,
            read_at INTEGER
         );
         INSERT INTO rule_events (message) SELECT id FROM messages WHERE uid <> 0;
         CREATE TABLE rule_firings (
            message INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            PRIMARY KEY (message, rule_id)
         ) WITHOUT ROWID;
         CREATE TRIGGER rule_events_ai AFTER INSERT ON messages WHEN new.uid <> 0 BEGIN
            INSERT INTO rule_events (message) VALUES (new.id);
         END;
         CREATE TRIGGER rule_events_au AFTER UPDATE OF seen ON messages
         WHEN new.uid <> 0 AND old.seen <> new.seen BEGIN
            UPDATE rule_events SET read_at = CASE WHEN new.seen <> 0
                THEN unixepoch() ELSE NULL END WHERE message = new.id;
         END;
         CREATE TRIGGER rule_events_ad AFTER DELETE ON messages BEGIN
            DELETE FROM rule_events WHERE message = old.id;
            DELETE FROM rule_firings WHERE message = old.id;
         END;
         PRAGMA user_version = 25;",
    )?;
    Ok(())
}

/// Sequence numbers distinguish events before and after a rule is saved even
/// within the same second. Existing v25 clocks remain eligible for saved rules.
fn migrate_v26(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE rule_event_sequence (id INTEGER PRIMARY KEY CHECK (id = 1), value INTEGER NOT NULL);
         INSERT INTO rule_event_sequence VALUES (1, 1);
         ALTER TABLE rule_events ADD COLUMN arrived_sequence INTEGER NOT NULL DEFAULT 1;
         ALTER TABLE rule_events ADD COLUMN read_sequence INTEGER NOT NULL DEFAULT 1;
         CREATE TABLE rule_activation (rule_id TEXT PRIMARY KEY, event_sequence INTEGER NOT NULL) WITHOUT ROWID;
         CREATE TABLE rule_evaluations (
            message INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            event_sequence INTEGER NOT NULL,
            query TEXT NOT NULL,
            PRIMARY KEY (message, rule_id)
         ) WITHOUT ROWID;
         CREATE TRIGGER rule_arrival_au AFTER UPDATE OF arrived_at ON rule_events
         WHEN old.arrived_at IS NULL AND new.arrived_at IS NOT NULL BEGIN
            UPDATE rule_event_sequence SET value = value + 1;
            UPDATE rule_events SET arrived_sequence = (SELECT value FROM rule_event_sequence) WHERE message = new.message;
         END;
         DROP TRIGGER rule_events_au;
         CREATE TRIGGER rule_events_au AFTER UPDATE OF seen ON messages
         WHEN new.uid <> 0 AND old.seen <> new.seen BEGIN
            UPDATE rule_event_sequence SET value = value + 1 WHERE new.seen <> 0;
            UPDATE rule_events SET
                read_at = CASE WHEN new.seen <> 0 THEN unixepoch() ELSE NULL END,
                read_sequence = (SELECT value FROM rule_event_sequence)
            WHERE message = new.id;
         END;
         DROP TRIGGER rule_events_ad;
         CREATE TRIGGER rule_events_ad AFTER DELETE ON messages BEGIN
            DELETE FROM rule_events WHERE message = old.id;
            DELETE FROM rule_firings WHERE message = old.id;
            DELETE FROM rule_evaluations WHERE message = old.id;
         END;
         PRAGMA user_version = 26;",
    )?;
    Ok(())
}

/// A later envelope or metadata fill can correct headers used by a rule.
/// Reuse negative checks only while those values remain unchanged.
fn migrate_v27(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TRIGGER rule_evaluations_au AFTER UPDATE OF subject, from_name, from_addr, date, json, files ON messages
         WHEN new.subject IS NOT old.subject OR new.from_name IS NOT old.from_name
           OR new.from_addr IS NOT old.from_addr OR new.date IS NOT old.date
           OR new.json IS NOT old.json OR new.files IS NOT old.files BEGIN
            DELETE FROM rule_evaluations WHERE message = new.id;
         END;
         PRAGMA user_version = 27;",
    )?;
    Ok(())
}

/// Negative checks remain reusable for mutable conditions until their inputs
/// change. Tag keys cover all mailbox copies carrying the same Message-ID.
fn migrate_v28(conn: &Connection) -> Result<()> {
    let key = super::tags::TAG_KEY_SQL;
    conn.execute_batch(&format!(
        "DROP TRIGGER rule_evaluations_au;
         CREATE TRIGGER rule_evaluations_au AFTER UPDATE OF subject, from_name, from_addr, date, json, files, recipients, seen, starred ON messages
         WHEN new.subject IS NOT old.subject OR new.from_name IS NOT old.from_name
           OR new.from_addr IS NOT old.from_addr OR new.date IS NOT old.date
           OR new.json IS NOT old.json OR new.files IS NOT old.files
           OR new.recipients IS NOT old.recipients OR new.seen IS NOT old.seen
           OR new.starred IS NOT old.starred BEGIN
            DELETE FROM rule_evaluations WHERE message = new.id;
         END;
         CREATE TRIGGER rule_tag_evaluations_ai AFTER INSERT ON message_tags BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = new.account AND {key} = new.tag_key);
         END;
         CREATE TRIGGER rule_tag_evaluations_ad AFTER DELETE ON message_tags BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = old.account AND {key} = old.tag_key);
         END;
         CREATE TRIGGER rule_tag_evaluations_au AFTER UPDATE ON message_tags
         WHEN new.state IS NOT old.state OR new.tag IS NOT old.tag
           OR new.account IS NOT old.account OR new.tag_key IS NOT old.tag_key BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = old.account AND {key} = old.tag_key
                 UNION SELECT id FROM messages WHERE uid <> 0 AND account = new.account AND {key} = new.tag_key);
         END;
         PRAGMA user_version = 28;"
    ))?;
    Ok(())
}

/// OpenPGP keys (nonbili/meron#46): own and peer keys, per-account switch,
/// dismissed encryption offers. See `openpgp::store`.
fn migrate_v29(conn: &Connection) -> Result<()> {
    conn.execute_batch(crate::openpgp::store::DDL)?;
    conn.execute_batch("PRAGMA user_version = 29;")?;
    Ok(())
}
