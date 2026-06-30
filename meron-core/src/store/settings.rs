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
