use crate::imap::MessageHeader;
use crate::store;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn canon_folder(folder: &str) -> String {
    if folder.eq_ignore_ascii_case("inbox") {
        "INBOX".to_string()
    } else {
        folder.to_string()
    }
}

/// How many of a batch's arrivals are described individually in a
/// `mail.newMessages` detail. Clients post one notification per entry, and a
/// notification shade can't usefully show more than a handful; `count` still
/// reports the true size of the batch.
pub const NEW_MESSAGES_DETAIL_MAX: usize = 8;

/// The `mail.newMessages` payload for a batch of arrivals, newest first.
///
/// `from`/`subject`/`threadKey` describe the newest message and are what a
/// single-notification client (desktop) reads; `messages` carries the same
/// fields per arrival plus a body snippet, for clients that post one
/// notification per mail. Snippets come from cached bodies, so a message whose
/// body hasn't been fetched yet contributes an empty `preview` rather than
/// holding up the event.
pub fn new_messages_detail(
    conn: &Connection,
    account_id: &str,
    account_name: &str,
    muted: bool,
    headers: &[MessageHeader],
) -> Option<Value> {
    let latest = headers.first()?;
    let messages: Vec<Value> = headers
        .iter()
        .take(NEW_MESSAGES_DETAIL_MAX)
        .map(|header| {
            json!({
                "uid": header.uid,
                "from": display_from(header),
                "subject": header.subject,
                "preview": store::cached_body_preview(conn, account_id, "INBOX", header.uid)
                    .unwrap_or_default(),
                "threadKey": store::card_thread_key(header),
                "date": header.date,
            })
        })
        .collect();
    Some(json!({
        "account": account_id,
        "accountName": account_name,
        "folder": "inbox",
        "count": headers.len(),
        "muted": muted,
        "from": display_from(latest),
        "subject": latest.subject,
        "preview": messages
            .first()
            .and_then(|message| message.get("preview"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
        // Branch-aware card key so a notification tap opens the exact list card
        // the grouping produced.
        "threadKey": store::card_thread_key(latest),
        "messages": messages,
    }))
}

/// Sender name for notifications, falling back to the address when the envelope
/// carries no display name.
pub fn display_from(header: &MessageHeader) -> String {
    if !header.from_name.trim().is_empty() {
        header.from_name.trim().to_string()
    } else {
        header.from_addr.trim().to_string()
    }
}

/// Gate for deleting a folder on the server. Both clients call this so the rule
/// lives in one place: special-use mailboxes are off limits (the app routes mail
/// through Inbox/Sent/Drafts/Trash/Junk/Archive, and the server keeps no copy
/// once they are gone), and that applies to anything nested under the folder
/// too, since deleting it takes the whole subtree.
pub fn check_folder_deletable(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<(), String> {
    if !store::folder_exists(conn, account, folder).map_err(|err| err.to_string())? {
        return Err(format!("{folder} is not in the synchronized folder list"));
    }
    let role = store::folder_role(conn, account, folder).map_err(|err| err.to_string())?;
    if role != "folder" {
        return Err(format!(
            "{folder} is a special folder and cannot be deleted"
        ));
    }
    for child in store::child_folders(conn, account, folder).map_err(|err| err.to_string())? {
        let role = store::folder_role(conn, account, &child).map_err(|err| err.to_string())?;
        if role != "folder" {
            return Err(format!("{child} is a special folder and cannot be deleted"));
        }
    }
    Ok(())
}

/// The folders a delete of `folder` removes, deepest first: IMAP DELETE fails on
/// a mailbox that still has inferiors, so the subtree has to come off leaf-end
/// first, with `folder` itself last. Sorting by descending name length puts every
/// descendant ahead of its own parent, whatever the server's delimiter is.
pub fn folder_delete_targets(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Vec<String>, String> {
    let mut targets = store::child_folders(conn, account, folder).map_err(|err| err.to_string())?;
    targets.sort_by_key(|name| std::cmp::Reverse(name.len()));
    targets.push(folder.to_string());
    Ok(targets)
}

pub fn format_thread_id(account_id: &str, folder: &str, thread_key: &str) -> String {
    if let Some(uid) = thread_key.strip_prefix("uid:") {
        return format!("{account_id}#{}#{uid}", canon_folder(folder));
    }
    let encoded = URL_SAFE_NO_PAD.encode(thread_key.as_bytes());
    format!("{account_id}#{}#t.{encoded}", canon_folder(folder))
}

pub fn thread_cards_json(
    conn: &Connection,
    account_id: &str,
    folder_id: &str,
    messages: Vec<MessageHeader>,
    draft_thread_keys: &HashSet<String>,
) -> anyhow::Result<Vec<Value>> {
    Ok(
        thread_cards_json_keyed(conn, account_id, folder_id, messages, draft_thread_keys)?
            .into_iter()
            .map(|(_, card)| card)
            .collect(),
    )
}

/// [`thread_cards_json`] with each card paired to its thread key. The key is
/// folder-independent, so callers that span folders (the starred view) can tell
/// two copies of one thread apart from two different threads.
fn thread_cards_json_keyed(
    conn: &Connection,
    account_id: &str,
    folder_id: &str,
    messages: Vec<MessageHeader>,
    draft_thread_keys: &HashSet<String>,
) -> anyhow::Result<Vec<(String, Value)>> {
    let mut cards = store::group_thread_cards_with_drafts(messages, folder_id, draft_thread_keys);

    // Grouping pins a card to the mailbox being read when the page holds a copy
    // there, so a Sent reply landing on top does not mint a second id for a
    // thread the list already shows (see `group_thread_cards_with_drafts`). The
    // page is a slice, though: the mailbox's own copy of an old thread can sit
    // below it, leaving the card on the folder its newest hit came from. The
    // cache has the whole mailbox, so it settles those.
    let strays: Vec<String> = cards
        .iter()
        .filter(|card| !card.header.folder.eq_ignore_ascii_case(folder_id))
        .map(|card| card.thread_key.clone())
        .collect();
    if !strays.is_empty() {
        let here = store::card_keys_in_folder(conn, account_id, folder_id, &strays)?;
        for card in &mut cards {
            if here.contains(&card.thread_key) {
                card.header.folder = folder_id.to_string();
            }
        }
    }

    // The page these cards were grouped from is a filtered, cursor-paged slice
    // of messages, so its per-card tally is not the thread size. Re-count from
    // the cached folder, bucketed by the folder each card actually sits in
    // (starred and search pages span several). The page tally stays as a floor:
    // a live IMAP page can hold messages the cache has not stored yet.
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut keys_by_folder: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for card in &cards {
        keys_by_folder
            .entry(card.header.folder.as_str())
            .or_default()
            .push(card.thread_key.clone());
    }
    for (folder, keys) in keys_by_folder {
        counts.extend(store::card_message_counts(conn, account_id, folder, &keys)?);
    }

    cards
        .into_iter()
        .map(|card| {
            let folder = card.header.folder.as_str();
            let message_count = counts
                .get(&card.thread_key)
                .copied()
                .unwrap_or(0)
                .max(card.message_count);
            let folder_role = store::folder_role(conn, account_id, folder)?;
            let thread_id = format_thread_id(account_id, folder, &card.thread_key);
            let original_thread_id = card
                .original_thread_key
                .as_deref()
                .map(|key| format_thread_id(account_id, folder, key));
            Ok((
                card.thread_key.clone(),
                json!({
                "id": thread_id,
                "account_id": account_id,
                "folder_id": folder,
                "folder_role": folder_role,
                "thread_id": thread_id,
                "original_thread_id": original_thread_id,
                "from_name": card.header.from_name,
                "from_addr": card.header.from_addr,
                "to": "",
                "subject": card.header.subject,
                "preview": "",
                "body": "",
                "date": card.header.date,
                "unread": card.unread_count > 0,
                "unread_count": card.unread_count,
                "message_count": message_count,
                "starred": card.header.starred,
                "has_draft": card.has_draft,
                "has_attachments": false,
                "recipient_overflow": card.header.recipient_overflow,
                }),
            ))
        })
        .collect()
}

/// Every starred mail message across all accounts, as ordinary thread cards.
///
/// The starred view used to emit one row per starred *message* (`id` was
/// `{thread_id}#{uid}`), which made it the only list in the app whose rows were
/// not threads — hence its own row menu, its own bulk-selection surface and a
/// `selectedStarredItem` selection model running alongside the normal one.
/// Building the same cards every other mailbox builds collapses all of that:
/// star is just another folder whose contents happen to span accounts.
///
/// Cards are grouped per (account, folder) rather than in one pass, because
/// [`store::group_thread_cards_with_drafts`] keys on the thread key alone. A
/// single pass would merge a thread starred in both Inbox and Archive into one
/// card and pin it to whichever folder happened to sort first. The per-folder
/// pass is then collapsed back to one card per thread by
/// [`starred_folder_rank`], so a thread the server files under several folders
/// is listed once, in its most specific one.
///
/// `max_threads` bounds the conversations read, not the rows: the cards are
/// what [`starred_page`] then searches, sorts and pages through, so a budget
/// spent on duplicate folder copies would cut the list short with no cursor
/// left to reach what it dropped.
pub fn starred_thread_cards(conn: &Connection, max_threads: u32) -> anyhow::Result<Vec<Value>> {
    let mut by_location: BTreeMap<(String, String), Vec<MessageHeader>> = BTreeMap::new();
    // The conversations the budget let through. Expanding a starred hit below
    // re-reads its whole root thread, which can carry subject branches the
    // budget stopped short of; those are a later page's rows, not this set's.
    let mut admitted: HashSet<(String, String)> = HashSet::new();
    for (account, header) in store::get_starred_all_accounts(conn, max_threads)? {
        let folder = if header.folder.is_empty() {
            "INBOX".to_string()
        } else {
            header.folder.clone()
        };
        admitted.insert((
            account.clone(),
            store::starred_thread_identity(&folder, &store::card_thread_key(&header)),
        ));
        by_location
            .entry((account, folder))
            .or_default()
            .push(header);
    }

    let mut draft_keys_by_account: HashMap<String, HashSet<String>> = HashMap::new();
    let mut cards: Vec<Value> = Vec::new();
    // Where each (account, thread key) already landed, so a second folder
    // holding the same thread replaces that card instead of adding a row.
    let mut card_slots: HashMap<(String, String), usize> = HashMap::new();
    for ((account, folder), starred_headers) in by_location {
        // A starred hit admits its whole thread into the view. Build the card
        // from every cached message in that thread so its date, unread count and
        // sender match the ordinary mailbox card rather than the particular
        // starred message that happened to admit it.
        let mut headers = Vec::new();
        let mut seen_thread_keys = HashSet::new();
        for header in starred_headers {
            // Already the `uid:<uid>` fallback when the message carried no
            // threading headers — the cache read fills that in.
            let thread_key = header.thread_key;
            if seen_thread_keys.insert(thread_key.clone()) {
                headers.extend(store::get_thread_headers(
                    conn,
                    &account,
                    &folder,
                    &thread_key,
                )?);
            }
        }
        for header in &mut headers {
            header.folder = folder.clone();
        }
        headers.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.uid.cmp(&a.uid)));
        let draft_thread_keys = match draft_keys_by_account.get(&account) {
            Some(keys) => keys,
            None => {
                let keys = store::draft_thread_keys(conn, &account)?;
                draft_keys_by_account.entry(account.clone()).or_insert(keys)
            }
        };
        for (thread_key, card) in
            thread_cards_json_keyed(conn, &account, &folder, headers, draft_thread_keys)?
        {
            // Branching can split one root thread by subject. Only branches that
            // actually contain a starred message belong in the starred view.
            if !card["starred"].as_bool().unwrap_or(false) {
                continue;
            }
            let identity = (
                account.clone(),
                store::starred_thread_identity(&folder, &thread_key),
            );
            if !admitted.contains(&identity) {
                continue;
            }
            match card_slots.entry(identity) {
                Entry::Occupied(slot) => {
                    let kept = &mut cards[*slot.get()];
                    if card_rank(&card) < card_rank(kept) {
                        *kept = card;
                    }
                }
                Entry::Vacant(slot) => {
                    slot.insert(cards.len());
                    cards.push(card);
                }
            }
        }
    }
    Ok(cards)
}

fn card_rank(card: &Value) -> u8 {
    starred_folder_rank(card["folder_role"].as_str().unwrap_or("folder"))
}

/// Which folder's copy of a thread the starred view shows when the account
/// caches the same thread in more than one — Gmail keeps every message in All
/// Mail alongside its Inbox or label copy, which would otherwise put one
/// identical row per folder in the list. Lowest rank wins: the copy in a real
/// mailbox beats the catch-all archive, and a still-inboxed thread is listed
/// where the user would look for it.
fn starred_folder_rank(role: &str) -> u8 {
    match role {
        "inbox" => 0,
        "folder" => 1,
        "sent" => 2,
        "drafts" => 3,
        "archive" => 4,
        "junk" => 5,
        "trash" => 6,
        _ => 7,
    }
}

pub fn allocate_message_id(account_id: &str, draft: bool) -> String {
    let domain = account_id
        .split_once('@')
        .map(|(_, domain)| domain)
        .filter(|domain| !domain.is_empty())
        .unwrap_or("meron");
    let prefix = if draft { "meron-draft-" } else { "meron-" };
    format!("{prefix}{}@{domain}", uuid::Uuid::new_v4().simple())
}

pub fn starred_page(
    mut items: Vec<Value>,
    query: &str,
    filter: &str,
    limit: usize,
    before_cursor: Option<&str>,
) -> Value {
    if filter.eq_ignore_ascii_case("unread") {
        items.retain(|item| item.get("unread").and_then(Value::as_bool).unwrap_or(false));
    }
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        items.retain(|item| {
            [
                "subject",
                "preview",
                "from_name",
                "from_addr",
                "account_id",
                "folder_id",
            ]
            .iter()
            .filter_map(|field| item.get(field).and_then(Value::as_str))
            .any(|value| value.to_lowercase().contains(&query))
        });
    }
    items.sort_by(|a, b| {
        b.get("date")
            .and_then(Value::as_i64)
            .cmp(&a.get("date").and_then(Value::as_i64))
            .then_with(|| {
                b.get("id")
                    .and_then(Value::as_str)
                    .cmp(&a.get("id").and_then(Value::as_str))
            })
    });
    let start = before_cursor
        .and_then(decode_starred_cursor)
        .and_then(|(date, id)| {
            items.iter().position(|item| {
                item.get("date").and_then(Value::as_i64) == Some(date)
                    && item.get("id").and_then(Value::as_str) == Some(id.as_str())
            })
        })
        .map(|index| index + 1)
        .unwrap_or_default();
    let end = (start + limit).min(items.len());
    let page = items[start..end].to_vec();
    let next_cursor = if end < items.len() {
        page.last().and_then(|item| {
            Some(encode_starred_cursor(
                item.get("date")?.as_i64()?,
                item.get("id")?.as_str()?,
            ))
        })
    } else {
        None
    };
    let mut out = json!({ "items": page });
    if let Some(cursor) = next_cursor {
        out["next_cursor"] = Value::String(cursor);
    }
    out
}

pub fn mutation_result(
    mut result: Value,
    conn: &Connection,
    account_id: &str,
    thread_id: &str,
    source_folder: &str,
    target_folder: Option<&str>,
    unread: Option<bool>,
    starred: Option<bool>,
    removed: bool,
) -> anyhow::Result<Value> {
    let mut folders = serde_json::Map::new();
    let mut folder_counts = Vec::new();
    let mut seen_folders = std::collections::HashSet::new();
    for folder in std::iter::once(source_folder)
        .chain(target_folder)
        .filter(|folder| seen_folders.insert(*folder))
    {
        let unread = store::get_folder_unread(conn, account_id, folder)?;
        folders.insert(folder.to_string(), json!(unread));
        folder_counts
            .push(json!({ "account_id": account_id, "folder_id": folder, "unread": unread }));
    }
    result["change"] = json!({
        "thread_id": thread_id,
        "account_id": account_id,
        "source_folder": source_folder,
        "source_role": store::folder_role(conn, account_id, source_folder)?,
        "target_folder": target_folder,
        "target_role": target_folder.map(|folder| store::folder_role(conn, account_id, folder)).transpose()?,
        "unread": unread,
        "starred": starred,
        "removed": removed,
    });
    let mut folder_unreads = serde_json::Map::new();
    folder_unreads.insert(account_id.to_string(), Value::Object(folders));
    result["folder_unreads"] = Value::Object(folder_unreads);
    result["folder_counts"] = Value::Array(folder_counts);
    Ok(result)
}

fn encode_starred_cursor(date: i64, id: &str) -> String {
    let value = json!({ "date": date, "id": id });
    format!(
        "starred:{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value).unwrap_or_default())
    )
}

fn decode_starred_cursor(cursor: &str) -> Option<(i64, String)> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor.strip_prefix("starred:")?.as_bytes())
        .ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    Some((
        value.get("date")?.as_i64()?,
        value.get("id")?.as_str()?.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocated_message_ids_use_account_domain_and_lifecycle_prefix() {
        let draft = allocate_message_id("me@example.com", true);
        let outgoing = allocate_message_id("me@example.com", false);
        assert!(draft.starts_with("meron-draft-"));
        assert!(outgoing.starts_with("meron-"));
        assert!(!outgoing.starts_with("meron-draft-"));
        assert!(draft.ends_with("@example.com"));
        assert!(outgoing.ends_with("@example.com"));
        assert_ne!(draft, outgoing);
    }

    #[test]
    fn starred_cards_stop_at_the_budget_even_within_one_root_thread() {
        // Expanding a starred hit re-reads its whole root thread, so a root that
        // branched by subject hands back branches the budget never admitted.
        // Those must not ride along: the budget is what bounds both the work
        // here and the list the caller then pages through.
        let conn = Connection::open_in_memory().unwrap();
        store::run_migrations(&conn).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 1,
                    subject: "Sprint planning".to_string(),
                    date: 300,
                    starred: true,
                    thread_key: "root@example.com".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 2,
                    subject: "Lunch orders".to_string(),
                    date: 200,
                    starred: true,
                    thread_key: "root@example.com".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();

        let one = starred_thread_cards(&conn, 1).unwrap();
        assert_eq!(one.len(), 1, "{one:?}");
        assert_eq!(one[0]["subject"], "Sprint planning");

        let both = starred_thread_cards(&conn, 2).unwrap();
        assert_eq!(both.len(), 2, "{both:?}");
    }

    // A search page is a slice of the merged mailbox+Sent results. When the
    // thread's copy in the searched mailbox sits below the page — an old hit,
    // pushed down by the reply that was just sent — the page alone would leave
    // the card on Sent, minting a second id for a thread the list already shows
    // under the mailbox's.
    #[test]
    fn a_search_card_keeps_the_searched_mailbox_even_when_its_copy_paged_out() {
        let conn = Connection::open_in_memory().unwrap();
        store::run_migrations(&conn).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::ensure_folder(&conn, "me@example.com", "Sent").unwrap();
        let inbox_copy = MessageHeader {
            uid: 1,
            folder: "INBOX".to_string(),
            subject: "Topic".to_string(),
            date: 100,
            thread_key: "root@example.com".to_string(),
            ..Default::default()
        };
        let sent_reply = MessageHeader {
            uid: 9,
            folder: "Sent".to_string(),
            subject: "Re: Topic".to_string(),
            date: 300,
            thread_key: "root@example.com".to_string(),
            ..Default::default()
        };
        store::upsert_messages(&conn, "me@example.com", "INBOX", &[inbox_copy]).unwrap();
        store::upsert_messages(&conn, "me@example.com", "Sent", &[sent_reply.clone()]).unwrap();

        // Only the reply made the page.
        let cards = thread_cards_json(
            &conn,
            "me@example.com",
            "INBOX",
            vec![sent_reply],
            &HashSet::new(),
        )
        .unwrap();

        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["folder_id"], "INBOX");
        assert_eq!(
            cards[0]["thread_id"],
            format_thread_id("me@example.com", "INBOX", "root@example.com#Topic")
        );
        // Both copies are one thread, page or no page.
        assert_eq!(cards[0]["message_count"], 2);
    }

    #[test]
    fn folder_delete_gate_requires_a_synced_ordinary_folder() {
        let conn = Connection::open_in_memory().unwrap();
        store::run_migrations(&conn).unwrap();
        store::upsert_folders(
            &conn,
            "me@example.com",
            &[
                crate::imap::Folder {
                    name: "Projects".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Projects/2026".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Projects/2026/Q1".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Localized Sent".to_string(),
                    delimiter: Some("/".to_string()),
                    special_use: Some("sent".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Mail".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Mail/Archive".to_string(),
                    delimiter: Some("/".to_string()),
                    special_use: Some("archive".to_string()),
                    ..Default::default()
                },
            ],
        )
        .unwrap();

        // A parent goes with its subfolders, so nesting is no bar on its own.
        assert!(check_folder_deletable(&conn, "me@example.com", "Projects").is_ok());
        assert!(
            check_folder_deletable(&conn, "me@example.com", "Localized Sent")
                .unwrap_err()
                .contains("special folder")
        );
        // ...but a special-use folder buried in the subtree still blocks it.
        assert!(
            check_folder_deletable(&conn, "me@example.com", "Mail")
                .unwrap_err()
                .contains("Mail/Archive is a special folder")
        );
        assert!(
            check_folder_deletable(&conn, "me@example.com", "Uncached")
                .unwrap_err()
                .contains("synchronized folder list")
        );
    }

    #[test]
    fn folder_delete_targets_list_the_subtree_deepest_first() {
        let conn = Connection::open_in_memory().unwrap();
        store::run_migrations(&conn).unwrap();
        store::upsert_folders(
            &conn,
            "me@example.com",
            &[
                crate::imap::Folder {
                    name: "Projects".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Projects/2026".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Projects/2026/Q1".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Projects2".to_string(),
                    delimiter: Some("/".to_string()),
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Flat".to_string(),
                    delimiter: None,
                    ..Default::default()
                },
                crate::imap::Folder {
                    name: "Flat/Independent".to_string(),
                    delimiter: None,
                    ..Default::default()
                },
            ],
        )
        .unwrap();

        // Every child comes before its own parent, and the lookalike sibling
        // that merely shares a name prefix stays out of it.
        assert_eq!(
            folder_delete_targets(&conn, "me@example.com", "Projects").unwrap(),
            vec![
                "Projects/2026/Q1".to_string(),
                "Projects/2026".to_string(),
                "Projects".to_string(),
            ]
        );
        assert_eq!(
            folder_delete_targets(&conn, "me@example.com", "Projects/2026/Q1").unwrap(),
            vec!["Projects/2026/Q1".to_string()]
        );
        // A NIL hierarchy delimiter makes slash ordinary name punctuation.
        assert_eq!(
            folder_delete_targets(&conn, "me@example.com", "Flat").unwrap(),
            vec!["Flat".to_string()]
        );
    }

    #[test]
    fn starred_page_filters_and_pages_with_an_opaque_cursor() {
        let items = vec![
            json!({"id":"three","date":300,"subject":"Other"}),
            json!({"id":"two","date":200,"subject":"Design review"}),
            json!({"id":"one","date":100,"subject":"Design notes"}),
        ];
        let first = starred_page(items.clone(), "design", "all", 1, None);
        assert_eq!(first["items"][0]["id"], "two");
        let cursor = first["next_cursor"].as_str().unwrap();
        assert!(cursor.starts_with("starred:"));
        let second = starred_page(items, "design", "all", 1, Some(cursor));
        assert_eq!(second["items"][0]["id"], "one");
        assert!(second.get("next_cursor").is_none());
    }

    #[test]
    fn starred_page_filters_unread_before_paging() {
        let items = vec![
            json!({"id":"read","date":300,"unread":false}),
            json!({"id":"unread-two","date":200,"unread":true}),
            json!({"id":"unread-one","date":100,"unread":true}),
        ];
        let first = starred_page(items.clone(), "", "unread", 1, None);
        assert_eq!(first["items"][0]["id"], "unread-two");
        let cursor = first["next_cursor"].as_str().unwrap();
        let second = starred_page(items, "", "unread", 1, Some(cursor));
        assert_eq!(second["items"][0]["id"], "unread-one");
        assert!(second.get("next_cursor").is_none());
    }

    #[test]
    fn mutation_result_reports_authoritative_counts_and_folder_roles() {
        let conn = Connection::open_in_memory().unwrap();
        store::run_migrations(&conn).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::ensure_folder(&conn, "me@example.com", "Archive").unwrap();
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen)
             VALUES('me@example.com', 'INBOX', 'one', 1, 'One', 2, 0),
                   ('me@example.com', 'Archive', 'two', 2, 'Two', 1, 0)",
            [],
        )
        .unwrap();

        let result = mutation_result(
            json!({ "ok": true }),
            &conn,
            "me@example.com",
            "me@example.com#INBOX#thread",
            "INBOX",
            Some("Archive"),
            Some(false),
            None,
            true,
        )
        .unwrap();

        assert_eq!(result["folder_unreads"]["me@example.com"]["INBOX"], 1);
        assert_eq!(result["folder_unreads"]["me@example.com"]["Archive"], 1);
        assert_eq!(result["change"]["source_role"], "inbox");
        assert_eq!(result["change"]["target_role"], "archive");
        assert_eq!(result["change"]["removed"], true);
        assert_eq!(result["folder_counts"].as_array().unwrap().len(), 2);
    }
}
