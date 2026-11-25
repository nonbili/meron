use super::*;

#[test]
fn folders_round_trip_special_use_and_ensure_folder_keeps_it() {
    let conn = test_conn();
    upsert_folders(
        &conn,
        "acct",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
            Folder {
                name: "Mail/Entwürfe".to_string(),
                delimiter: Some("/".to_string()),
                special_use: Some("drafts".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    // Message syncs call ensure_folder on every pass; it must not clobber the
    // role recorded by the folder LIST sync.
    ensure_folder(&conn, "acct", "Mail/Entwürfe").unwrap();

    let folders = get_folders(&conn, "acct").unwrap();
    let drafts = folders.iter().find(|f| f.name == "Mail/Entwürfe").unwrap();
    assert_eq!(drafts.special_use.as_deref(), Some("drafts"));
    let inbox = folders.iter().find(|f| f.name == "INBOX").unwrap();
    assert_eq!(inbox.special_use, None);
}

#[test]
fn folder_unread_counts_messages_without_requiring_a_folder_row() {
    let conn = test_conn();
    insert_message(&conn, 1, "Unread", "Ada", "ada@example.com", None);
    conn.execute(
        "UPDATE messages SET seen = 1 WHERE account = 'acct' AND folder = 'INBOX' AND uid = 1",
        [],
    )
    .unwrap();
    insert_message(&conn, 2, "Unread", "Bea", "bea@example.com", None);

    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 1);
    assert_eq!(get_folder_unread(&conn, "acct", "Archive").unwrap(), 0);
}

#[test]
fn folder_unread_includes_unseen_server_mail_outside_the_cache() {
    let conn = test_conn();
    insert_message(&conn, 10, "Cached", "Ada", "ada@example.com", None);
    // The server reports 10 (cached) and 3..=5 (older, not cached) unseen.
    set_uncached_unseen(&conn, "acct", "INBOX", &[3, 4, 5, 10].into_iter().collect()).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 4);
    ensure_folder(&conn, "acct", "INBOX").unwrap();
    let folders = get_folders(&conn, "acct").unwrap();
    assert_eq!(
        folders.iter().find(|f| f.name == "INBOX").unwrap().unread,
        4
    );

    // Paging an older message into the cache doesn't count it twice, and
    // reading it there is reflected.
    insert_message(&conn, 4, "Paged in", "Bea", "bea@example.com", None);
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 4);
    update_message_seen(&conn, "acct", "INBOX", 4, true).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 3);

    let mut unseen = get_unseen_uids(&conn, "acct", "INBOX").unwrap();
    unseen.sort_unstable();
    assert_eq!(unseen, vec![3, 5, 10]);
    mark_folder_seen(&conn, "acct", "INBOX", true).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 0);
}

#[test]
fn a_stale_unseen_snapshot_cannot_restore_an_archived_uid_to_unread_counts() {
    let conn = test_conn();
    insert_message(&conn, 7, "Archive me", "Ada", "ada@example.com", None);
    let (sync, fetched_at) = begin_message_sync(&conn).unwrap();
    let stale_unseen = [3, 7].into_iter().collect();
    delete_messages_by_uid(&conn, "acct", "INBOX", &[7]).unwrap();
    release_removed_messages(&conn, "acct", "INBOX", &[7], fetched_at).unwrap();
    set_uncached_unseen(&conn, "acct", "INBOX", &stale_unseen).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 1);
    assert_eq!(get_unseen_uids(&conn, "acct", "INBOX").unwrap(), vec![3]);
    // Removal markers are scoped to their source mailbox and account.
    set_uncached_unseen(&conn, "acct", "Archive", &stale_unseen).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "Archive").unwrap(), 2);
    set_uncached_unseen(&conn, "other", "INBOX", &stale_unseen).unwrap();
    assert_eq!(get_folder_unread(&conn, "other", "INBOX").unwrap(), 2);
    end_message_sync(&conn, sync).unwrap();
    // A later valid unseen read still counts ordinary uncached mail.
    set_uncached_unseen(&conn, "acct", "INBOX", &[3, 8].into_iter().collect()).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 2);
}

#[test]
fn removing_a_paged_in_message_does_not_revive_its_uncached_unread() {
    let conn = test_conn();
    set_uncached_unseen(&conn, "acct", "INBOX", &[4].into_iter().collect()).unwrap();
    insert_message(&conn, 4, "Paged in", "Bea", "bea@example.com", None);
    update_message_seen(&conn, "acct", "INBOX", 4, true).unwrap();
    conn.execute(
        "DELETE FROM messages WHERE account = 'acct' AND folder = 'INBOX' AND uid = 4",
        [],
    )
    .unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 0);
}

#[test]
fn moving_a_cached_message_onto_an_uncached_unseen_uid_counts_it_once() {
    let conn = test_conn();
    insert_message(&conn, 4, "Moved", "Bea", "bea@example.com", None);
    set_uncached_unseen(&conn, "acct", "Archive", &[4].into_iter().collect()).unwrap();
    move_messages_by_uid(&conn, "acct", "INBOX", "Archive", &[4]).unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "Archive").unwrap(), 1);
}

#[test]
fn emptying_a_folder_clears_its_uncached_unread() {
    let conn = test_conn();
    set_uncached_unseen(&conn, "acct", "INBOX", &[3, 5].into_iter().collect()).unwrap();
    delete_folder_messages(&conn, "acct", "INBOX").unwrap();
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 0);
}

#[test]
fn delete_folder_drops_the_row_its_messages_and_its_sync_state() {
    let conn = test_conn();
    upsert_folders(
        &conn,
        "acct",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
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
        ],
    )
    .unwrap();
    insert_message(&conn, 1, "Kept", "Ada", "ada@example.com", None);
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen)
         VALUES('acct', 'Work/Reports', '7', 7, 'Q3', 1779580800, 0)",
        [],
    )
    .unwrap();
    set_folder_state(&conn, "acct", "Work/Reports", 1, 8).unwrap();

    // Nesting is read off each row's own delimiter, so the parent knows it has
    // children while the leaf has none.
    assert_eq!(
        child_folders(&conn, "acct", "Work").unwrap(),
        vec!["Work/Reports".to_string()]
    );
    assert!(
        child_folders(&conn, "acct", "Work/Reports")
            .unwrap()
            .is_empty()
    );

    assert_eq!(delete_folder(&conn, "acct", "Work/Reports").unwrap(), 1);
    let names: Vec<String> = get_folders(&conn, "acct")
        .unwrap()
        .into_iter()
        .map(|folder| folder.name)
        .collect();
    assert_eq!(names, vec!["INBOX".to_string(), "Work".to_string()]);
    assert_eq!(
        get_folder_state(&conn, "acct", "Work/Reports").unwrap(),
        None
    );
    // Untouched folders keep their mail.
    assert_eq!(get_folder_unread(&conn, "acct", "INBOX").unwrap(), 1);
}

#[test]
fn delimiterless_folder_names_do_not_form_a_delete_subtree() {
    let conn = test_conn();
    upsert_folders(
        &conn,
        "acct",
        &[
            Folder {
                name: "Work".to_string(),
                delimiter: None,
                ..Default::default()
            },
            Folder {
                name: "Work/Reports".to_string(),
                delimiter: None,
                ..Default::default()
            },
        ],
    )
    .unwrap();

    assert!(child_folders(&conn, "acct", "Work").unwrap().is_empty());
}

#[test]
fn folder_role_assignment_uses_special_use_then_name_fallback() {
    let cases = [
        ("Mail/Entwürfe", Some("drafts"), "drafts"),
        ("Archive", Some("sent"), "sent"),
        ("INBOX", None, "inbox"),
        ("Sent Mail", None, "sent"),
        ("Drafts", None, "drafts"),
        ("Deleted Items", None, "trash"),
        ("Spam", None, "junk"),
        ("All Mail", None, "archive"),
        ("Projects", None, "folder"),
    ];
    for (name, special_use, role) in cases {
        assert_eq!(classify_folder_role(name, special_use), role, "{name}");
    }
}

#[test]
fn only_the_folder_delete_empties_in_place_holds_the_trash_role() {
    let folder = |name: &str, special_use: Option<&str>| Folder {
        name: name.to_string(),
        special_use: special_use.map(str::to_string),
        ..Default::default()
    };

    // The server flags Gmail's Trash: a plain "Trash" beside it is a folder
    // like any other, where delete moves mail to "[Gmail]/Trash".
    let conn = test_conn();
    upsert_folders(
        &conn,
        "gmail",
        &[
            folder("Trash", None),
            folder("[Gmail]/Trash", Some("trash")),
            folder("Projects/Trash", None),
        ],
    )
    .unwrap();
    assert_eq!(
        folder_role(&conn, "gmail", "[Gmail]/Trash").unwrap(),
        "trash"
    );
    assert_eq!(folder_role(&conn, "gmail", "Trash").unwrap(), "folder");
    assert_eq!(
        folder_role(&conn, "gmail", "Projects/Trash").unwrap(),
        "folder"
    );
    assert_eq!(
        folder_for_role(&conn, "gmail", "trash").unwrap().as_deref(),
        Some("[Gmail]/Trash")
    );
    let roles: BTreeMap<String, String> = get_folders(&conn, "gmail")
        .unwrap()
        .into_iter()
        .map(|f| (f.name, f.role))
        .collect();
    assert_eq!(roles["Trash"], "folder");
    assert_eq!(roles["[Gmail]/Trash"], "trash");

    // No flags (the server lacks SPECIAL-USE): names decide, localized ones
    // included, and of two look-alikes the first by name holds the role.
    upsert_folders(
        &conn,
        "plain",
        &[
            folder("Trash", None),
            folder("Papierkorb", None),
            folder("INBOX", None),
        ],
    )
    .unwrap();
    assert_eq!(folder_role(&conn, "plain", "Papierkorb").unwrap(), "trash");
    assert_eq!(folder_role(&conn, "plain", "Trash").unwrap(), "folder");

    // Nothing cached for the account: the name alone stands.
    assert_eq!(folder_role(&conn, "fresh", "Trash").unwrap(), "trash");

    let folders = [
        folder("Drafts", None),
        folder("Entw&APw-rfe", Some("drafts")),
        folder("Trash", None),
    ];
    assert_eq!(role_folder(&folders, "drafts"), Some("Entw&APw-rfe"));
    assert_eq!(role_folder(&folders, "trash"), Some("Trash"));
    assert_eq!(resolve_folder_role("Drafts", None, &folders), "folder");
}

#[test]
fn draft_thread_keys_detects_special_use_and_name_fallback() {
    let conn = test_conn();
    upsert_folders(
        &conn,
        "acct",
        &[
            Folder {
                name: "Mail/Entwürfe".to_string(),
                special_use: Some("drafts".to_string()),
                ..Default::default()
            },
            Folder {
                name: "[Gmail]/Drafts".to_string(),
                ..Default::default()
            },
            Folder {
                name: "INBOX".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, thread_key)
         VALUES('acct', 'Mail/Entwürfe', '1', 1, 'draft', 'root@h'),
               ('acct', '[Gmail]/Drafts', '2', 2, 'draft', 'other@h'),
               ('acct', 'INBOX', '3', 3, 'not draft', 'inbox@h'),
               ('acct', 'Mail/Entwürfe', '4', 7, 'unthreaded draft', '')",
        [],
    )
    .unwrap();

    let keys = draft_thread_keys(&conn, "acct").unwrap();
    assert!(keys.contains("root@h"));
    assert!(keys.contains("other@h"));
    assert!(!keys.contains("inbox@h"));
    // A draft with no threading headers falls back to its own UID, which is only
    // unique within its mailbox — badging every `uid:7` card in the account with
    // it would mark unrelated mail as having a draft.
    assert!(!keys.contains("uid:7"));
    assert!(!keys.contains(""));
}

#[test]
fn folder_notify_opt_in_survives_folder_list_sync() {
    let conn = test_conn();
    let folder = Folder {
        name: "Work".into(),
        display_name: "Work".into(),
        ..Default::default()
    };
    upsert_folders(&conn, "acct", std::slice::from_ref(&folder)).unwrap();
    ensure_folder(&conn, "acct", "INBOX").unwrap();
    assert!(folder_notifies(&conn, "acct", "inbox").unwrap());
    assert!(!folder_notifies(&conn, "acct", "Work").unwrap());
    assert!(!set_folder_notify(&conn, "acct", "Missing", true).unwrap());

    assert!(set_folder_notify(&conn, "acct", "Work", true).unwrap());
    // A later LIST sync rewrites the row's server-owned columns only.
    upsert_folders(&conn, "acct", &[folder]).unwrap();
    assert!(folder_notifies(&conn, "acct", "Work").unwrap());
    assert_eq!(notify_folders(&conn, "acct").unwrap(), vec!["Work"]);
    let listed = get_folders(&conn, "acct").unwrap();
    assert!(listed.iter().any(|f| f.name == "Work" && f.notify));
    assert!(listed.iter().any(|f| f.name == "INBOX" && !f.notify));

    assert!(set_folder_notify(&conn, "acct", "Work", false).unwrap());
    assert!(notify_folders(&conn, "acct").unwrap().is_empty());
}

#[test]
fn notify_plan_gives_idle_slots_to_the_earliest_opt_ins() {
    let conn = test_conn();
    // Opted in out of name order: slots follow the opt-in order, not the name.
    let names = ["Zeta", "Beta", "Mid", "Alpha", "Omega"];
    for name in names {
        ensure_folder(&conn, "acct", name).unwrap();
        assert!(set_folder_notify(&conn, "acct", name, true).unwrap());
    }
    let plan = notify_folder_plan(&conn, "acct").unwrap();
    assert_eq!(plan.live, vec!["Zeta", "Beta", "Mid"]);
    assert_eq!(plan.polled, vec!["Alpha", "Omega"]);
    assert!(folder_notifies_live(&conn, "acct", "Mid").unwrap());
    assert!(!folder_notifies_live(&conn, "acct", "Alpha").unwrap());

    // Enabling again keeps a folder's place; a later opt-in never demotes it.
    assert!(set_folder_notify(&conn, "acct", "Zeta", true).unwrap());
    assert_eq!(notify_folder_plan(&conn, "acct").unwrap(), plan);

    // Freeing a slot promotes the longest-waiting polled folder.
    assert!(set_folder_notify(&conn, "acct", "Beta", false).unwrap());
    let plan = notify_folder_plan(&conn, "acct").unwrap();
    assert_eq!(plan.live, vec!["Zeta", "Mid", "Alpha"]);
    assert_eq!(plan.polled, vec!["Omega"]);
    assert!(
        get_folders(&conn, "acct")
            .unwrap()
            .iter()
            .all(|f| f.notify == (f.name != "Beta"))
    );
}

#[test]
fn rename_folder_keeps_the_rows_and_drops_the_cache() {
    let conn = test_conn();
    upsert_folders(
        &conn,
        "acct",
        &[
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
                name: "Workshop".to_string(),
                delimiter: Some("/".to_string()),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    set_folder_notify(&conn, "acct", "Work/Reports", true).unwrap();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen)
         VALUES('acct', 'Work/Reports', '7', 7, 'Q3', 1779580800, 0)",
        [],
    )
    .unwrap();
    set_folder_state(&conn, "acct", "Work/Reports", 1, 8).unwrap();

    let pairs = folder_rename_pairs(&conn, "acct", "Work", "Jobs").unwrap();
    assert_eq!(
        pairs,
        vec![
            ("Work".to_string(), "Jobs".to_string()),
            ("Work/Reports".to_string(), "Jobs/Reports".to_string()),
        ]
    );
    rename_folder(&conn, "acct", &pairs).unwrap();
    let folders = get_folders(&conn, "acct").unwrap();
    assert_eq!(
        folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Jobs", "Jobs/Reports", "Workshop"]
    );
    let reports = folders
        .iter()
        .find(|folder| folder.name == "Jobs/Reports")
        .unwrap();
    assert!(reports.notify);
    assert_eq!(reports.delimiter.as_deref(), Some("/"));
    assert_eq!(
        get_folder_state(&conn, "acct", "Work/Reports").unwrap(),
        None
    );
    assert_eq!(get_folder_unread(&conn, "acct", "Work/Reports").unwrap(), 0);
}
