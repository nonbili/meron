//! Upgrade fixtures: the schema at selected old versions, captured from the
//! code that shipped them, upgraded with seeded fictional rows and compared
//! against checked-in expectations. They pin what the migration ladder does
//! to an existing store, independently of how the ladder is written.
//!
//! `testdata/schema.sql` is the schema a fresh store ends up with. After a
//! deliberate schema change, regenerate it (and the upgrade expectations) with
//! `MERON_BLESS=1 cargo test store::db` and review the diff.

use super::{now_unix, run_migrations};
use rusqlite::Connection;
use rusqlite::types::ValueRef;
use std::path::{Path, PathBuf};

const TESTDATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/store/db/testdata");

fn testdata(name: &str) -> PathBuf {
    Path::new(TESTDATA).join(name)
}

fn read_testdata(name: &str) -> String {
    std::fs::read_to_string(testdata(name))
        .unwrap_or_else(|err| panic!("read {name}: {err}"))
        .replace("\r\n", "\n")
}

/// Compare `actual` with the checked-in `name`, or rewrite it under
/// `MERON_BLESS`. Reports the first differing line rather than two walls of
/// text.
fn check_or_bless(name: &str, actual: &str) {
    if std::env::var_os("MERON_BLESS").is_some() {
        std::fs::write(testdata(name), actual).unwrap();
        return;
    }
    let expected = read_testdata(name);
    if expected == actual {
        return;
    }
    let (expected_lines, actual_lines): (Vec<_>, Vec<_>) =
        (expected.lines().collect(), actual.lines().collect());
    let line = expected_lines
        .iter()
        .zip(&actual_lines)
        .position(|(e, a)| e != a)
        .unwrap_or(expected_lines.len().min(actual_lines.len()));
    panic!(
        "{name} differs at line {}:\n  expected: {:?}\n  actual:   {:?}\n\
         (rerun with MERON_BLESS=1 if the change is intended, and review the diff)",
        line + 1,
        expected_lines.get(line),
        actual_lines.get(line),
    );
}

/// FTS5 shadow tables, which the virtual table creates and owns.
fn shadow_tables(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT name FROM pragma_table_list WHERE schema = 'main' AND type = 'shadow'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// The schema as loadable SQL: tables, then indexes, then triggers, each by
/// name, without root pages, SQLite's own objects or FTS shadow tables.
fn schema_text(conn: &Connection) -> String {
    let shadow = shadow_tables(conn);
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let mut out = format!("PRAGMA user_version = {version};\n");
    let mut stmt = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
             ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1
                                WHEN 'view' THEN 2 ELSE 3 END, name",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .unwrap();
    for row in rows {
        let (kind, name, sql) = row.unwrap();
        if shadow.contains(&name) {
            continue;
        }
        out.push_str(&format!("\n-- {kind} {name}\n{sql};\n"));
    }
    out
}

/// A store at an old version, from its captured schema.
fn load_fixture(name: &str) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&read_testdata(name)).unwrap();
    conn
}

/// Every row of every ordinary table, sorted, plus what full-text search
/// finds, as text. Values within a day of now are written as `<now>`: those
/// are stamped by `unixepoch()` while upgrading.
fn data_text(conn: &Connection) -> String {
    let shadow = shadow_tables(conn);
    let now = now_unix();
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
               AND sql NOT LIKE 'CREATE VIRTUAL TABLE%'
             ORDER BY name",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut out = String::new();
    for table in tables.iter().filter(|table| !shadow.contains(table)) {
        let stmt = conn.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
        let columns = stmt.column_count();
        let order = (1..=columns)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY {order}"))
            .unwrap();
        let names = stmt
            .column_names()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        out.push_str(&format!("\n[{table}] {}\n", names.join(", ")));
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let values = (0..columns)
                .map(|i| match row.get_ref(i).unwrap() {
                    ValueRef::Null => "NULL".to_string(),
                    ValueRef::Integer(v) if (v - now).abs() < 86_400 => "<now>".to_string(),
                    ValueRef::Integer(v) => v.to_string(),
                    ValueRef::Real(v) => v.to_string(),
                    ValueRef::Text(v) => format!("{:?}", String::from_utf8_lossy(v)),
                    ValueRef::Blob(v) => format!("x'{}'", hex(v)),
                })
                .collect::<Vec<_>>();
            out.push_str(&values.join(" | "));
            out.push('\n');
        }
    }
    for (table, column, query) in [
        ("messages_fts", "messages_fts", "quarterly"),
        ("messages_fts", "messages_fts", "grace"),
        ("messages_fts", "messages_fts", "\"日本語\""),
        (
            "messages_recipients_fts",
            "messages_recipients_fts",
            "linus",
        ),
        (
            "messages_recipients_fts",
            "messages_recipients_fts",
            "\"example.org\"",
        ),
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_schema WHERE name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        if !exists {
            continue;
        }
        let hits: Vec<i64> = conn
            .prepare(&format!(
                "SELECT rowid FROM {table} WHERE {column} MATCH ?1 ORDER BY rowid"
            ))
            .unwrap()
            .query_map([query], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        out.push_str(&format!("\n[{table} MATCH {query}] {hits:?}\n"));
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The checks a healthy store passes whatever it holds.
fn assert_consistent(conn: &Connection) {
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let violations: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0, "foreign key violations");
    for table in ["messages_fts", "messages_recipients_fts"] {
        conn.execute(
            &format!("INSERT INTO {table}({table}, rank) VALUES('integrity-check', 1)"),
            [],
        )
        .unwrap_or_else(|err| panic!("{table} integrity-check: {err}"));
    }
}

/// Rows every version since v1 can hold: a mail account with a thread of
/// replies, CJK text, recipients in To and Cc, a Gmail copy, and an RSS item.
const SEED_V1: &str = r#"
INSERT INTO accounts(id, engine, provider, email, display_name, created_at, updated_at)
VALUES ('acc', 'mail', 'custom', 'ada@example.test', 'Ada', 10, 10),
       ('feeds', 'rss', 'rss', '', 'Feeds', 10, 10);
INSERT INTO folders(account, name, delimiter) VALUES
  ('acc', 'INBOX', '/'), ('acc', 'Sent', '/'), ('acc', 'Trash', '/');
INSERT INTO folder_state(account, folder, uidvalidity, uid_next) VALUES ('acc', 'INBOX', 7, 5);
INSERT INTO subscriptions(id, account, url, title) VALUES ('sub', 'feeds', 'https://feeds.example.test/a.xml', 'A feed');
INSERT INTO messages(id, account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key, body, json) VALUES
  (1, 'acc', 'INBOX', '1', 1, 'Quarterly report', 'Grace Hopper', 'grace@example.org', 1000, 1, 0, '<r1@example.org>', 'The quarterly numbers are in.',
   '{"message_id":"<R1@example.org>","to":[{"name":"Ada","addr":"ada@example.test"},{"name":"Linus","addr":"linus@example.org"}],"cc":[{"name":"","addr":"Team@Example.org"}]}'),
  (2, 'acc', 'INBOX', '2', 2, 'Re: Quarterly report', 'Linus', 'linus@example.org', 2000, 0, 1, '<r1@example.org>', 'Looks good to me.',
   '{"message_id":"<r2@example.org>","in_reply_to":"<r1@example.org>","to":[{"name":"Grace Hopper","addr":"grace@example.org"}],"cc":[]}'),
  (3, 'acc', 'Sent', '1', 1, '日本語のメール', 'Ada', 'ada@example.test', 3000, 1, 0, NULL, '日本語の本文です。',
   '{"message_id":"<s1@example.test>","to":[{"name":"Grace Hopper","addr":"grace@example.org"}],"cc":[{"name":"Linus","addr":"linus@example.org"}]}'),
  (4, 'acc', 'INBOX', '4', 4, 'Gmail copy', 'Grace Hopper', 'grace@example.org', 4000, 0, 0, NULL, NULL,
   '{"gmail_msg_id":"18c0ffee","to":[{"name":"Ada","addr":"ada@example.test"}]}'),
  (5, 'acc', 'Trash', '9', 9, 'Old news', NULL, 'noreply@example.org', 0, 1, 0, NULL, NULL, '{}'),
  (6, 'feeds', 'sub', 'item-1', 0, 'A feed item', 'A feed', '', 5000, 0, 0, NULL, 'Feed body', '{"link":"https://feeds.example.test/1"}');
"#;

/// Special-use roles (v4).
const SEED_V4: &str = "
UPDATE folders SET special_use = 'sent' WHERE name = 'Sent';
UPDATE folders SET special_use = 'trash' WHERE name = 'Trash';
";

/// Tasks (v10), invitation answers (v18) and events (v19).
const SEED_V19: &str = r#"
INSERT INTO task_lists(id, title) VALUES ('list', 'Errands');
INSERT INTO tasks(id, list_id, title, account, thread_id) VALUES ('t1', 'list', 'Reply to Grace', 'acc', '<r1@example.org>');
INSERT INTO invite_responses(account, uid, response, sequence) VALUES ('acc', 'evt-1@example.org', 'accepted', 1);
INSERT INTO events(id, title, start_at, end_at, recurring, rrule, account, invite_uid, response)
VALUES ('e1', 'Standup', 1000, 2000, 1, 'weekly', 'acc', 'evt-1@example.org', 'accepted'),
       ('e2', 'Lunch', 3000, 4000, 0, '', '', '', '');
"#;

/// CalDAV (v30) and CardDAV (v31).
const SEED_V31: &str = r#"
INSERT INTO carddav_accounts(id, server, username) VALUES ('dav', 'https://dav.example.test/', 'ada@example.test');
INSERT INTO carddav_books(id, account, url, name, sync_token) VALUES ('book', 'dav', '/ada/book/', 'Work', 'token-7');
INSERT INTO carddav_cards(book, url, etag, name, emails) VALUES ('book', '/ada/book/1.vcf', '"v1"', 'Grace Hopper', '["grace@example.org"]');
"#;

fn upgrade(fixture: &str, seeds: &[&str], expected: &str) {
    let conn = load_fixture(fixture);
    for seed in seeds {
        conn.execute_batch(seed).unwrap();
    }
    run_migrations(&conn).unwrap();
    assert_consistent(&conn);
    assert_eq!(
        schema_text(&conn),
        read_testdata("schema.sql"),
        "{fixture} upgraded differs from a fresh store"
    );
    check_or_bless(expected, &data_text(&conn));
    // Upgrading again is a no-op.
    let before = data_text(&conn);
    run_migrations(&conn).unwrap();
    assert_eq!(data_text(&conn), before);
}

#[test]
fn fresh_schema_matches_snapshot() {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    assert_consistent(&conn);
    check_or_bless("schema.sql", &schema_text(&conn));
}

#[test]
fn upgrades_from_v5() {
    upgrade("v05.sql", &[SEED_V1, SEED_V4], "upgrade_v05.expected");
}

#[test]
fn upgrades_from_v8() {
    upgrade("v08.sql", &[SEED_V1, SEED_V4], "upgrade_v08.expected");
}

#[test]
fn upgrades_from_v16() {
    upgrade("v16.sql", &[SEED_V1, SEED_V4], "upgrade_v16.expected");
}

#[test]
fn upgrades_from_v23() {
    upgrade(
        "v23.sql",
        &[SEED_V1, SEED_V4, SEED_V19],
        "upgrade_v23.expected",
    );
}

#[test]
fn upgrades_from_v31() {
    upgrade(
        "v31.sql",
        &[SEED_V1, SEED_V4, SEED_V19, SEED_V31],
        "upgrade_v31.expected",
    );
}

/// v0.4.9 and v0.5.0 reached v29 before `openpgp_sign_plain` joined the
/// OpenPGP schema.
#[test]
fn upgrades_from_v29_without_sign_plain() {
    upgrade(
        "v29_no_sign_plain.sql",
        &[SEED_V1, SEED_V4, SEED_V19],
        "upgrade_v29_no_sign_plain.expected",
    );
}
