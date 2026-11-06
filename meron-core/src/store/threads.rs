//! Thread grouping: thread cards, root subjects, and reference gaps.

use anyhow::Result;
use rusqlite::{Connection, params, params_from_iter};
use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::imap::MessageHeader;

use super::tags::{TAG_KEY_SQL, has_tags, tags_of_cards};

#[derive(Clone)]
pub struct ThreadCard {
    pub thread_key: String,
    pub original_thread_key: Option<String>,
    pub header: MessageHeader,
    pub unread_count: u32,
    /// Every message in the card's thread, read or not — the Gmail-style "3"
    /// beside the sender. Counted per card, so a subject-branched thread counts
    /// only its own branch.
    pub message_count: u32,
    pub has_draft: bool,
}

pub fn group_thread_cards(messages: Vec<MessageHeader>, default_folder: &str) -> Vec<ThreadCard> {
    group_thread_cards_with_drafts(messages, default_folder, &HashSet::new())
}

pub fn group_thread_cards_with_drafts(
    messages: Vec<MessageHeader>,
    default_folder: &str,
    draft_thread_keys: &HashSet<String>,
) -> Vec<ThreadCard> {
    use std::collections::HashMap;

    struct RootSubject {
        /// The card's title: the oldest message's subject verbatim. Reply
        /// prefixes are stripped for grouping, never for display — when the
        /// thread's own first message is not in the mailbox the oldest one here
        /// is itself a reply, and dropping its `Re:` claims a thread start that
        /// this folder does not hold.
        display: String,
        group: String,
    }

    // What one card collects. A real thread key is the same thread wherever its
    // copies sit, but a `uid:` key names a message by (folder, uid) — the fallback
    // for one with no threading headers — so on a page that spans folders (search
    // covers the mailbox plus Sent) two unrelated messages that happen to share a
    // UID carry the same key. Qualifying those with the folder keeps them apart.
    let group_key = |message: &MessageHeader, key: &str| -> String {
        if !key.starts_with("uid:") {
            return key.to_string();
        }
        let folder = if message.folder.is_empty() {
            default_folder
        } else {
            message.folder.as_str()
        };
        format!("{folder}\u{1}{key}")
    };

    let mut candidates: HashMap<String, Vec<RootCandidate>> = HashMap::new();
    for message in &messages {
        if message.uid == 0 {
            continue;
        }
        let thread_key = group_key(message, &effective_thread_key(message));
        let folder = if message.folder.is_empty() {
            default_folder
        } else {
            message.folder.as_str()
        };
        candidates
            .entry(thread_key)
            .or_default()
            .push(RootCandidate {
                folder: folder.to_string(),
                uid: message.uid,
                date: message.date,
                subject: message.subject.clone(),
            });
    }
    let roots: HashMap<String, RootSubject> = candidates
        .into_iter()
        .filter_map(|(key, candidates)| {
            let root = pick_root(candidates.clone(), default_folder)?;
            let display = thread_title(&root, &candidates, default_folder);
            // The title's branch, not the oldest message's: a thread start not
            // yet decrypted would otherwise make every reply a "changed subject".
            let group = thread_grouping_subject(&display);
            Some((key, RootSubject { display, group }))
        })
        .collect();

    let mut groups: HashMap<String, ThreadCard> = HashMap::new();
    let mut order = Vec::new();
    // One message can reach a folder-spanning page twice: search reads the
    // mailbox plus Sent, and a self-sent message is cached in both. The cached
    // tally this page's is floored against folds those copies by Message-ID (see
    // `card_message_counts`), so fold them here too, or the card claims a message
    // more than the mailbox's own list gives it. A row with no Message-ID cannot
    // be matched to a copy and counts on its own. The value is whether a copy has
    // already been counted as unread.
    let mut folded: HashMap<(String, String), bool> = HashMap::new();
    for message in messages {
        if message.uid == 0 {
            continue;
        }
        let base_key = effective_thread_key(&message);
        let branch = should_branch_thread_by_subject(&base_key);
        // A message not yet decrypted joins its thread's titled branch rather
        // than forming a card under the placeholder.
        let titled_group = (branch && is_encrypted_placeholder_subject(&message.subject))
            .then(|| roots.get(&group_key(&message, &base_key)).map(|root| root.group.clone()))
            .flatten();
        let group_subject = if let Some(group) = &titled_group {
            group.clone()
        } else if branch {
            thread_grouping_subject(&message.subject)
        } else {
            String::new()
        };
        let compound_key = match &titled_group {
            Some(group) => branch_compound_key(&base_key, group),
            None => card_thread_key(&message),
        };
        let slot = group_key(&message, &compound_key);
        let fold_key =
            (!message.message_id.is_empty()).then(|| (slot.clone(), message.message_id.clone()));
        let first_copy = fold_key
            .as_ref()
            .is_none_or(|key| !folded.contains_key(key));
        // Copies can disagree on the read flag — the mailbox's copy unread, the
        // Sent one not — and the message is unread if any of them is, whichever
        // order the page happens to put them in.
        let counts_unread = !message.seen
            && fold_key
                .as_ref()
                .is_none_or(|key| !folded.get(key).copied().unwrap_or(false));
        if let Some(key) = fold_key {
            *folded.entry(key).or_insert(false) |= !message.seen;
        }
        let card = groups.entry(slot.clone()).or_insert_with(|| {
            order.push(slot);
            let root = roots.get(&group_key(&message, &base_key));
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
                message_count: 0,
                has_draft: draft_thread_keys.contains(&base_key),
            }
        });
        // A page can span folders — search covers the mailbox plus Sent — and the
        // card above took its folder, and so its thread id, from whichever copy
        // came first (newest). Sending a reply makes the Sent copy the newest one,
        // which would mint a second id for the same thread: the list shows the
        // card twice, and every id-keyed update (the draft badge a post-send
        // discard clears, unread reconciliation) misses the row it means. The
        // searched mailbox wins whenever it holds a copy of the thread, so the id
        // is the one that mailbox's own list would give. Not for `uid:` keys:
        // those name a single message by (folder, uid), so the folder cannot move
        // without pointing at a different message.
        if !base_key.starts_with("uid:")
            && !card.header.folder.eq_ignore_ascii_case(default_folder)
            && message.folder.eq_ignore_ascii_case(default_folder)
        {
            card.header.folder = message.folder.clone();
        }
        if first_copy {
            card.message_count += 1;
        }
        if counts_unread {
            card.unread_count += 1;
        }
        if !message.seen {
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

/// Which of `card_keys` the cache holds a copy of in `folder`, as card keys.
///
/// A folder-spanning page (search reads the mailbox plus Sent) sees only its own
/// slice, so the copy that decides a card's identity — see
/// [`group_thread_cards_with_drafts`] — may have paged out below it. The cache
/// has the whole mailbox, so it answers for the ones the page cannot.
///
/// `uid:` keys are folder-scoped by construction and never move, so they are not
/// asked about.
pub fn card_keys_in_folder(
    conn: &Connection,
    account: &str,
    folder: &str,
    card_keys: &[String],
) -> Result<HashSet<String>> {
    let mut roots: Vec<String> = card_keys
        .iter()
        .map(|key| split_thread_key(key).0)
        .filter(|root| !root.starts_with("uid:"))
        .collect();
    roots.sort();
    roots.dedup();

    let mut present = HashSet::new();
    for chunk in roots.chunks(100) {
        let placeholders = (3..3 + chunk.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = conn.prepare(&format!(
            "SELECT thread_key, subject FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid <> 0
               AND thread_key IN ({placeholders})"
        ))?;
        let mut args: Vec<&str> = vec![account, folder];
        args.extend(chunk.iter().map(String::as_str));
        let rows = stmt.query_map(params_from_iter(args), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (root, subject) = row?;
            present.insert(if should_branch_thread_by_subject(&root) {
                branch_compound_key(&root, &thread_grouping_subject(&subject))
            } else {
                root
            });
        }
    }
    Ok(present)
}

/// Total cached messages behind each of `card_keys`, keyed by card key.
///
/// The header page a list request read is not the thread: it is filtered
/// (unread-only, starred-only, search hits) and cursor-paged by *message*, so
/// counting the headers handed to [`group_thread_cards_with_drafts`] would
/// report "1" for a long thread with a single unread message and would split a
/// thread that straddles a page boundary. The cache holds the whole thread, so
/// the count is taken from there and the page tally is only a floor for
/// messages the cache has not seen yet.
///
/// The scope matches what opening the row shows, so the badge never contradicts
/// the reader (see `thread_read`): real threading keys count across every folder
/// in the account, because a received message and the user's own Sent reply
/// belong to one thread; synthetic `uid:N` keys stay inside `folder`, because
/// UIDs are folder-scoped and spanning folders would pull in an unrelated
/// message that happens to share the UID.
///
/// Cross-folder rows are deduplicated by Message-ID exactly as
/// [`get_thread_headers_all_folders`] does, so a self-sent message cached in
/// both Sent and Inbox counts once — the reader renders it as one bubble.
///
/// Counting re-derives each row's card key rather than grouping on the raw
/// thread key, so a subject-branched thread counts only the branch its card
/// stands for — the same split [`card_thread_key`] makes.
pub fn card_message_counts(
    conn: &Connection,
    account: &str,
    folder: &str,
    card_keys: &[String],
) -> Result<std::collections::HashMap<String, u32>> {
    use std::collections::HashMap;

    let mut roots: Vec<String> = card_keys
        .iter()
        .map(|key| split_thread_key(key).0)
        .collect();
    roots.sort();
    roots.dedup();
    let (uid_roots, threaded_roots): (Vec<String>, Vec<String>) =
        roots.into_iter().partition(|root| root.starts_with("uid:"));

    let mut counts: HashMap<String, u32> = HashMap::new();
    count_card_rows(conn, account, Some(folder), &uid_roots, &mut counts)?;
    count_card_rows(conn, account, None, &threaded_roots, &mut counts)?;
    Ok(counts)
}

/// The title each of `card_keys` should show: the subject of the oldest cached
/// message of its thread, verbatim.
///
/// The page a card was grouped from is a filtered, cursor-paged slice — an
/// unread view can hold a reply while the seen thread opener sits in the same
/// folder just outside the page — so a title read off that slice can call a
/// reply the thread's start. The cache has the whole mailbox and settles it, the
/// same way [`card_message_counts`] settles the tally.
///
/// Keyed by root thread key, not card key: every subject branch of a thread
/// carries the root thread's title.
pub fn card_root_subjects(
    conn: &Connection,
    account: &str,
    folder: &str,
    card_keys: &[String],
) -> Result<std::collections::HashMap<String, String>> {
    use std::collections::HashMap;

    let mut roots: Vec<String> = card_keys
        .iter()
        .map(|key| split_thread_key(key).0)
        .collect();
    roots.sort();
    roots.dedup();
    let (uid_roots, threaded_roots): (Vec<String>, Vec<String>) =
        roots.into_iter().partition(|root| root.starts_with("uid:"));

    // `uid:` roots name a message by (folder, uid) and never move; a real thread
    // key is the same thread wherever its copies sit, so its oldest message may
    // be cached in another folder.
    let mut candidates: HashMap<String, Vec<RootCandidate>> = HashMap::new();
    collect_root_candidates(conn, account, Some(folder), &uid_roots, &mut candidates)?;
    collect_root_candidates(conn, account, None, &threaded_roots, &mut candidates)?;

    Ok(candidates
        .into_iter()
        .filter_map(|(root, candidates)| {
            let oldest = pick_root(candidates.clone(), folder)?;
            let subject = thread_title(&oldest, &candidates, folder);
            Some((root, subject))
        })
        .collect())
}

/// Gather the cached rows of `roots` as root candidates, keyed by root thread
/// key. `folder` scopes the query to a single mailbox; `None` spans the account.
pub(super) fn collect_root_candidates(
    conn: &Connection,
    account: &str,
    folder: Option<&str>,
    roots: &[String],
    candidates: &mut std::collections::HashMap<String, Vec<RootCandidate>>,
) -> Result<()> {
    // SQLite caps bound parameters per statement; chunk so an unpaginated caller
    // cannot overrun it (see `count_card_rows`).
    for chunk in roots.chunks(100) {
        let first = if folder.is_some() { 3 } else { 2 };
        let placeholders = (first..first + chunk.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let folder_clause = if folder.is_some() {
            "folder = ?2 AND "
        } else {
            ""
        };
        let mut stmt = conn.prepare(&format!(
            "SELECT COALESCE(NULLIF(thread_key, ''), 'uid:' || uid), folder, uid, date, subject
             FROM messages
             WHERE account = ?1 AND {folder_clause}uid <> 0
               AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) IN ({placeholders})"
        ))?;
        let mut args: Vec<&str> = vec![account];
        args.extend(folder);
        args.extend(chunk.iter().map(String::as_str));
        let rows = stmt.query_map(params_from_iter(args), |row| {
            Ok((
                row.get::<_, String>(0)?,
                RootCandidate {
                    folder: row.get(1)?,
                    uid: row.get(2)?,
                    date: row.get(3)?,
                    subject: row.get(4)?,
                },
            ))
        })?;
        for row in rows {
            let (root, candidate) = row?;
            candidates.entry(root).or_default().push(candidate);
        }
    }
    Ok(())
}

/// One attachment a thread card names, as the `files` column stores it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CardFile {
    pub filename: String,
    pub mime: String,
}

/// The real attachments behind each of `card_keys`, in thread order, keyed by
/// card key; cards with none are absent. Scoped and deduplicated exactly like
/// [`card_message_counts`], so the chips speak for the same messages the reader
/// opens. Header sync supplies names from BODYSTRUCTURE; a cached body refines
/// them using the HTML references to exclude inline images.
pub fn card_attachments(
    conn: &Connection,
    account: &str,
    folder: &str,
    card_keys: &[String],
) -> Result<std::collections::HashMap<String, Vec<CardFile>>> {
    use std::collections::HashMap;

    let mut roots: Vec<String> = card_keys
        .iter()
        .map(|key| split_thread_key(key).0)
        .collect();
    roots.sort();
    roots.dedup();
    let (uid_roots, threaded_roots): (Vec<String>, Vec<String>) =
        roots.into_iter().partition(|root| root.starts_with("uid:"));

    let mut rows: HashMap<String, Vec<(i64, Vec<CardFile>)>> = HashMap::new();
    let mut collect = |key: String, row: CardRow| {
        if !row.files.is_empty() {
            rows.entry(key).or_default().push((row.date, row.files));
        }
    };
    for_each_card_row(conn, account, Some(folder), &uid_roots, &mut collect)?;
    for_each_card_row(conn, account, None, &threaded_roots, &mut collect)?;

    Ok(rows
        .into_iter()
        .map(|(key, mut rows)| {
            rows.sort_by_key(|(date, _)| *date);
            (key, rows.into_iter().flat_map(|(_, files)| files).collect())
        })
        .collect())
}

/// Tally the cached rows of `roots` into `counts`, one card key at a time.
/// `folder` scopes the query to a single mailbox; `None` spans the account.
pub(super) fn count_card_rows(
    conn: &Connection,
    account: &str,
    folder: Option<&str>,
    roots: &[String],
    counts: &mut std::collections::HashMap<String, u32>,
) -> Result<()> {
    for_each_card_row(conn, account, folder, roots, |key, _| {
        *counts.entry(key).or_insert(0) += 1;
    })
}

/// The tags behind each of `card_keys`, sorted, keyed by card key. Scoped and
/// deduplicated like [`card_message_counts`], so a card's tags are those of the
/// messages it counts.
pub fn card_tags(
    conn: &Connection,
    account: &str,
    folder: &str,
    card_keys: &[String],
) -> Result<std::collections::HashMap<String, Vec<String>>> {
    use std::collections::HashMap;

    if !has_tags(conn, account)? {
        return Ok(HashMap::new());
    }
    let mut roots: Vec<String> = card_keys
        .iter()
        .map(|key| split_thread_key(key).0)
        .collect();
    roots.sort();
    roots.dedup();
    let (uid_roots, threaded_roots): (Vec<String>, Vec<String>) =
        roots.into_iter().partition(|root| root.starts_with("uid:"));

    let mut keys_by_card: HashMap<String, Vec<String>> = HashMap::new();
    let mut collect =
        |card: String, row: CardRow| keys_by_card.entry(card).or_default().push(row.tag_key);
    for_each_card_row(conn, account, Some(folder), &uid_roots, &mut collect)?;
    for_each_card_row(conn, account, None, &threaded_roots, &mut collect)?;
    tags_of_cards(conn, account, keys_by_card)
}

/// One sender of a thread card, as the list row names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardSender {
    /// Short label: the display name's first word, else the address local part.
    /// Empty when `me` is set — the UI says "me" in its own language.
    pub name: String,
    pub me: bool,
    /// Send time of this sender's newest message in the thread, epoch seconds.
    pub last_date: i64,
}

/// The distinct senders behind each of `card_keys`, oldest first, keyed by card
/// key — the Gmail-style "me, Dana, Bob" in place of the newest sender alone.
///
/// Scoped and deduplicated exactly like [`card_message_counts`], so the names
/// and the count beside them describe the same messages. Senders are told apart
/// by address, and every one of the account's own addresses (or anything filed
/// in Sent, see [`super::is_outgoing`]) collapses into a single `me`.
pub fn card_senders(
    conn: &Connection,
    account: &str,
    folder: &str,
    card_keys: &[String],
) -> Result<std::collections::HashMap<String, Vec<CardSender>>> {
    use std::collections::HashMap;

    let mut roots: Vec<String> = card_keys
        .iter()
        .map(|key| split_thread_key(key).0)
        .collect();
    roots.sort();
    roots.dedup();
    let (uid_roots, threaded_roots): (Vec<String>, Vec<String>) =
        roots.into_iter().partition(|root| root.starts_with("uid:"));

    let mine = super::self_addrs(conn, account);
    let mut rows: HashMap<String, Vec<CardRow>> = HashMap::new();
    let mut collect = |key: String, row: CardRow| rows.entry(key).or_default().push(row);
    for_each_card_row(conn, account, Some(folder), &uid_roots, &mut collect)?;
    for_each_card_row(conn, account, None, &threaded_roots, &mut collect)?;

    Ok(rows
        .into_iter()
        .map(|(key, mut rows)| {
            rows.sort_by_key(|row| row.date);
            let me_of = |row: &CardRow| {
                let addr = row.from_addr.trim().to_lowercase();
                let me =
                    super::account_message_is_outgoing(account, &mine, &row.folder, &addr, false);
                (addr, me)
            };
            // Address alone is not a person: GitHub, Jira and mailing lists send
            // every participant's mail from one address under that participant's
            // display name, so a named sender keys on address and name. Collect
            // each address's distinct names (with one raw spelling to show) first.
            let mut named: HashMap<String, BTreeMap<String, String>> = HashMap::new();
            for row in &rows {
                let (addr, me) = me_of(row);
                if let Some(name_key) = sender_name_key(&row.from_name, &addr)
                    && !me
                {
                    named
                        .entry(addr)
                        .or_default()
                        .entry(name_key)
                        .or_insert_with(|| row.from_name.clone());
                }
            }
            // Identity → position in `senders`, so a repeat voice only moves
            // that sender's `last_date` forward.
            let mut seen: HashMap<String, usize> = HashMap::new();
            let mut senders: Vec<CardSender> = Vec::new();
            for row in rows {
                let (addr, me) = me_of(&row);
                if !me && addr.is_empty() {
                    continue;
                }
                // A message with no usable name joins its address's named sender
                // when there is exactly one — `Alice Smith <a@x>` and `<a@x>` are
                // one person. With none it keys on the address; with several (a
                // shared notification address) it cannot be told which, so it
                // stays apart rather than being credited to the wrong one.
                let (identity, display) = if me {
                    (String::new(), String::new())
                } else {
                    let names = named.get(&addr);
                    match sender_name_key(&row.from_name, &addr) {
                        Some(name_key) => (format!("{addr}\u{1}{name_key}"), row.from_name.clone()),
                        None => match names.filter(|names| names.len() == 1) {
                            Some(names) => {
                                let (name_key, raw) = names.iter().next().unwrap();
                                (format!("{addr}\u{1}{name_key}"), raw.clone())
                            }
                            None => (addr.clone(), String::new()),
                        },
                    }
                };
                if let Some(&index) = seen.get(&identity) {
                    senders[index].last_date = row.date;
                    continue;
                }
                let name = if me {
                    String::new()
                } else {
                    short_sender_name(&display, &addr)
                };
                seen.insert(identity, senders.len());
                senders.push(CardSender {
                    name,
                    me,
                    last_date: row.date,
                });
            }
            (key, senders)
        })
        .collect())
}

/// The comparison key for a sender's display name, or None when the name says
/// nothing over the address (missing, or one that repeats the address).
fn sender_name_key(name: &str, addr_lower: &str) -> Option<String> {
    let name = name.trim().trim_matches('"').trim().to_lowercase();
    (!name.is_empty() && !name.contains(addr_lower)).then_some(name)
}

/// "Dana Evans" → "Dana"; a missing or junk name (one that repeats the
/// address) falls back to the address local part.
fn short_sender_name(name: &str, addr_lower: &str) -> String {
    let name = name.trim().trim_matches('"').trim();
    if name.is_empty() || name.to_lowercase().contains(addr_lower) {
        return addr_lower.split('@').next().unwrap_or_default().to_string();
    }
    name.split_whitespace()
        .next()
        .unwrap_or(name)
        .trim_end_matches(',')
        .to_string()
}

/// The cached fields [`for_each_card_row`] hands its visitor.
pub(super) struct CardRow {
    folder: String,
    from_name: String,
    from_addr: String,
    date: i64,
    files: Vec<CardFile>,
    files_authoritative: bool,
    tag_key: String,
}

/// Visit every cached message behind `roots` once, with the card key it belongs
/// to. `folder` scopes the scan (for `uid:` roots); Message-ID copies across
/// folders are folded, so a self-sent message cached in Inbox and Sent is one
/// visit.
///
/// The first copy supplies identity. Attachment metadata prefers a cached body
/// over header-only copies, even when its refined list is empty.
fn for_each_card_row(
    conn: &Connection,
    account: &str,
    folder: Option<&str>,
    roots: &[String],
    mut visit: impl FnMut(String, CardRow),
) -> Result<()> {
    use std::collections::{HashMap, hash_map::Entry};

    // Message-ID duplicates are folded in Rust rather than with the correlated
    // NOT EXISTS that reading a single thread uses: with a page of keys and no
    // index on thread_key, that subquery would re-scan the account's messages
    // once per row, where one pass plus a set of seen ids is linear.
    let mut seen_ids: HashMap<(String, String), usize> = HashMap::new();
    let mut visits: Vec<(String, CardRow)> = Vec::new();
    // SQLite caps bound parameters per statement; a page of cards stays well
    // under it, but chunk anyway so an unpaginated caller cannot overrun it.
    for chunk in roots.chunks(100) {
        let first = if folder.is_some() { 3 } else { 2 };
        let placeholders = (first..first + chunk.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let folder_clause = if folder.is_some() {
            "folder = ?2 AND "
        } else {
            ""
        };
        let mut stmt = conn.prepare(&format!(
            "SELECT COALESCE(NULLIF(thread_key, ''), 'uid:' || uid), subject,
                    COALESCE(json_extract(json, '$.message_id'), ''),
                    folder, COALESCE(from_name, ''), COALESCE(from_addr, ''), date, files, body IS NOT NULL,
                    {TAG_KEY_SQL}
             FROM messages
             WHERE account = ?1 AND {folder_clause}uid <> 0
               AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) IN ({placeholders})"
        ))?;
        let mut args: Vec<&str> = vec![account];
        args.extend(folder);
        args.extend(chunk.iter().map(String::as_str));
        let rows = stmt.query_map(params_from_iter(args), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                CardRow {
                    folder: row.get(3)?,
                    from_name: row.get(4)?,
                    from_addr: row.get(5)?,
                    date: row.get(6)?,
                    files_authoritative: row.get(8)?,
                    tag_key: row.get(9)?,
                    files: row
                        .get::<_, Option<String>>(7)?
                        .and_then(|files| serde_json::from_str(&files).ok())
                        .unwrap_or_default(),
                },
            ))
        })?;
        for row in rows {
            let (root, subject, message_id, card_row) = row?;
            let key = if should_branch_thread_by_subject(&root) {
                branch_compound_key(&root, &thread_grouping_subject(&subject))
            } else {
                root
            };
            // A row with no Message-ID cannot be matched to a copy, so it counts
            // on its own — same call the thread reader makes.
            if !message_id.is_empty() {
                match seen_ids.entry((key.clone(), message_id)) {
                    Entry::Occupied(kept) => {
                        let kept = &mut visits[*kept.get()].1;
                        if (card_row.files_authoritative && !kept.files_authoritative)
                            || (!kept.files_authoritative && kept.files.is_empty())
                        {
                            kept.files = card_row.files;
                            kept.files_authoritative = card_row.files_authoritative;
                        }
                        continue;
                    }
                    Entry::Vacant(slot) => {
                        slot.insert(visits.len());
                    }
                }
            }
            visits.push((key, card_row));
        }
    }
    for (key, card_row) in visits {
        visit(key, card_row);
    }
    Ok(())
}

/// A candidate for a thread's opening message: what [`pick_root`] compares.
#[derive(Clone)]
pub(super) struct RootCandidate {
    folder: String,
    uid: u32,
    /// Send time as Unix epoch seconds, 0 when the row carries no usable Date.
    date: i64,
    subject: String,
}

/// The subject an encrypted message carries outside its encryption (RFC 9788
/// protected headers: `[...]`, Thunderbird `...`) until its body is decrypted.
fn is_encrypted_subject_placeholder(subject: &str) -> bool {
    matches!(subject.trim(), "..." | "[...]")
}

/// The title of a thread whose oldest message is `root`: its subject, unless
/// that is still an encrypted message's placeholder, in which case the oldest
/// message that does know its subject (decrypted, or not encrypted) titles it.
/// Only the title: grouping keeps keying on the oldest message.
fn thread_title(root: &RootCandidate, candidates: &[RootCandidate], read_folder: &str) -> String {
    if is_encrypted_subject_placeholder(&root.subject) {
        let known: Vec<RootCandidate> = candidates
            .iter()
            .filter(|candidate| !is_encrypted_subject_placeholder(&candidate.subject))
            .cloned()
            .collect();
        if let Some(titled) = pick_root(known, read_folder) {
            return titled.subject.trim().to_string();
        }
    }
    root.subject.trim().to_string()
}

/// Choose the oldest message of one thread from `candidates`. `read_folder` is
/// the mailbox the card belongs to.
///
/// UIDs ascend with arrival inside a folder, so there the lowest UID is the
/// oldest message — authoritative even for a row whose Date header is missing.
/// Across folders UIDs are unrelated: on a page that merges the mailbox with
/// Sent (search, and any thread the user replied to) a Sent reply's UID 1 is not
/// older than an Inbox root's UID 100. So each folder's own pick is made by UID
/// first, and only those are compared, by send time alone — UID says nothing
/// here and must not break the tie, or the Sent reply wins back whenever the two
/// share a timestamp or neither carries one.
///
/// What breaks a cross-folder tie is the mailbox being read: it is the folder
/// the card sits in, so its copy is the one that should title it. The folder
/// name settles what is left, so the pick never follows input order.
pub(super) fn pick_root(
    candidates: Vec<RootCandidate>,
    read_folder: &str,
) -> Option<RootCandidate> {
    let mut per_folder: BTreeMap<String, RootCandidate> = BTreeMap::new();
    for candidate in candidates {
        match per_folder.get(&candidate.folder) {
            Some(best) if best.uid <= candidate.uid => {}
            _ => {
                per_folder.insert(candidate.folder.clone(), candidate);
            }
        }
    }
    per_folder.into_values().min_by(|left, right| {
        fn rank<'a>(candidate: &'a RootCandidate, read_folder: &str) -> (i64, bool, &'a str) {
            (
                // An undated pick loses to a dated one.
                if candidate.date == 0 {
                    i64::MAX
                } else {
                    candidate.date
                },
                !candidate.folder.eq_ignore_ascii_case(read_folder),
                candidate.folder.as_str(),
            )
        }
        rank(left, read_folder).cmp(&rank(right, read_folder))
    })
}

pub(super) fn effective_thread_key(message: &MessageHeader) -> String {
    if message.thread_key.is_empty() {
        format!("uid:{}", message.uid)
    } else {
        message.thread_key.clone()
    }
}

/// Whether a grouping subject is nothing but an encrypted message's
/// placeholder: `[...]` strips to nothing (a bracket tag), `...` stays.
fn is_placeholder_group(group: &str) -> bool {
    group.is_empty() || group == "..."
}

/// Encrypted replies whose protected subject is only the placeholder (written
/// by a client that had not decrypted the thread: `Re: [...]`) take the
/// thread's real subject as `Re: <subject>`, from the oldest message that has
/// one. Left as they were, they group under an empty subject and form a
/// branch of their own, out of the thread. Only encrypted rows are touched: a
/// plain mail really titled `[...]` keeps it. `thread_key` limits the repair to
/// one thread. Returns how many rows changed.
pub fn repair_placeholder_reply_subjects(
    conn: &Connection,
    account: &str,
    thread_key: Option<&str>,
) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT folder, msg_id, thread_key, subject FROM messages
         WHERE account = ?1 AND (?2 IS NULL OR thread_key = ?2)
           AND thread_key IS NOT NULL AND thread_key <> ''
           AND json_extract(json, '$.crypto.encrypted') = 1",
    )?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map(params![account, thread_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut real: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    let mut changed = 0;
    for (folder, msg_id, key, subject) in rows {
        if !should_branch_thread_by_subject(&key) || !is_placeholder_group(&thread_grouping_subject(&subject)) {
            continue;
        }
        if !real.contains_key(&key) {
            let mut oldest = conn.prepare(
                "SELECT subject FROM messages WHERE account = ?1 AND thread_key = ?2
                 ORDER BY date = 0, date, uid",
            )?;
            let found = oldest
                .query_map(params![account, key], |r| r.get::<_, String>(0))?
                .filter_map(|s| s.ok())
                .map(|s| thread_grouping_subject(&s))
                .find(|group| !is_placeholder_group(group));
            real.insert(key.clone(), found);
        }
        if let Some(Some(group)) = real.get(&key) {
            changed += conn.execute(
                "UPDATE messages SET subject = ?4 WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
                params![account, folder, msg_id, format!("Re: {group}")],
            )?;
        }
    }
    Ok(changed)
}

/// [`repair_placeholder_reply_subjects`] for every account.
pub fn repair_placeholder_reply_subjects_everywhere(conn: &Connection) -> Result<usize> {
    let accounts: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT account FROM messages
             WHERE json_extract(json, '$.crypto.encrypted') = 1",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut changed = 0;
    for account in accounts {
        changed += repair_placeholder_reply_subjects(conn, &account, None)?;
    }
    Ok(changed)
}

/// Whether `subject` is only an encrypted message's protected-headers
/// placeholder (`[...]`, Thunderbird `...`), reply prefixes aside: a message
/// not yet decrypted. An empty subject is not one.
pub fn is_encrypted_placeholder_subject(subject: &str) -> bool {
    let group = thread_grouping_subject(subject);
    group == "..." || (group.is_empty() && subject.contains("[...]"))
}

/// Whether a message with `subject` belongs to the subject branch `filter`. A
/// message not yet decrypted hasn't shown its subject, so it belongs to every
/// branch of its thread: on Gmail our own Sent copies stay like that until
/// fetched, and a branch filter dropped them all from the conversation.
pub fn subject_in_branch(subject: &str, filter: &str) -> bool {
    is_encrypted_placeholder_subject(subject) || thread_grouping_subject(subject) == filter
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

pub(super) fn strip_reply_prefix(subject: &str) -> Option<&str> {
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

pub(super) fn strip_reply_count(rest: &str) -> Option<&str> {
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

pub(super) fn strip_leading_bracket_tag(subject: &str) -> Option<&str> {
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
/// `Message-ID`, the unread copy if any, otherwise the lowest-`id` row. Rows
/// without a `Message-ID` (e.g. drafts) are never collapsed — a NULL
/// `Message-ID` never equates in SQL.
pub fn get_thread_headers_all_folders(
    conn: &Connection,
    account: &str,
    thread_key: &str,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT m.uid, m.folder, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred, m.thread_key,
                json_extract(m.json, '$.in_reply_to')
         FROM messages m
         WHERE m.account = ?1 AND m.uid <> 0
           AND COALESCE(NULLIF(m.thread_key, ''), 'uid:' || m.uid) = ?2
           AND NOT EXISTS (
             SELECT 1 FROM messages dup
             WHERE dup.account = m.account
               AND COALESCE(NULLIF(dup.thread_key, ''), 'uid:' || dup.uid) = ?2
               AND COALESCE(json_extract(m.json, '$.message_id'), '') <> ''
               AND json_extract(dup.json, '$.message_id') = json_extract(m.json, '$.message_id')
               AND (dup.seen < m.seen OR (dup.seen = m.seen AND dup.id < m.id))
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
            in_reply_to: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
            ..Default::default()
        })
    })?;
    // `date` is now an epoch integer, so the SQL ORDER BY sorts chronologically
    // (it could not when date was an RFC 2822 string). Unknown dates (0) sort first.
    let headers = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(headers)
}
