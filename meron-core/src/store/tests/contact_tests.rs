use super::*;

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
fn contact_history_matches_exact_participants_across_accounts_and_folders() {
    let conn = test_conn();
    let message = |uid, from: &str, to: &str, cc: &str, date| crate::imap::MessageHeader {
        uid,
        subject: "A topic".into(),
        from_addr: from.into(),
        to: vec![crate::imap::Recipient {
            name: "Person".into(),
            addr: to.into(),
        }],
        cc: vec![crate::imap::Recipient {
            name: String::new(),
            addr: cc.into(),
        }],
        date,
        thread_key: format!("mid:{uid}"),
        message_id: format!("{uid}@example.com"),
        ..Default::default()
    };
    upsert_messages(
        &conn,
        "acct",
        "Archive",
        &[message(1, "Alice@example.com", "me@example.com", "", 10)],
    )
    .unwrap();
    upsert_messages(
        &conn,
        "other",
        "Sent",
        &[message(2, "me@example.com", "alice@example.com", "", 20)],
    )
    .unwrap();
    upsert_messages(
        &conn,
        "acct",
        "Projects",
        &[message(
            3,
            "bob@example.com",
            "me@example.com",
            "ALICE@example.com",
            30,
        )],
    )
    .unwrap();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[message(4, "notalice@example.com", "me@example.com", "", 40)],
    )
    .unwrap();
    let hits = contact_messages(&conn, " alice@example.com ").unwrap();
    assert_eq!(
        hits.iter().map(|(_, h)| h.uid).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(hits[0].1.folder, "Projects");
    assert_eq!(hits[1].0, "other");
    assert_eq!(hits[1].1.folder, "Sent");
    assert_eq!(hits[0].1.message_id, "3@example.com");
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[message(8, "Élodie@x.com", "me@example.com", "", 1)],
    )
    .unwrap();
    assert_eq!(contact_messages(&conn, "élodie@x.com").unwrap().len(), 1);
    upsert_messages(
        &conn,
        "acct",
        "Sent",
        &[message(9, "me@example.com", "Élodie@x.com", "", 2)],
    )
    .unwrap();
    assert_eq!(contact_messages(&conn, "élodie@x.com").unwrap().len(), 2);
    let first = contact_thread_page(&conn, "alice@example.com", None, 1).unwrap();
    assert_eq!(first["threads"].as_array().unwrap().len(), 1);
    assert!(
        first["threads"][0]["thread_id"]
            .as_str()
            .unwrap()
            .contains("#")
    );
    let cursor = first["next_cursor"].as_str().unwrap();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[message(5, "alice@example.com", "me@example.com", "", 50)],
    )
    .unwrap();
    let second = contact_thread_page(&conn, "alice@example.com", Some(cursor), 1).unwrap();
    assert_ne!(
        first["threads"][0]["thread_id"],
        second["threads"][0]["thread_id"]
    );
    assert_eq!(second["threads"][0]["date"], 20);
    // That card's only message is the user's own Sent copy: it still shows the
    // correspondent, not the user.
    assert_eq!(second["threads"][0]["from_addr"], "alice@example.com");
    assert!(
        contact_messages(&conn, "nobody@example.com")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn contacts_exclude_junk_and_trash_using_live_folder_roles() {
    let conn = test_conn();
    let folder = |name: &str, role: &str| Folder {
        name: name.into(),
        special_use: Some(role.into()),
        ..Default::default()
    };
    upsert_folders(
        &conn,
        "acct",
        &[
            folder("迷惑メール", "junk"),
            folder("Deleted items custom", "trash"),
            folder("Spam", LOCAL_SPECIAL_USE),
        ],
    )
    .unwrap();
    let header = |uid, addr: &str, date| crate::imap::MessageHeader {
        uid,
        from_addr: addr.into(),
        date,
        subject: addr.into(),
        thread_key: format!("mid:{uid}"),
        message_id: format!("{uid}@example.com"),
        ..Default::default()
    };
    for (name, message) in [
        ("INBOX", header(1, "alice@example.com", 10)),
        ("迷惑メール", header(2, "alice@example.com", 100)),
        (
            "Deleted items custom",
            header(3, "deleted@example.com", 200),
        ),
        ("Junk", header(4, "junkonly@example.com", 300)),
        ("Spam", header(5, "local@example.com", 20)),
        ("Projects", header(6, "bob@example.com", 30)),
    ] {
        upsert_messages(&conn, "acct", name, &[message]).unwrap();
    }
    let addresses = |sort| {
        list_contacts(&conn, "", "", 20, sort, true)
            .unwrap()
            .into_iter()
            .map(|c| c.addr)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        addresses(ContactSort::Recent),
        vec!["bob@example.com", "local@example.com", "alice@example.com"]
    );
    assert_eq!(
        contact_messages(&conn, "alice@example.com").unwrap().len(),
        1
    );
    assert!(
        contact_messages(&conn, "deleted@example.com")
            .unwrap()
            .is_empty()
    );
    assert!(
        contact_messages(&conn, "junkonly@example.com")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        contact_messages(&conn, "local@example.com").unwrap().len(),
        1
    );
    assert!(
        list_contacts(&conn, "", "deleted@", 20, ContactSort::Name, true)
            .unwrap()
            .is_empty()
    );
    // Metadata arrives after mail caching or is corrected by the provider.
    upsert_folders(&conn, "acct", &[folder("Projects", "trash")]).unwrap();
    assert!(!addresses(ContactSort::Recent).contains(&"bob@example.com".to_string()));
    assert!(
        contact_thread_page(&conn, "bob@example.com", None, 50).unwrap()["threads"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    upsert_folders(&conn, "acct", &[folder("Projects", LOCAL_SPECIAL_USE)]).unwrap();
    assert!(addresses(ContactSort::Recent).contains(&"bob@example.com".to_string()));
}

#[test]
fn list_contacts_sorts_by_frequency_name_or_recency() {
    let conn = test_conn();
    let at = |uid: u32, name: &str, addr: &str, date: i64| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen)
             VALUES('acct', 'INBOX', ?1, ?2, 's', ?3, ?4, ?5, 0)",
            params![uid.to_string(), uid, name, addr, date],
        )
        .unwrap();
    };
    // Zed writes most, Amy most recently, and bob@ has no name.
    at(1, "Zed", "zed@example.com", 100);
    at(2, "Zed", "zed@example.com", 200);
    at(3, "Zed", "zed@example.com", 300);
    at(4, "Amy", "amy@example.com", 900);
    at(5, "", "bob@example.com", 500);
    let order = |sort, query: &str| {
        list_contacts(&conn, "acct", query, 10, sort, false)
            .unwrap()
            .into_iter()
            .map(|c| c.addr)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        order(ContactSort::Frequent, ""),
        ["zed@example.com", "amy@example.com", "bob@example.com"]
    );
    assert_eq!(
        order(ContactSort::Name, ""),
        ["amy@example.com", "bob@example.com", "zed@example.com"]
    );
    assert_eq!(
        order(ContactSort::Recent, ""),
        ["amy@example.com", "bob@example.com", "zed@example.com"]
    );
    // A search keeps the chosen order rather than ranking by match.
    assert_eq!(
        order(ContactSort::Recent, "e"),
        ["amy@example.com", "bob@example.com", "zed@example.com"]
    );
    // A message re-dated later moves its sender up.
    conn.execute("UPDATE messages SET date = 1000 WHERE uid = 5", [])
        .unwrap();
    assert_eq!(order(ContactSort::Recent, "")[0], "bob@example.com");
    assert_eq!(ContactSort::parse("bogus"), ContactSort::Frequent);
}

#[test]
fn contact_directory_hides_own_and_automated_addresses() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO accounts(id, email, prefs)
         VALUES('acct', 'Me@example.com', '{\"aliases\":[{\"email\":\"alias@example.com\"}]}')",
        [],
    )
    .unwrap();
    insert_message(&conn, 1, "a", "Me", "me@example.com", None);
    insert_message(&conn, 2, "b", "Alias", "alias@example.com", None);
    insert_message(
        &conn,
        3,
        "c",
        "Microsoft",
        "account-security-noreply@example.com",
        None,
    );
    insert_message(&conn, 4, "d", "Proton", "no-reply@notify.example.com", None);
    insert_message(&conn, 5, "e", "Bounces", "MAILER-DAEMON@example.com", None);
    insert_message(&conn, 6, "f", "Aki", "aki@example.com", None);
    let addrs = |query: &str, directory| {
        let mut addrs = list_contacts(&conn, "acct", query, 10, ContactSort::Name, directory)
            .unwrap()
            .into_iter()
            .map(|c| c.addr.to_lowercase())
            .collect::<Vec<_>>();
        addrs.sort();
        addrs
    };
    assert_eq!(addrs("", true), ["aki@example.com"]);
    // A search still finds automated senders, never the user's own addresses.
    assert_eq!(
        addrs("example", true),
        [
            "account-security-noreply@example.com",
            "aki@example.com",
            "mailer-daemon@example.com",
            "no-reply@notify.example.com"
        ]
    );
    // Autocomplete keeps everything, so mailing yourself still works.
    assert_eq!(addrs("", false).len(), 6);
}

#[test]
fn contact_directory_hides_no_reply_first_domain_label() {
    let conn = test_conn();
    let hidden = [
        "comment@noreply.github.com",
        "author@NO-REPLY.github.com",
        "subscribed@do_not_reply.example.com",
        "meron@donotreply.github.com",
    ];
    let visible = [
        "person@notifications.example.com",
        "person@bounce.example.com",
        "person@mail.noreply.example.com",
        "person@github.com",
    ];
    for (index, addr) in hidden.iter().chain(visible.iter()).enumerate() {
        insert_message(&conn, index as u32 + 1, addr, "Sender", addr, None);
    }
    let directory = list_contacts(&conn, "acct", "", 20, ContactSort::Name, true).unwrap();
    let mut addresses: Vec<_> = directory.iter().map(|c| c.addr.as_str()).collect();
    addresses.sort();
    let mut expected = visible.to_vec();
    expected.sort();
    assert_eq!(addresses, expected);

    // Hidden domain-based senders remain searchable and available to compose.
    for addr in hidden {
        let found = list_contacts(&conn, "acct", addr, 20, ContactSort::Name, true).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].addr, addr);
    }
    assert_eq!(suggest_contacts(&conn, "acct", "", 20).unwrap().len(), 8);
}

#[test]
fn suggest_contacts_includes_to_and_cc_recipients() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    // A sent message we authored: the useful contacts are its To/Cc, not us.
    let msg = MessageHeader {
        uid: 10,
        subject: "Lunch".into(),
        from_name: "Me".into(),
        from_addr: "me@example.com".into(),
        date: 1_779_580_800, // 2026-05-24
        to: vec![crate::imap::Recipient {
            name: "Cleo".into(),
            addr: "cleo@example.com".into(),
        }],
        cc: vec![crate::imap::Recipient {
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
        // Sent-folder row from an *unconfigured* alias (e.g. webmail send-as):
        // folder provenance alone marks it outbound.
        MessageHeader {
            uid: 5,
            folder: "[Gmail]/Sent Mail".into(),
            from_name: "Me".into(),
            from_addr: "unknown-alias@other.jp".into(),
            to: vec![recipient("Cleo", "cleo@example.com")],
            ..Default::default()
        },
    ];
    apply_card_identity(&conn, "acct", "INBOX", &mut headers);

    assert_eq!(headers[0].from_name, "Cleo");
    assert_eq!(headers[0].from_addr, "cleo@example.com");
    assert_eq!(headers[0].recipient_overflow, 1);
    assert_eq!(headers[1].from_name, "Cleo");
    assert_eq!(headers[1].recipient_overflow, 0);
    assert_eq!(headers[2].from_name, "Cleo");
    assert_eq!(headers[2].from_addr, "cleo@example.com");
    assert_eq!(headers[3].from_name, "Me");
    assert_eq!(headers[3].from_addr, "me@example.com");
    assert_eq!(headers[4].from_name, "Cleo");
    assert_eq!(headers[4].from_addr, "cleo@example.com");
}

#[test]
fn is_outgoing_matches_own_addresses_and_sent_folder_provenance() {
    let mine: std::collections::HashSet<String> =
        ["me@example.com".to_string(), "alias@other.jp".to_string()].into();
    // Own address (any casing), regardless of folder.
    assert!(is_outgoing(&mine, "INBOX", "Me@Example.Com", false));
    assert!(is_outgoing(&mine, "", "alias@other.jp", false));
    // Sent-folder provenance wins even for an unconfigured alias.
    assert!(is_outgoing(
        &mine,
        "[Gmail]/Sent Mail",
        "unknown@other.jp",
        false
    ));
    assert!(is_outgoing(&mine, "Sent", "unknown@other.jp", false));
    // Inbound in a regular folder is not ours.
    assert!(!is_outgoing(&mine, "INBOX", "cleo@example.com", false));
    assert!(!is_outgoing(&mine, "INBOX", "", false));
    // A colleague sending from a shared address configured here as an alias:
    // delivery headers say we received it, wherever it has since been filed.
    assert!(!is_outgoing(&mine, "INBOX", "alias@other.jp", true));
    assert!(!is_outgoing(&mine, "Archive", "alias@other.jp", true));
    assert!(!is_outgoing(&mine, "Trash", "alias@other.jp", true));
    // Our own copy, moved out of Sent, keeps no delivery headers.
    assert!(is_outgoing(&mine, "Archive", "alias@other.jp", false));
    // A self-addressed copy in Sent stays outgoing despite being delivered too.
    assert!(is_outgoing(&mine, "Sent", "me@example.com", true));
    // A local folder named Sent is just a name: received mail kept there stays incoming.
    assert!(!account_message_is_outgoing(
        LOCAL_ACCOUNT_ID,
        &mine,
        "Sent",
        "cleo@example.com",
        false
    ));
    assert!(account_message_is_outgoing(
        LOCAL_ACCOUNT_ID,
        &mine,
        "Sent",
        "me@example.com",
        false
    ));
}

#[test]
fn apply_card_identity_resolves_junk_display_names() {
    use crate::imap::MessageHeader;
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
            to: vec![crate::imap::Recipient {
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
    apply_card_identity(&conn, "acct", "INBOX", &mut headers);

    assert_eq!(headers[0].from_name, "Austin Mentions");
    assert_eq!(headers[0].from_addr, "bot@example.com");
    assert_eq!(headers[1].from_name, "Austin Mentions");
    assert_eq!(headers[2].from_name, "");
}

#[test]
fn save_cached_message_preserves_envelope_recipients_in_json() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let header = MessageHeader {
        uid: 42,
        subject: "Hi".into(),
        from_name: "Sender".into(),
        from_addr: "sender@example.com".into(),
        date: 1_779_580_800, // 2026-05-24
        to: vec![crate::imap::Recipient {
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
fn save_cached_message_keeps_the_header_date_when_the_body_has_none() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let header = MessageHeader {
        uid: 7,
        subject: "No Date header".into(),
        date: 1_790_645_280, // the sync's INTERNALDATE fallback
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[header]).unwrap();

    let body = Message {
        subject: "No Date header".into(),
        body: "hello".into(),
        ..Default::default()
    };
    save_cached_message(&conn, "acct", "INBOX", 7, &body).unwrap();
    let message = get_cached_message(&conn, "acct", "INBOX", 7)
        .unwrap()
        .unwrap();
    assert_eq!(message.date, 1_790_645_280);

    // A body that does carry a date still wins.
    let dated = Message {
        date: 1_790_600_000,
        ..body
    };
    save_cached_message(&conn, "acct", "INBOX", 7, &dated).unwrap();
    let message = get_cached_message(&conn, "acct", "INBOX", 7)
        .unwrap()
        .unwrap();
    assert_eq!(message.date, 1_790_600_000);
}

#[test]
fn contacts_follow_every_message_write() {
    use crate::imap::{MessageHeader, Recipient};
    let conn = test_conn();
    let to = |addr: &str| Recipient {
        name: String::new(),
        addr: addr.to_string(),
    };
    let message = |uid: u32, recipients: Vec<Recipient>| MessageHeader {
        uid,
        subject: "Hi".into(),
        from_name: "Aki".into(),
        from_addr: "Aki@example.com".into(),
        to: recipients,
        ..Default::default()
    };
    let contacts = |conn: &Connection| {
        let mut stmt = conn
            .prepare("SELECT addr_key, name, count FROM contacts ORDER BY addr_key")
            .unwrap();
        stmt.query_map([], |row| {
            Ok(format!(
                "{} {} {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    };

    // An address listed twice on one message counts it once.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[
            message(1, vec![to("bea@example.com"), to("BEA@example.com")]),
            message(2, vec![to("cleo@example.com")]),
        ],
    )
    .unwrap();
    assert_eq!(
        contacts(&conn),
        [
            "aki@example.com Aki 2",
            "bea@example.com  1",
            "cleo@example.com  1"
        ]
    );

    // A resync of the same envelope leaves the counts alone; a changed
    // recipient list moves them.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[message(2, vec![to("cleo@example.com")])],
    )
    .unwrap();
    conn.execute(
        "UPDATE messages SET seen = 1, json = json_set(json, '$.x', 1)",
        [],
    )
    .unwrap();
    assert_eq!(contacts(&conn).len(), 3);
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[message(2, vec![to("dan@example.com")])],
    )
    .unwrap();
    assert_eq!(
        contacts(&conn),
        [
            "aki@example.com Aki 2",
            "bea@example.com  1",
            "dan@example.com  1"
        ]
    );

    // Deleting the last message an address is on drops it.
    conn.execute("DELETE FROM messages WHERE uid = 1", [])
        .unwrap();
    assert_eq!(
        contacts(&conn),
        ["aki@example.com Aki 1", "dan@example.com  1"]
    );

    // Each name is kept apart, so one leaves with its last message.
    let named = |uid: u32, name: &str| MessageHeader {
        uid,
        from_name: name.into(),
        from_addr: "person@example.com".into(),
        ..Default::default()
    };
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[named(3, "Alice"), named(4, "Ally"), named(5, "Ally")],
    )
    .unwrap();
    let person = |query: &str| {
        suggest_contacts(&conn, "acct", query, 10)
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
    };
    // Matched by name, it shows the name that matched.
    assert_eq!(person("alice"), ["Alice"]);
    assert_eq!(person("person"), ["Ally"]);
    conn.execute("DELETE FROM messages WHERE uid IN (4, 5)", [])
        .unwrap();
    assert_eq!(person("ally"), Vec::<String>::new());
    assert_eq!(person("alice"), ["Alice"]);
    conn.execute("DELETE FROM messages WHERE uid = 3", [])
        .unwrap();

    // A shared sender matched only by one of its names ranks, and shows, by
    // that name; a word-start match outranks one inside a word.
    let notification = |uid: u32, name: &str| MessageHeader {
        uid,
        from_name: name.into(),
        from_addr: "notifications@github.com".into(),
        ..Default::default()
    };
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[
            notification(10, "Flathub Bot"),
            notification(11, "Flathub Bot"),
            notification(12, "Flathub Bot"),
            notification(13, "ROBIN42"),
            message(14, vec![to("robin42@example.com")]),
            message(15, vec![to("robin42@example.com")]),
            message(16, vec![to("derobin@example.com")]),
            message(17, vec![to("derobin@example.com")]),
            message(18, vec![to("derobin@example.com")]),
        ],
    )
    .unwrap();
    let matches = suggest_contacts(&conn, "acct", "robin", 10).unwrap();
    assert_eq!(
        matches
            .iter()
            .map(|c| format!("{} {}", c.name, c.addr))
            .collect::<Vec<_>>(),
        [
            " robin42@example.com",
            "ROBIN42 notifications@github.com",
            " derobin@example.com"
        ]
    );
    conn.execute("DELETE FROM messages WHERE uid >= 10", [])
        .unwrap();

    // Non-ASCII capitals still make one address.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[
            message(20, vec![to("Éva@example.com")]),
            message(21, vec![to("éva@example.com")]),
        ],
    )
    .unwrap();
    assert_eq!(suggest_contacts(&conn, "acct", "éva", 10).unwrap().len(), 1);
    conn.execute("DELETE FROM messages WHERE uid >= 20", [])
        .unwrap();

    // Feed items aren't correspondents.
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, from_addr)
         VALUES('acct', 'feed', 'item', 0, 'feed@example.com')",
        [],
    )
    .unwrap();
    assert_eq!(
        contacts(&conn),
        ["aki@example.com Aki 1", "dan@example.com  1"]
    );
}

#[test]
fn migration_backfills_contacts_from_cached_mail() {
    let conn = Connection::open_in_memory().unwrap();
    db::migrate_to(&conn, 8).unwrap();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, from_name, from_addr, date, json)
         VALUES('acct', 'INBOX', '1', 1, 'Aki', 'aki@example.com', 100,
                '{\"to\":[{\"name\":\"Bea\",\"addr\":\"bea@example.com\"}],
                  \"cc\":[{\"name\":\"\",\"addr\":\"Aki@example.com\"}]}'),
               ('acct', 'Sent', '2', 2, 'Me', 'me@example.com', 200,
                '{\"to\":[{\"name\":\"\",\"addr\":\"bea@example.com\"}]}'),
               ('acct', 'feed', 'item', 0, 'Feed', 'feed@example.com', 300, '{}')",
        [],
    )
    .unwrap();
    db::run_migrations(&conn).unwrap();

    let top = suggest_contacts(&conn, "acct", "", 10).unwrap();
    assert_eq!(
        top.iter()
            .map(|c| format!("{} {}", c.addr, c.name))
            .collect::<Vec<_>>(),
        // Aki sent message 1 and is Cc'd on it nameless: two names, counted
        // once each.
        [
            "aki@example.com Aki",
            "bea@example.com Bea",
            "me@example.com Me"
        ]
    );
    // v23 backfills when each was last seen: bea@ is on both messages.
    let recent = list_contacts(&conn, "acct", "", 10, ContactSort::Recent, false).unwrap();
    assert_eq!(
        recent.iter().map(|c| c.addr.as_str()).collect::<Vec<_>>(),
        ["bea@example.com", "me@example.com", "aki@example.com"]
    );
}
