//! Mail folders: the folder list, special-use roles, and per-folder sync state.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

use crate::imap::Folder;

pub fn upsert_folders(conn: &Connection, account: &str, folders: &[Folder]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for folder in folders {
        tx.execute(
            "INSERT INTO folders(account, name, delimiter, special_use) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(account, name) DO UPDATE SET
               delimiter = excluded.delimiter,
               special_use = excluded.special_use",
            params![account, folder.name, folder.delimiter, folder.special_use],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Guarantee a folder row exists for a folder we have messages in, without
/// touching its delimiter. The full folder LIST sync (`upsert_folders`) only
/// runs when an account is opened directly, so in the unified view a freshly
/// added account never gets folder rows — and `get_folders` (which JOINs the
/// folders table) would then report zero unread even with unseen mail in the
/// store, leaving the tray dot and unread badges dark. Calling this on every
/// message sync keeps the count honest and self-heals existing accounts.
pub fn ensure_folder(conn: &Connection, account: &str, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO folders(account, name, delimiter) VALUES(?1, ?2, NULL)",
        params![account, name],
    )?;
    Ok(())
}

pub fn folder_exists(conn: &Connection, account: &str, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM folders WHERE account = ?1 AND name = ?2)",
        params![account, name],
        |row| row.get(0),
    )?)
}

/// Forget a folder that no longer exists on the server: its row, its cached
/// messages, its sync state and any cached search hits pointing into it. Pairs
/// with `imap::delete_folder`. Returns the number of cached messages dropped.
pub fn delete_folder(conn: &Connection, account: &str, name: &str) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let deleted = delete_folder_rows(&tx, account, name)?;
    tx.commit()?;
    Ok(deleted)
}

/// [`delete_folder`] inside a transaction the caller holds.
pub(super) fn delete_folder_rows(conn: &Connection, account: &str, name: &str) -> Result<usize> {
    let deleted = conn.execute(
        "DELETE FROM messages WHERE account = ?1 AND folder = ?2",
        params![account, name],
    )?;
    conn.execute(
        "DELETE FROM mail_search_hits WHERE account = ?1 AND folder = ?2",
        params![account, name],
    )?;
    // A snapshot's scope and pending hits name its folders. Snapshots are
    // disposable, so the account's are dropped rather than rewritten.
    conn.execute(
        "DELETE FROM mail_search_snapshots WHERE account = ?1",
        params![account],
    )?;
    conn.execute(
        "DELETE FROM mail_search_pending WHERE account = ?1",
        params![account],
    )?;
    conn.execute(
        "DELETE FROM folder_state WHERE account = ?1 AND folder = ?2",
        params![account, name],
    )?;
    conn.execute(
        "DELETE FROM uncached_unseen WHERE account = ?1 AND folder = ?2",
        params![account, name],
    )?;
    conn.execute(
        "DELETE FROM folders WHERE account = ?1 AND name = ?2",
        params![account, name],
    )?;
    Ok(deleted)
}

/// The renames a server RENAME of `from` to `to` makes: the folder and every
/// folder nested under it, each paired with its new name.
pub fn folder_rename_pairs(
    conn: &Connection,
    account: &str,
    from: &str,
    to: &str,
) -> Result<Vec<(String, String)>> {
    let mut pairs = vec![(from.to_string(), to.to_string())];
    for child in child_folders(conn, account, from)? {
        let renamed = format!("{to}{}", &child[from.len()..]);
        pairs.push((child, renamed));
    }
    Ok(pairs)
}

/// Follow a server-side rename in the folder list. Each folder keeps its row
/// (delimiter, notify opt-in) under the new name, but its cached mail is
/// dropped like a deleted folder's: cached bodies point at attachment files
/// keyed by the old name, and the next sync fetches the folder afresh. One
/// transaction, so the list never shows a folder under both names.
pub fn rename_folder(conn: &Connection, account: &str, pairs: &[(String, String)]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (from, to) in pairs {
        let row: Option<(Option<String>, Option<String>, i64)> = tx
            .query_row(
                "SELECT delimiter, special_use, notify FROM folders WHERE account = ?1 AND name = ?2",
                params![account, from],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        delete_folder_rows(&tx, account, from)?;
        let (delimiter, special_use, notify) = row.unwrap_or((None, None, 0));
        tx.execute(
            "INSERT INTO folders(account, name, delimiter, special_use, notify) VALUES(?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(account, name) DO UPDATE SET
               delimiter = excluded.delimiter,
               special_use = excluded.special_use,
               notify = excluded.notify",
            params![account, to, delimiter, special_use, notify],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Names of the folders nested under `name`, using the target folder's
/// server-reported delimiter. A NULL/empty delimiter means the server exposed
/// no hierarchy for this mailbox; punctuation in another mailbox's name must
/// not turn it into a destructive delete target.
pub fn child_folders(conn: &Connection, account: &str, name: &str) -> Result<Vec<String>> {
    let delimiter: Option<String> = conn
        .query_row(
            "SELECT delimiter FROM folders WHERE account = ?1 AND name = ?2",
            params![account, name],
            |row| row.get(0),
        )
        .optional()?
        .flatten()
        .filter(|delimiter: &String| !delimiter.is_empty());
    let Some(delimiter) = delimiter else {
        return Ok(Vec::new());
    };

    let mut stmt =
        conn.prepare("SELECT name FROM folders WHERE account = ?1 AND name <> ?2 ORDER BY name")?;
    let rows = stmt.query_map(params![account, name], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        let child = row?;
        if child.starts_with(&format!("{name}{delimiter}")) {
            out.push(child);
        }
    }
    Ok(out)
}

pub fn get_folders(conn: &Connection, account: &str) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT f.name, f.delimiter, f.special_use, {UNREAD_SQL} AS unread, f.notify <> 0
           FROM folders f WHERE f.account = ?1 ORDER BY f.name",
        UNREAD_SQL = unread_sql("f.account", "f.name"),
    ))?;
    let rows = stmt.query_map(params![account], |row| {
        let name = row.get::<_, String>(0)?;
        let special_use = row.get::<_, Option<String>>(2)?;
        Ok(Folder {
            role: String::new(),
            display_name: crate::utf7::decode(&name),
            name,
            delimiter: row.get(1)?,
            special_use,
            unread: row.get::<_, i64>(3)? as u32,
            notify: row.get(4)?,
        })
    })?;
    let mut folders = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let roles: Vec<&str> = folders
        .iter()
        .map(|folder| resolve_folder_role(&folder.name, folder.special_use.as_deref(), &folders))
        .collect();
    for (folder, role) in folders.iter_mut().zip(roles) {
        folder.role = role.to_string();
    }
    Ok(folders)
}

/// Opt a folder in to (or out of) live sync and new-mail notifications.
/// Returns false when the account has no such folder.
pub fn set_folder_notify(
    conn: &Connection,
    account: &str,
    folder: &str,
    enabled: bool,
) -> Result<bool> {
    // The flag doubles as the opt-in order (see `notify_folder_plan`), so an
    // already opted-in folder keeps its place when enabled again.
    let changed = if enabled {
        conn.execute(
            "UPDATE folders SET notify = CASE WHEN notify <> 0 THEN notify ELSE
               (SELECT COALESCE(MAX(notify), 0) + 1 FROM folders WHERE account = ?1) END
             WHERE account = ?1 AND name = ?2",
            params![account, folder],
        )?
    } else {
        conn.execute(
            "UPDATE folders SET notify = 0 WHERE account = ?1 AND name = ?2",
            params![account, folder],
        )?
    };
    Ok(changed > 0)
}

/// Whether new mail in `folder` raises a notification: always for INBOX,
/// otherwise only when the user opted the folder in.
pub fn folder_notifies(conn: &Connection, account: &str, folder: &str) -> Result<bool> {
    if folder.eq_ignore_ascii_case("INBOX") {
        return Ok(true);
    }
    Ok(conn
        .query_row(
            "SELECT notify <> 0 FROM folders WHERE account = ?1 AND name = ?2",
            params![account, folder],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false))
}

/// Whether `folder` holds one of the account's IDLE slots for opted-in folders
/// (see [`notify_folder_plan`]), and so is watched whether or not a view shows it.
pub fn folder_notifies_live(conn: &Connection, account: &str, folder: &str) -> Result<bool> {
    Ok(notify_folder_plan(conn, account)?
        .live
        .iter()
        .any(|name| name == folder))
}

/// The folders, besides INBOX, the user opted in to live sync and new-mail
/// notifications, in the order they were opted in.
pub fn notify_folders(conn: &Connection, account: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM folders
          WHERE account = ?1 AND notify <> 0 AND name <> 'INBOX' COLLATE NOCASE
          ORDER BY notify, name",
    )?;
    let rows = stmt.query_map(params![account], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// How many opted-in folders per account get a dedicated IMAP IDLE connection.
/// IDLE watches one mailbox per connection and providers cap connections per
/// account (Gmail at 15), so the rest are polled over the session pool.
pub const NOTIFY_LIVE_MAX: usize = 3;

/// How an account's opted-in folders are kept current.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct NotifyFolderPlan {
    /// Watched over IMAP IDLE: the first [`NOTIFY_LIVE_MAX`] opted in, so a
    /// later opt-in never takes a connection away from an earlier one.
    pub live: Vec<String>,
    /// Checked on a timer over pooled sessions.
    pub polled: Vec<String>,
}

pub fn notify_folder_plan(conn: &Connection, account: &str) -> Result<NotifyFolderPlan> {
    let mut live = notify_folders(conn, account)?;
    let polled = live.split_off(live.len().min(NOTIFY_LIVE_MAX));
    Ok(NotifyFolderPlan { live, polled })
}

/// Unread total for the folder named by the `account`/`folder` SQL expressions:
/// cached unseen messages plus the unseen server UIDs the cache doesn't hold.
fn unread_sql(account: &str, folder: &str) -> String {
    format!(
        "((SELECT COUNT(*) FROM messages m
            WHERE m.account = {account} AND m.folder = {folder} AND m.seen = 0)
          + (SELECT COUNT(*) FROM uncached_unseen u
            WHERE u.account = {account} AND u.folder = {folder}))"
    )
}

/// Authoritative unread-message total for one folder. Thread-list responses
/// carry this alongside their cards so clients do not have to join a separately
/// cached folder-list response to the freshly loaded page.
pub fn get_folder_unread(conn: &Connection, account: &str, folder: &str) -> Result<u32> {
    let unread = conn.query_row(
        &format!("SELECT {}", unread_sql("?1", "?2")),
        params![account, folder],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(unread as u32)
}

/// Replace the folder's `uncached_unseen` rows with the members of
/// `unseen` (the server's unseen UIDs) that aren't cached or guarded by a
/// removal marker. The snapshot guard retains concurrent removals until this
/// write completes, as it does for header and body persistence.
pub fn set_uncached_unseen(
    conn: &Connection,
    account: &str,
    folder: &str,
    unseen: &std::collections::HashSet<u32>,
) -> Result<()> {
    conn.execute(
        "DELETE FROM uncached_unseen WHERE account = ?1 AND folder = ?2",
        params![account, folder],
    )?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO uncached_unseen(account, folder, uid)
         SELECT ?1, ?2, ?3 WHERE NOT EXISTS (SELECT 1 FROM messages
           WHERE account = ?1 AND folder = ?2 AND uid = ?3)
         AND NOT EXISTS (SELECT 1 FROM removed_message_uids
           WHERE account = ?1 AND folder = ?2 AND uid = ?3)",
    )?;
    for uid in unseen {
        insert.execute(params![account, folder, uid])?;
    }
    Ok(())
}

pub fn classify_folder_role(name: &str, special_use: Option<&str>) -> &'static str {
    match special_use.unwrap_or_default() {
        "inbox" => "inbox",
        "sent" => "sent",
        "drafts" => "drafts",
        "trash" => "trash",
        "junk" => "junk",
        "archive" | "all" => "archive",
        // A local folder is plain whatever it is called.
        super::LOCAL_SPECIAL_USE => "folder",
        _ if name.eq_ignore_ascii_case("INBOX") => "inbox",
        _ if crate::imap::looks_like_sent(name) => "sent",
        _ if crate::imap::looks_like_drafts(name) => "drafts",
        _ if crate::imap::looks_like_trash(name) => "trash",
        _ if crate::imap::looks_like_junk(name) => "junk",
        _ if crate::imap::looks_like_archive(name) => "archive",
        _ => "folder",
    }
}

/// The one folder of an account that holds `role` ("drafts" or "trash") in the
/// cached list, as `imap::find_trash_folder` / `find_drafts_folder` pick it
/// live: the one the server flags with it, or else one whose name says so.
/// Delete expunges in place only there (`engine::delete_to_trash`) and moves
/// mail from anywhere else to it. The cache keeps no LIST order, so where
/// several folders tie this takes the first by name.
pub fn role_folder<'a>(folders: &'a [Folder], role: &str) -> Option<&'a str> {
    let first = |matches: &dyn Fn(&Folder) -> bool| {
        folders
            .iter()
            .filter(|folder| matches(folder))
            .map(|folder| folder.name.as_str())
            .min()
    };
    first(&|folder| folder.special_use.as_deref() == Some(role)).or_else(|| {
        first(&|folder| classify_folder_role(&folder.name, folder.special_use.as_deref()) == role)
    })
}

/// [`classify_folder_role`] within the account's `folders`: a folder that only
/// looks like Drafts or Trash by name, beside the one that holds the role (a
/// plain "Trash" beside Gmail's "[Gmail]/Trash"), is an ordinary folder, as it
/// is to delete. A folder the server flags keeps its role whatever else does,
/// and with no other folder cached in the role the name stands.
pub fn resolve_folder_role(
    name: &str,
    special_use: Option<&str>,
    folders: &[Folder],
) -> &'static str {
    let role = classify_folder_role(name, special_use);
    if matches!(role, "drafts" | "trash")
        && special_use != Some(role)
        && role_folder(folders, role).is_some_and(|holder| holder != name)
    {
        return "folder";
    }
    role
}

fn role_lookup_folders(conn: &Connection, account: &str) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare("SELECT name, special_use FROM folders WHERE account = ?1")?;
    let rows = stmt.query_map(params![account], |row| {
        Ok(Folder {
            name: row.get(0)?,
            special_use: row.get(1)?,
            ..Default::default()
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// The account's folder for a special-use role, or `None` when it has none.
///
/// Backs the unified view's folder switcher: "Sent" there means each account's
/// own Sent, and an account whose server has no Archive or Junk is simply left
/// out of that view rather than reported as a failure.
pub fn folder_for_role(conn: &Connection, account: &str, role: &str) -> Result<Option<String>> {
    // Every account has an Inbox, and RSS accounts have no folder rows at all —
    // resolving it from the table would drop them out of the unified inbox.
    if role.eq_ignore_ascii_case("inbox") {
        return Ok(Some("INBOX".to_string()));
    }
    Ok(get_folders(conn, account)?
        .into_iter()
        .find(|folder| folder.role == role)
        .map(|folder| folder.name))
}

pub fn get_folder_state(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Option<(u32, u32)>> {
    let row = conn
        .query_row(
            "SELECT uidvalidity, uid_next FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![account, folder],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .ok();
    Ok(row.map(|(v, n)| (v.unwrap_or(0) as u32, n.unwrap_or(0) as u32)))
}

pub fn set_folder_state(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uid_next: u32,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_state(account, folder, uidvalidity, uid_next) VALUES(?1, ?2, ?3, ?4)
         ON CONFLICT(account, folder) DO UPDATE SET
           uidvalidity = excluded.uidvalidity, uid_next = excluded.uid_next",
        params![account, folder, uidvalidity as i64, uid_next as i64],
    )?;
    Ok(())
}

pub fn get_folder_modseq(conn: &Connection, account: &str, folder: &str) -> Result<u64> {
    let modseq = conn
        .query_row(
            "SELECT highest_modseq FROM folder_state WHERE account = ?1 AND folder = ?2",
            params![account, folder],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0);
    Ok(modseq as u64)
}

pub fn set_folder_modseq(
    conn: &Connection,
    account: &str,
    folder: &str,
    modseq: u64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO folder_state(account, folder, highest_modseq) VALUES(?1, ?2, ?3)
         ON CONFLICT(account, folder) DO UPDATE SET highest_modseq = excluded.highest_modseq",
        params![account, folder, modseq as i64],
    )?;
    Ok(())
}

/// A folder's special-use role, resolved from the synced folders table (server
/// attribute when recorded, name heuristic otherwise).
pub fn folder_role(conn: &Connection, account: &str, folder: &str) -> Result<String> {
    let special_use: Option<String> = conn
        .query_row(
            "SELECT special_use FROM folders WHERE account = ?1 AND name = ?2",
            params![account, folder],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let role = classify_folder_role(folder, special_use.as_deref());
    if !matches!(role, "drafts" | "trash") {
        return Ok(role.to_string());
    }
    let folders = role_lookup_folders(conn, account)?;
    Ok(resolve_folder_role(folder, special_use.as_deref(), &folders).to_string())
}
