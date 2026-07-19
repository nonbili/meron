use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub type AccountCursors = BTreeMap<String, String>;

pub fn encode_cursor(cursors: &AccountCursors) -> String {
    let json = serde_json::to_vec(cursors).unwrap_or_default();
    format!("unified:{}", URL_SAFE_NO_PAD.encode(json))
}

pub fn decode_cursor(cursor: &str) -> Option<AccountCursors> {
    let encoded = cursor.strip_prefix("unified:")?;
    let json = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).ok()?;
    serde_json::from_slice(&json).ok()
}

pub fn merge_pages(pages: Vec<(String, Result<Value, String>)>, items_field: &str) -> Value {
    let mut items = Vec::new();
    let mut next_cursors = AccountCursors::new();
    let mut folder_unreads = Map::new();
    let mut failures = Vec::new();
    for (account_id, page) in pages {
        match page {
            Ok(result) => {
                items.extend(
                    result
                        .get(items_field)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default(),
                );
                let unread = result
                    .get("folder_unread")
                    .and_then(Value::as_u64)
                    .unwrap_or_default();
                folder_unreads.insert(account_id.clone(), json!(unread));
                if let Some(cursor) = result.get("next_cursor").and_then(Value::as_str) {
                    next_cursors.insert(account_id, cursor.to_string());
                }
            }
            Err(message) => failures.push(json!({ "account_id": account_id, "message": message })),
        }
    }
    items.sort_by(|a, b| {
        b.get("date")
            .and_then(Value::as_i64)
            .cmp(&a.get("date").and_then(Value::as_i64))
    });
    let folder_unread = folder_unreads
        .values()
        .filter_map(Value::as_u64)
        .sum::<u64>();
    let mut out = json!({
        items_field: items,
        "folder_unread": folder_unread,
        "folder_unreads": folder_unreads,
        "failures": failures,
    });
    if !next_cursors.is_empty() {
        out.as_object_mut().unwrap().insert(
            "next_cursor".to_string(),
            Value::String(encode_cursor(&next_cursors)),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trips_per_account_state() {
        let cursors = AccountCursors::from([
            ("first@example.com".to_string(), "date:200:7".to_string()),
            ("second@example.com".to_string(), "date:100:3".to_string()),
        ]);
        let encoded = encode_cursor(&cursors);
        assert!(encoded.starts_with("unified:"));
        assert_eq!(decode_cursor(&encoded), Some(cursors));
        assert_eq!(decode_cursor("date:100:3"), None);
    }

    #[test]
    fn pages_merge_sort_counts_cursors_and_failures() {
        let merged = merge_pages(
            vec![
                (
                    "first".to_string(),
                    Ok(json!({
                        "threads": [{"id":"old","date":100}],
                        "folder_unread": 2,
                        "next_cursor": "date:100:1"
                    })),
                ),
                (
                    "second".to_string(),
                    Ok(json!({"threads":[{"id":"new","date":200}],"folder_unread":3})),
                ),
                ("broken".to_string(), Err("offline".to_string())),
            ],
            "threads",
        );
        assert_eq!(merged["threads"][0]["id"], "new");
        assert_eq!(merged["folder_unread"], 5);
        assert_eq!(merged["folder_unreads"]["first"], 2);
        assert_eq!(merged["failures"][0]["account_id"], "broken");
        assert!(
            merged["next_cursor"]
                .as_str()
                .unwrap()
                .starts_with("unified:")
        );
    }
}
