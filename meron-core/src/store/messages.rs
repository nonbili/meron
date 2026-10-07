//! Mail messages: upserts, recent pages, starred lists, and cached bodies.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::HashSet;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::imap::MessageHeader;
use crate::parse::{Attachment, Message};

use super::*;

pub fn upsert_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    messages: &[MessageHeader],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let removed = removed_uids_in(&tx, account, folder)?;
    // The Message-IDs this batch cached: the only ids whose arrival can hand a
    // canonical thread key down to rows already in the cache.
    let mut upserted_ids: HashSet<String> = HashSet::new();
    for m in messages {
        // A move or delete already dropped this UID. A sync that fetched it
        // beforehand must not put the row back into the folder it left.
        if removed.contains(&m.uid) {
            continue;
        }
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
        if m.files.is_some() {
            extra.insert("attachment_metadata".to_string(), json!(true));
        }
        let extra_json = Value::Object(extra).to_string();
        let thread_key = resolve_message_thread_key(&tx, account, &m.thread_key)?;
        let message_id = m.message_id.trim().to_lowercase();
        if !message_id.is_empty() {
            upserted_ids.insert(message_id);
        }
        tx.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key, json, recipients, files)
             VALUES(?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(account, folder, msg_id) DO UPDATE SET
               subject    = excluded.subject,
               from_name  = excluded.from_name,
               from_addr  = excluded.from_addr,
               date       = excluded.date,
               seen       = excluded.seen,
               starred    = excluded.starred,
               thread_key = excluded.thread_key,
               json       = json_patch(messages.json, excluded.json),
               -- Same rule as the recipient lists in `json`: a flag-only resync
               -- carries no envelope, so it must not blank what we already indexed.
               recipients = COALESCE(excluded.recipients, messages.recipients),
               -- A fetched body is authoritative, especially for inline images.
               files = CASE WHEN messages.body IS NOT NULL OR ?14 = 0
                            THEN messages.files ELSE excluded.files END",
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
                extra_json,
                recipients_index_text(&m.to, &m.cc),
                m.files.as_ref().filter(|files| !files.is_empty()).map(|files| json!(files).to_string()),
                m.files.is_some() as i64
            ],
        )?;
    }
    reconcile_thread_keys_from(&tx, account, upserted_ids)?;
    tx.commit()?;
    Ok(())
}

pub(super) fn resolve_message_thread_key(
    conn: &rusqlite::Transaction<'_>,
    account: &str,
    thread_key: &str,
) -> Result<String> {
    let key = thread_key.trim();
    if key.is_empty() || key.starts_with("uid:") || key.starts_with("gmthrid:") {
        return Ok(thread_key.to_string());
    }

    // References chooses a Message-ID as the raw thread key. That message may
    // itself already have inherited an older canonical root. Following the
    // key through its cached Message-ID keeps later replies in that same root,
    // even when their immediate In-Reply-To names a different message. Proton
    // Bridge exposes exactly this shape after a reply round trip.
    Ok(cached_thread_key_of(conn, account, &key.to_lowercase())?
        .unwrap_or_else(|| thread_key.to_string()))
}

/// The thread key of the cached message with this (lowercased) Message-ID, or
/// `None` when we haven't cached it. Duplicate copies of one message — the same
/// id in several folders — resolve to the oldest row, so the answer doesn't
/// depend on which folder synced last.
pub(super) fn cached_thread_key_of(
    conn: &rusqlite::Transaction<'_>,
    account: &str,
    message_id: &str,
) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT thread_key FROM messages
             WHERE account = ?1
               AND lower(COALESCE(json_extract(json, '$.message_id'), '')) = ?2
               AND COALESCE(thread_key, '') <> ''
             ORDER BY date ASC, uid ASC
             LIMIT 1",
            params![account, message_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

/// Hand the canonical thread key down to rows that named a message we only just
/// cached.
///
/// A fresh folder sync may upsert a reply before the referenced row whose
/// Message-ID reveals the root key, so `resolve_message_thread_key` alone can't
/// settle those. Walking out from the ids this batch wrote — rather than
/// re-scanning the account cache — keeps the work proportional to the batch:
/// each newly cached id fixes the rows that name it, and each row so fixed
/// becomes the next id to walk out from, since its own children inherited
/// through it. `seen` stops the walk from revisiting an id, so a reference cycle
/// (two messages naming each other) terminates instead of flip-flopping.
pub(super) fn reconcile_thread_keys_from(
    conn: &rusqlite::Transaction<'_>,
    account: &str,
    seeds: HashSet<String>,
) -> Result<()> {
    let mut seen = seeds;
    let mut pending: Vec<String> = seen.iter().cloned().collect();

    while !pending.is_empty() {
        let mut next: Vec<String> = Vec::new();
        for parent_id in pending.drain(..) {
            let Some(canonical) = cached_thread_key_of(conn, account, &parent_id)? else {
                continue;
            };
            let mut stmt = conn.prepare_cached(
                "SELECT id, lower(COALESCE(json_extract(json, '$.message_id'), ''))
                   FROM messages
                  WHERE account = ?1
                    AND uid <> 0
                    AND lower(COALESCE(thread_key, '')) = ?2
                    AND thread_key <> ?3
                    AND thread_key NOT LIKE 'uid:%'
                    AND thread_key NOT LIKE 'gmthrid:%'",
            )?;
            let children = stmt
                .query_map(params![account, parent_id, canonical], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (id, child_message_id) in children {
                conn.execute(
                    "UPDATE messages SET thread_key = ?1 WHERE id = ?2",
                    params![canonical, id],
                )?;
                if !child_message_id.is_empty() && seen.insert(child_message_id.clone()) {
                    next.push(child_message_id);
                }
            }
        }
        pending = next;
    }
    Ok(())
}

/// Older header-only rows from before BODYSTRUCTURE was indexed. Bound each
/// sync's catch-up work; later syncs continue after the rows marked complete.
pub fn missing_attachment_metadata_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    limit: u32,
) -> Result<Vec<u32>> {
    let mut stmt = conn.prepare(
        "SELECT uid FROM messages WHERE account = ?1 AND folder = ?2 AND body IS NULL
         AND COALESCE(json_extract(json, '$.attachment_metadata'), 0) = 0
         AND COALESCE(json_extract(json, '$.attachment_metadata_attempted'), 0) = 0
         ORDER BY date DESC, uid DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![account, folder, limit], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Mark bounded catch-up work even when a server omitted BODYSTRUCTURE or a
/// malformed response was isolated. Fresh header sync can still supply metadata.
pub fn mark_attachment_metadata_attempted(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for uid in uids {
        tx.execute(
            "UPDATE messages SET json = json_set(json, '$.attachment_metadata_attempted', 1)
                    WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Which cached rows a newest-first page keeps. The two narrowings combine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecentFilter {
    pub unread_only: bool,
    /// Messages whose *thread* has a cached message with a real attachment —
    /// thread-wide, like the card's paperclip, so an unread plain reply to a
    /// read message that carried the file still lists its thread under
    /// "unread with attachments". A subject-branched thread shares one key, so
    /// this is a superset; the card-level narrowing
    /// (`ThreadListQuery::retain_attachment_threads`) trims it to the branch.
    /// Header sync supplies BODYSTRUCTURE metadata even before a body is cached.
    pub attachments_only: bool,
}

pub fn get_recent_page(
    conn: &Connection,
    account: &str,
    folder: &str,
    limit: u32,
    before_cursor: Option<(i64, u32)>,
    filter: RecentFilter,
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
           AND (?7 = 0 OR files IS NOT NULL
                OR (COALESCE(thread_key, '') <> '' AND EXISTS (
                      SELECT 1 FROM messages t
                      WHERE t.account = messages.account
                        AND lower(COALESCE(t.thread_key, '')) = lower(COALESCE(messages.thread_key, ''))
                        AND t.files IS NOT NULL)))
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
            filter.unread_only as i64,
            filter.attachments_only as i64
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

#[cfg(test)]
pub(super) fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(super) fn mail_identity_from_parts(
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

pub(super) fn message_identity(header: &MessageHeader, folder: &str) -> String {
    mail_identity_from_parts(folder, header.uid, header.gmail_msg_id, &header.message_id)
}

#[cfg(test)]
pub(super) fn gmail_msg_id_from_json(value: Option<String>) -> Option<u64> {
    value.and_then(|value| value.parse::<u64>().ok())
}

/// Record message identities and return the subset that had not been observed
/// before this call.
#[cfg(test)]
pub(super) fn record_observed_mail_identities(
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

/// [`classify_arrivals`] for INBOX.
pub fn classify_inbox_arrivals(
    conn: &Connection,
    account: &str,
    before: u32,
    after: u32,
    messages: &[MessageHeader],
) -> Result<Vec<MessageHeader>> {
    classify_arrivals(conn, account, "INBOX", before, after, messages)
}

/// Unread messages in `folder` in the UID range that appeared during the last
/// sync, newest first. Returns the whole batch rather than just its latest
/// message so notifications can post one entry per arrival; empty when nothing
/// new and unread landed. A message already observed anywhere in the account
/// (one the user filed here from INBOX, say) is not an arrival.
pub fn classify_arrivals(
    conn: &Connection,
    account: &str,
    folder: &str,
    before: u32,
    after: u32,
    messages: &[MessageHeader],
) -> Result<Vec<MessageHeader>> {
    let mut arrivals = Vec::new();
    if before == 0 || after <= before {
        return Ok(arrivals);
    }
    let mut stmt = conn
        .prepare("SELECT 1 FROM observed_mail_identities WHERE account = ?1 AND identity = ?2")?;
    let mut identities = HashSet::new();
    for message in messages {
        if message.seen || message.uid < before || message.uid >= after {
            continue;
        }
        let identity = message_identity(message, folder);
        if stmt
            .query_row(params![account, &identity], |_| Ok(()))
            .optional()?
            .is_none()
            && identities.insert(identity)
        {
            arrivals.push(message.clone());
        }
    }
    arrivals.sort_by_key(|message| std::cmp::Reverse(message.uid));
    Ok(arrivals)
}

#[cfg(test)]
pub fn new_unread_inbox_messages(
    conn: &Connection,
    account: &str,
    uid_next_before: u32,
    uid_next_after: u32,
    synced_messages: &[MessageHeader],
) -> Result<Option<Vec<MessageHeader>>> {
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
    if headers.is_empty() {
        return Ok(None);
    }
    Ok(Some(headers))
}

/// One-line body snippet for a cached message, or `None` when the body hasn't
/// been fetched yet. Notifications use it to show the mail itself rather than
/// the subject alone; a miss degrades to a subject-only notification.
pub fn cached_body_preview(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
) -> Option<String> {
    let message = get_cached_message(conn, account, folder, uid).ok()??;
    let preview = crate::parse::preview_of(&message.body);
    (!preview.trim().is_empty()).then_some(preview)
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

/// What makes two cached rows the same starred conversation.
///
/// A Message-ID or `gmthrid:` thread key names the conversation itself and
/// holds wherever the server files it, so those dedupe across folders. `uid:`
/// is the reserved key [`crate::store::card_thread_key`] mints for a message
/// that arrived with no threading headers at all, and an IMAP UID means nothing
/// outside its own mailbox — `uid:1` in Inbox and `uid:1` in Archive are
/// ordinarily two unrelated messages, so those stay scoped to their folder.
///
/// Both the query budget below and the card-level dedupe in
/// `mail_model::starred_thread_cards` key on this, so a row that survives the
/// budget as its own conversation cannot then be swallowed as a copy of
/// another, or vice versa.
pub fn starred_thread_identity(folder: &str, thread_key: &str) -> String {
    if thread_key.starts_with("uid:") {
        format!("{folder}\u{0}{thread_key}")
    } else {
        thread_key.to_string()
    }
}

/// Every starred mail message across all accounts and folders, newest first,
/// for the newest `max_threads` distinct conversations.
///
/// The budget counts conversations rather than rows because the caller renders
/// one card per conversation: an account whose server files each message under
/// several folders (Gmail's All Mail beside the Inbox copy, plus every label)
/// would otherwise spend the whole budget on copies of the same few threads and
/// silently drop older ones off the end of the list.
///
/// A conversation past the budget is skipped rather than ending the scan, since
/// its rows carry no signal about where the remaining copies of the admitted
/// ones are: two folders' copies of one message can hold different cached dates,
/// which puts them arbitrarily far apart in this ordering. Stopping at the first
/// over-budget row would drop those stragglers, and the caller picks which
/// folder's copy to show from exactly this set — losing the Inbox copy would put
/// the thread under All Mail instead.
///
/// RSS rows carry `uid = 0` and are excluded; `rss::starred_items` covers them.
pub fn get_starred_all_accounts(
    conn: &Connection,
    max_threads: u32,
) -> Result<Vec<(String, MessageHeader)>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to'), account, folder FROM messages
         WHERE starred <> 0 AND uid <> 0
         ORDER BY date DESC, uid DESC",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    let mut threads: HashSet<(String, String)> = HashSet::new();
    while let Some(row) = rows.next()? {
        let mut header = message_header_from_row(row)?;
        header.folder = row.get(10)?;
        let account: String = row.get(9)?;
        // The branch-aware key, not the stored root: one root split by subject
        // becomes one card per branch, and a budget counting the root would let
        // a heavily branched conversation return far more cards than it allows.
        let thread = (
            account,
            starred_thread_identity(&header.folder, &card_thread_key(&header)),
        );
        if !threads.contains(&thread) {
            if threads.len() as u32 >= max_threads {
                continue;
            }
            threads.insert(thread.clone());
        }
        out.push((thread.0, header));
    }
    Ok(out)
}

pub fn get_thread_headers(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.in_reply_to') FROM messages
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
            in_reply_to: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
            folder: String::new(),
            ..Default::default()
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// The newest message of a thread (or of one subject branch of it), as a
/// single-element uid list — empty when the thread has no cached messages.
///
/// Marking a whole thread unread uses this: the gesture means "bring this back
/// to me", not "I read none of these", so only the newest message carries the
/// flag. The thread then shows one unread message and opening it lands on that
/// message, instead of reopening at the oldest one and shedding the count again
/// as the reader scrolls down.
pub fn newest_thread_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    subject_filter: Option<&str>,
) -> Result<Vec<u32>> {
    // get_thread_headers orders by date ascending, so the newest is last.
    let newest = get_thread_headers(conn, account, folder, thread_key)?
        .into_iter()
        .filter(|header| match subject_filter {
            Some(filter) => thread_grouping_subject(&header.subject) == filter,
            None => true,
        })
        .next_back();
    Ok(newest.map(|header| header.uid).into_iter().collect())
}

/// A thread's messages grouped by mailbox, for a whole-thread "mark read".
/// Covers every folder the reader shows the thread from (see `thread_read`),
/// not just the card's own, so an unread copy filed elsewhere is not left
/// behind a card that now reads as read. A synthetic `uid:N` key stays
/// folder-local: UIDs are only unique within one mailbox.
///
/// `unseen_only` narrows it to messages the store still has unread. Only a
/// caller that updates the store after the server write may use it: one that
/// marks the store first would leave a retry of a failed write nothing to send.
pub fn thread_uids_by_folder(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    subject_filter: Option<&str>,
    unseen_only: bool,
) -> Result<std::collections::BTreeMap<String, Vec<u32>>> {
    let only_folder = thread_key.starts_with("uid:").then_some(folder);
    let mut stmt = conn.prepare(
        "SELECT folder, uid, subject FROM messages
         WHERE account = ?1 AND uid <> 0 AND (?4 = 0 OR seen = 0)
           AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?2
           AND (?3 IS NULL OR folder = ?3)
         ORDER BY folder, uid",
    )?;
    let rows = stmt.query_map(
        params![account, thread_key, only_folder, unseen_only],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        },
    )?;
    let mut by_folder = std::collections::BTreeMap::<String, Vec<u32>>::new();
    for row in rows {
        let (folder, uid, subject) = row?;
        if subject_filter.is_some_and(|filter| thread_grouping_subject(&subject) != filter) {
            continue;
        }
        by_folder.entry(folder).or_default().push(uid);
    }
    Ok(by_folder)
}

/// Which of the account's threads have a draft waiting in them — the keys the
/// list's Draft badge is decided by. Only real thread keys: a draft with no
/// threading headers falls back to `uid:<its own uid>`, which says nothing about
/// any thread and would collide with an unrelated message that happens to hold
/// that UID in another mailbox (UIDs are unique only within one).
pub fn draft_thread_keys(conn: &Connection, account: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT m.thread_key, m.folder, f.special_use
           FROM messages m
           LEFT JOIN folders f ON f.account = m.account AND f.name = m.folder
          WHERE m.account = ?1 AND m.uid <> 0 AND m.thread_key <> ''",
    )?;
    let rows = stmt.query_map(params![account], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut out = HashSet::new();
    for row in rows {
        let (thread_key, folder, special_use) = row?;
        if classify_folder_role(&folder, special_use.as_deref()) == "drafts" {
            out.insert(thread_key);
        }
    }
    Ok(out)
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
    let mut gone = Vec::new();
    for uid in local_uids {
        if !server_uids.contains(&uid) {
            conn.execute(
                "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![account, folder, uid],
            )?;
            gone.push(uid);
            removed += 1;
        }
    }
    remember_removed_uids(conn, account, folder, &gone)?;
    Ok(removed)
}

/// Epoch of the latest UID removal. A folder sync reads this before it talks
/// to the server; removals that happen while that fetch is in flight get a
/// higher epoch and stay hidden from that snapshot.
pub fn removed_message_epoch(conn: &Connection) -> Result<i64> {
    Ok(conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM meta WHERE key = 'removed_uid_epoch'",
            [],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Forget removals at or before `fetched_at_epoch` for these UIDs. The sync
/// that passes the epoch it read before fetching may store a UID only when
/// nothing removed it afterwards — the snapshot and the removal agree.
pub fn release_removed_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
    fetched_at_epoch: i64,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "DELETE FROM removed_message_uids
         WHERE account = ?1 AND folder = ?2 AND uid = ?3 AND epoch <= ?4",
    )?;
    for uid in uids {
        stmt.execute(params![account, folder, *uid, fetched_at_epoch])?;
    }
    Ok(())
}

/// Names this process in `active_message_syncs`. Rows carrying another name
/// were left by a process that died mid-sync.
fn message_sync_owner() -> &'static str {
    static OWNER: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    OWNER.get_or_init(|| uuid::Uuid::new_v4().to_string())
}

/// Register a folder sync before network I/O. The registry is an ordinary
/// table because engines in one process each hold their own connection (the
/// mobile foreground engine and a transient background one), and a removal on
/// one must stay hidden from a snapshot another is still fetching.
/// Registration and cleanup run under the engine's database lock.
pub fn begin_message_sync(conn: &Connection) -> Result<(i64, i64)> {
    // Take the write lock before reading the epoch. A deferred transaction that
    // reads first fails with SQLITE_BUSY_SNAPSHOT when another connection
    // commits in between, and the busy timeout cannot wait that out.
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    let epoch = removed_message_epoch(&tx)?;
    tx.execute(
        "INSERT INTO active_message_syncs(epoch, owner) VALUES(?1, ?2)",
        params![epoch, message_sync_owner()],
    )?;
    let id = tx.last_insert_rowid();
    prune_removed_message_markers(&tx)?;
    tx.commit()?;
    Ok((id, epoch))
}

/// A finished, failed or cancelled sync no longer needs its removal markers.
pub fn end_message_sync(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM active_message_syncs WHERE id = ?1",
        params![id],
    )?;
    prune_removed_message_markers(conn)
}

fn prune_removed_message_markers(conn: &Connection) -> Result<()> {
    // Abandoned snapshots must not survive a restart and pin markers forever.
    conn.execute(
        "DELETE FROM active_message_syncs WHERE owner != ?1",
        params![message_sync_owner()],
    )?;
    // A snapshot begun at E cannot contain a UID removed before or at E.
    // Keep all later removals until the oldest active snapshot has finished.
    conn.execute(
        "DELETE FROM removed_message_uids
         WHERE epoch <= COALESCE((SELECT MIN(epoch) FROM active_message_syncs), ?1)",
        params![removed_message_epoch(conn)?],
    )?;
    Ok(())
}

fn removed_uids_in(conn: &Connection, account: &str, folder: &str) -> Result<HashSet<u32>> {
    let mut stmt =
        conn.prepare("SELECT uid FROM removed_message_uids WHERE account = ?1 AND folder = ?2")?;
    let rows = stmt.query_map(params![account, folder], |row| row.get(0))?;
    Ok(rows.collect::<std::result::Result<HashSet<_>, _>>()?)
}

pub(crate) fn remember_removed_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<()> {
    let uids: Vec<u32> = uids.iter().copied().filter(|uid| *uid != 0).collect();
    if uids.is_empty() {
        return Ok(());
    }
    let epoch = removed_message_epoch(conn)? + 1;
    conn.execute(
        "INSERT INTO meta(key, value) VALUES('removed_uid_epoch', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![epoch.to_string()],
    )?;
    let mut stmt = conn.prepare(
        "INSERT INTO removed_message_uids(account, folder, uid, epoch)
         VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(account, folder, uid) DO UPDATE SET epoch = excluded.epoch",
    )?;
    for uid in uids {
        stmt.execute(params![account, folder, uid, epoch])?;
    }
    Ok(())
}

pub fn clear_folder_messages(conn: &Connection, account: &str, folder: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM messages WHERE account = ?1 AND folder = ?2",
        params![account, folder],
    )?;
    conn.execute(
        "DELETE FROM uncached_unseen WHERE account = ?1 AND folder = ?2",
        params![account, folder],
    )?;
    // UIDVALIDITY changed: these UID numbers no longer name the messages that
    // were removed, so they must not block the folder's new contents.
    conn.execute(
        "DELETE FROM removed_message_uids WHERE account = ?1 AND folder = ?2",
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
pub(super) fn format_recipient_list(json: Option<&str>) -> String {
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

pub(super) fn format_recipient_value(value: Option<&Value>) -> String {
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
        delivered: extra["delivered"].as_bool().unwrap_or(false),
        date,
        body,
        body_html,
        body_is_rendered: extra["body_is_rendered"].as_bool().unwrap_or(false),
        preview: String::new(),
        attachments,
    }))
}

/// The `files` column for `message`: its real attachments as the list cards
/// name them, or NULL when there are none (see the v11 migration).
fn files_json(message: &Message) -> Option<String> {
    let files: Vec<Value> = message
        .file_attachments()
        .map(|attachment| json!({ "filename": attachment.filename, "mime": attachment.mime }))
        .collect();
    (!files.is_empty()).then(|| Value::Array(files).to_string())
}

/// Progress of [`backfill_files_batch`] in `meta`: the last `messages.id`
/// done, or `done`.
const FILES_BACKFILL_KEY: &str = "files_backfill";

/// Fill `messages.files` for up to `batch` rows cached before the column
/// existed (see the v11 migration), resuming where the last call stopped.
/// Returns true once every row has been visited. Each call is one short
/// transaction, so the caller can release the store between batches.
///
/// Mirrors [`Message::file_attachments`] in SQL: an attachment counts unless the
/// stored HTML shows it inline.
pub fn backfill_files_batch(conn: &Connection, batch: u32) -> Result<bool> {
    let cursor = match super::db::meta_get(conn, FILES_BACKFILL_KEY)?.as_deref() {
        Some("done") => return Ok(true),
        Some(id) => id.parse::<i64>().unwrap_or(0),
        None => 0,
    };
    let tx = conn.unchecked_transaction()?;
    let last: Option<i64> = tx.query_row(
        "SELECT max(id) FROM (SELECT id FROM messages WHERE id > ?1 ORDER BY id LIMIT ?2)",
        params![cursor, batch],
        |row| row.get(0),
    )?;
    let Some(last) = last else {
        super::db::meta_set(&tx, FILES_BACKFILL_KEY, "done")?;
        tx.commit()?;
        return Ok(true);
    };
    tx.execute(
        "UPDATE messages SET files = (
           SELECT CASE WHEN count(*) = 0 THEN NULL ELSE json_group_array(json_object(
             'filename', json_extract(attachment.value, '$.filename'),
             'mime', json_extract(attachment.value, '$.mime'))) END
           FROM json_each(messages.json, '$.attachments') AS attachment
           WHERE json_extract(attachment.value, '$.key') IS NULL
              OR instr(COALESCE(json_extract(messages.json, '$.body_html'), ''),
                       '/media/' || json_extract(attachment.value, '$.key')) = 0
         )
         WHERE id > ?1 AND id <= ?2 AND uid <> 0
           AND json_extract(json, '$.attachments') IS NOT NULL",
        params![cursor, last],
    )?;
    super::db::meta_set(&tx, FILES_BACKFILL_KEY, &last.to_string())?;
    tx.commit()?;
    Ok(false)
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
        "delivered": message.delivered,
        "body_html": message.body_html,
        "body_is_rendered": message.body_is_rendered,
        "attachments": attachments_json,
    })
    .to_string();

    conn.execute(
        "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json, files)
         SELECT ?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
         WHERE NOT EXISTS (
           SELECT 1 FROM removed_message_uids
           WHERE account = ?1 AND folder = ?2 AND uid = ?3
         )
         ON CONFLICT(account, folder, msg_id) DO UPDATE SET
           subject = excluded.subject,
           from_name = excluded.from_name,
           from_addr = excluded.from_addr,
           -- A body whose Date header didn't parse must not undo the header
           -- sync's INTERNALDATE fallback.
           date = CASE WHEN excluded.date = 0 THEN messages.date ELSE excluded.date END,
           body = excluded.body,
           json = json_patch(messages.json, excluded.json),
           files = excluded.files",
        params![
            account,
            folder,
            uid,
            message.subject,
            message.from_name,
            message.from_addr,
            message.date,
            message.body,
            extra_json,
            files_json(message)
        ],
    )?;

    Ok(())
}
