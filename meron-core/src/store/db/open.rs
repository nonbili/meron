//! Where the store and the archive live, and opening them: the SQLCipher key,
//! the one-time upgrade of a plaintext store to an encrypted one, connection
//! pragmas, schema migrations and data upgrades, and what happens when the
//! store belongs to a newer Meron.

use super::migrations::{self, NewerSchema, SCHEMA_VERSION, run_migrations, user_version};
use super::retry::{with_anyhow_busy_retry, with_busy_retry};
use super::{data_upgrades, meta_get, meta_set};
use anyhow::{Context, Result};
use rusqlite::config::DbConfig;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `meta` key holding the newest schema the user chose to open anyway (see
/// [`accept_newer_schema_at`]).
const ACCEPTED_NEWER_SCHEMA: &str = "accepted_newer_schema";

pub fn open() -> Result<Connection> {
    open_notifying(|| {})
}

/// [`open`], calling `on_upgrade` first if this open has to rewrite an existing
/// store, which can take a while on a large cache. A brand-new store and an
/// up-to-date one stay silent.
pub fn open_notifying(on_upgrade: impl FnOnce()) -> Result<Connection> {
    let path = db_path();
    let key = store_key()?;
    open_inner(&path, key.as_deref(), on_upgrade)
}

/// The store's key from the keychain, resolved once per process. `None` when
/// the keychain is disabled (tests, headless, `MERON_KEYRING=off`), which
/// keeps the store plaintext.
fn store_key() -> Result<Option<String>> {
    if let Some(key) = STORE_KEY.get() {
        return Ok(key.clone());
    }
    let path = db_path();
    // The key comes from the OS keychain, historically the slowest and most
    // failure-prone step of startup (a Flatpak Secret portal with no backend
    // used to hang here forever). Bracket it in the log so a stall is visible.
    crate::mlog!(crate::log::Level::Warn, "store", "resolving store key");
    let started = std::time::Instant::now();
    let store_exists = is_encrypted_database(&path);
    let key = crate::secrets::db_key(store_exists)
        .map(|key| key.map(|key| recover_store_key(&path, store_exists, key)));
    crate::mlog!(
        crate::log::Level::Warn,
        "store",
        "store key resolved in {:?}: {}",
        started.elapsed(),
        match &key {
            Ok(Some(_)) => "encrypted".to_string(),
            Ok(None) => "plaintext (keychain disabled)".to_string(),
            Err(error) => format!("failed: {error:#}"),
        }
    );
    let key = key?;
    let _ = STORE_KEY.set(key.clone());
    Ok(key)
}

/// [`open_notifying`] for a store at a host-chosen path and key.
pub fn open_at_notifying(
    path: impl AsRef<Path>,
    key: Option<&str>,
    on_upgrade: impl FnOnce(),
) -> Result<Connection> {
    open_inner(path.as_ref(), key, on_upgrade)
}

/// The key [`open`] resolved, so the local-folder archive opens with the same
/// one without asking the keychain a second time.
static STORE_KEY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Open the local-folder archive (`archive.db`, beside the store), encrypted
/// with the store's key. See `store::archive`.
pub fn open_archive() -> Result<Connection> {
    open_archive_at(archive_path(), store_key()?.as_deref())
}

/// `key` if it opens the store at `path` (or there is no encrypted store yet),
/// else the key a shadowed keyring holds when that one does. Falls back to
/// `key`, so a store neither key opens still fails with the usual error.
fn recover_store_key(path: &Path, store_exists: bool, key: String) -> String {
    if !store_exists || key_opens(path, &key) {
        return key;
    }
    match crate::secrets::recover_shadowed_db_key(|candidate| key_opens(path, candidate)) {
        Some(recovered) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "store",
                "recovered the store key from the keyring the local one was shadowing"
            );
            recovered
        }
        None => key,
    }
}

/// Whether `key` decrypts the existing store at `path`. Only reads the schema.
fn key_opens(path: &Path, key: &str) -> bool {
    if check_key(key).is_err() {
        return false;
    }
    let Ok(conn) = open_holding_wal(path) else {
        return false;
    };
    apply_key(&conn, key).is_ok() && database_readable(&conn)
}

/// Open `path` without letting the connection fold the WAL into the database
/// when it closes, which SQLite otherwise does for the last connection. Until
/// the version check has passed, the store may belong to a newer Meron, whose
/// committed-but-uncheckpointed frames this version must not write into the
/// file — not even when the user then quits. [`release_wal`] lifts the hold.
fn open_holding_wal(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("open db {}", path.display()))?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .context("hold the WAL")?;
    Ok(conn)
}

/// The store is this version's to write (or the user said so): checkpoint on
/// close as usual.
fn release_wal(conn: &Connection) -> Result<()> {
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, false)
        .context("release the WAL")?;
    Ok(())
}

/// Open an archive at `path`, keyed when `key` is given. The archive shares
/// the store's schema, so every message-list and thread read works on it
/// unchanged, plus the `archive_raw` table holding the original messages.
/// It runs the schema migrations but none of the data upgrades (see
/// `data_upgrades`).
pub fn open_archive_at(path: impl AsRef<Path>, key: Option<&str>) -> Result<Connection> {
    let path = path.as_ref();
    if let Some(key) = key {
        check_key(key)?;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let fresh = !path.exists();
    let conn = with_anyhow_busy_retry(|| open_keyed_connection(path, key, || {}))?;
    conn.busy_timeout(Duration::from_millis(5000))
        .context("set busy timeout")?;
    let accepted_newer = refuse_newer(&conn)?;
    release_wal(&conn)?;
    if accepted_newer {
        return Ok(conn);
    }
    // Deleting archived mail should give the disk space back. auto_vacuum only
    // takes effect when set before the first table exists.
    if fresh {
        conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")
            .context("set archive auto_vacuum")?;
    }
    configure_connection(&conn)?;
    // The archive holds the only copy of its mail: a commit must reach the
    // disk before the server copy is removed.
    conn.execute_batch("PRAGMA synchronous = FULL;")
        .context("set archive synchronous")?;
    with_anyhow_busy_retry(|| run_migrations(&conn)).context("run migrations")?;
    conn.execute_batch(super::super::archive::ARCHIVE_DDL)
        .context("create archive tables")?;
    Ok(conn)
}

pub(super) fn archive_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_ARCHIVE_DB") {
        return PathBuf::from(path);
    }
    app_dir().join("archive.db")
}

/// Open an unencrypted store. SQLCipher leaves a database untouched until a key
/// is set, so this stays byte-compatible with pre-encryption databases and is
/// what tests and the headless/`MERON_KEYRING=off` path use.
pub fn open_at(path: impl AsRef<Path>) -> Result<Connection> {
    open_inner(path.as_ref(), None, || {})
}

/// Open an encrypted store, keying the connection with `key` (64 hex chars = a
/// raw 32-byte SQLCipher key). A legacy plaintext database at `path` is migrated
/// in place to an encrypted one on first keyed open.
pub fn open_at_keyed(path: impl AsRef<Path>, key: &str) -> Result<Connection> {
    open_inner(path.as_ref(), Some(key), || {})
}

fn open_inner(path: &Path, key: Option<&str>, on_upgrade: impl FnOnce()) -> Result<Connection> {
    if let Some(key) = key {
        check_key(key)?;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut on_upgrade = Some(on_upgrade);
    let mut announce_upgrade = || {
        if let Some(on_upgrade) = on_upgrade.take() {
            on_upgrade();
        }
    };
    // Retried so a second first-open waits out the one encrypting the store.
    let conn = with_anyhow_busy_retry(|| open_keyed_connection(path, key, &mut announce_upgrade))?;
    conn.busy_timeout(Duration::from_millis(5000))
        .context("set busy timeout")?;
    let accepted_newer = refuse_newer(&conn)?;
    release_wal(&conn)?;
    if accepted_newer {
        return Ok(conn);
    }
    configure_connection(&conn)?;
    if upgrade_pending(&conn) {
        announce_upgrade();
    }
    with_anyhow_busy_retry(|| run_migrations(&conn)).context("run migrations")?;
    with_anyhow_busy_retry(|| data_upgrades::invalidate_body_cache_if_needed(&conn))
        .context("invalidate body cache if needed")?;
    with_anyhow_busy_retry(|| data_upgrades::reread_synced_repeat_rules(&conn))
        .context("reread synced repeat rules")?;
    Ok(conn)
}

/// For a store a newer Meron last used: fails with [`NewerSchema`] unless the
/// user chose to open this schema anyway, in which case it answers `true` and
/// the store opens as it is — no pragmas, migrations or data upgrades, all of
/// which would rewrite data the newer version owns. `false` for every other
/// store. Runs before anything is written.
fn refuse_newer(conn: &Connection) -> Result<bool> {
    let found = user_version(conn)?;
    if found <= SCHEMA_VERSION {
        return Ok(false);
    }
    let accepted = meta_get(conn, ACCEPTED_NEWER_SCHEMA)
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok());
    if accepted.is_some_and(|accepted| accepted >= found) {
        crate::mlog!(
            crate::log::Level::Warn,
            "store",
            "opening schema {found} as the user chose; this version supports {SCHEMA_VERSION}"
        );
        return Ok(true);
    }
    Err(NewerSchema {
        found,
        supported: SCHEMA_VERSION,
    }
    .into())
}

/// The store at `path` when a newer Meron last used it and the user hasn't
/// chosen to open that schema anyway; `None` for every other store, including
/// one this can't read (opening it reports why). Reads the version and the
/// user's choice only, so a host can ask before anything opens the store.
pub fn check_newer_schema_at(path: impl AsRef<Path>, key: Option<&str>) -> Option<NewerSchema> {
    let path = path.as_ref();
    if !path.exists() || key.is_some_and(|key| check_key(key).is_err()) {
        return None;
    }
    let conn = open_holding_wal(path).ok()?;
    if let Some(key) = key.filter(|_| is_encrypted_database(path)) {
        apply_key(&conn, key).ok()?;
    }
    match refuse_newer(&conn) {
        Err(err) => err.downcast_ref::<NewerSchema>().copied(),
        Ok(_) => None,
    }
}

/// Record that the user chose to open the store at `path`, which a newer
/// Meron last used, anyway: every open then takes it as it is. `shown` is the
/// schema the user was asked about. That, and not whatever is on disk by now,
/// is what gets recorded, so a store an even newer Meron has touched since is
/// asked about again. A store this version knows is left alone.
pub fn accept_newer_schema_at(path: impl AsRef<Path>, key: Option<&str>, shown: i64) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(());
    }
    if let Some(key) = key {
        check_key(key)?;
    }
    let conn = open_holding_wal(path)?;
    if let Some(key) = key.filter(|_| is_encrypted_database(path)) {
        apply_key(&conn, key)?;
    }
    conn.busy_timeout(Duration::from_millis(5000))?;
    let found = user_version(&conn)?;
    // Past what the user was shown, the choice doesn't cover it: write
    // nothing, and the next open asks about the schema now there.
    if found > SCHEMA_VERSION && found <= shown {
        release_wal(&conn)?;
        meta_set(&conn, ACCEPTED_NEWER_SCHEMA, &shown.to_string())?;
    }
    Ok(())
}

/// This profile's databases a newer Meron last used and the user hasn't
/// chosen to open anyway: the store, and the local-folder archive beside it.
pub fn newer_schemas() -> Result<Vec<(PathBuf, NewerSchema)>> {
    let key = store_key()?;
    Ok([db_path(), archive_path()]
        .into_iter()
        .filter_map(|path| {
            let newer = check_newer_schema_at(&path, key.as_deref())?;
            Some((path, newer))
        })
        .collect())
}

/// [`accept_newer_schema_at`] for each of [`newer_schemas`], at the schema
/// it found.
pub fn accept_newer_schemas(shown: &[(PathBuf, NewerSchema)]) -> Result<()> {
    let key = store_key()?;
    for (path, newer) in shown {
        accept_newer_schema_at(path, key.as_deref(), newer.found)?;
    }
    Ok(())
}

/// The profile's store, for hosts that name it (a newer-schema prompt that
/// only the open itself noticed).
pub fn store_path() -> PathBuf {
    db_path()
}

fn configure_connection(conn: &Connection) -> Result<()> {
    // WAL + synchronous=NORMAL is the durable-but-fast desktop combo; the busy
    // timeout (set as the connection opens) avoids spurious SQLITE_BUSY now
    // that a reader can overlap a writer under WAL. foreign_keys is on by
    // default in the bundled SQLite; it is set anyway because the CardDAV
    // tables rely on ON DELETE CASCADE.
    with_busy_retry(|| {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
    })
    .context("set connection pragmas")?;
    Ok(())
}

/// Opens `path` and applies the SQLCipher `key` (if any). When the key is set
/// but the file turns out to be an unencrypted legacy database, it is migrated
/// in place to an encrypted database before returning, calling `on_encrypt`
/// first because that exports the whole file.
fn open_keyed_connection(
    path: &Path,
    key: Option<&str>,
    on_encrypt: impl FnOnce(),
) -> Result<Connection> {
    let conn = open_holding_wal(path)?;
    let Some(key) = key else {
        return Ok(conn);
    };
    apply_key(&conn, key)?;
    if database_readable(&conn) {
        return Ok(conn);
    }
    // The key didn't unlock the file. Either it predates encryption and is
    // plaintext (the one-time upgrade below), or we were handed the wrong key —
    // which happens when the keychain lost our entry and minted a fresh one.
    // Refuse the migration in that case: it would fail anyway, and the explicit
    // error names the real problem instead of blaming the schema.
    if is_encrypted_database(path) {
        anyhow::bail!(
            "{} is encrypted with a different key — the keychain entry holding \
             the store key is missing or was replaced",
            path.display()
        );
    }
    drop(conn);
    // A plaintext store from a newer Meron is not this version's to rewrite:
    // refused, or, when the user chose to open it anyway, opened as it is.
    {
        let plain = open_holding_wal(path)?;
        if refuse_newer(&plain)? {
            return Ok(plain);
        }
    }
    on_encrypt();
    encrypt_plaintext_db(path, key)
        .with_context(|| format!("encrypt legacy plaintext db {}", path.display()))?;
    let conn = open_holding_wal(path).with_context(|| format!("reopen db {}", path.display()))?;
    apply_key(&conn, key)?;
    if !database_readable(&conn) {
        anyhow::bail!(
            "database still unreadable after encryption migration: {}",
            path.display()
        );
    }
    Ok(conn)
}

/// A raw SQLCipher key is 64 hex characters (32 bytes). Anything else is a
/// caller's mistake, and must not reach the SQL it is written into.
fn check_key(key: &str) -> Result<()> {
    if crate::secrets::is_hex_key(key) {
        Ok(())
    } else {
        anyhow::bail!("the store key is malformed: expected 64 hex characters")
    }
}

/// Apply the raw 32-byte key (as 64 hex chars, checked by the caller). Using
/// the raw-key form makes SQLCipher skip PBKDF2 derivation, so opening is
/// cheap and deterministic.
fn apply_key(conn: &Connection, key: &str) -> Result<()> {
    check_key(key)?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{key}'\";"))
        .context("apply sqlcipher key")
}

/// Whether `path` holds an existing *encrypted* database, i.e. a non-empty file
/// that does not carry SQLite's plaintext header. Used to tell "wrong key" from
/// "legacy plaintext database" without guessing.
fn is_encrypted_database(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 16];
    match file.read_exact(&mut header) {
        Ok(()) => &header != b"SQLite format 3\0",
        // Shorter than a header: a freshly created or empty file, not encrypted.
        Err(_) => false,
    }
}

/// A cheap probe that succeeds only when the page cipher matches the file: a
/// plaintext file opened with a key (or a wrong key) fails to decrypt here.
fn database_readable(conn: &Connection) -> bool {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .is_ok()
}

/// One-time upgrade of a plaintext database to an encrypted one. Exports the
/// plaintext contents into a fresh encrypted sibling via `sqlcipher_export`,
/// checks it, then atomically replaces the original and drops its now-stale
/// WAL/SHM files.
fn encrypt_plaintext_db(path: &Path, key: &str) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("meron.db");
    let tmp = path.with_file_name(format!(
        "{file_name}.sqlcipher-migrating-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| -> Result<()> {
        let expected_version = {
            let plain = Connection::open(path)
                .with_context(|| format!("open plaintext {}", path.display()))?;
            // Fold any WAL frames into the main file, so the export sees every
            // commit and no WAL is left to pair with the encrypted file. This
            // fails while another connection has the database open, and then
            // replacing the file under it would corrupt it: stop.
            let mode: String = plain
                .pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0))
                .with_context(|| format!("{} is in use elsewhere", path.display()))?;
            if !mode.eq_ignore_ascii_case("delete") {
                anyhow::bail!("{} is in use elsewhere", path.display());
            }
            // `sqlcipher_export` copies the schema and rows but neither
            // `user_version` nor `auto_vacuum`, so carry them across:
            // otherwise migrations re-run on the encrypted copy and trip over
            // the already-migrated schema, and an archive stops returning
            // deleted mail's space.
            let user_version: i64 = plain.query_row("PRAGMA user_version", [], |r| r.get(0))?;
            let auto_vacuum: i64 = plain.query_row("PRAGMA auto_vacuum", [], |r| r.get(0))?;
            plain.execute(
                "ATTACH DATABASE ?1 AS encrypted KEY ?2",
                params![tmp.to_string_lossy(), format!("x'{key}'")],
            )?;
            let exported = plain.execute_batch(&format!(
                "PRAGMA encrypted.auto_vacuum = {auto_vacuum};
                 SELECT sqlcipher_export('encrypted');
                 PRAGMA encrypted.user_version = {user_version};"
            ));
            let detached = plain.execute_batch("DETACH DATABASE encrypted;");
            exported
                .and(detached)
                .context("sqlcipher_export to encrypted db")?;
            user_version
        };
        // Check the copy before it replaces the only original.
        {
            let encrypted = Connection::open(&tmp)?;
            apply_key(&encrypted, key)?;
            let check: String = encrypted.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
            if check != "ok" || user_version(&encrypted)? != expected_version {
                anyhow::bail!("the encrypted copy failed its check: {check}");
            }
        }
        sync_file(&tmp).context("flush encrypted db")?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }
    std::fs::rename(&tmp, path).context("replace plaintext db with encrypted db")?;
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(sibling_with_suffix(path, suffix));
    }
    sync_dir(path);
    Ok(())
}

/// Flush a file's contents to disk. Opened for writing, which Windows needs
/// to flush (`FlushFileBuffers`); nothing is written or truncated.
pub(super) fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .sync_all()
}

/// Make a rename or new file in `path`'s directory durable, where the
/// platform allows syncing a directory.
pub(super) fn sync_dir(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        let _ = std::fs::File::open(dir).and_then(|dir| dir.sync_all());
    }
    #[cfg(not(unix))]
    let _ = path;
}

pub(super) fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub(super) fn db_path() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_CORE_DB") {
        return PathBuf::from(path);
    }
    config_dir().join("meron.db")
}

/// The directory holding the store, and the natural home for anything else that
/// belongs to this app profile (the local keyring, for one). Follows
/// `MERON_CORE_DB`, so dev and production profiles stay separate.
pub fn app_dir() -> PathBuf {
    db_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(config_dir)
}

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config/meron")
}

/// Whether opening this store will rewrite existing data: a schema behind
/// [`SCHEMA_VERSION`], or a pending data upgrade. Only decides whether to tell
/// the user, so a failed read just answers "no".
fn upgrade_pending(conn: &Connection) -> bool {
    let version = user_version(conn).unwrap_or(0);
    if version == 0 {
        return false;
    }
    version < migrations::SCHEMA_VERSION || data_upgrades::pending(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    const OTHER_KEY: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

    fn tmp_db_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meron-db-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("meron.db")
    }

    fn setting(conn: &Connection) -> String {
        conn.query_row("SELECT value FROM settings WHERE key='k'", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn keyed_open_round_trips() {
        let path = tmp_db_path();
        {
            let conn = open_at_keyed(&path, TEST_KEY).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
                .unwrap();
        }
        // Reopening with the key sees the data.
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        assert_eq!(setting(&conn), "v");
        // The on-disk file is encrypted: a plaintext open can't read the schema.
        assert!(!database_readable(&Connection::open(&path).unwrap()));
    }

    #[test]
    fn legacy_plaintext_is_migrated_to_encrypted() {
        let path = tmp_db_path();
        // Simulate a pre-encryption store: create it plaintext with some data.
        {
            let conn = open_at(&path).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'legacy')", [])
                .unwrap();
        }
        assert!(database_readable(&Connection::open(&path).unwrap()));

        // First keyed open migrates the plaintext file in place, preserving data.
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        assert_eq!(setting(&conn), "legacy");
        drop(conn);

        // The file is now encrypted and still readable with the key on reopen,
        // and no temporary copy is left behind.
        assert!(!database_readable(&Connection::open(&path).unwrap()));
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        assert_eq!(setting(&conn), "legacy");
        let leftovers = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains("migrating"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn a_plaintext_archive_keeps_incremental_vacuum_when_encrypted() {
        let path = tmp_db_path().with_file_name("archive.db");
        drop(open_archive_at(&path, None).unwrap());
        let conn = open_archive_at(&path, Some(TEST_KEY)).unwrap();
        let auto_vacuum: i64 = conn
            .query_row("PRAGMA auto_vacuum", [], |row| row.get(0))
            .unwrap();
        assert_eq!(auto_vacuum, 2, "INCREMENTAL");
    }

    #[test]
    fn plaintext_in_use_elsewhere_is_not_replaced() {
        let path = tmp_db_path();
        let other = open_at(&path).unwrap();
        other
            .execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
            .unwrap();
        let error = open_at_keyed(&path, TEST_KEY).unwrap_err();
        assert!(format!("{error:#}").contains("in use"), "{error:#}");
        drop(other);
        assert!(database_readable(&Connection::open(&path).unwrap()));
        assert_eq!(setting(&open_at_keyed(&path, TEST_KEY).unwrap()), "v");
    }

    #[test]
    fn wrong_key_on_an_encrypted_store_is_named_not_migrated() {
        let path = tmp_db_path();
        {
            let conn = open_at_keyed(&path, TEST_KEY).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
                .unwrap();
        }
        assert!(is_encrypted_database(&path));

        // A keychain that lost our entry hands back a freshly minted key. Opening
        // with it must report that, not run the plaintext migration over an
        // encrypted file.
        let error = open_at_keyed(&path, OTHER_KEY).unwrap_err().to_string();
        assert!(error.contains("encrypted with a different key"), "{error}");

        // The original file is untouched: the real key still opens it.
        let conn = open_at_keyed(&path, TEST_KEY).unwrap();
        assert_eq!(setting(&conn), "v");
    }

    #[test]
    fn malformed_keys_are_refused_before_touching_the_file() {
        let path = tmp_db_path();
        for key in ["", "abc", "x'; DROP TABLE settings; --", &"g".repeat(64)] {
            let error = open_at_keyed(&path, key).unwrap_err().to_string();
            assert!(error.contains("malformed"), "{error}");
            assert!(
                !error.contains(key) || key.is_empty(),
                "the key leaked into the error"
            );
            let error = open_archive_at(&path, Some(key)).unwrap_err().to_string();
            assert!(error.contains("malformed"), "{error}");
            assert!(!key_opens(&path, key));
        }
        assert!(!path.exists(), "nothing was created");
    }

    #[test]
    fn key_opens_tells_the_right_key_from_a_minted_one() {
        let path = tmp_db_path();
        drop(open_at_keyed(&path, TEST_KEY).unwrap());
        assert!(key_opens(&path, TEST_KEY));
        assert!(!key_opens(&path, OTHER_KEY));
        // A key that opens is kept as is, with no keychain involved.
        assert_eq!(recover_store_key(&path, true, TEST_KEY.into()), TEST_KEY);
    }

    #[test]
    fn plaintext_and_missing_files_are_not_taken_for_encrypted() {
        let path = tmp_db_path();
        assert!(!is_encrypted_database(&path), "missing file");
        drop(open_at(&path).unwrap());
        assert!(!is_encrypted_database(&path), "plaintext file");
    }

    fn make_newer(path: &Path, key: Option<&str>) {
        let conn = match key {
            Some(key) => open_at_keyed(path, key).unwrap(),
            None => open_at(path).unwrap(),
        };
        conn.execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
            .unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
    }

    fn file_bytes(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    #[test]
    fn a_newer_store_is_refused_until_accepted() {
        let path = tmp_db_path();
        make_newer(&path, Some(TEST_KEY));
        // Let the newer version's WAL settle into the file first, so the
        // comparison below sees only what this open does.
        drop(open_keyed_connection(&path, Some(TEST_KEY), || {}).unwrap());
        let before = file_bytes(&path);
        let error = open_at_notifying(&path, Some(TEST_KEY), || panic!("no upgrade")).unwrap_err();
        assert_eq!(
            error.downcast_ref::<NewerSchema>(),
            Some(&NewerSchema {
                found: SCHEMA_VERSION + 1,
                supported: SCHEMA_VERSION
            })
        );
        assert_eq!(file_bytes(&path), before, "a refused store is untouched");

        accept_newer_schema_at(&path, Some(TEST_KEY), SCHEMA_VERSION + 1).unwrap();
        let conn = open_at_notifying(&path, Some(TEST_KEY), || panic!("no upgrade")).unwrap();
        assert_eq!(setting(&conn), "v");
        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION + 1);
        drop(conn);

        // Accepting a schema older than the one on disk writes nothing.
        let before = file_bytes(&path);
        accept_newer_schema_at(&path, Some(TEST_KEY), SCHEMA_VERSION).unwrap();
        assert_eq!(file_bytes(&path), before);

        // An even newer schema asks again.
        let conn = open_keyed_connection(&path, Some(TEST_KEY), || {}).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 2)
            .unwrap();
        drop(conn);
        let error = open_at_keyed(&path, TEST_KEY).unwrap_err();
        assert!(error.downcast_ref::<NewerSchema>().is_some());
        // A choice made about the older schema doesn't cover it, and writes
        // nothing into it.
        let before = file_bytes(&path);
        accept_newer_schema_at(&path, Some(TEST_KEY), SCHEMA_VERSION + 1).unwrap();
        assert_eq!(file_bytes(&path), before);
        assert!(open_at_keyed(&path, TEST_KEY).is_err());
    }

    #[test]
    fn checking_for_a_newer_store_reads_without_opening() {
        let path = tmp_db_path();
        assert_eq!(check_newer_schema_at(&path, Some(TEST_KEY)), None);
        assert!(!path.exists(), "the check created nothing");
        drop(open_at_keyed(&path, TEST_KEY).unwrap());
        assert_eq!(check_newer_schema_at(&path, Some(TEST_KEY)), None);
        make_newer(&path, Some(TEST_KEY));
        let newer = NewerSchema {
            found: SCHEMA_VERSION + 1,
            supported: SCHEMA_VERSION,
        };
        assert_eq!(check_newer_schema_at(&path, Some(TEST_KEY)), Some(newer));
        assert_eq!(check_newer_schema_at(&path, Some(OTHER_KEY)), None);
        accept_newer_schema_at(&path, Some(TEST_KEY), SCHEMA_VERSION + 1).unwrap();
        assert_eq!(check_newer_schema_at(&path, Some(TEST_KEY)), None);
    }

    /// A newer Meron that stopped with commits still in its WAL: refusing the
    /// store, and checking it, must leave those frames where they are rather
    /// than fold them into the database as the last connection closes.
    #[test]
    fn refusing_a_newer_store_leaves_its_wal_alone() {
        let path = tmp_db_path();
        let wal = sibling_with_suffix(&path, "-wal");
        drop(open_at_keyed(&path, TEST_KEY).unwrap());
        {
            let newer = open_keyed_connection(&path, Some(TEST_KEY), || {}).unwrap();
            newer
                .execute_batch("PRAGMA wal_autocheckpoint = 0;")
                .unwrap();
            newer
                .execute("INSERT INTO settings(key, value) VALUES('k', 'newer')", [])
                .unwrap();
            newer
                .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
            // Dropped holding the WAL, as a crash would leave it.
        }
        let (db_before, wal_before) = (file_bytes(&path), file_bytes(&wal));
        assert!(!wal_before.is_empty());
        assert!(open_at_keyed(&path, TEST_KEY).is_err());
        assert!(check_newer_schema_at(&path, Some(TEST_KEY)).is_some());
        assert!(key_opens(&path, TEST_KEY));
        assert_eq!(
            file_bytes(&path),
            db_before,
            "the database file is untouched"
        );
        assert_eq!(file_bytes(&wal), wal_before, "the WAL is untouched");
        // Once accepted, the store opens with the newer commits in it.
        accept_newer_schema_at(&path, Some(TEST_KEY), SCHEMA_VERSION + 1).unwrap();
        assert_eq!(setting(&open_at_keyed(&path, TEST_KEY).unwrap()), "newer");
    }

    #[test]
    fn a_newer_plaintext_store_is_not_encrypted() {
        let path = tmp_db_path();
        make_newer(&path, None);
        let error = open_at_keyed(&path, TEST_KEY).unwrap_err();
        assert!(error.downcast_ref::<NewerSchema>().is_some(), "{error:#}");
        assert!(!is_encrypted_database(&path));
        // Opened anyway, it stays as the newer version left it: plaintext.
        accept_newer_schema_at(&path, None, SCHEMA_VERSION + 1).unwrap();
        assert_eq!(setting(&open_at_keyed(&path, TEST_KEY).unwrap()), "v");
        assert!(!is_encrypted_database(&path));
    }

    #[test]
    fn a_newer_archive_is_refused_until_accepted() {
        let path = tmp_db_path().with_file_name("archive.db");
        drop(open_archive_at(&path, Some(TEST_KEY)).unwrap());
        {
            let conn = open_keyed_connection(&path, Some(TEST_KEY), || {}).unwrap();
            conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }
        let error = open_archive_at(&path, Some(TEST_KEY)).unwrap_err();
        assert!(error.downcast_ref::<NewerSchema>().is_some(), "{error:#}");
        accept_newer_schema_at(&path, Some(TEST_KEY), SCHEMA_VERSION + 1).unwrap();
        open_archive_at(&path, Some(TEST_KEY)).unwrap();
    }

    #[test]
    fn accepting_a_current_store_changes_nothing() {
        let path = tmp_db_path();
        drop(open_at(&path).unwrap());
        accept_newer_schema_at(&path, None, SCHEMA_VERSION + 1).unwrap();
        let conn = open_at(&path).unwrap();
        assert_eq!(meta_get(&conn, ACCEPTED_NEWER_SCHEMA).unwrap(), None);
        accept_newer_schema_at(path.with_file_name("missing.db"), None, SCHEMA_VERSION + 1)
            .unwrap();
    }
}
