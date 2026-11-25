use super::*;

#[test]
fn mobile_protocol_move_requires_usable_account_secret() {
    let data_dir = unique_data_dir("move-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "Move me".to_string(),
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":67,"method":"mail.move","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_folder_id":"Work"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 67);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    // INBOX in another account must not be mistaken for the source INBOX.
    let cross_account = invoke_mobile_protocol_json(
        r#"{"id":69,"method":"mail.move","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_account_id":"other@example.com","target_folder_id":"INBOX"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        cross_account["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_copy_requires_usable_account_secret() {
    let data_dir = unique_data_dir("copy-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");
    seed_mobile_account(&data_dir, "target@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "target@example.com", "Archive").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "Copy me".to_string(),
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":68,"method":"mail.copy","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_account_id":"target@example.com","target_folder_id":"Archive"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 68);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_flag_actions_leave_the_store_alone_when_the_server_write_fails() {
    let data_dir = unique_data_dir("thread-actions");
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
                subject: "First".to_string(),
                seen: false,
                starred: false,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 11,
                subject: "Second".to_string(),
                seen: false,
                starred: false,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let read = invoke_mobile_protocol_json(
        r#"{"id":68,"method":"mail.markRead","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","seen":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["id"], 68);
    assert!(
        read["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let starred = invoke_mobile_protocol_json(
        r#"{"id":69,"method":"mail.markStarred","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","starred":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(starred["id"], 69);
    assert!(
        starred["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let mark_all = invoke_mobile_protocol_json(
        r#"{"id":72,"method":"mail.markAllRead","params":{"account_id":"me@example.com","folder_id":"INBOX"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(mark_all["id"], 72);
    assert!(
        mark_all["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    // Emptying is gated on the folder role, before any account/network work:
    // INBOX is refused outright, while Trash gets as far as the creds check.
    let empty_inbox = invoke_mobile_protocol_json(
        r#"{"id":73,"method":"mail.emptyFolder","params":{"account_id":"me@example.com","folder_id":"INBOX"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(empty_inbox["id"], 73);
    assert!(
        empty_inbox["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Only Trash and Junk folders can be emptied")
    );

    let empty_trash = invoke_mobile_protocol_json(
        r#"{"id":74,"method":"mail.emptyFolder","params":{"account_id":"me@example.com","folder_id":"Trash"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(empty_trash["id"], 74);
    assert!(
        empty_trash["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let threads = invoke_mobile_protocol_json(
        r#"{"id":70,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let first = &threads["result"]["threads"][0];
    assert_eq!(
        first["thread_id"],
        "me@example.com#INBOX#t.dG9waWMjU2Vjb25k"
    );
    // None of the writes reached the server, so none reached the store.
    assert_eq!(first["unread"], true);
    assert_eq!(first["unread_count"], 1);
    assert_eq!(first["starred"], false);

    let unread = invoke_mobile_protocol_json(
        r#"{"id":71,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"unread"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(unread["result"]["threads"].as_array().unwrap().len(), 2);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_keeps_uid_thread_unread_when_server_write_fails() {
    let data_dir = unique_data_dir("uid-thread-actions");
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
        r#"{"id":72,"method":"mail.markRead","params":{"thread_id":"me@example.com#Drafts#4","seen":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let read = invoke_mobile_protocol_json(
        r#"{"id":73,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    // The server never took the change, so the store must not claim it did.
    assert_eq!(read["result"]["messages"][0]["unread"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_delete_requires_server_credentials() {
    let data_dir = unique_data_dir("thread-delete");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 20,
                subject: "Delete me".to_string(),
                date: 300,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 21,
                subject: "Delete me too".to_string(),
                date: 200,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 22,
                subject: "Keep me".to_string(),
                date: 100,
                thread_key: "other".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":75,"method":"mail.delete","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 75);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let threads = invoke_mobile_protocol_json(
        r#"{"id":76,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let rows = threads["result"]["threads"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["subject"], "Delete me");
    assert_eq!(rows[1]["subject"], "Delete me");
    assert_eq!(rows[2]["subject"], "Keep me");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_delete_uid_thread_requires_server_credentials() {
    let data_dir = unique_data_dir("uid-thread-delete");
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
        r#"{"id":77,"method":"mail.delete","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let read = invoke_mobile_protocol_json(
        r#"{"id":78,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["result"]["messages"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_archive_requires_server_credentials() {
    let data_dir = unique_data_dir("thread-archive");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Archive".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 31,
                subject: "Archive me".to_string(),
                date: 300,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 32,
                subject: "Archive me too".to_string(),
                date: 200,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 33,
                subject: "Keep me".to_string(),
                date: 100,
                thread_key: "other".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":79,"method":"mail.archive","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 79);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let inbox = invoke_mobile_protocol_json(
        r#"{"id":80,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let inbox_rows = inbox["result"]["threads"].as_array().unwrap();
    assert_eq!(inbox_rows.len(), 3);
    assert_eq!(inbox_rows[0]["subject"], "Archive me");
    assert_eq!(inbox_rows[1]["subject"], "Archive me");
    assert_eq!(inbox_rows[2]["subject"], "Keep me");

    let archive = invoke_mobile_protocol_json(
        r#"{"id":81,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"Archive","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let archive_rows = archive["result"]["threads"].as_array().unwrap();
    assert_eq!(archive_rows.len(), 0);

    let _ = std::fs::remove_dir_all(data_dir);
}

/// An account with Inbox and Archive folders, and thread "topic" in Archive.
fn seed_archived_topic(label: &str) -> std::path::PathBuf {
    let data_dir = unique_data_dir(label);
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Archive".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "Archive",
        &[MessageHeader {
            uid: 7,
            subject: "Archived".to_string(),
            date: 100,
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    data_dir
}

#[test]
fn mobile_protocol_archive_in_the_archive_folder_moves_the_inbox_copies() {
    let data_dir = seed_archived_topic("thread-archive-in-place");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 3,
            subject: "Reply".to_string(),
            date: 200,
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    // The Inbox copy was found and is being moved: with no password the move
    // stops at the server, where it used to answer "moved: 0" without trying.
    let value = invoke_mobile_protocol_json(
        r#"{"id":83,"method":"mail.archive","params":{"thread_id":"me@example.com#Archive#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect"),
        "{value}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_archive_in_the_archive_folder_without_inbox_copy_is_already_archived() {
    let data_dir = seed_archived_topic("thread-archive-in-place-none");

    let value = invoke_mobile_protocol_json(
        r#"{"id":84,"method":"mail.archive","params":{"thread_id":"me@example.com#Archive#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["result"]["already_archived"], true, "{value}");
    assert_eq!(value["result"]["moved"], 0);
    assert_eq!(value["result"]["folder"], "Archive");
    assert_eq!(
        value["result"]["thread_id"],
        "me@example.com#Archive#t.dG9waWM"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_archive_never_reads_an_archive_uid_in_the_inbox() {
    let data_dir = seed_archived_topic("thread-archive-in-place-uid");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    // An unrelated Inbox message that happens to share the Archive UID.
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "Unrelated".to_string(),
            thread_key: "other".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":85,"method":"mail.archive","params":{"thread_id":"me@example.com#Archive#7"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["result"]["already_archived"], true, "{value}");
    assert_eq!(value["result"]["thread_id"], "me@example.com#Archive#7");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_archive_requires_cached_archive_folder() {
    let data_dir = unique_data_dir("thread-archive-missing-folder");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 41,
            subject: "Archive me".to_string(),
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":82,"method":"mail.archive","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 82);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Archive folder not found")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_move_honors_explicit_message_ids() {
    // An undo moves back the copies an archive reported, by UID. Those rows
    // need not be cached in the archive folder, and a uid: thread key names
    // nothing there at all — so the explicit ids must be used as given rather
    // than resolved through the cached thread.
    let data_dir = unique_data_dir("move-explicit-ids");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "Archive").unwrap();
    drop(conn);

    let without_ids = invoke_mobile_protocol_json(
        r#"{"id":68,"method":"mail.move","params":{"thread_id":"me@example.com#Archive#t.dG9waWM","target_folder_id":"INBOX"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(without_ids["result"]["moved"], 0);

    // Past UID resolution to the server write, which this account can't make.
    let with_ids = invoke_mobile_protocol_json(
        r#"{"id":69,"method":"mail.move","params":{"thread_id":"me@example.com#Archive#t.dG9waWM","target_folder_id":"INBOX","message_ids":["42"]}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        with_ids["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect"),
        "{with_ids}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}
