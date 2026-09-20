//! Config-only backup and restore.
//!
//! A backup carries everything that would be tedious to retype — accounts and
//! their connection settings, per-account prefs, RSS subscriptions, and the
//! app-wide `settings` rows — but no cached mail. Messages, bodies and folder
//! sync state are reproducible from the server, so leaving them out keeps the
//! file small enough to mail to yourself and portable between desktop and
//! mobile, which do not share a database layout beyond these tables.
//!
//! Secrets (IMAP password, OAuth tokens) are opt-in, and asking for them forces
//! encryption: [`export`] refuses to emit a plaintext file containing them. The
//! secret store differs per platform (OS keychain on desktop, the keyed DB on
//! mobile), so the caller passes accessor closures rather than this module
//! reaching for either one.

use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::num::NonZeroU32;

use crate::secrets::Secrets;
use crate::store;

/// Backup file format version. Bump on any breaking change to the `data`
/// payload; [`import`] refuses anything newer than it understands.
pub const FORMAT_VERSION: u32 = 1;

/// PBKDF2-HMAC-SHA256 rounds. The OWASP 2023 floor for this PRF; a backup is
/// unlocked once, by hand, so a ~0.5s derivation is not felt.
const KDF_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Ceiling on the iteration count [`parse`] will honour from a file.
///
/// The count is attacker-controlled: it is read from the envelope *before* the
/// passphrase can be checked, so a crafted backup could otherwise ask for
/// billions of rounds and wedge the thread doing the derivation (on mobile,
/// that is the app). An order of magnitude above what we write leaves room to
/// raise [`KDF_ITERATIONS`] later while keeping the worst case a few seconds.
const MAX_KDF_ITERATIONS: u64 = 10_000_000;

/// Bound into every ciphertext as additional data, so a file whose envelope was
/// edited to claim a different format or cipher fails to open rather than
/// silently decrypting under the reader's assumptions.
const AAD: &[u8] = b"meron-backup-v1";

/// Who wrote a backup file, stamped into the envelope by [`export`].
///
/// Both fields come from the host because core knows neither: its own crate
/// version is not the app's, and the same core runs under every host.
/// [`platform`](Self::platform) is what makes [`app_version`](Self::app_version)
/// readable — desktop and mobile version independently (desktop 0.2.9 against
/// mobile 0.2.4 at the time of writing), so a bare number says nothing about
/// which build wrote the file. Use `desktop`, `android` or `ios`; a blank field
/// is left out of the envelope rather than recorded as an empty string.
#[derive(Clone, Copy, Debug, Default)]
pub struct Host<'a> {
    pub platform: &'a str,
    pub app_version: &'a str,
}

// ---- Backup payload ---------------------------------------------------------

/// One RSS subscription. `id` is re-derived from the URL on import, and item
/// caches / sync bookkeeping (etag, last_sync_at) are deliberately dropped —
/// a restored feed re-fetches from scratch.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BackupSubscription {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub site_url: String,
    #[serde(default)]
    pub feed_title: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// One account, as stored minus its cached mail. `config` and `prefs` are the
/// parsed forms of the two JSON columns, kept as free-form `Value` so a backup
/// written by a newer build round-trips fields this one does not know about.
///
/// Deliberately not `Debug`: it can hold a password, and the whole point of
/// keeping `Secrets` un-printable is lost if a wrapper re-exposes it.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BackupAccount {
    pub id: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub avatar_url: String,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub prefs: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subscriptions: Vec<BackupSubscription>,
    /// Present only in an encrypted backup exported with secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Secrets>,
}

/// One task list and its items. Tasks are local-only — no server holds a copy —
/// so unlike cached mail they cannot be re-fetched, which is why the config
/// backup carries them in full rather than a pointer.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BackupTaskList {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub tasks: Vec<BackupTask>,
}

/// One task, retaining its identity across exports and repeated imports.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BackupTask {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub due_at: i64,
    #[serde(default)]
    pub completed_at: i64,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub created_at: i64,
    /// The thread this task came from, empty for a task typed from scratch.
    /// Restored as-is: if the account it names isn't on this device the link is
    /// simply dead, which is better than dropping the task.
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub message_id: String,
}

/// The decrypted body of a backup. Not `Debug`, for the same reason as
/// [`BackupAccount`].
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct BackupData {
    #[serde(default)]
    pub accounts: Vec<BackupAccount>,
    /// The `settings` table, key -> parsed JSON value.
    #[serde(default)]
    pub settings: Map<String, Value>,
    /// Absent in files written before Tasks existed, hence `default`. That is
    /// also why [`FORMAT_VERSION`] does not move: an older build reading a
    /// newer file ignores the field rather than failing on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_lists: Vec<BackupTaskList>,
}

/// What [`import`] changed, for the "restored N accounts" confirmation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ImportSummary {
    pub accounts: u32,
    pub skipped: u32,
    pub feeds: u32,
    pub settings: u32,
    pub secrets: u32,
    pub tasks: u32,
}

impl ImportSummary {
    pub fn to_json(&self) -> Value {
        json!({
            "accounts": self.accounts,
            "skipped": self.skipped,
            "feeds": self.feeds,
            "settings": self.settings,
            "secrets": self.secrets,
            "tasks": self.tasks,
        })
    }
}

// ---- Export -----------------------------------------------------------------

/// Read every account, its subscriptions and the settings table into a
/// [`BackupData`]. `load_secrets` is consulted only when `include_secrets`.
pub fn collect(
    conn: &Connection,
    include_secrets: bool,
    load_secrets: &dyn Fn(&str) -> Secrets,
) -> Result<BackupData> {
    let mut stmt = conn.prepare(
        "SELECT id, engine, provider, email, display_name, avatar_url, sender_name, config, prefs, sort_order
         FROM accounts ORDER BY sort_order, id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BackupAccount {
                id: row.get(0)?,
                engine: row.get(1)?,
                provider: row.get(2)?,
                email: row.get(3)?,
                display_name: row.get(4)?,
                avatar_url: row.get(5)?,
                sender_name: row.get(6)?,
                config: parse_json_column(&row.get::<_, String>(7)?),
                prefs: parse_json_column(&row.get::<_, String>(8)?),
                sort_order: row.get(9)?,
                subscriptions: Vec::new(),
                secrets: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let mut accounts = Vec::with_capacity(rows.len());
    for mut account in rows {
        if account.engine == "rss" {
            account.subscriptions = collect_subscriptions(conn, &account.id)?;
        }
        if include_secrets {
            let secrets = load_secrets(&account.id);
            if !secrets.is_empty() {
                account.secrets = Some(secrets);
            }
        } else {
            redact_account_config(&mut account.config);
        }
        accounts.push(account);
    }

    let mut settings = collect_settings(conn)?;
    if !include_secrets {
        redact_settings(&mut settings);
    }

    Ok(BackupData {
        accounts,
        settings,
        task_lists: collect_task_lists(conn)?,
    })
}

/// Read every task list and its items. Tasks hold nothing secret — they are
/// whatever the user typed — so unlike account config they are not redacted
/// from a plaintext export.
fn collect_task_lists(conn: &Connection) -> Result<Vec<BackupTaskList>> {
    let lists = store::task_lists(conn)?;
    let mut out = Vec::with_capacity(lists.len());
    for list in lists {
        let tasks = store::tasks_for_list(conn, &list.id, true)?
            .into_iter()
            .map(|task| BackupTask {
                id: task.id,
                title: task.title,
                notes: task.notes,
                due_at: task.due_at,
                completed_at: task.completed_at,
                sort_order: task.sort_order,
                created_at: task.created_at,
                account: task.account,
                thread_id: task.thread_id,
                message_id: task.message_id,
            })
            .collect();
        out.push(BackupTaskList {
            id: list.id,
            title: list.title,
            sort_order: list.sort_order,
            tasks,
        });
    }
    Ok(out)
}

fn collect_subscriptions(conn: &Connection, account: &str) -> Result<Vec<BackupSubscription>> {
    let mut stmt = conn.prepare(
        "SELECT url, title, site_url, feed_title, enabled FROM subscriptions
         WHERE account = ?1 ORDER BY title COLLATE NOCASE, url",
    )?;
    let rows = stmt
        .query_map(params![account], |row| {
            Ok(BackupSubscription {
                url: row.get(0)?,
                title: row.get(1)?,
                site_url: row.get(2)?,
                feed_title: row.get(3)?,
                enabled: row.get::<_, i64>(4)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn collect_settings(conn: &Connection) -> Result<Map<String, Value>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Map::new();
    for (key, value) in rows {
        // Settings are stored as JSON text, but tolerate a bare string written
        // by an older build (settings_get has the same fallback).
        out.insert(
            key,
            serde_json::from_str(&value).unwrap_or_else(|_| json!(value)),
        );
    }
    Ok(out)
}

fn parse_json_column(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| json!({}))
}

/// Strip the credentials that hide inside the non-secret `config` column when
/// the user asked for a backup without secrets: the proxy password (typed by
/// the user, and often reused) and the OAuth client secret. An OAuth account
/// cannot be restored without its tokens anyway, so dropping the client secret
/// costs nothing — re-authenticating re-supplies it.
fn redact_account_config(config: &mut Value) {
    let Some(map) = config.as_object_mut() else {
        return;
    };
    map.remove("oauth_client_secret");
    if let Some(proxy) = map.get_mut("proxy") {
        redact_proxy(proxy);
    }
}

/// The app-wide proxy setting carries the same password as a per-account one.
fn redact_settings(settings: &mut Map<String, Value>) {
    if let Some(proxy) = settings.get_mut(crate::proxy::SETTING_KEY) {
        redact_proxy(proxy);
    }
}

fn redact_proxy(proxy: &mut Value) {
    if let Some(map) = proxy.as_object_mut()
        && map.contains_key("password")
    {
        map.insert("password".to_string(), json!(""));
    }
}

/// Serialize a backup to the JSON text written to disk. With a passphrase the
/// payload is encrypted and only the envelope stays readable.
///
/// Returns an error if `include_secrets` is set without a passphrase: secrets
/// never leave the device in the clear.
///
/// `host` stamps the envelope with who wrote the file — see [`Host`].
pub fn export(
    conn: &Connection,
    include_secrets: bool,
    passphrase: Option<&str>,
    host: Host<'_>,
    load_secrets: &dyn Fn(&str) -> Secrets,
) -> Result<String> {
    let passphrase = passphrase.filter(|p| !p.is_empty());
    if include_secrets && passphrase.is_none() {
        return Err(anyhow!(
            "a passphrase is required to include account passwords"
        ));
    }
    let data = collect(conn, include_secrets, load_secrets)?;
    let payload = serde_json::to_string(&data)?;

    let mut envelope = Map::new();
    envelope.insert("meron_backup".to_string(), json!(FORMAT_VERSION));
    for (field, value) in [
        ("platform", host.platform.trim()),
        ("app_version", host.app_version.trim()),
    ] {
        if !value.is_empty() {
            envelope.insert(field.to_string(), json!(value));
        }
    }
    envelope.insert("created_at".to_string(), json!(store::now_unix()));
    envelope.insert("encrypted".to_string(), json!(passphrase.is_some()));
    envelope.insert("has_secrets".to_string(), json!(include_secrets));

    match passphrase {
        Some(passphrase) => {
            let sealed = encrypt(payload.as_bytes(), passphrase)?;
            envelope.insert("kdf".to_string(), sealed.kdf_json());
            envelope.insert("cipher".to_string(), json!("aes-256-gcm"));
            envelope.insert("nonce".to_string(), json!(STANDARD.encode(sealed.nonce)));
            envelope.insert(
                "ciphertext".to_string(),
                json!(STANDARD.encode(&sealed.ciphertext)),
            );
        }
        None => {
            envelope.insert("data".to_string(), serde_json::to_value(&data)?);
        }
    }
    Ok(serde_json::to_string_pretty(&Value::Object(envelope))?)
}

// ---- Import -----------------------------------------------------------------

/// Parse a backup file, decrypting it when it is encrypted. A `None` passphrase
/// against an encrypted file reports [`needs_passphrase`]-style failure so the
/// caller can prompt and retry.
pub fn parse(text: &str, passphrase: Option<&str>) -> Result<BackupData> {
    let envelope: Value = serde_json::from_str(text).context("not a Meron backup file")?;
    let version = envelope
        .get("meron_backup")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("not a Meron backup file"))?;
    if version > FORMAT_VERSION as u64 {
        return Err(anyhow!(
            "this backup was written by a newer version of Meron (format {version})"
        ));
    }

    if !envelope
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let data = envelope
            .get("data")
            .ok_or_else(|| anyhow!("backup file has no data"))?;
        return data_from_value(data.clone());
    }

    let passphrase = passphrase
        .filter(|p| !p.is_empty())
        .ok_or_else(|| anyhow!(PASSPHRASE_REQUIRED))?;
    let cipher = envelope
        .get("cipher")
        .and_then(Value::as_str)
        .unwrap_or("aes-256-gcm");
    if cipher != "aes-256-gcm" {
        return Err(anyhow!("unsupported backup cipher: {cipher}"));
    }
    let salt = decode_b64(&envelope, &["kdf", "salt"])?;
    // We are the only writer of this format, so anything but our own salt size
    // is a damaged or hand-edited file rather than something to accommodate.
    if salt.len() != SALT_LEN {
        return Err(anyhow!("backup file has a malformed `kdf.salt`"));
    }
    let iterations = envelope
        .get("kdf")
        .and_then(|kdf| kdf.get("iterations"))
        .and_then(Value::as_u64)
        .unwrap_or(KDF_ITERATIONS as u64);
    // Checked as u64 before narrowing: a truncating cast would turn 2^32 into a
    // rejected 0 but 2^32 + 600_000 into a perfectly normal-looking count.
    if iterations == 0 || iterations > MAX_KDF_ITERATIONS {
        return Err(anyhow!(
            "backup file asks for an unreasonable amount of work to open it"
        ));
    }
    let iterations = iterations as u32;
    let nonce = decode_b64(&envelope, &["nonce"])?;
    let ciphertext = decode_b64(&envelope, &["ciphertext"])?;

    let plaintext = decrypt(&ciphertext, &nonce, &salt, iterations, passphrase)?;
    data_from_value(serde_json::from_slice(&plaintext).context("backup contents are corrupt")?)
}

fn data_from_value(mut value: Value) -> Result<BackupData> {
    migrate_legacy_platform(&mut value);
    Ok(serde_json::from_value(value)?)
}

/// Fold a pre-release `platform` map into `settings`.
///
/// Before mobile preferences became rows in the `settings` table they rode in
/// their own map, keyed `app:<name>` / `kanban:<name>`. That shape never reached
/// a release, so this exists only for files written from a development build —
/// but serde would silently ignore the field, restoring such a backup with every
/// appearance, language, layout and kanban preference quietly dropped, and a
/// silent partial restore is worth a dozen lines to avoid.
fn migrate_legacy_platform(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(Value::Object(platform)) = object.remove("platform") else {
        return;
    };
    let settings = object
        .entry("settings")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(settings) = settings.as_object_mut() else {
        return;
    };
    for (key, value) in platform {
        let Some((store, name)) = key.split_once(':') else {
            continue;
        };
        // A real settings row wins: it is the newer home for the same value.
        settings
            .entry(format!("mobile.{store}.{name}"))
            .or_insert(value);
    }
}

/// Error text [`parse`] returns when an encrypted file was opened without a
/// passphrase. Callers match on it to know to prompt rather than to give up.
pub const PASSPHRASE_REQUIRED: &str = "this backup is encrypted; a passphrase is required";

/// Whether an error from [`parse`] means "prompt for a passphrase".
pub fn needs_passphrase(error: &str) -> bool {
    error.contains(PASSPHRASE_REQUIRED)
}

fn decode_b64(envelope: &Value, path: &[&str]) -> Result<Vec<u8>> {
    let mut node = envelope;
    for key in path {
        node = node
            .get(key)
            .ok_or_else(|| anyhow!("backup file is missing `{}`", path.join(".")))?;
    }
    let text = node
        .as_str()
        .ok_or_else(|| anyhow!("backup file has a malformed `{}`", path.join(".")))?;
    STANDARD
        .decode(text)
        .with_context(|| format!("backup file has a malformed `{}`", path.join(".")))
}

/// Write a parsed backup into the store.
///
/// Accounts are matched by id; one that already exists is left completely alone
/// (counted in `skipped`) rather than overwritten, so restoring onto a machine
/// that is already set up cannot break a working connection. Settings, by
/// contrast, are the backup's whole point and do overwrite. `store_secrets` is
/// called only for accounts actually created, and only for backups that carry
/// secrets.
pub fn apply(
    conn: &Connection,
    data: &BackupData,
    store_secrets: &dyn Fn(&Connection, &str, &Secrets) -> Result<()>,
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    let tx = conn.unchecked_transaction()?;
    let now = store::now_unix();

    for account in &data.accounts {
        let id = account.id.trim();
        if id.is_empty() {
            continue;
        }
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)",
            params![id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if exists {
            summary.skipped += 1;
            continue;
        }
        let engine = if account.engine.is_empty() {
            "mail"
        } else {
            &account.engine
        };
        let provider = if account.provider.is_empty() {
            "custom"
        } else {
            &account.provider
        };
        tx.execute(
            "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url,
                                  sender_name, config, prefs, sort_order, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![
                id,
                engine,
                provider,
                account.email,
                account.display_name,
                account.avatar_url,
                account.sender_name,
                json_column(&account.config),
                json_column(&account.prefs),
                account.sort_order,
                now,
            ],
        )?;
        summary.accounts += 1;

        for sub in &account.subscriptions {
            summary.feeds += restore_subscription(&tx, id, sub, now)?;
        }

        if let Some(secrets) = &account.secrets
            && !secrets.is_empty()
        {
            store_secrets(&tx, id, secrets)?;
            summary.secrets += 1;
        }
    }

    for (key, value) in &data.settings {
        store::setting_set(&tx, key, value)?;
        summary.settings += 1;
    }

    for (index, list) in data.task_lists.iter().enumerate() {
        summary.tasks += restore_task_list(&tx, list, index, now)?;
    }

    tx.commit()?;
    Ok(summary)
}

/// Insert one subscription, re-deriving its id from the normalized URL the way
/// a live subscribe does. `INSERT OR IGNORE` because `url` is UNIQUE across all
/// accounts: a feed already subscribed elsewhere is skipped, not duplicated.
fn restore_subscription(
    conn: &Connection,
    account: &str,
    sub: &BackupSubscription,
    now: i64,
) -> Result<u32> {
    let Ok(url) = crate::rss::normalize_feed_url(&sub.url) else {
        return Ok(0);
    };
    let id = crate::rss::rss_subscription_id(&url);
    let added = conn.execute(
        "INSERT OR IGNORE INTO subscriptions
           (id, account, url, title, site_url, feed_title, enabled, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            account,
            url,
            sub.title.trim(),
            sub.site_url.trim(),
            sub.feed_title.trim(),
            i64::from(sub.enabled),
            now,
        ],
    )?;
    Ok(added as u32)
}

/// Preserve identities rather than merging unrelated rows with equal titles.
/// Old backups lack IDs: derive repeatable IDs from their contents and position
/// so even identical rows survive, and importing the same file is idempotent.
fn restore_task_list(
    conn: &Connection,
    list: &BackupTaskList,
    index: usize,
    now: i64,
) -> Result<u32> {
    let list_id = if list.id.is_empty() {
        legacy_task_id("tasklist", &serde_json::to_vec(&(index, list))?)
    } else {
        list.id.clone()
    };
    conn.execute(
        "INSERT OR IGNORE INTO task_lists(id, title, sort_order, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?4)",
        params![list_id, list.title.trim(), list.sort_order, now],
    )?;

    let mut restored = 0;
    for (index, task) in list.tasks.iter().enumerate() {
        let task_id = if task.id.is_empty() {
            legacy_task_id("task", &serde_json::to_vec(&(&list_id, index, task))?)
        } else {
            task.id.clone()
        };
        let added = conn.execute(
            "INSERT OR IGNORE INTO tasks(id, list_id, title, notes, due_at, completed_at, sort_order,
                               account, thread_id, message_id, json, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '{}', ?11, ?12)",
            params![
                task_id,
                list_id,
                task.title.trim(),
                task.notes,
                task.due_at.max(0),
                task.completed_at.max(0),
                task.sort_order,
                task.account,
                task.thread_id,
                task.message_id,
                if task.created_at > 0 {
                    task.created_at
                } else {
                    now
                },
                now,
            ],
        )?;
        restored += added as u32;
    }
    Ok(restored)
}

fn legacy_task_id(prefix: &str, bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    format!(
        "{prefix}-legacy-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest.as_ref())
    )
}

fn json_column(value: &Value) -> String {
    if value.is_object() {
        value.to_string()
    } else {
        "{}".to_string()
    }
}

// ---- Encryption -------------------------------------------------------------

struct Sealed {
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    ciphertext: Vec<u8>,
}

impl Sealed {
    fn kdf_json(&self) -> Value {
        json!({
            "algorithm": "pbkdf2-hmac-sha256",
            "iterations": KDF_ITERATIONS,
            "salt": STANDARD.encode(self.salt),
        })
    }
}

fn derive_key(passphrase: &str, salt: &[u8], iterations: u32) -> Result<[u8; KEY_LEN]> {
    let rounds = NonZeroU32::new(iterations).ok_or_else(|| anyhow!("invalid backup kdf"))?;
    let mut key = [0u8; KEY_LEN];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        rounds,
        salt,
        passphrase.as_bytes(),
        &mut key,
    );
    Ok(key)
}

fn encrypt(plaintext: &[u8], passphrase: &str) -> Result<Sealed> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rng.fill(&mut salt)
        .map_err(|_| anyhow!("could not generate a backup salt"))?;
    rng.fill(&mut nonce)
        .map_err(|_| anyhow!("could not generate a backup nonce"))?;

    let key = derive_key(passphrase, &salt, KDF_ITERATIONS)?;
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, &key).map_err(|_| anyhow!("could not build a backup key"))?,
    );
    let mut buf = plaintext.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(AAD),
        &mut buf,
    )
    .map_err(|_| anyhow!("could not encrypt the backup"))?;

    Ok(Sealed {
        salt,
        nonce,
        ciphertext: buf,
    })
}

fn decrypt(
    ciphertext: &[u8],
    nonce: &[u8],
    salt: &[u8],
    iterations: u32,
    passphrase: &str,
) -> Result<Vec<u8>> {
    let nonce: [u8; NONCE_LEN] = nonce
        .try_into()
        .map_err(|_| anyhow!("backup file has a malformed `nonce`"))?;
    let key = derive_key(passphrase, salt, iterations)?;
    let key = LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, &key).map_err(|_| anyhow!("could not build a backup key"))?,
    );
    let mut buf = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(AAD),
            &mut buf,
        )
        // GCM cannot tell a wrong key from a damaged file, and the overwhelmingly
        // likely cause is a mistyped passphrase, so say that.
        .map_err(|_| anyhow!("wrong passphrase, or the backup file is damaged"))?;
    Ok(plaintext.to_vec())
}

#[cfg(test)]
mod tests;
