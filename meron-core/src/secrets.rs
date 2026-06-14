//! OS keychain storage for per-account secrets (IMAP password + OAuth tokens).
//!
//! Secrets live in the platform keychain — Keychain on macOS, Credential Manager
//! on Windows, the Secret Service (D-Bus, e.g. gnome-keyring) on Linux — instead
//! of plaintext in the SQLite store. One keychain entry per account holds a JSON
//! blob; non-secret connection metadata (host, port, user, ...) stays in SQLite.

use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::imap::Creds;

// OS keychain service name, matching the app name.
const SERVICE: &str = "meron";

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

/// Store (or replace) an account's secrets in the OS keychain.
pub fn store(account: &str, secrets: &Secrets) -> Result<()> {
    if keyring_disabled() {
        return Ok(());
    }
    let blob = serde_json::to_string(secrets)?;
    entry(account)?
        .set_password(&blob)
        .with_context(|| format!("write keychain entry for {account}"))
}

/// Load an account's secrets, or defaults if no entry exists.
pub fn load(account: &str) -> Result<Secrets> {
    if keyring_disabled() {
        return Ok(Secrets::default());
    }
    match entry(account)?.get_password() {
        Ok(blob) => Ok(serde_json::from_str(&blob).unwrap_or_default()),
        Err(keyring::Error::NoEntry) => Ok(Secrets::default()),
        Err(e) => Err(e).with_context(|| format!("read keychain entry for {account}")),
    }
}

/// Remove an account's secrets from the keychain. A missing entry is not an error.
pub fn delete(account: &str) -> Result<()> {
    if keyring_disabled() {
        return Ok(());
    }
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete keychain entry for {account}")),
    }
}
