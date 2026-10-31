//! Local message search and saved search snapshot pages.

use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::imap::MessageHeader;
use crate::search_query::{SearchQuery, local_midnight};

use super::*;

/// Flatten `To`/`Cc` into the plain text `messages.recipients` indexes. The
/// recipient lists themselves live in the `json` catch-all, which FTS can't
/// reach, so this mirror is what makes "find the mail I sent to Ann" work
/// against the cache. `None` for a message with no addressees, which the write
/// path treats as "leave whatever is already indexed alone".
pub(super) fn recipients_index_text(
    to: &[crate::imap::Recipient],
    cc: &[crate::imap::Recipient],
) -> Option<String> {
    let text = to
        .iter()
        .chain(cc.iter())
        .map(|recipient| {
            format!("{} {}", recipient.name, recipient.addr)
                .trim()
                .to_string()
        })
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!text.is_empty()).then_some(text)
}

/// Search one folder's cached messages, newest first. `query` may carry the
/// operators [`SearchQuery`] understands; its free text is a substring match
/// over subject, sender, recipients and body.
///
/// Each text condition of 3+ codepoints is answered by the trigram FTS indexes;
/// shorter ones (common for CJK, where words are often 2 characters) are matched
/// in Rust while rows stream newest first, stopping at `limit` hits. The Rust
/// match is used rather than SQL `lower(..) LIKE ..` because SQLite's `lower()`
/// folds ASCII only, so a two-letter Cyrillic or Greek query would miss
/// capitalised text that the (Unicode-folding) trigram path finds once the query
/// grows a third letter.
///
/// `before_cursor` is the `(date, uid, folder)` of the last row of the previous
/// page. The folder tie-breaker is required because UIDs are mailbox-scoped.
pub fn search_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    query: &str,
    limit: u32,
    before_cursor: Option<&crate::thread_list::SearchCursor>,
) -> Result<Vec<MessageHeader>> {
    let query = SearchQuery::parse(query);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut filters = String::new();
    let mut args: Vec<Value> = vec![
        account.to_string().into(),
        folder.to_string().into(),
        before_cursor.map(|cursor| cursor.date).into(),
        before_cursor.map_or(0, |cursor| cursor.uid as i64).into(),
        before_cursor.map(|cursor| cursor.folder.clone()).into(),
    ];
    // Short text conditions, each its needles (any may match) and the result
    // columns they may hit.
    let mut scans: Vec<(Vec<String>, &[usize])> = Vec::new();
    // `index` is the FTS subquery answering one text condition, its `?` the
    // phrases; `columns` the same columns for a short needle. A condition
    // with any short needle is scanned whole, as the index can't find it.
    let mut text_condition =
        |needles: &[String], index: &str, scope: &str, columns: &'static [usize]| {
            if needles.iter().any(|needle| needle.chars().count() < 3) {
                scans.push((
                    needles.iter().map(|needle| needle.to_lowercase()).collect(),
                    columns,
                ));
            } else {
                filters.push_str(&format!(" AND m.id IN ({index})"));
                // Each quoted as one phrase, doubling any `"` so user input can't
                // change the FTS query: a substring match, like the scan.
                let phrases = needles
                    .iter()
                    .map(|needle| format!("\"{}\"", needle.replace('"', "\"\"")))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let expression = format!("{scope}({phrases})");
                let count = index.matches('?').count();
                args.extend(std::iter::repeat_n(Value::from(expression), count));
            }
        };
    const MESSAGES_FTS: &str = "SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?";
    const RECIPIENTS_FTS: &str =
        "SELECT rowid FROM messages_recipients_fts WHERE messages_recipients_fts MATCH ?";
    for text in &query.text {
        // Body last: it is by far the largest column to fold.
        text_condition(
            std::slice::from_ref(text),
            &format!("{MESSAGES_FTS} UNION {RECIPIENTS_FTS}"),
            "",
            &[1, 2, 3, 10, 11],
        );
    }
    for from in &query.from {
        text_condition(from, MESSAGES_FTS, "{from_name from_addr} : ", &[2, 3]);
    }
    for subject in &query.subject {
        text_condition(subject, MESSAGES_FTS, "subject : ", &[1]);
    }
    for to in &query.to {
        text_condition(to, RECIPIENTS_FTS, "", &[10]);
    }
    if query.read {
        filters.push_str(" AND m.seen <> 0");
    }
    if query.unread {
        filters.push_str(" AND m.seen = 0");
    }
    if query.starred {
        filters.push_str(" AND m.starred <> 0");
    }
    if query.has_attachment {
        filters.push_str(" AND m.files IS NOT NULL");
    }
    if let Some(after) = query.after {
        filters.push_str(" AND m.date >= ?");
        args.push(local_midnight(after).into());
    }
    if let Some(before) = query.before {
        filters.push_str(" AND m.date < ?");
        args.push(local_midnight(before).into());
    }
    // The scanned columns are only read when a short needle needs them; a
    // NULL keeps the body out of the sort otherwise.
    let scanned = if scans.is_empty() {
        "NULL, NULL"
    } else {
        "m.recipients, m.body"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT m.uid, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred,
                m.thread_key, json_extract(m.json, '$.to'),
                COALESCE(json_extract(m.json, '$.message_id'), ''),
                {scanned}
         FROM messages m
         WHERE m.account = ?1 AND m.folder = ?2 AND m.uid <> 0
           AND (?3 IS NULL
                OR m.date < ?3
                OR (m.date = ?3 AND m.uid < ?4)
                OR (m.date = ?3 AND m.uid = ?4 AND m.folder < ?5)){filters}
         ORDER BY m.date DESC, m.uid DESC"
    ))?;
    let mut rows = stmt.query(params_from_iter(args))?;
    let mut messages = Vec::new();
    while messages.len() < limit as usize {
        let Some(row) = rows.next()? else {
            break;
        };
        let mut hit = true;
        for (needles, columns) in &scans {
            let mut found = false;
            for &column in *columns {
                if let Some(text) = row.get_ref(column)?.as_str_or_null()?
                    && let text = text.to_lowercase()
                    && needles.iter().any(|needle| text.contains(needle.as_str()))
                {
                    found = true;
                    break;
                }
            }
            if !found {
                hit = false;
                break;
            }
        }
        if hit {
            messages.push(search_header_from_row(row)?);
        }
    }
    Ok(messages)
}

/// Search several folders (typically the open mailbox plus Sent) as one
/// newest-first result set. UIDs are folder-scoped, so each header carries the
/// folder it came from and ordering is by date — the only key comparable across
/// mailboxes. Used both for the cached half of a live search and, on mobile, as
/// the whole answer when the server can't be reached.
pub fn search_messages_in_folders(
    conn: &Connection,
    account: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    before_cursor: Option<&crate::thread_list::SearchCursor>,
) -> Result<Vec<MessageHeader>> {
    let mut messages = Vec::new();
    for folder in folders {
        for mut message in search_messages(conn, account, folder, query, limit, before_cursor)? {
            message.folder = folder.clone();
            messages.push(message);
        }
    }
    sort_search_hits(&mut messages, limit);
    Ok(messages)
}

/// Newest first by epoch send time, capped at `limit`; unknown dates (0) sort
/// last. Shared by every path that merges search hits from more than one source
/// so cached-only and cached+live results are ordered identically.
pub fn sort_search_hits(messages: &mut Vec<MessageHeader>, limit: u32) {
    sort_search_hits_all(messages);
    messages.truncate(limit as usize);
}

pub fn sort_search_hits_all(messages: &mut [MessageHeader]) {
    messages.sort_unstable_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then_with(|| b.uid.cmp(&a.uid))
            .then_with(|| b.folder.cmp(&a.folder))
    });
}

/// The search cursor for the page after `messages`, or `None` when
/// this page was short (a short page means the result set is exhausted).
pub fn search_next_cursor(messages: &[MessageHeader], limit: u32, scanned: u32) -> Option<String> {
    if messages.len() < limit as usize {
        return None;
    }
    messages.last().map(|header| {
        crate::thread_list::format_search_cursor(&crate::thread_list::SearchCursor {
            date: header.date,
            uid: header.uid,
            folder: header.folder.clone(),
            scanned,
            snapshot: None,
            offset: 0,
        })
    })
}

pub struct SearchSnapshotPage {
    pub messages: Vec<MessageHeader>,
    pub next_offset: u32,
    pub has_more: bool,
    /// Server hits still wait to be fetched.
    pub pending: bool,
    /// Set when the last batch cut off its cached-only hits: the keyset cursor
    /// of the last one it took, where paging past the end resumes the cache.
    /// Distinct from the snapshot's last row, which can be a server hit older
    /// than cached hits the cut skipped.
    pub cache_resume: Option<String>,
}

/// A search hit's display-order key, `(date, uid, folder)`, as
/// [`sort_search_hits_all`] ranks it (newest first means largest first).
pub type SearchHitKey<'a> = (i64, u32, &'a str);

/// One server hit a snapshot has not fetched yet, with the date its Date
/// header was resolved to when the snapshot was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSearchHit {
    pub folder: String,
    pub uid: u32,
    pub date: i64,
}

impl PendingSearchHit {
    pub fn key(&self) -> SearchHitKey<'_> {
        (self.date, self.uid, &self.folder)
    }
}

/// Start a live-search snapshot with every server hit still to fetch.
/// `pending` carries each server hit with its resolved date; batches of it are
/// taken newest first by [`take_search_pending`] and recorded by
/// [`finish_search_batch`] as paging needs them.
pub fn create_search_snapshot(
    conn: &Connection,
    account: &str,
    query: &str,
    folders: &[String],
    pending: &[PendingSearchHit],
) -> Result<String> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let scope = serde_json::to_string(folders)?;
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO mail_search_snapshots(token, account, query, scope, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![token, account, query, scope, created_at],
    )?;
    {
        let mut insert = tx.prepare(
            "INSERT OR IGNORE INTO mail_search_pending(token, account, folder, uid, date)
             VALUES(?1, ?2, ?3, ?4, ?5)",
        )?;
        for hit in pending {
            insert.execute(params![token, account, hit.folder, hit.uid, hit.date])?;
        }
    }
    // Search snapshots are disposable cache state. A one-day lease is long
    // enough for suspended mobile views to resume without letting abandoned
    // queries grow the database indefinitely.
    let expired = created_at.saturating_sub(86_400);
    tx.execute(
        "DELETE FROM mail_search_hits WHERE account = ?1 AND created_at < ?2",
        params![account, expired],
    )?;
    tx.execute(
        "DELETE FROM mail_search_pending
         WHERE token IN (SELECT token FROM mail_search_snapshots
                         WHERE account = ?1 AND created_at < ?2)",
        params![account, expired],
    )?;
    tx.execute(
        "DELETE FROM mail_search_snapshots WHERE account = ?1 AND created_at < ?2",
        params![account, expired],
    )?;
    tx.commit()?;
    Ok(token)
}

/// The cached dates of whichever of `uids` the cache holds in `folder`, so a
/// new snapshot only asks the server for the dates it lacks.
pub fn cached_message_dates(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<std::collections::HashMap<u32, i64>> {
    let mut stmt = conn.prepare_cached(
        "SELECT date FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
    )?;
    let mut dates = std::collections::HashMap::new();
    for &uid in uids {
        if let Some(date) = stmt
            .query_row(params![account, folder, uid], |row| row.get::<_, i64>(0))
            .optional()?
        {
            dates.insert(uid, date);
        }
    }
    Ok(dates)
}

/// The cached headers of whichever of `uids` the cache holds in `folder`, so a
/// search batch only fetches the hits it lacks. `files` is always set: rows
/// whose attachments were never read are left out, for the fetch to fill in.
pub fn cached_search_headers(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare_cached(
        "SELECT m.uid, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred,
                m.thread_key, json_extract(m.json, '$.to'),
                COALESCE(json_extract(m.json, '$.message_id'), ''),
                CAST(json_extract(m.json, '$.gmail_msg_id') AS TEXT),
                m.files
         FROM messages m
         WHERE m.account = ?1 AND m.folder = ?2 AND m.uid = ?3
           AND (m.body IS NOT NULL
                OR COALESCE(json_extract(m.json, '$.attachment_metadata'), 0) <> 0)",
    )?;
    let mut headers = Vec::new();
    for &uid in uids {
        let header = stmt
            .query_row(params![account, folder, uid], |row| {
                let mut header = search_header_from_row(row)?;
                header.gmail_msg_id = row
                    .get::<_, Option<String>>(10)?
                    .and_then(|id| id.parse().ok());
                header.files = Some(
                    row.get::<_, Option<String>>(11)?
                        .and_then(|files| serde_json::from_str(&files).ok())
                        .unwrap_or_default(),
                );
                header.folder = folder.to_string();
                Ok(header)
            })
            .optional()?;
        headers.extend(header);
    }
    Ok(headers)
}

/// The snapshot's `count` newest unfetched server hits by date, across all its
/// folders, plus the newest one after them: nothing older than that one can be
/// placed until it is fetched. Nothing is consumed until
/// [`finish_search_batch`] records the fetch.
pub fn take_search_pending(
    conn: &Connection,
    token: &str,
    count: usize,
) -> Result<(Vec<PendingSearchHit>, Option<PendingSearchHit>)> {
    let mut stmt = conn.prepare(
        "SELECT folder, uid, date FROM mail_search_pending WHERE token = ?1
         ORDER BY date DESC, uid DESC, folder DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![token, count.saturating_add(1) as i64], |row| {
        Ok(PendingSearchHit {
            folder: row.get(0)?,
            uid: row.get(1)?,
            date: row.get(2)?,
        })
    })?;
    let mut hits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let next = (hits.len() > count).then(|| hits.pop()).flatten();
    Ok((hits, next))
}

/// Record one fetched batch: drop the `done` hits from the pending set, and
/// append `messages` (already in display order) after the snapshot's current
/// end, skipping any the snapshot already lists.
pub fn finish_search_batch(
    conn: &Connection,
    token: &str,
    account: &str,
    done: &[(String, u32)],
    messages: &[MessageHeader],
    cache_resume: Option<&str>,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let Some((query, scope, created_at)) = tx
        .query_row(
            "SELECT query, scope, created_at FROM mail_search_snapshots
             WHERE token = ?1 AND account = ?2",
            params![token, account],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?
    else {
        // Expired or dropped while the batch was in flight.
        return Ok(());
    };
    {
        let mut delete = tx.prepare(
            "DELETE FROM mail_search_pending WHERE token = ?1 AND folder = ?2 AND uid = ?3",
        )?;
        for (folder, uid) in done {
            delete.execute(params![token, folder, uid])?;
        }
    }
    tx.execute(
        "UPDATE mail_search_snapshots SET cache_resume = ?2 WHERE token = ?1",
        params![token, cache_resume],
    )?;
    let mut position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM mail_search_hits WHERE token = ?1",
        params![token],
        |row| row.get(0),
    )?;
    for message in messages {
        let listed = tx
            .query_row(
                "SELECT 1 FROM mail_search_hits WHERE token = ?1 AND folder = ?2 AND uid = ?3",
                params![token, message.folder, message.uid],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if listed {
            continue;
        }
        tx.execute(
            "INSERT INTO mail_search_hits(
               token, account, query, scope, position, folder, uid, created_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                token,
                account,
                query,
                scope,
                position,
                message.folder,
                message.uid,
                created_at
            ],
        )?;
        position += 1;
    }
    tx.commit()?;
    Ok(())
}

/// Every (folder, uid) a snapshot already lists, so a batch can skip cached
/// hits an earlier batch appended.
pub fn search_snapshot_members(
    conn: &Connection,
    token: &str,
) -> Result<std::collections::HashSet<(String, u32)>> {
    let mut stmt = conn.prepare("SELECT folder, uid FROM mail_search_hits WHERE token = ?1")?;
    let rows = stmt.query_map(params![token], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Read a stable live-search page. `None` means the cursor is stale or belongs
/// to a different query/scope, in which case callers can resume keyset paging
/// through ordinary cached search results.
pub fn get_search_snapshot_page(
    conn: &Connection,
    account: &str,
    query: &str,
    folders: &[String],
    token: &str,
    offset: u32,
    limit: u32,
) -> Result<Option<SearchSnapshotPage>> {
    let scope = serde_json::to_string(folders)?;
    let snapshot = conn
        .query_row(
            "SELECT cache_resume,
                    EXISTS(SELECT 1 FROM mail_search_pending WHERE token = ?1)
             FROM mail_search_snapshots
             WHERE token = ?1 AND account = ?2 AND query = ?3 AND scope = ?4",
            params![token, account, query, scope],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()?;
    let Some((cache_resume, pending)) = snapshot else {
        return Ok(None);
    };

    let fetch_limit = limit.saturating_add(1);
    let mut stmt = conn.prepare(
        "SELECT m.uid, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred,
                m.thread_key, json_extract(m.json, '$.to'), h.folder, h.position,
                COALESCE(json_extract(m.json, '$.message_id'), '')
         FROM mail_search_hits h
         JOIN messages m
           ON m.account = h.account AND m.folder = h.folder AND m.uid = h.uid
         WHERE h.token = ?1 AND h.account = ?2
           AND h.position >= ?3
         ORDER BY h.position
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![token, account, offset, fetch_limit], |row| {
        let mut message = message_header_from_row(row)?;
        message.folder = row.get(9)?;
        message.message_id = row.get(11)?;
        Ok((message, row.get::<_, u32>(10)?))
    })?;
    let mut rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_offset = rows
        .last()
        .map(|(_, position)| position.saturating_add(1))
        .unwrap_or(offset);
    Ok(Some(SearchSnapshotPage {
        messages: rows.into_iter().map(|(message, _)| message).collect(),
        next_offset,
        has_more,
        pending,
        cache_resume,
    }))
}
