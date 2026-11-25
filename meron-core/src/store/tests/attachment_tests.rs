use super::*;

/// A cached body carrying `attachments` (and optionally HTML that shows some of
/// them inline), saved the way the body fetch saves it.
fn save_body_with_attachments(conn: &Connection, uid: u32, keys: &[&str], html: Option<&str>) {
    let message = Message {
        subject: "Hi".into(),
        body: "body".into(),
        body_html: html.map(str::to_string),
        attachments: keys
            .iter()
            .map(|key| crate::parse::Attachment {
                filename: key.rsplit('/').next().unwrap().into(),
                mime: "application/octet-stream".into(),
                size: 1,
                key: Some(key.to_string()),
                oversized: false,
            })
            .collect(),
        ..Default::default()
    };
    save_cached_message(conn, "acct", "INBOX", uid, &message).unwrap();
}

#[test]
fn get_recent_page_can_return_only_messages_with_attachments() {
    let conn = test_conn();
    for uid in 1..=4 {
        insert_message(&conn, uid, "Hi", "Aki", "aki@example.com", None);
    }
    save_body_with_attachments(&conn, 1, &["acct/INBOX/1/0.pdf"], None);
    // Only a logo the HTML draws inline: not an attachment.
    save_body_with_attachments(
        &conn,
        2,
        &["acct/INBOX/2/0.png"],
        Some("<img src=\"/media/acct/INBOX/2/0.png\">"),
    );
    save_body_with_attachments(&conn, 3, &["acct/INBOX/3/0.zip"], None);
    // uid 4 keeps no cached body, so nothing is known about its parts.

    let (page, cursor) = get_recent_page(&conn, "acct", "INBOX", 1, None, ATTACHMENTS).unwrap();
    assert_eq!(page.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![3]);
    let (rest, cursor) = get_recent_page(
        &conn,
        "acct",
        "INBOX",
        1,
        parse_cursor(cursor.as_deref()),
        ATTACHMENTS,
    )
    .unwrap();
    assert_eq!(rest.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
    assert_eq!(cursor, None);

    // The two narrowings combine: reading uid 3 drops it from "unread with
    // attachments" but not from "with attachments".
    conn.execute("UPDATE messages SET seen = 1 WHERE uid = 3", [])
        .unwrap();
    let both = RecentFilter {
        unread_only: true,
        attachments_only: true,
    };
    let (page, _) = get_recent_page(&conn, "acct", "INBOX", 10, None, both).unwrap();
    assert_eq!(page.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
    let (page, _) = get_recent_page(&conn, "acct", "INBOX", 10, None, ATTACHMENTS).unwrap();
    assert_eq!(page.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![3, 1]);

    // Re-caching a body without the file clears the flag.
    save_body_with_attachments(&conn, 3, &[], None);
    let (page, _) = get_recent_page(&conn, "acct", "INBOX", 10, None, ATTACHMENTS).unwrap();
    assert_eq!(page.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![1]);
}

fn parse_cursor(cursor: Option<&str>) -> Option<(i64, u32)> {
    let (date, uid) = cursor?.strip_prefix("date:")?.split_once(':')?;
    Some((date.parse().ok()?, uid.parse().ok()?))
}

#[test]
fn card_attachments_span_the_thread_not_the_page() {
    let conn = test_conn();
    for (uid, key) in [
        (1, "a@example.com"),
        (2, "a@example.com"),
        (3, "b@example.com"),
    ] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, thread_key)
             VALUES('acct', 'INBOX', ?1, ?2, 'Hi', 1779580800, ?3)",
            params![uid.to_string(), uid, key],
        )
        .unwrap();
    }
    // The file sits on the thread's first message; a page holding only the
    // newest reply still marks the card.
    save_body_with_attachments(&conn, 1, &["acct/INBOX/1/0.pdf"], None);

    let header = |thread_key: &str| crate::imap::MessageHeader {
        subject: "Hi".into(),
        thread_key: thread_key.into(),
        ..Default::default()
    };
    let (a, b) = (
        card_thread_key(&header("a@example.com")),
        card_thread_key(&header("b@example.com")),
    );
    let files = card_attachments(&conn, "acct", "INBOX", &[a.clone(), b]).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(
        files[&a],
        vec![CardFile {
            filename: "0.pdf".into(),
            mime: "application/octet-stream".into()
        }]
    );
}

#[test]
fn files_backfill_runs_in_batches_after_the_migration() {
    let conn = Connection::open_in_memory().unwrap();
    db::migrate_to(&conn, 8).unwrap();
    let insert = |uid: u32, json: Value| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, body, json)
             VALUES('acct', 'INBOX', ?1, ?2, 'Hi', 1779580800, 'body', ?3)",
            params![uid.to_string(), uid, json.to_string()],
        )
        .unwrap();
    };
    let file = |key: Option<&str>| json!({"filename": "f", "mime": "x/y", "size": 1, "key": key});
    insert(
        1,
        json!({"attachments": [file(Some("acct/INBOX/1/0.pdf"))]}),
    );
    insert(
        2,
        json!({
            "body_html": "<img src=\"/media/acct/INBOX/2/0.png\">",
            "attachments": [file(Some("acct/INBOX/2/0.png"))],
        }),
    );
    insert(3, json!({"attachments": [file(None)]}));
    insert(4, json!({"attachments": []}));
    insert(5, json!({}));

    db::run_migrations(&conn).unwrap();
    let filled = || -> i64 {
        conn.query_row(
            "SELECT count(*) FROM messages WHERE files IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    // The migration itself stays cheap: nothing is derived at startup.
    assert_eq!(filled(), 0);

    // Two rows per batch: three calls cover five rows, the fourth finds none.
    let mut calls = 0;
    while !backfill_files_batch(&conn, 2).unwrap() {
        calls += 1;
        assert!(calls < 10, "backfill never finished");
    }
    assert_eq!(calls, 3);
    assert!(backfill_files_batch(&conn, 2).unwrap(), "stays done");

    let mut stmt = conn
        .prepare("SELECT uid, files FROM messages WHERE files IS NOT NULL ORDER BY uid")
        .unwrap();
    let flagged: Vec<(u32, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let file = json!([{"filename": "f", "mime": "x/y"}]);
    assert_eq!(
        flagged
            .into_iter()
            .map(|(uid, files)| (uid, serde_json::from_str::<Value>(&files).unwrap()))
            .collect::<Vec<_>>(),
        vec![(1, file.clone()), (3, file)]
    );
}

#[test]
fn attachments_filter_spans_the_thread_not_the_row() {
    let conn = test_conn();
    // A read message that carried the file, and an unread plain reply to it.
    for (uid, seen) in [(1, 1), (2, 0)] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, thread_key)
             VALUES('acct', 'INBOX', ?1, ?2, 'Hi', 'Aki', 'aki@example.com', ?3, ?4, 'root@example.com')",
            params![uid.to_string(), uid, 1779580800 + uid as i64, seen],
        )
        .unwrap();
    }
    conn.execute(
        "UPDATE messages SET files = '[{\"filename\":\"plan.pdf\",\"mime\":\"application/pdf\"}]' WHERE uid = 1",
        [],
    )
    .unwrap();

    let both = RecentFilter {
        unread_only: true,
        attachments_only: true,
    };
    let (page, _) = get_recent_page(&conn, "acct", "INBOX", 10, None, both).unwrap();
    assert_eq!(page.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![2]);
}

#[test]
fn card_attachments_prefer_the_cached_copy_of_a_folded_message() {
    let conn = test_conn();
    // One message filed twice: the Inbox copy (read first) has no cached body,
    // the Sent copy does.
    for (folder, files) in [
        ("INBOX", None),
        (
            "Sent",
            Some("[{\"filename\":\"plan.pdf\",\"mime\":\"application/pdf\"}]"),
        ),
    ] {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, thread_key, json, files)
             VALUES('acct', ?1, '1', 1, 'Hi', 1779580800, 'root@example.com',
                    '{\"message_id\":\"root@example.com\"}', ?2)",
            params![folder, files],
        )
        .unwrap();
    }
    let key = card_thread_key(&crate::imap::MessageHeader {
        subject: "Hi".into(),
        thread_key: "root@example.com".into(),
        ..Default::default()
    });
    let files = card_attachments(&conn, "acct", "INBOX", &[key.clone()]).unwrap();
    assert_eq!(files[&key].len(), 1, "one message, listed once");
    // Folding still counts the message once.
    assert_eq!(
        card_message_counts(&conn, "acct", "INBOX", &[key.clone()]).unwrap()[&key],
        1
    );
}

#[test]
fn attachments_filter_finds_unopened_messages_from_header_metadata() {
    let conn = test_conn();
    let file = CardFile {
        filename: "report.pdf".into(),
        mime: "application/pdf".into(),
    };
    let mut header = MessageHeader {
        uid: 1,
        subject: "Report".into(),
        thread_key: "uid:1".into(),
        files: Some(vec![file.clone()]),
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[header.clone()]).unwrap();
    let (page, _) = get_recent_page(&conn, "acct", "INBOX", 50, None, ATTACHMENTS).unwrap();
    assert_eq!(page.len(), 1);
    assert!(
        get_cached_message(&conn, "acct", "INBOX", 1)
            .unwrap()
            .is_none()
    );
    let files = card_attachments(&conn, "acct", "INBOX", &["uid:1".into()]).unwrap();
    assert_eq!(files["uid:1"], vec![file]);

    // A header/flag update that lacks BODYSTRUCTURE must preserve metadata.
    header.files = None;
    upsert_messages(&conn, "acct", "INBOX", &[header.clone()]).unwrap();
    assert_eq!(
        get_recent_page(&conn, "acct", "INBOX", 50, None, ATTACHMENTS)
            .unwrap()
            .0
            .len(),
        1
    );
    // A known empty structure clears the provisional names.
    header.files = Some(vec![]);
    upsert_messages(&conn, "acct", "INBOX", &[header.clone()]).unwrap();
    assert!(
        get_recent_page(&conn, "acct", "INBOX", 50, None, ATTACHMENTS)
            .unwrap()
            .0
            .is_empty()
    );
    // Cached bodies take precedence over provisional metadata on later syncs.
    save_body_with_attachments(&conn, 1, &[], None);
    header.files = Some(vec![CardFile {
        filename: "logo.png".into(),
        mime: "image/png".into(),
    }]);
    upsert_messages(&conn, "acct", "INBOX", &[header]).unwrap();
    assert!(
        get_recent_page(&conn, "acct", "INBOX", 50, None, ATTACHMENTS)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn attachment_metadata_catchup_is_bounded_and_skips_completed_rows() {
    let conn = test_conn();
    for uid in 1..=4 {
        insert_message(&conn, uid, "Hi", "Aki", "aki@example.com", None);
    }
    save_body_with_attachments(&conn, 4, &[], None);
    assert_eq!(
        missing_attachment_metadata_uids(&conn, "acct", "INBOX", 2).unwrap(),
        vec![3, 2]
    );
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 3,
            files: Some(vec![]),
            ..Default::default()
        }],
    )
    .unwrap();
    assert_eq!(
        missing_attachment_metadata_uids(&conn, "acct", "INBOX", 2).unwrap(),
        vec![2, 1]
    );
    assert!(
        missing_attachment_metadata_uids(&conn, "other", "INBOX", 2)
            .unwrap()
            .is_empty()
    );
    assert!(
        missing_attachment_metadata_uids(&conn, "acct", "Sent", 2)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn attachment_metadata_attempts_advance_past_missing_and_poison_rows() {
    let conn = test_conn();
    for uid in 1..=4 {
        insert_message(&conn, uid, "Hi", "Aki", "aki@example.com", None);
    }
    let attempted = missing_attachment_metadata_uids(&conn, "acct", "INBOX", 2).unwrap();
    assert_eq!(attempted, vec![4, 3]);
    mark_attachment_metadata_attempted(&conn, "other", "INBOX", &attempted).unwrap();
    assert_eq!(
        missing_attachment_metadata_uids(&conn, "acct", "INBOX", 2).unwrap(),
        attempted
    );
    // A malformed UID and a UID whose reply had no BODYSTRUCTURE both advance.
    mark_attachment_metadata_attempted(&conn, "acct", "INBOX", &attempted).unwrap();
    assert_eq!(
        missing_attachment_metadata_uids(&conn, "acct", "INBOX", 2).unwrap(),
        vec![2, 1]
    );
    // Attempted metadata remains unknown, rather than claiming no attachments.
    let known: Option<bool> = conn
        .query_row(
            "SELECT json_extract(json, '$.attachment_metadata') FROM messages WHERE uid = 4",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(known, None);
    // A future regular header refresh can still discover files for that UID.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            uid: 4,
            files: Some(vec![CardFile {
                filename: "later.pdf".into(),
                mime: "application/pdf".into(),
            }]),
            ..Default::default()
        }],
    )
    .unwrap();
    assert_eq!(
        get_recent_page(&conn, "acct", "INBOX", 50, None, ATTACHMENTS)
            .unwrap()
            .0
            .len(),
        1
    );
}

#[test]
fn folded_attachment_metadata_prefers_cached_bodies_including_empty_lists() {
    for cached_folder in ["INBOX", "Sent"] {
        for real_files in [vec![], vec!["real.pdf"]] {
            let conn = test_conn();
            for folder in ["INBOX", "Sent"] {
                upsert_messages(
                    &conn,
                    "acct",
                    folder,
                    &[MessageHeader {
                        uid: 1,
                        subject: "Hi".into(),
                        thread_key: "root@example.com".into(),
                        message_id: "root@example.com".into(),
                        files: Some(vec![CardFile {
                            filename: "provisional.png".into(),
                            mime: "image/png".into(),
                        }]),
                        ..Default::default()
                    }],
                )
                .unwrap();
            }
            let message = crate::parse::Message {
                subject: "Hi".into(),
                message_id: "root@example.com".into(),
                body: "Hello".into(),
                attachments: real_files
                    .iter()
                    .map(|name| crate::parse::Attachment {
                        filename: (*name).into(),
                        mime: "application/pdf".into(),
                        size: 1,
                        key: None,
                        oversized: false,
                    })
                    .collect(),
                ..Default::default()
            };
            save_cached_message(&conn, "acct", cached_folder, 1, &message).unwrap();
            let key = card_thread_key(&MessageHeader {
                subject: "Hi".into(),
                thread_key: "root@example.com".into(),
                ..Default::default()
            });
            let files = card_attachments(&conn, "acct", "INBOX", &[key.clone()]).unwrap();
            let names: Vec<_> = files
                .get(&key)
                .into_iter()
                .flatten()
                .map(|file| file.filename.as_str())
                .collect();
            assert_eq!(names, real_files, "cached copy in {cached_folder} must win");
            let (headers, cursor) =
                get_recent_page(&conn, "acct", "INBOX", 50, None, ATTACHMENTS).unwrap();
            let mut page =
                crate::thread_list::mail_page(&conn, "acct", "INBOX", headers, cursor, true)
                    .unwrap();
            crate::thread_list::ThreadListQuery::from_params(
                &json!({"attachments": true}),
                "folder",
            )
            .retain_attachment_threads(&mut page);
            assert_eq!(
                page["threads"].as_array().unwrap().len(),
                usize::from(!real_files.is_empty())
            );
            assert_eq!(
                card_message_counts(&conn, "acct", "INBOX", &[key.clone()]).unwrap()[&key],
                1
            );
        }
    }
}
