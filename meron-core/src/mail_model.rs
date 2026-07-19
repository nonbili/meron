use crate::imap::MessageHeader;
use crate::store;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::HashSet;

pub fn canon_folder(folder: &str) -> String {
    if folder.eq_ignore_ascii_case("inbox") {
        "INBOX".to_string()
    } else {
        folder.to_string()
    }
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
    store::group_thread_cards_with_drafts(messages, folder_id, draft_thread_keys)
        .into_iter()
        .map(|card| {
            let folder = card.header.folder.as_str();
            let folder_role = store::folder_role(conn, account_id, folder)?;
            let thread_id = format_thread_id(account_id, folder, &card.thread_key);
            let original_thread_id = card
                .original_thread_key
                .as_deref()
                .map(|key| format_thread_id(account_id, folder, key));
            Ok(json!({
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
                "starred": card.header.starred,
                "has_draft": card.has_draft,
                "has_attachments": false,
                "recipient_overflow": card.header.recipient_overflow,
            }))
        })
        .collect()
}

pub fn starred_item_json(
    conn: &Connection,
    account_id: &str,
    header: &MessageHeader,
) -> anyhow::Result<Value> {
    let folder = if header.folder.is_empty() {
        "INBOX"
    } else {
        header.folder.as_str()
    };
    let folder_role = store::folder_role(conn, account_id, folder)?;
    let thread_key = if header.thread_key.is_empty() {
        format!("uid:{}", header.uid)
    } else {
        store::card_thread_key(header)
    };
    let thread_id = format_thread_id(account_id, folder, &thread_key);
    Ok(json!({
        "id": format!("{thread_id}#{}", header.uid),
        "account_id": account_id,
        "folder_id": folder,
        "folder_role": folder_role,
        "thread_id": thread_id,
        "from_name": header.from_name.as_str(),
        "from_addr": header.from_addr.as_str(),
        "to": "",
        "reply_to": "",
        "cc": "",
        "bcc": "",
        "message_id": "",
        "references": "",
        "subject": header.subject.as_str(),
        "preview": "",
        "body": "",
        "body_html": "",
        "date": header.date,
        "unread": !header.seen,
        "starred": true,
        "has_attachments": false,
        "attachments": [],
    }))
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
    limit: usize,
    before_cursor: Option<&str>,
) -> Value {
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
    fn starred_page_filters_and_pages_with_an_opaque_cursor() {
        let items = vec![
            json!({"id":"three","date":300,"subject":"Other"}),
            json!({"id":"two","date":200,"subject":"Design review"}),
            json!({"id":"one","date":100,"subject":"Design notes"}),
        ];
        let first = starred_page(items.clone(), "design", 1, None);
        assert_eq!(first["items"][0]["id"], "two");
        let cursor = first["next_cursor"].as_str().unwrap();
        assert!(cursor.starts_with("starred:"));
        let second = starred_page(items, "design", 1, Some(cursor));
        assert_eq!(second["items"][0]["id"], "one");
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
