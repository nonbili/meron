use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

const BODY_CACHE_VERSION: &str = "1";

pub fn open() -> Result<Connection> {
    let path = db_path();
    match crate::secrets::db_key()? {
        Some(key) => open_at_keyed(path, &key),
        // Keyring disabled (tests/headless): keep the store plaintext as before.
        None => open_at(path),
    }
}

/// Open an unencrypted store. SQLCipher leaves a database untouched until a key
/// is set, so this stays byte-compatible with pre-encryption databases and is
/// what tests and the headless/`MERON_KEYRING=off` path use.
pub fn open_at(path: impl AsRef<Path>) -> Result<Connection> {
    open_inner(path.as_ref(), None)
}

/// Open an encrypted store, keying the connection with `key` (64 hex chars = a
/// raw 32-byte SQLCipher key). A legacy plaintext database at `path` is migrated
/// in place to an encrypted one on first keyed open.
pub fn open_at_keyed(path: impl AsRef<Path>, key: &str) -> Result<Connection> {
    open_inner(path.as_ref(), Some(key))
}

fn open_inner(path: &Path, key: Option<&str>) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = open_keyed_connection(path, key)?;
    conn.busy_timeout(Duration::from_millis(5000))
        .context("set busy timeout")?;
    // WAL + synchronous=NORMAL is the durable-but-fast desktop combo; busy_timeout
    // avoids spurious SQLITE_BUSY now that a reader can overlap a writer under WAL.
    conn.execute_batch(
        // foreign_keys is per-connection and off by default. No table declares a
        // FOREIGN KEY today, so this is a no-op for now — it's set so that any FK
        // constraints added in a future migration are enforced automatically.
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )
    .context("set connection pragmas")?;
    run_migrations(&conn).context("run migrations")?;
    invalidate_body_cache_if_needed(&conn)?;
    Ok(conn)
}

/// Opens `path` and applies the SQLCipher `key` (if any). When the key is set
/// but the file turns out to be an unencrypted legacy database, it is migrated
/// in place to an encrypted database before returning.
fn open_keyed_connection(path: &Path, key: Option<&str>) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("open db {}", path.display()))?;
    let Some(key) = key else {
        return Ok(conn);
    };
    apply_key(&conn, key)?;
    if database_readable(&conn) {
        return Ok(conn);
    }
    // The key didn't unlock the file: it predates encryption and is plaintext
    // (a wrong/empty file would also land here, but the key is persistent so in
    // practice this is the one-time legacy-plaintext upgrade). Migrate in place.
    drop(conn);
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

fn db_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_CORE_DB") {
        return PathBuf::from(path);
    }
    config_dir().join("meron.db")
}

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/meron")
}

// ---- Migrations -------------------------------------------------------------

/// Apply schema migrations in order, tracked by SQLite's `PRAGMA user_version`.
///
/// Pending steps run inside one IMMEDIATE transaction that also bumps
/// `user_version`, so concurrent first-open callers serialize before reading
/// the version. A crash mid-migration rolls back cleanly and the step re-runs
/// next launch. Append-only: to evolve the schema, add a new `if version < N`
/// block; never edit or reorder a shipped one. (Cache-only invalidation lives in
/// `invalidate_body_cache_if_needed`, kept off this counter so a render change
/// doesn't look like a schema change.)
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

    tx.commit()?;
    Ok(())
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

fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
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

pub(super) fn now_unix() -> i64 {
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
}
