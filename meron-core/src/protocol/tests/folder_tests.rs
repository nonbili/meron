use super::*;

#[test]
fn mobile_protocol_lists_folders_from_store() {
    let data_dir = unique_data_dir("folders");
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
            Folder {
                name: "Quarantine".to_string(),
                special_use: Some("junk".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Unread".to_string(),
            seen: false,
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":63,"method":"mail.folderList","params":{"account_id":"me@example.com"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 63);
    assert_eq!(value["result"]["folders"][0]["id"], "Archive");
    assert_eq!(value["result"]["folders"][0]["role"], "archive");
    assert_eq!(value["result"]["folders"][1]["id"], "INBOX");
    assert_eq!(value["result"]["folders"][1]["role"], "inbox");
    assert_eq!(value["result"]["folders"][1]["unread"], 1);
    assert_eq!(value["result"]["folders"][0]["notify"], false);
    assert_eq!(value["result"]["folders"][2]["id"], "Quarantine");
    assert_eq!(value["result"]["folders"][2]["special_use"], "junk");

    // Opting a folder in answers with the refreshed list.
    let value = invoke_mobile_protocol_json(
        r#"{"id":64,"method":"mail.folderSetNotify","params":{"account_id":"me@example.com","folder_id":"Archive","enabled":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["result"]["folders"][0]["id"], "Archive");
    assert_eq!(value["result"]["folders"][0]["notify"], true);
    assert_eq!(value["result"]["folders"][0]["notify_live"], true);
    assert_eq!(value["result"]["folders"][1]["notify_live"], false);
    assert_eq!(value["result"]["folders"][1]["notify"], false);
    let missing = invoke_mobile_protocol_json(
        r#"{"id":65,"method":"mail.folderSetNotify","params":{"account_id":"me@example.com","folder_id":"Nope","enabled":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Folder not found")
    );

    // Arrivals in the opted-in folder are described under its own name, with
    // snippets read from that folder's cache.
    let arrival = MessageHeader {
        uid: 9,
        subject: "Filed by a rule".to_string(),
        thread_key: "rule".to_string(),
        ..Default::default()
    };
    let detail = crate::ffi::mobile_new_messages_detail(
        data_dir.to_str().unwrap(),
        "me@example.com",
        "Archive",
        &[arrival],
    )
    .unwrap();
    assert_eq!(detail["folder"], "Archive");
    assert_eq!(detail["folderName"], "Archive");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_folder_create_requires_usable_account_secret() {
    let data_dir = unique_data_dir("folder-create-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");

    let value = invoke_mobile_protocol_json(
        r#"{"id":66,"method":"mail.folderCreate","params":{"account_id":"me@example.com","name":"Work"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 66);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_folder_delete_refuses_special_use_anywhere_in_the_subtree() {
    let data_dir = unique_data_dir("folder-delete-gate");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                special_use: Some("inbox".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Work".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Work/Reports".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Receipts".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Mail".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Mail/Junk".to_string(),
                delimiter: Some("/".to_string()),
                special_use: Some("junk".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    // Special-use folders are refused before any account/network work.
    let inbox = invoke_mobile_protocol_json(
        r#"{"id":80,"method":"mail.folderDelete","params":{"account_id":"me@example.com","folder_id":"INBOX"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(inbox["id"], 80);
    assert!(
        inbox["error"]["message"]
            .as_str()
            .unwrap()
            .contains("special folder")
    );

    // So is one nested under an ordinary parent: the parent's delete would take
    // the whole subtree with it.
    let parent = invoke_mobile_protocol_json(
        r#"{"id":81,"method":"mail.folderDelete","params":{"account_id":"me@example.com","folder_id":"Mail"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        parent["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Mail/Junk is a special folder")
    );

    // An ordinary parent passes the gate and gets as far as the creds check —
    // nesting alone no longer blocks the delete.
    let nested = invoke_mobile_protocol_json(
        r#"{"id":83,"method":"mail.folderDelete","params":{"account_id":"me@example.com","folder_id":"Work"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        nested["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    // An ordinary leaf folder passes the gate and gets as far as the creds check.
    let leaf = invoke_mobile_protocol_json(
        r#"{"id":82,"method":"mail.folderDelete","params":{"account_id":"me@example.com","folder_id":"Receipts"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        leaf["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_folder_rename_refuses_bad_targets_before_the_server() {
    let data_dir = unique_data_dir("folder-rename-gate");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                special_use: Some("inbox".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Work".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Work/Reports".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Receipts".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);
    let rename = |id: u32, folder: &str, name: &str| {
        invoke_mobile_protocol_json(
            &json!({
                "id": id,
                "method": "mail.folderRename",
                "params": { "account_id": "me@example.com", "folder_id": folder, "name": name },
            })
            .to_string(),
            Some(data_dir.to_str().unwrap()),
        )
    };
    let error = |value: &Value| {
        value["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };

    assert!(error(&rename(90, "INBOX", "Elsewhere")).contains("special folder"));
    assert!(error(&rename(91, "Work", " ")).contains("Folder name is required"));
    assert!(error(&rename(92, "Work", "Work/Reports/Old")).contains("inside itself"));
    assert!(error(&rename(93, "Work", "Receipts")).contains("already exists"));
    // Unchanged is answered locally, with nothing renamed.
    let unchanged = rename(94, "Work", "Work");
    assert_eq!(unchanged["result"]["renamed"], json!([]));
    // An ordinary folder passes the gates and gets as far as the creds check.
    assert!(error(&rename(95, "Work", "Jobs")).contains("account needs reconnect"));

    let _ = std::fs::remove_dir_all(data_dir);
}
