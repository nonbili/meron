//! Local SQLite store (rusqlite, bundled) — the single source of truth.
//!
//! `meron.db` (renamed from the old `cache.sqlite`) holds *all* accounts (mail and
//! RSS), fetched folders/messages, per-folder UID sync state, and RSS
//! subscriptions, so the UI renders instantly from disk and history persists
//! across runs. The desktop bridge sets `MERON_CORE_DB` to the active app
//! profile (`meron` or `meron-dev`); standalone runs default under
//! `~/.config/meron`.
//!
//! Accounts and messages share one table each, with a catch-all JSON column
//! absorbing per-engine fields so new account kinds don't force schema
//! migrations (accounts: `config` for mail connection metadata, plus `prefs` for
//! user preferences; messages: `json` for rss item fields). Mail's hot-path
//! columns (integer `uid`, `seen`, `thread_key`) stay typed; JSON carries the
//! divergent tail.

mod db;

pub use db::{app_dir, now_unix, open};

#[allow(dead_code)]
pub fn open_at(path: impl AsRef<std::path::Path>) -> Result<Connection> {
    db::open_at(path)
}

#[allow(dead_code)]
pub fn open_at_keyed(path: impl AsRef<std::path::Path>, key: &str) -> Result<Connection> {
    db::open_at_keyed(path, key)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn run_migrations(conn: &Connection) -> Result<()> {
    db::run_migrations(conn)
}

use anyhow::Result;
use rusqlite::Connection;

use crate::imap::MessageHeader;

pub const DEFAULT_RSS_SYNC_INTERVAL_MINUTES: u64 = 60;

mod accounts;
mod contacts;
mod folders;
mod messages;
mod rss;
mod search;
mod settings;
mod tasks;
mod threads;
mod updates;

pub use accounts::*;
pub use contacts::*;
pub use folders::*;
pub use messages::*;
pub use rss::*;
pub use search::*;
pub use settings::*;
pub use tasks::*;
pub use threads::*;
pub use updates::*;

fn message_header_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageHeader> {
    let uid = row.get(0)?;
    Ok(MessageHeader {
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
            .unwrap_or_else(|| format!("uid:{}", uid)),
        to: parse_recipients_json(row.get::<_, Option<String>>(8)?),
        folder: String::new(),
        ..Default::default()
    })
}

/// [`message_header_from_row`] for the cached search reads, which select the
/// Message-ID as a tenth column. A search page spans folders, so the grouping
/// that turns it into cards needs the id to see the two copies of a self-sent
/// message as one message rather than two.
fn search_header_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageHeader> {
    let mut header = message_header_from_row(row)?;
    header.message_id = row.get(9)?;
    Ok(header)
}

/// Parse a cached `$.to` JSON array into recipients, tolerating null/garbage.
fn parse_recipients_json(json: Option<String>) -> Vec<crate::imap::Recipient> {
    json.and_then(|s| serde_json::from_str::<Vec<crate::imap::Recipient>>(&s).ok())
        .unwrap_or_default()
}

fn escape_like(value: String) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests;
