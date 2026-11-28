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

/// An Inbox search or a tag search spans the account, outside Trash and Junk.
/// Searches from other folders keep the folder plus Sent scope. Where the
/// account has an All Mail (`\All`) mailbox, as Gmail does, a text search from
/// the Inbox reads Inbox, Sent and All Mail: All Mail already holds Drafts
/// and every label once, and searching each label too returned a message once
/// per label it carried. Sent stays in, though All Mail covers it, because it is
/// synced locally while All Mail mostly is not, so cached and offline searches
/// still find sent mail; the Message-ID dedupe drops the All Mail copies.
pub fn query_folders(conn: &Connection, account: &str, folder: &str, query: &str) -> Vec<String> {
    let tagged = !crate::search_query::SearchQuery::parse(query)
        .tags
        .is_empty();
    let inbox = folder.eq_ignore_ascii_case("INBOX");
    if !inbox && !tagged {
        return search_folders(conn, account, folder);
    }
    let known = store::get_folders(conn, account).unwrap_or_default();
    if !tagged
        && let Some(all_mail) = known
            .iter()
            .find(|known| known.special_use.as_deref() == Some("all"))
    {
        let mut folders = search_folders(conn, account, folder);
        if !folders
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&all_mail.name))
        {
            folders.push(all_mail.name.clone());
        }
        return folders;
    }
    let mut folders = vec![folder.to_string()];
    folders.extend(
        known
            .into_iter()
            .filter(|known| !known.name.eq_ignore_ascii_case(folder))
            .filter(|known| {
                !matches!(known.role.as_str(), "trash" | "junk")
                    && !matches!(known.special_use.as_deref(), Some("flagged" | "noselect"))
            })
            .map(|known| known.name),
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
    // A real \Archive folder wins over \All, whatever order the folders come
    // in; `imap::find_archive_folder` follows the same rule.
    let by_attr = |attr: &str| {
        folders
            .iter()
            .find(|folder| {
                folder.special_use.as_deref() == Some(attr)
                    && !folder.name.eq_ignore_ascii_case(current)
            })
            .map(|folder| folder.name.clone())
    };
    by_attr("archive")
        .or_else(|| by_attr("all"))
        .or_else(|| find_role_folder(folders, "archive", imap::looks_like_archive, current))
}

/// The account's Archive folder, the same answer on every platform: from the
/// cached folder list, or from the server while nothing is cached yet.
pub async fn archive_folder(engine: &Arc<Engine>, account: &str) -> anyhow::Result<Option<String>> {
    let folders = store::get_folders(&engine.db.lock().unwrap(), account)?;
    if let Some(folder) = cached_archive_folder_from_folders(folders, "") {
        return Ok(Some(folder));
    }
    engine
        .with_read_session(account, |session| {
            Box::pin(async move { imap::find_archive_folder(session).await })
        })
        .await
}

/// Canonicalize folder names so "inbox"/"INBOX" map to one store key + mailbox.
pub fn canon_folder(folder: &str) -> String {
    crate::mail_model::canon_folder(folder)
}

#[cfg(test)]
mod query_folder_tests {
    use super::*;

    #[test]
    fn inbox_and_tag_searches_span_the_account_short_of_trash_and_junk() {
        let path = std::env::temp_dir().join(format!("meron-qf-{}.db", uuid::Uuid::new_v4()));
        let conn = store::open_at(&path).unwrap();
        for folder in ["INBOX", "Archive", "Projects", "Trash", "Junk"] {
            store::ensure_folder(&conn, "a", folder).unwrap();
            store::upsert_messages(
                &conn,
                "a",
                folder,
                &[imap::MessageHeader {
                    uid: 1,
                    subject: "Fictional project".to_string(),
                    ..Default::default()
                }],
            )
            .unwrap();
        }
        store::upsert_folders(
            &conn,
            "a",
            &[imap::Folder {
                name: "Parent".to_string(),
                special_use: Some("noselect".to_string()),
                ..Default::default()
            }],
        )
        .unwrap();
        let mut folders = query_folders(&conn, "a", "INBOX", "tag:todo");
        assert_eq!(folders.remove(0), "INBOX");
        assert_eq!(folders, ["Archive", "Projects"]);
        let folders = query_folders(&conn, "a", "INBOX", "fictional");
        assert_eq!(folders, ["INBOX", "Archive", "Projects"]);
        let hits =
            store::search_messages_in_folders(&conn, "a", &folders, "fictional", 10, None).unwrap();
        let mut hit_folders = hits.into_iter().map(|hit| hit.folder).collect::<Vec<_>>();
        hit_folders.sort();
        assert_eq!(hit_folders, ["Archive", "INBOX", "Projects"]);
        assert_eq!(query_folders(&conn, "a", "Projects", "plain"), ["Projects"]);
    }

    #[test]
    fn an_inbox_search_with_all_mail_reads_inbox_sent_and_all_mail_once() {
        let path = std::env::temp_dir().join(format!("meron-qf-{}.db", uuid::Uuid::new_v4()));
        let conn = store::open_at(&path).unwrap();
        let folder = |name: &str, special_use: Option<&str>| imap::Folder {
            name: name.to_string(),
            special_use: special_use.map(str::to_string),
            ..Default::default()
        };
        store::upsert_folders(
            &conn,
            "g",
            &[
                folder("INBOX", None),
                folder("[Gmail]/All Mail", Some("all")),
                folder("[Gmail]/Sent Mail", Some("sent")),
                folder("[Gmail]/Trash", Some("trash")),
                folder("Projects", None),
            ],
        )
        .unwrap();
        let hit = |uid: u32| imap::MessageHeader {
            uid,
            subject: "Fictional launch".to_string(),
            message_id: "<launch@example.com>".to_string(),
            date: 1_700_000_000,
            ..Default::default()
        };
        for (name, uid) in [("INBOX", 7), ("[Gmail]/All Mail", 40), ("Projects", 3)] {
            store::upsert_messages(&conn, "g", name, &[hit(uid)]).unwrap();
        }

        let folders = query_folders(&conn, "g", "INBOX", "launch");
        assert_eq!(folders, ["INBOX", "[Gmail]/Sent Mail", "[Gmail]/All Mail"]);
        let mut hits =
            store::search_messages_in_folders(&conn, "g", &folders, "launch", 10, None).unwrap();
        assert_eq!(hits.len(), 2);
        store::dedupe_search_hits(&mut hits, &folders);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].folder, "INBOX");

        // Sent is synced locally while All Mail mostly is not, so a sent
        // message cached only under Sent is still found offline.
        let sent = imap::MessageHeader {
            uid: 12,
            subject: "Fictional launch notes".to_string(),
            message_id: "<notes@example.com>".to_string(),
            date: 1_700_000_100,
            ..Default::default()
        };
        store::upsert_messages(&conn, "g", "[Gmail]/Sent Mail", &[sent]).unwrap();
        let mut hits =
            store::search_messages_in_folders(&conn, "g", &folders, "launch", 10, None).unwrap();
        store::dedupe_search_hits(&mut hits, &folders);
        let found = hits
            .iter()
            .map(|hit| hit.folder.as_str())
            .collect::<Vec<_>>();
        assert_eq!(found, ["[Gmail]/Sent Mail", "INBOX"]);

        // The first live search can find All Mail after the caller resolved
        // an Inbox-only scope; its copy still gives way to the Inbox one.
        let mut hits =
            store::search_messages_in_folders(&conn, "g", &folders, "launch", 10, None).unwrap();
        hits.retain(|hit| hit.message_id == "<launch@example.com>");
        store::dedupe_search_hits(&mut hits, &["INBOX".to_string()]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].folder, "INBOX");

        // A tag lives on the copy it was put on, so tag searches keep every label.
        let folders = query_folders(&conn, "g", "INBOX", "tag:todo");
        assert_eq!(
            folders,
            ["INBOX", "Projects", "[Gmail]/All Mail", "[Gmail]/Sent Mail"]
        );
    }
}
