use super::*;
use crate::imap::MessageHeader;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn
}

fn header(uid: u32) -> MessageHeader {
    MessageHeader {
        uid,
        seen: true,
        subject: "quote".into(),
        message_id: format!("<{uid}@example.com>"),
        ..Default::default()
    }
}

fn sent_rule() -> Rule {
    Rule {
        id: "sent".into(),
        sent: true,
        star: true,
        tags: vec!["client".into()],
        ..Default::default()
    }
}

fn received_rule() -> Rule {
    Rule {
        id: "received".into(),
        mark_read: true,
        ..Default::default()
    }
}

/// One read of `folder` that finds `uids`, as a sync or the read after a
/// send does it. Answers with what it took for newly sent.
fn read(conn: &Connection, folder: &str, uids: &[u32], uid_next: u32) -> Vec<u32> {
    let batch: Vec<MessageHeader> = uids.iter().map(|uid| header(*uid)).collect();
    let sent = new_sent_uids(conn, "a", folder, 7, uid_next, &batch).unwrap();
    crate::store::upsert_messages(conn, "a", folder, &batch).unwrap();
    apply_sent_rules(conn, "a", folder, 7, &sent).unwrap();
    crate::store::set_folder_state(conn, "a", folder, 7, uid_next).unwrap();
    sent
}

#[test]
fn old_rules_are_for_received_mail_and_sent_rules_are_limited() {
    let old: Rule = serde_json::from_str(r#"{"id":"old","star":true}"#).unwrap();
    assert!(!old.sent);
    let conn = db();
    let refused = [
        Rule {
            mark_read: true,
            ..sent_rule()
        },
        Rule {
            move_to: "trash".into(),
            ..sent_rule()
        },
        Rule {
            trigger: RuleTrigger::Age,
            delay_seconds: 60,
            ..sent_rule()
        },
        Rule {
            delay_seconds: 60,
            ..sent_rule()
        },
    ];
    for rule in refused {
        assert!(set_rules(&conn, vec![rule.clone()]).is_err(), "{rule:?}");
    }
    assert!(set_rules(&conn, vec![sent_rule()]).unwrap()[0].sent);
}

#[test]
fn sent_rules_run_once_on_copies_the_sent_folder_gains() {
    let conn = db();
    set_rules(&conn, vec![sent_rule(), received_rule()]).unwrap();
    // The first read is the mailbox as it was, not mail just sent.
    assert!(read(&conn, "Sent", &[1, 2], 3).is_empty());
    assert!(pending_rule_actions(&conn, "a", "Sent").unwrap().is_empty());

    assert_eq!(read(&conn, "Sent", &[2, 3], 4), vec![3]);
    let actions = pending_rule_actions(&conn, "a", "Sent").unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].uid, 3);
    assert!(actions[0].star);
    // Only the rule for sent mail: the other would have marked it read.
    assert!(!actions[0].mark_read);
    assert_eq!(actions[0].rule_ids, vec!["sent".to_string()]);
    let tagged: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_tags WHERE tag = 'client' AND tag_key = '3@example.com'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tagged, 1);

    // Reading the same tail again finds nothing new.
    finish_rule_actions(&conn, "a", "Sent", &[3], 0).unwrap();
    assert!(read(&conn, "Sent", &[2, 3], 4).is_empty());
    assert!(pending_rule_actions(&conn, "a", "Sent").unwrap().is_empty());
}

#[test]
fn only_live_sent_rules_for_the_account_count() {
    let conn = db();
    set_rules(&conn, vec![received_rule()]).unwrap();
    assert!(!has_sent_rules(&conn, "a").unwrap());
    let mut rule = Rule {
        accounts: vec!["b".into()],
        ..sent_rule()
    };
    set_rules(&conn, vec![rule.clone()]).unwrap();
    assert!(!has_sent_rules(&conn, "a").unwrap());
    assert!(has_sent_rules(&conn, "b").unwrap());
    rule.disabled = true;
    set_rules(&conn, vec![rule]).unwrap();
    assert!(!has_sent_rules(&conn, "b").unwrap());
}

#[test]
fn a_copy_already_seen_elsewhere_on_the_account_is_still_sent() {
    let conn = db();
    set_rules(&conn, vec![sent_rule()]).unwrap();
    read(&conn, "Sent", &[1], 2);
    // Mail to oneself, in the Inbox before its Sent copy is read.
    crate::store::upsert_messages(&conn, "a", "INBOX", &[header(2)]).unwrap();
    assert_eq!(read(&conn, "Sent", &[2], 3), vec![2]);
    assert_eq!(pending_rule_actions(&conn, "a", "Sent").unwrap().len(), 1);
}

#[test]
fn sent_rules_leave_received_mail_alone_and_the_other_way_round() {
    let conn = db();
    set_rules(&conn, vec![sent_rule(), received_rule()]).unwrap();
    crate::store::upsert_messages(&conn, "a", "INBOX", &[header(1)]).unwrap();
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    apply_due_rules(&conn, "a", "INBOX", 7, i64::MAX / 2).unwrap();
    let actions = pending_rule_actions(&conn, "a", "INBOX").unwrap();
    assert_eq!(actions.len(), 1);
    assert!(actions[0].mark_read && !actions[0].star);
    assert_eq!(actions[0].rule_ids, vec!["received".to_string()]);
}

#[test]
fn a_sent_rule_run_by_hand_looks_in_the_sent_folder() {
    let conn = db();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url, config, created_at, updated_at)
         VALUES('a', 'imap', 'imap', 'a', '', '', '{}', 0, 0)",
        [],
    )
    .unwrap();
    let folder = |name: &str, special_use: Option<&str>| crate::imap::Folder {
        name: name.into(),
        display_name: name.into(),
        special_use: special_use.map(str::to_string),
        ..Default::default()
    };
    crate::store::upsert_folders(
        &conn,
        "a",
        &[folder("INBOX", None), folder("Sent Items", Some("sent"))],
    )
    .unwrap();
    let id = set_rules(&conn, vec![sent_rule()]).unwrap()[0].id.clone();
    read(&conn, "Sent Items", &[1, 2], 3);
    crate::store::upsert_messages(&conn, "a", "INBOX", &[header(9)]).unwrap();
    crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
    assert_eq!(
        saved_rule_folder(&conn, &id, "a").unwrap().as_deref(),
        Some("Sent Items")
    );
    assert_eq!(count_existing_rule_matches(&conn, &id).unwrap(), 2);
    assert_eq!(
        queue_existing_rule(&conn, &id).unwrap(),
        vec![("a".to_string(), 2)]
    );
    assert_eq!(
        pending_rule_actions(&conn, "a", "Sent Items")
            .unwrap()
            .len(),
        2
    );
    assert!(
        pending_rule_actions(&conn, "a", "INBOX")
            .unwrap()
            .is_empty()
    );
}
