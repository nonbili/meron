use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

mod compose;
mod folders;
mod sync;
mod tags_and_rules;
mod thread_flags;
mod thread_moves;
mod threads;

pub(crate) use compose::{
    discard_mobile_draft, respond_mobile_invite, save_mobile_draft, send_mobile_message,
};
pub(crate) use folders::{
    create_mobile_folder, delete_mobile_folder, empty_mobile_folder, list_mobile_folders,
    mark_mobile_folder_all_read, rename_mobile_folder, set_mobile_folder_notify,
};
pub use sync::messages_still_unread;
pub(crate) use sync::mobile_messages_still_unread;
pub(crate) use sync::sync_mobile_mail;
pub(crate) use tags_and_rules::{
    list_mobile_tags, mobile_rule_matches, mobile_rule_status, mobile_rules, replace_mobile_tag,
    run_mobile_rule, set_mobile_rules, set_mobile_thread_tags, stop_mobile_rule,
};
pub(crate) use thread_flags::{mark_mobile_thread_read, mark_mobile_thread_starred};
pub(crate) use thread_moves::{
    archive_mobile_thread, copy_mobile_thread, delete_mobile_thread, move_mobile_thread,
};
pub(crate) use threads::{
    list_mobile_contact_threads, list_mobile_starred_items, list_mobile_threads, read_mobile_thread,
};

pub(crate) fn cached_thread_uids(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
) -> Result<Vec<u32>, String> {
    // A uid-style id names exactly one message; answer it directly. Its
    // display thread_key is "uid:{N}", which the store lookup below would no
    // longer match once a resync fills in the row's real thread_key.
    if let Some(uid) = parsed.uid {
        return Ok(vec![uid]);
    }
    store::resolve_message_uids(
        conn,
        &parsed.account,
        &parsed.folder,
        &parsed.thread_key,
        parsed.subject_filter.as_deref(),
        None,
        &[],
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn has_requested_mobile_message_ids(params: &Value) -> bool {
    params
        .get("message_ids")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str().is_some_and(|value| !value.trim().is_empty()))
        })
}

pub(crate) fn requested_mobile_uids(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
    params: &Value,
) -> Result<Vec<u32>, String> {
    let uids = params
        .get("message_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(mobile_message_id_uid)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if has_requested_mobile_message_ids(params) {
        return Ok(uids);
    }
    cached_thread_uids(conn, parsed)
}

pub(crate) fn update_mobile_read_state(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
    params: &Value,
    uids: &[u32],
    seen: bool,
) -> Result<(), String> {
    if has_requested_mobile_message_ids(params) || parsed.subject_filter.is_some() || !seen {
        // Explicit message ids, or a subject-branched card: `uids` is already
        // scoped to exactly the acted-on messages. The whole-thread update
        // below would flip sibling branches sharing the root thread_key.
        // Marking unread is per-uid too — only the newest message was flagged.
        for uid in uids {
            store::update_message_seen(conn, &parsed.account, &parsed.folder, *uid, seen)
                .map_err(|err| err.to_string())?;
        }
    } else if let Some(uid) = parsed.uid {
        store::update_message_seen(conn, &parsed.account, &parsed.folder, uid, seen)
            .map_err(|err| err.to_string())?;
    } else {
        store::update_thread_seen(
            conn,
            &parsed.account,
            &parsed.folder,
            &parsed.thread_key,
            seen,
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub(crate) fn mobile_message_id_uid(value: &str) -> Option<u32> {
    value
        .trim()
        .rsplit('#')
        .next()
        .and_then(|uid| uid.parse::<u32>().ok())
}

pub(crate) fn find_cached_archive_folder(
    conn: &rusqlite::Connection,
    account: &str,
) -> Result<String, String> {
    let folders = store::get_folders(conn, account).map_err(|err| err.to_string())?;
    crate::engine::cached_archive_folder_from_folders(folders, "")
        .ok_or_else(|| "Archive folder not found for this account".to_string())
}

pub(crate) fn canon_folder(folder: &str) -> String {
    crate::mail_model::canon_folder(folder)
}

pub(crate) fn format_thread_id(account_id: &str, folder: &str, thread_key: &str) -> String {
    crate::mail_model::format_thread_id(account_id, folder, thread_key)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedThreadId {
    pub(crate) account: String,
    pub(crate) folder: String,
    pub(crate) thread_key: String,
    pub(crate) uid: Option<u32>,
    pub(crate) subject_filter: Option<String>,
}

pub(crate) fn parse_thread_id(thread_id: &str) -> Option<ParsedThreadId> {
    let first = thread_id.find('#')?;
    let last = thread_id.rfind('#')?;
    if first == 0 || last <= first {
        return None;
    }
    let account = thread_id[..first].to_string();
    let folder = thread_id[first + 1..last].to_string();
    let key = &thread_id[last + 1..];
    if let Some(encoded) = key.strip_prefix("t.") {
        let decoded = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).ok()?;
        let thread_key = String::from_utf8(decoded).ok()?;
        let (root_thread_key, subject_filter) = store::split_thread_key(&thread_key);
        return Some(ParsedThreadId {
            account,
            folder,
            thread_key: root_thread_key,
            uid: None,
            subject_filter,
        });
    }
    let uid = key.parse::<u32>().ok()?;
    Some(ParsedThreadId {
        account,
        folder,
        thread_key: format!("uid:{uid}"),
        uid: Some(uid),
        subject_filter: None,
    })
}

pub(crate) fn parse_thread_id_with_request_folder(
    thread_id: &str,
    params: &Value,
) -> Option<ParsedThreadId> {
    let mut parsed = parse_thread_id(thread_id)?;
    if let Some(folder) = params
        .get("folder")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        // Canonicalize like `format_thread_id` does: clients spell the folder
        // however their UI keys it (the desktop sends "inbox"), and the store /
        // IMAP SELECT paths downstream need the canonical name.
        parsed.folder = canon_folder(folder);
    }
    Some(parsed)
}
