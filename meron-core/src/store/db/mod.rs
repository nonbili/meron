//! The store's SQLite database: where it lives and how it is opened and keyed
//! (`open`), its schema (`migrations`), the one-time rewrites that follow a
//! data-format change (`data_upgrades`), and archive snapshots (`backup`).

mod backup;
mod data_upgrades;
mod migrations;
mod open;
mod retry;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod upgrade_tests;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

pub use backup::{backup_archive_to, backup_database_files};
pub use migrations::NewerSchema;
pub use open::{
    accept_newer_schema_at, accept_newer_schemas, app_dir, check_newer_schema_at, newer_schemas,
    open, open_archive, open_archive_at, open_at, open_at_keyed, open_at_notifying, open_notifying,
    store_path,
};

#[cfg(test)]
pub(super) use data_upgrades::{invalidate_body_cache_if_needed, reread_synced_repeat_rules};
#[cfg(test)]
pub(super) use migrations::{SCHEMA_VERSION, migrate_to, run_migrations};

pub(super) fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

pub(super) fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
