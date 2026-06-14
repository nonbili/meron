use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;

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
";

const BODY_CACHE_VERSION: &str = "1";

pub fn open() -> Result<Connection> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(&path).with_context(|| format!("open db {}", path.display()))?;
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
/// Each step runs only when the DB is below its target version, inside its own
/// transaction that also bumps `user_version` — so a crash mid-migration rolls
/// back cleanly and the step re-runs next launch. Append-only: to evolve the
/// schema, add a new `if version < N` block; never edit or reorder a shipped
/// one. (Cache-only invalidation lives in `invalidate_body_cache_if_needed`,
/// kept off this counter so a render change doesn't look like a schema change.)
pub(super) fn run_migrations(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version < 1 {
        migrate_v1(conn)?;
    }

    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(ACCOUNTS_DDL)?;
    tx.execute_batch(MESSAGES_DDL)?;
    tx.execute_batch(SCHEMA)?;
    tx.execute_batch("PRAGMA user_version = 1;")?;
    tx.commit()?;
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
