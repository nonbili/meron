use super::*;

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

    let subject = search_messages(&conn, "acct", "INBOX", "quarterly", 10, None).unwrap();
    assert_eq!(subject.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);

    let sender = search_messages(&conn, "acct", "INBOX", "launch", 10, None).unwrap();
    assert_eq!(sender.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);

    let body = search_messages(&conn, "acct", "INBOX", "DEPLOY", 10, None).unwrap();
    assert_eq!(body.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![3]);
}

#[test]
fn cached_search_reads_carry_the_message_id() {
    use crate::imap::MessageHeader;
    // Search pages span folders, and the grouping that turns them into cards
    // folds a self-sent message's two copies by Message-ID. A cache-answered
    // search (offline, or the first page before the live results land) must
    // carry the id too, or its cards count that message twice.
    let conn = test_conn();
    let copy = |folder: &str, uid: u32| MessageHeader {
        uid,
        folder: folder.to_string(),
        subject: "Quarterly Plan".to_string(),
        date: 100,
        thread_key: "root@example.com".to_string(),
        message_id: "<self@example.com>".to_string(),
        ..Default::default()
    };
    ensure_folder(&conn, "acct", "INBOX").unwrap();
    ensure_folder(&conn, "acct", "Sent").unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[copy("INBOX", 1)]).unwrap();
    upsert_messages(&conn, "acct", "Sent", &[copy("Sent", 9)]).unwrap();

    let folders = vec!["INBOX".to_string(), "Sent".to_string()];
    // Both reads: the trigram index answers queries of three codepoints or
    // more, the LIKE scan the shorter ones.
    for query in ["quarterly", "an"] {
        let hits = search_messages_in_folders(&conn, "acct", &folders, query, 10, None).unwrap();
        assert_eq!(hits.len(), 2, "{query}");
        assert!(
            hits.iter()
                .all(|hit| hit.message_id == "<self@example.com>"),
            "{query}"
        );
        let cards = group_thread_cards(hits, "INBOX");
        assert_eq!(cards.len(), 1, "{query}");
        assert_eq!(cards[0].message_count, 1, "{query}");
    }
}

#[test]
fn search_messages_has_attachment_reads_the_files_column() {
    let conn = test_conn();
    insert_message(&conn, 1, "Plan", "Aki", "aki@example.com", None);
    insert_message(&conn, 2, "Plan", "Aki", "aki@example.com", None);
    conn.execute(
        "UPDATE messages SET files = '[{\"filename\":\"a.pdf\",\"mime\":\"application/pdf\"}]'
         WHERE uid = 2",
        [],
    )
    .unwrap();

    let uids = |query: &str| {
        search_messages(&conn, "acct", "INBOX", query, 10, None)
            .unwrap()
            .iter()
            .map(|message| message.uid)
            .collect::<Vec<_>>()
    };
    assert_eq!(uids("has:attachment"), [2]);
    assert_eq!(uids("plan has:attachment"), [2]);
}

#[test]
fn cached_search_headers_returns_the_rows_it_holds() {
    let conn = test_conn();
    insert_message(&conn, 1, "Plain", "Aki", "aki@example.com", Some("read"));
    insert_message(&conn, 2, "Files", "Aki", "aki@example.com", None);
    conn.execute(
        "UPDATE messages SET files = '[{\"filename\":\"a.pdf\",\"mime\":\"application/pdf\"}]',
                             json = '{\"gmail_msg_id\":7,\"message_id\":\"m2\",\"attachment_metadata\":true}'
         WHERE uid = 2",
        [],
    )
    .unwrap();

    // Header-only, attachments never read: the fetch has to fill this one in.
    insert_message(&conn, 3, "Unknown", "Aki", "aki@example.com", None);

    let headers = cached_search_headers(&conn, "acct", "INBOX", &[1, 2, 3, 4]).unwrap();
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].folder, "INBOX");
    assert_eq!(headers[0].files.as_deref().map(<[_]>::len), Some(0));
    assert_eq!(headers[1].files.as_deref().map(<[_]>::len), Some(1));
    assert_eq!(headers[1].gmail_msg_id, Some(7));
    assert_eq!(headers[1].message_id, "m2");
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
    let latin = search_messages(&conn, "acct", "INBOX", "arter", 10, None).unwrap();
    assert_eq!(latin.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);

    // CJK substring with no surrounding word breaks (>= 3 chars -> FTS).
    let cjk = search_messages(&conn, "acct", "INBOX", "会议通", 10, None).unwrap();
    assert_eq!(cjk.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);

    let miss = search_messages(&conn, "acct", "INBOX", "zzz", 10, None).unwrap();
    assert!(miss.is_empty());
}

#[test]
fn search_messages_short_query_falls_back_to_like() {
    // Queries below the trigram minimum (3 codepoints) must still match via
    // the LIKE fallback — important for 2-character CJK words.
    let conn = test_conn();
    insert_message(&conn, 1, "上海会议通知", "Aki", "aki@example.com", None);
    insert_message(&conn, 2, "Other", "Aki", "aki@example.com", None);

    let hit = search_messages(&conn, "acct", "INBOX", "上海", 10, None).unwrap();
    assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn search_messages_short_query_folds_non_ascii_case() {
    // SQLite's lower() is ASCII-only; the short-query path must fold Cyrillic
    // and accented capitals the way the trigram index does for longer queries.
    let conn = test_conn();
    insert_message(&conn, 1, "Ок, созвон", "Aki", "aki@example.com", None);
    insert_message(&conn, 2, "ÉT report", "Aki", "aki@example.com", None);
    insert_message(&conn, 3, "Other", "Aki", "aki@example.com", None);

    let hit = search_messages(&conn, "acct", "INBOX", "ок", 10, None).unwrap();
    assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
    let hit = search_messages(&conn, "acct", "INBOX", "ét", 10, None).unwrap();
    assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);
}

#[test]
fn search_messages_reindexes_on_body_update() {
    // The FTS triggers must follow a later body write, not just the initial insert.
    let conn = test_conn();
    insert_message(&conn, 1, "Notes", "Ops", "ops@example.com", None);
    assert!(
        search_messages(&conn, "acct", "INBOX", "deploy", 10, None)
            .unwrap()
            .is_empty()
    );

    conn.execute(
        "UPDATE messages SET body = 'deploy window confirmed' WHERE uid = 1",
        [],
    )
    .unwrap();

    let hit = search_messages(&conn, "acct", "INBOX", "deploy", 10, None).unwrap();
    assert_eq!(hit.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn search_messages_matches_to_and_cc_recipients() {
    // Recipients live in the JSON catch-all, so they are searchable only through
    // the mirrored `recipients` column and its index — both the trigram path and
    // the short-query LIKE fallback must see them.
    let conn = test_conn();
    upsert_messages(
        &conn,
        "acct",
        "Sent",
        &[MessageHeader {
            uid: 1,
            subject: "Lunch".to_string(),
            from_addr: "me@example.com".to_string(),
            to: vec![crate::imap::Recipient {
                name: "Ann Baker".to_string(),
                addr: "ann@example.com".to_string(),
            }],
            cc: vec![crate::imap::Recipient {
                name: String::new(),
                addr: "ops@example.com".to_string(),
            }],
            ..Default::default()
        }],
    )
    .unwrap();

    for query in ["Ann Baker", "ann@example", "ops@example", "an"] {
        let hits = search_messages(&conn, "acct", "Sent", query, 10, None).unwrap();
        assert_eq!(
            hits.iter().map(|m| m.uid).collect::<Vec<_>>(),
            vec![1],
            "query: {query}"
        );
    }
    assert!(
        search_messages(&conn, "acct", "Sent", "zoe@example", 10, None)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn search_messages_applies_gmail_style_operators() {
    let conn = test_conn();
    let message = |uid: u32, from: &str, to: &str, subject: &str, date: i64| MessageHeader {
        uid,
        subject: subject.to_string(),
        from_name: String::new(),
        from_addr: from.to_string(),
        date,
        seen: uid == 1,
        starred: uid == 2,
        to: vec![crate::imap::Recipient {
            name: String::new(),
            addr: to.to_string(),
        }],
        ..Default::default()
    };
    let day = |y, m, d| {
        chrono::NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_local_timezone(chrono::Local)
            .unwrap()
            .timestamp()
    };
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[
            message(1, "a@b.c", "d@e.f", "Budget plan", day(2026, 1, 1)),
            message(2, "a@b.c", "x@y.z", "Budget", day(2026, 2, 1)),
            message(3, "zoe@b.c", "d@e.f", "Lunch plan", day(2026, 3, 1)),
        ],
    )
    .unwrap();

    let uids = |query: &str| {
        search_messages(&conn, "acct", "INBOX", query, 10, None)
            .unwrap()
            .iter()
            .map(|m| m.uid)
            .collect::<Vec<_>>()
    };
    assert_eq!(uids("from:a@b.c"), [2, 1]);
    assert_eq!(uids("from:a@b.c to:d@e.f"), [1]);
    // Free text still searches every column, ANDed with the operators.
    assert_eq!(uids("to:d@e.f plan"), [3, 1]);
    assert_eq!(uids("subject:budget is:unread"), [2]);
    assert_eq!(uids("is:starred"), [2]);
    assert_eq!(uids("after:2026/01/15 before:2026/03/01"), [2]);
    assert_eq!(uids("after:2026/03/01"), [3]);
    // Short values take the scan path and still respect their column.
    assert_eq!(uids("from:zo"), [3]);
    assert_eq!(uids("subject:lu"), [3]);
    assert_eq!(uids("from:lu"), Vec::<u32>::new());
    // A bare operator is plain text, not an empty filter.
    assert_eq!(uids("from:"), Vec::<u32>::new());
    // Alternatives: any of them matches, indexed or scanned.
    assert_eq!(uids("from:{zoe@b.c x@y.z}"), [3]);
    assert_eq!(uids("to:(x@y.z OR d@e.f) subject:plan"), [3, 1]);
    assert_eq!(uids("subject:lunch OR subject:budget"), [3, 2, 1]);
    assert_eq!(uids("from:zo,nobody@x.y"), [3]);
}

#[test]
fn search_messages_keeps_indexed_recipients_through_a_flag_only_resync() {
    // A flag-only resync carries no envelope; it must not blank the recipients
    // the envelope fetch already indexed.
    let conn = test_conn();
    let envelope = MessageHeader {
        uid: 1,
        subject: "Lunch".to_string(),
        to: vec![crate::imap::Recipient {
            name: "Ann".to_string(),
            addr: "ann@example.com".to_string(),
        }],
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[envelope]).unwrap();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Lunch".to_string(),
            seen: true,
            ..Default::default()
        }],
    )
    .unwrap();

    let hits = search_messages(&conn, "acct", "INBOX", "ann@example", 10, None).unwrap();
    assert_eq!(hits.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn migration_indexes_recipients_already_cached_before_v6() {
    // An existing install carries its recipients only in `json`; the upgrade has
    // to backfill and index them, or search stays blind to everything synced
    // before it.
    let conn = Connection::open_in_memory().unwrap();
    db::migrate_to(&conn, 5).unwrap();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, json)
         VALUES('acct', 'Sent', '1', 1, 'Lunch', 'Me', 'me@example.com', 100,
                '{\"to\":[{\"name\":\"Ann Baker\",\"addr\":\"ann@example.com\"}]}')",
        [],
    )
    .unwrap();

    db::run_migrations(&conn).unwrap();

    let hits = search_messages(&conn, "acct", "Sent", "ann@example", 10, None).unwrap();
    assert_eq!(hits.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

#[test]
fn search_messages_pages_with_a_keyset_cursor() {
    let conn = test_conn();
    for uid in 1..=3u32 {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date)
             VALUES('acct', 'INBOX', ?1, ?2, 'deploy notes', 'Ops', 'ops@example.com', ?3)",
            params![uid.to_string(), uid, 100 * uid as i64],
        )
        .unwrap();
    }

    let first = search_messages(&conn, "acct", "INBOX", "deploy", 2, None).unwrap();
    assert_eq!(first.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![3, 2]);
    let cursor = crate::thread_list::SearchCursor {
        date: 200,
        uid: 2,
        folder: "INBOX".to_string(),
        scanned: 2,
        snapshot: None,
        offset: 0,
    };

    let second = search_messages(&conn, "acct", "INBOX", "deploy", 2, Some(&cursor)).unwrap();
    assert_eq!(second.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
    // A short page means the result set is exhausted, so paging stops.
    assert!(search_next_cursor(&second, 2, 4).is_none());
}

#[test]
fn search_messages_in_folders_merges_newest_first_and_tags_the_folder() {
    let conn = test_conn();
    let insert = |folder: &str, uid: u32, date: i64| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date)
             VALUES('acct', ?1, ?2, ?3, 'deploy notes', 'Ops', 'ops@example.com', ?4)",
            params![folder, uid.to_string(), uid, date],
        )
        .unwrap();
    };
    insert("INBOX", 1, 100);
    insert("Sent", 1, 200);
    insert("Archive", 9, 300);

    let hits = search_messages_in_folders(
        &conn,
        "acct",
        &["INBOX".to_string(), "Sent".to_string()],
        "deploy",
        10,
        None,
    )
    .unwrap();
    // Folder-scoped UIDs collide across mailboxes, so both uid-1 rows survive;
    // the un-searched folder does not.
    assert_eq!(
        hits.iter()
            .map(|m| (m.folder.as_str(), m.uid))
            .collect::<Vec<_>>(),
        vec![("Sent", 1), ("INBOX", 1)]
    );
}

#[test]
fn search_messages_in_folders_pages_across_equal_folder_scoped_uids() {
    let conn = test_conn();
    for folder in ["INBOX", "Sent"] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date)
             VALUES('acct', ?1, '1', 1, 'deploy notes', 'Ops', 'ops@example.com', 100)",
            params![folder],
        )
        .unwrap();
    }

    let folders = ["INBOX".to_string(), "Sent".to_string()];
    let first = search_messages_in_folders(&conn, "acct", &folders, "deploy", 1, None).unwrap();
    assert_eq!(first[0].folder, "Sent");

    let cursor = crate::thread_list::parse_search_cursor(
        &search_next_cursor(&first, 1, 1).expect("full page has a cursor"),
    )
    .unwrap();
    let second =
        search_messages_in_folders(&conn, "acct", &folders, "deploy", 1, Some(&cursor)).unwrap();
    assert_eq!(second[0].folder, "INBOX");
}

#[test]
fn search_snapshot_pages_in_resolved_date_order_when_uids_diverge() {
    let conn = test_conn();
    for (uid, date) in [(3u32, 100i64), (2, 300), (1, 200)] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date)
             VALUES('acct', 'INBOX', ?1, ?2, 'deploy notes', 'Ops', 'ops@example.com', ?3)",
            params![uid.to_string(), uid, date],
        )
        .unwrap();
    }
    let folders = ["INBOX".to_string()];
    let mut hits =
        search_messages_in_folders(&conn, "acct", &folders, "deploy", u32::MAX, None).unwrap();
    sort_search_hits_all(&mut hits);
    assert_eq!(
        hits.iter().map(|message| message.uid).collect::<Vec<_>>(),
        vec![2, 1, 3]
    );

    let token = create_search_snapshot(&conn, "acct", "deploy", &folders, &[]).unwrap();
    finish_search_batch(&conn, &token, "acct", &[], &hits, None).unwrap();
    let first = get_search_snapshot_page(&conn, "acct", "deploy", &folders, &token, 0, 1)
        .unwrap()
        .unwrap();
    assert_eq!(first.messages[0].uid, 2);
    assert!(first.has_more);

    let second = get_search_snapshot_page(
        &conn,
        "acct",
        "deploy",
        &folders,
        &token,
        first.next_offset,
        1,
    )
    .unwrap()
    .unwrap();
    assert_eq!(second.messages[0].uid, 1);
    assert!(
        get_search_snapshot_page(&conn, "acct", "different", &folders, &token, 0, 1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn search_pending_is_taken_newest_first_by_date_across_folders() {
    let conn = test_conn();
    let hit = |folder: &str, uid: u32, date: i64| PendingSearchHit {
        folder: folder.to_string(),
        uid,
        date,
    };
    let folders = ["INBOX".to_string(), "Sent".to_string()];
    // UID 1 was imported late: the lowest UID carries the newest date.
    let token = create_search_snapshot(
        &conn,
        "acct",
        "deploy",
        &folders,
        &[
            hit("INBOX", 3, 300),
            hit("INBOX", 2, 200),
            hit("INBOX", 1, 1000),
            hit("Sent", 9, 250),
        ],
    )
    .unwrap();

    let (batch, next) = take_search_pending(&conn, &token, 2).unwrap();
    assert_eq!(batch, vec![hit("INBOX", 1, 1000), hit("INBOX", 3, 300)]);
    assert_eq!(next, Some(hit("Sent", 9, 250)));
    let (batch, next) = take_search_pending(&conn, &token, 4).unwrap();
    assert_eq!(batch.len(), 4);
    assert_eq!(next, None);
}

#[test]
fn search_batches_consume_pending_hits_and_append_in_order() {
    let conn = test_conn();
    for (uid, date) in [(5u32, 500i64), (4, 400), (3, 300)] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date)
             VALUES('acct', 'INBOX', ?1, ?2, 'deploy notes', 'Ops', 'ops@example.com', ?3)",
            params![uid.to_string(), uid, date],
        )
        .unwrap();
    }
    let folders = ["INBOX".to_string()];
    let pending = [(5u32, 500i64), (4, 400), (3, 300)].map(|(uid, date)| PendingSearchHit {
        folder: "INBOX".to_string(),
        uid,
        date,
    });
    let token = create_search_snapshot(&conn, "acct", "deploy", &folders, &pending).unwrap();
    let header = |uid: u32, date: i64| MessageHeader {
        uid,
        date,
        folder: "INBOX".to_string(),
        ..Default::default()
    };

    let (batch, next) = take_search_pending(&conn, &token, 2).unwrap();
    assert_eq!(
        batch.iter().map(|hit| hit.uid).collect::<Vec<_>>(),
        vec![5, 4]
    );
    assert_eq!(next.map(|hit| hit.uid), Some(3));
    finish_search_batch(
        &conn,
        &token,
        "acct",
        &[("INBOX".to_string(), 5), ("INBOX".to_string(), 4)],
        &[header(5, 500), header(4, 400)],
        None,
    )
    .unwrap();
    let page = get_search_snapshot_page(&conn, "acct", "deploy", &folders, &token, 0, 10)
        .unwrap()
        .unwrap();
    assert_eq!(
        page.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![5, 4]
    );
    assert!(page.pending);

    // The last batch appends after the first and leaves nothing pending; a hit
    // already listed is not appended twice.
    finish_search_batch(
        &conn,
        &token,
        "acct",
        &[("INBOX".to_string(), 3)],
        &[header(4, 400), header(3, 300)],
        None,
    )
    .unwrap();
    let page = get_search_snapshot_page(&conn, "acct", "deploy", &folders, &token, 0, 10)
        .unwrap()
        .unwrap();
    assert_eq!(
        page.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![5, 4, 3]
    );
    assert!(!page.pending);
    assert_eq!(
        take_search_pending(&conn, &token, 2).unwrap(),
        (vec![], None)
    );
}

#[test]
fn search_messages_returns_empty_for_blank_query() {
    let conn = test_conn();
    insert_message(&conn, 1, "Hello", "Aki", "aki@example.com", None);

    let results = search_messages(&conn, "acct", "INBOX", "  ", 10, None).unwrap();
    assert!(results.is_empty());
}
