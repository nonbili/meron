//! Copies of a database: a portable, passphrase-encrypted snapshot of the
//! archive, and a plain file copy of a store nothing has opened yet.

use super::open::{archive_path, db_path, sibling_with_suffix, sync_dir, sync_file};
use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

/// Write a consistent copy of the open archive to `dest`, encrypted with
/// `passphrase` rather than the keychain's key, so it can be restored on a
/// computer that never had that key (see `store::restore_local_backup`).
/// Copying `archive.db` itself while Meron runs is not a backup: recent
/// commits live in its WAL.
pub fn backup_archive_to(conn: &Connection, dest: &Path, passphrase: &str) -> Result<()> {
    if passphrase.is_empty() {
        anyhow::bail!("A passphrase is required");
    }
    // Renaming the snapshot over a database in use (or its WAL) would corrupt it.
    let live = [
        conn.path().map(PathBuf::from),
        Some(archive_path()),
        Some(db_path()),
    ];
    for database in live.into_iter().flatten() {
        for suffix in ["", "-wal", "-shm"] {
            if same_file_path(dest, &sibling_with_suffix(&database, suffix)) {
                anyhow::bail!("Choose another file: {} is in use by Meron", dest.display());
            }
        }
    }
    let file_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("archive.db");
    let partial = dest.with_file_name(format!("{file_name}.partial"));
    let _ = std::fs::remove_file(&partial);
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let exported = conn
        .execute(
            "ATTACH DATABASE ?1 AS snapshot KEY ?2",
            params![partial.to_string_lossy(), passphrase],
        )
        .map_err(anyhow::Error::from)
        .and_then(|_| {
            conn.execute_batch(&format!(
                "SELECT sqlcipher_export('snapshot');
                 PRAGMA snapshot.user_version = {user_version};"
            ))
            .map_err(anyhow::Error::from)
        });
    // A snapshot still attached may not have reached its file in full.
    let detached = conn
        .execute_batch("DETACH DATABASE snapshot;")
        .map_err(anyhow::Error::from);
    if let Err(err) = exported.and(detached) {
        let _ = std::fs::remove_file(&partial);
        return Err(err).context("export archive snapshot");
    }
    sync_file(&partial).context("flush archive snapshot")?;
    std::fs::rename(&partial, dest).context("move archive snapshot into place")?;
    sync_dir(dest);
    Ok(())
}

/// Whether two paths name the same file. An existing path is resolved in
/// full, links included; one that doesn't exist yet through its directory, so
/// a path spelled through a symlink or `..` still matches.
fn same_file_path(a: &Path, b: &Path) -> bool {
    let resolve = |path: &Path| {
        if let Ok(real) = path.canonicalize() {
            return Some(real);
        }
        let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty());
        let dir = dir
            .map(|dir| dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        path.file_name().map(|name| dir.join(name))
    };
    matches!((resolve(a), resolve(b)), (Some(a), Some(b)) if a == b)
}

/// Copy the database at `path`, with its WAL and shared-memory files, beside
/// it as `<name>.bak-<local time>`, returning the copy's path; `None` when
/// there is no database there. Only for a database no connection has open,
/// since a live one's files don't make a consistent copy. The copy keeps the
/// original's encryption, so it opens with the same key.
pub fn backup_database_files(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("meron.db");
    let mut dest = path.with_file_name(format!("{file_name}.bak-{stamp}"));
    let mut n = 1;
    while dest.exists() {
        n += 1;
        dest = path.with_file_name(format!("{file_name}.bak-{stamp}-{n}"));
    }
    // A copy missing its WAL would read as a backup and lack recent commits:
    // on any failure, nothing is left behind under the backup's name.
    let mut copied = Vec::new();
    let result = (|| -> Result<()> {
        for suffix in ["", "-wal", "-shm"] {
            let source = sibling_with_suffix(path, suffix);
            if !source.exists() {
                continue;
            }
            let target = sibling_with_suffix(&dest, suffix);
            copied.push(target.clone());
            std::fs::copy(&source, &target)
                .with_context(|| format!("copy {} to {}", source.display(), target.display()))?;
            sync_file(&target).with_context(|| format!("flush {}", target.display()))?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        for target in copied {
            let _ = std::fs::remove_file(target);
        }
        return Err(err);
    }
    sync_dir(&dest);
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meron-backup-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn database_files_copy_opens_with_the_same_key() {
        const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let dir = tmp_dir();
        let path = dir.join("meron.db");
        assert_eq!(backup_database_files(&path).unwrap(), None);
        {
            let conn = super::super::open_at_keyed(&path, KEY).unwrap();
            conn.execute("INSERT INTO settings(key, value) VALUES('k', 'v')", [])
                .unwrap();
        }
        let copy = backup_database_files(&path).unwrap().unwrap();
        let second = backup_database_files(&path).unwrap().unwrap();
        assert_ne!(copy, second, "a second copy must not overwrite the first");
        assert!(
            copy.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("meron.db.bak-")
        );
        let conn = super::super::open_at_keyed(&copy, KEY).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'k'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "v");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_live_file_is_the_same_file() {
        let dir = tmp_dir();
        let real = dir.join("archive.db");
        std::fs::write(&real, b"").unwrap();
        let link = dir.join("innocent.db");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(same_file_path(&link, &real));
        assert!(!same_file_path(&dir.join("new.db"), &real));
    }
}
