use super::*;

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
    let hits = search_messages(&conn, "acct", "INBOX", "rendered", 10, None).unwrap();
    assert!(hits.iter().any(|m| m.subject == "HTML only"));
}

#[test]
fn upsert_messages_preserves_message_id_casing_in_json() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    // The stored ids feed reply In-Reply-To/References headers verbatim, and
    // receivers (Gmail, GitHub) match Message-IDs case-sensitively — a synced
    // row must never lowercase them.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Wont download videos".into(),
            date: 100,
            thread_key: "nonbili/NouTube/issues/253@github.com".into(),
            message_id: "nonbili/NouTube/issues/253@github.com".into(),
            ..Default::default()
        }],
    )
    .unwrap();

    let (message_id, thread_key): (String, String) = conn
        .query_row(
            "SELECT json_extract(json, '$.message_id'), thread_key FROM messages WHERE uid = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(message_id, "nonbili/NouTube/issues/253@github.com");
    assert_eq!(thread_key, "nonbili/NouTube/issues/253@github.com");
}

#[test]
fn delete_draft_copies_removes_stale_autosaves_keeping_live_uid() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    // Two autosaves of the same draft (stable Message-ID, fresh UID per APPEND)
    // plus an unrelated draft that must survive.
    let draft = |uid: u32, message_id: &str| MessageHeader {
        uid,
        subject: "Re: test".into(),
        date: 100 + uid as i64,
        thread_key: "root@mail.example".into(),
        message_id: message_id.into(),
        ..Default::default()
    };
    upsert_messages(
        &conn,
        "acct",
        "Drafts",
        &[
            draft(10, "Meron-Draft-1@meron"),
            draft(11, "meron-draft-1@meron"),
            draft(12, "meron-draft-2@meron"),
        ],
    )
    .unwrap();

    // Replace path: keep the newest copy, drop the stale one (id compared
    // case-insensitively, matching the thread-key inheritance lookup).
    let deleted =
        delete_draft_copies(&conn, "acct", "Drafts", "meron-draft-1@meron", Some(11)).unwrap();
    assert_eq!(deleted, 1);
    let uids: Vec<u32> = conn
        .prepare("SELECT uid FROM messages WHERE folder = 'Drafts' ORDER BY uid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(uids, vec![11, 12]);

    // Discard path: no survivor, every copy goes.
    let deleted =
        delete_draft_copies(&conn, "acct", "Drafts", "meron-draft-1@meron", None).unwrap();
    assert_eq!(deleted, 1);
    // Blank id is a no-op, never a mass delete.
    assert_eq!(
        delete_draft_copies(&conn, "acct", "Drafts", " ", None).unwrap(),
        0
    );
}

#[test]
fn thread_draft_cleanup_requires_real_thread_key_and_generated_id() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let draft = |uid: u32, thread_key: &str, message_id: &str| MessageHeader {
        uid,
        thread_key: thread_key.into(),
        message_id: message_id.into(),
        ..Default::default()
    };
    upsert_messages(
        &conn,
        "acct",
        "Drafts",
        &[
            draft(10, "root@example.com", "meron-draft-old@gmail.com"),
            draft(11, "root@example.com", "manual@example.com"),
            draft(12, "uid:12", "meron-draft-uid@gmail.com"),
        ],
    )
    .unwrap();

    assert_eq!(
        delete_quick_reply_drafts_in_thread(&conn, "acct", "Drafts", "uid:12").unwrap(),
        0
    );
    assert_eq!(
        delete_quick_reply_drafts_in_thread(&conn, "acct", "Drafts", "root@example.com").unwrap(),
        1
    );
    let uids: Vec<u32> = conn
        .prepare("SELECT uid FROM messages WHERE folder = 'Drafts' ORDER BY uid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(uids, vec![11, 12]);
}

#[test]
fn delete_draft_sibling_copies_drops_hidden_same_id_rows() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let draft = |uid: u32, message_id: &str| MessageHeader {
        uid,
        subject: "Re: test".into(),
        date: 100 + uid as i64,
        thread_key: "root@mail.example".into(),
        message_id: message_id.into(),
        ..Default::default()
    };
    // The pane dedupes uid 10/11 (same Message-ID) into one bubble; discarding
    // it deletes only the visible uid, so the sibling cleanup must catch the
    // hidden copy — while leaving the unrelated draft alone.
    upsert_messages(
        &conn,
        "acct",
        "Drafts",
        &[
            draft(10, "meron-draft-1@meron"),
            draft(11, "meron-draft-1@meron"),
            draft(12, "meron-draft-2@meron"),
        ],
    )
    .unwrap();

    let deleted = delete_draft_sibling_copies(&conn, "acct", "Drafts", &[11]).unwrap();
    assert_eq!(deleted, 1);
    delete_messages_by_uid(&conn, "acct", "Drafts", &[11]).unwrap();
    let uids: Vec<u32> = conn
        .prepare("SELECT uid FROM messages WHERE folder = 'Drafts' ORDER BY uid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(uids, vec![12]);
}

#[test]
fn collapse_thread_draft_headers_keeps_newest_draft_only() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    upsert_folders(
        &conn,
        "acct",
        &[crate::imap::Folder {
            name: "Drafts".into(),
            display_name: "Drafts".into(),
            delimiter: None,
            special_use: Some("drafts".into()),
            role: "drafts".into(),
            unread: 0,
            notify: false,
        }],
    )
    .unwrap();
    let header = |uid: u32, folder: &str, date: i64| MessageHeader {
        uid,
        folder: folder.into(),
        subject: "Re: test".into(),
        date,
        thread_key: "root@example.com".into(),
        message_id: format!("draft-{uid}@example.com"),
        ..Default::default()
    };

    let collapsed = collapse_thread_draft_headers(
        &conn,
        "acct",
        "INBOX",
        vec![
            header(1, "INBOX", 100),
            header(10, "Drafts", 110),
            header(11, "Drafts", 120),
        ],
    )
    .unwrap();

    let uids = collapsed
        .into_iter()
        .map(|header| header.uid)
        .collect::<Vec<_>>();
    assert_eq!(uids, vec![1, 11]);
}

#[test]
fn starred_mutation_clears_all_folder_copies_and_can_restore_them() {
    let conn = test_conn();
    for (account, folder, uid, subject) in [
        ("acct", "INBOX", 1, "Release"),
        ("acct", "All Mail", 42, "Re: Release"),
        ("acct", "All Mail", 43, "Other branch"),
        ("other", "INBOX", 1, "Release"),
    ] {
        upsert_messages(
            &conn,
            account,
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
    let key = branch_compound_key("root@example.com", &thread_grouping_subject("Release"));
    let targets = starred_mutation_targets(&conn, "acct", "INBOX", Some(&key), &[], false).unwrap();
    assert_eq!(
        targets,
        BTreeMap::from([("INBOX".into(), vec![1]), ("All Mail".into(), vec![42])])
    );
    for starred in [false, true] {
        for (folder, uids) in &targets {
            for uid in uids {
                update_message_starred(&conn, "acct", folder, *uid, starred).unwrap();
            }
        }
        let cards = crate::mail_model::starred_thread_cards(&conn, 100).unwrap();
        assert_eq!(
            cards
                .iter()
                .filter(|card| card["account_id"] == "acct")
                .count(),
            if starred { 2 } else { 1 }
        );
        assert_eq!(
            cards
                .iter()
                .filter(|card| card["account_id"] == "other")
                .count(),
            1
        );
    }
}

#[test]
fn starred_mutation_keeps_folder_local_thread_keys_scoped() {
    let conn = test_conn();
    for folder in ["INBOX", "Archive"] {
        upsert_messages(
            &conn,
            "acct",
            folder,
            &[MessageHeader {
                uid: 1,
                starred: true,
                ..Default::default()
            }],
        )
        .unwrap();
    }
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", Some("uid:1"), &[], false).unwrap(),
        BTreeMap::from([("INBOX".into(), vec![1])])
    );
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", None, &[1], false).unwrap(),
        BTreeMap::from([("INBOX".into(), vec![1])])
    );
}

#[test]
fn starred_mutation_of_a_message_follows_only_its_copies() {
    let conn = test_conn();
    for (account, folder, uid, message_id) in [
        ("acct", "INBOX", 1, "<ONE@example.com>"),
        ("acct", "Archive", 99, "one@example.com"),
        ("acct", "Archive", 1, "two@example.com"),
        ("other", "INBOX", 1, "one@example.com"),
    ] {
        upsert_messages(
            &conn,
            account,
            folder,
            &[MessageHeader {
                uid,
                message_id: message_id.into(),
                thread_key: "root@example.com".into(),
                starred: true,
                ..Default::default()
            }],
        )
        .unwrap();
    }
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", None, &[1], false).unwrap(),
        BTreeMap::from([("INBOX".into(), vec![1]), ("Archive".into(), vec![99])])
    );
    assert!(
        starred_mutation_targets(&conn, "acct", "INBOX", None, &[], false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn starred_mutation_follows_gmail_identity_without_message_id() {
    let conn = test_conn();
    for (folder, uid, gmail_msg_id) in [
        ("INBOX", 1, 123),
        ("All Mail", 9, 123),
        ("All Mail", 1, 456),
    ] {
        upsert_messages(
            &conn,
            "acct",
            folder,
            &[MessageHeader {
                uid,
                gmail_msg_id: Some(gmail_msg_id),
                starred: true,
                ..Default::default()
            }],
        )
        .unwrap();
    }
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", None, &[1], false).unwrap(),
        BTreeMap::from([("INBOX".into(), vec![1]), ("All Mail".into(), vec![9])])
    );
}

#[test]
fn starred_mutation_skips_matching_flags_and_stars_only_source_messages_and_copies() {
    let conn = test_conn();
    for (folder, uid, mid, starred) in [
        ("INBOX", 1, "one@example.com", false),
        ("INBOX", 2, "two@example.com", true),
        ("All Mail", 10, "one@example.com", true),
        ("All Mail", 20, "two@example.com", false),
        ("Sent", 30, "reply@example.com", false),
        ("Trash", 40, "old@example.com", true),
    ] {
        upsert_messages(
            &conn,
            "acct",
            folder,
            &[MessageHeader {
                uid,
                subject: "Release".into(),
                message_id: mid.into(),
                thread_key: "root@example.com".into(),
                starred,
                ..Default::default()
            }],
        )
        .unwrap();
    }
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", Some("root@example.com"), &[], true)
            .unwrap(),
        BTreeMap::from([("INBOX".into(), vec![1]), ("All Mail".into(), vec![20])])
    );
    // A multi-message action finds all copies in one pass, including a copy
    // whose source already has the desired flag.
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", None, &[1, 2], true).unwrap(),
        BTreeMap::from([("INBOX".into(), vec![1]), ("All Mail".into(), vec![20])])
    );
    assert_eq!(
        starred_mutation_targets(&conn, "acct", "INBOX", Some("root@example.com"), &[], false)
            .unwrap(),
        BTreeMap::from([
            ("INBOX".into(), vec![2]),
            ("All Mail".into(), vec![10]),
            ("Trash".into(), vec![40])
        ])
    );
    assert!(
        starred_mutation_targets(&conn, "acct", "Sent", None, &[30], false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invite_responses_are_kept_per_event_and_dropped_with_the_account() {
    let conn = test_conn();
    assert_eq!(invite_response(&conn, "acct", "uid-1", 0).unwrap(), None);
    record_invite_response(&conn, "acct", "uid-1", 0, "tentative", 1).unwrap();
    record_invite_response(&conn, "acct", "uid-1", 0, "accepted", 2).unwrap();
    // One occurrence of the series is a separate answer.
    record_invite_response(&conn, "acct", "uid-1", 1_792_688_400, "declined", 2).unwrap();
    let series = invite_response(&conn, "acct", "uid-1", 0).unwrap().unwrap();
    assert_eq!((series.response.as_str(), series.sequence), ("accepted", 2));
    let occurrence = invite_response(&conn, "acct", "uid-1", 1_792_688_400)
        .unwrap()
        .unwrap();
    assert_eq!(occurrence.response, "declined");
    assert_eq!(invite_response(&conn, "other", "uid-1", 0).unwrap(), None);

    delete_account(&conn, "acct").unwrap();
    assert_eq!(invite_response(&conn, "acct", "uid-1", 0).unwrap(), None);
}

#[test]
fn a_cached_message_keeps_its_invite() {
    let conn = test_conn();
    let invite = crate::calendar::Invite {
        method: "request".into(),
        uid: "uid-1".into(),
        summary: "Launch review".into(),
        ics: "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".into(),
        ..Default::default()
    };
    save_cached_message(
        &conn,
        "acct",
        "INBOX",
        9,
        &Message {
            body: "You are invited".into(),
            invite: Some(invite.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let cached = get_cached_message(&conn, "acct", "INBOX", 9)
        .unwrap()
        .unwrap();
    assert_eq!(cached.invite, Some(invite));
}

#[test]
fn matching_uids_reads_headers_and_never_a_cached_body() {
    let conn = test_conn();
    insert_message(
        &conn,
        1,
        "Plan",
        "Aki",
        "aki@example.com",
        Some("the secretword"),
    );
    insert_message(&conn, 2, "secretword", "Aki", "aki@example.com", None);
    insert_message(&conn, 3, "Plan", "Aki", "aki@example.com", Some("ab here"));
    insert_message(&conn, 4, "ab", "Aki", "aki@example.com", None);

    let hits = |query: &str| matching_uids(&conn, "acct", "INBOX", query, &[1, 2, 3, 4]).unwrap();
    assert_eq!(hits("secretword"), [2]);
    // Short enough to be scanned, not looked up in the index.
    assert_eq!(hits("ab"), [4]);
    let search = search_messages(&conn, "acct", "INBOX", "secretword", 10, None).unwrap();
    assert_eq!(search.len(), 2);
}

#[test]
fn synced_repeat_rules_are_read_again_once_the_editor_can_say_them() {
    let conn = test_conn();
    let ics = |rule: &str, extra: &str| {
        format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:x\r\nDTSTART:20261005T090000Z\r\n\
             DTEND:20261005T100000Z\r\nRRULE:{rule}\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n"
        )
    };
    let insert = |id: &str, ics: &str| {
        conn.execute(
            "INSERT INTO events (id, start_at, end_at, recurring, rrule, calendar, ics)
             VALUES (?1, 0, 0, 1, '', 'cal', ?2)",
            params![id, ics],
        )
        .unwrap();
    };
    insert("sayable", &ics("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE", ""));
    insert(
        "skipped",
        &ics("FREQ=WEEKLY", "EXDATE:20261012T090000Z\r\n"),
    );
    // Taken on as `weekly` before the editor left a zone only its object
    // defines to the server.
    conn.execute(
        "INSERT INTO events (id, start_at, end_at, recurring, rrule, calendar, ics)
         VALUES ('own-zone', 0, 0, 1, 'weekly', 'cal', ?1)",
        params![
            "BEGIN:VCALENDAR\r\nBEGIN:VTIMEZONE\r\nTZID:Office Time\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\n\
             TZOFFSETFROM:+0900\r\nTZOFFSETTO:+0900\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:z\r\n\
             DTSTART;TZID=Office Time:20261005T090000\r\nDTEND;TZID=Office Time:20261005T100000\r\n\
             RRULE:FREQ=WEEKLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        ],
    )
    .unwrap();
    let rule = |id: &str| -> String {
        conn.query_row("SELECT rrule FROM events WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
    };

    db::meta_set(&conn, "synced_rules_version", "0").unwrap();
    db::reread_synced_repeat_rules(&conn).unwrap();
    assert_eq!(rule("sayable"), "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE");
    // Skipped dates are still the server's to keep.
    assert_eq!(rule("skipped"), "");
    // And one it no longer takes on goes back to the server.
    assert_eq!(rule("own-zone"), "");

    // Once done, it is not done again.
    conn.execute("UPDATE events SET rrule = '' WHERE id = 'sayable'", [])
        .unwrap();
    db::reread_synced_repeat_rules(&conn).unwrap();
    assert_eq!(rule("sayable"), "");
}
