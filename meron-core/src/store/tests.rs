use super::*;
use rusqlite::OptionalExtension;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::imap::Folder;
use crate::parse::Message;

mod account_tests;
mod attachment_tests;
mod contact_tests;
mod encrypted_tests;
mod folder_tests;
mod message_tests;
mod recent_tests;
mod schema_tests;
mod search_tests;
mod thread_tests;

const UNREAD: RecentFilter = RecentFilter {
    unread_only: true,
    attachments_only: false,
};
const ATTACHMENTS: RecentFilter = RecentFilter {
    unread_only: false,
    attachments_only: true,
};

fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn
}

fn insert_message(
    conn: &Connection,
    uid: u32,
    subject: &str,
    from_name: &str,
    from_addr: &str,
    body: Option<&str>,
) {
    conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, body)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, ?4, ?5, 1779580800, 0, ?6)",
            params![uid.to_string(), uid, subject, from_name, from_addr, body],
        )
        .unwrap();
}
