use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;

// ---- Settings ---------------------------------------------------------------

fn setting_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

pub fn settings_get(conn: &Connection, keys: &[String]) -> Result<serde_json::Value> {
    let mut out = serde_json::Map::new();
    for key in keys {
        if let Some(value) = setting_get(conn, key)? {
            out.insert(
                key.clone(),
                serde_json::from_str(&value).unwrap_or(json!(value)),
            );
        }
    }
    Ok(serde_json::Value::Object(out))
}

pub fn setting_set(conn: &Connection, key: &str, value: &serde_json::Value) -> Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value.to_string()],
    )?;
    Ok(())
}

// ---- Remote content allowlist ----------------------------------------------

/// Settings row holding the app-wide remote-content sender allowlist.
pub const REMOTE_IMAGE_SENDERS_KEY: &str = "remote_image_senders";

/// Senders whose remote content always loads, whatever an account's own "load
/// remote images" toggle says. Normalized by [`super::normalize_sender`] and
/// deduped, so membership tests can compare bare lowercased addresses.
pub fn remote_image_senders(conn: &Connection) -> Result<Vec<String>> {
    let stored = setting_get(conn, REMOTE_IMAGE_SENDERS_KEY)?;
    let parsed: Vec<String> = stored
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    let mut out: Vec<String> = Vec::with_capacity(parsed.len());
    for addr in parsed {
        let addr = super::normalize_sender(&addr);
        if !addr.is_empty() && !out.contains(&addr) {
            out.push(addr);
        }
    }
    Ok(out)
}
