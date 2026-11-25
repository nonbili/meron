use super::*;

// An encrypted thread whose start was never fetched (older than the prefetch
// window) still carries the protected-headers placeholder "..." as its subject;
// only the newer, fetched and decrypted message knows the real one. The card
// must not be titled "...".
#[test]
fn card_title_skips_the_placeholder_of_an_encrypted_message_never_decrypted() {
    let conn = test_conn();
    let insert = |uid: u32, subject: &str, json: &str| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                                  date, seen, thread_key, json)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, 'Aki', 'aki@example.com', ?4, 1,
                    'root@example.com', ?5)",
            params![uid.to_string(), uid, subject, 100 * uid as i64, json],
        )
        .unwrap();
    };
    // Known encrypted from its Content-Type at fetch (`$.encrypted`).
    insert(1, "...", r#"{"encrypted":true}"#);
    insert(2, "Re: Secret plans", r#"{"crypto":{"encrypted":true}}"#);

    let subjects =
        card_root_subjects(&conn, "acct", "INBOX", &["root@example.com".to_string()]).unwrap();
    // The oldest message that knows its subject titles the card, verbatim.
    assert_eq!(
        subjects.get("root@example.com").map(String::as_str),
        Some("Re: Secret plans")
    );

    // With nothing decrypted there is no real subject to show: the
    // placeholder stays (chosen over an "Encrypted message" label).
    conn.execute(
        r#"INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                              date, seen, thread_key, json)
         VALUES('acct', 'INBOX', '9', 9, '[...]', 'Aki', 'aki@example.com', 900, 1,
                'other@example.com', '{"encrypted":true}')"#,
        [],
    )
    .unwrap();
    let subjects =
        card_root_subjects(&conn, "acct", "INBOX", &["other@example.com".to_string()]).unwrap();
    assert_eq!(
        subjects.get("other@example.com").map(String::as_str),
        Some("[...]")
    );

    // A plain message really titled "..." keeps its title.
    insert(3, "...", "{}");
    insert(4, "Re: Something else", "{}");
    conn.execute(
        "UPDATE messages SET thread_key = 'plain@example.com' WHERE uid IN (3, 4)",
        [],
    )
    .unwrap();
    let subjects =
        card_root_subjects(&conn, "acct", "INBOX", &["plain@example.com".to_string()]).unwrap();
    assert_eq!(
        subjects.get("plain@example.com").map(String::as_str),
        Some("...")
    );
}

// The page-grouping path titles cards the same way as the cache path above.
#[test]
fn grouped_card_title_skips_the_placeholder_of_an_encrypted_message_never_decrypted() {
    use crate::imap::MessageHeader;
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 2,
                date: 200,
                subject: "Re: Secret plans".to_string(),
                thread_key: "gmthrid:enc".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                date: 100,
                subject: "...".to_string(),
                thread_key: "gmthrid:enc".to_string(),
                encrypted: true,
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.subject, "Re: Secret plans");

    // Plain mail titled "..." is titled by its own subject.
    let cards = group_thread_cards(
        vec![
            MessageHeader {
                uid: 2,
                date: 200,
                subject: "Re: Something else".to_string(),
                thread_key: "gmthrid:plain".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 1,
                date: 100,
                subject: "...".to_string(),
                thread_key: "gmthrid:plain".to_string(),
                ..Default::default()
            },
        ],
        "INBOX",
    );
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].header.subject, "...");
}

// A reply written by a client that had not decrypted the thread carries the
// placeholder as its protected subject ("Re: [...]"). It would form a subject
// branch of its own (an empty grouping subject) and drop out of the thread.
#[test]
fn an_encrypted_reply_titled_with_the_placeholder_takes_the_threads_subject() {
    let conn = test_conn();
    let insert = |uid: u32, subject: &str, encrypted: bool, thread: &str| {
        let json = if encrypted {
            r#"{"crypto":{"encrypted":true}}"#
        } else {
            "{}"
        };
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                                  date, seen, thread_key, body, json)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, 'Aki', 'aki@example.com', ?4, 1, ?5, 'b', ?6)",
            params![
                uid.to_string(),
                uid,
                subject,
                100 * uid as i64,
                thread,
                json
            ],
        )
        .unwrap();
    };
    let subject = |uid: u32| -> String {
        conn.query_row("SELECT subject FROM messages WHERE uid = ?1", [uid], |r| {
            r.get(0)
        })
        .unwrap()
    };
    insert(1, "Testing This", true, "root@example.com");
    insert(2, "Re: [...]", true, "root@example.com");
    insert(3, "...", true, "root@example.com");
    // Not encrypted: a subject really written as "[...]" stays.
    insert(4, "Re: [...]", false, "root@example.com");
    // No real subject anywhere in its thread: nothing to take.
    insert(5, "Re: [...]", true, "other@example.com");

    assert_eq!(
        repair_placeholder_reply_subjects(&conn, "acct", None).unwrap(),
        2
    );
    assert_eq!(subject(2), "Re: Testing This");
    assert_eq!(subject(3), "Re: Testing This");
    assert_eq!(subject(4), "Re: [...]");
    assert_eq!(subject(5), "Re: [...]");
    assert_eq!(
        repair_placeholder_reply_subjects(&conn, "acct", None).unwrap(),
        0,
        "idempotent"
    );
}

// Saving a body repairs its thread either way round: the placeholder reply
// decrypted after the thread start, or the thread start after the reply.
#[test]
fn saving_a_decrypted_body_repairs_placeholder_replies_in_its_thread() {
    let conn = test_conn();
    let header = |uid: u32, subject: &str| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr,
                                  date, seen, thread_key, json)
             VALUES('acct', 'INBOX', ?1, ?2, ?3, 'Aki', 'aki@example.com', ?4, 1, 'root@example.com', '{}')",
            params![uid.to_string(), uid, subject, 100 * uid as i64],
        )
        .unwrap();
    };
    let decrypted = |subject: &str| crate::parse::Message {
        subject: subject.to_string(),
        from_name: "Aki".to_string(),
        from_addr: "aki@example.com".to_string(),
        body: "text".to_string(),
        crypto: Some(crate::openpgp::mime::CryptoStatus {
            encrypted: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    let subject = |uid: u32| -> String {
        conn.query_row("SELECT subject FROM messages WHERE uid = ?1", [uid], |r| {
            r.get(0)
        })
        .unwrap()
    };
    header(1, "[...]");
    header(2, "[...]");
    // The reply decrypts first: its thread has no real subject yet.
    save_cached_message(&conn, "acct", "INBOX", 2, &decrypted("Re: [...]")).unwrap();
    assert_eq!(subject(2), "Re: [...]");
    // Then the thread start: the reply takes its subject.
    save_cached_message(&conn, "acct", "INBOX", 1, &decrypted("Testing This")).unwrap();
    assert_eq!(subject(2), "Re: Testing This");
}

// After an encrypted send, the engine caches its own Sent copy's body at once;
// it finds the row the tail refresh just stored by Message-ID.
#[test]
fn a_sent_copy_without_a_body_is_found_by_message_id() {
    let conn = test_conn();
    let insert = |folder: &str, uid: u32, mid: &str, body: Option<&str>| {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date, seen, body, json)
             VALUES('acct', ?1, ?2, ?3, '[...]', 'Me', 'me@example.com', 100, 1, ?4, json_object('message_id', ?5))",
            params![folder, uid.to_string(), uid, body, mid],
        )
        .unwrap();
    };
    insert("Sent", 7, "<new@example.com>", None);
    insert("Sent", 6, "<old@example.com>", Some("cached"));
    insert("INBOX", 9, "<new@example.com>", None);
    let find =
        |folder: &str, mid: &str| sent_copy_uid_without_body(&conn, "acct", folder, mid).unwrap();
    assert_eq!(find("Sent", "<NEW@example.com>"), Some(7));
    assert_eq!(
        find("Sent", "<old@example.com>"),
        None,
        "already has a body"
    );
    assert_eq!(find("Sent", "<other@example.com>"), None);
    assert_eq!(find("Sent", ""), None);
}

// On Gmail an encrypted reply threads by its References (Gmail's thread id is
// ignored for the placeholder subject), and still joins the thread its root is
// cached under, Gmail's id included: a conversation that started unencrypted.
#[test]
fn an_encrypted_gmail_reply_joins_the_gmail_thread_of_the_message_it_answers() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let header = |uid: u32, subject: &str, message_id: &str, thread_key: &str| MessageHeader {
        uid,
        subject: subject.to_string(),
        message_id: message_id.to_string(),
        thread_key: thread_key.to_string(),
        date: 100 * uid as i64,
        ..Default::default()
    };
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[header(1, "Plans", "root@example.com", "gmthrid:42")],
    )
    .unwrap();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[header(2, "[...]", "reply@example.com", "root@example.com")],
    )
    .unwrap();
    // A new encrypted message with no References starts its own thread.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[header(3, "[...]", "new@example.com", "new@example.com")],
    )
    .unwrap();
    let key = |uid: u32| -> String {
        conn.query_row(
            "SELECT thread_key FROM messages WHERE uid = ?1",
            [uid],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(key(2), "gmthrid:42");
    assert_eq!(key(3), "new@example.com");
}

// On Gmail our sent messages are copies Gmail files itself, so until their body
// is fetched they carry the placeholder subject. They are still part of the
// thread: neither the thread view nor the list may treat them as a different
// subject branch (the Mac's chat lost every message it had sent).
#[test]
fn an_undecrypted_message_stays_in_its_threads_branch() {
    use crate::imap::MessageHeader;
    let header = |uid: u32, folder: &str, subject: &str, date: i64| MessageHeader {
        uid,
        folder: folder.to_string(),
        subject: subject.to_string(),
        thread_key: "root@example.com".to_string(),
        date,
        ..Default::default()
    };
    // Encrypted (multipart/encrypted at fetch), not decrypted yet.
    let sent = MessageHeader {
        encrypted: true,
        ..header(5, "[Gmail]/Sent Mail", "[...]", 100)
    };
    let reply = header(9, "INBOX", "Re: Hopefully the last test", 200);
    let later = MessageHeader {
        encrypted: true,
        ..header(6, "[Gmail]/Sent Mail", "...", 300)
    };

    assert!(subject_in_branch(
        &sent.subject,
        sent.encrypted,
        "Hopefully the last test"
    ));
    assert!(subject_in_branch(
        &later.subject,
        later.encrypted,
        "Hopefully the last test"
    ));
    assert!(subject_in_branch(
        &reply.subject,
        reply.encrypted,
        "Hopefully the last test"
    ));
    assert!(!subject_in_branch(
        "Lunch",
        false,
        "Hopefully the last test"
    ));

    let cards = group_thread_cards(vec![later, reply, sent], "INBOX");
    assert_eq!(cards.len(), 1, "one card, not a branch per subject");
    assert_eq!(
        cards[0].original_thread_key, None,
        "no 'changed subject' banner"
    );
    assert_eq!(cards[0].message_count, 3);
}

// Only an encrypted message's `...` is the protected-headers placeholder. A
// plain message titled `...` has that as its real subject: it is a branch of
// its own, not a member of every branch or of a titled one.
#[test]
fn a_plain_message_titled_like_a_placeholder_is_its_own_branch() {
    use crate::imap::MessageHeader;
    let header = |uid: u32, subject: &str, date: i64| MessageHeader {
        uid,
        folder: "INBOX".to_string(),
        subject: subject.to_string(),
        thread_key: "root@example.com".to_string(),
        date,
        ..Default::default()
    };
    let first = header(1, "Hopefully the last test", 100);
    let plain = header(2, "...", 200);
    assert!(!subject_in_branch(
        &plain.subject,
        plain.encrypted,
        "Hopefully the last test"
    ));
    let cards = group_thread_cards(vec![plain, first], "INBOX");
    assert_eq!(
        cards.len(),
        2,
        "the plain `...` message is a branch of its own"
    );
}

// Whether a message is encrypted is known from its headers before its body is
// fetched, and it has to survive the cache: every read that decides branch
// membership gets it back.
#[test]
fn the_encrypted_flag_survives_the_cache() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let header = |uid: u32, encrypted: bool| MessageHeader {
        uid,
        encrypted,
        subject: "...".to_string(),
        message_id: format!("m{uid}@example.com"),
        thread_key: "root@example.com".to_string(),
        date: 100 * uid as i64,
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "INBOX", &[header(1, true), header(2, false)]).unwrap();
    let flags = |headers: Vec<MessageHeader>| {
        headers
            .into_iter()
            .map(|h| (h.uid, h.encrypted))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        flags(get_thread_headers(&conn, "acct", "INBOX", "root@example.com").unwrap()),
        vec![(1, true), (2, false)]
    );
    assert_eq!(
        flags(get_thread_headers_all_folders(&conn, "acct", "root@example.com", &[]).unwrap()),
        vec![(1, true), (2, false)]
    );
    let mapped = conn
        .query_row(
            &format!(
                "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key, json_extract(json, '$.to'), {} \
                 FROM messages WHERE uid = 1",
                crate::store::ENCRYPTED_COLUMN
            ),
            [],
            crate::store::message_header_from_row,
        )
        .unwrap();
    assert!(mapped.encrypted, "the shared row mapper reads the column");

    // A branch-wide action (mark read, move) takes the encrypted placeholder
    // with its branch, but not a plain message titled `...`.
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[MessageHeader {
            subject: "Plans".to_string(),
            ..header(3, false)
        }],
    )
    .unwrap();
    let by_folder = thread_uids_by_folder(
        &conn,
        "acct",
        "INBOX",
        "root@example.com",
        Some("Plans"),
        false,
    )
    .unwrap();
    assert_eq!(by_folder.get("INBOX"), Some(&vec![1, 3]));
}

// On Gmail, encrypted messages thread by their own headers (Gmail's thread id
// is ignored for the placeholder subject). A plain reply in such a thread —
// encryption turned off — has a real subject, so Gmail files it in a new
// conversation and gives it a new thread id. It must still join the thread of
// the message it answers, or the conversation splits in two.
#[test]
fn a_plain_gmail_reply_joins_the_encrypted_thread_it_answers() {
    use crate::imap::MessageHeader;
    let header = |uid: u32,
                  subject: &str,
                  message_id: &str,
                  in_reply_to: &str,
                  thread_key: &str,
                  gmail: bool| {
        MessageHeader {
            uid,
            subject: subject.to_string(),
            message_id: message_id.to_string(),
            in_reply_to: in_reply_to.to_string(),
            gmail_msg_id: gmail.then_some(1000 + uid as u64),
            thread_key: thread_key.to_string(),
            date: 100 * uid as i64,
            ..Default::default()
        }
    };
    let key = |conn: &rusqlite::Connection, uid: u32| -> String {
        conn.query_row(
            "SELECT thread_key FROM messages WHERE uid = ?1",
            [uid],
            |r| r.get(0),
        )
        .unwrap()
    };
    let root = header(1, "[...]", "root@example.com", "", "root@example.com", true);
    let enc_reply = header(
        2,
        "[...]",
        "enc@example.com",
        "root@example.com",
        "root@example.com",
        true,
    );
    let plain = header(
        3,
        "Re: Plans",
        "plain@example.com",
        "enc@example.com",
        "gmthrid:99",
        true,
    );
    let plain_again = header(
        4,
        "Re: Plans",
        "plain2@example.com",
        "plain@example.com",
        "gmthrid:99",
        true,
    );

    // In order: each reply finds the message it answers already cached.
    let conn = test_conn();
    upsert_messages(&conn, "acct", "INBOX", &[root.clone(), enc_reply.clone()]).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[plain.clone()]).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[plain_again.clone()]).unwrap();
    assert_eq!(key(&conn, 3), "root@example.com");
    assert_eq!(key(&conn, 4), "root@example.com");

    // Out of order: the plain replies sync before the message they answer.
    let conn = test_conn();
    upsert_messages(
        &conn,
        "acct",
        "INBOX",
        &[plain.clone(), plain_again.clone()],
    )
    .unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[root, enc_reply]).unwrap();
    assert_eq!(key(&conn, 3), "root@example.com");
    assert_eq!(key(&conn, 4), "root@example.com");
}

// Plain Gmail mail keeps Gmail's threading: a reply to a message cached from
// our own bytes (no Gmail id, so keyed by Message-ID) stays in its Gmail
// conversation.
#[test]
fn a_plain_gmail_reply_to_a_locally_cached_message_keeps_gmails_thread() {
    use crate::imap::MessageHeader;
    let conn = test_conn();
    let local = MessageHeader {
        uid: 1,
        subject: "Plans".to_string(),
        message_id: "local@example.com".to_string(),
        thread_key: "local@example.com".to_string(),
        date: 100,
        ..Default::default()
    };
    let reply = MessageHeader {
        uid: 2,
        subject: "Re: Plans".to_string(),
        message_id: "reply@example.com".to_string(),
        in_reply_to: "local@example.com".to_string(),
        gmail_msg_id: Some(1002),
        thread_key: "gmthrid:99".to_string(),
        date: 200,
        ..Default::default()
    };
    upsert_messages(&conn, "acct", "Sent", &[local]).unwrap();
    upsert_messages(&conn, "acct", "INBOX", &[reply]).unwrap();
    let key: String = conn
        .query_row("SELECT thread_key FROM messages WHERE uid = 2", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(key, "gmthrid:99");
}

// The core decides whether a reply answers encrypted mail from the
// message it names in In-Reply-To, whatever the caller sends.
#[test]
fn a_reply_is_known_to_answer_encrypted_mail_by_its_in_reply_to() {
    let conn = test_conn();
    let insert = |uid: u32, message_id: &str, encrypted: bool, account: &str| {
        let crypto = if encrypted {
            r#","crypto":{"encrypted":true}"#
        } else {
            ""
        };
        let json = format!(r#"{{"message_id":"{message_id}"{crypto}}}"#);
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen, json)
             VALUES(?1, 'INBOX', ?2, ?3, 's', 1, 1, ?4)",
            params![account, uid.to_string(), uid, json],
        )
        .unwrap();
    };
    insert(1, "Secret@Example.com", true, "acct");
    insert(2, "plain@example.com", false, "acct");
    insert(3, "elsewhere@example.com", true, "other");
    let answers = |id: &str| crate::openpgp::store::answers_encrypted(&conn, "acct", id).unwrap();
    assert!(
        answers("<secret@example.com>"),
        "brackets and case don't matter"
    );
    assert!(answers("secret@example.com"));
    assert!(!answers("<plain@example.com>"));
    assert!(
        !answers("<elsewhere@example.com>"),
        "another account's message doesn't count"
    );
    assert!(!answers(""));
    assert!(!answers("<unknown@example.com>"));
}

// The reply rule as the sidecar applies it to send and save_draft.
#[test]
fn a_reply_to_encrypted_mail_saying_nothing_is_encrypted_and_marked_implicit() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date, seen, json)
         VALUES('acct', 'INBOX', '1', 1, 's', 1, 1, '{\"message_id\":\"secret@example.com\",\"crypto\":{\"encrypted\":true}}')",
        [],
    )
    .unwrap();
    let decide = |irt: &str, encrypt: bool, plain: bool| {
        crate::openpgp::store::reply_protection(&conn, "acct", irt, encrypt, plain).unwrap()
    };
    assert_eq!(
        decide("<secret@example.com>", false, false),
        (true, true),
        "nothing said: encrypted, implicitly"
    );
    assert_eq!(
        decide("<secret@example.com>", false, true),
        (false, false),
        "plain chosen"
    );
    assert_eq!(
        decide("<secret@example.com>", true, false),
        (true, false),
        "asked for"
    );
    assert_eq!(
        decide("<other@example.com>", false, false),
        (false, false),
        "answers plain mail"
    );
}
