use super::*;

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
fn get_starred_all_accounts_budgets_threads_not_rows() {
    let conn = test_conn();
    let insert = |folder: &str, uid: u32, thread_key: &str, date: i64| {
        conn.execute(
                "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key)
                 VALUES('a1', ?1, ?2, ?3, 'Subject', 'Aki', 'aki@example.com', ?4, 1, 1, ?5)",
                params![folder, format!("{folder}-{uid}"), uid, date, thread_key],
            )
            .unwrap();
    };
    // Two conversations, each cached in Inbox and All Mail the way Gmail files
    // them, plus an older third conversation.
    for (folder, uid_base) in [("INBOX", 10), ("All Mail", 20)] {
        insert(folder, uid_base + 1, "newest@example.com", 300);
        insert(folder, uid_base + 2, "middle@example.com", 200);
        insert(folder, uid_base + 3, "oldest@example.com", 100);
    }

    // A budget of two threads spends nothing on the duplicate folder copies: it
    // reaches back through both copies of the two newest conversations.
    let starred = get_starred_all_accounts(&conn, 2).unwrap();
    let mut keys = starred
        .iter()
        .map(|(_, m)| m.thread_key.as_str())
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys, vec!["middle@example.com", "newest@example.com"]);
    assert_eq!(starred.len(), 4);

    // Room for every conversation still returns every copy of each.
    assert_eq!(get_starred_all_accounts(&conn, 3).unwrap().len(), 6);
}

#[test]
fn get_starred_all_accounts_counts_unthreaded_messages_per_folder() {
    let conn = test_conn();
    // No thread_key at all: these fall back to "uid:<uid>", and a UID only means
    // something inside its own mailbox, so these are two conversations.
    for (folder, date) in [("INBOX", 200), ("Archive", 100)] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred)
             VALUES('a1', ?1, ?2, 1, 'Subject', 'Aki', 'aki@example.com', ?3, 1, 1)",
            params![folder, format!("{folder}-1"), date],
        )
        .unwrap();
    }

    let one = get_starred_all_accounts(&conn, 1).unwrap();
    assert_eq!(
        one.iter()
            .map(|(_, m)| m.folder.as_str())
            .collect::<Vec<_>>(),
        vec!["INBOX"]
    );
    let both = get_starred_all_accounts(&conn, 2).unwrap();
    assert_eq!(both.len(), 2);
}

#[test]
fn get_starred_all_accounts_counts_subject_branches_as_separate_conversations() {
    let conn = test_conn();
    // One root thread key that branched by subject: the list shows a card per
    // branch, so the budget has to count them the same way.
    for (uid, subject, date) in [(1, "Sprint planning", 300), (2, "Lunch orders", 200)] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key)
             VALUES('a1', 'INBOX', ?1, ?2, ?3, 'Aki', 'aki@example.com', ?4, 1, 1, 'root@example.com')",
            params![format!("m-{uid}"), uid, subject, date],
        )
        .unwrap();
    }

    let one = get_starred_all_accounts(&conn, 1).unwrap();
    assert_eq!(
        one.iter()
            .map(|(_, m)| m.subject.as_str())
            .collect::<Vec<_>>(),
        vec!["Sprint planning"]
    );
    assert_eq!(get_starred_all_accounts(&conn, 2).unwrap().len(), 2);
}

#[test]
fn get_starred_all_accounts_keeps_copies_of_admitted_threads_past_the_budget() {
    let conn = test_conn();
    let insert = |folder: &str, uid: u32, thread_key: &str, date: i64| {
        conn.execute(
                "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, starred, thread_key)
                 VALUES('a1', ?1, ?2, ?3, 'Subject', 'Aki', 'aki@example.com', ?4, 1, 1, ?5)",
                params![folder, format!("{folder}-{uid}"), uid, date, thread_key],
            )
            .unwrap();
    };
    // Two folders' copies of one message can hold different cached dates, which
    // puts an over-budget conversation between them in this ordering.
    insert("All Mail", 1, "kept@example.com", 300);
    insert("INBOX", 2, "over-budget@example.com", 200);
    insert("INBOX", 3, "kept@example.com", 100);

    let starred = get_starred_all_accounts(&conn, 1).unwrap();
    assert!(
        starred
            .iter()
            .all(|(_, m)| m.thread_key == "kept@example.com"),
        "over-budget conversation must not be admitted"
    );
    // Both copies of the admitted conversation come back: the caller chooses
    // which folder's card to show from this set, and dropping the Inbox copy
    // here would file the thread under All Mail instead.
    let mut folders = starred
        .iter()
        .map(|(_, m)| m.folder.as_str())
        .collect::<Vec<_>>();
    folders.sort_unstable();
    assert_eq!(folders, vec!["All Mail", "INBOX"]);
}

#[test]
fn card_message_counts_span_the_folder_not_the_page() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let insert = |uid: u32, subject: &str, seen: i64| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                                  date, seen, thread_key)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, 'Aki', 'aki@example.com', 1779580800, ?4,
                    'root@example.com')",
            params![uid.to_string(), uid, subject, seen],
        )
        .unwrap();
    };
    insert(1, "Old topic", 1);
    insert(2, "Re: Old topic", 1);
    insert(3, "Re: Old topic", 0);
    insert(4, "New topic", 0);

    let card_key = |uid: u32, subject: &str| {
        card_thread_key(&MessageHeader {
            uid,
            subject: subject.to_string(),
            thread_key: "root@example.com".to_string(),
            ..Default::default()
        })
    };
    let old_key = card_key(1, "Old topic");
    let new_key = card_key(4, "New topic");

    // The unread view hands grouping only the two unread messages, so the cards
    // it produces tally one message each.
    let (page, _) = get_recent_page(&conn, "acct", "INBOX", 50, None, UNREAD).unwrap();
    let cards = group_thread_cards(page, "INBOX");
    let keys = cards
        .iter()
        .map(|card| card.thread_key.clone())
        .collect::<Vec<_>>();
    assert!(cards.iter().all(|card| card.message_count == 1));

    // The cache knows better: the old branch holds three messages (one unread),
    // and the drifted subject is counted as its own branch, not folded in.
    let counts = card_message_counts(&conn, "acct", "INBOX", &keys).unwrap();
    assert_eq!(counts.get(&old_key).copied(), Some(3));
    assert_eq!(counts.get(&new_key).copied(), Some(1));
}

#[test]
fn card_message_counts_span_folders_and_dedupe_self_sent_copies() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let insert = |folder: &str, uid: u32, subject: &str, thread_key: &str, message_id: &str| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                                  date, seen, thread_key, json)
             VALUES('acct', ?1, ?2, ?3, ?4, 'Aki', 'aki@example.com', 1779580800, 1, ?5,
                    json_object('message_id', ?6))",
            params![
                folder,
                format!("{folder}-{uid}"),
                uid,
                subject,
                thread_key,
                message_id
            ],
        )
        .unwrap();
    };
    // A received message and the reply the user sent back: one thread, two
    // folders. The Sent reply is also cached in All Mail, as Gmail serves it.
    insert(
        "INBOX",
        1,
        "Old topic",
        "root@example.com",
        "<a@example.com>",
    );
    insert(
        "Sent",
        7,
        "Re: Old topic",
        "root@example.com",
        "<b@example.com>",
    );
    insert(
        "All Mail",
        9,
        "Re: Old topic",
        "root@example.com",
        "<b@example.com>",
    );
    insert(
        "INBOX",
        10,
        "Gmail topic",
        "gmthrid:123",
        "<gmail-a@example.com>",
    );
    insert(
        "Sent",
        11,
        "Different Gmail subject",
        "gmthrid:123",
        "<gmail-b@example.com>",
    );
    // Same UID, different folder, no threading headers of its own.
    insert("INBOX", 4, "Standalone", "", "<c@example.com>");
    insert("Archive", 4, "Unrelated", "", "<d@example.com>");

    let threaded = card_thread_key(&MessageHeader {
        uid: 1,
        subject: "Old topic".to_string(),
        thread_key: "root@example.com".to_string(),
        ..Default::default()
    });
    let gmail_threaded = "gmthrid:123".to_string();
    let counts = card_message_counts(
        &conn,
        "acct",
        "INBOX",
        &[
            threaded.clone(),
            gmail_threaded.clone(),
            "uid:4".to_string(),
        ],
    )
    .unwrap();

    // Two bubbles open in the reader, so the row says two: the All Mail copy of
    // the Sent reply is folded by Message-ID, not counted twice.
    assert_eq!(counts.get(&threaded).copied(), Some(2));
    // Gmail's atomic thread key also spans folders even though it does not
    // branch by subject: only synthetic uid: keys are folder-local.
    assert_eq!(counts.get(&gmail_threaded).copied(), Some(2));
    // A synthetic uid: key stays folder-local; the unrelated Archive row that
    // happens to share UID 4 is not part of this thread.
    assert_eq!(counts.get("uid:4").copied(), Some(1));
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
    let (all, all_cursor) =
        get_recent_page(&conn, "acct", "INBOX", 2, None, RecentFilter::default()).unwrap();
    assert_eq!(all.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![5, 4]);
    assert_eq!(all_cursor.as_deref(), Some(format!("date:{D}:4").as_str()));

    let (unread, unread_cursor) = get_recent_page(&conn, "acct", "INBOX", 2, None, UNREAD).unwrap();
    assert_eq!(unread.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![5, 3]);
    assert_eq!(
        unread_cursor.as_deref(),
        Some(format!("date:{D}:3").as_str())
    );
    assert!(unread.iter().all(|m| !m.seen));

    let (next_unread, next_cursor) =
        get_recent_page(&conn, "acct", "INBOX", 2, Some((D, 3)), UNREAD).unwrap();
    assert_eq!(
        next_unread.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(next_cursor, None);
}

#[test]
fn new_unread_inbox_messages_counts_uid_window_and_latest_unread() {
    let conn = test_conn();
    let inbox_messages = vec![
        MessageHeader {
            uid: 1,
            subject: "Before window".to_string(),
            from_addr: "old@example.com".to_string(),
            thread_key: "old".to_string(),
            ..Default::default()
        },
        MessageHeader {
            uid: 2,
            subject: "Lower bound".to_string(),
            from_addr: "lower@example.com".to_string(),
            thread_key: String::new(),
            ..Default::default()
        },
        MessageHeader {
            uid: 3,
            subject: "Already read".to_string(),
            from_addr: "read@example.com".to_string(),
            seen: true,
            thread_key: "read".to_string(),
            ..Default::default()
        },
        MessageHeader {
            uid: 4,
            subject: "Latest unread".to_string(),
            from_name: "Aki".to_string(),
            from_addr: "aki@example.com".to_string(),
            thread_key: "fresh".to_string(),
            ..Default::default()
        },
        MessageHeader {
            uid: 5,
            subject: "Upper bound excluded".to_string(),
            from_addr: "upper@example.com".to_string(),
            thread_key: "upper".to_string(),
            ..Default::default()
        },
    ];
    let arrivals = classify_inbox_arrivals(&conn, "acct", 2, 5, &inbox_messages).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &inbox_messages).unwrap();
    upsert_messages(
        &conn,
        "acct",
        "Archive",
        &[MessageHeader {
            uid: 4,
            subject: "Wrong folder".to_string(),
            from_addr: "archive@example.com".to_string(),
            thread_key: "archive".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();

    assert_eq!(arrivals.len(), 2);
    let latest = &arrivals[0];
    assert_eq!(latest.uid, 4);
    assert_eq!(latest.subject, "Latest unread");
    assert_eq!(latest.thread_key, "fresh");

    let conn = test_conn();
    let lower = vec![MessageHeader {
        uid: 2,
        subject: "Lower bound".to_string(),
        from_addr: "lower@example.com".to_string(),
        thread_key: String::new(),
        ..Default::default()
    }];
    let lower_bound = classify_inbox_arrivals(&conn, "acct", 2, 3, &lower)
        .unwrap()
        .remove(0);
    assert_eq!(lower_bound.uid, 2);
    assert_eq!(lower_bound.thread_key, "");
}

#[test]
fn cached_body_preview_collapses_body_and_misses_without_a_cached_body() {
    let conn = test_conn();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "Lunch".to_string(),
            from_addr: "aki@example.com".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();

    // Header only: no snippet to show yet.
    assert!(cached_body_preview(&conn, "acct", "INBOX", 7).is_none());

    save_cached_message(
        &conn,
        "acct",
        "INBOX",
        7,
        &Message {
            subject: "Lunch".into(),
            body: "  Are we still on\n\n  for lunch?  ".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        cached_body_preview(&conn, "acct", "INBOX", 7).unwrap(),
        "Are we still on for lunch?"
    );

    // A whitespace-only body is a miss, not an empty snippet line.
    save_cached_message(
        &conn,
        "acct",
        "INBOX",
        7,
        &Message {
            subject: "Lunch".into(),
            body: "   \n  ".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(cached_body_preview(&conn, "acct", "INBOX", 7).is_none());
}

#[test]
fn a_sync_snapshot_from_before_a_move_cannot_put_the_message_back() {
    let conn = test_conn();
    let message = MessageHeader {
        uid: 7,
        subject: "Archive me".into(),
        from_addr: "a@example.com".into(),
        thread_key: "topic".into(),
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[message.clone()]).unwrap();
    // What a folder sync reads before it goes to the network.
    let fetched_at = removed_message_epoch(&conn).unwrap();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[7]).unwrap();

    // The snapshot still contains the message, and it started before the move.
    release_removed_messages(&conn, "acct", "INBOX", &[7], fetched_at).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[message.clone()]).unwrap();
    assert!(!has_message(&conn, "acct", "INBOX", 7).unwrap());

    // A later sync really does see the UID in the folder again (moved back).
    let later = removed_message_epoch(&conn).unwrap();
    release_removed_messages(&conn, "acct", "INBOX", &[7], later).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[message]).unwrap();
    assert!(has_message(&conn, "acct", "INBOX", 7).unwrap());
}

#[test]
fn a_late_body_fetch_cannot_reinsert_a_removed_message() {
    let conn = test_conn();
    let header = MessageHeader {
        uid: 7,
        subject: "Archive me".into(),
        ..Default::default()
    };
    let body = Message {
        subject: "Archive me".into(),
        body: "Late body".into(),
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[header.clone()]).unwrap();
    let (body_fetch, _) = begin_message_sync(&conn).unwrap();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[7]).unwrap();
    // An unrelated fresh sync must not clean up the marker while the body fetch
    // still owns an older snapshot.
    let (other_sync, _) = begin_message_sync(&conn).unwrap();
    end_message_sync(&conn, other_sync).unwrap();
    // fill_thread_gaps writes the header and the body in succession.
    upsert_messages(&conn, "acct", "INBOX", &[header.clone()]).unwrap();
    save_cached_message(&conn, "acct", "INBOX", 7, &body).unwrap();
    assert!(!has_message(&conn, "acct", "INBOX", 7).unwrap());
    assert!(
        get_cached_message(&conn, "acct", "INBOX", 7)
            .unwrap()
            .is_none()
    );
    end_message_sync(&conn, body_fetch).unwrap();
    // A later valid fetch can still cache a body normally.
    upsert_messages(&conn, "acct", "INBOX", &[header]).unwrap();
    save_cached_message(&conn, "acct", "INBOX", 7, &body).unwrap();
    assert_eq!(
        get_cached_message(&conn, "acct", "INBOX", 7)
            .unwrap()
            .unwrap()
            .body,
        "Late body"
    );
}

#[test]
fn removed_uid_markers_expire_after_the_oldest_sync_finishes() {
    let conn = test_conn();
    let count = || -> i64 {
        conn.query_row("SELECT count(*) FROM removed_message_uids", [], |r| {
            r.get(0)
        })
        .unwrap()
    };
    let (first, _) = begin_message_sync(&conn).unwrap();
    let (same_epoch, _) = begin_message_sync(&conn).unwrap();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[7]).unwrap();
    let (later, _) = begin_message_sync(&conn).unwrap();
    assert_eq!(count(), 1);
    end_message_sync(&conn, first).unwrap();
    assert_eq!(count(), 1, "another old snapshot still needs the marker");
    end_message_sync(&conn, same_epoch).unwrap();
    assert_eq!(count(), 0, "the newer sync cannot contain the old UID");
    delete_messages_by_uid(&conn, "acct", "INBOX", &[8]).unwrap();
    assert_eq!(count(), 1);
    end_message_sync(&conn, later).unwrap();
    assert_eq!(count(), 0, "no active snapshots remain");
}

#[test]
fn another_connection_cannot_prune_markers_an_active_sync_needs() {
    let dir = std::env::temp_dir().join(format!("meron-store-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("meron.db");
    let syncing = db::open_at(&path).unwrap();
    let archiving = db::open_at(&path).unwrap();
    let count = || -> i64 {
        syncing
            .query_row("SELECT count(*) FROM removed_message_uids", [], |r| {
                r.get(0)
            })
            .unwrap()
    };
    let (stale, _) = begin_message_sync(&syncing).unwrap();
    delete_messages_by_uid(&archiving, "acct", "INBOX", &[7]).unwrap();
    let (other, _) = begin_message_sync(&archiving).unwrap();
    end_message_sync(&archiving, other).unwrap();
    assert_eq!(count(), 1, "the other connection's snapshot is still open");
    end_message_sync(&syncing, stale).unwrap();
    assert_eq!(count(), 0);

    // A snapshot left behind by a dead process does not pin markers.
    archiving
        .execute(
            "INSERT INTO active_message_syncs(epoch, owner) VALUES(0, 'dead')",
            [],
        )
        .unwrap();
    delete_messages_by_uid(&archiving, "acct", "INBOX", &[8]).unwrap();
    let (id, _) = begin_message_sync(&syncing).unwrap();
    assert_eq!(count(), 0);
    end_message_sync(&syncing, id).unwrap();

    drop(syncing);
    drop(archiving);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn concurrent_sync_registration_waits_instead_of_failing() {
    let dir = std::env::temp_dir().join(format!("meron-store-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("meron.db");
    db::open_at(&path).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let handles = (0..4u32)
        .map(|worker| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let conn = db::open_at(&path).unwrap();
                barrier.wait();
                for round in 0..50 {
                    let (id, _) = begin_message_sync(&conn).unwrap();
                    delete_messages_by_uid(&conn, "acct", "INBOX", &[worker * 100 + round + 1])
                        .unwrap();
                    end_message_sync(&conn, id).unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_new_sync_purges_removal_markers_left_without_active_snapshots() {
    let conn = test_conn();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[7, 8, 9]).unwrap();
    let (id, epoch) = begin_message_sync(&conn).unwrap();
    assert!(epoch > 0);
    let count: i64 = conn
        .query_row("SELECT count(*) FROM removed_message_uids", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
    end_message_sync(&conn, id).unwrap();
}

#[test]
fn cached_archive_identity_suppresses_inbox_arrival_and_survives_deletion() {
    let conn = test_conn();
    let mut message = MessageHeader {
        uid: 5,
        message_id: "known@example.com".into(),
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "Archive", &[message.clone()]).unwrap();
    delete_messages_by_uid(&conn, "acct", "Archive", &[5]).unwrap();
    message.uid = 20;
    assert!(
        classify_inbox_arrivals(&conn, "acct", 20, 21, &[message])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn custom_folder_arrivals_skip_mail_already_seen_elsewhere() {
    let conn = test_conn();
    let filed = MessageHeader {
        uid: 7,
        message_id: "filed@example.com".into(),
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[filed.clone()]).unwrap();
    // The user moved `filed` out of INBOX; a server rule delivered `direct`.
    let batch = [
        MessageHeader { uid: 20, ..filed },
        MessageHeader {
            uid: 21,
            message_id: "direct@example.com".into(),
            ..Default::default()
        },
    ];
    let arrivals = classify_arrivals(&conn, "acct", "Work", 20, 22, &batch).unwrap();
    assert_eq!(arrivals.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![21]);
}

#[test]
fn classified_arrivals_survive_companion_writes_and_later_inbox_sync() {
    let conn = test_conn();
    let arrival = MessageHeader {
        uid: 20,
        message_id: "arrival@example.com".into(),
        thread_key: "arrival@example.com".into(),
        ..Default::default()
    };
    let notified = classify_inbox_arrivals(&conn, "acct", 20, 21, &[arrival.clone()]).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[arrival.clone()]).unwrap();
    for folder in ["Sent", "Drafts"] {
        upsert_messages(
            &conn,
            "acct",
            folder,
            &[MessageHeader {
                uid: 1,
                message_id: format!("old-{folder}@example.com"),
                ..Default::default()
            }],
        )
        .unwrap();
    }
    assert!(
        classify_inbox_arrivals(&conn, "acct", 20, 21, &[arrival])
            .unwrap()
            .is_empty()
    );
    assert_eq!(notified.len(), 1);
    assert_eq!(notified[0].uid, 20);
}

#[test]
fn new_unread_inbox_messages_ignores_empty_or_non_growing_windows() {
    let conn = test_conn();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Unread".to_string(),
            from_addr: "new@example.com".to_string(),
            thread_key: "new".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();

    assert!(
        new_unread_inbox_messages(&conn, "acct", 0, 2, &[])
            .unwrap()
            .is_none()
    );
    assert!(
        new_unread_inbox_messages(&conn, "acct", 2, 2, &[])
            .unwrap()
            .is_none()
    );
    assert!(
        new_unread_inbox_messages(&conn, "acct", 3, 2, &[])
            .unwrap()
            .is_none()
    );
}

#[test]
fn new_unread_inbox_messages_ignores_observed_gmail_message_restored_with_new_uid() {
    let conn = test_conn();
    let old = vec![MessageHeader {
        uid: 10,
        subject: "Old unread".to_string(),
        from_addr: "old@example.com".to_string(),
        thread_key: "gmthrid:1".to_string(),
        gmail_msg_id: Some(999),
        ..Default::default()
    }];
    upsert_messages(&conn, "acct", "INBOX", &old).unwrap();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[10]).unwrap();

    let restored = vec![MessageHeader {
        uid: 20,
        subject: "Old unread".to_string(),
        from_addr: "old@example.com".to_string(),
        thread_key: "gmthrid:1".to_string(),
        gmail_msg_id: Some(999),
        ..Default::default()
    }];
    assert!(
        classify_inbox_arrivals(&conn, "acct", 20, 21, &restored)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn new_unread_inbox_messages_ignores_observed_message_id_restored_with_new_uid() {
    let conn = test_conn();
    let old = vec![MessageHeader {
        uid: 10,
        subject: "Old unread".to_string(),
        from_addr: "old@example.com".to_string(),
        thread_key: "mid@example.com".to_string(),
        message_id: "Mid@Example.com".to_string(),
        ..Default::default()
    }];
    upsert_messages(&conn, "acct", "INBOX", &old).unwrap();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[10]).unwrap();

    let restored = vec![MessageHeader {
        uid: 20,
        subject: "Old unread".to_string(),
        from_addr: "old@example.com".to_string(),
        thread_key: "mid@example.com".to_string(),
        message_id: "mid@example.com".to_string(),
        ..Default::default()
    }];
    assert!(
        classify_inbox_arrivals(&conn, "acct", 20, 21, &restored)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn resolve_message_uids_prefers_explicit_then_single_then_thread() {
    let conn = test_conn();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[
            MessageHeader {
                uid: 10,
                subject: "Root".to_string(),
                from_addr: "root@example.com".to_string(),
                thread_key: "thread-a".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 11,
                subject: "Reply".to_string(),
                from_addr: "reply@example.com".to_string(),
                thread_key: "thread-a".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();

    assert_eq!(
        resolve_message_uids(
            &conn,
            "acct",
            "INBOX",
            "thread-a",
            None,
            Some(10),
            &[42, 43]
        )
        .unwrap(),
        vec![42, 43]
    );
    assert_eq!(
        resolve_message_uids(&conn, "acct", "INBOX", "", None, Some(10), &[]).unwrap(),
        vec![10]
    );
    assert_eq!(
        resolve_message_uids(&conn, "acct", "INBOX", "", None, None, &[]).unwrap(),
        Vec::<u32>::new()
    );
    assert_eq!(
        resolve_message_uids(&conn, "acct", "INBOX", "thread-a", None, None, &[]).unwrap(),
        vec![10, 11]
    );
}

#[test]
fn thread_subject_normalization_matches_desktop_prefix_semantics() {
    // Parity with the Go subjectPrefixRegex alternation in mailjson.go: every
    // listed prefix must strip when followed by a colon, including the ones
    // that share a shorter list entry ("fwd" behind "fw", "res" behind "re").
    let cases = [
        ("Re: Topic", "Topic"),
        ("Fwd: Topic", "Topic"),
        ("FWD: Topic", "Topic"),
        ("Res: Topic", "Topic"),
        ("Re[2]: Topic", "Topic"),
        ("FW(3): Topic", "Topic"),
        ("回复：主题", "主题"),
        ("Re: Fwd: Topic", "Topic"),
        // Not reply prefixes: the colon check must reject these.
        ("Ready: set", "Ready: set"),
        ("Fwdish: nope", "Fwdish: nope"),
        ("Topic", "Topic"),
    ];
    for (subject, expected) in cases {
        assert_eq!(thread_grouping_subject(subject), expected, "{subject}");
    }
    // Leading bracket tags drop too.
    assert_eq!(thread_grouping_subject("[EXTERNAL] Re: Topic"), "Topic");
    assert_eq!(thread_grouping_subject("[JIRA-1] Topic"), "Topic");
}

#[test]
fn branch_compound_key_round_trips_hash_and_percent_in_root() {
    let cases = ["abc#123@host", "plain@host", "50%25#done@host"];
    for root in cases {
        let compound = branch_compound_key(root, "Topic");
        assert_eq!(
            split_branch_compound_key(&compound),
            (root.to_string(), Some("Topic".to_string())),
            "{root}"
        );
    }
    // Unbranched legacy keys pass through verbatim.
    assert_eq!(
        split_branch_compound_key("plain@host"),
        ("plain@host".to_string(), None)
    );
}
