use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::imap::{Creds, MessageHeader};

use super::DEFAULT_RSS_SYNC_INTERVAL_MINUTES;
use super::db::now_unix;

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
    /// Whether Meron uploads its own Sent copy after SMTP send. None = provider default.
    save_sent_copy: Option<bool>,
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

    fn save_sent_copy(&self) -> Option<bool> {
        self.save_sent_copy
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
        "oauth_client_id": creds.oauth_client_id,
        "oauth_client_secret": creds.oauth_client_secret,
        "oauth_token_url": creds.oauth_token_url,
        "oauth_scope": creds.oauth_scope,
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
        oauth_client_id: v["oauth_client_id"].as_str().unwrap_or("").to_string(),
        oauth_client_secret: v["oauth_client_secret"].as_str().unwrap_or("").to_string(),
        oauth_token_url: v["oauth_token_url"].as_str().unwrap_or("").to_string(),
        oauth_scope: v["oauth_scope"].as_str().unwrap_or("").to_string(),
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

/// Whether a cached message is one this account sent: its From is one of the
/// account's own addresses (primary or configured send-as alias), or it lives
/// in a Sent mailbox — a copy filed there is outbound by definition, even when
/// it was sent from an alias meron doesn't know about (e.g. a webmail send-as).
/// `mine` comes from [`self_addrs`]; `folder` is the message's source mailbox.
pub fn is_outgoing(
    mine: &std::collections::HashSet<String>,
    folder: &str,
    from_addr: &str,
) -> bool {
    let from = from_addr.trim().to_lowercase();
    (!from.is_empty() && mine.contains(&from)) || crate::imap::looks_like_sent(folder)
}

/// Rewrite the thread-card identity to show *the other party*: for messages this
/// account sent (see [`is_outgoing`]), replace `from_name`/`from_addr` with the
/// first recipient and record the count of additional recipients in
/// `recipient_overflow`. Inbound messages — and outbound ones with no usable
/// recipient (e.g. Bcc-only) — are left untouched. Display-only; these headers feed
/// the thread list, not the real message envelope. `folder` is the mailbox the
/// list was read from, used when a row doesn't carry its own source folder.
///
/// Junk display names (empty, or containing the address itself — some bots send
/// `From: "addr addr" <addr>`, and replies copy that into `To:`) are replaced by
/// the best name seen for that address anywhere in the cache, so the same thread
/// shows the same correspondent name in every folder.
pub fn apply_card_identity(
    conn: &Connection,
    account: &str,
    folder: &str,
    headers: &mut [MessageHeader],
) {
    let mine = self_addrs(conn, account);
    let mut name_cache: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for header in headers {
        let source_folder = if header.folder.is_empty() {
            folder
        } else {
            header.folder.as_str()
        };
        if is_outgoing(&mine, source_folder, &header.from_addr)
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
        "DELETE FROM observed_mail_identities WHERE account = ?1",
        params![id],
    )?;
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
        let save_sent_copy = p.save_sent_copy();
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
                "save_sent_copy": save_sent_copy,
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
                "save_sent_copy": save_sent_copy,
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

/// Explicit Sent-copy override for an account. None means use provider default.
pub fn save_sent_copy_pref(conn: &Connection, id: &str) -> Result<Option<bool>> {
    Ok(account_prefs(conn, id)?.and_then(|p| p.save_sent_copy()))
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
