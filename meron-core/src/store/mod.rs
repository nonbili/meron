//! Local SQLite store (rusqlite, bundled) — the single source of truth.
//!
//! `meron.db` (renamed from the old `cache.sqlite`) holds *all* accounts (mail and
//! RSS), fetched folders/messages, per-folder UID sync state, and RSS
//! subscriptions, so the UI renders instantly from disk and history persists
//! across runs. The desktop bridge sets `MERON_CORE_DB` to the active app
//! profile (`meron` or `meron-dev`); standalone runs default under
//! `~/.config/meron`.
//!
//! Accounts and messages share one table each, with a catch-all JSON column
//! absorbing per-engine fields so new account kinds don't force schema
//! migrations (accounts: `config` for mail connection metadata, plus `prefs` for
//! user preferences; messages: `json` for rss item fields). Mail's hot-path
//! columns (integer `uid`, `seen`, `thread_key`) stay typed; JSON carries the
//! divergent tail.

mod db;

pub use db::open;

#[allow(dead_code)]
pub fn open_at(path: impl AsRef<std::path::Path>) -> Result<Connection> {
    db::open_at(path)
}

#[allow(dead_code)]
pub fn open_at_keyed(path: impl AsRef<std::path::Path>, key: &str) -> Result<Connection> {
    db::open_at_keyed(path, key)
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn run_migrations(conn: &Connection) -> Result<()> {
    db::run_migrations(conn)
}

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::imap::{Creds, Folder, MessageHeader};
use crate::parse::{Attachment, Message};
use db::now_unix;

pub const DEFAULT_RSS_SYNC_INTERVAL_MINUTES: u64 = 60;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum ChatWallpaper {
    #[serde(rename = "preset")]
    Preset {
        #[serde(rename = "presetId")]
        preset_id: String,
    },
    #[serde(rename = "custom")]
    Custom { url: String },
}

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

// ---- Accounts ---------------------------------------------------------------

/// A send-as identity for an account: an address the user owns (verified with
/// their provider) and an optional From display name. A blank `name` falls back
/// to the account's `sender_name` when composing.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Alias {
    pub email: String,
    #[serde(default)]
    pub name: String,
}

/// User-editable per-account preferences, stored as the `prefs` JSON column.
/// Every field is optional so unset means "use the default" (resolved in code,
/// e.g. by engine); add a new pref by adding a field — no schema migration.
#[derive(Default, Deserialize)]
struct AccountPrefs {
    /// Whether remote inline images load; `None` = engine default (RSS on, mail off).
    load_remote_images: Option<bool>,
    /// Whether this account's inbox folds into the unified inbox; default on.
    included_in_unified: Option<bool>,
    /// Whether new-mail desktop notifications are suppressed; default off.
    muted: Option<bool>,
    /// Whether automatic mail/feed checking is paused; default off.
    paused: Option<bool>,
    /// Whether conversation bubbles render original HTML when available; default on.
    conversation_html: Option<bool>,
    /// RSS automatic sync interval in minutes; default 60.
    rss_sync_interval_minutes: Option<u64>,
    /// Send-as identities (besides the primary address); default none.
    aliases: Option<Vec<Alias>>,
    /// Per-account chat background; unset uses the app's default wallpaper.
    chat_wallpaper: Option<ChatWallpaper>,
}

impl AccountPrefs {
    fn parse(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }

    /// Resolve the effective "load remote images" value for an engine.
    fn images_enabled(&self, engine: &str) -> bool {
        self.load_remote_images.unwrap_or(engine == "rss")
    }

    /// Whether the account participates in the unified inbox (default true).
    fn in_unified(&self) -> bool {
        self.included_in_unified.unwrap_or(true)
    }

    /// Whether desktop notifications are suppressed for this account (default false).
    fn is_muted(&self) -> bool {
        self.muted.unwrap_or(false)
    }

    /// Whether automatic checking is paused for this account (default false).
    fn is_paused(&self) -> bool {
        self.paused.unwrap_or(false)
    }

    /// Whether conversation bubbles use HTML mode for messages with HTML.
    fn conversation_html(&self) -> bool {
        self.conversation_html.unwrap_or(true)
    }

    /// RSS automatic sync interval in minutes.
    fn rss_sync_interval_minutes(&self) -> u64 {
        self.rss_sync_interval_minutes
            .unwrap_or(DEFAULT_RSS_SYNC_INTERVAL_MINUTES)
    }

    /// Configured send-as aliases (never the primary address).
    fn aliases(&self) -> Vec<Alias> {
        self.aliases.clone().unwrap_or_default()
    }

    /// Aliases as a JSON array for the bridge `Account.aliases` field.
    fn aliases_json(&self) -> serde_json::Value {
        json!(self.aliases())
    }

    fn chat_wallpaper_json(&self) -> serde_json::Value {
        json!(self.chat_wallpaper)
    }
}

/// Non-secret account metadata persisted on connect, alongside `Creds`.
pub struct AccountMeta {
    pub engine: String,
    pub provider: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: String,
    pub sender_name: String,
}

fn creds_to_config(creds: &Creds) -> String {
    json!({
        "host": creds.host,
        "port": creds.port,
        "user": creds.user,
        "tls": creds.tls,
        "starttls": creds.starttls,
        "smtp_host": creds.smtp_host,
        "smtp_port": creds.smtp_port,
        "smtp_tls": creds.smtp_tls,
        "smtp_starttls": creds.smtp_starttls,
        "auth_type": creds.auth_type,
        "token_expires_at": creds.token_expires_at,
    })
    .to_string()
}

fn config_to_creds(json: &str) -> Creds {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_else(|_| json!({}));
    Creds {
        host: v["host"].as_str().unwrap_or("").to_string(),
        port: v["port"].as_u64().unwrap_or(993) as u16,
        user: v["user"].as_str().unwrap_or("").to_string(),
        password: String::new(),
        tls: v["tls"].as_bool().unwrap_or(true),
        starttls: v["starttls"].as_bool().unwrap_or(false),
        smtp_host: v["smtp_host"].as_str().unwrap_or("").to_string(),
        smtp_port: v["smtp_port"].as_u64().unwrap_or(587) as u16,
        smtp_tls: v["smtp_tls"].as_bool().unwrap_or(true),
        smtp_starttls: v["smtp_starttls"].as_bool().unwrap_or(false),
        auth_type: v["auth_type"].as_str().unwrap_or("password").to_string(),
        access_token: None,
        refresh_token: None,
        token_expires_at: v["token_expires_at"].as_i64().unwrap_or(0),
    }
}

/// Persist account connection metadata so IDLE can auto-resume on restart.
/// Secrets (IMAP password, OAuth tokens) are NOT written here — they live in the
/// OS keychain. Mail connection fields are packed into the `config` column.
pub fn upsert_account(
    conn: &Connection,
    id: &str,
    meta: &AccountMeta,
    creds: &Creds,
) -> Result<()> {
    let now = now_unix();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url, config, created_at, updated_at, sender_name, sort_order)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, COALESCE((SELECT MAX(sort_order) + 1 FROM accounts), 0))
         ON CONFLICT(id) DO UPDATE SET
           engine       = excluded.engine,
           provider     = excluded.provider,
           email        = excluded.email,
           display_name = excluded.display_name,
           avatar_url   = excluded.avatar_url,
           config       = excluded.config,
           updated_at   = excluded.updated_at,
           sender_name = excluded.sender_name",
        params![
            id,
            meta.engine,
            meta.provider,
            meta.email,
            meta.display_name,
            meta.avatar_url,
            creds_to_config(creds),
            now,
            meta.sender_name,
        ],
    )?;
    Ok(())
}

/// Rewrite only the connection `config` (e.g. a refreshed OAuth `token_expires_at`)
/// without disturbing the account's display metadata or prefs.
pub fn save_account_config(conn: &Connection, id: &str, creds: &Creds) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET config = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, creds_to_config(creds), now_unix()],
    )?;
    Ok(())
}

/// Persist the per-account "load remote images" preference (explicit on/off),
/// overriding the engine default. `json_set` updates just this key in place, so
/// other prefs are untouched.
pub fn set_load_remote_images(conn: &Connection, id: &str, enabled: bool) -> Result<()> {
    set_account_pref(conn, id, "load_remote_images", enabled)
}

/// Set a boolean field in an account's `prefs` JSON column. `key` is a top-level
/// pref name (e.g. "muted", "paused", "included_in_unified"); unset keys resolve
/// to their code default in `AccountPrefs`, so no schema migration is needed.
pub fn set_account_pref(conn: &Connection, id: &str, key: &str, enabled: bool) -> Result<()> {
    let path = format!("$.{key}");
    conn.execute(
        "UPDATE accounts
         SET prefs = json_set(prefs, ?2, json(?3)), updated_at = ?4
         WHERE id = ?1",
        params![id, path, if enabled { "true" } else { "false" }, now_unix()],
    )?;
    Ok(())
}

/// Set a numeric field in an account's `prefs` JSON column.
pub fn set_account_pref_u64(conn: &Connection, id: &str, key: &str, value: u64) -> Result<()> {
    let path = format!("$.{key}");
    conn.execute(
        "UPDATE accounts
         SET prefs = json_set(prefs, ?2, json(?3)), updated_at = ?4
         WHERE id = ?1",
        params![id, path, value.to_string(), now_unix()],
    )?;
    Ok(())
}

/// Set or clear a JSON field in an account's `prefs` column. `None` removes the
/// key so callers can return to the code default without storing sentinel data.
pub fn set_account_pref_json(
    conn: &Connection,
    id: &str,
    key: &str,
    value: Option<Value>,
) -> Result<()> {
    let path = format!("$.{key}");
    match value {
        Some(value) => {
            conn.execute(
                "UPDATE accounts
                 SET prefs = json_set(prefs, ?2, json(?3)), updated_at = ?4
                 WHERE id = ?1",
                params![id, path, value.to_string(), now_unix()],
            )?;
        }
        None => {
            conn.execute(
                "UPDATE accounts
                 SET prefs = json_remove(prefs, ?2), updated_at = ?3
                 WHERE id = ?1",
                params![id, path, now_unix()],
            )?;
        }
    }
    Ok(())
}

/// Replace an account's send-as aliases (the whole list). Stored in the `prefs`
/// JSON `aliases` key via `json_set`, leaving other prefs untouched. Entries are
/// normalized by the caller (trimmed, blank emails dropped).
pub fn set_account_aliases(conn: &Connection, id: &str, aliases: &[Alias]) -> Result<()> {
    conn.execute(
        "UPDATE accounts
         SET prefs = json_set(prefs, '$.aliases', json(?2)), updated_at = ?3
         WHERE id = ?1",
        params![id, json!(aliases).to_string(), now_unix()],
    )?;
    Ok(())
}

/// An account's configured send-as aliases, for validating/labelling an outgoing
/// From against addresses the user actually owns.
pub fn account_aliases(conn: &Connection, id: &str) -> Result<Vec<Alias>> {
    let prefs: Option<String> = conn
        .query_row(
            "SELECT prefs FROM accounts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match prefs {
        Some(prefs) => AccountPrefs::parse(&prefs).aliases(),
        None => Vec::new(),
    })
}

/// The account's own email addresses (primary + send-as aliases), lowercased, for
/// detecting which cached messages were sent *by* this account.
pub fn self_addrs(conn: &Connection, id: &str) -> std::collections::HashSet<String> {
    let mut addrs = std::collections::HashSet::new();
    if let Ok(Some(email)) = conn
        .query_row(
            "SELECT email FROM accounts WHERE id = ?1",
            params![id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
    {
        let email = email.trim().to_lowercase();
        if !email.is_empty() {
            addrs.insert(email);
        }
    }
    for alias in account_aliases(conn, id).unwrap_or_default() {
        let email = alias.email.trim().to_lowercase();
        if !email.is_empty() {
            addrs.insert(email);
        }
    }
    addrs
}

/// Rewrite the thread-card identity to show *the other party*: for messages this
/// account sent (sender is one of our own addresses), replace `from_name`/`from_addr`
/// with the first recipient and record the count of additional recipients in
/// `recipient_overflow`. Inbound messages — and outbound ones with no usable
/// recipient (e.g. Bcc-only) — are left untouched. Display-only; these headers feed
/// the thread list, not the real message envelope.
///
/// Junk display names (empty, or containing the address itself — some bots send
/// `From: "addr addr" <addr>`, and replies copy that into `To:`) are replaced by
/// the best name seen for that address anywhere in the cache, so the same thread
/// shows the same correspondent name in every folder.
pub fn apply_card_identity(conn: &Connection, account: &str, headers: &mut [MessageHeader]) {
    let mine = self_addrs(conn, account);
    let mut name_cache: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for header in headers {
        let from = header.from_addr.trim().to_lowercase();
        if !from.is_empty()
            && mine.contains(&from)
            && let Some(first) = header.to.first().cloned()
        {
            header.recipient_overflow = (header.to.len() - 1) as u32;
            header.from_name = first.name;
            header.from_addr = first.addr;
        }
        let addr = header.from_addr.trim().to_lowercase();
        if addr.is_empty() || !is_junk_display_name(&header.from_name, &addr) {
            continue;
        }
        let resolved = name_cache
            .entry(addr.clone())
            .or_insert_with(|| known_display_name(conn, account, &addr));
        // No better name anywhere: clear the junk so the UI falls back to the
        // address local part rather than showing "addr addr".
        header.from_name = resolved.clone().unwrap_or_default();
    }
}

/// A display name that adds no information over the address: empty, or one that
/// embeds the address itself (`"addr"`, `"addr addr"`).
fn is_junk_display_name(name: &str, addr_lower: &str) -> bool {
    let name = name.trim().to_lowercase();
    name.is_empty() || name.contains(addr_lower)
}

/// Best human display name seen for `addr` across the account's cached messages:
/// the most recent non-empty sender name that isn't itself junk.
fn known_display_name(conn: &Connection, account: &str, addr_lower: &str) -> Option<String> {
    conn.query_row(
        "SELECT from_name FROM messages
         WHERE account = ?1 AND lower(from_addr) = ?2
           AND TRIM(COALESCE(from_name, '')) <> ''
           AND instr(lower(from_name), ?2) = 0
         ORDER BY id DESC LIMIT 1",
        params![account, addr_lower],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Legacy no-op kept for the keychain-migration path: there are no plaintext
/// secret columns in the unified schema, so there is nothing to scrub.
pub fn scrub_account_secrets(_conn: &Connection, _id: &str) -> Result<()> {
    Ok(())
}

/// Mobile secret storage (no OS keychain). Persists a per-account JSON secret
/// blob in the app-private DB. Desktop uses the `secrets` keychain module
/// instead; these are only called from the mobile FFI path in `protocol.rs`.
#[allow(dead_code)]
pub fn upsert_secret(conn: &Connection, account_id: &str, blob: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO account_secrets(account_id, blob) VALUES(?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET blob = excluded.blob",
        params![account_id, blob],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn load_secret(conn: &Connection, account_id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT blob FROM account_secrets WHERE account_id = ?1",
            params![account_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

#[allow(dead_code)]
pub fn delete_secret(conn: &Connection, account_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM account_secrets WHERE account_id = ?1",
        params![account_id],
    )?;
    Ok(())
}

/// Remove an account and all of its cached state (mail folders/messages and rss
/// subscriptions/items) from the DB.
pub fn delete_account(conn: &Connection, id: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
    tx.execute("DELETE FROM folders WHERE account = ?1", params![id])?;
    tx.execute("DELETE FROM messages WHERE account = ?1", params![id])?;
    tx.execute("DELETE FROM folder_state WHERE account = ?1", params![id])?;
    tx.execute("DELETE FROM subscriptions WHERE account = ?1", params![id])?;
    tx.execute(
        "DELETE FROM account_secrets WHERE account_id = ?1",
        params![id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Load mail accounts (engine = 'mail') and their connection creds for IMAP/IDLE.
/// RSS accounts are skipped — they have no IMAP credentials.
pub fn load_accounts(conn: &Connection) -> Result<Vec<(String, Creds)>> {
    let mut stmt = conn.prepare("SELECT id, config FROM accounts WHERE engine = 'mail'")?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let config: String = row.get(1)?;
        Ok((id, config_to_creds(&config)))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// All accounts (mail + rss) as bridge-shaped JSON for `account.list`. Field
/// names match the desktop bridge's `Account` struct so the UI consumes them
/// directly.
pub fn list_accounts(conn: &Connection) -> Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, engine, provider, email, display_name, avatar_url, config, prefs, sort_order, sender_name
         FROM accounts ORDER BY sort_order, display_name COLLATE NOCASE, id",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let engine: String = row.get(1)?;
        let provider: String = row.get(2)?;
        let email: String = row.get(3)?;
        let display_name: String = row.get(4)?;
        let avatar_url: String = row.get(5)?;
        let config: String = row.get(6)?;
        let prefs: String = row.get(7)?;
        let sort_order: i64 = row.get(8)?;
        let sender_name: String = row.get(9)?;
        Ok((
            id,
            engine,
            provider,
            email,
            display_name,
            avatar_url,
            config,
            prefs,
            sort_order,
            sender_name,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            engine,
            provider,
            email,
            display_name,
            avatar_url,
            config,
            prefs,
            sort_order,
            sender_name,
        ) = row?;
        // Defaults resolved here (by engine) until the user sets an explicit value:
        // RSS images are content (on), email images are a tracking vector (off).
        let p = AccountPrefs::parse(&prefs);
        let load_remote_images = p.images_enabled(&engine);
        let included_in_unified = p.in_unified();
        let muted = p.is_muted();
        let paused = p.is_paused();
        let conversation_html = p.conversation_html();
        let rss_sync_interval_minutes = p.rss_sync_interval_minutes();
        let chat_wallpaper = p.chat_wallpaper_json();
        if engine == "rss" {
            out.push(json!({
                "id": id,
                "email": format!("{id}.local"),
                "display_name": display_name,
                "sender_name": sender_name,
                "avatar_url": avatar_url,
                "provider": "rss",
                "auth_type": "rss",
                "imap_host": "",
                "imap_port": 0,
                "smtp_host": "",
                "smtp_port": 0,
                "tls": false,
                "load_remote_images": load_remote_images,
                "included_in_unified": included_in_unified,
                "muted": muted,
                "paused": paused,
                "conversation_html": conversation_html,
                "chat_wallpaper": chat_wallpaper,
                "rss_sync_interval_minutes": rss_sync_interval_minutes,
                "sort_order": sort_order,
            }));
        } else {
            let c = config_to_creds(&config);
            out.push(json!({
                "id": id,
                "email": if email.is_empty() { c.user.clone() } else { email },
                "display_name": display_name,
                "sender_name": sender_name,
                "avatar_url": avatar_url,
                "provider": provider,
                "engine": "meron_mail",
                "auth_type": c.auth_type,
                "imap_host": c.host,
                "imap_port": c.port,
                "smtp_host": c.smtp_host,
                "smtp_port": c.smtp_port,
                "tls": c.tls,
                "load_remote_images": load_remote_images,
                "included_in_unified": included_in_unified,
                "muted": muted,
                "paused": paused,
                "conversation_html": conversation_html,
                "chat_wallpaper": chat_wallpaper,
                "sort_order": sort_order,
                "aliases": p.aliases_json(),
            }));
        }
    }
    Ok(out)
}

pub fn reorder_accounts(conn: &Connection, ids: &[String]) -> Result<()> {
    let mut stmt = conn.prepare("UPDATE accounts SET sort_order = ?1 WHERE id = ?2")?;
    for (index, id) in ids.iter().enumerate() {
        stmt.execute(params![index as i64, id])?;
    }
    Ok(())
}

/// The resolved "load remote images" preference for an account: the stored
/// explicit value, or the engine default (RSS on, mail off) when unset. Unknown
/// accounts return false.
pub fn load_remote_images(conn: &Connection, id: &str) -> Result<bool> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT engine, prefs FROM accounts WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((engine, prefs)) => AccountPrefs::parse(&prefs).images_enabled(&engine),
        None => false,
    })
}

/// Whether desktop notifications are suppressed for an account (default false).
/// Unknown accounts return false.
pub fn account_muted(conn: &Connection, id: &str) -> Result<bool> {
    Ok(account_prefs(conn, id)?
        .map(|p| p.is_muted())
        .unwrap_or(false))
}

/// Whether automatic mail/feed checking is paused for an account (default false).
/// Unknown accounts return false.
pub fn account_paused(conn: &Connection, id: &str) -> Result<bool> {
    Ok(account_prefs(conn, id)?
        .map(|p| p.is_paused())
        .unwrap_or(false))
}

/// The parsed `prefs` for an account, or None if the account row is missing.
fn account_prefs(conn: &Connection, id: &str) -> Result<Option<AccountPrefs>> {
    let prefs: Option<String> = conn
        .query_row(
            "SELECT prefs FROM accounts WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(prefs.map(|p| AccountPrefs::parse(&p)))
}

/// The engine ('mail' | 'rss') backing an account, if it exists. Lets the
/// dispatcher route generic message/folder calls to the right code path.
pub fn account_engine(conn: &Connection, id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT engine FROM accounts WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

// ---- Folders (mail) ---------------------------------------------------------

pub fn upsert_folders(conn: &Connection, account: &str, folders: &[Folder]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for folder in folders {
        tx.execute(
            "INSERT INTO folders(account, name, delimiter) VALUES(?1, ?2, ?3)
             ON CONFLICT(account, name) DO UPDATE SET delimiter = excluded.delimiter",
            params![account, folder.name, folder.delimiter],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Guarantee a folder row exists for a folder we have messages in, without
/// touching its delimiter. The full folder LIST sync (`upsert_folders`) only
/// runs when an account is opened directly, so in the unified view a freshly
/// added account never gets folder rows — and `get_folders` (which JOINs the
/// folders table) would then report zero unread even with unseen mail in the
/// store, leaving the tray dot and unread badges dark. Calling this on every
/// message sync keeps the count honest and self-heals existing accounts.
pub fn ensure_folder(conn: &Connection, account: &str, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO folders(account, name, delimiter) VALUES(?1, ?2, NULL)",
        params![account, name],
    )?;
    Ok(())
}

pub fn get_folders(conn: &Connection, account: &str) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare(
        "SELECT f.name, f.delimiter,
                (SELECT COUNT(*) FROM messages m
                  WHERE m.account = f.account AND m.folder = f.name AND m.seen = 0) AS unread
           FROM folders f WHERE f.account = ?1 ORDER BY f.name",
    )?;
    let rows = stmt.query_map(params![account], |row| {
        Ok(Folder {
            name: row.get(0)?,
            delimiter: row.get(1)?,
            unread: row.get::<_, i64>(2)? as u32,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---- Messages (mail) --------------------------------------------------------

pub fn upsert_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    messages: &[MessageHeader],
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for m in messages {
        // Store recipient lists as JSON. Skip empty lists so a later flag-only
        // resync (which carries no envelope) can't clobber recipients we already
        // cached with NULLs.
        let mut extra = serde_json::Map::new();
        if !m.to.is_empty() {
            extra.insert("to".to_string(), json!(m.to));
        }
        if !m.cc.is_empty() {
            extra.insert("cc".to_string(), json!(m.cc));
        }
        if !m.message_id.is_empty() {
            extra.insert("message_id".to_string(), json!(m.message_id));
        }
        if !m.in_reply_to.is_empty() {
            extra.insert("in_reply_to".to_string(), json!(m.in_reply_to));
        }
        let extra_json = Value::Object(extra).to_string();
        let thread_key = resolve_message_thread_key(&tx, account, &m.thread_key, &m.in_reply_to)?;
        tx.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key, json)
             VALUES(?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(account, folder, msg_id) DO UPDATE SET
               subject    = excluded.subject,
               from_name  = excluded.from_name,
               from_addr  = excluded.from_addr,
               date       = excluded.date,
               seen       = excluded.seen,
               starred    = excluded.starred,
               thread_key = excluded.thread_key,
               json       = json_patch(messages.json, excluded.json)",
            params![
                account,
                folder,
                m.uid,
                m.subject,
                m.from_name,
                m.from_addr,
                m.date,
                m.seen as i64,
                m.starred as i64,
                thread_key,
                extra_json
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn resolve_message_thread_key(
    conn: &rusqlite::Transaction<'_>,
    account: &str,
    thread_key: &str,
    in_reply_to: &str,
) -> Result<String> {
    let key = thread_key.trim();
    let parent_id = in_reply_to.trim();
    if key.is_empty()
        || parent_id.is_empty()
        || !key.eq_ignore_ascii_case(parent_id)
        || key.starts_with("uid:")
        || key.starts_with("gmthrid:")
    {
        return Ok(thread_key.to_string());
    }

    let inherited = conn
        .query_row(
            "SELECT thread_key FROM messages
             WHERE account = ?1
               AND lower(COALESCE(json_extract(json, '$.message_id'), '')) = lower(?2)
               AND COALESCE(thread_key, '') <> ''
             ORDER BY date ASC, uid ASC
             LIMIT 1",
            params![account, parent_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    Ok(inherited.unwrap_or_else(|| thread_key.to_string()))
}

pub fn get_recent_page(
    conn: &Connection,
    account: &str,
    folder: &str,
    limit: u32,
    before_cursor: Option<(i64, u32)>,
    unread_only: bool,
) -> Result<(Vec<MessageHeader>, Option<String>)> {
    let probe = limit.saturating_add(1);
    // Newest-first by send time. The cursor is the (date, uid) of the last row of
    // the previous page; uid is the keyset tiebreaker because `date` is not unique
    // (two messages can share a second), so it gives a stable, gap-free walk.
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to') FROM messages
         WHERE account = ?1 AND folder = ?2
           AND (?6 = 0 OR seen = 0)
           AND (?3 IS NULL
                OR date < ?3
                OR (date = ?3 AND uid < ?4))
         ORDER BY date DESC, uid DESC LIMIT ?5",
    )?;
    let cursor_date = before_cursor.map(|(date, _)| date);
    let cursor_uid = before_cursor.map(|(_, uid)| uid as i64).unwrap_or(0);
    let rows = stmt.query_map(
        params![
            account,
            folder,
            cursor_date,
            cursor_uid,
            probe as i64,
            unread_only as i64
        ],
        |row| {
            let uid = row.get(0)?;
            Ok(MessageHeader {
                uid,
                subject: row.get(1)?,
                from_name: row.get(2)?,
                from_addr: row.get(3)?,
                date: row.get(4)?,
                seen: row.get::<_, i64>(5)? != 0,
                starred: row.get::<_, i64>(6)? != 0,
                thread_key: row
                    .get::<_, Option<String>>(7)?
                    .filter(|key| !key.is_empty())
                    .unwrap_or_else(|| format!("uid:{}", uid)),
                to: parse_recipients_json(row.get::<_, Option<String>>(8)?),
                folder: String::new(),
                ..Default::default()
            })
        },
    )?;
    let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = out.len() > limit as usize;
    if has_more {
        out.truncate(limit as usize);
    }
    let next_cursor = if has_more {
        out.last()
            .map(|header| format!("date:{}:{}", header.date, header.uid))
    } else {
        None
    };
    Ok((out, next_cursor))
}

pub fn search_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // The trigram index needs >= 3 codepoints; serve shorter queries (common for
    // CJK, where words are often 2 characters) with the scoped LIKE scan instead.
    if q.chars().count() < 3 {
        return search_messages_like(conn, account, folder, q, limit);
    }
    // Whole query as one quoted FTS phrase -> trigram substring match (doubling
    // any `"` so user input can't change the query). Same substring semantics as
    // the LIKE path, just index-backed.
    let match_query = format!("\"{}\"", q.replace('"', "\"\""));
    let mut stmt = conn.prepare(
        "SELECT m.uid, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred,
                m.thread_key, json_extract(m.json, '$.to')
         FROM messages_fts f
         JOIN messages m ON m.id = f.rowid
         WHERE f.messages_fts MATCH ?1
           AND m.account = ?2 AND m.folder = ?3 AND m.uid <> 0
         ORDER BY m.date DESC, m.uid DESC LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        params![match_query, account, folder, limit],
        message_header_from_row,
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Substring search via a scoped table scan. Used for queries too short for the
/// trigram FTS index (< 3 codepoints).
fn search_messages_like(
    conn: &Connection,
    account: &str,
    folder: &str,
    q: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>> {
    let like = format!("%{}%", escape_like(q.to_lowercase()));
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to') FROM messages
         WHERE account = ?1 AND folder = ?2 AND uid <> 0
           AND (
             lower(COALESCE(subject, '')) LIKE ?3 ESCAPE '\\'
             OR lower(COALESCE(from_name, '')) LIKE ?3 ESCAPE '\\'
             OR lower(COALESCE(from_addr, '')) LIKE ?3 ESCAPE '\\'
             OR lower(COALESCE(body, '')) LIKE ?3 ESCAPE '\\'
           )
         ORDER BY date DESC, uid DESC LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        params![account, folder, like, limit],
        message_header_from_row,
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// A correspondent surfaced for recipient autocomplete.
#[derive(serde::Serialize)]
pub struct Contact {
    pub name: String,
    pub addr: String,
}

/// Suggest contacts for recipient autocomplete, drawn from both the senders and
/// the To/Cc recipients of cached messages. Results are distinct by address and
/// ranked by how often the address appears. An empty `query` returns the top
/// correspondents; otherwise we match the substring against address and display
/// name. When `account` is empty we search across all accounts (unified compose).
///
/// Recipient lists are stored as JSON, so aggregation happens in Rust rather than
/// SQL: we pull the candidate rows (coarsely pre-filtered by LIKE) and fold them
/// into a per-address tally.
pub fn suggest_contacts(
    conn: &Connection,
    account: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<Contact>> {
    use std::collections::HashMap;

    let q = query.trim().to_lowercase();
    let like = format!("%{}%", escape_like(q.clone()));
    let account_filter = if account.is_empty() {
        "?1 = ''"
    } else {
        "account = ?1"
    };
    // Pre-filter: keep rows where the query appears anywhere in the sender or the
    // (JSON) recipient lists. With an empty query every row qualifies.
    let sql = format!(
        "SELECT from_name, from_addr, json_extract(json, '$.to'), json_extract(json, '$.cc')
         FROM messages
         WHERE {account_filter}
           AND uid <> 0
           AND (?2 = ''
                OR lower(COALESCE(from_addr, '')) LIKE ?3 ESCAPE '\\'
                OR lower(COALESCE(from_name, '')) LIKE ?3 ESCAPE '\\'
                OR lower(COALESCE(json_extract(json, '$.to'), '')) LIKE ?3 ESCAPE '\\'
                OR lower(COALESCE(json_extract(json, '$.cc'), '')) LIKE ?3 ESCAPE '\\')"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![account, q, like], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    // Tally per lowercased address: first display name seen wins (falling back to
    // a later non-empty one), plus an occurrence count for ranking.
    struct Tally {
        name: String,
        addr: String,
        count: u32,
    }
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    let mut bump = |name: String, addr: String| {
        let addr = addr.trim();
        if addr.is_empty() {
            return;
        }
        let entry = tallies.entry(addr.to_lowercase()).or_insert_with(|| Tally {
            name: String::new(),
            addr: addr.to_string(),
            count: 0,
        });
        entry.count += 1;
        if entry.name.is_empty() && !name.trim().is_empty() {
            entry.name = name.trim().to_string();
        }
    };

    for row in rows {
        let (from_name, from_addr, to_json, cc_json) = row?;
        bump(from_name, from_addr);
        for json in [to_json, cc_json].into_iter().flatten() {
            if let Ok(list) = serde_json::from_str::<Vec<crate::imap::Recipient>>(&json) {
                for r in list {
                    bump(r.name, r.addr);
                }
            }
        }
    }

    // Keep only the addresses that actually match the query (the SQL pre-filter
    // admits whole rows, so a sender match can drag in non-matching recipients).
    let mut out: Vec<Tally> = tallies
        .into_values()
        .filter(|t| {
            q.is_empty() || t.addr.to_lowercase().contains(&q) || t.name.to_lowercase().contains(&q)
        })
        .collect();
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.addr.to_lowercase().cmp(&b.addr.to_lowercase()))
    });
    out.truncate(limit as usize);
    Ok(out
        .into_iter()
        .map(|t| Contact {
            name: t.name,
            addr: t.addr,
        })
        .collect())
}

pub fn get_starred(
    conn: &Connection,
    account: &str,
    folder: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to') FROM messages
         WHERE account = ?1 AND folder = ?2 AND starred <> 0 AND uid <> 0
         ORDER BY date DESC, uid DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![account, folder, limit], message_header_from_row)?;
    let mut out = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    for header in &mut out {
        header.folder = folder.to_string();
    }
    Ok(out)
}

/// Every starred mail message across all accounts and folders, newest first.
/// RSS rows carry `uid = 0` and are excluded; `rss::starred_items` covers them.
pub fn get_starred_all_accounts(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<(String, MessageHeader)>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to'), account, folder FROM messages
         WHERE starred <> 0 AND uid <> 0
         ORDER BY date DESC, uid DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        let mut header = message_header_from_row(row)?;
        header.folder = row.get(10)?;
        Ok((row.get::<_, String>(9)?, header))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_thread_headers(
    conn: &Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key FROM messages
         WHERE account = ?1 AND folder = ?2 AND COALESCE(NULLIF(thread_key, ''), 'uid:' || uid) = ?3
         ORDER BY date ASC, uid ASC",
    )?;
    let rows = stmt.query_map(params![account, folder, thread_key], |row| {
        let uid = row.get(0)?;
        Ok(MessageHeader {
            uid,
            subject: row.get(1)?,
            from_name: row.get(2)?,
            from_addr: row.get(3)?,
            date: row.get(4)?,
            seen: row.get::<_, i64>(5)? != 0,
            starred: row.get::<_, i64>(6)? != 0,
            thread_key: row
                .get::<_, Option<String>>(7)?
                .filter(|key| !key.is_empty())
                .unwrap_or_else(|| format!("uid:{}", uid)),
            folder: String::new(),
            ..Default::default()
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Message-IDs a thread references but hasn't cached locally. Across every
/// cached row of `account` sharing `thread_key`, collect the union of ids they
/// reference (each row's `References` chain) plus the root id (`thread_key`
/// itself), then subtract the ids already present as cached messages. The
/// remainder is the ancestry the thread links to but that lies outside the
/// synced window — the on-demand fetch target. All ids are lowercased so the
/// comparison is case-insensitive (the typed `thread_key` column is lowercased
/// by the sync path while `json.references` preserves header casing).
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
/// `Message-ID`, only the lowest-`id` row. Rows without a `Message-ID` (e.g.
/// drafts) are never collapsed — a NULL `Message-ID` never equates in SQL.
pub fn get_thread_headers_all_folders(
    conn: &Connection,
    account: &str,
    thread_key: &str,
) -> Result<Vec<MessageHeader>> {
    let mut stmt = conn.prepare(
        "SELECT m.uid, m.folder, m.subject, m.from_name, m.from_addr, m.date, m.seen, m.starred, m.thread_key
         FROM messages m
         WHERE m.account = ?1 AND m.uid <> 0
           AND COALESCE(NULLIF(m.thread_key, ''), 'uid:' || m.uid) = ?2
           AND NOT EXISTS (
             SELECT 1 FROM messages dup
             WHERE dup.account = m.account
               AND COALESCE(NULLIF(dup.thread_key, ''), 'uid:' || dup.uid) = ?2
               AND COALESCE(json_extract(m.json, '$.message_id'), '') <> ''
               AND json_extract(dup.json, '$.message_id') = json_extract(m.json, '$.message_id')
               AND dup.id < m.id
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
            ..Default::default()
        })
    })?;
    // `date` is now an epoch integer, so the SQL ORDER BY sorts chronologically
    // (it could not when date was an RFC 2822 string). Unknown dates (0) sort first.
    let headers = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(headers)
}

fn message_header_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageHeader> {
    let uid = row.get(0)?;
    Ok(MessageHeader {
        uid,
        subject: row.get(1)?,
        from_name: row.get(2)?,
        from_addr: row.get(3)?,
        date: row.get(4)?,
        seen: row.get::<_, i64>(5)? != 0,
        starred: row.get::<_, i64>(6)? != 0,
        thread_key: row
            .get::<_, Option<String>>(7)?
            .filter(|key| !key.is_empty())
            .unwrap_or_else(|| format!("uid:{}", uid)),
        to: parse_recipients_json(row.get::<_, Option<String>>(8)?),
        folder: String::new(),
        ..Default::default()
    })
}

/// Parse a cached `$.to` JSON array into recipients, tolerating null/garbage.
fn parse_recipients_json(json: Option<String>) -> Vec<crate::imap::Recipient> {
    json.and_then(|s| serde_json::from_str::<Vec<crate::imap::Recipient>>(&s).ok())
        .unwrap_or_default()
}

fn escape_like(value: String) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub fn get_folder_state(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Option<(u32, u32)>> {
    let row = conn
        .query_row(
            "SELECT uidvalidity, uid_next FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![account, folder],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .ok();
    Ok(row.map(|(v, n)| (v.unwrap_or(0) as u32, n.unwrap_or(0) as u32)))
}

pub fn set_folder_state(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uid_next: u32,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_state(account, folder, uidvalidity, uid_next) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(account, folder) DO UPDATE SET
           uidvalidity = excluded.uidvalidity, uid_next = excluded.uid_next",
        params![account, folder, uidvalidity as i64, uid_next as i64],
    )?;
    Ok(())
}

pub fn get_folder_modseq(conn: &Connection, account: &str, folder: &str) -> Result<u64> {
    let modseq = conn
        .query_row(
            "SELECT highest_modseq FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![account, folder],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0);
    Ok(modseq as u64)
}

pub fn set_folder_modseq(
    conn: &Connection,
    account: &str,
    folder: &str,
    modseq: u64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_state(account, folder, highest_modseq) VALUES(?1, ?2, ?3)
         ON CONFLICT(account, folder) DO UPDATE SET highest_modseq = excluded.highest_modseq",
        params![account, folder, modseq as i64],
    )?;
    Ok(())
}

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

pub fn update_rss_thread_seen(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    seen: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?3 WHERE account = ?1 AND folder = ?2",
        params![account, subscription_id, seen as i64],
    )?;
    Ok(())
}

pub fn update_rss_thread_starred(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    starred: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET starred = ?3 WHERE account = ?1 AND folder = ?2",
        params![account, subscription_id, starred as i64],
    )?;
    Ok(())
}

pub fn update_rss_item_seen(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    item_key: &str,
    seen: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET seen = ?4 WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
        params![account, subscription_id, item_key, seen as i64],
    )?;
    Ok(())
}

pub fn update_rss_item_starred(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    item_key: &str,
    starred: bool,
) -> Result<()> {
    conn.execute(
        "UPDATE messages SET starred = ?4 WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
        params![account, subscription_id, item_key, starred as i64],
    )?;
    Ok(())
}

/// Delete locally cached messages whose UIDs are no longer on the server.
/// `server_uids` must be the complete UID set for the folder. Returns the
/// number of rows removed.
pub fn prune_missing_messages(
    conn: &Connection,
    account: &str,
    folder: &str,
    server_uids: &std::collections::HashSet<u32>,
) -> Result<usize> {
    let mut stmt = conn.prepare("SELECT uid FROM messages WHERE account = ?1 AND folder = ?2")?;
    let local_uids: Vec<u32> = stmt
        .query_map(params![account, folder], |row| row.get::<_, u32>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut removed = 0usize;
    for uid in local_uids {
        if !server_uids.contains(&uid) {
            conn.execute(
                "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![account, folder, uid],
            )?;
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn clear_folder_messages(conn: &Connection, account: &str, folder: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM messages WHERE account = ?1 AND folder = ?2",
        params![account, folder],
    )?;
    Ok(())
}

pub fn has_cached_body(conn: &Connection, account: &str, folder: &str, uid: u32) -> Result<bool> {
    let found = conn
        .query_row(
            "SELECT 1 FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid = ?3
               AND (body IS NOT NULL
                    OR json_extract(json, '$.body_html') IS NOT NULL)",
            params![account, folder, uid],
            |_| Ok(()),
        )
        .ok()
        .is_some();
    Ok(found)
}

pub fn has_message(conn: &Connection, account: &str, folder: &str, uid: u32) -> Result<bool> {
    let found = conn
        .query_row(
            "SELECT 1 FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
            |_| Ok(()),
        )
        .ok()
        .is_some();
    Ok(found)
}

/// Render a stored recipient list (JSON `[{name, addr}]`) as a comma-separated
/// "Name <addr>" / "addr" string. Empty/missing/malformed input yields "".
fn format_recipient_list(json: Option<&str>) -> String {
    let Some(s) = json.filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) else {
        return String::new();
    };
    arr.iter()
        .filter_map(|v| {
            let addr = v["addr"].as_str().unwrap_or_default().trim();
            if addr.is_empty() {
                return None;
            }
            let name = v["name"].as_str().unwrap_or_default().trim();
            Some(if name.is_empty() {
                addr.to_string()
            } else {
                format!("{name} <{addr}>")
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_recipient_value(value: Option<&Value>) -> String {
    value
        .map(Value::to_string)
        .map(|json| format_recipient_list(Some(&json)))
        .unwrap_or_default()
}

pub fn get_cached_message(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
) -> Result<Option<Message>> {
    let mut stmt = conn.prepare(
        "SELECT subject, from_name, from_addr, date, body, json
         FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
    )?;

    let row = stmt
        .query_row(params![account, folder, uid], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .ok();

    let Some((subject, from_name, from_addr, date, body, json_extra)) = row else {
        return Ok(None);
    };
    let extra: Value = serde_json::from_str(&json_extra).unwrap_or_else(|_| json!({}));

    let to = format_recipient_value(extra.get("to"));

    let reply_to = extra["reply_to"].as_str().unwrap_or_default().to_string();
    let cc = format_recipient_value(extra.get("cc"));
    let bcc = extra["bcc"].as_str().unwrap_or_default().to_string();
    let message_id = extra["message_id"].as_str().unwrap_or_default().to_string();
    let references = extra["references"].as_str().unwrap_or_default().to_string();
    let body_html = extra["body_html"].as_str().map(str::to_string);

    // `body` is the canonical plain-text body. For legacy HTML-only rows where it
    // is missing, fall back to rendering the stored HTML source once.
    let body = match body {
        Some(body) => body,
        None => match &body_html {
            Some(html) => {
                let rendered = crate::parse::render_body(html);
                let _ = conn.execute(
                    "UPDATE messages SET body = ?4 WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                    params![account, folder, uid, rendered],
                );
                rendered
            }
            None => return Ok(None),
        },
    };

    let attachments: Vec<Attachment> = extra
        .get("attachments")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();

    // Keep the HTML source on the returned message so the read handler can build
    // the iframe-ready view (it injects the remote-image CSP per account setting).
    Ok(Some(Message {
        subject,
        from_name,
        from_addr,
        to,
        reply_to,
        cc,
        bcc,
        message_id,
        references,
        date,
        body,
        body_html,
        body_is_rendered: extra["body_is_rendered"].as_bool().unwrap_or(false),
        preview: String::new(),
        attachments,
    }))
}

pub fn save_cached_message(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    message: &Message,
) -> Result<()> {
    let attachments_json = serde_json::to_value(&message.attachments).unwrap_or_else(|_| json!([]));
    // The `json` catch-all column carries header fields that don't have a typed
    // column. Envelope recipients are written by `upsert_messages`; body fetches
    // must not overwrite them.
    let extra_json = json!({
        "reply_to": message.reply_to,
        "bcc": message.bcc,
        "message_id": message.message_id,
        "references": message.references,
        "body_html": message.body_html,
        "body_is_rendered": message.body_is_rendered,
        "attachments": attachments_json,
    })
    .to_string();

    conn.execute(
        "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
         VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(account, folder, msg_id) DO UPDATE SET
           subject = excluded.subject,
           from_name = excluded.from_name,
           from_addr = excluded.from_addr,
           date = excluded.date,
           body = excluded.body,
           json = json_patch(messages.json, excluded.json)",
        params![
            account,
            folder,
            uid,
            message.subject,
            message.from_name,
            message.from_addr,
            message.date,
            message.body,
            extra_json
        ],
    )?;

    Ok(())
}

// ---- RSS items (shared messages table) --------------------------------------

/// Per-item RSS fields that don't map to a typed mail column; stored as the
/// message row's `json` JSON.
pub struct RssItemExtra {
    pub author: String,
    pub link: String,
    pub summary: String,
    pub content: String,
    /// Inline images lifted from the item's HTML. Each carries its source `url`
    /// and, once downloaded to the media dir, a local `key` served at `/media`.
    pub images: Vec<RssMedia>,
    /// Inline videos lifted from the item's HTML. Remote-only (rendered straight
    /// from their source `url`); not cached to disk, so `key` stays `None`.
    pub videos: Vec<RssMedia>,
    pub published_at: i64,
    pub updated_at: i64,
    pub fetched_at: i64,
}

/// One inline feed media item (image or video): its remote source and, when
/// cached locally, its media key served at `/media`.
pub struct RssMedia {
    pub url: String,
    pub key: Option<String>,
}

/// Upsert one RSS item as a message row. `folder` is the subscription id and
/// `msg_id` the stable item key; the feed-specific payload lives in `json`.
/// Updates preserve the existing `seen` flag (don't un-read on refetch).
pub fn upsert_rss_item(
    conn: &Connection,
    account: &str,
    subscription_id: &str,
    item_key: &str,
    title: &str,
    unread: bool,
    body_html: Option<&str>,
    extra: &RssItemExtra,
) -> Result<bool> {
    // Tell new arrivals apart from re-syncs of items we already stored, so callers
    // can surface a "new items" notification only for genuinely new entries.
    let is_new = conn
        .query_row(
            "SELECT 1 FROM messages WHERE account = ?1 AND folder = ?2 AND msg_id = ?3",
            params![account, subscription_id, item_key],
            |_| Ok(()),
        )
        .optional()?
        .is_none();
    let extra_json = json!({
        "author": extra.author,
        "link": extra.link,
        "summary": extra.summary,
        "content": extra.content,
        "body_html": body_html,
        "images": extra.images.iter()
            .map(|img| json!({ "url": img.url, "key": img.key }))
            .collect::<Vec<_>>(),
        "videos": extra.videos.iter()
            .map(|vid| json!({ "url": vid.url, "key": vid.key }))
            .collect::<Vec<_>>(),
        "published_at": extra.published_at,
        "updated_at": extra.updated_at,
        "fetched_at": extra.fetched_at,
    })
    .to_string();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key, json)
         VALUES(?1, ?2, ?3, 0, ?4, ?5, '', ?6, ?7, ?2, ?8)
         ON CONFLICT(account, folder, msg_id) DO UPDATE SET
           subject = excluded.subject,
           date    = excluded.date,
           json   = excluded.json",
        params![
            account,
            subscription_id,
            item_key,
            title,
            extra.author,
            item_date_epoch(extra.published_at, extra.updated_at, extra.fetched_at),
            (!unread) as i64,
            extra_json,
        ],
    )?;
    Ok(is_new)
}

/// Best available timestamp for an RSS item as epoch seconds (0 when none),
/// preferring published > updated > fetched. Stored in the `date` column so RSS
/// rows sort alongside mail.
fn item_date_epoch(published: i64, updated: i64, fetched: i64) -> i64 {
    if published != 0 {
        published
    } else if updated != 0 {
        updated
    } else {
        fetched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::run_migrations(&conn).unwrap();
        conn
    }

    fn insert_message(
        conn: &Connection,
        uid: u32,
        subject: &str,
        from_name: &str,
        from_addr: &str,
        body: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, body)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, ?4, ?5, 1779580800, 0, ?6)",
            params![uid.to_string(), uid, subject, from_name, from_addr, body],
        )
        .unwrap();
    }

    #[test]
    fn search_messages_matches_subject_sender_and_body_case_insensitively() {
        let conn = test_conn();
        insert_message(&conn, 1, "Quarterly Plan", "Aki", "aki@example.com", None);
        insert_message(&conn, 2, "Hello", "Launch Team", "team@example.com", None);
        insert_message(
            &conn,
            3,
            "Notes",
            "Ops",
            "ops@example.com",
            Some("The deploy window is confirmed."),
        );
        insert_message(&conn, 4, "Other", "No Match", "other@example.com", None);

        let subject = search_messages(&conn, "acct", "INBOX", "quarterly", 10).unwrap();
        assert_eq!(subject.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);

        let sender = search_messages(&conn, "acct", "INBOX", "launch", 10).unwrap();
        assert_eq!(sender.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);

        let body = search_messages(&conn, "acct", "INBOX", "DEPLOY", 10).unwrap();
        assert_eq!(body.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn search_messages_matches_substrings_including_cjk() {
        let conn = test_conn();
        insert_message(
            &conn,
            1,
            "Quarterly planning",
            "Aki",
            "aki@example.com",
            None,
        );
        insert_message(&conn, 2, "上海会议通知", "Aki", "aki@example.com", None);

        // Mid-word Latin substring (trigram, >= 3 chars).
        let latin = search_messages(&conn, "acct", "INBOX", "arter", 10).unwrap();
        assert_eq!(latin.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);

        // CJK substring with no surrounding word breaks (>= 3 chars -> FTS).
        let cjk = search_messages(&conn, "acct", "INBOX", "会议通", 10).unwrap();
        assert_eq!(cjk.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);

        let miss = search_messages(&conn, "acct", "INBOX", "zzz", 10).unwrap();
        assert!(miss.is_empty());
    }

    #[test]
    fn search_messages_short_query_falls_back_to_like() {
        // Queries below the trigram minimum (3 codepoints) must still match via
        // the LIKE fallback — important for 2-character CJK words.
        let conn = test_conn();
        insert_message(&conn, 1, "上海会议通知", "Aki", "aki@example.com", None);
        insert_message(&conn, 2, "Other", "Aki", "aki@example.com", None);

        let hit = search_messages(&conn, "acct", "INBOX", "上海", 10).unwrap();
        assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn search_messages_reindexes_on_body_update() {
        // The FTS triggers must follow a later body write, not just the initial insert.
        let conn = test_conn();
        insert_message(&conn, 1, "Notes", "Ops", "ops@example.com", None);
        assert!(
            search_messages(&conn, "acct", "INBOX", "deploy", 10)
                .unwrap()
                .is_empty()
        );

        conn.execute(
            "UPDATE messages SET body = 'deploy window confirmed' WHERE uid = 1",
            [],
        )
        .unwrap();

        let hit = search_messages(&conn, "acct", "INBOX", "deploy", 10).unwrap();
        assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn get_starred_all_accounts_spans_accounts_and_skips_rss_rows() {
        let conn = test_conn();
        let insert = |account: &str,
                      folder: &str,
                      uid: u32,
                      subject: &str,
                      date: i64,
                      starred: i64| {
            conn.execute(
                "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred)
                 VALUES(?1, ?2, ?3, ?4, ?5, 'Aki', 'aki@example.com', ?6, 1, ?7)",
                params![account, folder, uid.to_string(), uid, subject, date, starred],
            )
            .unwrap();
        };
        insert("a1", "INBOX", 1, "Starred newer", 1_767_312_000, 1); // 2026-01-02
        insert("a1", "INBOX", 2, "Not starred", 1_767_398_400, 0); // 2026-01-03
        insert("a2", "Sent", 5, "Starred older", 1_767_225_600, 1); // 2026-01-01
        // RSS row: uid = 0, must be excluded even when starred.
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen, starred)
             VALUES('rss-acct', 'feed-1', 'item-1', 0, 'Feed item', 0, 1, 1)",
            [],
        )
        .unwrap();

        let starred = get_starred_all_accounts(&conn, 10).unwrap();
        assert_eq!(
            starred
                .iter()
                .map(|(account, m)| (account.as_str(), m.folder.as_str(), m.uid))
                .collect::<Vec<_>>(),
            vec![("a1", "INBOX", 1), ("a2", "Sent", 5)]
        );
        assert!(starred.iter().all(|(_, m)| m.starred));
    }

    #[test]
    fn get_recent_page_can_return_only_unread_messages() {
        let conn = test_conn();
        insert_message(&conn, 2, "Unread older", "Aki", "aki@example.com", None);
        insert_message(&conn, 3, "Unread middle", "Aki", "aki@example.com", None);
        insert_message(&conn, 4, "Read middle", "Aki", "aki@example.com", None);
        insert_message(&conn, 5, "Unread newest", "Aki", "aki@example.com", None);
        conn.execute(
            "UPDATE messages SET seen = 1 WHERE account = 'acct' AND folder = 'INBOX' AND uid = 4",
            [],
        )
        .unwrap();

        // insert_message stamps every row with the same date, so the date-ordered
        // list ties break on uid DESC and the cursor carries that shared date.
        const D: i64 = 1779580800;
        let (all, all_cursor) = get_recent_page(&conn, "acct", "INBOX", 2, None, false).unwrap();
        assert_eq!(all.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![5, 4]);
        assert_eq!(all_cursor.as_deref(), Some(format!("date:{D}:4").as_str()));

        let (unread, unread_cursor) =
            get_recent_page(&conn, "acct", "INBOX", 2, None, true).unwrap();
        assert_eq!(unread.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![5, 3]);
        assert_eq!(
            unread_cursor.as_deref(),
            Some(format!("date:{D}:3").as_str())
        );
        assert!(unread.iter().all(|m| !m.seen));

        let (next_unread, next_cursor) =
            get_recent_page(&conn, "acct", "INBOX", 2, Some((D, 3)), true).unwrap();
        assert_eq!(
            next_unread.iter().map(|m| m.uid).collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(next_cursor, None);
    }

    #[test]
    fn suggest_contacts_dedupes_by_address_and_ranks_by_frequency() {
        let conn = test_conn();
        // aki appears twice → ranks above bea; the duplicate collapses to one row.
        insert_message(&conn, 1, "Hi", "Aki", "aki@example.com", None);
        insert_message(&conn, 2, "Re: Hi", "Aki", "aki@example.com", None);
        insert_message(&conn, 3, "Plan", "Bea", "bea@example.com", None);

        let top = suggest_contacts(&conn, "acct", "", 10).unwrap();
        assert_eq!(
            top.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
            vec!["aki@example.com", "bea@example.com"]
        );

        // Query matches on both address and display name, case-insensitively.
        let by_name = suggest_contacts(&conn, "acct", "BEA", 10).unwrap();
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name[0].addr, "bea@example.com");
        assert_eq!(by_name[0].name, "Bea");

        // A different account sees none of acct's correspondents.
        assert!(suggest_contacts(&conn, "other", "", 10).unwrap().is_empty());
    }

    #[test]
    fn suggest_contacts_includes_to_and_cc_recipients() {
        use crate::imap::{MessageHeader, Recipient};
        let conn = test_conn();
        // A sent message we authored: the useful contacts are its To/Cc, not us.
        let msg = MessageHeader {
            uid: 10,
            subject: "Lunch".into(),
            from_name: "Me".into(),
            from_addr: "me@example.com".into(),
            date: 1_779_580_800, // 2026-05-24
            to: vec![Recipient {
                name: "Cleo".into(),
                addr: "cleo@example.com".into(),
            }],
            cc: vec![Recipient {
                name: String::new(),
                addr: "dan@example.com".into(),
            }],
            ..Default::default()
        };
        upsert_messages(&conn, "acct", "Sent", &[msg]).unwrap();

        let cleo = suggest_contacts(&conn, "acct", "cleo", 10).unwrap();
        assert_eq!(cleo.len(), 1);
        assert_eq!(cleo[0].addr, "cleo@example.com");
        assert_eq!(cleo[0].name, "Cleo");

        // A Cc-only address with no display name is still suggested by address.
        let dan = suggest_contacts(&conn, "acct", "dan@", 10).unwrap();
        assert_eq!(
            dan.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
            vec!["dan@example.com"]
        );
    }

    #[test]
    fn apply_card_identity_rewrites_outbound_to_counterparty() {
        use crate::imap::{MessageHeader, Recipient};
        let conn = test_conn();
        conn.execute(
            "INSERT INTO accounts(id, engine, email, prefs)
             VALUES('acct', 'mail', 'me@example.com', '{\"aliases\":[{\"email\":\"alias@other.jp\"}]}')",
            [],
        )
        .unwrap();

        let recipient = |name: &str, addr: &str| Recipient {
            name: name.into(),
            addr: addr.into(),
        };
        let mut headers = vec![
            // Outbound from the primary address → shows first recipient, +1 overflow.
            MessageHeader {
                uid: 1,
                from_name: "Me".into(),
                from_addr: "me@example.com".into(),
                to: vec![
                    recipient("Cleo", "cleo@example.com"),
                    recipient("Dan", "dan@example.com"),
                ],
                ..Default::default()
            },
            // Outbound from an alias counts as self too.
            MessageHeader {
                uid: 2,
                from_name: "Me".into(),
                from_addr: "Alias@Other.JP".into(),
                to: vec![recipient("Cleo", "cleo@example.com")],
                ..Default::default()
            },
            // Inbound stays untouched.
            MessageHeader {
                uid: 3,
                from_name: "Cleo".into(),
                from_addr: "cleo@example.com".into(),
                ..Default::default()
            },
            // Outbound with no cached recipients (old row / Bcc-only) stays untouched.
            MessageHeader {
                uid: 4,
                from_name: "Me".into(),
                from_addr: "me@example.com".into(),
                ..Default::default()
            },
        ];
        apply_card_identity(&conn, "acct", &mut headers);

        assert_eq!(headers[0].from_name, "Cleo");
        assert_eq!(headers[0].from_addr, "cleo@example.com");
        assert_eq!(headers[0].recipient_overflow, 1);
        assert_eq!(headers[1].from_name, "Cleo");
        assert_eq!(headers[1].recipient_overflow, 0);
        assert_eq!(headers[2].from_name, "Cleo");
        assert_eq!(headers[2].from_addr, "cleo@example.com");
        assert_eq!(headers[3].from_name, "Me");
        assert_eq!(headers[3].from_addr, "me@example.com");
    }

    #[test]
    fn apply_card_identity_resolves_junk_display_names() {
        use crate::imap::{MessageHeader, Recipient};
        let conn = test_conn();
        conn.execute(
            "INSERT INTO accounts(id, engine, email) VALUES('acct', 'mail', 'me@example.com')",
            [],
        )
        .unwrap();
        // A cached inbound message carries the bot's proper display name.
        insert_message(&conn, 1, "Hi", "Austin Mentions", "bot@example.com", None);

        let mut headers = vec![
            // Outbound reply whose To name is the junk "addr addr" pattern.
            MessageHeader {
                uid: 2,
                from_name: "Me".into(),
                from_addr: "me@example.com".into(),
                to: vec![Recipient {
                    name: "bot@example.com bot@example.com".into(),
                    addr: "bot@example.com".into(),
                }],
                ..Default::default()
            },
            // Inbound with the same junk in From resolves too.
            MessageHeader {
                uid: 3,
                from_name: "bot@example.com bot@example.com".into(),
                from_addr: "bot@example.com".into(),
                ..Default::default()
            },
            // Junk name with no better name cached anywhere is cleared.
            MessageHeader {
                uid: 4,
                from_name: "stranger@example.com".into(),
                from_addr: "stranger@example.com".into(),
                ..Default::default()
            },
        ];
        apply_card_identity(&conn, "acct", &mut headers);

        assert_eq!(headers[0].from_name, "Austin Mentions");
        assert_eq!(headers[0].from_addr, "bot@example.com");
        assert_eq!(headers[1].from_name, "Austin Mentions");
        assert_eq!(headers[2].from_name, "");
    }

    #[test]
    fn save_cached_message_preserves_envelope_recipients_in_json() {
        use crate::imap::{MessageHeader, Recipient};
        let conn = test_conn();
        let header = MessageHeader {
            uid: 42,
            subject: "Hi".into(),
            from_name: "Sender".into(),
            from_addr: "sender@example.com".into(),
            date: 1_779_580_800, // 2026-05-24
            to: vec![Recipient {
                name: "Me".into(),
                addr: "me@example.com".into(),
            }],
            ..Default::default()
        };
        upsert_messages(&conn, "acct", "INBOX", &[header]).unwrap();

        save_cached_message(
            &conn,
            "acct",
            "INBOX",
            42,
            &Message {
                subject: "Hi".into(),
                from_name: "Sender".into(),
                from_addr: "sender@example.com".into(),
                body: "hello".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let contacts = suggest_contacts(&conn, "acct", "me@", 10).unwrap();
        assert_eq!(
            contacts.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
            vec!["me@example.com"]
        );
        let message = get_cached_message(&conn, "acct", "INBOX", 42)
            .unwrap()
            .unwrap();
        assert_eq!(message.to, "Me <me@example.com>");
    }

    #[test]
    fn run_migrations_creates_schema_and_bumps_version() {
        let conn = Connection::open_in_memory().unwrap();
        db::run_migrations(&conn).unwrap();

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 3);

        for table in [
            "accounts",
            "messages",
            "messages_fts",
            "folders",
            "folder_state",
            "subscriptions",
            "meta",
            "settings",
        ] {
            let exists = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |_| Ok(()),
                )
                .optional()
                .unwrap()
                .is_some();
            assert!(exists, "missing table: {table}");
        }

        // Re-running is a no-op: version is already current, so nothing reapplies.
        db::run_migrations(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 3);
    }

    #[test]
    fn prefs_resolve_engine_default_and_persist_without_clobbering() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::ACCOUNTS_DDL).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine) VALUES('rss-1', 'rss')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine) VALUES('mail-1', 'mail')",
            [],
        )
        .unwrap();

        // Unset: resolved by engine (RSS on, mail off).
        assert!(load_remote_images(&conn, "rss-1").unwrap());
        assert!(!load_remote_images(&conn, "mail-1").unwrap());
        assert!(!load_remote_images(&conn, "missing").unwrap());

        // A sibling pref must survive an images toggle (json_set is in-place).
        conn.execute(
            "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'mail-1'",
            [],
        )
        .unwrap();
        set_load_remote_images(&conn, "mail-1", true).unwrap();
        assert!(load_remote_images(&conn, "mail-1").unwrap());
        let prefs: String = conn
            .query_row("SELECT prefs FROM accounts WHERE id = 'mail-1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(
            prefs.contains("\"other\""),
            "sibling pref clobbered: {prefs}"
        );

        // Explicit override of the engine default round-trips.
        set_load_remote_images(&conn, "rss-1", false).unwrap();
        assert!(!load_remote_images(&conn, "rss-1").unwrap());
    }

    #[test]
    fn conversation_html_defaults_on_and_respects_explicit_overrides() {
        let conn = Connection::open_in_memory().unwrap();
        db::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, prefs) VALUES
                ('unset', 'mail', 'custom', '{}'),
                ('plain', 'mail', 'custom', '{\"conversation_html\":false}'),
                ('html', 'mail', 'custom', '{\"conversation_html\":true}')",
            [],
        )
        .unwrap();

        let accounts = list_accounts(&conn).unwrap();
        let by_id = |id: &str| {
            accounts
                .iter()
                .find(|account| account["id"] == id)
                .expect("account present")
        };

        assert_eq!(by_id("unset")["conversation_html"], true);
        assert_eq!(by_id("plain")["conversation_html"], false);
        assert_eq!(by_id("html")["conversation_html"], true);
    }

    #[test]
    fn chat_wallpaper_round_trips_and_can_be_cleared() {
        let conn = Connection::open_in_memory().unwrap();
        db::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
            [],
        )
        .unwrap();

        let wallpaper = json!({"kind":"preset","presetId":"grid"});
        set_account_pref_json(&conn, "acct", "chat_wallpaper", Some(wallpaper.clone())).unwrap();
        let accounts = list_accounts(&conn).unwrap();
        assert_eq!(accounts[0]["chat_wallpaper"], wallpaper);

        conn.execute(
            "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'acct'",
            [],
        )
        .unwrap();
        set_account_pref_json(&conn, "acct", "chat_wallpaper", None).unwrap();
        let prefs: String = conn
            .query_row("SELECT prefs FROM accounts WHERE id = 'acct'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let value: Value = serde_json::from_str(&prefs).unwrap();
        assert_eq!(value["chat_wallpaper"], Value::Null);
        assert_eq!(value["other"], 7);
    }

    #[test]
    fn rss_sync_interval_defaults_and_persists_without_clobbering() {
        let conn = Connection::open_in_memory().unwrap();
        db::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider) VALUES('rss-1', 'rss', 'rss')",
            [],
        )
        .unwrap();

        let accounts = list_accounts(&conn).unwrap();
        assert_eq!(
            accounts[0]["rss_sync_interval_minutes"],
            DEFAULT_RSS_SYNC_INTERVAL_MINUTES
        );

        conn.execute(
            "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'rss-1'",
            [],
        )
        .unwrap();
        set_account_pref_u64(&conn, "rss-1", "rss_sync_interval_minutes", 30).unwrap();

        let accounts = list_accounts(&conn).unwrap();
        assert_eq!(accounts[0]["rss_sync_interval_minutes"], 30);
        let prefs: String = conn
            .query_row("SELECT prefs FROM accounts WHERE id = 'rss-1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(
            prefs.contains("\"other\""),
            "sibling pref clobbered: {prefs}"
        );
    }

    #[test]
    fn search_messages_returns_empty_for_blank_query() {
        let conn = test_conn();
        insert_message(&conn, 1, "Hello", "Aki", "aki@example.com", None);

        let results = search_messages(&conn, "acct", "INBOX", "  ", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn get_thread_headers_all_folders_sorts_chronologically_by_parsed_date() {
        let conn = test_conn();
        // Insert via the real RFC 2822 parser (the ingestion path), so this also
        // covers parse_date_to_epoch; ordering is then done by the stored epoch.
        let d = |s: &str| crate::parse::parse_date_to_epoch(s);
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
             VALUES('acct', 'Sent', '2', 2, 'Re: Topic', 'Me', 'me@example.com', ?1, 1, 'topic-key'),
                   ('acct', 'INBOX', '3', 3, 'Re: Topic', 'Them', 'them@example.com', ?2, 1, 'topic-key'),
                   ('acct', 'INBOX', '1', 1, 'Topic', 'Them', 'them@example.com', ?3, 1, 'topic-key')",
            params![
                d("Fri, 22 May 2026 21:05:20 -0700"),
                d("Fri, 22 May 2026 21:13:27 -0700"),
                d("Thu, 21 May 2026 22:02:02 -0700"),
            ],
        )
        .unwrap();

        let headers = get_thread_headers_all_folders(&conn, "acct", "topic-key").unwrap();
        let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
        assert_eq!(uids, vec![1, 2, 3]);
    }

    #[test]
    fn get_thread_headers_all_folders_dedupes_self_sent_by_message_id() {
        let conn = test_conn();
        // A message sent to yourself lands in both Sent and the Inbox with the
        // same RFC Message-ID but distinct per-folder UIDs. The thread view must
        // collapse the pair into one bubble.
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, thread_key, json)
             VALUES('acct', '[Gmail]/Sent Mail', '462', 462, 'test', 'Me', 'me@example.com', 100, 'self-key',
                      '{\"message_id\":\"mid@host\"}'),
                   ('acct', 'INBOX', '1440', 1440, 'test', 'Me', 'me@example.com', 100, 'self-key',
                      '{\"message_id\":\"mid@host\"}'),
                   ('acct', 'INBOX', '1441', 1441, 'reply', 'Them', 'them@example.com', 200, 'self-key',
                      '{\"message_id\":\"other@host\"}')",
            [],
        )
        .unwrap();

        let headers = get_thread_headers_all_folders(&conn, "acct", "self-key").unwrap();
        let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
        // The self-sent pair collapses to the lowest-id row (the Sent copy,
        // inserted first); the distinct reply stays.
        assert_eq!(uids, vec![462, 1441]);
    }

    #[test]
    fn get_thread_headers_all_folders_keeps_rows_without_message_id() {
        let conn = test_conn();
        // Rows lacking a Message-ID (e.g. drafts) must never be collapsed into
        // each other — a NULL Message-ID never equates in SQL.
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, thread_key, json)
             VALUES('acct', 'Drafts', '10', 10, 'draft a', 'Me', 'me@example.com', 100, 'k', '{}'),
                   ('acct', 'Drafts', '11', 11, 'draft b', 'Me', 'me@example.com', 200, 'k', '{}')",
            [],
        )
        .unwrap();

        let headers = get_thread_headers_all_folders(&conn, "acct", "k").unwrap();
        let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
        assert_eq!(uids, vec![10, 11]);
    }

    #[test]
    fn thread_reference_gaps_are_referenced_ids_not_yet_cached() {
        let conn = test_conn();
        // A draft replying into a thread: thread_key is the (lowercased) root id;
        // its json carries its own Message-ID and the full References chain. The
        // referenced ancestors aren't cached, so they're all gaps — except the
        // one ancestor we also store as a cached row below.
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, thread_key, json)
             VALUES('acct', 'Drafts', '99', 99, 'Re: Hi', 0, 'root@h',
               '{\"message_id\":\"leaf@h\",\"references\":\"root@h MID-ONE@h mid-two@h\"}')",
            [],
        )
        .unwrap();
        // An already-cached ancestor (its own Message-ID present) must be excluded.
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, thread_key, json)
             VALUES('acct', 'INBOX', '50', 50, 'Hi', 0, 'root@h',
               '{\"message_id\":\"mid-two@h\",\"references\":\"root@h\"}')",
            [],
        )
        .unwrap();

        let mut gaps = get_thread_reference_gaps(&conn, "acct", "root@h").unwrap();
        gaps.sort();
        // root@h and mid-one@h are referenced but uncached; mid-two@h is present;
        // leaf@h is a Message-ID, never a gap. Comparison is case-insensitive.
        assert_eq!(gaps, vec!["mid-one@h".to_string(), "root@h".to_string()]);
    }

    #[test]
    fn body_cache_invalidation_works() {
        let conn = test_conn();

        // Save an HTML-only message (body_is_rendered = true)
        let html = "<p>rendered plain</p>";
        let msg_html = Message {
            subject: "HTML only".into(),
            body: "stale render".into(),
            body_html: Some(html.into()),
            body_is_rendered: true,
            ..Default::default()
        };
        save_cached_message(&conn, "acct", "INBOX", 101, &msg_html).unwrap();

        // Save a plain-text message (body_is_rendered = false)
        let msg_plain = Message {
            subject: "Plain only".into(),
            body: "original plain".into(),
            body_is_rendered: false,
            ..Default::default()
        };
        save_cached_message(&conn, "acct", "INBOX", 102, &msg_plain).unwrap();

        // Verify they are initially saved with their bodies
        let retrieved_html = get_cached_message(&conn, "acct", "INBOX", 101)
            .unwrap()
            .unwrap();
        assert_eq!(retrieved_html.body, "stale render");
        assert!(retrieved_html.body_is_rendered);

        let retrieved_plain = get_cached_message(&conn, "acct", "INBOX", 102)
            .unwrap()
            .unwrap();
        assert_eq!(retrieved_plain.body, "original plain");
        assert!(!retrieved_plain.body_is_rendered);

        // Force cache invalidation by setting the meta version in DB to an older one
        db::meta_set(&conn, "body_cache_version", "old_version").unwrap();

        // Call the invalidator
        db::invalidate_body_cache_if_needed(&conn).unwrap();

        // The html-only body is re-rendered in place (not nulled), so the FTS
        // `body` column stays populated across the bump.
        let html_body_in_db: Option<String> = conn
            .query_row("SELECT body FROM messages WHERE uid = 101", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(html_body_in_db, Some(crate::parse::render_body(html)));

        // The plain-only body is left untouched.
        let plain_body_in_db: Option<String> = conn
            .query_row("SELECT body FROM messages WHERE uid = 102", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(plain_body_in_db, Some("original plain".to_string()));

        // FTS still matches the re-rendered html-only message after the bump.
        let hits = search_messages(&conn, "acct", "INBOX", "rendered", 10).unwrap();
        assert!(hits.iter().any(|m| m.subject == "HTML only"));
    }
}
