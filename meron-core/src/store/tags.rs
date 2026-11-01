//! Tags on cached mail.
//!
//! `message_tags` is the source of truth, so a tag works on every account,
//! online or not. On a server that stores keywords it is mirrored there, which
//! carries it to other devices and clients.
//!
//! A tag belongs to the message rather than to one mailbox copy: rows are keyed
//! by [`TAG_KEY_SQL`], which survives a move. Each copy's `server_tags` records
//! what that copy last showed, so sync merges the *change*: a copy that simply
//! never had a keyword (a fresh COPY, a folder without keyword support) says
//! nothing, and only a keyword that was there and went away removes a tag.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{now_unix, split_thread_key, thread_uids_by_folder};

const SYNCED: i64 = 0;
/// Added here, not yet on every copy on the server.
const PENDING_ADD: i64 = 1;
/// Removed here, possibly still on the server. Hidden from every read.
const PENDING_REMOVE: i64 = 2;
/// Removed everywhere a moment ago. Kept for [`STALE_ECHO_SECS`], hidden like a
/// pending removal, so a sync already on the wire cannot bring the tag back.
const REMOVED: i64 = 3;
/// SQL for the states a reader sees.
pub(super) const LIVE_STATES: &str = "(0, 1)";

/// A sync that was on the wire when a tag changed here reports the state from
/// before the change. For this long after a local change, a keyword going
/// missing may be that echo, and the local change wins: it is pushed again. If
/// it really was another client removing it just then, the two still converge,
/// which waiting for a later look would not promise: CONDSTORE reports a change
/// once.
const STALE_ECHO_SECS: i64 = 120;

/// A message's tag key: its normalized Message-ID, or its location when it has
/// none. Must match the expression of `messages_tag_key_idx` exactly.
pub(super) const TAG_KEY_SQL: &str = "COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid)";

fn parse_server_tags(text: Option<&str>) -> BTreeSet<String> {
    text.unwrap_or_default()
        .split(' ')
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn format_server_tags(tags: &BTreeSet<String>) -> String {
    tags.iter().cloned().collect::<Vec<_>>().join(" ")
}

fn set_server_tags(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    tags: &BTreeSet<String>,
) -> Result<()> {
    conn.prepare_cached(
        "UPDATE messages SET server_tags = ?4 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
    )?
    .execute(params![account, folder, uid, format_server_tags(tags)])?;
    Ok(())
}

/// The tag keys of the given mailbox copies.
pub fn message_tag_keys(
    conn: &Connection,
    account: &str,
    targets: &BTreeMap<String, Vec<u32>>,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TAG_KEY_SQL} FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3"
    ))?;
    let mut keys = BTreeSet::new();
    for (folder, uids) in targets {
        for uid in uids {
            if let Some(key) = stmt
                .query_row(params![account, folder, uid], |row| row.get::<_, String>(0))
                .optional()?
            {
                keys.insert(key);
            }
        }
    }
    Ok(keys.into_iter().collect())
}

/// Add and remove tags on messages. With `mirrored`, the change also waits to
/// be pushed to the server; the local account has no server to wait for.
pub fn change_tags(
    conn: &Connection,
    account: &str,
    keys: &[String],
    add: &[String],
    remove: &[String],
    mirrored: bool,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    write_tag_changes(&tx, account, keys, add, remove, mirrored)?;
    tx.commit()?;
    Ok(())
}

/// [`change_tags`], inside the caller's transaction.
pub(super) fn write_tag_changes(
    tx: &Connection,
    account: &str,
    keys: &[String],
    add: &[String],
    remove: &[String],
    mirrored: bool,
) -> Result<()> {
    let now = now_unix();
    for key in keys {
        for tag in add {
            let state = if mirrored { PENDING_ADD } else { SYNCED };
            tx.execute(
                "INSERT INTO message_tags(account, tag_key, tag, state, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(account, tag_key, tag) DO UPDATE SET
                   state = CASE WHEN state IN (?6, ?7) THEN ?4 ELSE state END,
                   updated_at = ?5",
                params![account, key, tag, state, now, PENDING_REMOVE, REMOVED],
            )?;
        }
        // Asked to do both, the tag stays.
        for tag in remove.iter().filter(|tag| !add.contains(tag)) {
            if mirrored {
                tx.execute(
                    "UPDATE message_tags SET state = ?4, updated_at = ?5
                     WHERE account = ?1 AND tag_key = ?2 AND tag = ?3 AND state <> ?6",
                    params![account, key, tag, PENDING_REMOVE, now, REMOVED],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM message_tags WHERE account = ?1 AND tag_key = ?2 AND tag = ?3",
                    params![account, key, tag],
                )?;
            }
        }
    }
    Ok(())
}

/// Which copies a tag change applies to: the given messages, or a whole thread
/// in every folder it spans. Tags read as a property of the conversation, so it
/// takes all of it, Sent replies included.
pub fn tag_targets(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    uids: &[u32],
) -> Result<BTreeMap<String, Vec<u32>>> {
    if !uids.is_empty() {
        return Ok(BTreeMap::from([(folder.to_string(), uids.to_vec())]));
    }
    let (root, subject) = split_thread_key(thread_key);
    // A `uid:` key names one message, even after a resync gave its row a real
    // thread key that the lookup below would no longer find it by.
    if let Some(uid) = root.strip_prefix("uid:").and_then(|uid| uid.parse().ok()) {
        return Ok(BTreeMap::from([(folder.to_string(), vec![uid])]));
    }
    thread_uids_by_folder(conn, account, folder, &root, subject.as_deref(), false)
}

/// Apply a tag change to the given copies, and return the tags they now carry.
/// See [`change_tags`].
pub fn retag(
    conn: &Connection,
    account: &str,
    targets: &BTreeMap<String, Vec<u32>>,
    add: &[String],
    remove: &[String],
    mirrored: bool,
) -> Result<Vec<String>> {
    let keys = message_tag_keys(conn, account, targets)?;
    change_tags(conn, account, &keys, add, remove, mirrored)?;
    let tags: BTreeSet<String> = tags_by_key(conn, account, &keys)?
        .into_values()
        .flatten()
        .collect();
    Ok(tags.into_iter().collect())
}

/// The tags of the given copies, to carry to another account or store with
/// [`apply_carried_tags`]. Two steps, so a caller moving between stores never
/// holds one store's lock while waiting for the other's. A location key means
/// nothing elsewhere, so a message without a Message-ID leaves its tags behind.
pub fn carried_tags(
    conn: &Connection,
    account: &str,
    copies: &BTreeMap<String, Vec<u32>>,
) -> Result<HashMap<String, Vec<String>>> {
    let keys = message_tag_keys(conn, account, copies)?;
    let mut tags = tags_by_key(conn, account, &keys)?;
    tags.retain(|key, _| !key.starts_with("uid:"));
    Ok(tags)
}

/// See [`carried_tags`].
pub fn apply_carried_tags(
    conn: &Connection,
    account: &str,
    carried: HashMap<String, Vec<String>>,
    mirrored: bool,
) -> Result<()> {
    for (key, tags) in carried {
        change_tags(conn, account, &[key], &tags, &[], mirrored)?;
    }
    Ok(())
}

/// Restore a backed-up tag. Never over a row already here: an edit made since
/// the export is kept. It starts pending, as the server may not have it.
pub fn restore_tag(conn: &Connection, account: &str, key: &str, tag: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO message_tags(account, tag_key, tag, state, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![account, key, tag, PENDING_ADD, now_unix()],
    )?;
    Ok(())
}

/// Merge what the server shows on one mailbox copy. See the module docs.
pub fn observe_server_tags(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    observed: &[String],
) -> Result<()> {
    let row = conn
        .prepare_cached(&format!(
            "SELECT server_tags, {TAG_KEY_SQL} FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid = ?3"
        ))?
        .query_row(params![account, folder, uid], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
        })
        .optional()?;
    let Some((previous, key)) = row else {
        return Ok(());
    };
    let observed: BTreeSet<String> = observed.iter().cloned().collect();
    let first_look = previous.is_none();
    let previous = parse_server_tags(previous.as_deref());
    if !first_look && previous == observed {
        return Ok(());
    }
    let now = now_unix();
    for tag in observed.difference(&previous) {
        let row: Option<(i64, i64)> = tag_row(conn, account, &key, tag)?;
        match row {
            // Just removed here. As with an add, the local change wins, and
            // pushing it again settles a real re-add elsewhere just the same.
            Some((REMOVED, updated_at)) if now - updated_at < STALE_ECHO_SECS => {
                set_tag_state(conn, account, &key, tag, PENDING_REMOVE)?;
            }
            Some((REMOVED, _)) => set_tag_state(conn, account, &key, tag, SYNCED)?,
            Some(_) => {}
            // `updated_at` dates local changes; this is not one. Copies still
            // without it are for the push to bring level.
            None => {
                let state = if other_copy_lacks_tag(conn, account, &key, folder, uid, tag)? {
                    PENDING_ADD
                } else {
                    SYNCED
                };
                conn.prepare_cached(
                    "INSERT INTO message_tags(account, tag_key, tag, state, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 0)",
                )?
                .execute(params![account, key, tag, state])?;
            }
        }
    }
    for tag in previous.difference(&observed) {
        let row = tag_row(conn, account, &key, tag)?;
        match row {
            Some((SYNCED, updated_at)) if now - updated_at < STALE_ECHO_SECS => {
                set_tag_state(conn, account, &key, tag, PENDING_ADD)?;
            }
            Some((SYNCED, _)) => {
                if other_copy_has_tag(conn, account, &key, folder, uid, tag)? {
                    // Removed elsewhere: follow through on the remaining copies.
                    set_tag_state(conn, account, &key, tag, PENDING_REMOVE)?;
                } else {
                    conn.prepare_cached(
                        "DELETE FROM message_tags
                         WHERE account = ?1 AND tag_key = ?2 AND tag = ?3",
                    )?
                    .execute(params![account, key, tag])?;
                }
            }
            _ => {}
        }
    }
    // A copy seen for the first time and missing a tag is not a removal, but
    // other clients reading this copy should still find the tag on it.
    if first_look {
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT tag FROM message_tags
             WHERE account = ?1 AND tag_key = ?2 AND state = {SYNCED}"
        ))?;
        let carried = stmt
            .query_map(params![account, key], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for tag in carried.iter().filter(|tag| !observed.contains(*tag)) {
            set_tag_state(conn, account, &key, tag, PENDING_ADD)?;
        }
    }
    set_server_tags(conn, account, folder, uid, &observed)
}

fn tag_row(conn: &Connection, account: &str, key: &str, tag: &str) -> Result<Option<(i64, i64)>> {
    Ok(conn
        .prepare_cached(
            "SELECT state, updated_at FROM message_tags
             WHERE account = ?1 AND tag_key = ?2 AND tag = ?3",
        )?
        .query_row(params![account, key, tag], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?)
}

fn set_tag_state(conn: &Connection, account: &str, key: &str, tag: &str, state: i64) -> Result<()> {
    conn.prepare_cached(
        "UPDATE message_tags SET state = ?4, updated_at = ?5
         WHERE account = ?1 AND tag_key = ?2 AND tag = ?3",
    )?
    .execute(params![account, key, tag, state, now_unix()])?;
    Ok(())
}

fn other_copy_has_tag(
    conn: &Connection,
    account: &str,
    key: &str,
    folder: &str,
    uid: u32,
    tag: &str,
) -> Result<bool> {
    Ok(other_copies(conn, account, key, folder, uid)?
        .iter()
        .any(|server_tags| parse_server_tags(server_tags.as_deref()).contains(tag)))
}

fn other_copy_lacks_tag(
    conn: &Connection,
    account: &str,
    key: &str,
    folder: &str,
    uid: u32,
    tag: &str,
) -> Result<bool> {
    Ok(other_copies(conn, account, key, folder, uid)?
        .iter()
        .any(|server_tags| !parse_server_tags(server_tags.as_deref()).contains(tag)))
}

/// What the message's other mailbox copies last showed.
fn other_copies(
    conn: &Connection,
    account: &str,
    key: &str,
    folder: &str,
    uid: u32,
) -> Result<Vec<Option<String>>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT server_tags FROM messages
         WHERE account = ?1 AND uid <> 0 AND {TAG_KEY_SQL} = ?2
           AND NOT (folder = ?3 AND uid = ?4)"
    ))?;
    let rows = stmt.query_map(params![account, key, folder, uid], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Whether a mailbox holds mail whose keywords were never read, as all of it
/// was when tags arrived.
pub fn needs_tag_baseline(conn: &Connection, account: &str, folder: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM messages
                       WHERE account = ?1 AND folder = ?2 AND uid <> 0 AND server_tags IS NULL)",
        params![account, folder],
        |row| row.get(0),
    )?)
}

/// A full read of the mailbox is done. Rows it did not cover are gone from the
/// server, and must not ask for another one on every sync.
pub fn mark_tag_baseline(conn: &Connection, account: &str, folder: &str) -> Result<()> {
    conn.execute(
        "UPDATE messages SET server_tags = ''
         WHERE account = ?1 AND folder = ?2 AND uid <> 0 AND server_tags IS NULL",
        params![account, folder],
    )?;
    Ok(())
}

/// Forget tags keyed by location in a mailbox whose UIDs were reset: those UIDs
/// now name different messages.
pub fn clear_location_tags(conn: &Connection, account: &str, folder: &str) -> Result<()> {
    let prefix = format!("uid:{folder}:");
    conn.execute(
        "DELETE FROM message_tags
         WHERE account = ?1 AND substr(tag_key, 1, length(?2)) = ?2",
        params![account, prefix],
    )?;
    Ok(())
}

/// Whether the account has any tag at all, which most never will: the card
/// pages can then skip reading them.
pub(super) fn has_tags(conn: &Connection, account: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM message_tags WHERE account = ?1)",
        params![account],
        |row| row.get(0),
    )?)
}

/// Whether anything is waiting to reach the server.
pub fn has_pending_tags(conn: &Connection, account: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM message_tags WHERE account = ?1 AND state IN (1, 2))",
        params![account],
        |row| row.get(0),
    )?)
}

/// One keyword write the server still needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPush {
    pub folder: String,
    pub tag: String,
    pub add: bool,
    pub uids: Vec<u32>,
}

/// A pending tag and the mailbox copies its message has.
struct PendingTag {
    key: String,
    tag: String,
    state: i64,
    copies: Vec<(String, u32, Option<String>)>,
}

fn pending_tags(conn: &Connection, account: &str) -> Result<Vec<PendingTag>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT t.tag_key, t.tag, t.state, m.folder, m.uid, m.server_tags
         FROM message_tags t
         LEFT JOIN messages m ON m.account = t.account AND m.uid <> 0
              AND {} = t.tag_key
         WHERE t.account = ?1 AND t.state IN ({PENDING_ADD}, {PENDING_REMOVE})
         ORDER BY t.tag_key, t.tag",
        qualified_tag_key("m"),
    ))?;
    let rows = stmt.query_map(params![account], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<u32>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    let mut pending: Vec<PendingTag> = Vec::new();
    for row in rows {
        let (key, tag, state, folder, uid, server_tags) = row?;
        if pending
            .last()
            .is_none_or(|last| last.key != key || last.tag != tag)
        {
            pending.push(PendingTag {
                key,
                tag,
                state,
                copies: Vec::new(),
            });
        }
        if let (Some(folder), Some(uid), Some(entry)) = (folder, uid, pending.last_mut()) {
            entry.copies.push((folder, uid, server_tags));
        }
    }
    Ok(pending)
}

/// [`TAG_KEY_SQL`] with its columns qualified, for joins.
pub(super) fn qualified_tag_key(alias: &str) -> String {
    TAG_KEY_SQL
        .replace("json_extract(json", &format!("json_extract({alias}.json"))
        .replace("|| folder ||", &format!("|| {alias}.folder ||"))
        .replace("|| uid)", &format!("|| {alias}.uid)"))
}

/// Whether a copy still needs a write for this pending tag. A copy never looked
/// at might hold a keyword being removed, so it is written to rather than
/// assumed clean.
fn copy_needs_push(state: i64, tag: &str, server_tags: Option<&str>) -> bool {
    let has = parse_server_tags(server_tags).contains(tag);
    if state == PENDING_ADD {
        !has
    } else {
        has || server_tags.is_none()
    }
}

/// The keyword writes that would bring the server up to date, per mailbox.
pub fn pending_tag_pushes(conn: &Connection, account: &str) -> Result<Vec<TagPush>> {
    let mut grouped: BTreeMap<(String, String, bool), BTreeSet<u32>> = BTreeMap::new();
    for pending in pending_tags(conn, account)? {
        for (folder, uid, server_tags) in pending.copies {
            if copy_needs_push(pending.state, &pending.tag, server_tags.as_deref()) {
                grouped
                    .entry((folder, pending.tag.clone(), pending.state == PENDING_ADD))
                    .or_default()
                    .insert(uid);
            }
        }
    }
    Ok(grouped
        .into_iter()
        .map(|((folder, tag, add), uids)| TagPush {
            folder,
            tag,
            add,
            uids: uids.into_iter().collect(),
        })
        .collect())
}

/// Record a keyword write the server accepted on `push`'s copies.
pub fn record_tag_push(conn: &Connection, account: &str, push: &TagPush) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for uid in &push.uids {
        let current: Option<Option<String>> = tx
            .prepare_cached(
                "SELECT server_tags FROM messages
                 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            )?
            .query_row(params![account, push.folder, uid], |row| row.get(0))
            .optional()?;
        let Some(current) = current else { continue };
        let mut tags = parse_server_tags(current.as_deref());
        if push.add {
            tags.insert(push.tag.clone());
        } else {
            tags.remove(&push.tag);
        }
        set_server_tags(&tx, account, &push.folder, *uid, &tags)?;
    }
    tx.commit()?;
    Ok(())
}

/// Close out pending tags whose every copy now agrees. `refused` lists the
/// (mailbox, tag) pairs that turned a keyword down: those copies have no say in
/// an add, which stays pending while they are all that is left of it, in case
/// the mailbox learns keywords.
pub fn settle_pending_tags(
    conn: &Connection,
    account: &str,
    refused: &HashSet<(String, String)>,
) -> Result<()> {
    for pending in pending_tags(conn, account)? {
        let adding = pending.state == PENDING_ADD;
        let copies: Vec<_> = pending
            .copies
            .iter()
            .filter(|(folder, _, _)| {
                !(adding && refused.contains(&(folder.clone(), pending.tag.clone())))
            })
            .collect();
        let outstanding = copies.iter().any(|(_, _, server_tags)| {
            copy_needs_push(pending.state, &pending.tag, server_tags.as_deref())
        });
        if outstanding || copies.is_empty() {
            continue;
        }
        let settled = if adding { SYNCED } else { REMOVED };
        set_tag_state(conn, account, &pending.key, &pending.tag, settled)?;
    }
    conn.execute(
        "DELETE FROM message_tags WHERE account = ?1 AND state = ?2 AND updated_at < ?3",
        params![account, REMOVED, now_unix() - STALE_ECHO_SECS],
    )?;
    Ok(())
}

/// How many messages carry each tag on this account, by tag.
pub fn tag_counts(conn: &Connection, account: Option<&str>) -> Result<BTreeMap<String, u32>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT tag, COUNT(*) FROM message_tags
         WHERE state IN {LIVE_STATES} AND (?1 IS NULL OR account = ?1)
         GROUP BY tag"
    ))?;
    let rows = stmt.query_map(params![account], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// [`tag_counts`] as the `tags.list` answer.
pub fn tag_counts_json(counts: BTreeMap<String, u32>) -> serde_json::Value {
    let tags: Vec<_> = counts
        .into_iter()
        .map(|(tag, count)| serde_json::json!({ "tag": tag, "count": count }))
        .collect();
    serde_json::json!({ "tags": tags })
}

/// Rename `tag` on every message of this store, or take it off with no `to`, and
/// answer with the accounts that changed. A message already carrying `to` just
/// loses `tag`. Only mail cached here can change: a keyword on mail not yet
/// downloaded reads as the old tag when it arrives.
pub fn replace_tag(
    conn: &Connection,
    tag: &str,
    to: Option<&str>,
    mirrored: bool,
) -> Result<Vec<String>> {
    let tx = conn.unchecked_transaction()?;
    let accounts = replace_message_tag(&tx, tag, to, mirrored)?;
    tx.commit()?;
    Ok(accounts)
}

/// [`replace_tag`] and [`super::replace_rule_tag`] as one step, for a store whose
/// rules sync applies. A write transaction from the start, as
/// [`super::apply_rules`] holds one: rules it read can't then land after this sweep and bring the tag back.
pub fn replace_tag_and_rules(
    conn: &Connection,
    tag: &str,
    to: Option<&str>,
) -> Result<(Vec<String>, Vec<super::Rule>)> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let rules = super::replace_rule_tag(&tx, tag, to)?;
    let accounts = replace_message_tag(&tx, tag, to, true)?;
    tx.commit()?;
    Ok((accounts, rules))
}

fn replace_message_tag(
    conn: &Connection,
    tag: &str,
    to: Option<&str>,
    mirrored: bool,
) -> Result<Vec<String>> {
    if to == Some(tag) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT account, tag_key FROM message_tags
         WHERE tag = ?1 AND state IN {LIVE_STATES}"
    ))?;
    let mut keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in stmt.query_map(params![tag], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (account, key) = row?;
        keys.entry(account).or_default().push(key);
    }
    let add: Vec<String> = to.iter().map(|to| to.to_string()).collect();
    let remove = [tag.to_string()];
    for (account, keys) in &keys {
        write_tag_changes(conn, account, keys, &add, &remove, mirrored)?;
    }
    Ok(keys.into_keys().collect())
}

/// The tags of one mailbox copy's message.
pub fn message_tags(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
) -> Result<Vec<String>> {
    let targets = BTreeMap::from([(folder.to_string(), vec![uid])]);
    let keys = message_tag_keys(conn, account, &targets)?;
    Ok(tags_by_key(conn, account, &keys)?
        .into_values()
        .next()
        .unwrap_or_default())
}

/// The tags of each tag key among `keys`.
pub fn tags_by_key(
    conn: &Connection,
    account: &str,
    keys: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT tag FROM message_tags
         WHERE account = ?1 AND tag_key = ?2 AND state IN {LIVE_STATES}
         ORDER BY tag"
    ))?;
    let mut out = HashMap::new();
    for key in keys {
        let tags = stmt
            .query_map(params![account, key], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !tags.is_empty() {
            out.insert(key.clone(), tags);
        }
    }
    Ok(out)
}

/// The tags on a thread card's messages, keyed by card key, scoped and folded
/// like [`card_message_counts`](super::card_message_counts).
pub(super) fn tags_of_cards(
    conn: &Connection,
    account: &str,
    keys_by_card: HashMap<String, Vec<String>>,
) -> Result<HashMap<String, Vec<String>>> {
    let all: Vec<String> = keys_by_card.values().flatten().cloned().collect();
    let tags = tags_by_key(conn, account, &all)?;
    Ok(keys_by_card
        .into_iter()
        .filter_map(|(card, keys)| {
            let merged: BTreeSet<&String> = keys
                .iter()
                .filter_map(|key| tags.get(key))
                .flatten()
                .collect();
            (!merged.is_empty()).then(|| (card, merged.into_iter().cloned().collect()))
        })
        .collect())
}

/// Every live tag of an account as `(tag_key, tag)`, for backup. Not the ones
/// keyed by location: a UID proves nothing once the mailbox may have been reset.
pub fn account_tags(conn: &Connection, account: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT tag_key, tag FROM message_tags
         WHERE account = ?1 AND state IN {LIVE_STATES} AND tag_key NOT LIKE 'uid:%'
         ORDER BY tag_key, tag"
    ))?;
    let rows = stmt.query_map(params![account], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::MessageHeader;
    use crate::store::{
        Rule, RuleCopy, apply_rules, count_existing_rule_matches, finish_rule_actions,
        pending_rule_actions, queue_existing_rule, replace_rule_tag, retry_rule_actions, rules,
        set_rules,
    };

    fn db() -> Connection {
        let path = std::env::temp_dir().join(format!(
            "meron-tags-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        crate::store::open_at(&path).unwrap()
    }

    fn message(uid: u32, id: &str, server_tags: Option<&[&str]>) -> MessageHeader {
        MessageHeader {
            uid,
            subject: "s".into(),
            thread_key: "t".into(),
            message_id: id.into(),
            server_tags: server_tags.map(|tags| tags.iter().map(|tag| tag.to_string()).collect()),
            ..Default::default()
        }
    }

    fn tags(conn: &Connection, id: &str) -> Vec<String> {
        tags_by_key(conn, "a", &[id.to_string()])
            .unwrap()
            .remove(id)
            .unwrap_or_default()
    }

    fn state(conn: &Connection, tag: &str) -> i64 {
        conn.query_row(
            "SELECT state FROM message_tags WHERE tag = ?1",
            [tag],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn age_tags(conn: &Connection) {
        conn.execute("UPDATE message_tags SET updated_at = 0", [])
            .unwrap();
    }

    fn upsert(conn: &Connection, folder: &str, messages: &[MessageHeader]) {
        crate::store::upsert_messages(conn, "a", folder, messages).unwrap();
    }

    #[test]
    fn the_key_index_matches_every_query_form() {
        let conn = db();
        for query in [
            format!(
                "SELECT 1 FROM messages WHERE account = 'a' AND uid <> 0 AND {TAG_KEY_SQL} = 'x'"
            ),
            format!(
                "SELECT 1 FROM messages m WHERE m.account = 'a' AND m.uid <> 0 AND {} = 'x'",
                qualified_tag_key("m")
            ),
        ] {
            let plan: String = conn
                .query_row(&format!("EXPLAIN QUERY PLAN {query}"), [], |row| row.get(3))
                .unwrap();
            assert!(plan.contains("messages_tag_key_idx"), "{plan}");
        }
    }

    #[test]
    fn a_keyword_from_another_client_becomes_a_tag_and_leaves_with_it() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "<M@x>", Some(&["todo"]))]);
        assert_eq!(tags(&conn, "m@x"), ["todo"]);
        age_tags(&conn);
        upsert(&conn, "INBOX", &[message(1, "<M@x>", Some(&[]))]);
        assert!(tags(&conn, "m@x").is_empty());
    }

    #[test]
    fn a_copy_that_never_had_the_keyword_keeps_the_tag_and_gets_it_written() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        age_tags(&conn);
        upsert(&conn, "Archive", &[message(9, "m@x", Some(&[]))]);
        upsert(&conn, "Archive", &[message(9, "m@x", Some(&[]))]);
        assert_eq!(tags(&conn, "m@x"), ["todo"]);
        assert_eq!(
            pending_tag_pushes(&conn, "a").unwrap(),
            [TagPush {
                folder: "Archive".into(),
                tag: "todo".into(),
                add: true,
                uids: vec![9],
            }]
        );
    }

    #[test]
    fn a_stale_sync_right_after_a_push_cannot_remove_the_tag() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        change_tags(&conn, "a", &["m@x".into()], &["todo".into()], &[], true).unwrap();
        for push in pending_tag_pushes(&conn, "a").unwrap() {
            record_tag_push(&conn, "a", &push).unwrap();
        }
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        assert_eq!(state(&conn, "todo"), SYNCED);

        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        assert_eq!(tags(&conn, "m@x"), ["todo"], "echo from before the push");
        assert_eq!(
            pending_tag_pushes(&conn, "a").unwrap().len(),
            1,
            "written again, so server and store converge without another look"
        );
    }

    #[test]
    fn a_stale_sync_right_after_a_removal_cannot_revive_the_tag() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        change_tags(&conn, "a", &["m@x".into()], &[], &["todo".into()], true).unwrap();
        for push in pending_tag_pushes(&conn, "a").unwrap() {
            record_tag_push(&conn, "a", &push).unwrap();
        }
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        assert_eq!(state(&conn, "todo"), REMOVED);

        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        assert!(tags(&conn, "m@x").is_empty(), "echo from before the push");
        assert_eq!(state(&conn, "todo"), PENDING_REMOVE);
    }

    #[test]
    fn a_tombstone_gives_way_to_a_later_add_and_expires() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        change_tags(&conn, "a", &["m@x".into()], &[], &["todo".into()], true).unwrap();
        for push in pending_tag_pushes(&conn, "a").unwrap() {
            record_tag_push(&conn, "a", &push).unwrap();
        }
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        change_tags(&conn, "a", &["m@x".into()], &["todo".into()], &[], true).unwrap();
        assert_eq!(tags(&conn, "m@x"), ["todo"], "added back by hand");

        change_tags(&conn, "a", &["m@x".into()], &[], &["todo".into()], true).unwrap();
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        age_tags(&conn);
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM message_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn tag_search_finds_any_copy_and_skips_removed_tags() {
        let conn = db();
        let mut archived = message(9, "m@x", Some(&["todo"]));
        archived.date = 5;
        upsert(&conn, "Archive", &[archived]);
        upsert(&conn, "INBOX", &[message(1, "other@x", Some(&[]))]);
        let folders = ["INBOX".to_string(), "Archive".to_string()];
        let found = |query: &str| {
            crate::store::search_messages_in_folders(&conn, "a", &folders, query, 10, None)
                .unwrap()
                .len()
        };
        assert_eq!(found("tag:todo"), 1);
        assert_eq!(found("tag:other"), 0);
        change_tags(&conn, "a", &["m@x".into()], &[], &["todo".into()], true).unwrap();
        assert_eq!(found("tag:todo"), 0);
    }

    #[test]
    fn a_search_for_several_tags_wants_them_all() {
        let conn = db();
        upsert(
            &conn,
            "INBOX",
            &[
                message(1, "both@x", Some(&["todo", "work"])),
                message(2, "one@x", Some(&["todo"])),
            ],
        );
        let folders = ["INBOX".to_string()];
        let found = |query: &str| {
            crate::store::search_messages_in_folders(&conn, "a", &folders, query, 10, None)
                .unwrap()
                .len()
        };
        assert_eq!(found("tag:todo"), 2);
        assert_eq!(found("tag:todo tag:work"), 1);
        assert_eq!(found("tag:todo tag:absent"), 0);
    }

    #[test]
    fn rules_tag_only_the_new_mail_that_matches() {
        let conn = db();
        let rule = Rule {
            query: "from:billing".into(),
            tags: vec!["Finance".into()],
            ..Default::default()
        };
        assert_eq!(set_rules(&conn, vec![rule]).unwrap()[0].tags, ["finance"]);

        let mut new_bill = message(2, "new@x", Some(&[]));
        new_bill.from_addr = "billing@shop.test".into();
        let mut old_bill = message(1, "old@x", Some(&[]));
        old_bill.from_addr = "billing@shop.test".into();
        let other = message(3, "other@x", Some(&[]));
        upsert(&conn, "INBOX", &[old_bill, new_bill, other]);

        apply_rules(&conn, "a", "INBOX", 1, &[2, 3]).unwrap();
        assert_eq!(tags(&conn, "new@x"), ["finance"]);
        assert!(tags(&conn, "old@x").is_empty(), "already here before");
        assert!(tags(&conn, "other@x").is_empty());
    }

    #[test]
    fn mail_filed_from_another_folder_has_not_arrived() {
        let conn = db();
        let known = message(1, "m@x", Some(&[]));
        upsert(&conn, "INBOX", std::slice::from_ref(&known));
        let filed = message(7, "m@x", Some(&[]));
        let fresh = message(8, "new@x", Some(&[]));
        let first_seen =
            crate::store::first_seen_messages(&conn, "a", "Archive", 7, 9, &[filed, fresh])
                .unwrap();
        let uids: Vec<u32> = first_seen.iter().map(|message| message.uid).collect();
        assert_eq!(uids, [8]);
    }

    #[test]
    fn rules_leave_sent_mail_alone_and_refuse_to_be_empty() {
        let conn = db();
        let rule = Rule {
            query: "subject:s".into(),
            tags: vec!["todo".into()],
            ..Default::default()
        };
        set_rules(&conn, vec![rule.clone()]).unwrap();
        upsert(&conn, "Sent", &[message(1, "m@x", Some(&[]))]);
        apply_rules(&conn, "a", "Sent", 1, &[1]).unwrap();
        assert!(tags(&conn, "m@x").is_empty());

        // No search is all new mail; no action is nothing at all.
        let no_search = Rule {
            query: " ".into(),
            ..rule.clone()
        };
        assert_eq!(set_rules(&conn, vec![no_search]).unwrap()[0].query, "");
        let no_tag = Rule {
            tags: vec![],
            ..rule
        };
        assert!(set_rules(&conn, vec![no_tag]).is_err());
    }

    #[test]
    fn renaming_a_tag_moves_it_on_the_server_and_merges_into_an_existing_one() {
        let conn = db();
        upsert(
            &conn,
            "INBOX",
            &[
                message(1, "one@x", Some(&["old"])),
                message(2, "both@x", Some(&["new", "old"])),
                message(3, "other@x", Some(&["keep"])),
            ],
        );
        assert_eq!(replace_tag(&conn, "old", Some("new"), true).unwrap(), ["a"]);
        assert_eq!(tags(&conn, "one@x"), ["new"]);
        assert_eq!(tags(&conn, "both@x"), ["new"]);
        assert_eq!(tags(&conn, "other@x"), ["keep"]);
        assert_eq!(tag_counts(&conn, None).unwrap().get("old"), None);

        let pushes = pending_tag_pushes(&conn, "a").unwrap();
        assert_eq!(pushes.len(), 2);
        assert!(
            pushes
                .iter()
                .any(|push| push.tag == "new" && push.add && push.uids == [1])
        );
        assert!(
            pushes
                .iter()
                .any(|push| push.tag == "old" && !push.add && push.uids == [1, 2])
        );
    }

    #[test]
    fn a_deleted_tag_leaves_every_account_and_stays_gone_through_a_stale_sync() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["old", "keep"]))]);
        crate::store::upsert_messages(&conn, "b", "INBOX", &[message(1, "m@x", Some(&["old"]))])
            .unwrap();
        assert_eq!(replace_tag(&conn, "old", None, true).unwrap(), ["a", "b"]);
        assert_eq!(tags(&conn, "m@x"), ["keep"]);
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["old", "keep"]))]);
        assert_eq!(tags(&conn, "m@x"), ["keep"], "echo from before the push");
    }

    #[test]
    fn replacing_a_tag_with_itself_or_an_unknown_one_changes_nothing() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        assert!(
            replace_tag(&conn, "todo", Some("todo"), true)
                .unwrap()
                .is_empty()
        );
        assert!(replace_tag(&conn, "absent", None, true).unwrap().is_empty());
        assert_eq!(tags(&conn, "m@x"), ["todo"]);
        assert!(pending_tag_pushes(&conn, "a").unwrap().is_empty());
    }

    #[test]
    fn local_mail_renames_without_waiting_for_a_server() {
        let conn = db();
        upsert(&conn, "Notes", &[message(1, "m@x", None)]);
        change_tags(&conn, "a", &["m@x".into()], &["old".into()], &[], false).unwrap();
        replace_tag(&conn, "old", Some("new"), false).unwrap();
        assert_eq!(tags(&conn, "m@x"), ["new"]);
        assert!(!has_pending_tags(&conn, "a").unwrap());
    }

    #[test]
    fn rules_follow_a_rename_and_lose_a_deleted_tag() {
        let conn = db();
        let rule = |query: &str, tags: &[&str]| Rule {
            query: query.into(),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            ..Default::default()
        };
        set_rules(
            &conn,
            vec![
                rule("from:a", &["old", "keep"]),
                rule("from:b tag:OLD tag:older", &["keep"]),
                rule("from:c", &["old"]),
            ],
        )
        .unwrap();

        let renamed = replace_rule_tag(&conn, "old", Some("new")).unwrap();
        assert_eq!(renamed[0].tags, ["keep", "new"]);
        assert_eq!(renamed[1].query, "from:b tag:new tag:older");
        assert_eq!(renamed[2].tags, ["new"]);

        let deleted = replace_rule_tag(&conn, "new", None).unwrap();
        assert_eq!(deleted.len(), 2, "a rule with nothing left to do is gone");
        assert_eq!(deleted[0].tags, ["keep"]);
        assert_eq!(rules(&conn).unwrap().len(), 2);
    }

    #[test]
    fn a_deleted_tag_leaves_a_rule_its_other_actions() {
        let conn = db();
        let rule = Rule {
            query: "from:a".into(),
            tags: vec!["old".into()],
            star: true,
            ..Default::default()
        };
        set_rules(&conn, vec![rule]).unwrap();
        let kept = replace_rule_tag(&conn, "old", None).unwrap();
        assert_eq!(kept.len(), 1);
        assert!(kept[0].tags.is_empty() && kept[0].star);
    }

    #[test]
    fn rules_may_do_anything_but_nothing() {
        let conn = db();
        let rule = |move_to: &str, folder: &str| Rule {
            query: "from:a".into(),
            move_to: move_to.into(),
            folder: folder.into(),
            ..Default::default()
        };
        assert!(set_rules(&conn, vec![rule("", "")]).is_err());
        assert!(set_rules(&conn, vec![rule("folder", " ")]).is_err());
        assert!(set_rules(&conn, vec![rule("somewhere", "")]).is_err());
        let kept = set_rules(
            &conn,
            vec![rule("trash", "stray"), rule("folder", " Work ")],
        )
        .unwrap();
        assert_eq!(kept[0].folder, "");
        assert_eq!(kept[1].folder, "Work");
    }

    #[test]
    fn matching_rules_add_up_and_the_first_move_wins() {
        let conn = db();
        let rule = |query: &str| Rule {
            query: query.into(),
            ..Default::default()
        };
        set_rules(
            &conn,
            vec![
                Rule {
                    mark_read: true,
                    ..rule("subject:s")
                },
                Rule {
                    move_to: "trash".into(),
                    ..rule("subject:s")
                },
                Rule {
                    star: true,
                    move_to: "archive".into(),
                    ..rule("subject:s")
                },
            ],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 9, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued.len(), 1);
        let action = &queued[0];
        assert_eq!((action.uid, action.uidvalidity), (1, 9));
        assert!(action.mark_read && action.star && action.trash);
        assert_eq!(action.target, "");
    }

    #[test]
    fn a_rule_without_a_search_takes_all_new_mail() {
        let conn = db();
        set_rules(
            &conn,
            vec![Rule {
                star: true,
                ..Default::default()
            }],
        )
        .unwrap();
        upsert(
            &conn,
            "INBOX",
            &[message(1, "m@x", Some(&[])), message(2, "n@x", Some(&[]))],
        );

        apply_rules(&conn, "a", "INBOX", 1, &[1, 2]).unwrap();
        assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 2);
    }

    #[test]
    fn running_a_rule_by_hand_takes_inbox_mail_and_is_safe_to_repeat() {
        let conn = db();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url, config, created_at, updated_at)
             VALUES('a', 'imap', 'imap', 'a@x.test', '', '', '{}', 0, 0)",
            [],
        )
        .unwrap();
        crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
        let rules = set_rules(
            &conn,
            vec![Rule {
                id: "r".into(),
                query: "subject:s".into(),
                star: true,
                copy_account: "a".into(),
                copy_folder: "Saved".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        let id = &rules[0].id;
        upsert(
            &conn,
            "INBOX",
            &[message(1, "m@x", Some(&[])), message(2, "n@x", Some(&[]))],
        );
        upsert(&conn, "Saved", &[message(9, "n@x", Some(&[]))]);
        upsert(&conn, "Other", &[message(5, "o@x", Some(&[]))]);

        assert_eq!(count_existing_rule_matches(&conn, id).unwrap(), 2);
        assert_eq!(
            queue_existing_rule(&conn, id).unwrap(),
            [("a".to_string(), 2)]
        );
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued.len(), 2);
        assert_eq!(queued[0].uidvalidity, 7);
        assert_eq!(queued[0].copies.len(), 1);
        assert!(
            pending_rule_actions(&conn, "a", "Other")
                .unwrap()
                .is_empty()
        );
        assert!(queue_existing_rule(&conn, id).unwrap().is_empty());
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued[0].copies.len(), 1);
    }

    fn account(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url, config, created_at, updated_at)
             VALUES(?1, 'imap', 'imap', ?1, '', '', '{}', 0, 0)",
            [id],
        )
        .unwrap();
    }

    #[test]
    fn a_rule_turned_off_does_nothing() {
        let conn = db();
        account(&conn, "a");
        crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
        let rules = set_rules(
            &conn,
            vec![Rule {
                star: true,
                disabled: true,
                ..Default::default()
            }],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );
        assert!(queue_existing_rule(&conn, &rules[0].id).is_err());
    }

    #[test]
    fn mail_without_a_message_id_is_not_copied_or_moved_across_accounts() {
        let conn = db();
        set_rules(
            &conn,
            vec![Rule {
                star: true,
                move_to: "folder".into(),
                folder: "Saved".into(),
                folder_account: "b".into(),
                copy_account: "b".into(),
                copy_folder: "Other".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert!(queued[0].star);
        assert!(queued[0].copies.is_empty() && !queued[0].trash);
        let log = crate::store::rule_log(&conn).unwrap();
        assert!(log.iter().any(|entry| entry.failed));
    }

    #[test]
    fn removing_an_account_narrows_a_rule_or_turns_it_off() {
        let conn = db();
        let rule = |accounts: &[&str], copy: &str| Rule {
            star: true,
            accounts: accounts.iter().map(|id| id.to_string()).collect(),
            copy_account: copy.into(),
            copy_folder: if copy.is_empty() { "" } else { "Saved" }.into(),
            ..Default::default()
        };
        set_rules(
            &conn,
            vec![
                rule(&["gone", "kept"], ""),
                rule(&["gone"], ""),
                rule(&[], "gone"),
            ],
        )
        .unwrap();

        crate::store::forget_rule_account(&conn, "gone").unwrap();
        let rules = crate::store::rules(&conn).unwrap();
        assert_eq!(rules[0].accounts, ["kept"]);
        assert!(!rules[0].disabled);
        assert_eq!(rules[1].accounts, ["gone"]);
        assert!(rules[1].disabled && rules[2].disabled);
    }

    #[test]
    fn what_rules_did_is_logged_and_a_given_up_action_counts_against_its_rule() {
        let conn = db();
        let rules = set_rules(
            &conn,
            vec![Rule {
                star: true,
                ..Default::default()
            }],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();

        for _ in 0..5 {
            retry_rule_actions(&conn, "a", "INBOX", &[1], 0, "no such folder").unwrap();
        }
        let log = crate::store::rule_log(&conn).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].rule_id, rules[0].id);
        assert!(log[0].failed);
        assert_eq!(log[0].detail, "no such folder");
        assert_eq!(log[0].subject, "s");
        assert_eq!(log[1].subject, "s");
        assert!(!log[1].failed);
    }

    #[test]
    fn a_run_by_hand_reports_what_is_left_and_can_be_stopped() {
        let conn = db();
        account(&conn, "a");
        crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
        let rules = set_rules(
            &conn,
            vec![Rule {
                query: "subject:s".into(),
                star: true,
                ..Default::default()
            }],
        )
        .unwrap();
        let id = rules[0].id.clone();
        upsert(
            &conn,
            "INBOX",
            &[message(1, "m@x", Some(&[])), message(2, "n@x", Some(&[]))],
        );

        queue_existing_rule(&conn, &id).unwrap();
        assert_eq!(crate::store::running_rules(&conn).unwrap()[&id], 2);
        crate::store::stop_rule(&conn, &id).unwrap();
        assert!(crate::store::running_rules(&conn).unwrap().is_empty());
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_run_by_hand_doesnt_skip_mail_another_rule_has_work_queued_for() {
        let conn = db();
        account(&conn, "a");
        crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
        let rule = |star: bool| Rule {
            query: "subject:s".into(),
            star,
            mark_read: !star,
            ..Default::default()
        };
        let rules = set_rules(&conn, vec![rule(true), rule(false)]).unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        queue_existing_rule(&conn, &rules[0].id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert_eq!(
            queue_existing_rule(&conn, &rules[1].id).unwrap(),
            [("a".to_string(), 1)]
        );
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued.len(), 2);
        assert!(queued.iter().any(|action| action.star));
        assert!(queued.iter().any(|action| action.mark_read));

        // The same rule again adds nothing.
        assert!(queue_existing_rule(&conn, &rules[1].id).unwrap().is_empty());
    }

    #[test]
    fn what_a_stopped_run_had_in_flight_leaves_a_later_run_alone() {
        let conn = db();
        account(&conn, "a");
        crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
        let rules = set_rules(
            &conn,
            vec![Rule {
                query: "subject:s".into(),
                copy_account: "a".into(),
                copy_folder: "Saved".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        let id = rules[0].id.clone();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        queue_existing_rule(&conn, &id).unwrap();
        let old = pending_rule_actions(&conn, "a", "INBOX").unwrap().remove(0);
        crate::store::stop_rule(&conn, &id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        queue_existing_rule(&conn, &id).unwrap();
        for _ in 0..5 {
            retry_rule_actions(&conn, "a", "INBOX", &[1], old.run, "x").unwrap();
        }
        finish_rule_actions(&conn, "a", "INBOX", &[1], old.run).unwrap();

        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued.len(), 1);
        assert_ne!(queued[0].run, old.run);
        assert_eq!(queued[0].copies, old.copies);
    }

    #[test]
    fn a_rule_runs_only_on_its_accounts() {
        let conn = db();
        let rule = |accounts: &[&str]| Rule {
            query: "subject:s".into(),
            star: true,
            accounts: accounts.iter().map(|id| id.to_string()).collect(),
            ..Default::default()
        };
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        set_rules(&conn, vec![rule(&["other"])]).unwrap();
        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );

        set_rules(&conn, vec![rule(&["a", "other"])]).unwrap();
        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 1);
    }

    #[test]
    fn a_copy_is_queued_unless_the_mail_is_already_there() {
        let conn = db();
        set_rules(
            &conn,
            vec![
                Rule {
                    query: "subject:s".into(),
                    copy_account: "b".into(),
                    copy_folder: "Saved".into(),
                    ..Default::default()
                },
                Rule {
                    query: "subject:s".into(),
                    copy_account: "a".into(),
                    copy_folder: "INBOX".into(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        // Not to where the message already is.
        let copy = RuleCopy {
            account: "b".into(),
            folder: "Saved".into(),
        };
        assert_eq!(queued[0].copies, [copy.clone()]);
        assert!(!queued[0].trash && queued[0].target.is_empty());
    }

    #[test]
    fn a_copy_to_where_the_mail_is_moving_is_left_to_the_move() {
        let conn = db();
        crate::store::ensure_folder(&conn, "a", "Saved").unwrap();
        set_rules(
            &conn,
            vec![
                Rule {
                    query: "subject:s".into(),
                    copy_account: "a".into(),
                    copy_folder: "Saved".into(),
                    ..Default::default()
                },
                Rule {
                    query: "subject:s".into(),
                    move_to: "folder".into(),
                    folder: "Saved".into(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued[0].target, "Saved");
        assert!(queued[0].copies.is_empty());
    }

    #[test]
    fn a_copy_to_trash_is_left_to_the_move_there() {
        let conn = db();
        crate::store::upsert_folders(
            &conn,
            "a",
            &[crate::imap::Folder {
                name: "Bin".into(),
                role: "trash".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        set_rules(
            &conn,
            vec![Rule {
                query: "subject:s".into(),
                move_to: "trash".into(),
                copy_account: "a".into(),
                copy_folder: "Bin".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert!(queued[0].trash && queued[0].copies.is_empty());
    }

    #[test]
    fn a_move_that_lost_to_another_rule_counts_as_a_failure() {
        let conn = db();
        let lost = crate::store::RuleAction {
            rule_ids: vec!["r".into()],
            ..Default::default()
        };
        crate::store::log_rule_lost_moves(&conn, "a", &[lost]).unwrap();
        let log = crate::store::rule_log(&conn).unwrap();
        assert_eq!(log[0].rule_id, "r");
        assert!(log[0].failed);
    }

    #[test]
    fn a_move_to_another_account_is_a_copy_and_then_trash() {
        let conn = db();
        set_rules(
            &conn,
            vec![Rule {
                query: "subject:s".into(),
                move_to: "folder".into(),
                folder: "Saved".into(),
                folder_account: "b".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert!(queued[0].trash);
        assert_eq!(queued[0].target, "");
        assert_eq!(queued[0].copies.len(), 1);
        assert_eq!(queued[0].copies[0].account, "b");
    }

    #[test]
    fn a_copy_needs_both_its_account_and_folder() {
        let conn = db();
        let refused = set_rules(
            &conn,
            vec![Rule {
                query: "subject:s".into(),
                copy_folder: "Saved".into(),
                ..Default::default()
            }],
        );
        assert!(refused.is_err());
    }

    #[test]
    fn a_star_is_queued_and_a_tag_is_not() {
        let conn = db();
        let starred = Rule {
            query: "from:x".into(),
            star: true,
            ..Default::default()
        };
        let tagged = Rule {
            query: "subject:s".into(),
            tags: vec!["todo".into()],
            ..Default::default()
        };
        set_rules(&conn, vec![starred, tagged]).unwrap();
        let mut from_x = message(1, "m@x", Some(&[]));
        from_x.from_addr = "x@x.test".into();
        upsert(&conn, "INBOX", &[from_x, message(2, "n@x", Some(&[]))]);

        apply_rules(&conn, "a", "INBOX", 1, &[1, 2]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(queued.iter().map(|a| a.uid).collect::<Vec<_>>(), [1]);
        assert_eq!(tags(&conn, "n@x"), ["todo"]);
    }

    #[test]
    fn a_move_to_a_folder_the_account_lacks_is_skipped() {
        let conn = db();
        let rule = Rule {
            query: "subject:s".into(),
            mark_read: true,
            move_to: "folder".into(),
            folder: "Nowhere".into(),
            ..Default::default()
        };
        set_rules(&conn, vec![rule]).unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        let queued = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert!(queued[0].mark_read);
        assert_eq!(queued[0].target, "");
        assert!(!queued[0].trash);
    }

    #[test]
    fn a_rule_finds_a_folder_by_the_name_people_see() {
        let conn = db();
        let wire = crate::utf7::encode("日本語");
        conn.execute(
            "INSERT INTO folders(account, name) VALUES ('a', ?1)",
            [&wire],
        )
        .unwrap();
        let rule = Rule {
            query: "subject:s".into(),
            move_to: "folder".into(),
            folder: "日本語".into(),
            ..Default::default()
        };
        set_rules(&conn, vec![rule]).unwrap();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        apply_rules(&conn, "a", "INBOX", 1, &[1]).unwrap();
        assert_eq!(
            pending_rule_actions(&conn, "a", "INBOX").unwrap()[0].target,
            wire
        );
    }

    #[test]
    fn queued_actions_retry_a_few_times_then_give_up() {
        let conn = db();
        let rule = Rule {
            query: "subject:s".into(),
            move_to: "trash".into(),
            ..Default::default()
        };
        set_rules(&conn, vec![rule]).unwrap();
        upsert(
            &conn,
            "INBOX",
            &[message(1, "m@x", Some(&[])), message(2, "n@x", Some(&[]))],
        );
        apply_rules(&conn, "a", "INBOX", 1, &[1, 2]).unwrap();

        finish_rule_actions(&conn, "a", "INBOX", &[2], 0).unwrap();
        for _ in 0..4 {
            assert_eq!(
                retry_rule_actions(&conn, "a", "INBOX", &[1], 0, "x").unwrap(),
                0
            );
        }
        assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 1);
        assert_eq!(
            retry_rule_actions(&conn, "a", "INBOX", &[1], 0, "x").unwrap(),
            1
        );
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_rename_with_its_rules_changes_both_and_leaves_no_old_tag_for_new_mail() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["old"]))]);
        let rule = Rule {
            query: "subject:s".into(),
            tags: vec!["old".into()],
            ..Default::default()
        };
        set_rules(&conn, vec![rule]).unwrap();

        let (accounts, rules) = replace_tag_and_rules(&conn, "old", Some("new")).unwrap();
        assert_eq!(accounts, ["a"]);
        assert_eq!(rules[0].tags, ["new"]);
        assert_eq!(tags(&conn, "m@x"), ["new"]);

        upsert(&conn, "INBOX", &[message(2, "n@x", Some(&[]))]);
        apply_rules(&conn, "a", "INBOX", 1, &[2]).unwrap();
        assert_eq!(tags(&conn, "n@x"), ["new"]);
        assert_eq!(tag_counts(&conn, None).unwrap().get("old"), None);
    }

    #[test]
    fn a_tag_learned_from_the_server_can_leave_right_away() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        assert!(tags(&conn, "m@x").is_empty());
    }

    #[test]
    fn removal_elsewhere_clears_the_remaining_copies() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        upsert(&conn, "Archive", &[message(9, "m@x", Some(&["todo"]))]);
        age_tags(&conn);
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        assert!(tags(&conn, "m@x").is_empty());
        assert_eq!(
            pending_tag_pushes(&conn, "a").unwrap(),
            [TagPush {
                folder: "Archive".into(),
                tag: "todo".into(),
                add: false,
                uids: vec![9],
            }]
        );
    }

    #[test]
    fn pushes_cover_every_copy_and_settle_only_when_all_agree() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        upsert(&conn, "Archive", &[message(9, "m@x", Some(&[]))]);
        change_tags(&conn, "a", &["m@x".into()], &["todo".into()], &[], true).unwrap();
        let pushes = pending_tag_pushes(&conn, "a").unwrap();
        assert_eq!(pushes.len(), 2);

        record_tag_push(&conn, "a", &pushes[0]).unwrap();
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        assert_eq!(state(&conn, "todo"), PENDING_ADD, "one copy still lacks it");

        record_tag_push(&conn, "a", &pushes[1]).unwrap();
        settle_pending_tags(&conn, "a", &HashSet::new()).unwrap();
        assert_eq!(state(&conn, "todo"), SYNCED);
        assert!(pending_tag_pushes(&conn, "a").unwrap().is_empty());
    }

    #[test]
    fn a_refused_keyword_keeps_the_tag_and_blocks_only_that_tag() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        change_tags(
            &conn,
            "a",
            &["m@x".into()],
            &["new".into(), "old".into()],
            &[],
            true,
        )
        .unwrap();
        let refused = HashSet::from([("INBOX".to_string(), "new".to_string())]);
        for push in pending_tag_pushes(&conn, "a").unwrap() {
            if push.tag == "old" {
                record_tag_push(&conn, "a", &push).unwrap();
            }
        }
        settle_pending_tags(&conn, "a", &refused).unwrap();
        assert_eq!(tags(&conn, "m@x"), ["new", "old"]);
        assert_eq!(state(&conn, "old"), SYNCED);
        assert_eq!(state(&conn, "new"), PENDING_ADD);

        change_tags(&conn, "a", &["m@x".into()], &[], &["new".into()], true).unwrap();
        settle_pending_tags(&conn, "a", &refused).unwrap();
        assert_eq!(tags(&conn, "m@x"), ["old"]);
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM message_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 2, "removal done, its tombstone kept for a while");
    }

    #[test]
    fn local_only_changes_never_wait_for_a_server() {
        let conn = db();
        upsert(&conn, "Notes", &[message(1, "m@x", None)]);
        change_tags(&conn, "a", &["m@x".into()], &["todo".into()], &[], false).unwrap();
        assert_eq!(state(&conn, "todo"), SYNCED);
        assert!(pending_tag_pushes(&conn, "a").unwrap().is_empty());
        change_tags(&conn, "a", &["m@x".into()], &[], &["todo".into()], false).unwrap();
        assert!(tags(&conn, "m@x").is_empty());
    }

    #[test]
    fn a_message_without_a_message_id_is_tagged_by_location_until_uids_reset() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(4, "", Some(&["todo"]))]);
        assert_eq!(tags(&conn, "uid:INBOX:4"), ["todo"]);
        crate::store::clear_folder_messages(&conn, "a", "INBOX").unwrap();
        assert!(tags(&conn, "uid:INBOX:4").is_empty());
    }

    #[test]
    fn a_keyword_seen_on_one_copy_is_written_to_the_others() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&[]))]);
        upsert(&conn, "Archive", &[message(9, "m@x", Some(&[]))]);
        upsert(&conn, "INBOX", &[message(1, "m@x", Some(&["todo"]))]);
        assert_eq!(
            pending_tag_pushes(&conn, "a").unwrap(),
            [TagPush {
                folder: "Archive".into(),
                tag: "todo".into(),
                add: true,
                uids: vec![9],
            }]
        );
    }

    #[test]
    fn mail_cached_before_tags_gets_one_full_read() {
        let conn = db();
        upsert(&conn, "INBOX", &[message(1, "m@x", None)]);
        assert!(needs_tag_baseline(&conn, "a", "INBOX").unwrap());
        observe_server_tags(&conn, "a", "INBOX", 1, &["todo".to_string()]).unwrap();
        assert_eq!(tags(&conn, "m@x"), ["todo"]);
        upsert(&conn, "INBOX", &[message(2, "n@x", None)]);
        mark_tag_baseline(&conn, "a", "INBOX").unwrap();
        assert!(!needs_tag_baseline(&conn, "a", "INBOX").unwrap());
    }
}
