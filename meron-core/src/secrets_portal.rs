//! Synchronous facade over oo7's async keyrings, with a local fallback.
//!
//! The sidecar's storage hooks are synchronous and can run from inside its
//! Tokio runtime, where calling `block_on` directly would panic. A dedicated
//! worker owns one keyring and serializes all operations.
//!
//! Two backends, picked once on first use:
//!
//!   * the Secret portal (`oo7::Keyring` under Flatpak), preferred;
//!   * a local file keyring under the app dir, encrypted with a key file.
//!
//! The fallback exists because the Secret portal can be absent — nothing
//! implements `org.freedesktop.portal.Secret` unless a backend such as
//! xdg-desktop-portal-gnome is installed — and, worse, can accept the call and
//! never answer, since the reply arrives as a `Response` signal on a Request
//! object the frontend may never emit. Every operation is therefore bounded by
//! [`OP_TIMEOUT`]; a timeout demotes the process to the local backend for the
//! rest of the run instead of wedging the sidecar before it reads its first
//! request.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};

use crate::log::Level;

const SERVICE: &str = "meron";

/// How long any single keyring operation may take before the backend counts as
/// unavailable. Generous enough for a real portal round trip that shows an
/// unlock prompt, short enough that a wedged portal doesn't look like a hang.
const OP_TIMEOUT: Duration = Duration::from_secs(10);

/// Caller-side backstop, always longer than [`OP_TIMEOUT`], so a worker busy
/// timing out someone else's request still can't block us indefinitely.
const REPLY_TIMEOUT: Duration = Duration::from_secs(35);

/// Sticky demotion to the local file keyring, set when the portal (or, via
/// [`force_local`], the Secret Service) turns out to be unusable. Sticky
/// because a missing D-Bus backend does not appear mid-session, and retrying it
/// per call would pay the timeout over and over.
static LOCAL_ONLY: AtomicBool = AtomicBool::new(false);

/// Demote to the local file keyring for the rest of this run. Logged once.
pub fn force_local(reason: &str) {
    if !LOCAL_ONLY.swap(true, Ordering::SeqCst) {
        crate::mlog!(
            Level::Warn,
            "keyring",
            "falling back to the local file keyring: {reason}"
        );
    }
}

/// Set when the local keyring on disk turned out to be a leftover that lacks
/// the store key the Secret Service still holds, so it no longer claims the
/// profile (see `secrets::service_key_behind_stale_local_keyring`).
static LOCAL_STALE: AtomicBool = AtomicBool::new(false);

/// Stop treating the local keyring on disk as this profile's backend.
pub fn prefer_service() {
    if !LOCAL_STALE.swap(true, Ordering::SeqCst) {
        crate::mlog!(
            Level::Warn,
            "keyring",
            "local file keyring has no store key but the Secret Service does; using the Secret Service"
        );
    }
}

/// Whether the local file keyring is already the active backend.
pub fn local_forced() -> bool {
    LOCAL_ONLY.load(Ordering::SeqCst)
}

/// Whether this profile has already committed to the local backend. Once the
/// database key has been stored here, silently returning to Secret Service on a
/// later launch would mint a different key and make the database unreadable.
pub fn local_keyring_exists() -> bool {
    !LOCAL_STALE.load(Ordering::SeqCst)
        && local_keyring_exists_in(&crate::store::app_dir().join("keyring"))
}

fn local_keyring_exists_in(dir: &Path) -> bool {
    dir.join("local.key").is_file() && dir.join("meron.keyring").is_file()
}

fn local_only() -> bool {
    local_forced()
        || std::env::var_os("MERON_KEYRING").is_some_and(|value| value == "file")
        || (!std::env::var_os("MERON_KEYRING").is_some_and(|value| value == "service")
            && local_keyring_exists())
}

enum Backend {
    Portal(oo7::Keyring),
    Local(oo7::file::UnlockedKeyring),
}

enum Request {
    Store {
        account: String,
        value: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Load {
        account: String,
        reply: mpsc::Sender<Result<Option<String>, String>>,
    },
    Delete {
        account: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
}

impl Request {
    fn fail(self, error: String) {
        match self {
            Self::Store { reply, .. } | Self::Delete { reply, .. } => {
                let _ = reply.send(Err(error));
            }
            Self::Load { reply, .. } => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

struct PortalKeyring {
    requests: mpsc::Sender<Request>,
}

static PORTAL_KEYRING: OnceLock<PortalKeyring> = OnceLock::new();

fn portal_keyring() -> &'static PortalKeyring {
    PORTAL_KEYRING.get_or_init(|| {
        let (requests, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("meron-portal-keyring".into())
            .spawn(move || worker(receiver))
            .expect("failed to start portal keyring worker");
        PortalKeyring { requests }
    })
}

fn worker(receiver: mpsc::Receiver<Request>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            for request in receiver {
                request.fail(format!("create portal keyring runtime: {error}"));
            }
            return;
        }
    };
    let mut backend = None;

    for request in receiver {
        if backend.is_none() {
            match runtime.block_on(open_backend()) {
                Ok(opened) => backend = Some(opened),
                Err(error) => {
                    request.fail(format!("{error:#}"));
                    continue;
                }
            }
        }
        match request {
            Request::Store {
                account,
                value,
                reply,
            } => {
                let active = backend.as_ref().expect("keyring backend initialized");
                let mut result =
                    block_with_timeout(&runtime, "store", store_async(active, &account, &value));
                if result.is_err() && local_forced() && matches!(active, Backend::Portal(_)) {
                    match runtime.block_on(open_backend()) {
                        Ok(opened) => {
                            backend = Some(opened);
                            let active =
                                backend.as_ref().expect("local keyring backend initialized");
                            result = block_with_timeout(
                                &runtime,
                                "store",
                                store_async(active, &account, &value),
                            );
                        }
                        Err(error) => result = Err(format!("{error:#}")),
                    }
                }
                let _ = reply.send(result);
            }
            Request::Load { account, reply } => {
                let active = backend.as_ref().expect("keyring backend initialized");
                let mut result = block_with_timeout(&runtime, "load", load_async(active, &account));
                if result.is_err() && local_forced() && matches!(active, Backend::Portal(_)) {
                    match runtime.block_on(open_backend()) {
                        Ok(opened) => {
                            backend = Some(opened);
                            let active =
                                backend.as_ref().expect("local keyring backend initialized");
                            result =
                                block_with_timeout(&runtime, "load", load_async(active, &account));
                        }
                        Err(error) => result = Err(format!("{error:#}")),
                    }
                }
                let _ = reply.send(result);
            }
            Request::Delete { account, reply } => {
                let active = backend.as_ref().expect("keyring backend initialized");
                let mut result =
                    block_with_timeout(&runtime, "delete", delete_async(active, &account));
                if result.is_err() && local_forced() && matches!(active, Backend::Portal(_)) {
                    match runtime.block_on(open_backend()) {
                        Ok(opened) => {
                            backend = Some(opened);
                            let active =
                                backend.as_ref().expect("local keyring backend initialized");
                            result = block_with_timeout(
                                &runtime,
                                "delete",
                                delete_async(active, &account),
                            );
                        }
                        Err(error) => result = Err(format!("{error:#}")),
                    }
                }
                let _ = reply.send(result);
            }
        }
    }
}

/// Run one keyring operation under [`OP_TIMEOUT`]. A timed-out operation also
/// demotes the process to the local backend: a portal that stops answering
/// mid-run answers nothing afterwards either.
fn block_with_timeout<T>(
    runtime: &tokio::runtime::Runtime,
    label: &str,
    future: impl Future<Output = Result<T>>,
) -> Result<T, String> {
    block_with_deadline(runtime, label, OP_TIMEOUT, future)
}

fn block_with_deadline<T>(
    runtime: &tokio::runtime::Runtime,
    label: &str,
    timeout: Duration,
    future: impl Future<Output = Result<T>>,
) -> Result<T, String> {
    // The timeout has to be *created* inside the runtime context — building it
    // as an argument to block_on registers a timer with no reactor and panics.
    match runtime.block_on(async move { tokio::time::timeout(timeout, future).await }) {
        Ok(result) => result.map_err(|error| format!("{error:#}")),
        Err(_) => {
            let message = format!("keyring {label} timed out after {}s", timeout.as_secs_f32());
            force_local(&message);
            Err(message)
        }
    }
}

async fn open_backend() -> Result<Backend> {
    if !local_only() {
        let started = Instant::now();
        match tokio::time::timeout(OP_TIMEOUT, oo7::Keyring::new()).await {
            Ok(Ok(keyring)) => {
                crate::mlog!(
                    Level::Warn,
                    "keyring",
                    "portal keyring ready in {:?}",
                    started.elapsed()
                );
                return Ok(Backend::Portal(keyring));
            }
            Ok(Err(error)) => force_local(&format!("portal keyring unavailable: {error}")),
            Err(_) => force_local(&format!(
                "the Secret portal did not answer within {}s \
                 (no org.freedesktop.portal.Secret backend installed?)",
                OP_TIMEOUT.as_secs()
            )),
        }
    }
    let keyring = open_local().await?;
    crate::mlog!(Level::Warn, "keyring", "local file keyring ready");
    Ok(Backend::Local(keyring))
}

/// Open (creating on first use) the app-private keyring file. Its encryption
/// key lives in a 0600 file beside it, so this protects the secrets at rest
/// about as well as the SQLCipher key it also holds — strictly weaker than a
/// real Secret Service, and only used when there is none.
async fn open_local() -> Result<oo7::file::UnlockedKeyring> {
    let dir = local_dir()?;
    let secret = local_secret(&dir)?;
    oo7::file::UnlockedKeyring::load(dir.join("meron.keyring"), oo7::Secret::text(&secret))
        .await
        .context("open local file keyring")
}

/// `<app dir>/keyring`, created 0700. Deliberately *not* oo7's default path
/// (`~/.local/share/keyrings`), which belongs to gnome-keyring.
fn local_dir() -> Result<PathBuf> {
    let dir = crate::store::app_dir().join("keyring");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create keyring dir {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Read (or mint) the local keyring's encryption key: 64 hex chars in a 0600
/// file. A malformed file is replaced — a fresh key loses stored secrets, which
/// is the same position as having no keyring file at all.
fn local_secret(dir: &Path) -> Result<String> {
    let path = dir.join("local.key");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim().to_owned();
        if is_hex_key(&existing) {
            return Ok(existing);
        }
        crate::mlog!(
            Level::Warn,
            "keyring",
            "local keyring key at {} is malformed, minting a new one",
            path.display()
        );
    }
    let key = generate_key();
    write_private(&path, &key).with_context(|| format!("write keyring key {}", path.display()))?;
    Ok(key)
}

fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

fn generate_key() -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for _ in 0..2 {
        for byte in uuid::Uuid::new_v4().as_bytes() {
            let _ = write!(hex, "{byte:02x}");
        }
    }
    hex
}

fn is_hex_key(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn attributes(account: &str) -> HashMap<&str, &str> {
    HashMap::from([("service", SERVICE), ("username", account)])
}

async fn store_async(backend: &Backend, account: &str, value: &str) -> Result<()> {
    let label = format!("{account}@{SERVICE}");
    match backend {
        Backend::Portal(keyring) => keyring
            .create_item(&label, &attributes(account), oo7::Secret::text(value), true)
            .await
            .map_err(Into::into),
        Backend::Local(keyring) => keyring
            .create_item(&label, &attributes(account), oo7::Secret::text(value), true)
            .await
            .map(|_| ())
            .map_err(Into::into),
    }
}

async fn load_async(backend: &Backend, account: &str) -> Result<Option<String>> {
    let bytes = match backend {
        Backend::Portal(keyring) => {
            let Some(item) = keyring.search_items(&attributes(account)).await?.pop() else {
                return Ok(None);
            };
            item.secret().await?.as_bytes().to_vec()
        }
        Backend::Local(keyring) => {
            let Some(item) = keyring.search_items(&attributes(account)).await?.pop() else {
                return Ok(None);
            };
            item.as_unlocked().secret().as_bytes().to_vec()
        }
    };
    String::from_utf8(bytes)
        .map(Some)
        .context("keyring entry is not UTF-8")
}

async fn delete_async(backend: &Backend, account: &str) -> Result<()> {
    match backend {
        Backend::Portal(keyring) => keyring
            .delete(&attributes(account))
            .await
            .map_err(Into::into),
        Backend::Local(keyring) => keyring
            .delete(&attributes(account))
            .await
            .map_err(Into::into),
    }
}

/// Wait for the worker's reply, bounded so a wedged backend surfaces as an
/// error to the caller rather than as a hung sidecar.
fn response<T>(receiver: mpsc::Receiver<Result<T, String>>) -> Result<T> {
    receiver
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => {
                force_local("keyring worker did not reply in time");
                anyhow!(
                    "keyring worker timed out after {}s",
                    REPLY_TIMEOUT.as_secs()
                )
            }
            mpsc::RecvTimeoutError::Disconnected => anyhow!("keyring worker stopped"),
        })?
        .map_err(anyhow::Error::msg)
}

pub fn store(account: &str, value: &str) -> Result<()> {
    let (reply, receiver) = mpsc::channel();
    portal_keyring()
        .requests
        .send(Request::Store {
            account: account.to_owned(),
            value: value.to_owned(),
            reply,
        })
        .map_err(|_| anyhow!("portal keyring worker stopped"))?;
    response(receiver)
}

pub fn load(account: &str) -> Result<Option<String>> {
    let (reply, receiver) = mpsc::channel();
    portal_keyring()
        .requests
        .send(Request::Load {
            account: account.to_owned(),
            reply,
        })
        .map_err(|_| anyhow!("portal keyring worker stopped"))?;
    response(receiver)
}

pub fn delete(account: &str) -> Result<()> {
    let (reply, receiver) = mpsc::channel();
    portal_keyring()
        .requests
        .send(Request::Delete {
            account: account.to_owned(),
            reply,
        })
        .map_err(|_| anyhow!("portal keyring worker stopped"))?;
    response(receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyring_attributes_are_app_scoped() {
        let attributes = attributes("account-1");
        assert_eq!(attributes.get("service"), Some(&"meron"));
        assert_eq!(attributes.get("username"), Some(&"account-1"));
    }

    #[test]
    fn generated_key_is_a_64_char_hex_string() {
        let key = generate_key();
        assert!(is_hex_key(&key), "{key}");
        assert!(!is_hex_key(""));
        assert!(!is_hex_key(&"z".repeat(64)));
    }

    /// The regression this module exists for: a keyring call that never
    /// resolves must give up and demote, not block its caller forever. The
    /// Secret portal did exactly this on desktops with no Secret backend,
    /// wedging the sidecar before it read its first request.
    #[test]
    fn a_wedged_operation_times_out_and_demotes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let wedged = std::future::pending::<Result<()>>();
        let error = block_with_deadline(&runtime, "load", Duration::from_millis(50), wedged)
            .expect_err("a pending future must time out");
        assert!(error.contains("timed out"), "{error}");
        assert!(local_forced(), "a timeout demotes to the local keyring");
    }

    #[test]
    fn local_secret_is_stable_and_private() {
        let dir = std::env::temp_dir().join(format!("meron-keyring-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = local_secret(&dir).unwrap();
        let second = local_secret(&dir).unwrap();
        assert_eq!(first, second);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("local.key"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // A corrupted key file is replaced rather than propagated.
        std::fs::write(dir.join("local.key"), "not-a-key").unwrap();
        let third = local_secret(&dir).unwrap();
        assert!(is_hex_key(&third));
        assert_ne!(third, first);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn persisted_local_backend_requires_both_keyring_files() {
        let dir = std::env::temp_dir().join(format!("meron-keyring-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!local_keyring_exists_in(&dir));
        std::fs::write(dir.join("local.key"), "key").unwrap();
        assert!(!local_keyring_exists_in(&dir));
        std::fs::write(dir.join("meron.keyring"), "keyring").unwrap();
        assert!(local_keyring_exists_in(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
