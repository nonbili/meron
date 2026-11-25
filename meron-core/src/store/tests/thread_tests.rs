use super::*;

#[test]
fn group_thread_cards_branches_subject_drift_and_links_to_root() {
    use crate::imap::MessageHeader;
    let messages = vec![
        MessageHeader {
            uid: 3,
            subject: "New topic".to_string(),
            from_name: "Newest".to_string(),
            from_addr: "new@example.com".to_string(),
            date: 300,
            seen: false,
            thread_key: "refs-root".to_string(),
            ..Default::default()
        },
        MessageHeader {
            uid: 2,
            subject: "Re: Old topic".to_string(),
            from_name: "Reply".to_string(),
            from_addr: "reply@example.com".to_string(),
            date: 200,
            seen: true,
            thread_key: "refs-root".to_string(),
            ..Default::default()
        },
        MessageHeader {
            uid: 1,
            subject: "Old topic".to_string(),
            from_name: "Root".to_string(),
            from_addr: "root@example.com".to_string(),
            date: 100,
            seen: false,
            thread_key: "refs-root".to_string(),
            ..Default::default()
        },
    ];

    let cards = group_thread_cards(messages, "INBOX");
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].thread_key, "refs-root#New topic");
    assert_eq!(
        cards[0].original_thread_key.as_deref(),
        Some("refs-root#Old topic")
    );
    assert_eq!(cards[0].header.subject, "Old topic");
    assert_eq!(cards[0].unread_count, 1);
    // Counts follow the branch, not the root thread key: the drifted subject
    // stands alone while the original keeps its reply.
    assert_eq!(cards[0].message_count, 1);
    assert!(!cards[0].header.seen);

    assert_eq!(cards[1].thread_key, "refs-root#Old topic");
    assert_eq!(cards[1].original_thread_key, None);
    assert_eq!(cards[1].header.subject, "Old topic");
    assert_eq!(cards[1].unread_count, 1);
    assert_eq!(cards[1].message_count, 2);
}

#[test]
fn group_thread_cards_keep_reply_prefix_when_the_thread_start_is_missing() {
    use crate::imap::MessageHeader;
    // Only replies reached this mailbox, so the oldest message here is itself a
    // reply. The card must say so rather than pass the reply off as the thread's
    // opening message.
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 2,
                subject: "Re: abdullah <> ping".to_string(),
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                subject: "Re: abdullah <> ping".to_string(),
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.subject, "Re: abdullah <> ping");
    // Grouping still ignores the prefix, so the branch key is the bare subject.
    assert_eq!(cards[0].thread_key, "refs-root#abdullah <> ping");
    assert_eq!(cards[0].message_count, 2);
}

#[test]
fn group_thread_cards_pick_the_root_by_date_across_folders() {
    use crate::imap::MessageHeader;
    // A folder-spanning page (search, or any thread the user replied to) merges
    // Inbox and Sent, whose UID spaces are unrelated: the Sent reply's UID 1 is
    // not older than the Inbox root's UID 100. Send time is what orders them.
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 1,
                folder: "Sent".to_string(),
                subject: "Re: Topic".to_string(),
                date: 200,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 100,
                folder: "INBOX".to_string(),
                subject: "Topic".to_string(),
                date: 100,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.subject, "Topic");
}

#[test]
fn group_thread_cards_break_a_cross_folder_tie_with_the_read_mailbox() {
    use crate::imap::MessageHeader;
    // Neither copy carries a date, so send time cannot separate them. UID must
    // not step in — it is folder-local, and the Sent reply's UID 1 would win
    // against the Inbox root's UID 100. The mailbox being read owns the card.
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 1,
                folder: "Sent".to_string(),
                subject: "Re: Topic".to_string(),
                date: 0,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 100,
                folder: "INBOX".to_string(),
                subject: "Topic".to_string(),
                date: 0,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.subject, "Topic");
}

#[test]
fn group_thread_cards_keep_uid_order_within_a_folder_when_a_date_is_missing() {
    use crate::imap::MessageHeader;
    // `date == 0` means unknown, not "epoch". Inside one folder UIDs already
    // order by arrival, so the undated root stays the root.
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 2,
                subject: "Re: Topic".to_string(),
                date: 100,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                subject: "Topic".to_string(),
                date: 0,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.subject, "Topic");
}

#[test]
fn card_root_subjects_read_the_thread_start_the_page_left_out() {
    let conn = test_conn();
    let insert = |uid: u32, subject: &str, seen: i64| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                                  date, seen, thread_key)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, 'Aki', 'aki@example.com', ?4, ?5,
                    'root@example.com')",
            params![uid.to_string(), uid, subject, 100 * uid as i64, seen],
        )
        .unwrap();
    };
    insert(1, "Topic", 1);
    insert(2, "Re: Topic", 0);

    // The unread view's page holds only the reply, so grouping alone would title
    // the card "Re: Topic". The cache still has the seen thread start.
    let subjects = card_root_subjects(
        &conn,
        "acct",
        "INBOX",
        &["root@example.com#Topic".to_string()],
    )
    .unwrap();
    assert_eq!(
        subjects.get("root@example.com").map(String::as_str),
        Some("Topic")
    );

    // A thread the cache has never stored answers nothing, leaving the caller
    // its own page-derived title.
    assert!(
        card_root_subjects(
            &conn,
            "acct",
            "INBOX",
            &["missing@example.com#Topic".to_string()]
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn group_thread_cards_keeps_uid_and_gmail_threads_atomic() {
    use crate::imap::MessageHeader;
    let uid_cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 2,
                subject: "Other".to_string(),
                thread_key: "uid:2".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                subject: "Root".to_string(),
                thread_key: "uid:2".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(uid_cards.len(), 1);
    assert_eq!(uid_cards[0].thread_key, "uid:2");
    // Read messages count too; the badge is thread size, not unread size.
    assert_eq!(uid_cards[0].message_count, 2);

    let gmail_cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 2,
                subject: "Other".to_string(),
                thread_key: "gmthrid:abc".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                subject: "Root".to_string(),
                thread_key: "gmthrid:abc".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(gmail_cards.len(), 1);
    assert_eq!(gmail_cards[0].thread_key, "gmthrid:abc");
    assert_eq!(gmail_cards[0].message_count, 2);
}

#[test]
fn group_thread_cards_accumulates_unread_and_starred_across_group() {
    use crate::imap::MessageHeader;
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 3,
                subject: "Re: [EXTERNAL] Topic".to_string(),
                seen: false,
                starred: false,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "[EXTERNAL] Topic".to_string(),
                seen: false,
                starred: true,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                subject: "Topic".to_string(),
                seen: true,
                starred: false,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].thread_key, "refs-root#Topic");
    assert_eq!(cards[0].header.subject, "Topic");
    assert_eq!(cards[0].unread_count, 2);
    assert!(cards[0].header.starred);
}

#[test]
fn group_thread_cards_marks_groups_with_cached_drafts() {
    use crate::imap::MessageHeader;
    let draft_keys = ["refs-root".to_string()].into_iter().collect();
    let cards = group_thread_cards_with_drafts(
        vec![
            MessageHeader {
                uid: 2,
                subject: "Re: Topic".to_string(),
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                subject: "Other".to_string(),
                thread_key: "other-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
        &draft_keys,
    );

    let marked = cards
        .iter()
        .find(|card| card.thread_key == "refs-root#Topic")
        .unwrap();
    let unmarked = cards
        .iter()
        .find(|card| card.thread_key == "other-root#Other")
        .unwrap();
    assert!(marked.has_draft);
    assert!(!unmarked.has_draft);
}

#[test]
fn group_thread_cards_count_a_self_sent_copy_once() {
    use crate::imap::MessageHeader;
    // A message the user sent to themselves is cached in both Sent and Inbox,
    // and a search page reads both. It is one message in the thread: the tally
    // the card carries — a floor under the cached count, which folds copies by
    // Message-ID — must not count it twice, or the search list shows a longer
    // thread than the mailbox's own list does.
    let copy = |folder: &str, uid: u32| MessageHeader {
        uid,
        folder: folder.to_string(),
        subject: "Re: Topic".to_string(),
        date: 300,
        seen: false,
        thread_key: "refs-root".to_string(),
        message_id: "<reply@example.com>".to_string(),
        ..Default::default()
    };
    let cards = group_thread_cards(
        vec![
            copy("Sent", 7),
            copy("INBOX", 9),
            MessageHeader {
                uid: 2,
                folder: "INBOX".to_string(),
                subject: "Topic".to_string(),
                date: 100,
                seen: true,
                thread_key: "refs-root".to_string(),
                message_id: "<root@example.com>".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].message_count, 2);
    assert_eq!(cards[0].unread_count, 1);
    assert!(!cards[0].header.seen);

    // The copies can disagree on the read flag: Sent's is read, the mailbox's is
    // not. The message is unread whichever copy the page lists first.
    for order in [
        [("Sent", 7, true), ("INBOX", 9, false)],
        [("INBOX", 9, false), ("Sent", 7, true)],
    ] {
        let page = order
            .into_iter()
            .map(|(folder, uid, seen)| MessageHeader {
                seen,
                ..copy(folder, uid)
            })
            .collect::<Vec<_>>();
        let cards = group_thread_cards(page, "INBOX");
        assert_eq!(cards[0].message_count, 1);
        assert_eq!(cards[0].unread_count, 1);
        assert!(!cards[0].header.seen);
    }

    // Without a Message-ID there is nothing to match a copy on, so each row
    // still counts for itself.
    let anonymous = |folder: &str, uid: u32| MessageHeader {
        uid,
        folder: folder.to_string(),
        subject: "Topic".to_string(),
        thread_key: "refs-root".to_string(),
        ..Default::default()
    };
    let cards = group_thread_cards(vec![anonymous("Sent", 7), anonymous("INBOX", 9)], "INBOX");
    assert_eq!(cards[0].message_count, 2);
}

#[test]
fn group_thread_cards_keep_the_searched_folder_when_a_page_spans_folders() {
    use crate::imap::MessageHeader;
    // A search page merges the mailbox with Sent, newest first. Replying puts
    // the Sent copy on top, and the card must still be the searched mailbox's
    // one: its thread id is what the list already has the thread under.
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 7,
                folder: "Sent".to_string(),
                subject: "Re: Topic".to_string(),
                date: 300,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                folder: "INBOX".to_string(),
                subject: "Topic".to_string(),
                date: 100,
                thread_key: "refs-root".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.folder, "INBOX");
    // Only the identity moves: the card still shows the newest message.
    assert_eq!(cards[0].header.date, 300);
    assert_eq!(cards[0].message_count, 2);

    // A thread the searched mailbox has no copy of stays where it was found.
    let sent_only = group_thread_cards(
        vec![MessageHeader {
            uid: 7,
            folder: "Sent".to_string(),
            subject: "Topic".to_string(),
            thread_key: "refs-root".to_string(),
            ..Default::default()
        }],
        "INBOX",
    );
    assert_eq!(sent_only[0].header.folder, "Sent");

    // `uid:` keys name one message by (folder, uid). Two mailboxes reuse UIDs, so
    // such a key is only unique within its own folder: unrelated messages that
    // share one stay separate cards, and neither card's folder may move.
    let uid_keyed = group_thread_cards(
        vec![
            MessageHeader {
                uid: 7,
                folder: "Sent".to_string(),
                subject: "Reply to somebody".to_string(),
                thread_key: "uid:7".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 7,
                folder: "INBOX".to_string(),
                subject: "Unrelated mail".to_string(),
                seen: false,
                thread_key: "uid:7".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(uid_keyed.len(), 2);
    assert_eq!(uid_keyed[0].header.folder, "Sent");
    assert_eq!(uid_keyed[0].message_count, 1);
    assert_eq!(uid_keyed[1].header.folder, "INBOX");
    assert_eq!(uid_keyed[1].unread_count, 1);
}

#[test]
fn newest_thread_uids_names_only_the_latest_message() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
         VALUES('acct', 'INBOX', '1', 1, 'Topic', 'Them', 'them@example.com', 100, 1, 'topic-key'),
               ('acct', 'INBOX', '2', 2, 'Topic', 'Them', 'them@example.com', 300, 1, 'topic-key'),
               ('acct', 'INBOX', '3', 3, 'Topic', 'Them', 'them@example.com', 200, 1, 'topic-key')",
        params![],
    )
    .unwrap();

    // Marking a thread unread flags this one message, so the thread comes back
    // as a single unread message rather than an all-unread thread.
    let uids = newest_thread_uids(&conn, "acct", "INBOX", "topic-key", None).unwrap();

    assert_eq!(uids, vec![2]);
}

#[test]
fn newest_thread_uids_stays_within_a_subject_branch() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
         VALUES('acct', 'INBOX', '1', 1, 'Invoice', 'Them', 'them@example.com', 100, 1, 'root'),
               ('acct', 'INBOX', '2', 2, 'Re: Receipt', 'Them', 'them@example.com', 400, 1, 'root'),
               ('acct', 'INBOX', '3', 3, 'Re: Invoice', 'Them', 'them@example.com', 300, 1, 'root')",
        params![],
    )
    .unwrap();

    let uids = newest_thread_uids(&conn, "acct", "INBOX", "root", Some("Invoice")).unwrap();

    assert_eq!(uids, vec![3]);
}

#[test]
fn thread_uids_span_folders_and_can_skip_read_messages() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
         VALUES('acct', 'INBOX', '1', 1, 'Topic', 'Them', 'them@example.com', 100, 0, 'topic-key'),
               ('acct', 'INBOX', '2', 2, 'Topic', 'Them', 'them@example.com', 200, 1, 'topic-key'),
               ('acct', 'Archive', '3', 7, 'Re: Topic', 'Them', 'them@example.com', 300, 0, 'topic-key'),
               ('acct', 'Archive', '4', 8, 'Other', 'Them', 'them@example.com', 400, 0, 'other-key')",
        params![],
    )
    .unwrap();

    let by_folder = thread_uids_by_folder(&conn, "acct", "INBOX", "topic-key", None, true).unwrap();

    assert_eq!(
        by_folder.into_iter().collect::<Vec<_>>(),
        vec![
            ("Archive".to_string(), vec![7]),
            ("INBOX".to_string(), vec![1])
        ]
    );
}

#[test]
fn thread_uids_keep_uid_keys_folder_local() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
         VALUES('acct', 'INBOX', '1', 5, 'One', 'Them', 'them@example.com', 100, 0, ''),
               ('acct', 'Archive', '2', 5, 'Two', 'Them', 'them@example.com', 200, 0, '')",
        params![],
    )
    .unwrap();

    let by_folder = thread_uids_by_folder(&conn, "acct", "INBOX", "uid:5", None, false).unwrap();

    assert_eq!(
        by_folder.into_iter().collect::<Vec<_>>(),
        vec![("INBOX".to_string(), vec![5])]
    );
}

#[test]
fn newest_thread_uids_is_empty_for_an_unknown_thread() {
    let conn = test_conn();

    assert!(
        newest_thread_uids(&conn, "acct", "INBOX", "missing", None)
            .unwrap()
            .is_empty()
    );
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

    let headers = get_thread_headers_all_folders(&conn, "acct", "topic-key", &[]).unwrap();
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
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key, json)
             VALUES('acct', '[Gmail]/Sent Mail', '462', 462, 'test', 'Me', 'me@example.com', 100, 1, 'self-key',
                      '{\"message_id\":\"mid@host\"}'),
                   ('acct', 'INBOX', '1440', 1440, 'test', 'Me', 'me@example.com', 100, 0, 'self-key',
                      '{\"message_id\":\"mid@host\"}'),
                   ('acct', 'INBOX', '1441', 1441, 'reply', 'Them', 'them@example.com', 200, 1, 'self-key',
                      '{\"message_id\":\"other@host\"}')",
            [],
        )
        .unwrap();

    let headers = get_thread_headers_all_folders(&conn, "acct", "self-key", &[]).unwrap();
    let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
    // The self-sent pair collapses to the unread copy when one exists, so the
    // visible bubble can be marked read and clear the thread badge.
    assert_eq!(uids, vec![1440, 1441]);
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

    let headers = get_thread_headers_all_folders(&conn, "acct", "k", &[]).unwrap();
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
fn thread_key_inheritance_matches_parent_message_id_case_insensitively() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    // Parent cached with its original mixed-case Message-ID.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "test".into(),
            date: 100,
            thread_key: "Root@Mail.example".into(),
            message_id: "Root@Mail.example".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    // A reply whose computed key fell back to In-Reply-To, with different
    // casing than the cached parent (e.g. a row synced before ids preserved
    // case): the parent lookup must still match and hand down the root key.
    upsert_messages(
        &conn,
        "acct",
        "Sent",
        &[MessageHeader {
            uid: 2,
            subject: "Re: test".into(),
            date: 200,
            thread_key: "root@mail.example".into(),
            message_id: "reply@mailo.com".into(),
            in_reply_to: "root@mail.example".into(),
            ..Default::default()
        }],
    )
    .unwrap();

    let child_key: String = conn
        .query_row("SELECT thread_key FROM messages WHERE uid = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(child_key, "Root@Mail.example");
}

#[test]
fn thread_key_inheritance_settles_a_reply_cached_before_its_parent() {
    use crate::imap::MessageHeader;
    let conn = test_conn();

    // Newest-first sync: the reply lands while the message its key names is
    // still unknown, so nothing can be inherited yet.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 2,
            subject: "Re: test".into(),
            date: 200,
            thread_key: "first-reply@local".into(),
            message_id: "second-reply@local".into(),
            ..Default::default()
        }],
    )
    .unwrap();

    // The parent arrives with the canonical root, and hands it down.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Re: test".into(),
            date: 100,
            thread_key: "root@mail.example".into(),
            message_id: "First-Reply@Local".into(),
            ..Default::default()
        }],
    )
    .unwrap();

    let child_key: String = conn
        .query_row("SELECT thread_key FROM messages WHERE uid = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(child_key, "root@mail.example");
}

#[test]
fn thread_key_inheritance_terminates_on_a_reference_cycle() {
    use crate::imap::MessageHeader;
    let conn = test_conn();

    // Two messages naming each other: whatever key they settle on, the walk
    // must stop rather than trade keys back and forth.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[
            MessageHeader {
                uid: 1,
                subject: "test".into(),
                date: 100,
                thread_key: "b@local".into(),
                message_id: "a@local".into(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "Re: test".into(),
                date: 200,
                thread_key: "a@local".into(),
                message_id: "b@local".into(),
                ..Default::default()
            },
        ],
    )
    .unwrap();

    let keys: Vec<String> = conn
        .prepare("SELECT thread_key FROM messages ORDER BY uid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(keys.len(), 2);
}

/// The inheritance lookups match rows on `lower(json_extract(...))` and
/// `lower(thread_key)`, which only hit an index when the query spells the
/// expression exactly as `MESSAGES_THREAD_KEY_INDEXES_DDL` does. Without them
/// every upserted reply scans the account's whole message cache, so guard the
/// access path, not just the result.
#[test]
fn thread_key_lookups_use_their_expression_indexes() {
    let conn = test_conn();
    let plan_of = |sql: &str| -> String {
        conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
            .join("\n")
    };

    let by_message_id = plan_of(
        "SELECT thread_key FROM messages
         WHERE account = 'acct'
           AND lower(COALESCE(json_extract(json, '$.message_id'), '')) = 'x'
           AND COALESCE(thread_key, '') <> ''
         ORDER BY date ASC, uid ASC
         LIMIT 1",
    );
    assert!(
        by_message_id.contains("messages_message_id_idx"),
        "message-id lookup fell back to a scan: {by_message_id}"
    );

    let by_thread_key = plan_of(
        "SELECT id FROM messages
         WHERE account = 'acct'
           AND uid <> 0
           AND lower(COALESCE(thread_key, '')) = 'x'
           AND thread_key <> 'y'
           AND thread_key NOT LIKE 'uid:%'
           AND thread_key NOT LIKE 'gmthrid:%'",
    );
    assert!(
        by_thread_key.contains("messages_thread_key_idx"),
        "thread-key lookup fell back to a scan: {by_thread_key}"
    );
}

#[test]
fn thread_key_inheritance_follows_referenced_message_to_canonical_root() {
    use crate::imap::MessageHeader;
    let conn = test_conn();

    // Proton Bridge can return a later reply whose References root is the
    // first sent reply, while In-Reply-To names the newest received message.
    // The first reply has already inherited the conversation's older root.
    upsert_messages(
        &conn,
        "acct",
        "Sent",
        &[MessageHeader {
            uid: 1,
            subject: "Re: test".into(),
            date: 100,
            thread_key: "proton-root@protonmail.internalid".into(),
            message_id: "first-reply@local".into(),
            in_reply_to: "proton-root@protonmail.internalid".into(),
            ..Default::default()
        }],
    )
    .unwrap();

    upsert_messages(
        &conn,
        "acct",
        "Sent",
        &[MessageHeader {
            uid: 2,
            subject: "Re: test".into(),
            date: 200,
            thread_key: "first-reply@local".into(),
            message_id: "second-reply@local".into(),
            in_reply_to: "received-between-replies@example.com".into(),
            ..Default::default()
        }],
    )
    .unwrap();

    let child_key: String = conn
        .query_row("SELECT thread_key FROM messages WHERE uid = 2", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(child_key, "proton-root@protonmail.internalid");
}

// A card listed while its encrypted messages showed the placeholder names the
// placeholder branch; once they are decrypted that branch matches none of them.
// Reading such a card takes the whole thread rather than nothing. A titled
// branch that matches nothing stays empty: it is not stale, just not this
// thread's. Actions never widen: a stale card must not reach other branches.
#[test]
fn a_placeholder_branch_matching_nothing_reads_the_whole_thread() {
    let subjects = || {
        vec![
            ("Sig test again today", false),
            ("Re: Sig test again today", false),
        ]
    };
    fn subject_of<'a>(item: &'a (&'static str, bool)) -> (&'a str, bool) {
        (item.0, item.1)
    }
    assert_eq!(
        branch_members_or_thread(subjects(), Some(""), subject_of).len(),
        2
    );
    assert_eq!(
        branch_members_or_thread(subjects(), Some("..."), subject_of).len(),
        2
    );
    assert!(branch_members_or_thread(subjects(), Some("Lunch"), subject_of).is_empty());
    assert_eq!(
        branch_members_or_thread(subjects(), None, subject_of).len(),
        2
    );
    assert!(branch_members(subjects(), Some(""), subject_of).is_empty());
    assert!(branch_members(subjects(), Some("..."), subject_of).is_empty());

    // While anything still matches the placeholder branch, only that is kept.
    let mixed = vec![("[...]", true), ("Other topic", false)];
    assert_eq!(
        branch_members_or_thread(mixed, Some(""), subject_of),
        vec![("[...]", true)]
    );
}

// Unstarring an empty-subject card walks every folder of its thread; a folder
// holding only another branch's messages must contribute none of them.
#[test]
fn unstarring_an_empty_branch_leaves_other_branches_in_other_folders() {
    let conn = test_conn();
    for (folder, uid, subject) in [("INBOX", 1, ""), ("Sent", 7, "Re: Lunch")] {
        upsert_messages(
            &conn,
            "acct",
            folder,
            &[MessageHeader {
                uid,
                subject: subject.into(),
                thread_key: "root@example.com".into(),
                starred: true,
                ..Default::default()
            }],
        )
        .unwrap();
    }
    let key = branch_compound_key("root@example.com", "");
    let targets = starred_mutation_targets(&conn, "acct", "INBOX", Some(&key), &[], false).unwrap();
    assert_eq!(targets, BTreeMap::from([("INBOX".into(), vec![1])]));
}
