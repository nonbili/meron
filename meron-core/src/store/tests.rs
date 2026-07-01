use super::*;

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

#[test]
fn search_messages_matches_subject_sender_and_body_case_insensitively() {
    let conn = test_conn();
    insert_message(&conn, 1, "Quarterly Plan", "Aki", "aki@example.com", None);
    insert_message(&conn, 2, "Hello", "Launch Team", "team@example.com", None);
    insert_message(
        &conn,
        3,
        "Notes",
        "Ops",
        "ops@example.com",
        Some("The deploy window is confirmed."),
    );
    insert_message(&conn, 4, "Other", "No Match", "other@example.com", None);

    let subject = search_messages(&conn, "acct", "INBOX", "quarterly", 10).unwrap();
    assert_eq!(subject.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);

    let sender = search_messages(&conn, "acct", "INBOX", "launch", 10).unwrap();
    assert_eq!(sender.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);

    let body = search_messages(&conn, "acct", "INBOX", "DEPLOY", 10).unwrap();
    assert_eq!(body.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![3]);
}

#[test]
fn search_messages_matches_substrings_including_cjk() {
    let conn = test_conn();
    insert_message(
        &conn,
        1,
        "Quarterly planning",
        "Aki",
        "aki@example.com",
        None,
    );
    insert_message(&conn, 2, "上海会议通知", "Aki", "aki@example.com", None);

    // Mid-word Latin substring (trigram, >= 3 chars).
    let latin = search_messages(&conn, "acct", "INBOX", "arter", 10).unwrap();
    assert_eq!(latin.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);

    // CJK substring with no surrounding word breaks (>= 3 chars -> FTS).
    let cjk = search_messages(&conn, "acct", "INBOX", "会议通", 10).unwrap();
    assert_eq!(cjk.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);

    let miss = search_messages(&conn, "acct", "INBOX", "zzz", 10).unwrap();
    assert!(miss.is_empty());
}

#[test]
fn search_messages_short_query_falls_back_to_like() {
    // Queries below the trigram minimum (3 codepoints) must still match via
    // the LIKE fallback — important for 2-character CJK words.
    let conn = test_conn();
    insert_message(&conn, 1, "上海会议通知", "Aki", "aki@example.com", None);
    insert_message(&conn, 2, "Other", "Aki", "aki@example.com", None);

    let hit = search_messages(&conn, "acct", "INBOX", "上海", 10).unwrap();
    assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn search_messages_reindexes_on_body_update() {
    // The FTS triggers must follow a later body write, not just the initial insert.
    let conn = test_conn();
    insert_message(&conn, 1, "Notes", "Ops", "ops@example.com", None);
    assert!(
        search_messages(&conn, "acct", "INBOX", "deploy", 10)
            .unwrap()
            .is_empty()
    );

    conn.execute(
        "UPDATE messages SET body = 'deploy window confirmed' WHERE uid = 1",
        [],
    )
    .unwrap();

    let hit = search_messages(&conn, "acct", "INBOX", "deploy", 10).unwrap();
    assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn get_starred_all_accounts_spans_accounts_and_skips_rss_rows() {
    let conn = test_conn();
    let insert = |account: &str, folder: &str, uid: u32, subject: &str, date: i64, starred: i64| {
        conn.execute(
                "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred)
                 VALUES(?1, ?2, ?3, ?4, ?5, 'Aki', 'aki@example.com', ?6, 1, ?7)",
                params![account, folder, uid.to_string(), uid, subject, date, starred],
            )
            .unwrap();
    };
    insert("a1", "INBOX", 1, "Starred newer", 1_767_312_000, 1); // 2026-01-02
    insert("a1", "INBOX", 2, "Not starred", 1_767_398_400, 0); // 2026-01-03
    insert("a2", "Sent", 5, "Starred older", 1_767_225_600, 1); // 2026-01-01
    // RSS row: uid = 0, must be excluded even when starred.
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen, starred)
             VALUES('rss-acct', 'feed-1', 'item-1', 0, 'Feed item', 0, 1, 1)",
        [],
    )
    .unwrap();

    let starred = get_starred_all_accounts(&conn, 10).unwrap();
    assert_eq!(
        starred
            .iter()
            .map(|(account, m)| (account.as_str(), m.folder.as_str(), m.uid))
            .collect::<Vec<_>>(),
        vec![("a1", "INBOX", 1), ("a2", "Sent", 5)]
    );
    assert!(starred.iter().all(|(_, m)| m.starred));
}

#[test]
fn get_recent_page_can_return_only_unread_messages() {
    let conn = test_conn();
    insert_message(&conn, 2, "Unread older", "Aki", "aki@example.com", None);
    insert_message(&conn, 3, "Unread middle", "Aki", "aki@example.com", None);
    insert_message(&conn, 4, "Read middle", "Aki", "aki@example.com", None);
    insert_message(&conn, 5, "Unread newest", "Aki", "aki@example.com", None);
    conn.execute(
        "UPDATE messages SET seen = 1 WHERE account = 'acct' AND folder = 'INBOX' AND uid = 4",
        [],
    )
    .unwrap();

    // insert_message stamps every row with the same date, so the date-ordered
    // list ties break on uid DESC and the cursor carries that shared date.
    const D: i64 = 1779580800;
    let (all, all_cursor) = get_recent_page(&conn, "acct", "INBOX", 2, None, false).unwrap();
    assert_eq!(all.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![5, 4]);
    assert_eq!(all_cursor.as_deref(), Some(format!("date:{D}:4").as_str()));

    let (unread, unread_cursor) = get_recent_page(&conn, "acct", "INBOX", 2, None, true).unwrap();
    assert_eq!(unread.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![5, 3]);
    assert_eq!(
        unread_cursor.as_deref(),
        Some(format!("date:{D}:3").as_str())
    );
    assert!(unread.iter().all(|m| !m.seen));

    let (next_unread, next_cursor) =
        get_recent_page(&conn, "acct", "INBOX", 2, Some((D, 3)), true).unwrap();
    assert_eq!(
        next_unread.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(next_cursor, None);
}

#[test]
fn suggest_contacts_dedupes_by_address_and_ranks_by_frequency() {
    let conn = test_conn();
    // aki appears twice → ranks above bea; the duplicate collapses to one row.
    insert_message(&conn, 1, "Hi", "Aki", "aki@example.com", None);
    insert_message(&conn, 2, "Re: Hi", "Aki", "aki@example.com", None);
    insert_message(&conn, 3, "Plan", "Bea", "bea@example.com", None);

    let top = suggest_contacts(&conn, "acct", "", 10).unwrap();
    assert_eq!(
        top.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
        vec!["aki@example.com", "bea@example.com"]
    );

    // Query matches on both address and display name, case-insensitively.
    let by_name = suggest_contacts(&conn, "acct", "BEA", 10).unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].addr, "bea@example.com");
    assert_eq!(by_name[0].name, "Bea");

    // A different account sees none of acct's correspondents.
    assert!(suggest_contacts(&conn, "other", "", 10).unwrap().is_empty());
}

#[test]
fn suggest_contacts_includes_to_and_cc_recipients() {
    use crate::imap::{MessageHeader, Recipient};
    let conn = test_conn();
    // A sent message we authored: the useful contacts are its To/Cc, not us.
    let msg = MessageHeader {
        uid: 10,
        subject: "Lunch".into(),
        from_name: "Me".into(),
        from_addr: "me@example.com".into(),
        date: 1_779_580_800, // 2026-05-24
        to: vec![Recipient {
            name: "Cleo".into(),
            addr: "cleo@example.com".into(),
        }],
        cc: vec![Recipient {
            name: String::new(),
            addr: "dan@example.com".into(),
        }],
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "Sent", &[msg]).unwrap();

    let cleo = suggest_contacts(&conn, "acct", "cleo", 10).unwrap();
    assert_eq!(cleo.len(), 1);
    assert_eq!(cleo[0].addr, "cleo@example.com");
    assert_eq!(cleo[0].name, "Cleo");

    // A Cc-only address with no display name is still suggested by address.
    let dan = suggest_contacts(&conn, "acct", "dan@", 10).unwrap();
    assert_eq!(
        dan.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
        vec!["dan@example.com"]
    );
}

#[test]
fn apply_card_identity_rewrites_outbound_to_counterparty() {
    use crate::imap::{MessageHeader, Recipient};
    let conn = test_conn();
    conn.execute(
            "INSERT INTO accounts(id, engine, email, prefs)
             VALUES('acct', 'mail', 'me@example.com', '{\"aliases\":[{\"email\":\"alias@other.jp\"}]}')",
            [],
        )
        .unwrap();

    let recipient = |name: &str, addr: &str| Recipient {
        name: name.into(),
        addr: addr.into(),
    };
    let mut headers = vec![
        // Outbound from the primary address → shows first recipient, +1 overflow.
        MessageHeader {
            uid: 1,
            from_name: "Me".into(),
            from_addr: "me@example.com".into(),
            to: vec![
                recipient("Cleo", "cleo@example.com"),
                recipient("Dan", "dan@example.com"),
            ],
            ..Default::default()
        },
        // Outbound from an alias counts as self too.
        MessageHeader {
            uid: 2,
            from_name: "Me".into(),
            from_addr: "Alias@Other.JP".into(),
            to: vec![recipient("Cleo", "cleo@example.com")],
            ..Default::default()
        },
        // Inbound stays untouched.
        MessageHeader {
            uid: 3,
            from_name: "Cleo".into(),
            from_addr: "cleo@example.com".into(),
            ..Default::default()
        },
        // Outbound with no cached recipients (old row / Bcc-only) stays untouched.
        MessageHeader {
            uid: 4,
            from_name: "Me".into(),
            from_addr: "me@example.com".into(),
            ..Default::default()
        },
    ];
    apply_card_identity(&conn, "acct", &mut headers);

    assert_eq!(headers[0].from_name, "Cleo");
    assert_eq!(headers[0].from_addr, "cleo@example.com");
    assert_eq!(headers[0].recipient_overflow, 1);
    assert_eq!(headers[1].from_name, "Cleo");
    assert_eq!(headers[1].recipient_overflow, 0);
    assert_eq!(headers[2].from_name, "Cleo");
    assert_eq!(headers[2].from_addr, "cleo@example.com");
    assert_eq!(headers[3].from_name, "Me");
    assert_eq!(headers[3].from_addr, "me@example.com");
}

#[test]
fn apply_card_identity_resolves_junk_display_names() {
    use crate::imap::{MessageHeader, Recipient};
    let conn = test_conn();
    conn.execute(
        "INSERT INTO accounts(id, engine, email) VALUES('acct', 'mail', 'me@example.com')",
        [],
    )
    .unwrap();
    // A cached inbound message carries the bot's proper display name.
    insert_message(&conn, 1, "Hi", "Austin Mentions", "bot@example.com", None);

    let mut headers = vec![
        // Outbound reply whose To name is the junk "addr addr" pattern.
        MessageHeader {
            uid: 2,
            from_name: "Me".into(),
            from_addr: "me@example.com".into(),
            to: vec![Recipient {
                name: "bot@example.com bot@example.com".into(),
                addr: "bot@example.com".into(),
            }],
            ..Default::default()
        },
        // Inbound with the same junk in From resolves too.
        MessageHeader {
            uid: 3,
            from_name: "bot@example.com bot@example.com".into(),
            from_addr: "bot@example.com".into(),
            ..Default::default()
        },
        // Junk name with no better name cached anywhere is cleared.
        MessageHeader {
            uid: 4,
            from_name: "stranger@example.com".into(),
            from_addr: "stranger@example.com".into(),
            ..Default::default()
        },
    ];
    apply_card_identity(&conn, "acct", &mut headers);

    assert_eq!(headers[0].from_name, "Austin Mentions");
    assert_eq!(headers[0].from_addr, "bot@example.com");
    assert_eq!(headers[1].from_name, "Austin Mentions");
    assert_eq!(headers[2].from_name, "");
}

#[test]
fn save_cached_message_preserves_envelope_recipients_in_json() {
    use crate::imap::{MessageHeader, Recipient};
    let conn = test_conn();
    let header = MessageHeader {
        uid: 42,
        subject: "Hi".into(),
        from_name: "Sender".into(),
        from_addr: "sender@example.com".into(),
        date: 1_779_580_800, // 2026-05-24
        to: vec![Recipient {
            name: "Me".into(),
            addr: "me@example.com".into(),
        }],
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[header]).unwrap();

    save_cached_message(
        &conn,
        "acct",
        "INBOX",
        42,
        &Message {
            subject: "Hi".into(),
            from_name: "Sender".into(),
            from_addr: "sender@example.com".into(),
            body: "hello".into(),
            ..Default::default()
        },
    )
    .unwrap();

    let contacts = suggest_contacts(&conn, "acct", "me@", 10).unwrap();
    assert_eq!(
        contacts.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
        vec!["me@example.com"]
    );
    let message = get_cached_message(&conn, "acct", "INBOX", 42)
        .unwrap()
        .unwrap();
    assert_eq!(message.to, "Me <me@example.com>");
}

#[test]
fn run_migrations_creates_schema_and_bumps_version() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3);

    for table in [
        "accounts",
        "messages",
        "messages_fts",
        "folders",
        "folder_state",
        "subscriptions",
        "meta",
        "settings",
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
    assert_eq!(version, 3);
}

#[test]
fn prefs_resolve_engine_default_and_persist_without_clobbering() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(db::ACCOUNTS_DDL).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine) VALUES('rss-1', 'rss')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine) VALUES('mail-1', 'mail')",
        [],
    )
    .unwrap();

    // Unset: resolved by engine (RSS on, mail off).
    assert!(load_remote_images(&conn, "rss-1").unwrap());
    assert!(!load_remote_images(&conn, "mail-1").unwrap());
    assert!(!load_remote_images(&conn, "missing").unwrap());

    // A sibling pref must survive an images toggle (json_set is in-place).
    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'mail-1'",
        [],
    )
    .unwrap();
    set_load_remote_images(&conn, "mail-1", true).unwrap();
    assert!(load_remote_images(&conn, "mail-1").unwrap());
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'mail-1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        prefs.contains("\"other\""),
        "sibling pref clobbered: {prefs}"
    );

    // Explicit override of the engine default round-trips.
    set_load_remote_images(&conn, "rss-1", false).unwrap();
    assert!(!load_remote_images(&conn, "rss-1").unwrap());
}

#[test]
fn conversation_html_defaults_on_and_respects_explicit_overrides() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES
                ('unset', 'mail', 'custom', '{}'),
                ('plain', 'mail', 'custom', '{\"conversation_html\":false}'),
                ('html', 'mail', 'custom', '{\"conversation_html\":true}')",
        [],
    )
    .unwrap();

    let accounts = list_accounts(&conn).unwrap();
    let by_id = |id: &str| {
        accounts
            .iter()
            .find(|account| account["id"] == id)
            .expect("account present")
    };

    assert_eq!(by_id("unset")["conversation_html"], true);
    assert_eq!(by_id("plain")["conversation_html"], false);
    assert_eq!(by_id("html")["conversation_html"], true);
}

#[test]
fn chat_wallpaper_round_trips_and_can_be_cleared() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
        [],
    )
    .unwrap();

    let wallpaper = json!({"kind":"preset","presetId":"grid"});
    set_account_pref_json(&conn, "acct", "chat_wallpaper", Some(wallpaper.clone())).unwrap();
    let accounts = list_accounts(&conn).unwrap();
    assert_eq!(accounts[0]["chat_wallpaper"], wallpaper);

    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'acct'",
        [],
    )
    .unwrap();
    set_account_pref_json(&conn, "acct", "chat_wallpaper", None).unwrap();
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'acct'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let value: Value = serde_json::from_str(&prefs).unwrap();
    assert_eq!(value["chat_wallpaper"], Value::Null);
    assert_eq!(value["other"], 7);
}

#[test]
fn save_sent_copy_pref_round_trips_and_can_be_cleared() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
        [],
    )
    .unwrap();

    assert_eq!(save_sent_copy_pref(&conn, "acct").unwrap(), None);
    set_account_pref_json(&conn, "acct", "save_sent_copy", Some(json!(false))).unwrap();
    assert_eq!(save_sent_copy_pref(&conn, "acct").unwrap(), Some(false));
    assert_eq!(list_accounts(&conn).unwrap()[0]["save_sent_copy"], false);

    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'acct'",
        [],
    )
    .unwrap();
    set_account_pref_json(&conn, "acct", "save_sent_copy", None).unwrap();
    assert_eq!(save_sent_copy_pref(&conn, "acct").unwrap(), None);
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'acct'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let value: Value = serde_json::from_str(&prefs).unwrap();
    assert_eq!(value["save_sent_copy"], Value::Null);
    assert_eq!(value["other"], 7);
}

#[test]
fn rss_sync_interval_defaults_and_persists_without_clobbering() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider) VALUES('rss-1', 'rss', 'rss')",
        [],
    )
    .unwrap();

    let accounts = list_accounts(&conn).unwrap();
    assert_eq!(
        accounts[0]["rss_sync_interval_minutes"],
        DEFAULT_RSS_SYNC_INTERVAL_MINUTES
    );

    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'rss-1'",
        [],
    )
    .unwrap();
    set_account_pref_u64(&conn, "rss-1", "rss_sync_interval_minutes", 30).unwrap();

    let accounts = list_accounts(&conn).unwrap();
    assert_eq!(accounts[0]["rss_sync_interval_minutes"], 30);
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'rss-1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        prefs.contains("\"other\""),
        "sibling pref clobbered: {prefs}"
    );
}

#[test]
fn search_messages_returns_empty_for_blank_query() {
    let conn = test_conn();
    insert_message(&conn, 1, "Hello", "Aki", "aki@example.com", None);

    let results = search_messages(&conn, "acct", "INBOX", "  ", 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn get_thread_headers_all_folders_sorts_chronologically_by_parsed_date() {
    let conn = test_conn();
    // Insert via the real RFC 2822 parser (the ingestion path), so this also
    // covers parse_date_to_epoch; ordering is then done by the stored epoch.
    let d = |s: &str| crate::parse::parse_date_to_epoch(s);
    conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
             VALUES('acct', 'Sent', '2', 2, 'Re: Topic', 'Me', 'me@example.com', ?1, 1, 'topic-key'),
                   ('acct', 'INBOX', '3', 3, 'Re: Topic', 'Them', 'them@example.com', ?2, 1, 'topic-key'),
                   ('acct', 'INBOX', '1', 1, 'Topic', 'Them', 'them@example.com', ?3, 1, 'topic-key')",
            params![
                d("Fri, 22 May 2026 21:05:20 -0700"),
                d("Fri, 22 May 2026 21:13:27 -0700"),
                d("Thu, 21 May 2026 22:02:02 -0700"),
            ],
        )
        .unwrap();

    let headers = get_thread_headers_all_folders(&conn, "acct", "topic-key").unwrap();
    let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
    assert_eq!(uids, vec![1, 2, 3]);
}

#[test]
fn get_thread_headers_all_folders_dedupes_self_sent_by_message_id() {
    let conn = test_conn();
    // A message sent to yourself lands in both Sent and the Inbox with the
    // same RFC Message-ID but distinct per-folder UIDs. The thread view must
    // collapse the pair into one bubble.
    conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, thread_key, json)
             VALUES('acct', '[Gmail]/Sent Mail', '462', 462, 'test', 'Me', 'me@example.com', 100, 'self-key',
                      '{\"message_id\":\"mid@host\"}'),
                   ('acct', 'INBOX', '1440', 1440, 'test', 'Me', 'me@example.com', 100, 'self-key',
                      '{\"message_id\":\"mid@host\"}'),
                   ('acct', 'INBOX', '1441', 1441, 'reply', 'Them', 'them@example.com', 200, 'self-key',
                      '{\"message_id\":\"other@host\"}')",
            [],
        )
        .unwrap();

    let headers = get_thread_headers_all_folders(&conn, "acct", "self-key").unwrap();
    let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
    // The self-sent pair collapses to the lowest-id row (the Sent copy,
    // inserted first); the distinct reply stays.
    assert_eq!(uids, vec![462, 1441]);
}

#[test]
fn get_thread_headers_all_folders_keeps_rows_without_message_id() {
    let conn = test_conn();
    // Rows lacking a Message-ID (e.g. drafts) must never be collapsed into
    // each other — a NULL Message-ID never equates in SQL.
    conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, thread_key, json)
             VALUES('acct', 'Drafts', '10', 10, 'draft a', 'Me', 'me@example.com', 100, 'k', '{}'),
                   ('acct', 'Drafts', '11', 11, 'draft b', 'Me', 'me@example.com', 200, 'k', '{}')",
            [],
        )
        .unwrap();

    let headers = get_thread_headers_all_folders(&conn, "acct", "k").unwrap();
    let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
    assert_eq!(uids, vec![10, 11]);
}

#[test]
fn thread_reference_gaps_are_referenced_ids_not_yet_cached() {
    let conn = test_conn();
    // A draft replying into a thread: thread_key is the (lowercased) root id;
    // its json carries its own Message-ID and the full References chain. The
    // referenced ancestors aren't cached, so they're all gaps — except the
    // one ancestor we also store as a cached row below.
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date, thread_key, json)
             VALUES('acct', 'Drafts', '99', 99, 'Re: Hi', 0, 'root@h',
               '{\"message_id\":\"leaf@h\",\"references\":\"root@h MID-ONE@h mid-two@h\"}')",
        [],
    )
    .unwrap();
    // An already-cached ancestor (its own Message-ID present) must be excluded.
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date, thread_key, json)
             VALUES('acct', 'INBOX', '50', 50, 'Hi', 0, 'root@h',
               '{\"message_id\":\"mid-two@h\",\"references\":\"root@h\"}')",
        [],
    )
    .unwrap();

    let mut gaps = get_thread_reference_gaps(&conn, "acct", "root@h").unwrap();
    gaps.sort();
    // root@h and mid-one@h are referenced but uncached; mid-two@h is present;
    // leaf@h is a Message-ID, never a gap. Comparison is case-insensitive.
    assert_eq!(gaps, vec!["mid-one@h".to_string(), "root@h".to_string()]);
}

#[test]
fn body_cache_invalidation_works() {
    let conn = test_conn();

    // Save an HTML-only message (body_is_rendered = true)
    let html = "<p>rendered plain</p>";
    let msg_html = Message {
        subject: "HTML only".into(),
        body: "stale render".into(),
        body_html: Some(html.into()),
        body_is_rendered: true,
        ..Default::default()
    };
    save_cached_message(&conn, "acct", "INBOX", 101, &msg_html).unwrap();

    // Save a plain-text message (body_is_rendered = false)
    let msg_plain = Message {
        subject: "Plain only".into(),
        body: "original plain".into(),
        body_is_rendered: false,
        ..Default::default()
    };
    save_cached_message(&conn, "acct", "INBOX", 102, &msg_plain).unwrap();

    // Verify they are initially saved with their bodies
    let retrieved_html = get_cached_message(&conn, "acct", "INBOX", 101)
        .unwrap()
        .unwrap();
    assert_eq!(retrieved_html.body, "stale render");
    assert!(retrieved_html.body_is_rendered);

    let retrieved_plain = get_cached_message(&conn, "acct", "INBOX", 102)
        .unwrap()
        .unwrap();
    assert_eq!(retrieved_plain.body, "original plain");
    assert!(!retrieved_plain.body_is_rendered);

    // Force cache invalidation by setting the meta version in DB to an older one
    db::meta_set(&conn, "body_cache_version", "old_version").unwrap();

    // Call the invalidator
    db::invalidate_body_cache_if_needed(&conn).unwrap();

    // The html-only body is re-rendered in place (not nulled), so the FTS
    // `body` column stays populated across the bump.
    let html_body_in_db: Option<String> = conn
        .query_row("SELECT body FROM messages WHERE uid = 101", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(html_body_in_db, Some(crate::parse::render_body(html)));

    // The plain-only body is left untouched.
    let plain_body_in_db: Option<String> = conn
        .query_row("SELECT body FROM messages WHERE uid = 102", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(plain_body_in_db, Some("original plain".to_string()));

    // FTS still matches the re-rendered html-only message after the bump.
    let hits = search_messages(&conn, "acct", "INBOX", "rendered", 10).unwrap();
    assert!(hits.iter().any(|m| m.subject == "HTML only"));
}
