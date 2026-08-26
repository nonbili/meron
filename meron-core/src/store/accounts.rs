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
/// How an account picks the signature appended to its outgoing mail: follow the
/// app-wide signature (the default when the pref is absent), send nothing, or
/// use this account's own `html`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AccountSignature {
    pub mode: SignatureMode,
    #[serde(default)]
    pub html: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignatureMode {
    #[default]
    Global,
    None,
    Custom,
}

impl SignatureMode {
    /// Parse a bridge-supplied mode string; unknown values are rejected so a
    /// typo can't silently disable a user's signature.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "global" => Some(Self::Global),
            "none" => Some(Self::None),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Upper bound on stored signature HTML. Generous enough for a rich signature
/// with a logo, small enough that a runaway paste can't bloat the prefs column.
pub const MAX_SIGNATURE_HTML: usize = 256 * 1024;

impl AccountSignature {
    /// Validate a bridge `signature` param. `None`/null clears the override so
    /// the account falls back to the app-wide signature; an object is stored
    /// as-is (the `html` is kept even in `global`/`none` mode, so toggling the
    /// mode back doesn't lose what the user wrote).
    pub fn from_param(value: Option<&Value>) -> Result<Option<Self>, String> {
        let value = match value {
            None | Some(Value::Null) => return Ok(None),
            Some(value) => value,
        };
        let obj = value
            .as_object()
            .ok_or_else(|| "signature must be an object".to_string())?;
        let mode = obj
            .get("mode")
            .and_then(Value::as_str)
            .and_then(SignatureMode::parse)
            .ok_or_else(|| "unknown signature mode".to_string())?;
        let html = obj.get("html").and_then(Value::as_str).unwrap_or_default();
        if html.len() > MAX_SIGNATURE_HTML {
            return Err("signature is too large".to_string());
        }
        Ok(Some(Self {
            mode,
            html: html.trim().to_string(),
        }))
    }
}

// ---- Accounts ---------------------------------------------------------------

/// A send-as identity for an account: an address the user owns and an optional
/// From display name. A blank `name` falls back to the account's `sender_name`
/// when composing. Purely local — meron cannot check whether the provider will
/// accept the address, and providers that authorize senders themselves (Gmail's
/// "Send mail as") rewrite the `From` of anything they haven't verified.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Alias {
    pub email: String,
    #[serde(default)]
    pub name: String,
}

/// Deserialize one pref leniently: a value serde cannot read into `T` — hand
/// edited, restored from a newer build, corrupted — becomes `None` (so that
/// pref falls back to its default) instead of failing the whole object and
/// silently resetting every *other* pref alongside it.
fn lenient_pref<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

/// User-editable per-account preferences, stored as the `prefs` JSON column.
/// Every field is optional so unset means "use the default" (resolved in code,
/// e.g. by engine); add a new pref by adding a field — no schema migration.
/// Every field also decodes leniently (see [`lenient_pref`]), so one unreadable
/// pref cannot take the rest of the account's settings down with it.
#[derive(Default, Deserialize)]
#[serde(default)]
struct AccountPrefs {
    /// Whether remote inline images load; `None` = engine default (RSS on, mail off).
    #[serde(deserialize_with = "lenient_pref")]
    load_remote_images: Option<bool>,
    /// Whether this account's inbox folds into the unified inbox; default on.
    #[serde(deserialize_with = "lenient_pref")]
    included_in_unified: Option<bool>,
    /// Whether new-mail desktop notifications are suppressed; default off.
    #[serde(deserialize_with = "lenient_pref")]
    muted: Option<bool>,
    /// Whether automatic mail/feed checking is paused; default off.
    #[serde(deserialize_with = "lenient_pref")]
    paused: Option<bool>,
    /// Whether conversation bubbles render original HTML when available; default on.
    #[serde(deserialize_with = "lenient_pref")]
    conversation_html: Option<bool>,
    /// Whether Meron uploads its own Sent copy after SMTP send. None = provider default.
    #[serde(deserialize_with = "lenient_pref")]
    save_sent_copy: Option<bool>,
    /// RSS automatic sync interval in minutes; default 60.
    #[serde(deserialize_with = "lenient_pref")]
    rss_sync_interval_minutes: Option<u64>,
    /// Send-as identities (besides the primary address); default none.
    #[serde(deserialize_with = "lenient_pref")]
    aliases: Option<Vec<Alias>>,
    /// Per-account chat background; unset uses the app's default wallpaper.
    #[serde(deserialize_with = "lenient_pref")]
    chat_wallpaper: Option<ChatWallpaper>,
    /// Signature override; unset follows the app-wide signature setting.
    #[serde(deserialize_with = "lenient_pref")]
    signature: Option<AccountSignature>,
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

    /// The signature override as JSON for the bridge; null means "follow the
    /// app-wide signature".
    fn signature_json(&self) -> serde_json::Value {
        json!(self.signature)
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
        "proxy": creds.proxy.to_json(),
        "cert_pin": creds.cert_pin,
        "smtp_cert_pin": creds.smtp_cert_pin,
    })
    .to_string()
}

/// Pins are stored as hex strings; absent, null and empty all mean "no pin"
/// (accounts written before pinning existed have no key at all).
fn config_cert_pin(config: &serde_json::Value, key: &str) -> Option<String> {
    config[key]
        .as_str()
        .map(str::trim)
        .filter(|pin| !pin.is_empty())
        .map(|pin| pin.to_ascii_lowercase())
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
        // Accounts written before proxy support have no key here, which parses
        // as "follow the app-wide setting".
        proxy: crate::proxy::ProxyChoice::from_json(&v["proxy"]),
        cert_pin: config_cert_pin(&v, "cert_pin"),
        smtp_cert_pin: config_cert_pin(&v, "smtp_cert_pin"),
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
           -- An empty avatar means unspecified, not cleared: the account edit
           -- paths (server settings, reconnect) re-run this upsert without one
           -- and would otherwise wipe a custom avatar. account.setAvatar is how
           -- an avatar is set or cleared.
           avatar_url   = CASE WHEN excluded.avatar_url = '' THEN accounts.avatar_url
                               ELSE excluded.avatar_url END,
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

/// Replace just the proxy entry in an account's connection `config`, leaving
/// the rest of the JSON (hosts, ports, OAuth metadata) untouched.
pub fn set_account_proxy(
    conn: &Connection,
    id: &str,
    choice: &crate::proxy::ProxyChoice,
) -> Result<()> {
    conn.execute(
        "UPDATE accounts
         SET config = json_set(config, '$.proxy', json(?2)), updated_at = ?3
         WHERE id = ?1",
        params![id, choice.to_json().to_string(), now_unix()],
    )?;
    Ok(())
}

/// Replace just the certificate pins in an account's connection `config`. Used
/// when the user accepts a server certificate for an account that already
/// exists, so the pin lands without re-entering the password. `None` clears the
/// pin for that server.
pub fn set_account_cert_pins(
    conn: &Connection,
    id: &str,
    cert_pin: Option<&str>,
    smtp_cert_pin: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE accounts
         SET config = json_set(config, '$.cert_pin', ?2, '$.smtp_cert_pin', ?3), updated_at = ?4
         WHERE id = ?1",
        params![id, cert_pin, smtp_cert_pin, now_unix()],
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

/// Resolve the `From` address and display name for an outgoing message, shared
/// by desktop and mobile so both honor the same send-as rules.
///
/// An account may send as its own address, as the mailbox login, or as any
/// configured alias; an empty request means "the account's own address". The
/// account address and the login are separate fields (a custom IMAP account
/// carries both, and they differ whenever the server authenticates by user
/// name), so both count as identities the user owns.
///
/// An address that is none of those is an error rather than a silent
/// substitution: quietly rewriting the sender is how a message goes out as the
/// wrong identity without anyone noticing. Note that a provider can still
/// override the `From` we transmit — Gmail rewrites it to the authenticated
/// address unless the alias is verified in its own settings — which is outside
/// what this can enforce.
pub fn resolve_send_from(
    conn: &Connection,
    id: &str,
    login_user: &str,
    requested_from: &str,
) -> Result<(String, String)> {
    let (email, sender_name) = conn
        .query_row(
            "SELECT email, sender_name FROM accounts WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            },
        )
        .optional()?
        .unwrap_or_default();

    let login = login_user.trim();
    // Mirrors `account.list`, which falls back to the login when the account
    // was stored without an explicit address.
    let primary = if email.trim().is_empty() {
        login
    } else {
        email.trim()
    };

    let requested = requested_from.trim().to_lowercase();
    if requested.is_empty() || requested == primary.to_lowercase() {
        return Ok((primary.to_string(), sender_name));
    }
    if requested == login.to_lowercase() {
        return Ok((login.to_string(), sender_name));
    }
    match account_aliases(conn, id)?
        .into_iter()
        .find(|alias| alias.email.trim().to_lowercase() == requested)
    {
        Some(alias) => {
            let name = if alias.name.trim().is_empty() {
                sender_name
            } else {
                alias.name
            };
            Ok((alias.email.trim().to_string(), name))
        }
        None => Err(anyhow::anyhow!(
            "{} is not an address this account can send from",
            requested_from.trim()
        )),
    }
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

/// Whether a cached message is one this account sent: it lives in a Sent mailbox
/// — a copy filed there is outbound by definition, even when it was sent from an
/// alias meron doesn't know about (e.g. a webmail send-as) — or its From is one
/// of the account's own addresses (primary or configured send-as alias).
///
/// The address match alone can't settle direction: a shared address configured
/// here as an alias is also used by colleagues, whose mail then looks like ours.
/// `delivered` (see [`crate::parse::Message::delivered`]) is the veto — a copy
/// carrying delivery headers was received, not written here — and unlike the
/// mailbox it sits in, it stays true after the message is archived or moved.
/// Pass false when the body isn't cached and the headers are unknown.
///
/// `mine` comes from [`self_addrs`]; `folder` is the message's source mailbox.
pub fn is_outgoing(
    mine: &std::collections::HashSet<String>,
    folder: &str,
    from_addr: &str,
    delivered: bool,
) -> bool {
    if crate::imap::looks_like_sent(folder) {
        return true;
    }
    if delivered {
        return false;
    }
    let from = from_addr.trim().to_lowercase();
    !from.is_empty() && mine.contains(&from)
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
        // Envelope headers carry no delivery headers, so the thread card keeps
        // the address match; the reader and reply paths read the cached body.
        if is_outgoing(&mine, source_folder, &header.from_addr, false)
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
    tx.execute(
        "DELETE FROM mail_search_hits WHERE account = ?1",
        params![id],
    )?;
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

/// One mail account's stored connection creds, or `None` when it does not
/// exist yet. Used when saving an account has to preserve settings the caller
/// did not resend (see [`crate::imap::Creds::preserving`]).
pub fn load_account(conn: &Connection, id: &str) -> Result<Option<Creds>> {
    let config: Option<String> = conn
        .query_row(
            "SELECT config FROM accounts WHERE id = ?1 AND engine = 'mail'",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(config.as_deref().map(config_to_creds))
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
                // The login name, which is not always the address: editors that
                // resend the account must preserve it rather than assume email.
                "username": c.user,
                "imap_host": c.host,
                "imap_port": c.port,
                "smtp_host": c.smtp_host,
                "smtp_port": c.smtp_port,
                "tls": c.tls,
                "starttls": c.starttls,
                "smtp_tls": c.smtp_tls,
                "smtp_starttls": c.smtp_starttls,
                "load_remote_images": load_remote_images,
                "included_in_unified": included_in_unified,
                "muted": muted,
                "paused": paused,
                "conversation_html": conversation_html,
                "save_sent_copy": save_sent_copy,
                "chat_wallpaper": chat_wallpaper,
                "sort_order": sort_order,
                "aliases": p.aliases_json(),
                "signature": p.signature_json(),
                "proxy": c.proxy.to_json(),
                "cert_pin": c.cert_pin,
                "smtp_cert_pin": c.smtp_cert_pin,
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

/// A sender address as the allowlist stores and compares it: trimmed, unwrapped
/// from any `Name <addr>` form, lowercased.
pub fn normalize_sender(addr: &str) -> String {
    let addr = addr.trim();
    let addr = match (addr.rfind('<'), addr.rfind('>')) {
        (Some(open), Some(close)) if close > open => &addr[open + 1..close],
        _ => addr,
    };
    addr.trim().to_ascii_lowercase()
}

/// The effective remote-content rule for reading an account: its own toggle
/// plus the app-wide sender allowlist. Resolved once per read so baking a whole
/// thread needs a single database round trip.
#[derive(Debug, Default, Clone)]
pub struct RemoteImagePolicy {
    /// The account-wide "load remote images" toggle.
    pub all: bool,
    /// App-wide allowed senders, normalized by [`normalize_sender`].
    pub senders: Vec<String>,
}

impl RemoteImagePolicy {
    /// Whether a message from `from_addr` may load its remote content.
    pub fn allows(&self, from_addr: &str) -> bool {
        self.all || {
            let addr = normalize_sender(from_addr);
            !addr.is_empty() && self.senders.contains(&addr)
        }
    }
}

/// The resolved remote-content policy for an account: its own toggle plus the
/// app-wide allowlist. Unknown accounts deny, but still carry the allowlist.
pub fn remote_image_policy(conn: &Connection, id: &str) -> Result<RemoteImagePolicy> {
    Ok(RemoteImagePolicy {
        all: load_remote_images(conn, id)?,
        senders: super::remote_image_senders(conn)?,
    })
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

/// Friendly display name for an account in user-facing notifications: its email
/// address, else its display name, else the bare id for an account row that is
/// missing or carries neither (RSS accounts have no email, so they show their
/// display name).
pub fn account_label(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT display_name, email FROM accounts WHERE id = ?1",
        params![id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .ok()
    .and_then(|(display_name, email)| {
        let email = email.trim();
        if !email.is_empty() {
            return Some(email.to_string());
        }
        let display_name = display_name.trim();
        if !display_name.is_empty() {
            return Some(display_name.to_string());
        }
        None
    })
    .unwrap_or_else(|| id.to_string())
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
