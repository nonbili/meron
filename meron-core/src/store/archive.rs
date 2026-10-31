//! Local folders: mail kept on this computer instead of on a server.
//!
//! There is one local account, [`LOCAL_ACCOUNT_ID`]. Its row lives in the store
//! like any account's, so it lists, orders and themes like one, but its folders
//! and mail live in a separate database, `archive.db` (see
//! `db::open_archive_at`). The store is a cache that sync may clear and refill;
//! the archive holds the only copy of what it keeps, so no cache-management
//! path can reach it.
//!
//! The archive shares the store's schema, so the message-list, search and
//! thread code reads it unchanged. `archive_raw` is the source of truth: the
//! original RFC822 message, zstd-compressed, with the few fields it does not
//! carry itself (flags, thread key, a date fallback). The `messages` row of an
//! archived message is derived from it by [`index_raw`] and can be rebuilt at
//! any time, which is how a move between local folders re-keys a message.

use anyhow::{Context, Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::imap::Folder;

/// The local account's id, in the store and in thread ids.
pub const LOCAL_ACCOUNT_ID: &str = "local";

/// Local folders nest with `/`, whatever the source servers used.
pub const LOCAL_DELIMITER: &str = "/";

/// The `special_use` every local folder carries, so a folder the user names
/// "Sent" or "Trash" is still an ordinary folder (see `classify_folder_role`).
pub const LOCAL_SPECIAL_USE: &str = "local";

const ZSTD_LEVEL: i32 = 9;

pub(super) const ARCHIVE_DDL: &str = "
CREATE TABLE IF NOT EXISTS archive_raw (
  id            INTEGER PRIMARY KEY,
  folder        TEXT NOT NULL,
  uid           INTEGER NOT NULL,
  sha256        TEXT NOT NULL,              -- of the uncompressed message
  size          INTEGER NOT NULL,           -- uncompressed bytes
  seen          INTEGER NOT NULL DEFAULT 0,
  starred       INTEGER NOT NULL DEFAULT 0,
  thread_key    TEXT NOT NULL DEFAULT '',   -- the source's key; '' = derive from headers
  date_fallback INTEGER NOT NULL DEFAULT 0, -- used when the Date header is unusable
  source        TEXT NOT NULL DEFAULT '',   -- where it came from, for the user
  archived_at   INTEGER NOT NULL DEFAULT 0,
  data          BLOB NOT NULL,              -- zstd
  UNIQUE (folder, uid)
);
CREATE INDEX IF NOT EXISTS archive_raw_sha_idx ON archive_raw(folder, sha256);
";

pub fn is_local_account(id: &str) -> bool {
    id == LOCAL_ACCOUNT_ID
}

/// Create the local account's row in the store. Returns false when it exists.
pub fn add_local_account(conn: &Connection, display_name: &str) -> Result<bool> {
    let now = super::now_unix();
    // Local folders have no inbox to merge, and nothing to send from.
    let prefs = json!({ "included_in_unified": false }).to_string();
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO accounts(id, engine, provider, display_name, config, prefs, created_at, updated_at, sort_order)
         VALUES(?1, 'local', 'local', ?2, '{}', ?3, ?4, ?4, COALESCE((SELECT MAX(sort_order) + 1 FROM accounts), 0))",
        params![LOCAL_ACCOUNT_ID, display_name, prefs, now],
    )?;
    Ok(inserted > 0)
}

pub fn has_local_account(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)",
        params![LOCAL_ACCOUNT_ID],
        |row| row.get(0),
    )?)
}

/// Copy what thread reads need from the store into the archive's own account
/// row: the local account's preferences (remote images, HTML mode) and every
/// address the user owns, as aliases, so mail the user sent before archiving
/// still reads as outgoing.
pub fn sync_local_identity(store: &Connection, archive: &Connection) -> Result<()> {
    let prefs: Option<String> = store
        .query_row(
            "SELECT prefs FROM accounts WHERE id = ?1",
            params![LOCAL_ACCOUNT_ID],
            |row| row.get(0),
        )
        .optional()?;
    let mut prefs = prefs
        .and_then(|prefs| serde_json::from_str::<Value>(&prefs).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let mut addrs = super::all_self_addrs(store).into_iter().collect::<Vec<_>>();
    addrs.sort();
    prefs["aliases"] = Value::Array(
        addrs
            .into_iter()
            .map(|email| json!({ "email": email, "name": "" }))
            .collect(),
    );
    let prefs = prefs.to_string();
    // Reads call this; the archive syncs every commit to disk, so only write a change.
    let current: Option<String> = archive
        .query_row(
            "SELECT prefs FROM accounts WHERE id = ?1",
            params![LOCAL_ACCOUNT_ID],
            |row| row.get(0),
        )
        .optional()?;
    if current.as_deref() == Some(prefs.as_str()) {
        return Ok(());
    }
    archive.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES(?1, 'local', 'local', ?2)
         ON CONFLICT(id) DO UPDATE SET prefs = excluded.prefs",
        params![LOCAL_ACCOUNT_ID, prefs],
    )?;
    Ok(())
}

/// Validate and normalize a local folder path: trimmed segments joined by
/// [`LOCAL_DELIMITER`], none of them empty, and "inbox" in any case spelled
/// `INBOX` — the list, thread-id and read paths canonicalize folder names that
/// way for every account, so a local folder must be stored under the same form.
pub fn normalize_local_folder_name(name: &str) -> Result<String> {
    Ok(canon_local_folder(&tidy_folder_path(name)?))
}

/// A local folder path with a top-level "inbox" in any case spelled `INBOX`,
/// so the folder and everything nested under it share one spelling.
pub fn canon_local_folder(path: &str) -> String {
    match path.split_once(LOCAL_DELIMITER) {
        Some((top, rest)) => format!(
            "{}{LOCAL_DELIMITER}{rest}",
            crate::mail_model::canon_folder(top)
        ),
        None => crate::mail_model::canon_folder(path),
    }
}

fn tidy_folder_path(name: &str) -> Result<String> {
    let segments = name
        .split(LOCAL_DELIMITER)
        .map(str::trim)
        .collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        bail!("Folder name is required");
    }
    Ok(segments.join(LOCAL_DELIMITER))
}

/// Create a local folder, and any missing parents so the tree stays whole.
/// Like a server folder, it is addressed by its modified UTF-7 name, so every
/// folder id the app passes around has the same form.
pub fn create_local_folder(conn: &Connection, display_name: &str) -> Result<Folder> {
    let display_name = tidy_folder_path(display_name)?;
    let name = canon_local_folder(&crate::utf7::encode(&display_name));
    if super::folder_exists(conn, LOCAL_ACCOUNT_ID, &name)? {
        bail!("A folder named {display_name} already exists");
    }
    let tx = conn.unchecked_transaction()?;
    let segments = name.split(LOCAL_DELIMITER).collect::<Vec<_>>();
    for depth in 1..=segments.len() {
        let path = segments[..depth].join(LOCAL_DELIMITER);
        tx.execute(
            "INSERT OR IGNORE INTO folders(account, name, delimiter, special_use) VALUES(?1, ?2, ?3, ?4)",
            params![LOCAL_ACCOUNT_ID, path, LOCAL_DELIMITER, LOCAL_SPECIAL_USE],
        )?;
        // A local folder is always "synced": the empty state must not offer
        // to wait for a server.
        tx.execute(
            "INSERT OR IGNORE INTO folder_state(account, folder, uidvalidity, uid_next) VALUES(?1, ?2, 1, 1)",
            params![LOCAL_ACCOUNT_ID, path],
        )?;
    }
    tx.commit()?;
    Ok(Folder {
        display_name,
        name,
        delimiter: Some(LOCAL_DELIMITER.to_string()),
        special_use: Some(LOCAL_SPECIAL_USE.to_string()),
        role: "folder".to_string(),
        ..Default::default()
    })
}

/// The folder and every folder nested under it, deepest first.
pub fn local_folder_subtree(conn: &Connection, name: &str) -> Result<Vec<String>> {
    if !super::folder_exists(conn, LOCAL_ACCOUNT_ID, name)? {
        bail!("Folder not found: {name}");
    }
    let mut targets = super::child_folders(conn, LOCAL_ACCOUNT_ID, name)?;
    targets.push(name.to_string());
    targets.sort_by_key(|folder| std::cmp::Reverse(folder.matches(LOCAL_DELIMITER).count()));
    Ok(targets)
}

/// Permanently delete a local folder, its subfolders and all the mail in them.
/// Returns the folders removed and how many messages went with them.
pub fn delete_local_folder(conn: &Connection, name: &str) -> Result<(Vec<String>, usize)> {
    let targets = local_folder_subtree(conn, name)?;
    let mut deleted = 0;
    for folder in &targets {
        conn.execute("DELETE FROM archive_raw WHERE folder = ?1", params![folder])?;
        deleted += super::delete_folder(conn, LOCAL_ACCOUNT_ID, folder)?;
    }
    reclaim_space(conn);
    Ok((targets, deleted))
}

/// Rename a local folder, moving its subfolders and their mail with it, and
/// create any missing parents of the new name. Returns each folder's old and
/// new name; none when the name is unchanged.
pub fn rename_local_folder(
    conn: &Connection,
    name: &str,
    display_name: &str,
    media_root: &Path,
) -> Result<Vec<(String, String)>> {
    let display_name = tidy_folder_path(display_name)?;
    let target = canon_local_folder(&crate::utf7::encode(&display_name));
    let sources = local_folder_subtree(conn, name)?;
    if target == name {
        return Ok(Vec::new());
    }
    if target.starts_with(&format!("{name}{LOCAL_DELIMITER}")) {
        bail!("A folder cannot be moved inside itself");
    }
    let pairs = sources
        .iter()
        .map(|from| (from.clone(), format!("{target}{}", &from[name.len()..])))
        .collect::<Vec<_>>();
    for (_, to) in &pairs {
        if super::folder_exists(conn, LOCAL_ACCOUNT_ID, to)? {
            bail!("A folder named {} already exists", crate::utf7::decode(to));
        }
    }

    // Every row moves in one transaction, so an interruption leaves the folder
    // whole under one name or the other. Attachment files and index rows are
    // written again afterwards; opening the archive redoes any that are missing
    // (`repair_local_index`).
    let mut moved = Vec::with_capacity(pairs.len());
    let tx = conn.unchecked_transaction()?;
    // The parents of the new name, as creating it would add them.
    let segments = target.split(LOCAL_DELIMITER).collect::<Vec<_>>();
    for depth in 1..segments.len() {
        let path = segments[..depth].join(LOCAL_DELIMITER);
        tx.execute(
            "INSERT OR IGNORE INTO folders(account, name, delimiter, special_use) VALUES(?1, ?2, ?3, ?4)",
            params![LOCAL_ACCOUNT_ID, path, LOCAL_DELIMITER, LOCAL_SPECIAL_USE],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO folder_state(account, folder, uidvalidity, uid_next) VALUES(?1, ?2, 1, 1)",
            params![LOCAL_ACCOUNT_ID, path],
        )?;
    }
    for (from, to) in &pairs {
        moved.push(local_folder_uids(&tx, from)?);
        tx.execute(
            "INSERT INTO folders(account, name, delimiter, special_use) VALUES(?1, ?2, ?3, ?4)",
            params![LOCAL_ACCOUNT_ID, to, LOCAL_DELIMITER, LOCAL_SPECIAL_USE],
        )?;
        // Keeping the UID counter, so new mail does not reuse a moved UID.
        tx.execute(
            "INSERT INTO folder_state(account, folder, uidvalidity, uid_next)
             SELECT account, ?3, uidvalidity, uid_next FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![LOCAL_ACCOUNT_ID, from, to],
        )?;
        tx.execute(
            "UPDATE archive_raw SET folder = ?2 WHERE folder = ?1",
            params![from, to],
        )?;
        super::delete_folder_rows(&tx, LOCAL_ACCOUNT_ID, from)?;
    }
    tx.commit()?;

    // Attachment files are keyed by folder, so each message's are written again
    // under the new name.
    for ((from, to), uids) in pairs.iter().zip(&moved) {
        for &uid in uids {
            crate::parse::remove_message_media(media_root, LOCAL_ACCOUNT_ID, from, uid);
            index_raw(conn, to, uid, media_root)?;
        }
    }
    Ok(pairs)
}

/// One message to keep in a local folder.
pub struct ArchiveInput<'a> {
    pub raw: &'a [u8],
    pub seen: bool,
    pub starred: bool,
    /// The thread key the message had where it came from, so a conversation
    /// keeps grouping the way it did. Empty derives one from its headers.
    pub thread_key: String,
    /// The source's date, for a message whose Date header does not parse.
    pub date_fallback: i64,
    /// A short description of the origin ("user@example.com/INBOX").
    pub source: String,
}

/// Store messages in a local folder. Each one is written, read back and
/// compared before the next, so a returned UID is a message that is safely on
/// disk; the caller may then remove the original. A message already in the
/// folder (same bytes) is not stored twice: its existing UID is returned.
pub fn archive_messages(
    conn: &Connection,
    folder: &str,
    messages: &[ArchiveInput<'_>],
    media_root: &Path,
) -> Result<Vec<u32>> {
    if !super::folder_exists(conn, LOCAL_ACCOUNT_ID, folder)? {
        bail!("Folder not found: {folder}");
    }
    let mut uids = Vec::with_capacity(messages.len());
    for message in messages {
        if message.raw.is_empty() {
            bail!("The server returned an empty message");
        }
        let sha256 = sha256_hex(message.raw);
        let existing: Option<u32> = conn
            .query_row(
                "SELECT uid FROM archive_raw WHERE folder = ?1 AND sha256 = ?2",
                params![folder, sha256],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(uid) = existing {
            uids.push(uid);
            continue;
        }
        let data = zstd::bulk::compress(message.raw, ZSTD_LEVEL).context("compress message")?;
        let uid = allocate_uids(conn, folder, 1)?;
        conn.execute(
            "INSERT INTO archive_raw(folder, uid, sha256, size, seen, starred, thread_key, date_fallback, source, archived_at, data)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                folder,
                uid,
                sha256,
                message.raw.len() as i64,
                message.seen as i64,
                message.starred as i64,
                message.thread_key,
                message.date_fallback,
                message.source,
                super::now_unix(),
                data,
            ],
        )?;
        // Read the stored copy back rather than trusting the write.
        let stored = raw_message(conn, folder, uid)?
            .ok_or_else(|| anyhow!("archived message {uid} could not be read back"))?;
        if stored.len() != message.raw.len() || sha256_hex(&stored) != sha256 {
            conn.execute(
                "DELETE FROM archive_raw WHERE folder = ?1 AND uid = ?2",
                params![folder, uid],
            )?;
            bail!("archived message {uid} did not read back intact");
        }
        index_raw(conn, folder, uid, media_root)?;
        uids.push(uid);
    }
    Ok(uids)
}

/// The original RFC822 bytes of an archived message.
pub fn raw_message(conn: &Connection, folder: &str, uid: u32) -> Result<Option<Vec<u8>>> {
    let data: Option<Vec<u8>> = conn
        .query_row(
            "SELECT data FROM archive_raw WHERE folder = ?1 AND uid = ?2",
            params![folder, uid],
            |row| row.get(0),
        )
        .optional()?;
    data.map(|data| zstd::stream::decode_all(data.as_slice()).context("decompress message"))
        .transpose()
}

/// Every UID kept in a local folder.
pub fn local_folder_uids(conn: &Connection, folder: &str) -> Result<Vec<u32>> {
    let mut stmt = conn.prepare("SELECT uid FROM archive_raw WHERE folder = ?1 ORDER BY uid")?;
    let rows = stmt.query_map(params![folder], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Archived messages as they are kept: raw bytes plus flags, in `uids` order.
/// UIDs that are not in the folder are left out.
pub fn raw_messages_for_copy(
    conn: &Connection,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<crate::imap::RawMessageCopy>> {
    let mut out = Vec::with_capacity(uids.len());
    for &uid in uids {
        let flags: Option<(bool, bool)> = conn
            .query_row(
                "SELECT seen, starred FROM archive_raw WHERE folder = ?1 AND uid = ?2",
                params![folder, uid],
                |row| Ok((row.get::<_, i64>(0)? != 0, row.get::<_, i64>(1)? != 0)),
            )
            .optional()?;
        let (Some((seen, starred)), Some(raw)) = (flags, raw_message(conn, folder, uid)?) else {
            continue;
        };
        out.push(crate::imap::RawMessageCopy {
            uid,
            raw,
            seen,
            starred,
        });
    }
    Ok(out)
}

/// (Re)build the `messages` row of one archived message from `archive_raw`,
/// writing its attachments into the media cache under keys of its current
/// folder and UID.
pub fn index_raw(conn: &Connection, folder: &str, uid: u32, media_root: &Path) -> Result<()> {
    let row: Option<(bool, bool, String, i64)> = conn
        .query_row(
            "SELECT seen, starred, thread_key, date_fallback FROM archive_raw WHERE folder = ?1 AND uid = ?2",
            params![folder, uid],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? != 0,
                    row.get::<_, i64>(1)? != 0,
                    row.get(2)?,
                    row.get(3)?,
                ))
            },
        )
        .optional()?;
    let Some((seen, starred, thread_key, date_fallback)) = row else {
        bail!("archived message {uid} not found in {folder}");
    };
    let raw = raw_message(conn, folder, uid)?.unwrap_or_default();
    let mut header = crate::imap::header_from_raw(&raw, uid, seen, starred);
    let ctx = crate::parse::MediaCtx {
        root: media_root.to_path_buf(),
        account: LOCAL_ACCOUNT_ID.to_string(),
        folder: folder.to_string(),
        uid,
    };
    let mut message = crate::parse::parse_message(&raw, Some(&ctx));
    // Reads treat a row without a Message-ID as one cached before ids were
    // stored and go to the server for it, which a local folder has not got.
    if header.message_id.is_empty() {
        header.message_id = format!("{}@local.meron", &sha256_hex(&raw)[..32]);
    }
    message.message_id = header.message_id.clone();
    // A `uid:` key names the message's UID where it came from; here it has
    // another, so the key derived for this UID stands instead.
    if !thread_key.is_empty() && !thread_key.starts_with("uid:") {
        header.thread_key = thread_key;
    }
    if header.date == 0 {
        header.date = date_fallback;
    }
    if message.date == 0 {
        message.date = header.date;
    }
    header.files = Some(
        message
            .file_attachments()
            .map(|attachment| super::CardFile {
                filename: attachment.filename.clone(),
                mime: attachment.mime.clone(),
            })
            .collect(),
    );
    super::upsert_messages(
        conn,
        LOCAL_ACCOUNT_ID,
        folder,
        std::slice::from_ref(&header),
    )?;
    super::save_cached_message(conn, LOCAL_ACCOUNT_ID, folder, uid, &message)?;
    Ok(())
}

/// Put back any attachment files of these archived messages that the media
/// cache has evicted. Rewrites the same keys, so stored HTML keeps pointing
/// at them.
pub fn restore_local_media(
    conn: &Connection,
    messages: &[(String, u32)],
    media_root: &Path,
) -> Result<()> {
    for (folder, uid) in messages {
        let Some(cached) = super::get_cached_message(conn, LOCAL_ACCOUNT_ID, folder, *uid)? else {
            continue;
        };
        if crate::parse::missing_media_count(media_root, &cached) == 0 {
            continue;
        }
        // Reindexing rewrites the files and the attachment list that names
        // them, which a write that failed the first time left without keys.
        index_raw(conn, folder, *uid, media_root)?;
    }
    Ok(())
}

/// Move archived messages to another local folder. They get new UIDs there,
/// returned in `uids` order; UIDs not found are skipped.
pub fn move_local_messages(
    conn: &Connection,
    folder: &str,
    target: &str,
    uids: &[u32],
    media_root: &Path,
) -> Result<Vec<u32>> {
    transfer_local_messages(conn, folder, target, uids, media_root, true)
}

/// Copy archived messages into another local folder.
pub fn copy_local_messages(
    conn: &Connection,
    folder: &str,
    target: &str,
    uids: &[u32],
    media_root: &Path,
) -> Result<Vec<u32>> {
    transfer_local_messages(conn, folder, target, uids, media_root, false)
}

fn transfer_local_messages(
    conn: &Connection,
    folder: &str,
    target: &str,
    uids: &[u32],
    media_root: &Path,
    remove_source: bool,
) -> Result<Vec<u32>> {
    if !super::folder_exists(conn, LOCAL_ACCOUNT_ID, target)? {
        bail!("Folder not found: {target}");
    }
    let mut out = Vec::with_capacity(uids.len());
    for &uid in uids {
        // Keep the flags the message has now, which the index row holds.
        let flags: Option<(i64, i64)> = conn
            .query_row(
                "SELECT seen, starred FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![LOCAL_ACCOUNT_ID, folder, uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let new_uid = allocate_uids(conn, target, 1)?;
        let tx = conn.unchecked_transaction()?;
        let changed = if remove_source {
            tx.execute(
                "UPDATE archive_raw SET folder = ?3, uid = ?4 WHERE folder = ?1 AND uid = ?2",
                params![folder, uid, target, new_uid],
            )?
        } else {
            tx.execute(
                "INSERT INTO archive_raw(folder, uid, sha256, size, seen, starred, thread_key, date_fallback, source, archived_at, data)
                 SELECT ?3, ?4, sha256, size, seen, starred, thread_key, date_fallback, source, archived_at, data
                   FROM archive_raw WHERE folder = ?1 AND uid = ?2",
                params![folder, uid, target, new_uid],
            )?
        };
        if changed == 0 {
            continue;
        }
        if let Some((seen, starred)) = flags {
            tx.execute(
                "UPDATE archive_raw SET seen = ?3, starred = ?4 WHERE folder = ?1 AND uid = ?2",
                params![target, new_uid, seen, starred],
            )?;
        }
        if remove_source {
            tx.execute(
                "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![LOCAL_ACCOUNT_ID, folder, uid],
            )?;
        }
        tx.commit()?;
        index_raw(conn, target, new_uid, media_root)?;
        out.push(new_uid);
    }
    Ok(out)
}

/// Permanently delete archived messages. Returns how many were removed.
pub fn delete_local_messages(conn: &Connection, folder: &str, uids: &[u32]) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut deleted = 0;
    for &uid in uids {
        deleted += tx.execute(
            "DELETE FROM archive_raw WHERE folder = ?1 AND uid = ?2",
            params![folder, uid],
        )?;
        tx.execute(
            "DELETE FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![LOCAL_ACCOUNT_ID, folder, uid],
        )?;
    }
    tx.commit()?;
    reclaim_space(conn);
    Ok(deleted)
}

/// Set the read and/or starred flag on archived messages, in both the index
/// row and the kept original.
pub fn set_local_flags(
    conn: &Connection,
    folder: &str,
    uids: &[u32],
    seen: Option<bool>,
    starred: Option<bool>,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for &uid in uids {
        if let Some(seen) = seen {
            for table in ["archive_raw", "messages"] {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET seen = ?3 WHERE folder = ?1 AND uid = ?2{}",
                        if table == "messages" {
                            " AND account = ?4"
                        } else {
                            ""
                        }
                    ),
                    rusqlite::params_from_iter(local_flag_params(folder, uid, seen, table)),
                )?;
            }
        }
        if let Some(starred) = starred {
            for table in ["archive_raw", "messages"] {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET starred = ?3 WHERE folder = ?1 AND uid = ?2{}",
                        if table == "messages" {
                            " AND account = ?4"
                        } else {
                            ""
                        }
                    ),
                    rusqlite::params_from_iter(local_flag_params(folder, uid, starred, table)),
                )?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

fn local_flag_params(
    folder: &str,
    uid: u32,
    value: bool,
    table: &str,
) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value as Sql;
    let mut values = vec![
        Sql::Text(folder.to_string()),
        Sql::Integer(uid as i64),
        Sql::Integer(value as i64),
    ];
    if table == "messages" {
        values.push(Sql::Text(LOCAL_ACCOUNT_ID.to_string()));
    }
    values
}

/// Mark every message in a local folder read or unread.
pub fn set_local_folder_seen(conn: &Connection, folder: &str, seen: bool) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let updated = tx.execute(
        "UPDATE archive_raw SET seen = ?2 WHERE folder = ?1 AND seen <> ?2",
        params![folder, seen as i64],
    )?;
    super::mark_folder_seen(&tx, LOCAL_ACCOUNT_ID, folder, seen)?;
    tx.commit()?;
    Ok(updated)
}

/// What a store row knows about a message that its raw bytes may not: the
/// thread key it grouped under (Gmail's thread id, an inherited root) and its
/// date with the INTERNALDATE fallback applied.
pub fn cached_thread_key_and_date(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
) -> Result<(String, i64)> {
    Ok(conn
        .query_row(
            "SELECT COALESCE(thread_key, ''), date FROM messages
              WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .unwrap_or_default())
}

/// "user@example.com/INBOX": where an archived message came from.
pub fn archive_source_label(conn: &Connection, account: &str, folder: &str) -> String {
    let email: Option<String> = conn
        .query_row(
            "SELECT COALESCE(NULLIF(email, ''), display_name) FROM accounts WHERE id = ?1",
            params![account],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();
    format!(
        "{}/{}",
        email.unwrap_or_else(|| account.to_string()),
        crate::utf7::decode(folder)
    )
}

/// Marks an error as a wrong passphrase (or a file that is no local folders
/// backup), so the UI can ask again instead of reporting a failure.
pub const WRONG_BACKUP_PASSPHRASE: &str = "wrong passphrase for this local folders backup";

/// Add the mail of a passphrase-protected local folders backup (see
/// `store::backup_archive_to`) to the archive: its folders, and every message
/// not already in the same folder. Nothing here is overwritten or removed.
/// Returns how many messages were added.
pub fn restore_local_backup(
    conn: &Connection,
    src: &Path,
    passphrase: &str,
    media_root: &Path,
) -> Result<usize> {
    conn.execute(
        "ATTACH DATABASE ?1 AS restore KEY ?2",
        params![src.to_string_lossy(), passphrase],
    )
    .map_err(|err| match err.sqlite_error_code() {
        // A wrong key reads as a file that is not a database.
        Some(rusqlite::ErrorCode::NotADatabase) => anyhow!(WRONG_BACKUP_PASSPHRASE),
        _ => anyhow::Error::from(err),
    })?;
    let restored = restore_attached(conn, media_root);
    let _ = conn.execute_batch("DETACH DATABASE restore;");
    restored
}

fn restore_attached(conn: &Connection, media_root: &Path) -> Result<usize> {
    let readable = conn
        .query_row(
            "SELECT count(*) FROM restore.sqlite_master WHERE name = 'archive_raw'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);
    if readable == 0 {
        bail!(WRONG_BACKUP_PASSPHRASE);
    }
    let folders = {
        let mut stmt = conn.prepare(
            "SELECT name FROM restore.folders WHERE account = ?1
             UNION SELECT DISTINCT folder FROM restore.archive_raw",
        )?;
        stmt.query_map(params![LOCAL_ACCOUNT_ID], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for folder in &folders {
        conn.execute(
            "INSERT OR IGNORE INTO main.folders(account, name, delimiter, special_use) VALUES(?1, ?2, ?3, ?4)",
            params![LOCAL_ACCOUNT_ID, folder, LOCAL_DELIMITER, LOCAL_SPECIAL_USE],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO main.folder_state(account, folder, uidvalidity, uid_next) VALUES(?1, ?2, 1, 1)",
            params![LOCAL_ACCOUNT_ID, folder],
        )?;
    }
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.folder FROM restore.archive_raw r
              WHERE NOT EXISTS (
                SELECT 1 FROM main.archive_raw m WHERE m.folder = r.folder AND m.sha256 = r.sha256
              )
              ORDER BY r.id",
        )?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    // A backup from before tags has no such table. Rows keyed by location stay
    // behind: the restore hands out new UIDs.
    let has_tags: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM restore.sqlite_master WHERE name = 'message_tags')",
        [],
        |row| row.get(0),
    )?;
    if has_tags {
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO main.message_tags(account, tag_key, tag, state, updated_at)
                 SELECT account, tag_key, tag, 0, 0 FROM restore.message_tags
                  WHERE state IN {} AND tag_key NOT LIKE 'uid:%'",
                super::tags::LIVE_STATES,
            ),
            [],
        )?;
    }
    for (id, folder) in &rows {
        let uid = allocate_uids(conn, folder, 1)?;
        conn.execute(
            "INSERT INTO main.archive_raw(folder, uid, sha256, size, seen, starred, thread_key, date_fallback, source, archived_at, data)
             SELECT folder, ?2, sha256, size, seen, starred, thread_key, date_fallback, source, archived_at, data
               FROM restore.archive_raw WHERE id = ?1",
            params![id, uid],
        )?;
        index_raw(conn, folder, uid, media_root)?;
    }
    Ok(rows.len())
}

/// Rebuild index rows for archived messages that have none: a crash between
/// storing a message and indexing it, or a migration that rebuilt `messages`.
/// Returns how many were indexed.
pub fn repair_local_index(conn: &Connection, media_root: &Path) -> Result<usize> {
    let missing = {
        let mut stmt = conn.prepare(
            "SELECT r.folder, r.uid FROM archive_raw r
              WHERE NOT EXISTS (
                SELECT 1 FROM messages m
                 WHERE m.account = ?1 AND m.folder = r.folder AND m.uid = r.uid
                   AND m.body IS NOT NULL
              )",
        )?;
        stmt.query_map(params![LOCAL_ACCOUNT_ID], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (folder, uid) in &missing {
        super::ensure_folder(conn, LOCAL_ACCOUNT_ID, folder)?;
        index_raw(conn, folder, *uid, media_root)?;
    }
    Ok(missing.len())
}

/// How much mail the archive keeps: messages, their original size, and the
/// space the database file takes on disk.
pub fn local_storage_stats(conn: &Connection) -> Result<Value> {
    let (messages, size): (i64, i64) = conn.query_row(
        "SELECT count(*), COALESCE(sum(size), 0) FROM archive_raw",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let page_count = pragma_int(conn, "page_count")?;
    let page_size = pragma_int(conn, "page_size")?;
    let free_pages = pragma_int(conn, "freelist_count")?;
    Ok(json!({
        "messages": messages,
        "message_bytes": size,
        "file_bytes": (page_count - free_pages).max(0) * page_size,
    }))
}

/// An integer pragma. SQLCipher answers `page_size` on a keyed database as
/// its own `cipher_page_size`, which is text.
fn pragma_int(conn: &Connection, name: &str) -> Result<i64> {
    let value: rusqlite::types::Value =
        conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))?;
    match value {
        rusqlite::types::Value::Integer(n) => Ok(n),
        rusqlite::types::Value::Text(text) => text
            .trim()
            .parse()
            .with_context(|| format!("PRAGMA {name} returned {text:?}")),
        other => bail!("PRAGMA {name} returned {other:?}"),
    }
}

/// Allocate `count` consecutive UIDs in a local folder from its `uid_next`.
fn allocate_uids(conn: &Connection, folder: &str, count: u32) -> Result<u32> {
    let next: u32 = conn
        .query_row(
            "SELECT uid_next FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![LOCAL_ACCOUNT_ID, folder],
            |row| row.get::<_, Option<u32>>(0),
        )
        .optional()?
        .flatten()
        .unwrap_or(1)
        .max(1);
    // Never reuse a UID still held by a row, whatever uid_next says.
    let held: u32 = conn.query_row(
        "SELECT COALESCE(MAX(uid), 0) + 1 FROM archive_raw WHERE folder = ?1",
        params![folder],
        |row| row.get(0),
    )?;
    let first = next.max(held);
    super::set_folder_state(conn, LOCAL_ACCOUNT_ID, folder, 1, first + count)?;
    Ok(first)
}

/// Give freed pages back to the file system after a delete.
fn reclaim_space(conn: &Connection) {
    let _ = conn.execute_batch("PRAGMA incremental_vacuum;");
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive() -> (Connection, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("meron-archive-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = super::super::open_archive_at(dir.join("archive.db"), None).unwrap();
        (conn, dir)
    }

    const RAW: &[u8] = b"Message-ID: <a@example.com>\r\nFrom: Ann <ann@example.com>\r\nTo: bob@example.com\r\nSubject: Hello\r\nDate: Tue, 1 Oct 2024 10:00:00 +0000\r\n\r\nBody text\r\n";

    fn input(raw: &[u8]) -> ArchiveInput<'_> {
        ArchiveInput {
            raw,
            seen: true,
            starred: false,
            thread_key: String::new(),
            date_fallback: 0,
            source: "test".into(),
        }
    }

    #[test]
    fn archived_message_reads_back_and_lists() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "Old/2024").unwrap();
        let uids = archive_messages(&conn, "Old/2024", &[input(RAW)], &dir).unwrap();
        assert_eq!(uids, vec![1]);
        assert_eq!(raw_message(&conn, "Old/2024", 1).unwrap().unwrap(), RAW);
        // The parent was created alongside.
        let folders = crate::store::get_folders(&conn, LOCAL_ACCOUNT_ID).unwrap();
        assert_eq!(
            folders.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["Old", "Old/2024"]
        );
        assert!(folders.iter().all(|f| f.role == "folder"));
        let cached = crate::store::get_cached_message(&conn, LOCAL_ACCOUNT_ID, "Old/2024", 1)
            .unwrap()
            .unwrap();
        assert_eq!(cached.subject, "Hello");
        assert_eq!(cached.message_id, "a@example.com");
        // The same bytes again are not stored twice.
        let again = archive_messages(&conn, "Old/2024", &[input(RAW)], &dir).unwrap();
        assert_eq!(again, vec![1]);
        let stats = local_storage_stats(&conn).unwrap();
        assert_eq!(stats["messages"], 1);
    }

    #[test]
    fn move_rekeys_and_keeps_flags() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "A").unwrap();
        create_local_folder(&conn, "B").unwrap();
        archive_messages(&conn, "A", &[input(RAW)], &dir).unwrap();
        set_local_flags(&conn, "A", &[1], Some(false), Some(true)).unwrap();
        let moved = move_local_messages(&conn, "A", "B", &[1], &dir).unwrap();
        assert_eq!(moved, vec![1]);
        assert!(raw_message(&conn, "A", 1).unwrap().is_none());
        let copies = raw_messages_for_copy(&conn, "B", &moved).unwrap();
        assert_eq!(copies.len(), 1);
        assert!(!copies[0].seen && copies[0].starred);
        let (seen, starred): (i64, i64) = conn
            .query_row(
                "SELECT seen, starred FROM messages WHERE account = 'local' AND folder = 'B' AND uid = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((seen, starred), (0, 1));
        let gone: i64 = conn
            .query_row(
                "SELECT count(*) FROM messages WHERE account = 'local' AND folder = 'A'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(gone, 0);
    }

    #[test]
    fn restoring_media_repairs_an_attachment_left_without_a_file() {
        let (conn, dir) = archive();
        let created = create_local_folder(&conn, "Archive").unwrap();
        assert_eq!(created.role, "folder");
        let raw = b"Message-ID: <b@example.com>\r\nFrom: ann@example.com\r\nSubject: File\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nHi\r\n--b\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"a.txt\"\r\n\r\nfile\r\n--b--\r\n";
        archive_messages(&conn, "Archive", &[input(raw)], &dir).unwrap();
        // As a failed cache write leaves it: an attachment with no file key.
        conn.execute(
            "UPDATE messages SET json = json_set(json, '$.attachments[0].key', NULL) WHERE account = 'local'",
            [],
        )
        .unwrap();
        let missing = |conn: &Connection| {
            let cached = crate::store::get_cached_message(conn, LOCAL_ACCOUNT_ID, "Archive", 1)
                .unwrap()
                .unwrap();
            crate::parse::missing_media_count(&dir, &cached)
        };
        assert_eq!(missing(&conn), 1);
        restore_local_media(&conn, &[("Archive".to_string(), 1)], &dir).unwrap();
        assert_eq!(missing(&conn), 0);
    }

    #[test]
    fn deleted_folder_takes_its_subtree_and_mail() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "A/B").unwrap();
        archive_messages(&conn, "A/B", &[input(RAW)], &dir).unwrap();
        let (removed, deleted) = delete_local_folder(&conn, "A").unwrap();
        assert_eq!(removed, vec!["A/B".to_string(), "A".to_string()]);
        assert_eq!(deleted, 1);
        assert_eq!(local_storage_stats(&conn).unwrap()["messages"], 0);
    }

    #[test]
    fn renamed_folder_takes_its_subtree_and_mail() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "A/B").unwrap();
        create_local_folder(&conn, "C").unwrap();
        archive_messages(&conn, "A/B", &[input(RAW)], &dir).unwrap();
        set_local_flags(&conn, "A/B", &[1], None, Some(true)).unwrap();
        let renamed = rename_local_folder(&conn, "A", "X/Y", &dir).unwrap();
        assert_eq!(
            renamed,
            vec![
                ("A/B".to_string(), "X/Y/B".to_string()),
                ("A".to_string(), "X/Y".to_string()),
            ]
        );
        let names = crate::store::get_folders(&conn, LOCAL_ACCOUNT_ID)
            .unwrap()
            .into_iter()
            .map(|folder| folder.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["C", "X", "X/Y", "X/Y/B"]);
        assert_eq!(raw_message(&conn, "X/Y/B", 1).unwrap().unwrap(), RAW);
        let cached = crate::store::get_cached_message(&conn, LOCAL_ACCOUNT_ID, "X/Y/B", 1)
            .unwrap()
            .unwrap();
        assert_eq!(cached.subject, "Hello");
        let starred: i64 = conn
            .query_row(
                "SELECT starred FROM messages WHERE account = 'local' AND folder = 'X/Y/B' AND uid = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(starred, 1);
        // The UID counter came along: the next message does not reuse UID 1.
        let next = archive_messages(
            &conn,
            "X/Y/B",
            &[input(b"Message-ID: <c@example.com>\r\n\r\nHi\r\n")],
            &dir,
        )
        .unwrap();
        assert_eq!(next, vec![2]);

        assert!(
            rename_local_folder(&conn, "C", "X/Y", &dir)
                .unwrap_err()
                .to_string()
                .contains("already exists")
        );
        assert!(
            rename_local_folder(&conn, "X", "X/Y/Z", &dir)
                .unwrap_err()
                .to_string()
                .contains("inside itself")
        );
        assert!(
            rename_local_folder(&conn, "C", "C", &dir)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn index_is_rebuilt_from_the_kept_original() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "A").unwrap();
        archive_messages(&conn, "A", &[input(RAW)], &dir).unwrap();
        conn.execute("DELETE FROM messages", []).unwrap();
        assert_eq!(repair_local_index(&conn, &dir).unwrap(), 1);
        assert_eq!(repair_local_index(&conn, &dir).unwrap(), 0);
    }

    #[test]
    fn folder_names_are_normalized() {
        assert_eq!(normalize_local_folder_name(" A / B ").unwrap(), "A/B");
        assert!(normalize_local_folder_name("A//B").is_err());
        assert!(normalize_local_folder_name("  ").is_err());
        assert_eq!(normalize_local_folder_name("inbox").unwrap(), "INBOX");
        assert_eq!(
            normalize_local_folder_name("inbox/2024").unwrap(),
            "INBOX/2024"
        );
    }

    #[test]
    fn a_parent_named_inbox_is_addressable_with_its_children() {
        let (conn, _dir) = archive();
        create_local_folder(&conn, "inbox/2024").unwrap();
        let (removed, _) = delete_local_folder(&conn, "INBOX").unwrap();
        assert_eq!(removed, vec!["INBOX/2024".to_string(), "INBOX".to_string()]);
    }

    #[test]
    fn a_snapshot_never_replaces_the_open_archive() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "A").unwrap();
        for name in [
            "archive.db",
            "archive.db-wal",
            "archive.db-shm",
            "./archive.db",
        ] {
            assert!(crate::store::backup_archive_to(&conn, &dir.join(name), "pass").is_err());
        }
        assert!(super::super::folder_exists(&conn, LOCAL_ACCOUNT_ID, "A").unwrap());
    }

    #[test]
    fn a_backup_restores_into_another_archive_with_its_passphrase() {
        const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let dir = std::env::temp_dir().join(format!("meron-archive-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::store::open_archive_at(dir.join("archive.db"), Some(KEY)).unwrap();
        create_local_folder(&conn, "Old/2024").unwrap();
        archive_messages(&conn, "Old/2024", &[input(RAW)], &dir).unwrap();
        let stats = local_storage_stats(&conn).unwrap();
        assert_eq!(stats["messages"], 1);
        assert!(stats["file_bytes"].as_i64().unwrap() > 0);
        let tag_key = ["kept@example.com".to_string()];
        crate::store::change_tags(
            &conn,
            LOCAL_ACCOUNT_ID,
            &tag_key,
            &["todo".into()],
            &[],
            false,
        )
        .unwrap();
        let dest = dir.join("copy.db");
        crate::store::backup_archive_to(&conn, &dest, "correct horse").unwrap();

        // Another computer: another key, an archive of its own.
        let other = crate::store::open_archive_at(dir.join("other.db"), None).unwrap();
        let wrong = restore_local_backup(&other, &dest, "wrong", &dir).unwrap_err();
        assert!(format!("{wrong:#}").contains(WRONG_BACKUP_PASSPHRASE));
        assert_eq!(
            restore_local_backup(&other, &dest, "correct horse", &dir).unwrap(),
            1
        );
        assert_eq!(raw_message(&other, "Old/2024", 1).unwrap().unwrap(), RAW);
        assert_eq!(
            crate::store::tags_by_key(&other, LOCAL_ACCOUNT_ID, &tag_key).unwrap()[&tag_key[0]],
            ["todo"]
        );
        assert!(super::super::folder_exists(&other, LOCAL_ACCOUNT_ID, "Old").unwrap());
        // Restoring again adds nothing twice.
        assert_eq!(
            restore_local_backup(&other, &dest, "correct horse", &dir).unwrap(),
            0
        );
    }

    #[test]
    fn a_source_uid_thread_key_is_rekeyed_to_the_local_uid() {
        let (conn, dir) = archive();
        create_local_folder(&conn, "inbox").unwrap();
        let headerless = b"From: ann@example.com\r\nSubject: Note\r\n\r\nBody\r\n";
        let mut message = input(headerless);
        message.thread_key = "uid:77".into();
        let uids = archive_messages(&conn, "INBOX", &[message], &dir).unwrap();
        let key: String = conn
            .query_row(
                "SELECT thread_key FROM messages WHERE account = 'local' AND folder = 'INBOX' AND uid = ?1",
                [uids[0]],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(key, "uid:77");
    }

    #[test]
    fn folders_are_addressed_by_their_wire_name() {
        let (conn, _dir) = archive();
        let folder = create_local_folder(&conn, "Steuer & Bü/2024").unwrap();
        assert_eq!(folder.display_name, "Steuer & Bü/2024");
        assert_eq!(folder.name, crate::utf7::encode("Steuer & Bü/2024"));
        let folders = crate::store::get_folders(&conn, LOCAL_ACCOUNT_ID).unwrap();
        assert_eq!(folders[1].display_name, "Steuer & Bü/2024");
        assert_eq!(folders[0].display_name, "Steuer & Bü");
    }
}
