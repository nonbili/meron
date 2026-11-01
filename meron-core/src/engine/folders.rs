//! Resolving role folders (Sent, Drafts, Archive) from cached folder lists.

use rusqlite::Connection;
use std::sync::Arc;

use crate::{imap, store};

use super::*;

/// Pick the folder filling a special-use role from a cached folder list. The
/// server-advertised RFC 6154 attribute (recorded by the folder LIST sync)
/// wins when present; folders synced before the attribute was recorded — or
/// servers without the extension — fall back to the name heuristic. `exclude`
/// (the folder currently being synced/viewed) never matches, so callers don't
/// handle the same folder twice.
pub(super) fn find_role_folder(
    folders: Vec<imap::Folder>,
    special_use: &str,
    looks_like: impl Fn(&str) -> bool,
    exclude: &str,
) -> Option<String> {
    folders
        .iter()
        .find(|folder| {
            folder.special_use.as_deref() == Some(special_use)
                && !folder.name.eq_ignore_ascii_case(exclude)
        })
        .map(|folder| folder.name.clone())
        .or_else(|| {
            folders
                .into_iter()
                .map(|folder| folder.name)
                .find(|name| looks_like(name) && !name.eq_ignore_ascii_case(exclude))
        })
}

/// The cached Sent mailbox name for `account`, if any — used so a chat-view
/// search reaches the user's own replies (and old mail filed under Sent), not
/// just the inbox. `inbox` is excluded so we never list it twice.
pub fn cached_sent_folder(engine: &Arc<Engine>, account: &str, inbox: &str) -> Option<String> {
    sent_folder_from_db(&engine.db.lock().unwrap(), account, inbox)
}

/// [`cached_sent_folder`] against a plain connection, for callers that hold the
/// store but not the engine (mobile's offline search).
pub fn sent_folder_from_db(conn: &Connection, account: &str, inbox: &str) -> Option<String> {
    find_role_folder(
        store::get_folders(conn, account).ok()?,
        "sent",
        imap::looks_like_sent,
        inbox,
    )
}

/// The folders a chat-view search covers: the open mailbox plus the account's
/// Sent, so a lookup surfaces both received and self-sent mail. Shared by the
/// live and cache-only search paths so they never disagree on scope.
pub fn search_folders(conn: &Connection, account: &str, folder: &str) -> Vec<String> {
    let mut folders = vec![folder.to_string()];
    if let Some(sent) = sent_folder_from_db(conn, account, folder) {
        folders.push(sent);
    }
    folders
}

/// The mailboxes `query` reads from `folder`. A tag belongs to the message
/// wherever it is filed, so a tag search spans the account, short of the mail
/// its owner threw away; the mailbox asked for stays first, and always included.
pub fn query_folders(conn: &Connection, account: &str, folder: &str, query: &str) -> Vec<String> {
    if crate::search_query::SearchQuery::parse(query)
        .tags
        .is_empty()
    {
        return search_folders(conn, account, folder);
    }
    let mut folders = vec![folder.to_string()];
    let names = store::get_folders(conn, account).unwrap_or_default();
    folders.extend(
        names
            .into_iter()
            .map(|known| known.name)
            .filter(|name| !name.eq_ignore_ascii_case(folder))
            .filter(|name| {
                let role = store::folder_role(conn, account, name).unwrap_or_default();
                role != "trash" && role != "junk"
            }),
    );
    folders
}

/// The cached Drafts mailbox name for `account`, if any — used so the regular
/// mailbox sync also pulls drafts and replies saved from another client thread
/// into the conversation view straight from the local store. `current` is
/// excluded so we never sync the same folder twice in one pass.
pub fn cached_drafts_folder(engine: &Arc<Engine>, account: &str, current: &str) -> Option<String> {
    let folders = store::get_folders(&engine.db.lock().unwrap(), account).ok()?;
    find_role_folder(folders, "drafts", imap::looks_like_drafts, current)
}

pub fn cached_archive_folder_from_folders(
    folders: Vec<imap::Folder>,
    current: &str,
) -> Option<String> {
    // Gmail advertises All Mail as \All rather than \Archive; either fills the
    // archive role (the live `imap::find_archive_folder` accepts both too, and
    // a localized name defeats the `looks_like_archive` fallback).
    folders
        .iter()
        .find(|folder| {
            matches!(folder.special_use.as_deref(), Some("archive" | "all"))
                && !folder.name.eq_ignore_ascii_case(current)
        })
        .map(|folder| folder.name.clone())
        .or_else(|| find_role_folder(folders, "archive", imap::looks_like_archive, current))
}

/// Canonicalize folder names so "inbox"/"INBOX" map to one store key + mailbox.
pub fn canon_folder(folder: &str) -> String {
    crate::mail_model::canon_folder(folder)
}

#[cfg(test)]
mod query_folder_tests {
    use super::*;

    #[test]
    fn a_tag_search_spans_the_account_short_of_trash_and_junk() {
        let path = std::env::temp_dir().join(format!("meron-qf-{}.db", uuid::Uuid::new_v4()));
        let conn = store::open_at(&path).unwrap();
        for folder in ["INBOX", "Projects", "Trash", "Junk"] {
            store::ensure_folder(&conn, "a", folder).unwrap();
        }
        let mut folders = query_folders(&conn, "a", "INBOX", "tag:todo");
        assert_eq!(folders.remove(0), "INBOX");
        assert_eq!(folders, ["Projects"]);
        assert_eq!(query_folders(&conn, "a", "INBOX", "plain"), ["INBOX"]);
    }
}
