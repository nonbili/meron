use super::*;

#[test]
fn bundled_sqlcipher_uses_patched_sqlite() {
    let conn = Connection::open_in_memory().unwrap();
    let cipher_version: String = conn
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .expect("Meron must link SQLCipher");
    assert!(!cipher_version.is_empty());
    assert!(
        rusqlite::version_number() >= 3_054_000,
        "SQLCipher {cipher_version} uses SQLite {}, affected by RUSTSEC-2026-0459",
        rusqlite::version()
    );
}

#[test]
fn migration_records_cached_identities_and_retires_pending_queue() {
    let conn = Connection::open_in_memory().unwrap();
    db::migrate_to(&conn, 8).unwrap();
    insert_message(&conn, 1, "cached", "", "", None);
    db::run_migrations(&conn).unwrap();
    let pending = || -> i64 {
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'pending_mail_identities'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    let observed = || -> i64 {
        conn.query_row("SELECT count(*) FROM observed_mail_identities", [], |r| {
            r.get(0)
        })
        .unwrap()
    };
    assert_eq!(pending(), 0, "queue must be retired");
    assert_eq!(pending(), 0);
    assert_eq!(observed(), 1);
    let before = conn.total_changes();
    conn.execute("UPDATE messages SET json = json", []).unwrap();
    assert_eq!(
        conn.total_changes(),
        before + 1,
        "unchanged metadata must only update the message row"
    );

    insert_message(&conn, 2, "arrival", "", "", None);
    assert_eq!(pending(), 0);
    assert_eq!(observed(), 2, "cached rows must be observed immediately");
    assert_eq!(pending(), 0);
    assert_eq!(observed(), 2);
    conn.execute("UPDATE messages SET json = json, seen = 1", [])
        .unwrap();
    assert_eq!(pending(), 0, "unchanged metadata must not requeue rows");
    conn.execute(
        "UPDATE messages SET json = '{\"message_id\":\"identity@example.com\"}' WHERE uid = 2",
        [],
    )
    .unwrap();
    assert_eq!(observed(), 3, "changed identities are observed immediately");
    assert_eq!(observed(), 3);
    insert_message(&conn, 3, "deleted", "", "", None);
    conn.execute("DELETE FROM messages WHERE uid = 3", [])
        .unwrap();
    assert_eq!(pending(), 0, "deletion cleans the queue");
}

#[test]
fn migration_v9_skips_fts_writes_for_unchanged_headers() {
    let conn = Connection::open_in_memory().unwrap();
    db::migrate_to(&conn, 8).unwrap();
    insert_message(
        &conn,
        1,
        "originalsubject",
        "sendername",
        "sender@example.com",
        Some("cachedbody"),
    );
    conn.execute(
        "UPDATE messages SET recipients = 'recipient@example.com'",
        [],
    )
    .unwrap();
    let refresh = "UPDATE messages SET subject = subject, from_name = from_name,
        from_addr = from_addr, body = body, recipients = recipients, seen = 1, starred = 1";
    let before = conn.total_changes();
    conn.execute(refresh, []).unwrap();
    assert!(conn.total_changes() - before > 1, "v8 refresh rewrites FTS");

    db::run_migrations(&conn).unwrap();
    let before = conn.total_changes();
    conn.execute(refresh, []).unwrap();
    assert_eq!(
        conn.total_changes() - before,
        1,
        "only the message row should be written"
    );
    // Migrating replaces triggers without losing the existing index contents.
    for (table, term) in [
        ("messages_fts", "cachedbody"),
        ("messages_recipients_fts", "recipient"),
    ] {
        let count: i64 = conn
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE {table} MATCH ?1"),
                [term],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
    db::run_migrations(&conn).unwrap();
}

#[test]
fn fts_updates_follow_changed_text_and_null_transitions() {
    let conn = test_conn();
    insert_message(&conn, 1, "", "", "", None);
    for (column, table) in [
        ("subject", "messages_fts"),
        ("from_name", "messages_fts"),
        ("from_addr", "messages_fts"),
        ("body", "messages_fts"),
        ("recipients", "messages_recipients_fts"),
    ] {
        let count = |term: &str| -> i64 {
            conn.query_row(
                &format!("SELECT count(*) FROM {table} WHERE {table} MATCH ?1"),
                [term],
                |row| row.get(0),
            )
            .unwrap()
        };
        for (old, new) in [("absenttoken", "firsttoken"), ("firsttoken", "secondtoken")] {
            conn.execute(&format!("UPDATE messages SET {column} = ?1"), [new])
                .unwrap();
            assert_eq!(count(old), 0, "{column}: stale term");
            assert_eq!(count(new), 1, "{column}: new term");
        }
        conn.execute(&format!("UPDATE messages SET {column} = NULL"), [])
            .unwrap();
        assert_eq!(count("secondtoken"), 0, "{column}: cleared term");
        let before = conn.total_changes();
        conn.execute(&format!("UPDATE messages SET {column} = NULL"), [])
            .unwrap();
        assert_eq!(
            conn.total_changes() - before,
            1,
            "{column}: NULL to NULL must skip FTS"
        );
        conn.execute(
            &format!("UPDATE messages SET {column} = 'restoredtoken'"),
            [],
        )
        .unwrap();
        assert_eq!(count("restoredtoken"), 1, "{column}: NULL to text");
        conn.execute(&format!("UPDATE messages SET {column} = NULL"), [])
            .unwrap();
    }
}

#[test]
fn run_migrations_creates_schema_and_bumps_version() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);

    for table in [
        "accounts",
        "messages",
        "messages_fts",
        "messages_recipients_fts",
        "mail_search_hits",
        "mail_search_snapshots",
        "mail_search_pending",
        "folders",
        "folder_state",
        "subscriptions",
        "meta",
        "settings",
        "observed_mail_identities",
        "task_lists",
        "tasks",
        "removed_message_uids",
        "active_message_syncs",
    ] {
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                params![table],
                |_| Ok(()),
            )
            .optional()
            .unwrap()
            .is_some();
        assert!(exists, "missing table: {table}");
    }

    // Re-running is a no-op: version is already current, so nothing reapplies.
    db::run_migrations(&conn).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);
}

#[test]
fn concurrent_first_open_runs_migrations_once() {
    let dir = std::env::temp_dir().join(format!("meron-store-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("meron.db");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(6));
    let handles = (0..6)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                db::open_at(&path).map(|_| ())
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().unwrap().unwrap();
    }

    let conn = Connection::open(&path).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tasks_tables_arrive_on_an_existing_install() {
    let conn = Connection::open_in_memory().unwrap();
    db::migrate_to(&conn, 8).unwrap();
    insert_message(&conn, 1, "existing", "", "", None);
    let table_count = |name: &str| -> i64 {
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(table_count("tasks"), 0, "not there before the migration");

    db::run_migrations(&conn).unwrap();

    assert_eq!(table_count("tasks"), 1);
    assert_eq!(table_count("task_lists"), 1);
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);

    // Cached mail is untouched, and the new tables are writable.
    let messages: i64 = conn
        .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(messages, 1);
    insert_task_list(&conn, "list1", "My Tasks").unwrap();
    insert_task(&conn, "task1", "list1", "Write it down", "", 0, "", "", "").unwrap();
    assert_eq!(tasks_for_list(&conn, "list1", false).unwrap().len(), 1);
}

/// `open_notifying` leans on this constant to spot a store it must upgrade.
#[test]
fn schema_version_matches_the_last_migration() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, db::SCHEMA_VERSION);
}

/// Encrypting a legacy store exports all of it, so it has to be announced even
/// when the schema is already current, and only by the open that does it.
#[test]
fn encrypting_a_plaintext_store_is_announced_once_before_it_starts() {
    let dir = std::env::temp_dir().join(format!("meron-encrypt-notify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("meron.db");
    let _ = std::fs::remove_file(&path);
    drop(db::open_at(&path).unwrap());
    let key = "ab".repeat(32);

    let mut announced = 0;
    let conn = db::open_at_notifying(&path, Some(&key), || {
        // The file is still the plaintext original at this point.
        let header = std::fs::read(&path).unwrap();
        assert!(header.starts_with(b"SQLite format 3\0"));
        announced += 1;
    })
    .unwrap();
    drop(conn);
    assert_eq!(announced, 1);

    let conn = db::open_at_notifying(&path, Some(&key), || panic!("nothing to upgrade")).unwrap();
    drop(conn);
    std::fs::remove_dir_all(&dir).unwrap();
}
