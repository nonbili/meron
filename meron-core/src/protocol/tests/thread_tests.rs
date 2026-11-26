use super::*;

#[test]
fn mobile_protocol_lists_threads_from_store() {
    let data_dir = unique_data_dir("threads");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 7,
                subject: "Newest".to_string(),
                from_name: "Ada".to_string(),
                from_addr: "ada@example.com".to_string(),
                date: 200,
                seen: false,
                starred: true,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 3,
                subject: "Older".to_string(),
                from_name: "Bea".to_string(),
                from_addr: "bea@example.com".to_string(),
                date: 100,
                seen: true,
                starred: false,
                thread_key: "older-topic".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":64,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 64);
    assert_eq!(value["result"]["folder_unread"], 1);
    assert_eq!(value["result"]["folder_synced"], false);
    let first = &value["result"]["threads"][0];
    assert_eq!(first["account_id"], "me@example.com");
    assert_eq!(first["folder_id"], "INBOX");
    assert_eq!(first["subject"], "Newest");
    assert_eq!(first["from_name"], "Ada");
    assert_eq!(first["unread"], true);
    assert_eq!(first["unread_count"], 1);
    assert_eq!(first["starred"], true);
    assert_eq!(
        first["thread_id"],
        "me@example.com#INBOX#t.dG9waWMjTmV3ZXN0"
    );

    let unread = invoke_mobile_protocol_json(
        r#"{"id":65,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"unread"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(unread["result"]["threads"].as_array().unwrap().len(), 1);

    let starred = invoke_mobile_protocol_json(
        r#"{"id":66,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"starred"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let starred_threads = starred["result"]["threads"].as_array().unwrap();
    assert_eq!(starred_threads.len(), 1);
    assert_eq!(starred_threads[0]["subject"], "Newest");

    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::set_folder_state(&conn, "me@example.com", "INBOX", 1, 8).unwrap();
    drop(conn);
    let synced = invoke_mobile_protocol_json(
        r#"{"id":67,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(synced["result"]["folder_synced"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_contact_threads_pages_exact_matches_across_accounts() {
    let data_dir = unique_data_dir("contact-threads");
    seed_mobile_account(&data_dir, "me@example.com");
    seed_mobile_account(&data_dir, "other@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    for (account, folder, uid, addr, date) in [
        ("me@example.com", "INBOX", 1, "Alice@example.com", 300),
        ("other@example.com", "Archive", 2, "alice@example.com", 200),
        ("me@example.com", "INBOX", 3, "notalice@example.com", 400),
        ("me@example.com", "Spam", 4, "alice@example.com", 900),
        ("other@example.com", "Trash", 5, "alice@example.com", 1000),
    ] {
        store::upsert_messages(
            &conn,
            account,
            folder,
            &[MessageHeader {
                uid,
                subject: format!("Topic {uid}"),
                from_name: "Alice".to_string(),
                from_addr: addr.to_string(),
                date,
                thread_key: format!("mid:{uid}"),
                starred: true,
                ..Default::default()
            }],
        )
        .unwrap();
    }
    drop(conn);
    let invoke = |method: &str, params: Value| {
        invoke_mobile_protocol_json(
            &json!({"id": 1, "method": method, "params": params}).to_string(),
            Some(data_dir.to_str().unwrap()),
        )
    };
    let first = invoke(
        "mail.contactThreads",
        json!({"addr": " alice@example.com ", "limit": 1}),
    );
    let cards = first["result"]["threads"].as_array().unwrap();
    assert_eq!(cards.len(), 1, "{first}");
    assert_eq!(cards[0]["account_id"], "me@example.com");
    assert_eq!(cards[0]["folder_id"], "INBOX");
    assert_eq!(cards[0]["id"], cards[0]["thread_id"]);
    assert_eq!(cards[0]["unread"], true);
    assert_eq!(cards[0]["starred"], true);
    let mailbox = invoke(
        "mail.threadList",
        json!({"account_id": "me@example.com", "folder_id": "INBOX"}),
    );
    assert!(
        mailbox["result"]["threads"]
            .as_array()
            .unwrap()
            .contains(&cards[0])
    );
    let second = invoke(
        "contacts.threads",
        json!({
            "addr": "alice@example.com", "limit": 1,
            "before_cursor": first["result"]["next_cursor"],
        }),
    );
    assert_eq!(
        second["result"]["threads"][0]["account_id"],
        "other@example.com"
    );
    assert_eq!(second["result"]["threads"][0]["folder_id"], "Archive");
    assert!(second["result"]["next_cursor"].is_null());
    assert!(
        invoke("contacts.threads", json!({"addr": "nobody@example.com"}))["result"]["threads"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for params in [
        json!({}),
        json!({"addr": "  "}),
        json!({"addr": "alice@example.com", "before_cursor": "invalid"}),
    ] {
        assert!(invoke("mail.contactThreads", params)["error"]["message"].is_string());
    }
    let clamped = invoke(
        "contacts.threads",
        json!({"addr": "alice@example.com", "limit": 0}),
    );
    assert_eq!(clamped["result"]["threads"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_lists_unified_threads_and_excludes_opted_out_accounts() {
    let data_dir = unique_data_dir("unified-threads");
    for account in [
        "first@example.com",
        "second@example.com",
        "excluded@example.com",
    ] {
        seed_mobile_account(&data_dir, account);
    }
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::set_account_pref(&conn, "excluded@example.com", "included_in_unified", false).unwrap();
    for (account, uid, date, unread) in [
        ("first@example.com", 7, 100, true),
        ("second@example.com", 9, 200, true),
        ("excluded@example.com", 11, 300, true),
    ] {
        store::ensure_folder(&conn, account, "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            account,
            "INBOX",
            &[MessageHeader {
                uid,
                subject: format!("Message from {account}"),
                from_addr: "sender@example.com".to_string(),
                date,
                seen: !unread,
                thread_key: format!("topic-{uid}"),
                ..Default::default()
            }],
        )
        .unwrap();
    }
    store::set_folder_state(&conn, "first@example.com", "INBOX", 1, 8).unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":164,"method":"mail.threadList","params":{"account_id":"unified","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let threads = value["result"]["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 2, "{value}");
    assert_eq!(threads[0]["account_id"], "second@example.com", "{value}");
    assert_eq!(threads[1]["account_id"], "first@example.com", "{value}");
    assert_eq!(value["result"]["folder_unread"], 2, "{value}");
    assert_eq!(value["result"]["folder_unreads"]["first@example.com"], 1);
    assert_eq!(value["result"]["folder_unreads"]["second@example.com"], 1);
    assert_eq!(value["result"]["failures"], json!([]));
    assert_eq!(
        value["result"]["unsynced_folders"],
        json!([{ "account_id": "second@example.com", "folder_id": "INBOX" }])
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn starred_rows_group_into_threads_across_folders() {
    let data_dir = unique_data_dir("starred-threads");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    for folder in ["INBOX", "Archive"] {
        store::ensure_folder(&conn, "me@example.com", folder).unwrap();
    }
    // Two starred messages of one thread in INBOX collapse to a single card...
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 1,
                subject: "Trip plans".to_string(),
                from_addr: "ana@example.com".to_string(),
                date: 100,
                seen: true,
                starred: true,
                thread_key: "trip".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "Re: Trip plans".to_string(),
                from_addr: "ana@example.com".to_string(),
                date: 150,
                seen: true,
                starred: true,
                thread_key: "trip".to_string(),
                ..Default::default()
            },
            // The card is admitted by the stars above, but its ordinary thread
            // metadata must still come from this newer unread reply.
            MessageHeader {
                uid: 3,
                subject: "Re: Trip plans".to_string(),
                from_addr: "bob@example.com".to_string(),
                date: 200,
                thread_key: "trip".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    // ...and so does the same thread starred in Archive: it is one conversation
    // to the reader however many folders the server files it under, and it is
    // listed in the more specific of them.
    store::upsert_messages(
        &conn,
        "me@example.com",
        "Archive",
        &[MessageHeader {
            uid: 5,
            subject: "Trip plans".to_string(),
            from_addr: "ana@example.com".to_string(),
            date: 120,
            starred: true,
            thread_key: "trip".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":75,"method":"mail.starredItems","params":{"limit":10}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let items = value["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{value}");
    let inbox = &items[0];
    assert_eq!(inbox["id"], inbox["thread_id"], "{value}");
    assert_eq!(inbox["folder_id"], "INBOX", "{value}");
    assert_eq!(inbox["date"], 200, "{value}");
    assert_eq!(inbox["unread"], true, "{value}");
    assert_eq!(inbox["unread_count"], 1, "{value}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn unified_folder_role_resolves_per_account_and_skips_accounts_without_it() {
    let data_dir = unique_data_dir("unified-role");
    for account in ["has-sent@example.com", "no-sent@example.com"] {
        seed_mobile_account(&data_dir, account);
    }
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    // One account files sent mail under a non-obvious name flagged \Sent; the
    // other has no sent mailbox at all and must drop out of the merge.
    store::upsert_folders(
        &conn,
        "has-sent@example.com",
        &[crate::imap::Folder {
            name: "Postausgang".to_string(),
            special_use: Some("sent".to_string()),
            ..Default::default()
        }],
    )
    .unwrap();
    store::ensure_folder(&conn, "has-sent@example.com", "Postausgang").unwrap();
    store::upsert_messages(
        &conn,
        "has-sent@example.com",
        "Postausgang",
        &[MessageHeader {
            uid: 3,
            subject: "Sent mail".to_string(),
            from_addr: "me@example.com".to_string(),
            date: 500,
            seen: false,
            thread_key: "sent-topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":165,"method":"mail.threadList","params":{"account_id":"unified","folder_id":"sent","folder_role":"sent","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let threads = value["result"]["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1, "{value}");
    assert_eq!(threads[0]["account_id"], "has-sent@example.com", "{value}");
    assert_eq!(threads[0]["folder_id"], "Postausgang", "{value}");
    // A missing role is not an error: an account whose server has no Sent
    // mailbox is simply absent, with no failure banner for the view to show.
    assert_eq!(value["result"]["failures"], json!([]), "{value}");

    // Folder-wide mutations use the same role resolution. The one account with
    // a Sent mailbox reaches authentication, while the account without one is
    // skipped; treating "sent" as a literal folder would silently update neither.
    let mark_all = invoke_mobile_protocol_json(
        r#"{"id":168,"method":"mail.markAllRead","params":{"account_id":"unified","folder_id":"sent"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let failures = mark_all["result"]["failures"].as_array().unwrap();
    assert_eq!(failures.len(), 1, "{mark_all}");
    assert_eq!(
        failures[0]["account_id"], "has-sent@example.com",
        "{mark_all}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_keeps_gmail_thread_id_atomic_across_subject_drift() {
    let data_dir = unique_data_dir("gmail-thread-id");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 10,
                subject: "[nonbili/Nora] Profiles bug (Issue #295)".to_string(),
                from_name: "NightStars".to_string(),
                date: 100,
                seen: true,
                thread_key: "gmthrid:123".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 11,
                subject: "[nonbili/Nora] Profiles bug [Linux Flatpak] (Issue #295)".to_string(),
                from_name: "NightStars".to_string(),
                date: 200,
                seen: false,
                thread_key: "gmthrid:123".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":166,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let threads = value["result"]["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1, "{value}");
    assert_eq!(
        threads[0]["thread_id"],
        "me@example.com#INBOX#t.Z210aHJpZDoxMjM"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_inherits_parent_thread_key_for_sent_reply_chain() {
    let data_dir = unique_data_dir("sent-reply-chain");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "me@example.com", "Sent").unwrap();

    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "test mailo2".to_string(),
            from_name: "Gmail".to_string(),
            from_addr: "sender@gmail.com".to_string(),
            date: 100,
            seen: true,
            thread_key: "root@mail.gmail.com".to_string(),
            message_id: "root@mail.gmail.com".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "INBOX",
        7,
        &Message {
            subject: "test mailo2".to_string(),
            from_name: "Gmail".to_string(),
            from_addr: "sender@gmail.com".to_string(),
            to: "Me <me@example.com>".to_string(),
            message_id: "root@mail.gmail.com".to_string(),
            date: 100,
            body: "root".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    // Reproduces a fresh account add / Sent sync: newest rows can be processed
    // before older parents. The second sent reply has no usable References root,
    // so its computed key fell back to In-Reply-To (the first sent reply), even
    // though that parent appears later in the same batch.
    store::upsert_messages(
        &conn,
        "me@example.com",
        "Sent",
        &[
            MessageHeader {
                uid: 9,
                subject: "Re: test mailo2".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 300,
                seen: true,
                thread_key: "first-reply@mailo.com".to_string(),
                message_id: "second-reply@mailo.com".to_string(),
                in_reply_to: "first-reply@mailo.com".to_string(),
                ..Default::default()
            },
            MessageHeader {
                // Deliberately collides with the root's INBOX UID. IMAP UIDs
                // are mailbox-local, so both messages must keep distinct ids.
                uid: 7,
                subject: "Re: test mailo2".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 200,
                seen: true,
                thread_key: "root@mail.gmail.com".to_string(),
                message_id: "first-reply@mailo.com".to_string(),
                in_reply_to: "root@mail.gmail.com".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Sent",
        7,
        &Message {
            subject: "Re: test mailo2".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            to: "sender@gmail.com".to_string(),
            message_id: "first-reply@mailo.com".to_string(),
            references: "root@mail.gmail.com".to_string(),
            date: 200,
            body: "first".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Sent",
        9,
        &Message {
            subject: "Re: test mailo2".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            to: "sender@gmail.com".to_string(),
            message_id: "second-reply@mailo.com".to_string(),
            references: "root@mail.gmail.com first-reply@mailo.com".to_string(),
            date: 300,
            body: "second".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let sent = invoke_mobile_protocol_json(
        r#"{"id":167,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"Sent","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let sent_threads = sent["result"]["threads"].as_array().unwrap();
    assert_eq!(sent_threads.len(), 1, "{sent}");
    assert_eq!(
        sent_threads[0]["thread_id"],
        "me@example.com#Sent#t.cm9vdEBtYWlsLmdtYWlsLmNvbSN0ZXN0IG1haWxvMg"
    );

    let read = invoke_mobile_protocol_json(
        r#"{"id":168,"method":"mail.threadRead","params":{"thread_id":"me@example.com#INBOX#t.cm9vdEBtYWlsLmdtYWlsLmNvbQ"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let messages = read["result"]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "{read}");
    assert_eq!(messages[0]["id"], "me@example.com#INBOX#7");
    assert_eq!(messages[1]["id"], "me@example.com#Sent#7");
    assert_eq!(messages[0]["message_id"], "root@mail.gmail.com");
    assert_eq!(messages[1]["message_id"], "first-reply@mailo.com");
    assert_eq!(messages[2]["message_id"], "second-reply@mailo.com");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn print_thread_read_keeps_uncached_messages_when_connection_is_unavailable() {
    let data_dir = unique_data_dir("print-thread-offline");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "offline@example.com", "INBOX").unwrap();
    let headers: Vec<_> = (1..=60)
        .map(|uid| MessageHeader {
            uid,
            subject: "Print topic".to_string(),
            thread_key: "topic".to_string(),
            date: uid as i64,
            ..Default::default()
        })
        .collect();
    store::upsert_messages(&conn, "offline@example.com", "INBOX", &headers).unwrap();
    drop(conn);
    // No credentials: every body fetch fails, including pages with no cache.
    let mut cursor = serde_json::Value::Null;
    let mut count = 0;
    for expected in [50, 10] {
        let request = serde_json::json!({
            "id": 1, "method": "mail.threadRead",
            "params": { "thread_id": "offline@example.com#INBOX#t.dG9waWM",
                "limit": 50, "for_print": true, "before_cursor": cursor }
        });
        let response =
            invoke_mobile_protocol_json(&request.to_string(), Some(data_dir.to_str().unwrap()));
        assert!(response.get("error").is_none(), "{response}");
        let messages = response["result"]["messages"].as_array().unwrap();
        assert_eq!(messages.len(), expected);
        assert!(
            messages
                .iter()
                .all(|message| message["body_missing"] == true)
        );
        count += messages.len();
        cursor = response["result"]["next_cursor"].clone();
    }
    assert_eq!(count, 60);
    assert!(cursor.is_null());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reads_cached_thread_messages_from_store() {
    let data_dir = unique_data_dir("thread-read");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 9,
            subject: "Cached subject".to_string(),
            from_name: "Ada".to_string(),
            from_addr: "ada@example.com".to_string(),
            date: 300,
            seen: false,
            starred: true,
            thread_key: "topic".to_string(),
            to: vec![Recipient {
                name: "Me".to_string(),
                addr: "me@example.com".to_string(),
            }],
            ..Default::default()
        }],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "INBOX",
        9,
        &Message {
            subject: "Cached subject".to_string(),
            from_name: "Ada".to_string(),
            from_addr: "ada@example.com".to_string(),
            to: "Me <me@example.com>".to_string(),
            message_id: "topic".to_string(),
            references: "root".to_string(),
            date: 300,
            body: "Hello from cache".to_string(),
            body_html: Some("<p>Hello from cache</p>".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":66,"method":"mail.threadRead","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 66);
    let first = &value["result"]["messages"][0];
    assert_eq!(first["id"], "me@example.com#INBOX#9");
    assert_eq!(first["account_id"], "me@example.com");
    assert_eq!(first["folder_id"], "INBOX");
    assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");
    assert_eq!(first["from_name"], "Ada");
    assert_eq!(first["to"], "Me <me@example.com>");
    assert_eq!(first["message_id"], "topic");
    assert_eq!(first["references"], "root");
    assert_eq!(first["subject"], "Cached subject");
    assert_eq!(first["body"], "Hello from cache");
    // Prepared by the core, like the desktop's: sanitized, quote-marked, CSP baked in.
    let body_html = first["body_html"].as_str().unwrap();
    assert!(
        body_html.contains("<body><p>Hello from cache</p></body>"),
        "{body_html}"
    );
    assert!(body_html.contains("Content-Security-Policy"), "{body_html}");
    assert_eq!(first["unread"], true);
    assert_eq!(first["starred"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reads_uid_thread_message_from_store() {
    let data_dir = unique_data_dir("uid-thread-read");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Drafts",
        4,
        &Message {
            subject: "Draft".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            date: 400,
            body: "Draft body".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":67,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 67);
    assert_eq!(value["result"]["messages"][0]["subject"], "Draft");
    assert_eq!(value["result"]["messages"][0]["body"], "Draft body");

    let _ = std::fs::remove_dir_all(data_dir);
}

// Notifications are withdrawn for mail no longer unread in the cache: read,
// or gone from the folder (moved/deleted here, or pruned by a sync after
// another device did it).
#[test]
fn mobile_protocol_reports_which_notified_messages_are_still_unread() {
    let data_dir = unique_data_dir("still-unread");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    let header = |uid: u32, seen: bool| MessageHeader {
        uid,
        subject: format!("Mail {uid}"),
        from_addr: "aki@example.com".to_string(),
        date: 100 + i64::from(uid),
        seen,
        ..Default::default()
    };
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[header(1, false), header(2, true), header(3, false)],
    )
    .unwrap();
    store::delete_messages_by_uid(&conn, "me@example.com", "INBOX", &[3]).unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":5,"method":"mail.stillUnread","params":{"messages":[
            {"account_id":"me@example.com","folder_id":"INBOX","message_id":"1"},
            {"account_id":"me@example.com","folder_id":"inbox","message_id":"1"},
            {"account_id":"me@example.com","folder_id":"INBOX","message_id":"2"},
            {"account_id":"me@example.com","folder_id":"INBOX","message_id":"3"},
            {"account_id":"other@example.com","folder_id":"INBOX","message_id":"1"},
            {"account_id":"me@example.com","folder_id":"","message_id":"1"}
        ]}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(
        value["result"]["unread"],
        // The inbox as a `mail.newMessages` detail names it ("inbox") finds the
        // stored "INBOX" row, and comes back named as it was asked.
        json!([
            { "account_id": "me@example.com", "folder_id": "INBOX", "message_id": "1" },
            { "account_id": "me@example.com", "folder_id": "inbox", "message_id": "1" }
        ])
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

// A reply drafted on another client lands in the local store via the regular
// Drafts folder sync; opening the thread must surface it from the store alone
// (thread read spans folders), with no per-thread network check.
#[test]
fn mobile_protocol_thread_read_includes_cached_draft_reply() {
    let data_dir = unique_data_dir("thread-read-draft-reply");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "me@example.com", "Drafts").unwrap();

    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "test mailo2".to_string(),
            from_name: "Gmail".to_string(),
            from_addr: "sender@gmail.com".to_string(),
            date: 100,
            seen: true,
            thread_key: "root@mail.gmail.com".to_string(),
            message_id: "root@mail.gmail.com".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "INBOX",
        7,
        &Message {
            subject: "test mailo2".to_string(),
            from_name: "Gmail".to_string(),
            from_addr: "sender@gmail.com".to_string(),
            to: "Me <me@example.com>".to_string(),
            message_id: "root@mail.gmail.com".to_string(),
            date: 100,
            body: "root".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    store::upsert_messages(
        &conn,
        "me@example.com",
        "Drafts",
        &[MessageHeader {
            uid: 3,
            subject: "Re: test mailo2".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            date: 200,
            seen: true,
            thread_key: "root@mail.gmail.com".to_string(),
            message_id: "draft-reply@mailo.com".to_string(),
            in_reply_to: "root@mail.gmail.com".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Drafts",
        3,
        &Message {
            subject: "Re: test mailo2".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            to: "sender@gmail.com".to_string(),
            message_id: "draft-reply@mailo.com".to_string(),
            references: "root@mail.gmail.com".to_string(),
            date: 200,
            body: "draft reply".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let read = invoke_mobile_protocol_json(
        r#"{"id":69,"method":"mail.threadRead","params":{"thread_id":"me@example.com#INBOX#t.cm9vdEBtYWlsLmdtYWlsLmNvbQ"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let messages = read["result"]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2, "{read}");
    assert_eq!(messages[0]["message_id"], "root@mail.gmail.com");
    assert_eq!(messages[1]["message_id"], "draft-reply@mailo.com");
    assert_eq!(messages[1]["in_reply_to"], "root@mail.gmail.com");
    assert_eq!(messages[1]["folder_id"], "Drafts");
    assert_eq!(messages[1]["body"], "draft reply");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_lists_starred_items_from_store() {
    let data_dir = unique_data_dir("starred-items");
    seed_mobile_account(&data_dir, "a1@example.com");
    seed_mobile_account(&data_dir, "a2@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "a1@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "a2@example.com", "Sent").unwrap();
    store::upsert_messages(
        &conn,
        "a1@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 1,
                subject: "Older starred".to_string(),
                from_name: "Ada".to_string(),
                from_addr: "ada@example.com".to_string(),
                date: 100,
                seen: false,
                starred: true,
                thread_key: "older".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "Not starred".to_string(),
                date: 300,
                seen: false,
                starred: false,
                thread_key: "skip".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "a2@example.com",
        "Sent",
        &[MessageHeader {
            uid: 7,
            subject: "Newest starred".to_string(),
            from_name: "Bea".to_string(),
            from_addr: "bea@example.com".to_string(),
            date: 200,
            seen: true,
            starred: true,
            thread_key: "newer".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":74,"method":"mail.starredItems","params":{"limit":2}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 74);
    let items = value["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["account_id"], "a2@example.com");
    assert_eq!(items[0]["folder_id"], "Sent");
    assert_eq!(items[0]["subject"], "Newest starred");
    assert_eq!(
        items[0]["thread_id"],
        "a2@example.com#Sent#t.bmV3ZXIjTmV3ZXN0IHN0YXJyZWQ"
    );
    // Starred rows are thread cards like every other mailbox: the row id *is*
    // the thread id, not a per-message "{thread_id}#{uid}".
    assert_eq!(items[0]["id"], items[0]["thread_id"]);
    assert_eq!(items[0]["unread"], false);
    assert_eq!(items[0]["starred"], true);
    assert_eq!(items[1]["account_id"], "a1@example.com");
    assert_eq!(items[1]["subject"], "Older starred");

    // Filtering happens before the limit is applied, so a newer read row cannot
    // hide an older unread row on the first page.
    let unread = invoke_mobile_protocol_json(
        r#"{"id":75,"method":"mail.starredItems","params":{"filter":"unread","limit":1}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let unread_items = unread["result"]["items"].as_array().unwrap();
    assert_eq!(unread_items.len(), 1);
    assert_eq!(unread_items[0]["subject"], "Older starred");
    assert!(unread["result"].get("next_cursor").is_none());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn starred_lists_a_thread_once_when_several_folders_cache_it() {
    // Gmail files every message in All Mail as well as its Inbox/label copy, so
    // the same starred thread is cached under several folders. It is one thread
    // and gets one row, in the most specific folder holding it.
    let data_dir = unique_data_dir("starred-cross-folder");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                ..Default::default()
            },
            Folder {
                name: "Receipts".to_string(),
                ..Default::default()
            },
            Folder {
                name: "[Gmail]/All Mail".to_string(),
                special_use: Some("all".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let header = MessageHeader {
        uid: 5,
        subject: "Your update is live".to_string(),
        from_name: "Google Play Console".to_string(),
        from_addr: "noreply@google.com".to_string(),
        date: 400,
        seen: true,
        starred: true,
        thread_key: "play-update@google.com".to_string(),
        ..Default::default()
    };
    for folder in ["INBOX", "Receipts", "[Gmail]/All Mail"] {
        store::upsert_messages(&conn, "me@example.com", folder, &[header.clone()]).unwrap();
    }
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":76,"method":"mail.starredItems","params":{"limit":10}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let items = value["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["folder_id"], "INBOX");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn starred_keeps_unthreaded_messages_that_share_a_uid_apart() {
    // A message with no threading headers falls back to a "uid:<uid>" key, and
    // IMAP UIDs only mean anything inside their own mailbox. Two unrelated
    // starred messages that happen to be uid 1 in different folders are not one
    // conversation and neither may swallow the other.
    let data_dir = unique_data_dir("starred-uid-keys");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    for folder in ["INBOX", "Archive"] {
        store::ensure_folder(&conn, "me@example.com", folder).unwrap();
    }
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Inbox note".to_string(),
            from_addr: "ana@example.com".to_string(),
            date: 100,
            starred: true,
            ..Default::default()
        }],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "Archive",
        &[MessageHeader {
            uid: 1,
            subject: "Archived note".to_string(),
            from_addr: "bob@example.com".to_string(),
            date: 200,
            starred: true,
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":77,"method":"mail.starredItems","params":{"limit":10}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let items = value["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{value}");
    let subjects = items
        .iter()
        .map(|item| item["subject"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(subjects.contains(&"Inbox note"), "{value}");
    assert!(subjects.contains(&"Archived note"), "{value}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn requested_mobile_uids_uses_message_ids_without_thread_fallback() {
    let data_dir = unique_data_dir("targeted-uids");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 7,
                subject: "One".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 8,
                subject: "Two".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let parsed = ParsedThreadId {
        account: "me@example.com".to_string(),
        folder: "INBOX".to_string(),
        thread_key: "topic".to_string(),
        uid: None,
        subject_filter: None,
    };

    let targeted = requested_mobile_uids(
        &conn,
        &parsed,
        &json!({ "message_ids": ["me@example.com#INBOX#t.dG9waWM#8"] }),
    )
    .unwrap();
    assert_eq!(targeted, vec![8]);

    let malformed =
        requested_mobile_uids(&conn, &parsed, &json!({ "message_ids": ["bad"] })).unwrap();
    assert!(malformed.is_empty());

    let fallback = requested_mobile_uids(&conn, &parsed, &json!({})).unwrap();
    assert_eq!(fallback, vec![7, 8]);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn parse_thread_id_round_trips_branch_root_containing_hash() {
    // '#' is legal atext in a Message-ID; the branch join must not truncate it.
    let compound = store::branch_compound_key("abc#123@host", "Topic");
    let id = format_thread_id("me@example.com", "INBOX", &compound);
    let parsed = parse_thread_id(&id).unwrap();
    assert_eq!(parsed.thread_key, "abc#123@host");
    assert_eq!(parsed.subject_filter.as_deref(), Some("Topic"));

    // Legacy unbranched ids keep resolving to the whole thread.
    let plain = parse_thread_id(&format_thread_id("me@example.com", "INBOX", "topic")).unwrap();
    assert_eq!(plain.thread_key, "topic");
    assert_eq!(plain.subject_filter, None);
}

#[test]
fn cached_thread_uids_uid_style_survives_thread_key_backfill() {
    let data_dir = unique_data_dir("uid-style-backfill");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    // The row was rendered as a uid-style card while its thread_key was empty,
    // then a later sync filled in the real key.
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 9,
            subject: "One".to_string(),
            thread_key: "real-key@host".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    let parsed = parse_thread_id("me@example.com#INBOX#9").unwrap();
    assert_eq!(parsed.uid, Some(9));
    assert_eq!(cached_thread_uids(&conn, &parsed).unwrap(), vec![9]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn update_mobile_read_state_scopes_branched_threads_to_their_uids() {
    let data_dir = unique_data_dir("branch-read-scope");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 1,
                subject: "Topic".to_string(),
                thread_key: "root@host".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "New topic".to_string(),
                thread_key: "root@host".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let parsed = ParsedThreadId {
        account: "me@example.com".to_string(),
        folder: "INBOX".to_string(),
        thread_key: "root@host".to_string(),
        uid: None,
        subject_filter: Some("New topic".to_string()),
    };
    let uids = cached_thread_uids(&conn, &parsed).unwrap();
    assert_eq!(uids, vec![2]);
    update_mobile_read_state(&conn, &parsed, &json!({}), &uids, true).unwrap();

    // Only the acted-on branch flips; the sibling branch keeps its unread state.
    let headers = store::get_thread_headers(&conn, "me@example.com", "INBOX", "root@host").unwrap();
    let seen_by_uid: std::collections::HashMap<u32, bool> = headers
        .iter()
        .map(|header| (header.uid, header.seen))
        .collect();
    assert!(seen_by_uid[&2]);
    assert!(!seen_by_uid[&1]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn parse_thread_id_with_request_folder_overrides_encoded_folder() {
    let parsed = parse_thread_id_with_request_folder(
        "me@example.com#INBOX#t.dG9waWM",
        &json!({ "folder": "Trash" }),
    )
    .unwrap();

    assert_eq!(parsed.account, "me@example.com");
    assert_eq!(parsed.folder, "Trash");
    assert_eq!(parsed.thread_key, "topic");
}
