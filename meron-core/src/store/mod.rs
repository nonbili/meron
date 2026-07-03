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

pub use db::open;

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
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::imap::{Folder, MessageHeader};
use crate::parse::{Attachment, Message};

pub const DEFAULT_RSS_SYNC_INTERVAL_MINUTES: u64 = 60;

mod accounts;
mod settings;

pub use accounts::*;
pub use settings::*;

// ---- Folders (mail) ---------------------------------------------------------

pub fn upsert_folders(conn: &Connection, account: &str, folders: &[Folder]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for folder in folders {
        tx.execute(
            "INSERT INTO folders(account, name, delimiter, special_use) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(account, name) DO UPDATE SET
               delimiter = excluded.delimiter,
               special_use = excluded.special_use",
            params![account, folder.name, folder.delimiter, folder.special_use],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Guarantee a folder row exists for a folder we have messages in, without
/// touching its delimiter. The full folder LIST sync (`upsert_folders`) only
/// runs when an account is opened directly, so in the unified view a freshly
/// added account never gets folder rows — and `get_folders` (which JOINs the
/// folders table) would then report zero unread even with unseen mail in the
/// store, leaving the tray dot and unread badges dark. Calling this on every
/// message sync keeps the count honest and self-heals existing accounts.
pub fn ensure_folder(conn: &Connection, account: &str, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO folders(account, name, delimiter) VALUES(?1, ?2, NULL)",
        params![account, name],
    )?;
    Ok(())
}

pub fn get_folders(conn: &Connection, account: &str) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare(
        "SELECT f.name, f.delimiter, f.special_use,
                (SELECT COUNT(*) FROM messages m
                  WHERE m.account = f.account AND m.folder = f.name AND m.seen = 0) AS unread
           FROM folders f WHERE f.account = ?1 ORDER BY f.name",
    )?;
    let rows = stmt.query_map(params![account], |row| {
        let name = row.get::<_, String>(0)?;
        let special_use = row.get::<_, Option<String>>(2)?;
        Ok(Folder {
            role: classify_folder_role(&name, special_use.as_deref()).to_string(),
            name,
            delimiter: row.get(1)?,
            special_use,
            unread: row.get::<_, i64>(3)? as u32,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn classify_folder_role(name: &str, special_use: Option<&str>) -> &'static str {
    match special_use.unwrap_or_default() {
        "inbox" => "inbox",
        "sent" => "sent",
        "drafts" => "drafts",
        "trash" => "trash",
        "junk" => "junk",
        "archive" | "all" => "archive",
        _ if name.eq_ignore_ascii_case("INBOX") => "inbox",
        _ if crate::imap::looks_like_sent(name) => "sent",
        _ if crate::imap::looks_like_drafts(name) => "drafts",
        _ if crate::imap::looks_like_trash(name) => "trash",
        _ if crate::imap::looks_like_junk(name) => "junk",
        _ if crate::imap::looks_like_archive(name) => "archive",
        _ => "folder",
    }
}

// ---- Messages (mail) --------------------------------------------------------

pub fn upsert_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    messages: &[MessageHeader],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for m in messages {
        // Store recipient lists as JSON. Skip empty lists so a later flag-only
        // resync (which carries no envelope) can't clobber recipients we already
        // cached with NULLs.
        let mut extra = serde_json::Map::new();
        if !m.to.is_empty() {
            extra.insert("to".to_string(), json!(m.to));
        }
        if !m.cc.is_empty() {
            extra.insert("cc".to_string(), json!(m.cc));
        }
        if !m.message_id.is_empty() {
            extra.insert("message_id".to_string(), json!(m.message_id));
        }
        if let Some(gmail_msg_id) = m.gmail_msg_id {
            extra.insert("gmail_msg_id".to_string(), json!(gmail_msg_id));
        }
        if !m.in_reply_to.is_empty() {
            extra.insert("in_reply_to".to_string(), json!(m.in_reply_to));
        }
        let extra_json = Value::Object(extra).to_string();
        let thread_key = resolve_message_thread_key(&tx, account, &m.thread_key, &m.in_reply_to)?;
        tx.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key, json)
             VALUES(?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(account, folder, msg_id) DO UPDATE SET
               subject    = excluded.subject,
               from_name  = excluded.from_name,
               from_addr  = excluded.from_addr,
               date       = excluded.date,
               seen       = excluded.seen,
               starred    = excluded.starred,
               thread_key = excluded.thread_key,
               json       = json_patch(messages.json, excluded.json)",
            params![
                account,
                folder,
                m.uid,
                m.subject,
                m.from_name,
                m.from_addr,
                m.date,
                m.seen as i64,
                m.starred as i64,
                thread_key,
                extra_json
            ],
        )?;
    }
    reconcile_account_thread_keys(&tx, account)?;
    tx.commit()?;
    Ok(())
}

fn resolve_message_thread_key(
    conn: &rusqlite::Transaction<'_>,
    account: &str,
    thread_key: &str,
    in_reply_to: &str,
) -> Result<String> {
    let key = thread_key.trim();
    let parent_id = in_reply_to.trim();
    if key.is_empty()
        || parent_id.is_empty()
        || !key.eq_ignore_ascii_case(parent_id)
        || key.starts_with("uid:")
        || key.starts_with("gmthrid:")
    {
        return Ok(thread_key.to_string());
    }

    let inherited = conn
        .query_row(
            "SELECT thread_key FROM messages
             WHERE account = ?1
               AND lower(COALESCE(json_extract(json, '$.message_id'), '')) = lower(?2)
               AND COALESCE(thread_key, '') <> ''
             ORDER BY date ASC, uid ASC
             LIMIT 1",
            params![account, parent_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    Ok(inherited.unwrap_or_else(|| thread_key.to_string()))
}

fn reconcile_account_thread_keys(conn: &rusqlite::Transaction<'_>, account: &str) -> Result<()> {
    // A fresh folder sync may upsert replies newest-first, so a child can arrive
    // before the parent row whose Message-ID would reveal the canonical root key.
    // Re-run the same parent-key inheritance over the complete account cache.
    for _ in 0..16 {
        let changed = conn.execute(
            "UPDATE messages AS child
                SET thread_key = (
                    SELECT parent.thread_key
                      FROM messages parent
                     WHERE parent.account = child.account
                       AND lower(COALESCE(json_extract(parent.json, '$.message_id'), '')) =
                           lower(COALESCE(json_extract(child.json, '$.in_reply_to'), ''))
                       AND COALESCE(parent.thread_key, '') <> ''
                       AND parent.thread_key <> child.thread_key
                     ORDER BY parent.date ASC, parent.uid ASC
                     LIMIT 1
                )
              WHERE child.account = ?1
                AND child.uid <> 0
                AND COALESCE(child.thread_key, '') <> ''
                AND COALESCE(json_extract(child.json, '$.in_reply_to'), '') <> ''
                AND lower(COALESCE(child.thread_key, '')) =
                    lower(COALESCE(json_extract(child.json, '$.in_reply_to'), ''))
                AND child.thread_key NOT LIKE 'uid:%'
                AND child.thread_key NOT LIKE 'gmthrid:%'
                AND EXISTS (
                    SELECT 1
                      FROM messages parent
                     WHERE parent.account = child.account
                       AND lower(COALESCE(json_extract(parent.json, '$.message_id'), '')) =
                           lower(COALESCE(json_extract(child.json, '$.in_reply_to'), ''))
                       AND COALESCE(parent.thread_key, '') <> ''
                       AND parent.thread_key <> child.thread_key
                )",
            params![account],
        )?;
        if changed == 0 {
            break;
        }
    }
    Ok(())
}

pub fn get_recent_page(
    conn: &Connection,
    account: &str,
    folder: &str,
    limit: u32,
    before_cursor: Option<(i64, u32)>,
    unread_only: bool,
) -> Result<(Vec<MessageHeader>, Option<String>)> {
    let probe = limit.saturating_add(1);
    // Newest-first by send time. The cursor is the (date, uid) of the last row of
    // the previous page; uid is the keyset tiebreaker because `date` is not unique
    // (two messages can share a second), so it gives a stable, gap-free walk.
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to') FROM messages
         WHERE account = ?1 AND folder = ?2
           AND (?6 = 0 OR seen = 0)
           AND (?3 IS NULL
                OR date < ?3
                OR (date = ?3 AND uid < ?4))
         ORDER BY date DESC, uid DESC LIMIT ?5",
    )?;
    let cursor_date = before_cursor.map(|(date, _)| date);
    let cursor_uid = before_cursor.map(|(_, uid)| uid as i64).unwrap_or(0);
    let rows = stmt.query_map(
        params![
            account,
            folder,
            cursor_date,
            cursor_uid,
            probe as i64,
            unread_only as i64
        ],
        |row| {
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
        },
    )?;
    let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = out.len() > limit as usize;
    if has_more {
        out.truncate(limit as usize);
    }
    let next_cursor = if has_more {
        out.last()
            .map(|header| format!("date:{}:{}", header.date, header.uid))
    } else {
        None
    };
    Ok((out, next_cursor))
}

fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn mail_identity_from_parts(
    folder: &str,
    uid: u32,
    gmail_msg_id: Option<u64>,
    message_id: &str,
) -> String {
    if let Some(gmail_msg_id) = gmail_msg_id {
        return format!("gmail:{gmail_msg_id}");
    }
    let message_id = message_id.trim();
    if !message_id.is_empty() {
        return format!("message-id:{}", message_id.to_lowercase());
    }
    format!("uid:{}:{uid}", folder.to_lowercase())
}

fn message_identity(header: &MessageHeader, folder: &str) -> String {
    mail_identity_from_parts(folder, header.uid, header.gmail_msg_id, &header.message_id)
}

fn gmail_msg_id_from_json(value: Option<String>) -> Option<u64> {
    value.and_then(|value| value.parse::<u64>().ok())
}

pub fn backfill_observed_mail_identities(conn: &Connection, account: &str) -> Result<()> {
    let now = now_epoch_seconds();
    conn.execute(
        "INSERT OR IGNORE INTO observed_mail_identities(account, identity, first_seen_at)
         SELECT account,
                CASE
                  WHEN json_extract(json, '$.gmail_msg_id') IS NOT NULL
                    THEN 'gmail:' || json_extract(json, '$.gmail_msg_id')
                  WHEN COALESCE(json_extract(json, '$.message_id'), '') <> ''
                    THEN 'message-id:' || lower(json_extract(json, '$.message_id'))
                  ELSE 'uid:' || lower(folder) || ':' || uid
                END,
                ?2
         FROM messages
         WHERE account = ?1 AND uid <> 0",
        params![account, now],
    )?;
    Ok(())
}

/// Record message identities and return the subset that had not been observed
/// before this call.
fn record_observed_mail_identities(
    conn: &Connection,
    account: &str,
    folder: &str,
    messages: &[MessageHeader],
) -> Result<std::collections::HashSet<String>> {
    let now = now_epoch_seconds();
    let tx = conn.unchecked_transaction()?;
    let mut new_identities = std::collections::HashSet::new();
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(?1, ?2, ?3)",
        )?;
        for message in messages {
            let identity = message_identity(message, folder);
            let inserted = stmt.execute(params![account, &identity, now])?;
            if inserted > 0 {
                new_identities.insert(identity);
            }
        }
    }
    tx.commit()?;
    Ok(new_identities)
}

/// Unread INBOX messages in the UID range that appeared during the last sync.
pub fn new_unread_inbox_summary(
    conn: &Connection,
    account: &str,
    uid_next_before: u32,
    uid_next_after: u32,
    synced_messages: &[MessageHeader],
) -> Result<Option<(u32, MessageHeader)>> {
    if uid_next_before == 0 || uid_next_after <= uid_next_before {
        return Ok(None);
    }
    let newly_observed = record_observed_mail_identities(conn, account, "INBOX", synced_messages)?;
    if newly_observed.is_empty() {
        return Ok(None);
    }

    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to'),
                CAST(json_extract(json, '$.gmail_msg_id') AS TEXT),
                json_extract(json, '$.message_id') FROM messages
         WHERE account = ?1 AND folder = 'INBOX'
           AND uid >= ?2 AND uid < ?3 AND seen = 0
         ORDER BY uid DESC",
    )?;
    let rows = stmt.query_map(
        params![account, uid_next_before as i64, uid_next_after as i64],
        |row| {
            let mut header = message_header_from_row(row)?;
            header.gmail_msg_id = gmail_msg_id_from_json(row.get::<_, Option<String>>(9)?);
            header.message_id = row.get::<_, Option<String>>(10)?.unwrap_or_default();
            Ok(header)
        },
    )?;
    let headers = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|header| newly_observed.contains(&message_identity(header, "INBOX")))
        .collect::<Vec<_>>();
    let Some(latest) = headers.first().cloned() else {
        return Ok(None);
    };
    Ok(Some((headers.len() as u32, latest)))
}

pub fn search_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // The trigram index needs >= 3 codepoints; serve shorter queries (common for
    // CJK, where words are often 2 characters) with the scoped LIKE scan instead.
    if q.chars().count() < 3 {
        return search_messages_like(conn, account, folder, q, limit);
    }
    // Whole query as one quoted FTS phrase -> trigram substring match (doubling
    // any `"` so user input can't change the query). Same substring semantics as
    // the LIKE path, just index-backed.
    let match_query = format!("\"{}\"", q.replace('"', "\"\""));
    let mut stmt = conn.prepare(
        "SELECT m.uid, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred,
                m.thread_key, json_extract(m.json, '$.to')
         FROM messages_fts f
         JOIN messages m ON m.id = f.rowid
         WHERE f.messages_fts MATCH ?1
           AND m.account = ?2 AND m.folder = ?3 AND m.uid <> 0
         ORDER BY m.date DESC, m.uid DESC LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        params![match_query, account, folder, limit],
        message_header_from_row,
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Substring search via a scoped table scan. Used for queries too short for the
/// trigram FTS index (< 3 codepoints).
fn search_messages_like(
    conn: &Connection,
    account: &str,
    folder: &str,
    q: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>> {
    let like = format!("%{}%", escape_like(q.to_lowercase()));
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to') FROM messages
         WHERE account = ?1 AND folder = ?2 AND uid <> 0
           AND (
             lower(COALESCE(subject, '')) LIKE ?3 ESCAPE '\\'
             OR lower(COALESCE(from_name, '')) LIKE ?3 ESCAPE '\\'
             OR lower(COALESCE(from_addr, '')) LIKE ?3 ESCAPE '\\'
             OR lower(COALESCE(body, '')) LIKE ?3 ESCAPE '\\'
           )
         ORDER BY date DESC, uid DESC LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        params![account, folder, like, limit],
        message_header_from_row,
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// A correspondent surfaced for recipient autocomplete.
#[derive(serde::Serialize)]
pub struct Contact {
    pub name: String,
    pub addr: String,
}

/// Suggest contacts for recipient autocomplete, drawn from both the senders and
/// the To/Cc recipients of cached messages. Results are distinct by address and
/// ranked by how often the address appears. An empty `query` returns the top
/// correspondents; otherwise we match the substring against address and display
/// name. When `account` is empty we search across all accounts (unified compose).
///
/// Recipient lists are stored as JSON, so aggregation happens in Rust rather than
/// SQL: we pull the candidate rows (coarsely pre-filtered by LIKE) and fold them
/// into a per-address tally.
pub fn suggest_contacts(
    conn: &Connection,
    account: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<Contact>> {
    use std::collections::HashMap;

    let q = query.trim().to_lowercase();
    let like = format!("%{}%", escape_like(q.clone()));
    let account_filter = if account.is_empty() {
        "?1 = ''"
    } else {
        "account = ?1"
    };
    // Pre-filter: keep rows where the query appears anywhere in the sender or the
    // (JSON) recipient lists. With an empty query every row qualifies.
    let sql = format!(
        "SELECT from_name, from_addr, json_extract(json, '$.to'), json_extract(json, '$.cc')
         FROM messages
         WHERE {account_filter}
           AND uid <> 0
           AND (?2 = ''
                OR lower(COALESCE(from_addr, '')) LIKE ?3 ESCAPE '\\'
                OR lower(COALESCE(from_name, '')) LIKE ?3 ESCAPE '\\'
                OR lower(COALESCE(json_extract(json, '$.to'), '')) LIKE ?3 ESCAPE '\\'
                OR lower(COALESCE(json_extract(json, '$.cc'), '')) LIKE ?3 ESCAPE '\\')"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![account, q, like], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    // Tally per lowercased address: first display name seen wins (falling back to
    // a later non-empty one), plus an occurrence count for ranking.
    struct Tally {
        name: String,
        addr: String,
        count: u32,
    }
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let mut bump = |name: String, addr: String| {
        let addr = addr.trim();
        if addr.is_empty() {
            return;
        }
        let entry = tallies.entry(addr.to_lowercase()).or_insert_with(|| Tally {
            name: String::new(),
            addr: addr.to_string(),
            count: 0,
        });
        entry.count += 1;
        if entry.name.is_empty() && !name.trim().is_empty() {
            entry.name = name.trim().to_string();
        }
    };

    for row in rows {
        let (from_name, from_addr, to_json, cc_json) = row?;
        bump(from_name, from_addr);
        for json in [to_json, cc_json].into_iter().flatten() {
            if let Ok(list) = serde_json::from_str::<Vec<crate::imap::Recipient>>(&json) {
                for r in list {
                    bump(r.name, r.addr);
                }
            }
        }
    }

    // Keep only the addresses that actually match the query (the SQL pre-filter
    // admits whole rows, so a sender match can drag in non-matching recipients).
    let mut out: Vec<Tally> = tallies
        .into_values()
        .filter(|t| {
            q.is_empty() || t.addr.to_lowercase().contains(&q) || t.name.to_lowercase().contains(&q)
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.addr.to_lowercase().cmp(&b.addr.to_lowercase()))
    });
    out.truncate(limit as usize);
    Ok(out
        .into_iter()
        .map(|t| Contact {
            name: t.name,
            addr: t.addr,
        })
        .collect())
}

pub fn get_starred(
    conn: &Connection,
    account: &str,
    folder: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to') FROM messages
         WHERE account = ?1 AND folder = ?2 AND starred <> 0 AND uid <> 0
         ORDER BY date DESC, uid DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![account, folder, limit], message_header_from_row)?;
    let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    for header in &mut out {
        header.folder = folder.to_string();
    }
    Ok(out)
}

/// Every starred mail message across all accounts and folders, newest first.
/// RSS rows carry `uid = 0` and are excluded; `rss::starred_items` covers them.
pub fn get_starred_all_accounts(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<(String, MessageHeader)>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to'), account, folder FROM messages
         WHERE starred <> 0 AND uid <> 0
         ORDER BY date DESC, uid DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        let mut header = message_header_from_row(row)?;
        header.folder = row.get(10)?;
        Ok((row.get::<_, String>(9)?, header))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_thread_headers(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key FROM messages
         WHERE account = ?1 AND folder = ?2 AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?3
         ORDER BY date ASC, uid ASC",
    )?;
    let rows = stmt.query_map(params![account, folder, thread_key], |row| {
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
            folder: String::new(),
            ..Default::default()
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn resolve_message_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    subject_filter: Option<&str>,
    uid: Option<u32>,
    explicit_uids: &[u32],
) -> Result<Vec<u32>> {
    if !explicit_uids.is_empty() {
        return Ok(explicit_uids.to_vec());
    }
    if thread_key.is_empty() {
        return Ok(uid.into_iter().collect());
    }
    let mut headers = get_thread_headers(conn, account, folder, thread_key)?;
    if let Some(filter) = subject_filter {
        headers.retain(|header| thread_grouping_subject(&header.subject) == filter);
    }
    Ok(headers.into_iter().map(|header| header.uid).collect())
}

#[derive(Clone)]
pub struct ThreadCard {
    pub thread_key: String,
    pub original_thread_key: Option<String>,
    pub header: MessageHeader,
    pub unread_count: u32,
}

pub fn group_thread_cards(messages: Vec<MessageHeader>, default_folder: &str) -> Vec<ThreadCard> {
    use std::collections::HashMap;

    #[derive(Default)]
    struct RootSubject {
        display: String,
        group: String,
        uid: u32,
    }

    let mut roots: HashMap<String, RootSubject> = HashMap::new();
    for message in &messages {
        if message.uid == 0 {
            continue;
        }
        let thread_key = effective_thread_key(message);
        let entry = roots.entry(thread_key).or_default();
        if entry.uid == 0 || message.uid < entry.uid {
            entry.uid = message.uid;
            entry.display = normalize_thread_subject(&message.subject);
            entry.group = thread_grouping_subject(&message.subject);
        }
    }

    let mut groups: HashMap<String, ThreadCard> = HashMap::new();
    let mut order = Vec::new();
    for message in messages {
        if message.uid == 0 {
            continue;
        }
        let base_key = effective_thread_key(&message);
        let branch = should_branch_thread_by_subject(&base_key);
        let group_subject = if branch {
            thread_grouping_subject(&message.subject)
        } else {
            String::new()
        };
        let compound_key = card_thread_key(&message);
        let card = groups.entry(compound_key.clone()).or_insert_with(|| {
            order.push(compound_key.clone());
            let root = roots.get(&base_key);
            let mut header = message.clone();
            if header.folder.is_empty() {
                header.folder = default_folder.to_string();
            }
            header.thread_key = compound_key.clone();
            let title = root.map(|root| root.display.as_str()).unwrap_or_default();
            if !title.is_empty() {
                header.subject = title.to_string();
            }
            let original_thread_key = if branch
                && root
                    .map(|root| root.group.as_str() != group_subject.as_str())
                    .unwrap_or(false)
            {
                root.map(|root| branch_compound_key(&base_key, &root.group))
            } else {
                None
            };
            ThreadCard {
                thread_key: compound_key.clone(),
                original_thread_key,
                header,
                unread_count: 0,
            }
        });
        if !message.seen {
            card.unread_count += 1;
            card.header.seen = false;
        }
        if message.starred {
            card.header.starred = true;
        }
    }

    order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .collect()
}

fn effective_thread_key(message: &MessageHeader) -> String {
    if message.thread_key.is_empty() {
        format!("uid:{}", message.uid)
    } else {
        message.thread_key.clone()
    }
}

/// The branch-aware thread key a list card for `message` carries: the root
/// thread key joined with the message's grouping subject for branchable
/// threads, or the bare root for uid:/gmthrid: keys. Every path that mints a
/// clickable thread id (thread lists, starred items, new-mail notifications)
/// must use this so the id matches the card the grouping produced.
pub fn card_thread_key(message: &MessageHeader) -> String {
    let base = effective_thread_key(message);
    if should_branch_thread_by_subject(&base) {
        branch_compound_key(&base, &thread_grouping_subject(&message.subject))
    } else {
        base
    }
}

/// Split a `thread_key` request parameter into (root key, branch subject
/// filter). Card-minted keys for branchable threads always carry the
/// `#subject` suffix (see [`card_thread_key`]); uid:/gmthrid: keys never do.
pub fn split_thread_key(thread_key: &str) -> (String, Option<String>) {
    if should_branch_thread_by_subject(thread_key) {
        split_branch_compound_key(thread_key)
    } else {
        (thread_key.to_string(), None)
    }
}

pub fn should_branch_thread_by_subject(thread_key: &str) -> bool {
    !thread_key.starts_with("uid:") && !thread_key.starts_with("gmthrid:")
}

/// Join a root thread key and a grouping subject into one branch key. The root
/// is a raw Message-ID, where `#` is legal atext — escape it so
/// [`split_branch_compound_key`] can split at the first literal `#`
/// unambiguously (the subject side stays verbatim; it is only ever compared
/// whole against other grouping subjects).
pub fn branch_compound_key(root: &str, group_subject: &str) -> String {
    let escaped = root.replace('%', "%25").replace('#', "%23");
    format!("{escaped}#{group_subject}")
}

/// Split a branch key built by [`branch_compound_key`] back into
/// (root thread key, grouping subject). Keys without a `#` separator were
/// never escaped (unbranched legacy ids) and come back verbatim with no
/// subject.
pub fn split_branch_compound_key(compound: &str) -> (String, Option<String>) {
    match compound.split_once('#') {
        Some((root, subject)) => (
            root.replace("%23", "#").replace("%25", "%"),
            Some(subject.to_string()),
        ),
        None => (compound.to_string(), None),
    }
}

pub fn normalize_thread_subject(subject: &str) -> String {
    let mut subject = subject.trim();
    while let Some(rest) = strip_reply_prefix(subject) {
        subject = rest.trim();
    }
    subject.to_string()
}

pub fn thread_grouping_subject(subject: &str) -> String {
    let mut subject = subject.trim();
    loop {
        if let Some(rest) = strip_reply_prefix(subject) {
            subject = rest.trim();
            continue;
        }
        if let Some(rest) = strip_leading_bracket_tag(subject) {
            subject = rest.trim();
            continue;
        }
        break;
    }
    subject.to_string()
}

fn strip_reply_prefix(subject: &str) -> Option<&str> {
    let mut probe = subject.trim_start();
    while let Some(rest) = strip_leading_bracket_tag(probe) {
        probe = rest.trim_start();
    }

    const PREFIXES: &[&str] = &[
        "re", "fw", "fwd", "aw", "sv", "vs", "rv", "res", "tr", "antw", "wg", "答复", "回复",
        "转发",
    ];
    // Try every prefix that matches, not just the first: "fw" is a string
    // prefix of "Fwd:" but fails the colon check, and only the "fwd" entry
    // succeeds (mirrors the Go regex alternation, where the engine picks the
    // alternative that lets the trailing colon match).
    for prefix in PREFIXES {
        if !probe.is_char_boundary(prefix.len())
            || !probe[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            continue;
        }
        let mut rest = &probe[prefix.len()..];
        if let Some(after_count) = strip_reply_count(rest) {
            rest = after_count;
        }
        let mut chars = rest.chars();
        match chars.next() {
            Some(':') | Some('：') => return Some(chars.as_str()),
            _ => continue,
        }
    }
    None
}

fn strip_reply_count(rest: &str) -> Option<&str> {
    let bytes = rest.as_bytes();
    let close = match bytes.first()? {
        b'[' => b']',
        b'(' => b')',
        _ => return None,
    };
    let end = bytes.iter().position(|byte| *byte == close)?;
    if end <= 1 || !bytes[1..end].iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(&rest[end + 1..])
}

fn strip_leading_bracket_tag(subject: &str) -> Option<&str> {
    let trimmed = subject.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed.find(']')?;
    Some(&trimmed[end + 1..])
}

/// Message-IDs a thread references but hasn't cached locally. Across every
/// cached row of `account` sharing `thread_key`, collect the union of ids they
/// reference (each row's `References` chain) plus the root id (`thread_key`
/// itself), then subtract the ids already present as cached messages. The
/// remainder is the ancestry the thread links to but that lies outside the
/// synced window — the on-demand fetch target. All ids are lowercased here so
/// the set comparison is case-insensitive; stored ids preserve the original
/// header casing.
pub fn get_thread_reference_gaps(
    conn: &Connection,
    account: &str,
    thread_key: &str,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT json FROM messages
         WHERE account = ?1 AND uid <> 0
           AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?2",
    )?;
    let rows = stmt.query_map(params![account, thread_key], |row| {
        row.get::<_, Option<String>>(0)
    })?;
    let mut referenced: std::collections::BTreeSet<String> = Default::default();
    let mut present: std::collections::HashSet<String> = Default::default();
    let normalize = |id: &str| {
        id.trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim()
            .to_ascii_lowercase()
    };
    for row in rows {
        let json = row?.unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
        if let Some(mid) = parsed.get("message_id").and_then(|v| v.as_str()) {
            let mid = normalize(mid);
            if !mid.is_empty() {
                present.insert(mid);
            }
        }
        if let Some(refs) = parsed.get("references").and_then(|v| v.as_str()) {
            for id in refs.split_whitespace() {
                let id = normalize(id);
                if !id.is_empty() {
                    referenced.insert(id);
                }
            }
        }
    }
    // The non-synthetic `thread_key` is the thread's root Message-ID.
    if !thread_key.starts_with("uid:") {
        let root = normalize(thread_key);
        if !root.is_empty() {
            referenced.insert(root);
        }
    }
    Ok(referenced
        .into_iter()
        .filter(|id| !present.contains(id))
        .collect())
}

/// Like `get_thread_headers`, but spans any folder of `account` whose row
/// shares the `thread_key`. Used by the cross-folder thread view so a reply
/// stored in Sent appears alongside the inbox messages it threads with.
/// Each header carries its source `folder` so the reader can fetch the body
/// from the right mailbox. Ordered by `date` ASC (UIDs are folder-scoped, so
/// they aren't comparable across folders).
///
/// A self-addressed message is delivered to both Sent and the Inbox, so the
/// same RFC `Message-ID` lands as two rows under one `thread_key` (distinct
/// per-folder UIDs sidestep the `UNIQUE(account, folder, msg_id)` constraint).
/// We collapse those to a single bubble by keeping, per non-empty
/// `Message-ID`, only the lowest-`id` row. Rows without a `Message-ID` (e.g.
/// drafts) are never collapsed — a NULL `Message-ID` never equates in SQL.
pub fn get_thread_headers_all_folders(
    conn: &Connection,
    account: &str,
    thread_key: &str,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT m.uid, m.folder, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred, m.thread_key
         FROM messages m
         WHERE m.account = ?1 AND m.uid <> 0
           AND COALESCE(NULLIF(m.thread_key, ''), 'uid:' || m.uid) = ?2
           AND NOT EXISTS (
             SELECT 1 FROM messages dup
             WHERE dup.account = m.account
               AND COALESCE(NULLIF(dup.thread_key, ''), 'uid:' || dup.uid) = ?2
               AND COALESCE(json_extract(m.json, '$.message_id'), '') <> ''
               AND json_extract(dup.json, '$.message_id') = json_extract(m.json, '$.message_id')
               AND dup.id < m.id
           )
         ORDER BY m.date ASC, m.uid ASC",
    )?;
    let rows = stmt.query_map(params![account, thread_key], |row| {
        let uid: u32 = row.get(0)?;
        Ok(MessageHeader {
            uid,
            folder: row.get(1)?,
            subject: row.get(2)?,
            from_name: row.get(3)?,
            from_addr: row.get(4)?,
            date: row.get(5)?,
            seen: row.get::<_, i64>(6)? != 0,
            starred: row.get::<_, i64>(7)? != 0,
            thread_key: row
                .get::<_, Option<String>>(8)?
                .filter(|key| !key.is_empty())
                .unwrap_or_else(|| format!("uid:{uid}")),
            ..Default::default()
        })
    })?;
    // `date` is now an epoch integer, so the SQL ORDER BY sorts chronologically
    // (it could not when date was an RFC 2822 string). Unknown dates (0) sort first.
    let headers = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(headers)
}

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

pub fn get_folder_state(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Option<(u32, u32)>> {
    let row = conn
        .query_row(
            "SELECT uidvalidity, uid_next FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![account, folder],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .ok();
    Ok(row.map(|(v, n)| (v.unwrap_or(0) as u32, n.unwrap_or(0) as u32)))
}

pub fn set_folder_state(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uid_next: u32,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_state(account, folder, uidvalidity, uid_next) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(account, folder) DO UPDATE SET
           uidvalidity = excluded.uidvalidity, uid_next = excluded.uid_next",
        params![account, folder, uidvalidity as i64, uid_next as i64],
    )?;
    Ok(())
}

pub fn get_folder_modseq(conn: &Connection, account: &str, folder: &str) -> Result<u64> {
    let modseq = conn
        .query_row(
            "SELECT highest_modseq FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![account, folder],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0);
    Ok(modseq as u64)
}

pub fn set_folder_modseq(
    conn: &Connection,
    account: &str,
    folder: &str,
    modseq: u64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_state(account, folder, highest_modseq) VALUES(?1, ?2, ?3)
         ON CONFLICT(account, folder) DO UPDATE SET highest_modseq = excluded.highest_modseq",
        params![account, folder, modseq as i64],
    )?;
    Ok(())
}

pub fn update_message_seen(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    seen: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?4 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
        params![account, folder, uid, seen as i64],
    )?;
    Ok(())
}

pub fn update_message_starred(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    starred: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET starred = ?4 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
        params![account, folder, uid, starred as i64],
    )?;
    Ok(())
}

pub fn update_thread_seen(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    seen: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?4
         WHERE account = ?1 AND folder = ?2
           AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?3",
        params![account, folder, thread_key, seen as i64],
    )?;
    Ok(())
}

pub fn update_thread_starred(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    starred: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET starred = ?4
         WHERE account = ?1 AND folder = ?2
           AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?3",
        params![account, folder, thread_key, starred as i64],
    )?;
    Ok(())
}

pub fn delete_messages_by_uid(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<usize> {
    let mut deleted = 0usize;
    for uid in uids {
        deleted += conn.execute(
            "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, *uid],
        )?;
    }
    Ok(deleted)
}

#[allow(dead_code)]
pub fn move_messages_by_uid(
    conn: &Connection,
    account: &str,
    source_folder: &str,
    target_folder: &str,
    uids: &[u32],
) -> Result<usize> {
    if source_folder == target_folder || uids.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    let mut moved = 0usize;
    for uid in uids {
        let msg_id = tx
            .query_row(
                "SELECT msg_id FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![account, source_folder, *uid],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(msg_id) = msg_id else {
            continue;
        };
        tx.execute(
            "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
            params![account, target_folder, msg_id],
        )?;
        moved += tx.execute(
            "UPDATE messages SET folder = ?4 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, source_folder, *uid, target_folder],
        )?;
    }
    tx.commit()?;
    Ok(moved)
}

/// UIDs of every unseen message in a folder. Used by "mark all as read" to set
/// `\Seen` on the server for exactly the messages currently flagged unread.
pub fn get_unseen_uids(conn: &Connection, account: &str, folder: &str) -> Result<Vec<u32>> {
    let mut stmt = conn.prepare(
        "SELECT uid FROM messages WHERE account = ?1 AND folder = ?2 AND seen = 0 AND uid <> 0",
    )?;
    let rows = stmt.query_map(params![account, folder], |row| row.get::<_, u32>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Flip `seen` for every message in a folder (mark all read / unread).
pub fn mark_folder_seen(conn: &Connection, account: &str, folder: &str, seen: bool) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?3 WHERE account = ?1 AND folder = ?2",
        params![account, folder, seen as i64],
    )?;
    Ok(())
}

pub fn update_rss_thread_seen(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    seen: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?3 WHERE account = ?1 AND folder = ?2",
        params![account, subscription_id, seen as i64],
    )?;
    Ok(())
}

pub fn update_rss_thread_starred(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    starred: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET starred = ?3 WHERE account = ?1 AND folder = ?2",
        params![account, subscription_id, starred as i64],
    )?;
    Ok(())
}

pub fn update_rss_item_seen(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    item_key: &str,
    seen: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?4 WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
        params![account, subscription_id, item_key, seen as i64],
    )?;
    Ok(())
}

pub fn update_rss_item_starred(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    item_key: &str,
    starred: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET starred = ?4 WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
        params![account, subscription_id, item_key, starred as i64],
    )?;
    Ok(())
}

/// Delete locally cached messages whose UIDs are no longer on the server.
/// `server_uids` must be the complete UID set for the folder. Returns the
/// number of rows removed.
pub fn prune_missing_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    server_uids: &std::collections::HashSet<u32>,
) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT uid FROM messages WHERE account = ?1 AND folder = ?2")?;
    let local_uids: Vec<u32> = stmt
        .query_map(params![account, folder], |row| row.get::<_, u32>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut removed = 0usize;
    for uid in local_uids {
        if !server_uids.contains(&uid) {
            conn.execute(
                "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![account, folder, uid],
            )?;
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn clear_folder_messages(conn: &Connection, account: &str, folder: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM messages WHERE account = ?1 AND folder = ?2",
        params![account, folder],
    )?;
    Ok(())
}

pub fn has_cached_body(conn: &Connection, account: &str, folder: &str, uid: u32) -> Result<bool> {
    let found = conn
        .query_row(
            "SELECT 1 FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid = ?3
               AND (body IS NOT NULL
                    OR json_extract(json, '$.body_html') IS NOT NULL)",
            params![account, folder, uid],
            |_| Ok(()),
        )
        .ok()
        .is_some();
    Ok(found)
}

pub fn has_message(conn: &Connection, account: &str, folder: &str, uid: u32) -> Result<bool> {
    let found = conn
        .query_row(
            "SELECT 1 FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
            |_| Ok(()),
        )
        .ok()
        .is_some();
    Ok(found)
}

/// Render a stored recipient list (JSON `[{name, addr}]`) as a comma-separated
/// "Name <addr>" / "addr" string. Empty/missing/malformed input yields "".
fn format_recipient_list(json: Option<&str>) -> String {
    let Some(s) = json.filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) else {
        return String::new();
    };
    arr.iter()
        .filter_map(|v| {
            let addr = v["addr"].as_str().unwrap_or_default().trim();
            if addr.is_empty() {
                return None;
            }
            let name = v["name"].as_str().unwrap_or_default().trim();
            Some(if name.is_empty() {
                addr.to_string()
            } else {
                format!("{name} <{addr}>")
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_recipient_value(value: Option<&Value>) -> String {
    value
        .map(Value::to_string)
        .map(|json| format_recipient_list(Some(&json)))
        .unwrap_or_default()
}

pub fn get_cached_message(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
) -> Result<Option<Message>> {
    let mut stmt = conn.prepare(
        "SELECT subject, from_name, from_addr, date, body, json
         FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
    )?;

    let row = stmt
        .query_row(params![account, folder, uid], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .ok();

    let Some((subject, from_name, from_addr, date, body, json_extra)) = row else {
        return Ok(None);
    };
    let extra: Value = serde_json::from_str(&json_extra).unwrap_or_else(|_| json!({}));

    let to = format_recipient_value(extra.get("to"));

    let reply_to = extra["reply_to"].as_str().unwrap_or_default().to_string();
    let cc = format_recipient_value(extra.get("cc"));
    let bcc = extra["bcc"].as_str().unwrap_or_default().to_string();
    let message_id = extra["message_id"].as_str().unwrap_or_default().to_string();
    let references = extra["references"].as_str().unwrap_or_default().to_string();
    let body_html = extra["body_html"].as_str().map(str::to_string);

    // `body` is the canonical plain-text body. For legacy HTML-only rows where it
    // is missing, fall back to rendering the stored HTML source once.
    let body = match body {
        Some(body) => body,
        None => match &body_html {
            Some(html) => {
                let rendered = crate::parse::render_body(html);
                let _ = conn.execute(
                    "UPDATE messages SET body = ?4 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                    params![account, folder, uid, rendered],
                );
                rendered
            }
            None => return Ok(None),
        },
    };

    let attachments: Vec<Attachment> = extra
        .get("attachments")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();

    // Keep the HTML source on the returned message so the read handler can build
    // the iframe-ready view (it injects the remote-image CSP per account setting).
    Ok(Some(Message {
        subject,
        from_name,
        from_addr,
        to,
        reply_to,
        cc,
        bcc,
        message_id,
        references,
        date,
        body,
        body_html,
        body_is_rendered: extra["body_is_rendered"].as_bool().unwrap_or(false),
        preview: String::new(),
        attachments,
    }))
}

pub fn save_cached_message(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    message: &Message,
) -> Result<()> {
    let attachments_json = serde_json::to_value(&message.attachments).unwrap_or_else(|_| json!([]));
    // The `json` catch-all column carries header fields that don't have a typed
    // column. Envelope recipients are written by `upsert_messages`; body fetches
    // must not overwrite them.
    let extra_json = json!({
        "reply_to": message.reply_to,
        "bcc": message.bcc,
        "message_id": message.message_id,
        "references": message.references,
        "body_html": message.body_html,
        "body_is_rendered": message.body_is_rendered,
        "attachments": attachments_json,
    })
    .to_string();

    conn.execute(
        "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
         VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(account, folder, msg_id) DO UPDATE SET
           subject = excluded.subject,
           from_name = excluded.from_name,
           from_addr = excluded.from_addr,
           date = excluded.date,
           body = excluded.body,
           json = json_patch(messages.json, excluded.json)",
        params![
            account,
            folder,
            uid,
            message.subject,
            message.from_name,
            message.from_addr,
            message.date,
            message.body,
            extra_json
        ],
    )?;

    Ok(())
}

// ---- RSS items (shared messages table) --------------------------------------

/// Per-item RSS fields that don't map to a typed mail column; stored as the
/// message row's `json` JSON.
pub struct RssItemExtra {
    pub author: String,
    pub link: String,
    pub summary: String,
    pub content: String,
    /// Inline images lifted from the item's HTML. Each carries its source `url`
    /// and, once downloaded to the media dir, a local `key` served at `/media`.
    pub images: Vec<RssMedia>,
    /// Inline videos lifted from the item's HTML. Remote-only (rendered straight
    /// from their source `url`); not cached to disk, so `key` stays `None`.
    pub videos: Vec<RssMedia>,
    pub published_at: i64,
    pub updated_at: i64,
    pub fetched_at: i64,
}

/// One inline feed media item (image or video): its remote source and, when
/// cached locally, its media key served at `/media`.
pub struct RssMedia {
    pub url: String,
    pub key: Option<String>,
}

/// Upsert one RSS item as a message row. `folder` is the subscription id and
/// `msg_id` the stable item key; the feed-specific payload lives in `json`.
/// Updates preserve the existing `seen` flag (don't un-read on refetch).
pub fn upsert_rss_item(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    item_key: &str,
    title: &str,
    unread: bool,
    body_html: Option<&str>,
    extra: &RssItemExtra,
) -> Result<bool> {
    // Tell new arrivals apart from re-syncs of items we already stored, so callers
    // can surface a "new items" notification only for genuinely new entries.
    let is_new = conn
        .query_row(
            "SELECT 1 FROM messages WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
            params![account, subscription_id, item_key],
            |_| Ok(()),
        )
        .optional()?
        .is_none();
    let extra_json = json!({
        "author": extra.author,
        "link": extra.link,
        "summary": extra.summary,
        "content": extra.content,
        "body_html": body_html,
        "images": extra.images.iter()
            .map(|img| json!({ "url": img.url, "key": img.key }))
            .collect::<Vec<_>>(),
        "videos": extra.videos.iter()
            .map(|vid| json!({ "url": vid.url, "key": vid.key }))
            .collect::<Vec<_>>(),
        "published_at": extra.published_at,
        "updated_at": extra.updated_at,
        "fetched_at": extra.fetched_at,
    })
    .to_string();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key, json)
         VALUES(?1, ?2, ?3, 0, ?4, ?5, '', ?6, ?7, ?2, ?8)
         ON CONFLICT(account, folder, msg_id) DO UPDATE SET
           subject = excluded.subject,
           date    = excluded.date,
           json   = excluded.json",
        params![
            account,
            subscription_id,
            item_key,
            title,
            extra.author,
            item_date_epoch(extra.published_at, extra.updated_at, extra.fetched_at),
            (!unread) as i64,
            extra_json,
        ],
    )?;
    Ok(is_new)
}

/// Best available timestamp for an RSS item as epoch seconds (0 when none),
/// preferring published > updated > fetched. Stored in the `date` column so RSS
/// rows sort alongside mail.
fn item_date_epoch(published: i64, updated: i64, fetched: i64) -> i64 {
    if published != 0 {
        published
    } else if updated != 0 {
        updated
    } else {
        fetched
    }
}

#[cfg(test)]
mod tests;
