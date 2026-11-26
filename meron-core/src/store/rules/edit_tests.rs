use super::*;
use serde_json::json;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    super::super::run_migrations(&conn).unwrap();
    conn
}

#[test]
fn partial_rule_edits_keep_scope_and_disabled_until_explicitly_changed() {
    let conn = db();
    conn.execute(
        "INSERT INTO accounts (id, engine, provider, email, display_name, avatar_url, config, created_at, updated_at)
         VALUES ('work', 'mail', 'imap', 'work@example.com', '', '', '{}', 0, 0)",
        [],
    )
    .unwrap();
    super::super::ensure_folder(&conn, "work", "INBOX").unwrap();
    assert_eq!(
        rule_targets(&conn).unwrap(),
        json!({"accounts": [{"id": "work", "name": "work@example.com", "folders": [{"id": "INBOX", "name": "INBOX"}]}]})
    );
    let original = Rule {
        accounts: vec!["work".into()],
        disabled: true,
        tags: vec!["receipts".into()],
        ..Rule::default()
    };
    let saved = edit_rule(&conn, RuleEdit::Create(original)).unwrap();
    let patch = |value: serde_json::Value| {
        RuleEdit::Update(saved.id.clone(), value.as_object().unwrap().clone())
    };
    let updated = edit_rule(&conn, patch(json!({ "star": true }))).unwrap();
    assert_eq!(updated.accounts, saved.accounts);
    assert!(updated.disabled);
    assert_eq!(updated.tags, saved.tags);
    assert!(edit_rule(&conn, patch(json!({ "accounts": ["unknown"] }))).is_err());
    assert!(
        edit_rule(
            &conn,
            patch(json!({
                "copy_account": "work", "copy_folder": "missing"
            }))
        )
        .is_err()
    );
    assert_eq!(rules_for_editing(&conn).unwrap(), vec![updated.clone()]);
    let updated = edit_rule(
        &conn,
        patch(json!({
            "accounts": [],
            "disabled": false,
            "copy_account": "work",
            "copy_folder": "INBOX"
        })),
    )
    .unwrap();
    assert!(updated.accounts.is_empty());
    assert!(!updated.disabled);
    assert_eq!(updated.copy_folder, "INBOX");
    super::super::delete_folder(&conn, "work", "INBOX").unwrap();
    let disabled = edit_rule(&conn, patch(json!({ "disabled": true }))).unwrap();
    assert!(disabled.disabled);
    assert_eq!(disabled.copy_folder, "INBOX");
    assert!(edit_rule(&conn, patch(json!({ "disabled": false }))).is_err());
}

#[test]
fn age_rules_can_only_be_enabled_in_the_app() {
    let conn = db();
    let active = Rule {
        trigger: RuleTrigger::Age,
        star: true,
        ..Rule::default()
    };
    assert!(edit_rule(&conn, RuleEdit::Create(active.clone())).is_err());
    let disabled = Rule {
        disabled: true,
        ..active
    };
    let saved = edit_rule(&conn, RuleEdit::Create(disabled)).unwrap();
    let patch = |value: serde_json::Value| {
        RuleEdit::Update(saved.id.clone(), value.as_object().unwrap().clone())
    };
    assert!(edit_rule(&conn, patch(json!({ "disabled": false }))).is_err());

    let enabled = Rule {
        disabled: false,
        ..saved.clone()
    };
    set_rules(&conn, vec![enabled.clone()]).unwrap();
    assert!(edit_rule(&conn, patch(json!({ "move_to": "trash" }))).is_err());
    assert_eq!(rules_for_editing(&conn).unwrap(), vec![enabled]);
    let saved = edit_rule(
        &conn,
        patch(json!({ "move_to": "trash", "disabled": true })),
    )
    .unwrap();
    assert_eq!(saved.move_to, "trash");
    assert!(saved.disabled);
}

#[test]
fn editing_never_overwrites_an_unreadable_rule_list() {
    let conn = db();
    let stored = json!([{ "id": "legacy", "trigger": "future_trigger", "star": true }]);
    super::super::setting_set(&conn, RULES_KEY, &stored).unwrap();
    let rule = Rule {
        star: true,
        ..Rule::default()
    };
    assert!(rules_for_editing(&conn).is_err());
    assert!(edit_rule(&conn, RuleEdit::Create(rule.clone())).is_err());
    assert!(set_rules_if_current(&conn, &[], vec![rule]).is_err());
    assert_eq!(rule_data(&conn).unwrap(), Some(stored));
}

#[test]
fn stale_rules_cannot_overwrite_the_list_or_run() {
    let conn = db();
    let rule = Rule {
        star: true,
        ..Rule::default()
    };
    let (snapshot, saved) = set_rules_if_current(&conn, &[], vec![rule]).unwrap();
    assert!(saved);
    let mut changed = snapshot[0].clone();
    changed.mark_read = true;
    set_rules(&conn, vec![changed.clone()]).unwrap();

    let (current, saved) = set_rules_if_current(&conn, &snapshot, vec![]).unwrap();
    assert!(!saved);
    assert_eq!(current, vec![changed.clone()]);
    assert!(queue_existing_rule_checked(&conn, &snapshot[0]).is_err());
    assert!(
        queue_existing_rule_checked(&conn, &changed)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn editing_or_deleting_requires_one_matching_id() {
    let conn = db();
    let rule = Rule {
        id: "duplicate".into(),
        star: true,
        ..Rule::default()
    };
    let saved = set_rules(&conn, vec![rule.clone(), rule]).unwrap();
    for id in ["", "missing", "duplicate"] {
        assert!(edit_rule(&conn, RuleEdit::Delete(id.into())).is_err());
        assert!(edit_rule(&conn, RuleEdit::Update(id.into(), Default::default())).is_err());
    }
    assert_eq!(rules_for_editing(&conn).unwrap(), saved);
}
