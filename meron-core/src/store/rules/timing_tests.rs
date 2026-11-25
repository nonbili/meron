use super::*;
use crate::imap::MessageHeader;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn
}

fn mail(conn: &Connection, uid: u32, seen: bool, date: i64) {
    crate::store::upsert_messages(
        conn,
        "a",
        "INBOX",
        &[MessageHeader {
            uid,
            seen,
            date,
            subject: "login code".into(),
            message_id: format!("<{uid}@example.com>"),
            ..Default::default()
        }],
    )
    .unwrap();
}

fn rule(trigger: RuleTrigger, delay_seconds: i64) -> Rule {
    Rule {
        id: "timed".into(),
        trigger,
        delay_seconds,
        star: true,
        ..Default::default()
    }
}

fn due(conn: &Connection, now: i64) -> Vec<RuleAction> {
    apply_due_rules(conn, "a", "INBOX", 7, now).unwrap();
    pending_rule_actions(conn, "a", "INBOX").unwrap()
}

#[test]
fn old_rules_default_to_immediate_arrival_and_delays_are_validated() {
    let old: Rule = serde_json::from_str(r#"{"id":"old","star":true}"#).unwrap();
    assert_eq!(old.trigger, RuleTrigger::Arrival);
    assert_eq!(old.delay_seconds, 0);
    let conn = db();
    assert!(set_rules(&conn, vec![rule(RuleTrigger::Read, -1)]).is_err());
    assert!(set_rules(&conn, vec![rule(RuleTrigger::Age, i64::MAX)]).is_err());
    assert!(serde_json::from_str::<Rule>(r#"{"trigger":"unknown"}"#).is_err());
}

#[test]
fn forwards_are_queued_and_struck_off_once_out() {
    let conn = db();
    let forward = |id: &str, to: &str| Rule {
        id: id.into(),
        forward_to: to.into(),
        ..Default::default()
    };
    let saved = set_rules(
        &conn,
        vec![
            forward("one", "Dan Example <dan@example.org>; DAN@example.org"),
            Rule {
                star: true,
                ..forward("two", "eve@example.org, dan@example.org")
            },
        ],
    )
    .unwrap();
    assert_eq!(saved[0].forward_to, "dan@example.org");
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    let actions = pending_rule_actions(&conn, "a", "INBOX").unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].forwards, ["dan@example.org", "eve@example.org"]);

    // What else the rules want done stays queued for a retry.
    finish_rule_forward(&conn, "a", "INBOX", 1, actions[0].run).unwrap();
    let actions = pending_rule_actions(&conn, "a", "INBOX").unwrap();
    assert!(actions[0].forwards.is_empty());
    assert!(actions[0].star);
}

#[test]
fn a_forward_needs_addresses_and_received_mail() {
    let conn = db();
    let forward = |to: &str, sent| Rule {
        forward_to: to.into(),
        sent,
        ..Default::default()
    };
    assert!(set_rules(&conn, vec![forward("not an address", false)]).is_err());
    assert!(set_rules(&conn, vec![forward("dan@example.org", true)]).is_err());
    assert!(set_rules(&conn, vec![forward("dan@example.org", false)]).is_ok());
}

#[test]
fn arrivals_in_folders_rules_skip_are_still_recorded() {
    let conn = db();
    set_rules(&conn, vec![rule(RuleTrigger::Arrival, 0)]).unwrap();
    crate::store::upsert_messages(
        &conn,
        "a",
        "Trash",
        &[MessageHeader {
            uid: 1,
            message_id: "<1@example.com>".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    assert_eq!(
        crate::store::folder_role(&conn, "a", "Trash").unwrap(),
        "trash"
    );
    apply_rules(&conn, "a", "Trash", 7, &[1]).unwrap();
    let arrived: Option<i64> = conn
        .query_row("SELECT arrived_at FROM rule_events", [], |r| r.get(0))
        .unwrap();
    assert!(arrived.is_some());
    assert!(
        pending_rule_actions(&conn, "a", "Trash")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn arrival_delay_survives_sync_and_does_not_apply_to_initial_cache() {
    let conn = db();
    set_rules(&conn, vec![rule(RuleTrigger::Arrival, 300)]).unwrap();
    mail(&conn, 1, false, 100);
    mail(&conn, 2, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[2]).unwrap();
    assert!(
        pending_rule_actions(&conn, "a", "INBOX")
            .unwrap()
            .is_empty()
    );
    conn.execute(
        "UPDATE rule_events SET arrived_at = 1000 WHERE arrived_at IS NOT NULL",
        [],
    )
    .unwrap();
    mail(&conn, 2, false, 100); // Envelope refresh must preserve the clock.
    assert!(due(&conn, 1299).is_empty());
    let actions = due(&conn, 1300);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].uid, 2);
    assert_eq!(actions[0].uidvalidity, 7);
    finish_rule_actions(&conn, "a", "INBOX", &[2], actions[0].run).unwrap();
    assert!(due(&conn, 1500).is_empty());
}

fn body(conn: &Connection, uid: u32, text: &str) {
    conn.execute(
        "UPDATE messages SET body = ?2 WHERE account = 'a' AND folder = 'INBOX' AND uid = ?1",
        params![uid, text],
    )
    .unwrap();
}

fn body_rule(field: &str, value: &str, negate: bool) -> Rule {
    Rule {
        conditions: vec![RuleCondition {
            field: field.into(),
            value: value.into(),
            negate,
        }],
        ..rule(RuleTrigger::Arrival, 0)
    }
}

#[test]
fn a_rule_reading_the_body_waits_for_it() {
    let conn = db();
    set_rules(
        &conn,
        vec![
            body_rule("body", "Invoice", false),
            Rule {
                id: "not".into(),
                ..body_rule("body", "invoice", true)
            },
        ],
    )
    .unwrap();
    mail(&conn, 1, false, 100);
    mail(&conn, 2, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1, 2]).unwrap();
    conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
        .unwrap();
    // Headers alone decide nothing, either way.
    assert!(due(&conn, 1000).is_empty());
    assert_eq!(
        rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 50, false).unwrap(),
        [2, 1]
    );
    assert_eq!(
        rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 1, true).unwrap(),
        [2]
    );

    body(&conn, 1, "please find the INVOICE attached");
    body(&conn, 2, "lunch?");
    let actions = due(&conn, 1000);
    let star = |uid| {
        let action = actions.iter().find(|action| action.uid == uid).unwrap();
        action.rule_ids.clone()
    };
    assert_eq!(star(1), ["timed"]);
    assert_eq!(star(2), ["not"]);
    assert!(
        rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 50, false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rules_acting_on_arrival_wait_together_once_one_reads_the_body() {
    let conn = db();
    let header = |id: &str| Rule {
        id: id.into(),
        conditions: vec![RuleCondition {
            field: "subject".into(),
            value: "login".into(),
            negate: false,
        }],
        star: false,
        ..rule(RuleTrigger::Arrival, 0)
    };
    set_rules(
        &conn,
        vec![
            Rule {
                id: "body".into(),
                star: false,
                move_to: "trash".into(),
                forward_to: "dan@example.org".into(),
                ..body_rule("body", "code", false)
            },
            Rule {
                move_to: "archive".into(),
                forward_to: "dan@example.org".into(),
                ..header("header")
            },
        ],
    )
    .unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    // The header rule matches already, but waits with the other.
    assert!(
        pending_rule_actions(&conn, "a", "INBOX")
            .unwrap()
            .is_empty()
    );
    conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
        .unwrap();
    body(&conn, 1, "your code is 1234");
    let actions = due(&conn, 1000);
    assert_eq!(actions.len(), 1, "one run");
    assert!(actions[0].trash, "the first in the list moves it");
    assert_eq!(actions[0].forwards, ["dan@example.org"]);
    assert_eq!(actions[0].rule_ids, ["body", "header"]);
}

#[test]
fn a_rule_acting_on_arrival_checks_a_message_once() {
    let conn = db();
    let read = Rule {
        conditions: vec![
            RuleCondition {
                field: "unread".into(),
                value: String::new(),
                negate: true,
            },
            RuleCondition {
                field: "body".into(),
                value: "code".into(),
                negate: false,
            },
        ],
        ..rule(RuleTrigger::Arrival, 0)
    };
    set_rules(&conn, vec![read]).unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
        .unwrap();
    body(&conn, 1, "your code");
    assert!(due(&conn, 1000).is_empty(), "unread as it arrived");
    // Read later: no longer what arrived, and not news to this rule.
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    assert!(due(&conn, 1000).is_empty());
}

#[test]
fn starting_to_wait_for_bodies_does_not_replay_arrivals() {
    let conn = db();
    let header = Rule {
        conditions: vec![RuleCondition {
            field: "subject".into(),
            value: "login".into(),
            negate: false,
        }],
        ..rule(RuleTrigger::Arrival, 0)
    };
    set_rules(&conn, vec![header.clone()]).unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    let ran = pending_rule_actions(&conn, "a", "INBOX").unwrap();
    finish_rule_actions(&conn, "a", "INBOX", &[1], ran[0].run).unwrap();
    body(&conn, 1, "hello");
    set_rules(
        &conn,
        vec![
            header,
            Rule {
                id: "body".into(),
                ..body_rule("body", "nothing like it", false)
            },
        ],
    )
    .unwrap();
    assert!(
        due(&conn, store_now()).is_empty(),
        "the header rule ran on it already"
    );
}

#[test]
fn mail_that_could_not_be_opened_has_no_body_to_read() {
    let conn = db();
    set_rules(&conn, vec![body_rule("body", "invoice", true)]).unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
        .unwrap();
    body(&conn, 1, "");
    conn.execute(
        "UPDATE messages SET json = json_set(json, '$.crypto', json('{\"encrypted\":true}'))",
        [],
    )
    .unwrap();
    assert!(due(&conn, 1000).is_empty(), "its body may well say invoice");
    assert!(
        rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 50, false)
            .unwrap()
            .is_empty()
    );
    conn.execute(
        "UPDATE messages SET json = json_set(json, '$.crypto.decrypted_with', 'ABCD')",
        [],
    )
    .unwrap();
    assert_eq!(due(&conn, 1000).len(), 1);

    // Only part of it opened: the rest may still say it.
    conn.execute(
        "UPDATE messages SET json = json_set(json, '$.crypto', json('{\"problem\":\"partly_encrypted\"}'))",
        [],
    )
    .unwrap();
    conn.execute("DELETE FROM rule_firings", []).unwrap();
    conn.execute("DELETE FROM rule_evaluations", []).unwrap();
    conn.execute("DELETE FROM rule_actions", []).unwrap();
    assert!(due(&conn, 1000).is_empty());
}

#[test]
fn only_bodies_rules_acting_on_arrival_wait_on_hold_up_a_sync() {
    let conn = db();
    let age = Rule {
        id: "age".into(),
        trigger: RuleTrigger::Age,
        delay_seconds: 60,
        ..body_rule("body", "x", false)
    };
    set_rules(&conn, vec![age]).unwrap();
    mail(&conn, 1, false, 100);
    let waiting = |arrivals| {
        rule_body_waiting_uids(&conn, "a", EXISTING_RULE_FOLDER, 1000, 50, arrivals).unwrap()
    };
    assert_eq!(waiting(false), [1]);
    assert!(waiting(true).is_empty());
}

#[test]
fn mail_cached_with_only_html_is_read_as_shown() {
    let conn = db();
    set_rules(&conn, vec![body_rule("body", "invoice", false)]).unwrap();
    mail(&conn, 1, false, 100);
    mail(&conn, 2, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1, 2]).unwrap();
    conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
        .unwrap();
    let html = |uid: u32, html: &str| {
        conn.execute(
            "UPDATE messages SET json = json_set(json, '$.body_html', ?2) WHERE uid = ?1",
            params![uid, html],
        )
        .unwrap();
    };
    // As the body of mail downloaded now is: markup gone, text kept.
    html(1, "<p>Your <span class=\"x\">invoice</span> is ready</p>");
    html(2, "<p class=\"invoice\">Hello</p>");
    let uids: Vec<u32> = due(&conn, 1000).iter().map(|action| action.uid).collect();
    assert_eq!(uids, [1]);
}

#[test]
fn subject_or_body_finds_either() {
    let conn = db();
    set_rules(&conn, vec![body_rule("subject_or_body", "LOGIN", false)]).unwrap();
    mail(&conn, 1, false, 100); // Its subject is "login code".
    mail(&conn, 2, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1, 2]).unwrap();
    conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
        .unwrap();
    conn.execute("UPDATE messages SET subject = 'hello' WHERE uid = 2", [])
        .unwrap();
    body(&conn, 1, "");
    body(&conn, 2, "your login link");
    let uids: Vec<u32> = due(&conn, 1000).iter().map(|action| action.uid).collect();
    assert_eq!(uids, [1, 2]);
}

#[test]
fn starting_to_read_the_body_does_not_replay_old_arrivals() {
    let conn = db();
    set_rules(&conn, vec![rule(RuleTrigger::Arrival, 0)]).unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    finish_rule_actions(&conn, "a", "INBOX", &[1], 0).unwrap();
    body(&conn, 1, "invoice");
    set_rules(&conn, vec![body_rule("body", "invoice", false)]).unwrap();
    assert!(due(&conn, store_now()).is_empty());
}

fn store_now() -> i64 {
    crate::store::now_unix() + 10
}

#[test]
fn body_conditions_need_text_and_received_mail() {
    let conn = db();
    assert!(set_rules(&conn, vec![body_rule("body", " ", false)]).is_err());
    let sent = Rule {
        sent: true,
        ..body_rule("body", "x", false)
    };
    assert!(set_rules(&conn, vec![sent]).is_err());
    assert_eq!(
        body_rule("subject_or_body", "x", false).compat_query(),
        "is:read AND is:unread"
    );
}

#[test]
fn read_delay_tracks_transitions_and_unread_cancels_the_clock() {
    let conn = db();
    set_rules(&conn, vec![rule(RuleTrigger::Read, 300)]).unwrap();
    mail(&conn, 1, false, 100);
    mail(&conn, 2, true, 100); // Already-read historical mail has no read event.
    assert!(due(&conn, 1000).is_empty());
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    conn.execute(
        "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
        [],
    )
    .unwrap();
    mail(&conn, 1, true, 100); // Repeated seen flags must not restart the delay.
    assert!(due(&conn, 1299).is_empty());
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, false).unwrap();
    assert!(due(&conn, 1300).is_empty());
    // Bulk flag changes use the same clock as per-message operations.
    conn.execute("UPDATE messages SET seen = 1 WHERE uid = 1", [])
        .unwrap();
    conn.execute(
        "UPDATE rule_events SET read_at = 2000 WHERE read_at IS NOT NULL",
        [],
    )
    .unwrap();
    assert!(due(&conn, 2299).is_empty());
    assert_eq!(due(&conn, 2300)[0].uid, 1);
}

#[test]
fn age_rules_include_existing_mail_and_respect_conditions_and_disable() {
    let conn = db();
    mail(&conn, 1, false, 100);
    mail(&conn, 2, false, 901);
    mail(&conn, 3, false, 0); // Unknown dates must not look infinitely old.
    let mut rule = rule(RuleTrigger::Age, 100);
    rule.query = "subject:login".into();
    rule.accounts = vec!["other".into()];
    set_rules(&conn, vec![rule.clone()]).unwrap();
    assert!(due(&conn, 1000).is_empty());
    rule.accounts.clear();
    rule.disabled = true;
    set_rules(&conn, vec![rule.clone()]).unwrap();
    assert!(due(&conn, 1000).is_empty());
    rule.disabled = false;
    rule.query = "subject:unmatched".into();
    set_rules(&conn, vec![rule.clone()]).unwrap();
    assert!(due(&conn, 1000).is_empty());
    rule.query = "subject:login".into();
    set_rules(&conn, vec![rule]).unwrap();
    let actions = due(&conn, 1000);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].uid, 1);
    conn.execute("DELETE FROM messages WHERE uid = 1", [])
        .unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM rule_firings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn newly_created_read_rules_only_consider_later_events_even_in_the_same_second() {
    let conn = db();
    mail(&conn, 1, false, 100);
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    set_rules(&conn, vec![rule(RuleTrigger::Read, 300)]).unwrap();
    conn.execute(
        "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
        [],
    )
    .unwrap();
    assert!(due(&conn, 2000).is_empty());
    // A fresh transition on this same message is eligible. Eligibility uses
    // sequences rather than wall time, so ties in event seconds are safe.
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, false).unwrap();
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    conn.execute(
        "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
        [],
    )
    .unwrap();
    assert_eq!(due(&conn, 2000)[0].uid, 1);
}

#[test]
fn changing_an_immediate_rule_to_delayed_does_not_replay_old_arrivals() {
    let conn = db();
    set_rules(&conn, vec![rule(RuleTrigger::Arrival, 0)]).unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    finish_rule_actions(&conn, "a", "INBOX", &[1], 0).unwrap();
    set_rules(&conn, vec![rule(RuleTrigger::Arrival, 300)]).unwrap();
    mail(&conn, 2, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[2]).unwrap();
    conn.execute(
        "UPDATE rule_events SET arrived_at = 1000 WHERE arrived_at IS NOT NULL",
        [],
    )
    .unwrap();
    let actions = due(&conn, 2000);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].uid, 2);
}

#[test]
fn unchanged_rules_keep_their_pending_event_clocks() {
    let conn = db();
    let original = rule(RuleTrigger::Read, 300);
    set_rules(&conn, vec![original.clone()]).unwrap();
    mail(&conn, 1, false, 100);
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    conn.execute(
        "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
        [],
    )
    .unwrap();
    let edited = Rule {
        move_to: "trash".into(),
        ..original
    };
    set_rules(&conn, vec![edited]).unwrap();
    assert!(due(&conn, 1299).is_empty());
    assert!(due(&conn, 1300)[0].trash);
}

#[test]
fn age_checks_are_chunked_and_nonmatching_events_are_not_researched() {
    let conn = db();
    let timed = Rule {
        query: "subject:missing".into(),
        ..rule(RuleTrigger::Age, 100)
    };
    set_rules(&conn, vec![timed.clone()]).unwrap();
    for uid in 1..=1001 {
        mail(&conn, uid, false, 100);
    }
    assert!(due(&conn, 1000).is_empty());
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM rule_evaluations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1001);
    let changes = conn.total_changes();
    assert!(due(&conn, 2000).is_empty());
    assert_eq!(conn.total_changes(), changes);
    // Changing the search still evaluates existing aged mail.
    set_rules(
        &conn,
        vec![Rule {
            query: "subject:login".into(),
            ..timed
        }],
    )
    .unwrap();
    assert_eq!(due(&conn, 2000).len(), 1001);
}

#[test]
fn mutable_conditions_are_rechecked_after_the_deadline() {
    for trigger in [RuleTrigger::Arrival, RuleTrigger::Age] {
        for query in ["is:read", "is:starred", "tag:ready", "has:attachment"] {
            let conn = db();
            set_rules(
                &conn,
                vec![Rule {
                    query: query.into(),
                    ..rule(trigger, 300)
                }],
            )
            .unwrap();
            mail(&conn, 1, false, 100);
            apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
            conn.execute(
                "UPDATE rule_events SET arrived_at = 1000 WHERE arrived_at IS NOT NULL",
                [],
            )
            .unwrap();
            assert!(due(&conn, 2000).is_empty(), "{trigger:?} {query}");
            let changes = conn.total_changes();
            assert!(due(&conn, 2100).is_empty());
            assert_eq!(
                conn.total_changes(),
                changes,
                "unchanged {trigger:?} {query}"
            );
            match query {
                "is:read" => {
                    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap()
                }
                "is:starred" => {
                    crate::store::update_message_starred(&conn, "a", "INBOX", 1, true).unwrap()
                }
                "tag:ready" => crate::store::change_tags(
                    &conn,
                    "a",
                    &["1@example.com".into()],
                    &["ready".into()],
                    &[],
                    false,
                )
                .unwrap(),
                _ => {
                    conn.execute("UPDATE messages SET files = '[{}]' WHERE uid = 1", [])
                        .unwrap();
                }
            }
            assert_eq!(due(&conn, 2000).len(), 1, "{trigger:?} {query}");
        }
    }
}

#[test]
fn tag_changes_invalidate_all_copies_but_timestamp_refreshes_keep_the_cache() {
    let conn = db();
    set_rules(
        &conn,
        vec![Rule {
            query: "tag:ready".into(),
            ..rule(RuleTrigger::Read, 300)
        }],
    )
    .unwrap();
    mail(&conn, 1, false, 100);
    crate::store::upsert_messages(
        &conn,
        "a",
        "Saved",
        &[MessageHeader {
            uid: 9,
            message_id: "<1@example.com>".into(),
            ..Default::default()
        }],
    )
    .unwrap();
    for (folder, uid) in [("INBOX", 1), ("Saved", 9)] {
        crate::store::update_message_seen(&conn, "a", folder, uid, true).unwrap();
    }
    conn.execute("UPDATE rule_events SET read_at = 1000", [])
        .unwrap();
    let evaluate = || {
        apply_due_rules(&conn, "a", "INBOX", 7, 2000).unwrap();
        apply_due_rules(&conn, "a", "Saved", 7, 2000).unwrap();
    };
    let cached = || {
        conn.query_row("SELECT COUNT(*) FROM rule_evaluations", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
    };
    evaluate();
    assert_eq!(cached(), 2);
    conn.execute("INSERT INTO message_tags(account, tag_key, tag, state) VALUES ('a', '1@example.com', 'ready', 2)", []).unwrap();
    assert_eq!(cached(), 0);
    evaluate();
    assert_eq!(cached(), 2);
    conn.execute("UPDATE message_tags SET updated_at = 1", [])
        .unwrap();
    let changes = conn.total_changes();
    evaluate();
    assert_eq!(conn.total_changes(), changes);
    conn.execute("UPDATE message_tags SET state = 0", [])
        .unwrap();
    assert_eq!(cached(), 0);
    evaluate();
    assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 1);
    assert_eq!(pending_rule_actions(&conn, "a", "Saved").unwrap().len(), 1);
    conn.execute("DELETE FROM message_tags", []).unwrap();
    assert_eq!(cached(), 0);
    let key = crate::store::tags::TAG_KEY_SQL;
    let plan: String = conn.query_row(&format!(
        "EXPLAIN QUERY PLAN SELECT id FROM messages WHERE uid <> 0 AND account = 'a' AND {key} = '1@example.com'"
    ), [], |r| r.get(3)).unwrap();
    assert!(plan.contains("messages_tag_key_idx"), "{plan}");
}

#[test]
fn header_corrections_invalidate_negative_checks() {
    let conn = db();
    set_rules(
        &conn,
        vec![Rule {
            query: "subject:corrected".into(),
            ..rule(RuleTrigger::Age, 300)
        }],
    )
    .unwrap();
    mail(&conn, 1, false, 100);
    assert!(due(&conn, 2000).is_empty());
    conn.execute(
        "UPDATE messages SET subject = 'corrected' WHERE uid = 1",
        [],
    )
    .unwrap();
    assert_eq!(due(&conn, 2000).len(), 1);
}

#[test]
fn reenabled_event_rules_ignore_events_from_while_disabled() {
    for trigger in [RuleTrigger::Read, RuleTrigger::Arrival] {
        let conn = db();
        let mut timed = rule(trigger, 300);
        set_rules(&conn, vec![timed.clone()]).unwrap();
        timed.disabled = true;
        set_rules(&conn, vec![timed.clone()]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        timed.disabled = false;
        set_rules(&conn, vec![timed]).unwrap();
        conn.execute(
            "UPDATE rule_events SET arrived_at = 1000, read_at = 1000",
            [],
        )
        .unwrap();
        assert!(due(&conn, 2000).is_empty());
        mail(&conn, 2, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[2]).unwrap();
        crate::store::update_message_seen(&conn, "a", "INBOX", 2, true).unwrap();
        conn.execute(
            "UPDATE rule_events SET arrived_at = 1000, read_at = 1000",
            [],
        )
        .unwrap();
        let actions = due(&conn, 2000);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].uid, 2);
    }
}

#[test]
fn expanding_account_scope_does_not_replay_past_events() {
    let conn = db();
    let mut timed = Rule {
        accounts: vec!["other".into()],
        ..rule(RuleTrigger::Read, 300)
    };
    set_rules(&conn, vec![timed.clone()]).unwrap();
    mail(&conn, 1, false, 100);
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    timed.accounts.clear();
    set_rules(&conn, vec![timed]).unwrap();
    conn.execute("UPDATE rule_events SET read_at = 1000", [])
        .unwrap();
    assert!(due(&conn, 2000).is_empty());
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, false).unwrap();
    crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
    conn.execute("UPDATE rule_events SET read_at = 1000", [])
        .unwrap();
    assert_eq!(due(&conn, 2000).len(), 1);
}

#[test]
fn scheduled_actions_preserve_existing_automatic_work() {
    let conn = db();
    let immediate = Rule {
        id: "immediate".into(),
        mark_read: true,
        ..Default::default()
    };
    set_rules(&conn, vec![immediate, rule(RuleTrigger::Age, 100)]).unwrap();
    mail(&conn, 1, false, 100);
    apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
    let actions = due(&conn, 1000);
    assert_eq!(actions.len(), 2);
    assert!(actions.iter().any(|a| a.mark_read && a.run == 0));
    assert!(actions.iter().any(|a| a.star && a.run < 0));
    assert!(running_rules(&conn).unwrap().is_empty());
    stop_rule(&conn, "timed").unwrap();
    assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 2);
}
