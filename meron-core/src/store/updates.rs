//! Local flag, draft, move, and delete updates to cached mail.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::imap::MessageHeader;

use super::*;

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

/// Resolve differing flags to physical mailbox copies. Unstar uses the same
/// conversation identity as starred cards; star follows the source mailbox's
/// messages and their duplicates. Message actions follow only those messages'
/// copies. Never equate unrelated folder-local UIDs.
pub fn starred_mutation_targets(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: Option<&str>,
    uids: &[u32],
    starred: bool,
) -> Result<BTreeMap<String, Vec<u32>>> {
    let mut targets: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    if let Some(key) = thread_key {
        let (root, subject) = split_thread_key(key);
        let matches_branch = |header: &MessageHeader| {
            header.uid != 0
                && subject
                    .as_deref()
                    .is_none_or(|subject| thread_grouping_subject(&header.subject) == subject)
        };
        if starred {
            // Star the source mailbox's messages and their physical duplicates,
            // not other replies (e.g. Sent) merely sharing the conversation root.
            let ids = get_thread_headers(conn, account, folder, &root)?
                .into_iter()
                .filter(matches_branch)
                .map(|header| header.uid)
                .collect::<Vec<_>>();
            return starred_mutation_targets(conn, account, folder, None, &ids, starred);
        }
        // Unstar must clear the whole conversation admitted by unified Starred.
        let mut stmt = conn.prepare(
            "SELECT DISTINCT folder FROM messages WHERE account = ?1 AND uid <> 0
             AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?2
             AND (?3 = 0 OR folder = ?4) AND starred <> ?5",
        )?;
        let folders = stmt
            .query_map(
                params![account, root, root.starts_with("uid:"), folder, starred],
                |row| row.get::<_, String>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for location in folders {
            let ids = get_thread_headers(conn, account, &location, &root)?
                .into_iter()
                .filter(|header| matches_branch(header) && header.starred != starred)
                .map(|header| header.uid)
                .collect::<Vec<_>>();
            if !ids.is_empty() {
                targets.insert(location, ids);
            }
        }
    } else if !uids.is_empty() {
        // Fetch all originals with the mailbox/UID index, then scan candidate
        // copies once per action instead of joining the entire account per UID.
        let placeholders = uids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let mut stmt = conn.prepare(&format!(
            "SELECT uid, starred, CAST(json_extract(json, '$.gmail_msg_id') AS TEXT),
                    lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>'))
             FROM messages WHERE account = ? AND folder = ? AND uid IN ({placeholders})"
        ))?;
        let mut args: Vec<&dyn rusqlite::ToSql> = vec![&account, &folder];
        args.extend(uids.iter().map(|uid| uid as &dyn rusqlite::ToSql));
        let mut originals = HashSet::new();
        let mut gmail_ids = HashSet::new();
        let mut message_ids = HashSet::new();
        for row in stmt.query_map(params_from_iter(args), |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, bool>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })? {
            let (uid, current, gmail_id, message_id) = row?;
            originals.insert(uid);
            if current != starred {
                targets.entry(folder.to_string()).or_default().push(uid);
            }
            if let Some(id) = gmail_id {
                gmail_ids.insert(id);
            }
            if !message_id.is_empty() {
                message_ids.insert(message_id);
            }
        }
        // Explicit UIDs can be actionable before their headers are cached.
        targets
            .entry(folder.to_string())
            .or_default()
            .extend(uids.iter().filter(|uid| !originals.contains(uid)).copied());
        if !gmail_ids.is_empty() || !message_ids.is_empty() {
            let mut copies = conn.prepare(
                "SELECT folder, uid, CAST(json_extract(json, '$.gmail_msg_id') AS TEXT),
                        lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>'))
                 FROM messages WHERE account = ?1 AND uid <> 0 AND starred <> ?2",
            )?;
            for row in copies.query_map(params![account, starred], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })? {
                let (location, uid, gmail_id, message_id) = row?;
                if gmail_id.is_some_and(|id| gmail_ids.contains(&id))
                    || message_ids.contains(&message_id)
                {
                    targets.entry(location).or_default().push(uid);
                }
            }
        }
    }
    targets.retain(|_, ids| {
        ids.sort_unstable();
        ids.dedup();
        !ids.is_empty()
    });
    Ok(targets)
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

/// Collapse multiple cached draft autosave rows in a conversation to the newest
/// one for display. This is intentionally render-time only: old builds could
/// mint different Message-IDs for the same quick-reply draft, so Message-ID
/// dedupe alone cannot hide them, but deleting by thread would be too broad for
/// a cache repair.
pub fn collapse_thread_draft_headers(
    conn: &Connection,
    account: &str,
    default_folder: &str,
    headers: Vec<MessageHeader>,
) -> Result<Vec<MessageHeader>> {
    let mut newest_draft: Option<(usize, i64, u32)> = None;
    let mut draft_flags = Vec::with_capacity(headers.len());
    for (idx, header) in headers.iter().enumerate() {
        let folder = if header.folder.is_empty() {
            default_folder
        } else {
            header.folder.as_str()
        };
        let is_draft = folder_role(conn, account, folder)? == "drafts";
        draft_flags.push(is_draft);
        if is_draft {
            match newest_draft {
                Some((_, date, uid)) if (header.date, header.uid) <= (date, uid) => {}
                _ => newest_draft = Some((idx, header.date, header.uid)),
            }
        }
    }
    let Some((keep_idx, _, _)) = newest_draft else {
        return Ok(headers);
    };
    Ok(headers
        .into_iter()
        .enumerate()
        .filter_map(|(idx, header)| {
            if draft_flags[idx] && idx != keep_idx {
                None
            } else {
                Some(header)
            }
        })
        .collect())
}

/// Delete every locally cached row in `folder` sharing a Message-ID with any of
/// `uids`. The thread read collapses same-Message-ID copies into one bubble, so
/// discarding the visible draft must also drop hidden stale autosave siblings —
/// otherwise the thread card keeps reporting `has_draft`. Call before
/// `delete_messages_by_uid` (it reads the rows to get their ids).
pub fn delete_draft_sibling_copies(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<usize> {
    let mut deleted = 0usize;
    for uid in uids {
        let message_id: Option<String> = conn
            .query_row(
                "SELECT json_extract(json, '$.message_id') FROM messages
                 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![account, folder, *uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if let Some(message_id) = message_id {
            deleted += delete_draft_copies(conn, account, folder, &message_id, Some(*uid))?;
        }
    }
    Ok(deleted)
}

/// Remove locally cached copies of a draft (matched by its stable Message-ID),
/// optionally keeping one UID — the copy that survived the server-side
/// replace/prune. Autosaves APPEND under a fresh UID each time, so without this
/// the expunged prior copy lingers locally as a duplicate until the next full
/// Drafts sync.
pub fn delete_draft_copies(
    conn: &Connection,
    account: &str,
    folder: &str,
    message_id: &str,
    keep_uid: Option<u32>,
) -> Result<usize> {
    if message_id.trim().is_empty() {
        return Ok(0);
    }
    let deleted = conn.execute(
        "DELETE FROM messages
         WHERE account = ?1 AND folder = ?2
           AND lower(COALESCE(json_extract(json, '$.message_id'), '')) = lower(?3)
           AND (?4 IS NULL OR uid <> ?4)",
        params![account, folder, message_id.trim(), keep_uid],
    )?;
    Ok(deleted)
}

/// Remove cached Meron quick-reply drafts in a known conversation. Used by the
/// desktop cleanup path for stale ids left by older autosave code. UID-only
/// fallback keys are deliberately rejected by requiring a stored thread key;
/// equal IMAP UIDs in different folders are unrelated messages.
pub fn delete_quick_reply_drafts_in_thread(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
) -> Result<usize> {
    if thread_key.trim().is_empty() || thread_key.starts_with("uid:") {
        return Ok(0);
    }
    Ok(conn.execute(
        "DELETE FROM messages
         WHERE account = ?1 AND folder = ?2
           AND NULLIF(thread_key, '') = ?3
           AND lower(COALESCE(json_extract(json, '$.message_id'), '')) GLOB 'meron-draft-*'",
        params![account, folder, thread_key],
    )?)
}

/// The lowercased Message-ID of each message in `uids` the cache holds, `""`
/// for one without, `None` for one it doesn't. Read before a move drops the
/// rows; see `engine::moved_message_ids`.
pub fn cached_message_ids(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<Option<String>>> {
    let mut stmt = conn.prepare(
        "SELECT lower(COALESCE(json_extract(json, '$.message_id'), '')) FROM messages
         WHERE account = ?1 AND folder = ?2 AND uid = ?3",
    )?;
    let mut ids = Vec::with_capacity(uids.len());
    for uid in uids {
        let id: Option<String> = stmt
            .query_row(params![account, folder, *uid], |row| row.get(0))
            .optional()?;
        ids.push(id.map(|id| id.trim().to_string()));
    }
    Ok(ids)
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

/// Drop every cached message in a folder. Pairs with `imap::empty_folder`, which
/// clears the server side.
pub fn delete_folder_messages(conn: &Connection, account: &str, folder: &str) -> Result<usize> {
    let deleted = conn.execute(
        "DELETE FROM messages WHERE account = ?1 AND folder = ?2",
        params![account, folder],
    )?;
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
