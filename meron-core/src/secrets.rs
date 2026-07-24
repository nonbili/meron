//! OS keychain storage for per-account secrets (IMAP password + OAuth tokens).
//!
//! Secrets live in the platform keychain — Keychain on macOS, Credential Manager
//! on Windows, and the Secret Service (D-Bus, e.g. gnome-keyring) on native Linux.
//! Inside Flatpak, oo7 uses the Secret portal to encrypt an app-private keyring.
//! One entry per account holds a JSON blob; non-secret connection metadata (host,
//! port, user, ...) stays in SQLite.

use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::imap::Creds;

// OS keychain service name, matching the app name.
const SERVICE: &str = "meron";

// Reserved keychain "account" holding the SQLCipher key for the local store.
// The leading underscores keep it out of the real per-account id namespace.
const DB_KEY_ACCOUNT: &str = "__meron_db_key__";

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Secrets {
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

impl Secrets {
    /// Pull the secret-bearing fields out of a fully-populated `Creds`.
    pub fn from_creds(c: &Creds) -> Self {
        Secrets {
            password: c.password.clone(),
            access_token: c.access_token.clone().filter(|s| !s.is_empty()),
            refresh_token: c.refresh_token.clone().filter(|s| !s.is_empty()),
        }
    }

    /// Overlay these secrets onto a `Creds` loaded from the (secret-free) store.
    pub fn apply_to(&self, c: &mut Creds) {
        c.password = self.password.clone();
        c.access_token = self.access_token.clone();
        c.refresh_token = self.refresh_token.clone();
    }

    pub fn is_empty(&self) -> bool {
        self.password.is_empty()
            && self.access_token.as_deref().unwrap_or("").is_empty()
            && self.refresh_token.as_deref().unwrap_or("").is_empty()
    }
}

/// Test/headless-CI escape hatch: `MERON_KEYRING=off` turns all keychain
/// operations into no-ops (load returns empty secrets). Within a single
/// sidecar run secrets stay in memory, so only cross-restart persistence
/// is lost. Never set in normal use.
fn keyring_disabled() -> bool {
    std::env::var_os("MERON_KEYRING").is_some_and(|v| v == "off")
}

fn entry(account: &str) -> Result<Entry> {
    Entry::new(SERVICE, account).with_context(|| format!("open keychain entry for {account}"))
}

#[cfg(target_os = "linux")]
fn use_portal_keyring() -> bool {
    flatpak_detected(
        std::env::var_os("FLATPAK_ID").as_deref(),
        std::path::Path::new("/.flatpak-info").exists(),
    )
}

#[cfg(target_os = "linux")]
fn flatpak_detected(flatpak_id: Option<&std::ffi::OsStr>, marker_exists: bool) -> bool {
    flatpak_id.is_some_and(|id| !id.is_empty()) || marker_exists
}

#[cfg(not(target_os = "linux"))]
fn use_portal_keyring() -> bool {
    false
}

/// Store (or replace) an account's secrets in the OS keychain.
pub fn store(account: &str, secrets: &Secrets) -> Result<()> {
    if keyring_disabled() {
        return Ok(());
    }
    let blob = serde_json::to_string(secrets)?;
    #[cfg(target_os = "linux")]
    if use_portal_keyring() {
        return crate::secrets_portal::store(account, &blob)
            .with_context(|| format!("write portal keyring entry for {account}"));
    }
    entry(account)?
        .set_password(&blob)
        .with_context(|| format!("write keychain entry for {account}"))
}

/// Load an account's secrets, or defaults if no entry exists.
pub fn load(account: &str) -> Result<Secrets> {
    if keyring_disabled() {
        return Ok(Secrets::default());
    }
    #[cfg(target_os = "linux")]
    if use_portal_keyring() {
        return match crate::secrets_portal::load(account)
            .with_context(|| format!("read portal keyring entry for {account}"))?
        {
            Some(blob) => Ok(serde_json::from_str(&blob).unwrap_or_default()),
            None => Ok(Secrets::default()),
        };
    }
    match entry(account)?.get_password() {
        Ok(blob) => Ok(serde_json::from_str(&blob).unwrap_or_default()),
        Err(keyring::Error::NoEntry) => Ok(Secrets::default()),
        Err(e) => Err(e).with_context(|| format!("read keychain entry for {account}")),
    }
}

/// The SQLCipher key for the local store (64 hex chars = a raw 32-byte key),
/// created and persisted in the OS keychain on first use.
///
/// Returns `Ok(None)` when the keychain is disabled (`MERON_KEYRING=off`,
/// tests/headless), so the store falls back to plaintext exactly as before.
pub fn db_key() -> Result<Option<String>> {
    if keyring_disabled() {
        return Ok(None);
    }
    #[cfg(target_os = "linux")]
    if use_portal_keyring() {
        return match crate::secrets_portal::load(DB_KEY_ACCOUNT)
            .context("read db key from portal keyring")?
        {
            Some(key) if is_hex_key(&key) => Ok(Some(key)),
            Some(_) | None => {
                let key = generate_db_key();
                crate::secrets_portal::store(DB_KEY_ACCOUNT, &key)
                    .context("store db key in portal keyring")?;
                Ok(Some(key))
            }
        };
    }
    let entry = entry(DB_KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(key) if is_hex_key(&key) => Ok(Some(key)),
        // Missing (first run) or a malformed entry: mint and persist a fresh key.
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let key = generate_db_key();
            entry
                .set_password(&key)
                .context("store db key in keychain")?;
            Ok(Some(key))
        }
        Err(e) => Err(e).context("read db key from keychain"),
    }
}

/// 32 random bytes (two v4 UUIDs' worth) rendered as 64 lowercase hex chars.
fn generate_db_key() -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for _ in 0..2 {
        for byte in uuid::Uuid::new_v4().as_bytes() {
            let _ = write!(hex, "{byte:02x}");
        }
    }
    hex
}

fn is_hex_key(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Remove an account's secrets from the keychain. A missing entry is not an error.
pub fn delete(account: &str) -> Result<()> {
    if keyring_disabled() {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    if use_portal_keyring() {
        return crate::secrets_portal::delete(account)
            .with_context(|| format!("delete portal keyring entry for {account}"));
    }
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete keychain entry for {account}")),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::flatpak_detected;
    use std::ffi::OsStr;

    #[test]
    fn flatpak_detection_does_not_match_native_linux() {
        assert!(!flatpak_detected(None, false));
        assert!(!flatpak_detected(Some(OsStr::new("")), false));
        assert!(flatpak_detected(
            Some(OsStr::new("jp.nonbili.meron")),
            false
        ));
        assert!(flatpak_detected(None, true));
    }
}
