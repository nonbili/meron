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

/// `MERON_KEYRING` escape hatches, all opt-in and none set in normal use:
///
///   * `off` — no keychain at all: operations become no-ops and `load` returns
///     empty secrets (tests/headless CI). Within a single sidecar run secrets
///     stay in memory, so only cross-restart persistence is lost.
///   * `service` — force the D-Bus Secret Service even inside Flatpak, for
///     sandboxes that can reach the host service directly.
///   * `file` — force the local file keyring, skipping D-Bus entirely. The
///     supported workaround when a desktop has no working secret storage.
fn keyring_disabled() -> bool {
    std::env::var_os("MERON_KEYRING").is_some_and(|v| v == "off")
}

// Linux talks to the keychain through `backend_*` below, which builds its own
// entries inside the timeout wrapper.
#[cfg(not(target_os = "linux"))]
fn entry(account: &str) -> Result<Entry> {
    Entry::new(SERVICE, account).with_context(|| format!("open keychain entry for {account}"))
}

// ---- Windows blob chunking --------------------------------------------------
//
// Credential Manager caps one credential blob at CRED_MAX_CREDENTIAL_BLOB_SIZE
// (2560 bytes, i.e. 1280 UTF-16 units). A Microsoft OAuth pair alone is bigger
// than that — Google's fits, which is why only Outlook/Hotmail accounts failed
// to connect. Oversized blobs are therefore split across numbered companion
// entries, with the main entry holding a marker naming the chunk count. Blobs
// that fit are still written verbatim, so existing entries keep loading.
//
// An update writes its chunks under the *other* generation and only then swings
// the marker, so the generation the marker points at is never edited in place: a
// write that fails partway leaves the previous secrets intact and readable.

/// Chunk payload size, in UTF-16 units, kept under the 1280-unit platform cap.
const CHUNK_UTF16: usize = 1200;

/// Hard cap on chunks per generation — ~38K of secrets, far past the few
/// kilobytes an OAuth pair needs. Cleanup sweeps this whole range instead of
/// stopping at the first missing index, so a delete that failed earlier cannot
/// hide the chunks after it from every later sweep.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const MAX_CHUNKS: usize = 32;

/// Prefix of the marker written to the main entry, followed by `<generation>:<count>`.
/// Real blobs are JSON objects, so they never start with this.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const CHUNKED_MARKER: &str = "meron-chunked:";

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn chunk_account(account: &str, generation: u8, index: usize) -> String {
    format!("{account}--meron-g{generation}c{index}")
}

/// Read a main entry as a chunk marker: `Some((generation, count))`, or `None`
/// for a plain unchunked blob.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_marker(head: &str) -> Option<(u8, usize)> {
    let (generation, count) = head.strip_prefix(CHUNKED_MARKER)?.split_once(':')?;
    Some((generation.parse().ok()?, count.parse().ok()?))
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn fits_in_credential(blob: &str) -> bool {
    blob.encode_utf16().count() <= CHUNK_UTF16
}

/// Split a blob into pieces that each fit one credential, cutting only on char
/// boundaries so no surrogate pair or multi-byte char is torn in half.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn split_blob(blob: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut units = 0;
    for (offset, ch) in blob.char_indices() {
        let width = ch.len_utf16();
        if units + width > CHUNK_UTF16 {
            chunks.push(&blob[start..offset]);
            start = offset;
            units = 0;
        }
        units += width;
    }
    if start < blob.len() {
        chunks.push(&blob[start..]);
    }
    chunks
}

#[cfg(target_os = "linux")]
fn keyring_forced_to(mode: &str) -> bool {
    std::env::var_os("MERON_KEYRING").is_some_and(|v| v == mode)
}

#[cfg(target_os = "linux")]
fn use_portal_keyring() -> bool {
    if keyring_forced_to("service") {
        return false;
    }
    flatpak_detected(
        std::env::var_os("FLATPAK_ID").as_deref(),
        std::path::Path::new("/.flatpak-info").exists(),
    )
}

/// Whether to go through [`crate::secrets_portal`] (Secret portal, or its local
/// file fallback) rather than the D-Bus Secret Service.
#[cfg(target_os = "linux")]
fn use_oo7_keyring() -> bool {
    use_portal_keyring()
        || keyring_forced_to("file")
        || crate::secrets_portal::local_forced()
        || (!keyring_forced_to("service") && crate::secrets_portal::local_keyring_exists())
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
    backend_store(account, &blob)
}

/// Load an account's secrets, or defaults if no entry exists.
pub fn load(account: &str) -> Result<Secrets> {
    if keyring_disabled() {
        return Ok(Secrets::default());
    }
    match backend_load(account)? {
        Some(blob) => Ok(serde_json::from_str(&blob).unwrap_or_default()),
        None => Ok(Secrets::default()),
    }
}

// ---- Backends ---------------------------------------------------------------
//
// On Linux the keychain is a D-Bus service that may be absent (a Flatpak with
// no Secret portal backend, a Snap whose `password-manager-service` interface
// was never connected) and, when it is, may never answer. Every call is
// therefore bounded, and an unavailable service demotes the process to the
// local file keyring instead of failing every operation for the rest of the
// run. macOS and Windows keep the plain synchronous path.

#[cfg(target_os = "linux")]
fn backend_store(account: &str, blob: &str) -> Result<()> {
    let owned_account = account.to_owned();
    let owned_blob = blob.to_owned();
    with_service_fallback(
        move || {
            service_call("write", move || {
                Entry::new(SERVICE, &owned_account)?.set_password(&owned_blob)
            })
        },
        || {
            crate::secrets_portal::store(account, blob)
                .with_context(|| format!("write keyring entry for {account}"))
        },
    )
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn backend_store(account: &str, blob: &str) -> Result<()> {
    write_entry(account, blob)
}

#[cfg(target_os = "windows")]
fn backend_store(account: &str, blob: &str) -> Result<()> {
    let live = read_entry(account)?.as_deref().and_then(parse_marker);
    if fits_in_credential(blob) {
        write_entry(account, blob)?;
        // Best-effort cleanup: the secrets are safely stored either way, and a
        // failure here only leaves an unreferenced entry behind.
        let _ = sweep_chunks(account);
        return Ok(());
    }
    // Never touch the generation the current marker points at, so the stored
    // secrets stay loadable until the marker itself is replaced.
    let generation = live.map_or(0, |(live, _)| live ^ 1);
    let chunks = split_blob(blob);
    anyhow::ensure!(
        chunks.len() <= MAX_CHUNKS,
        "secrets for {account} need {} keychain chunks, over the {MAX_CHUNKS} limit",
        chunks.len()
    );
    for (index, chunk) in chunks.iter().enumerate() {
        write_entry(&chunk_account(account, generation, index), chunk)?;
    }
    write_entry(
        account,
        &format!("{CHUNKED_MARKER}{generation}:{}", chunks.len()),
    )?;
    // Best-effort cleanup: the secrets are safely stored either way, and a
    // failure here only leaves an unreferenced entry behind.
    let _ = delete_chunks(account, generation ^ 1, 0..MAX_CHUNKS);
    let _ = delete_chunks(account, generation, chunks.len()..MAX_CHUNKS);
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn write_entry(account: &str, blob: &str) -> Result<()> {
    entry(account)?
        .set_password(blob)
        .with_context(|| format!("write keychain entry for {account}"))
}

/// Delete a generation's chunk entries over `range` — a stale generation, or the
/// tail left behind when a value shrinks. Missing entries are fine; a real
/// failure does not stop the sweep, so one undeletable entry can never hide the
/// ones after it, and the first such error is returned once the range is done.
#[cfg(target_os = "windows")]
fn delete_chunks(account: &str, generation: u8, range: std::ops::Range<usize>) -> Result<()> {
    let mut failure = None;
    for index in range {
        let name = chunk_account(account, generation, index);
        let deleted = entry(&name).and_then(|e| match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e).with_context(|| format!("delete keychain entry for {name}")),
        });
        if let Err(e) = deleted {
            failure.get_or_insert(e);
        }
    }
    failure.map_or(Ok(()), Err)
}

/// Drop every chunk entry for an account, in either generation.
#[cfg(target_os = "windows")]
fn sweep_chunks(account: &str) -> Result<()> {
    let zero = delete_chunks(account, 0, 0..MAX_CHUNKS);
    zero.and(delete_chunks(account, 1, 0..MAX_CHUNKS))
}

#[cfg(target_os = "linux")]
fn backend_load(account: &str) -> Result<Option<String>> {
    let owned_account = account.to_owned();
    with_service_fallback(
        move || {
            service_call("read", move || {
                match Entry::new(SERVICE, &owned_account)?.get_password() {
                    Ok(blob) => Ok(Some(blob)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(error) => Err(error),
                }
            })
        },
        || {
            crate::secrets_portal::load(account)
                .with_context(|| format!("read keyring entry for {account}"))
        },
    )
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn backend_load(account: &str) -> Result<Option<String>> {
    read_entry(account)
}

#[cfg(target_os = "windows")]
fn backend_load(account: &str) -> Result<Option<String>> {
    let Some(head) = read_entry(account)? else {
        return Ok(None);
    };
    let Some((generation, count)) = parse_marker(&head) else {
        return Ok(Some(head));
    };
    let mut blob = String::new();
    for index in 0..count {
        let name = chunk_account(account, generation, index);
        let chunk = read_entry(&name)?
            .with_context(|| format!("missing keychain chunk {index} for {account}"))?;
        blob.push_str(&chunk);
    }
    Ok(Some(blob))
}

#[cfg(not(target_os = "linux"))]
fn read_entry(account: &str) -> Result<Option<String>> {
    match entry(account)?.get_password() {
        Ok(blob) => Ok(Some(blob)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read keychain entry for {account}")),
    }
}

#[cfg(target_os = "linux")]
fn backend_delete(account: &str) -> Result<()> {
    let owned_account = account.to_owned();
    with_service_fallback(
        move || {
            service_call("delete", move || {
                match Entry::new(SERVICE, &owned_account)?.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                    Err(error) => Err(error),
                }
            })
        },
        || {
            crate::secrets_portal::delete(account)
                .with_context(|| format!("delete keyring entry for {account}"))
        },
    )
}

#[cfg(target_os = "windows")]
fn backend_delete(account: &str) -> Result<()> {
    // Chunks first: while the marker survives, a failed sweep leaves an account
    // that still loads rather than one pointing at half-deleted secrets.
    sweep_chunks(account)?;
    delete_entry(account)
}

#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
fn backend_delete(account: &str) -> Result<()> {
    delete_entry(account)
}

#[cfg(not(target_os = "linux"))]
fn delete_entry(account: &str) -> Result<()> {
    match entry(account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete keychain entry for {account}")),
    }
}

/// A failed Secret Service call. `unavailable` separates "there is no usable
/// service here" — worth falling back for — from a real error such as an
/// ambiguous or oversized entry, which the local keyring would not fix.
#[cfg(target_os = "linux")]
struct ServiceFailure {
    error: anyhow::Error,
    unavailable: bool,
    /// Worth one retry before it counts (see [`service_call`]). A timeout is
    /// not: a wedged service would only cost the wait twice.
    transient: bool,
}

#[cfg(target_os = "linux")]
impl ServiceFailure {
    fn unavailable(message: String) -> Self {
        Self {
            error: anyhow::anyhow!(message),
            unavailable: true,
            transient: false,
        }
    }

    fn from_keyring(what: &str, error: keyring::Error) -> Self {
        let unavailable = matches!(
            error,
            keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_)
        );
        let transient = matches!(error, keyring::Error::PlatformFailure(_));
        Self {
            error: anyhow::Error::new(error).context(format!("Secret Service {what}")),
            unavailable,
            transient,
        }
    }
}

/// Run one blocking Secret Service call on a throwaway thread, bounded by
/// [`SERVICE_TIMEOUT`]. The thread is abandoned on timeout rather than joined:
/// a wedged D-Bus call never returns, and the sticky demotion means we stop
/// issuing these calls after the first one gives up.
#[cfg(target_os = "linux")]
const SERVICE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

///
/// A platform failure is tried once more before it counts: the service
/// encrypts each reply for a per-call session, and that exchange fails now and
/// then ("Unpad Error") against a keychain that is otherwise healthy. Demoting
/// on one such hiccup would strand every secret the service holds.
#[cfg(target_os = "linux")]
fn service_call<T: Send + 'static>(
    what: &str,
    call: impl Fn() -> std::result::Result<T, keyring::Error> + Send + Sync + 'static,
) -> std::result::Result<T, ServiceFailure> {
    let call = std::sync::Arc::new(call);
    let first = {
        let call = call.clone();
        service_call_once(what, move || call())
    };
    match first {
        Err(failure) if failure.transient => service_call_once(what, move || call()),
        other => other,
    }
}

#[cfg(target_os = "linux")]
fn service_call_once<T: Send + 'static>(
    what: &str,
    call: impl FnOnce() -> std::result::Result<T, keyring::Error> + Send + 'static,
) -> std::result::Result<T, ServiceFailure> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("meron-secret-service".into())
        .spawn(move || {
            let _ = sender.send(call());
        })
        .map_err(|error| ServiceFailure::unavailable(format!("spawn keychain thread: {error}")))?;
    match receiver.recv_timeout(SERVICE_TIMEOUT) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(ServiceFailure::from_keyring(what, error)),
        Err(_) => Err(ServiceFailure::unavailable(format!(
            "Secret Service {what} timed out after {}s",
            SERVICE_TIMEOUT.as_secs()
        ))),
    }
}

/// Try the Secret Service, falling back to the oo7 keyring (portal or local
/// file) when there is no usable service. Once demoted, later calls skip the
/// service entirely.
#[cfg(target_os = "linux")]
fn with_service_fallback<T>(
    service: impl FnOnce() -> std::result::Result<T, ServiceFailure>,
    local: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if use_oo7_keyring() {
        return local();
    }
    match service() {
        Ok(value) => Ok(value),
        Err(failure) if failure.unavailable => {
            crate::secrets_portal::force_local(&format!("{:#}", failure.error));
            local()
        }
        Err(failure) => Err(failure.error),
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
    match backend_load(DB_KEY_ACCOUNT).context("read db key from keychain")? {
        Some(key) if is_hex_key(&key) => Ok(Some(key)),
        // Missing (first run) or a malformed entry: mint and persist a fresh key.
        Some(_) | None => {
            #[cfg(target_os = "linux")]
            if let Some(key) = service_key_behind_stale_local_keyring() {
                return Ok(Some(key));
            }
            let key = generate_db_key();
            backend_store(DB_KEY_ACCOUNT, &key).context("store db key in keychain")?;
            Ok(Some(key))
        }
    }
}

/// The store key still held by the Secret Service when a local file keyring
/// without one is shadowing it.
///
/// A local keyring on disk normally means the profile committed to it, store
/// key included (see `secrets_portal::local_keyring_exists`). But a run that
/// read its key from the service and was demoted afterwards leaves a local
/// keyring holding no store key, and the next launch would mint a fresh one
/// there and lock itself out of the database. Finding the real key in the
/// service proves that is what happened, so the service takes over again.
#[cfg(target_os = "linux")]
fn service_key_behind_stale_local_keyring() -> Option<String> {
    let shadowed = !use_portal_keyring()
        && !keyring_forced_to("file")
        && !crate::secrets_portal::local_forced()
        && crate::secrets_portal::local_keyring_exists();
    if !shadowed {
        return None;
    }
    let key = service_call("read", || {
        match Entry::new(SERVICE, DB_KEY_ACCOUNT)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error),
        }
    })
    .ok()
    .flatten()
    .filter(|key| is_hex_key(key))?;
    crate::secrets_portal::prefer_service();
    Some(key)
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
    backend_delete(account)
}

#[cfg(test)]
mod chunk_tests {
    use super::{
        CHUNK_UTF16, MAX_CHUNKS, chunk_account, fits_in_credential, parse_marker, split_blob,
    };

    #[test]
    fn markers_round_trip_and_plain_blobs_are_not_markers() {
        assert_eq!(parse_marker("meron-chunked:1:4"), Some((1, 4)));
        assert_eq!(parse_marker(r#"{"password":"hunter2"}"#), None);
        assert_eq!(parse_marker("meron-chunked:4"), None);
    }

    #[test]
    fn generations_use_distinct_entries() {
        assert_ne!(
            chunk_account("a@b.com", 0, 0),
            chunk_account("a@b.com", 1, 0)
        );
    }

    #[test]
    fn short_blobs_are_not_chunked() {
        let blob = r#"{"password":"hunter2"}"#;
        assert!(fits_in_credential(blob));
    }

    #[test]
    fn chunks_fit_and_rejoin() {
        // Roughly the size of a Microsoft access + refresh token pair.
        let blob = format!(r#"{{"access_token":"{}"}}"#, "a".repeat(4000));
        assert!(!fits_in_credential(&blob));
        let chunks = split_blob(&blob);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| fits_in_credential(c)));
        assert_eq!(chunks.concat(), blob);
    }

    #[test]
    fn chunk_limit_has_room_for_realistic_secrets() {
        // Far larger than any password plus OAuth pair we store.
        let blob = "a".repeat(20_000);
        assert!(split_blob(&blob).len() <= MAX_CHUNKS);
    }

    #[test]
    fn multi_byte_chars_are_not_split() {
        // Emoji are two UTF-16 units each, so an odd boundary would tear one.
        let blob = "😀".repeat(CHUNK_UTF16);
        let chunks = split_blob(&blob);
        assert!(chunks.iter().all(|c| fits_in_credential(c)));
        assert_eq!(chunks.concat(), blob);
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
