//! Tests of what the schema's triggers maintain, through the store's own
//! readers.

use super::*;

fn contact_message(conn: &Connection, id: i64, folder: &str, date: i64) {
    conn.execute(
        "INSERT INTO messages(id, account, folder, msg_id, uid, from_name, from_addr, date, json)
         VALUES(?1, 'a', ?2, ?1, ?1, 'Person', 'person@example.com', ?3,
           '{\"to\":[{\"name\":\"Person\",\"addr\":\"person@example.com\"}],\"cc\":[]}')",
        params![id, folder, date],
    )
    .unwrap();
}

#[test]
fn contact_participants_track_moves_deletes_and_lower_dates() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    contact_message(&conn, 1, "INBOX", 10);
    contact_message(&conn, 2, "INBOX", 20);
    let tally = |folder: &str| {
        conn.query_row(
            "SELECT count, last_date FROM contacts WHERE account = 'a' AND folder = ?1",
            [folder],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .unwrap()
    };
    // A sender repeated in To counts only once on each message.
    assert_eq!(tally("INBOX"), Some((2, 20)));
    conn.execute("UPDATE messages SET folder = 'Trash' WHERE id = 2", [])
        .unwrap();
    assert_eq!(tally("INBOX"), Some((1, 10)));
    assert_eq!(tally("Trash"), Some((1, 20)));
    conn.execute(
        "UPDATE messages SET folder = 'INBOX', date = 5 WHERE id = 2",
        [],
    )
    .unwrap();
    assert_eq!(tally("Trash"), None);
    assert_eq!(tally("INBOX"), Some((2, 10)));
    conn.execute("DELETE FROM messages WHERE id = 1", [])
        .unwrap();
    assert_eq!(tally("INBOX"), Some((1, 5)));
    conn.execute("UPDATE messages SET uid = 0 WHERE id = 2", [])
        .unwrap();
    assert_eq!(tally("INBOX"), None);
}

#[test]
fn contact_exclusion_uses_current_metadata_and_name_fallback() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    contact_message(&conn, 1, "Odd mailbox", 10);
    let visible = || {
        super::super::list_contacts(
            &conn,
            "a",
            "",
            20,
            super::super::ContactSort::Frequent,
            false,
        )
        .unwrap()
        .len()
    };
    assert_eq!(visible(), 1);
    conn.execute(
        "INSERT INTO folders(account, name, special_use) VALUES('a', 'Odd mailbox', 'junk')",
        [],
    )
    .unwrap();
    assert_eq!(visible(), 0);
    conn.execute("UPDATE folders SET special_use = 'trash'", [])
        .unwrap();
    assert_eq!(visible(), 0);
    conn.execute("UPDATE folders SET special_use = NULL", [])
        .unwrap();
    assert_eq!(visible(), 1);
    conn.execute("UPDATE messages SET folder = 'Trash'", [])
        .unwrap();
    assert_eq!(visible(), 0);
    conn.execute(
        "INSERT INTO folders(account, name, special_use) VALUES('a', 'Trash', ?1)",
        [super::super::LOCAL_SPECIAL_USE],
    )
    .unwrap();
    assert_eq!(visible(), 1);
}

#[test]
fn other_contacts_leave_out_every_address_an_enabled_book_holds() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    contact_message(&conn, 1, "INBOX", 10);
    let addrs = |unsaved: bool| {
        super::super::list_contacts_in(
            &conn,
            "",
            "",
            20,
            super::super::ContactSort::Name,
            true,
            unsaved,
        )
        .unwrap()
        .into_iter()
        .map(|contact| contact.addr)
        .collect::<Vec<_>>()
    };
    assert_eq!(addrs(true), ["person@example.com"]);
    conn.execute_batch(
        r#"INSERT INTO carddav_accounts(id, server, username) VALUES ('a', 'https://dav.example.test/', 'cora@example.test');
        INSERT INTO carddav_books(id, account, url, name, enabled) VALUES ('b', 'a', '/b/', 'Work', 1);
        INSERT INTO carddav_cards(book, url, etag, name, emails) VALUES ('b', '/b/1.vcf', '"v1"', 'Person', '["Person@Example.com"]');
        INSERT INTO carddav_cards(book, url, etag, name, emails) VALUES ('b', '/b/2.vcf', '"v1"', 'Bea', '["bea@example.test"]');"#,
    )
    .unwrap();
    // Saved in a book, whatever the case: no longer an other contact.
    assert!(addrs(true).is_empty());
    assert_eq!(addrs(false), ["bea@example.test", "person@example.com"]);
    conn.execute("UPDATE carddav_books SET enabled = 0", [])
        .unwrap();
    assert_eq!(addrs(true), ["person@example.com"]);
}

#[test]
fn v32_makes_every_cached_card_download_again() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn.execute_batch(
        r#"INSERT INTO carddav_accounts(id, server, username) VALUES ('a', 'https://dav.example.test/', 'cora@example.test');
        INSERT INTO carddav_books(id, account, url, name, sync_token) VALUES ('b', 'a', '/b/', 'Work', 'token-1');
        INSERT INTO carddav_cards(book, url, etag, name, emails) VALUES ('b', '/b/1.vcf', '"v1"', 'Ada', '[]');
        PRAGMA user_version = 31;"#,
    )
    .unwrap();
    run_migrations(&conn).unwrap();
    let (token, etag): (String, String) = conn
        .query_row(
            "SELECT book.sync_token, card.etag FROM carddav_cards card JOIN carddav_books book ON book.id = card.book",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((token.as_str(), etag.as_str()), ("", ""));
}

#[test]
fn contact_v24_backfills_existing_cache_and_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    // Restore the v23 shape without changing the message cache.
    conn.execute_batch(
        "DROP TRIGGER rule_tag_evaluations_ai; DROP TRIGGER rule_tag_evaluations_au; DROP TRIGGER rule_tag_evaluations_ad;
         DROP TRIGGER rule_evaluations_au;
         DROP TRIGGER rule_arrival_au;
         DROP TABLE rule_activation; DROP TABLE rule_evaluations; DROP TABLE rule_event_sequence;
         DROP TRIGGER rule_events_ai; DROP TRIGGER rule_events_au; DROP TRIGGER rule_events_ad;
         DROP TABLE rule_events; DROP TABLE rule_firings;
         DROP TRIGGER contacts_ai; DROP TRIGGER contacts_ad; DROP TRIGGER contacts_au;
         DROP TABLE contact_participants; DROP TABLE contacts;
         CREATE TABLE contacts(account TEXT, addr_key TEXT, name TEXT, addr TEXT,
           count INTEGER, last_date INTEGER);
         PRAGMA user_version = 23;",
    )
    .unwrap();
    contact_message(&conn, 1, "INBOX", 10);
    contact_message(&conn, 2, "Trash", 20);
    run_migrations(&conn).unwrap();
    run_migrations(&conn).unwrap();
    let entries: i64 = conn
        .query_row("SELECT count(*) FROM contact_participants", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(entries, 2);
    let contacts = super::super::suggest_contacts(&conn, "a", "person", 10).unwrap();
    assert_eq!(contacts.len(), 1);
    conn.execute("DELETE FROM messages WHERE folder = 'INBOX'", [])
        .unwrap();
    assert!(
        super::super::suggest_contacts(&conn, "a", "person", 10)
            .unwrap()
            .is_empty()
    );
}

/// Removing a CardDAV account takes its books and cards with it, through
/// the tables' ON DELETE CASCADE, and leaves other accounts' rows alone.
#[test]
fn removing_a_carddav_account_cascades_to_its_books_and_cards() {
    let dir = std::env::temp_dir().join(format!("meron-cascade-test-{}", uuid::Uuid::new_v4()));
    let conn = super::open_at(dir.join("meron.db")).unwrap();
    let foreign_keys: bool = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert!(foreign_keys);
    conn.execute_batch(
        r#"INSERT INTO carddav_accounts(id, server, username) VALUES
             ('a', 'https://dav.example.test/', 'cora@example.test'),
             ('b', 'https://dav.example.test/', 'dan@example.test');
           INSERT INTO carddav_books(id, account, url, name) VALUES
             ('a1', 'a', '/a/', 'Work'), ('b1', 'b', '/b/', 'Home');
           INSERT INTO carddav_cards(book, url, etag, name, emails) VALUES
             ('a1', '/a/1.vcf', '"v1"', 'Ada', '[]'),
             ('b1', '/b/1.vcf', '"v1"', 'Bea', '[]');"#,
    )
    .unwrap();
    conn.execute("DELETE FROM carddav_accounts WHERE id = 'a'", [])
        .unwrap();
    let left = |table: &str| -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    };
    assert_eq!(left("carddav_books"), 1);
    assert_eq!(left("carddav_cards"), 1);
    let card: String = conn
        .query_row("SELECT name FROM carddav_cards", [], |row| row.get(0))
        .unwrap();
    assert_eq!(card, "Bea");
}
