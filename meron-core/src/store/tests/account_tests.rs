use super::*;

#[test]
fn delete_account_removes_account_scoped_state_only() {
    let conn = test_conn();
    for account in ["acct", "other"] {
        conn.execute(
            "INSERT INTO accounts(id, email) VALUES(?1, ?1)",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO folders(account, name) VALUES(?1, 'INBOX')",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid) VALUES(?1, 'INBOX', '1', 1)",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO folder_state(account, folder, uid_next) VALUES(?1, 'INBOX', 2)",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title) VALUES(?2, ?1, ?3, 'Feed')",
            params![
                account,
                format!("feed-{account}"),
                format!("https://{account}.example")
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO account_secrets(account_id, blob) VALUES(?1, 'secret')",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(?1, 'message-id:seen@example.com', 1)",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mail_search_hits(
               token, account, query, scope, position, folder, uid, created_at
             ) VALUES(?2, ?1, 'query', '[\"INBOX\"]', 0, 'INBOX', 1, 1)",
            params![account, format!("token-{account}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mail_search_snapshots(token, account, query, scope, created_at)
             VALUES(?2, ?1, 'query', '[\"INBOX\"]', 1)",
            params![account, format!("token-{account}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mail_search_pending(token, account, folder, uid, date)
             VALUES(?2, ?1, 'INBOX', 2, 1)",
            params![account, format!("token-{account}")],
        )
        .unwrap();
    }

    delete_account(&conn, "acct").unwrap();

    for (table, column) in [
        ("accounts", "id"),
        ("folders", "account"),
        ("messages", "account"),
        ("folder_state", "account"),
        ("subscriptions", "account"),
        ("account_secrets", "account_id"),
        ("observed_mail_identities", "account"),
        ("mail_search_hits", "account"),
        ("mail_search_snapshots", "account"),
        ("mail_search_pending", "account"),
    ] {
        let deleted_count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {column} = 'acct'"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deleted_count, 0, "{table} retained deleted account rows");

        let other_count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {column} = 'other'"),
                [],
                |row| row.get(0),
            )
            .unwrap();
        let expected = if table == "observed_mail_identities" {
            2
        } else {
            1
        };
        assert_eq!(
            other_count, expected,
            "{table} removed another account's row"
        );
    }
}

#[test]
fn prefs_resolve_engine_default_and_persist_without_clobbering() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine) VALUES('rss-1', 'rss')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine) VALUES('mail-1', 'mail')",
        [],
    )
    .unwrap();

    // Unset: resolved by engine (RSS on, mail off).
    assert!(load_remote_images(&conn, "rss-1").unwrap());
    assert!(!load_remote_images(&conn, "mail-1").unwrap());
    assert!(!load_remote_images(&conn, "missing").unwrap());

    // A sibling pref must survive an images toggle (json_set is in-place).
    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'mail-1'",
        [],
    )
    .unwrap();
    set_load_remote_images(&conn, "mail-1", true).unwrap();
    assert!(load_remote_images(&conn, "mail-1").unwrap());
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'mail-1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        prefs.contains("\"other\""),
        "sibling pref clobbered: {prefs}"
    );

    // Explicit override of the engine default round-trips.
    set_load_remote_images(&conn, "rss-1", false).unwrap();
    assert!(!load_remote_images(&conn, "rss-1").unwrap());
}

#[test]
fn allowed_senders_load_remote_images_while_the_account_toggle_stays_off() {
    let conn = test_conn();
    for id in ["mail-1", "mail-2"] {
        conn.execute(
            "INSERT INTO accounts(id, engine) VALUES(?1, 'mail')",
            params![id],
        )
        .unwrap();
    }

    let policy = remote_image_policy(&conn, "mail-1").unwrap();
    assert!(!policy.all);
    assert!(!policy.allows("news@example.com"));

    // The allowlist is app-wide: one entry lifts the block in every account,
    // while each account's own toggle stays off. Stored forms are normalized on
    // read, so a "Name <addr>" entry and casing still match.
    setting_set(
        &conn,
        REMOTE_IMAGE_SENDERS_KEY,
        &json!(["News <News@Example.com>", "news@example.com", ""]),
    )
    .unwrap();
    assert_eq!(
        remote_image_senders(&conn).unwrap(),
        vec!["news@example.com".to_string()]
    );
    for id in ["mail-1", "mail-2"] {
        let policy = remote_image_policy(&conn, id).unwrap();
        assert!(!policy.all);
        assert!(policy.allows("News@Example.COM"));
        assert!(!policy.allows("someone@else.test"));
        assert!(!policy.allows(""));
    }

    // The account-wide toggle still allows every sender on its own.
    set_load_remote_images(&conn, "mail-1", true).unwrap();
    assert!(
        remote_image_policy(&conn, "mail-1")
            .unwrap()
            .allows("anyone@example.com")
    );
}

#[test]
fn conversation_html_defaults_on_and_respects_explicit_overrides() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES
                ('unset', 'mail', 'custom', '{}'),
                ('plain', 'mail', 'custom', '{\"conversation_html\":false}'),
                ('html', 'mail', 'custom', '{\"conversation_html\":true}')",
        [],
    )
    .unwrap();

    let accounts = list_accounts(&conn).unwrap();
    let by_id = |id: &str| {
        accounts
            .iter()
            .find(|account| account["id"] == id)
            .expect("account present")
    };

    assert_eq!(by_id("unset")["conversation_html"], true);
    assert_eq!(by_id("plain")["conversation_html"], false);
    assert_eq!(by_id("html")["conversation_html"], true);
}

#[test]
fn chat_wallpaper_round_trips_and_can_be_cleared() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
        [],
    )
    .unwrap();

    let wallpaper = json!({"kind":"preset","presetId":"grid"});
    set_account_pref_json(&conn, "acct", "chat_wallpaper", Some(wallpaper.clone())).unwrap();
    let accounts = list_accounts(&conn).unwrap();
    assert_eq!(accounts[0]["chat_wallpaper"], wallpaper);

    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'acct'",
        [],
    )
    .unwrap();
    set_account_pref_json(&conn, "acct", "chat_wallpaper", None).unwrap();
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'acct'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let value: Value = serde_json::from_str(&prefs).unwrap();
    assert_eq!(value["chat_wallpaper"], Value::Null);
    assert_eq!(value["other"], 7);
}

#[test]
fn account_color_is_normalized_validated_and_clearable() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
        [],
    )
    .unwrap();

    assert_eq!(list_accounts(&conn).unwrap()[0]["color"], Value::Null);

    set_account_color(&conn, "acct", Some(" #3B82F6 ")).unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0]["color"], "#3b82f6");

    for bad in ["red", "#12345", "3b82f6", "#3b82fg"] {
        assert!(
            set_account_color(&conn, "acct", Some(bad)).is_err(),
            "{bad}"
        );
    }
    assert_eq!(list_accounts(&conn).unwrap()[0]["color"], "#3b82f6");

    set_account_color(&conn, "acct", Some("")).unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0]["color"], Value::Null);
    set_account_color(&conn, "acct", Some("#10b981")).unwrap();
    set_account_color(&conn, "acct", None).unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0]["color"], Value::Null);

    // A hand-edited, unreadable colour lists as unset rather than leaking through.
    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.color', 'blue') WHERE id = 'acct'",
        [],
    )
    .unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0]["color"], Value::Null);
}

#[test]
fn signature_pref_round_trips_and_can_be_cleared() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
        [],
    )
    .unwrap();

    // Absent means "follow the app-wide signature".
    assert_eq!(list_accounts(&conn).unwrap()[0]["signature"], Value::Null);

    let signature =
        AccountSignature::from_param(Some(&json!({"mode":"custom","html":"<p>Ping</p>"})))
            .unwrap()
            .unwrap();
    set_account_pref_json(&conn, "acct", "signature", Some(json!(signature))).unwrap();
    assert_eq!(
        list_accounts(&conn).unwrap()[0]["signature"],
        json!({"mode":"custom","html":"<p>Ping</p>"})
    );

    set_account_pref_json(&conn, "acct", "signature", None).unwrap();
    assert_eq!(list_accounts(&conn).unwrap()[0]["signature"], Value::Null);
}

#[test]
fn an_unreadable_pref_does_not_reset_the_others() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    // `signature` carries a mode this build cannot read, next to prefs that are
    // perfectly valid: only the signature falls back to its default.
    conn.execute(
        r#"INSERT INTO accounts(id, engine, provider, prefs)
           VALUES('acct', 'mail', 'custom',
                  '{"muted":true,"paused":true,"included_in_unified":false,
                    "conversation_html":false,"signature":"<p>Ping</p>"}')"#,
        [],
    )
    .unwrap();

    let account = list_accounts(&conn).unwrap().remove(0);
    assert_eq!(account["muted"], true);
    assert_eq!(account["paused"], true);
    assert_eq!(account["included_in_unified"], false);
    assert_eq!(account["conversation_html"], false);
    assert_eq!(account["signature"], Value::Null);
}

#[test]
fn signature_param_validates_its_mode_and_size() {
    assert!(AccountSignature::from_param(None).unwrap().is_none());
    assert!(
        AccountSignature::from_param(Some(&Value::Null))
            .unwrap()
            .is_none()
    );
    assert!(AccountSignature::from_param(Some(&json!({"mode":"sometimes"}))).is_err());
    assert!(AccountSignature::from_param(Some(&json!("<p>Ping</p>"))).is_err());
    assert!(
        AccountSignature::from_param(Some(&json!({
            "mode": "custom",
            "html": "x".repeat(MAX_SIGNATURE_HTML + 1),
        })))
        .is_err()
    );

    // The html is kept in every mode, so toggling back restores what was written.
    let kept = AccountSignature::from_param(Some(&json!({"mode":"none","html":" <p>Ping</p> "})))
        .unwrap()
        .unwrap();
    assert_eq!(kept.mode, SignatureMode::None);
    assert_eq!(kept.html, "<p>Ping</p>");
}

#[test]
fn save_sent_copy_pref_round_trips_and_can_be_cleared() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, prefs) VALUES('acct', 'mail', 'custom', '{}')",
        [],
    )
    .unwrap();

    assert_eq!(save_sent_copy_pref(&conn, "acct").unwrap(), None);
    set_account_pref_json(&conn, "acct", "save_sent_copy", Some(json!(false))).unwrap();
    assert_eq!(save_sent_copy_pref(&conn, "acct").unwrap(), Some(false));
    assert_eq!(list_accounts(&conn).unwrap()[0]["save_sent_copy"], false);

    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'acct'",
        [],
    )
    .unwrap();
    set_account_pref_json(&conn, "acct", "save_sent_copy", None).unwrap();
    assert_eq!(save_sent_copy_pref(&conn, "acct").unwrap(), None);
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'acct'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let value: Value = serde_json::from_str(&prefs).unwrap();
    assert_eq!(value["save_sent_copy"], Value::Null);
    assert_eq!(value["other"], 7);
}

#[test]
fn rss_sync_interval_defaults_and_persists_without_clobbering() {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider) VALUES('rss-1', 'rss', 'rss')",
        [],
    )
    .unwrap();

    let accounts = list_accounts(&conn).unwrap();
    assert_eq!(
        accounts[0]["rss_sync_interval_minutes"],
        DEFAULT_RSS_SYNC_INTERVAL_MINUTES
    );

    conn.execute(
        "UPDATE accounts SET prefs = json_set(prefs, '$.other', 7) WHERE id = 'rss-1'",
        [],
    )
    .unwrap();
    set_account_pref_u64(&conn, "rss-1", "rss_sync_interval_minutes", 30).unwrap();

    let accounts = list_accounts(&conn).unwrap();
    assert_eq!(accounts[0]["rss_sync_interval_minutes"], 30);
    let prefs: String = conn
        .query_row("SELECT prefs FROM accounts WHERE id = 'rss-1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        prefs.contains("\"other\""),
        "sibling pref clobbered: {prefs}"
    );
}

/// The account address and the mailbox login are separate fields, so both are
/// identities the user owns — a custom account whose server authenticates by
/// user name has them differ, and picking its own address used to fall through
/// to the login.
#[test]
fn resolve_send_from_honors_account_address_login_and_aliases() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO accounts(id, engine, email, sender_name, prefs)
         VALUES('acct', 'mail', 'me@example.com', 'Me',
                '{\"aliases\":[{\"email\":\"Sales@Example.JP\",\"name\":\"Sales\"},
                               {\"email\":\"quiet@example.com\"}]}')",
        [],
    )
    .unwrap();
    let resolve = |requested| resolve_send_from(&conn, "acct", "login-name", requested);

    // No request: the account's own address, not the login it authenticates as.
    assert_eq!(
        resolve("").unwrap(),
        ("me@example.com".to_string(), "Me".to_string())
    );
    assert_eq!(
        resolve("ME@example.com").unwrap(),
        ("me@example.com".to_string(), "Me".to_string())
    );
    // The login is still a legitimate send-as identity.
    assert_eq!(
        resolve("login-name").unwrap(),
        ("login-name".to_string(), "Me".to_string())
    );
    // Aliases match case-insensitively and keep their own display name.
    assert_eq!(
        resolve(" sales@example.jp ").unwrap(),
        ("Sales@Example.JP".to_string(), "Sales".to_string())
    );
    // A blank alias name inherits the account's sender name.
    assert_eq!(
        resolve("quiet@example.com").unwrap(),
        ("quiet@example.com".to_string(), "Me".to_string())
    );
}

/// An address the account doesn't own must fail the send rather than quietly
/// going out as someone else — a silent substitution is invisible to the sender.
#[test]
fn resolve_send_from_rejects_an_unowned_address() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO accounts(id, engine, email, sender_name, prefs)
         VALUES('acct', 'mail', 'me@example.com', 'Me', '{}')",
        [],
    )
    .unwrap();

    let err = resolve_send_from(&conn, "acct", "me@example.com", "someone@else.com").unwrap_err();
    assert!(
        err.to_string().contains("someone@else.com"),
        "error should name the rejected address: {err}"
    );
}

/// Accounts stored without an explicit address fall back to the login, matching
/// what `account.list` shows as the account's address.
#[test]
fn resolve_send_from_falls_back_to_the_login_without_an_account_address() {
    let conn = test_conn();
    conn.execute(
        "INSERT INTO accounts(id, engine, email, sender_name) VALUES('acct', 'mail', '', 'Me')",
        [],
    )
    .unwrap();

    assert_eq!(
        resolve_send_from(&conn, "acct", "me@example.com", "").unwrap(),
        ("me@example.com".to_string(), "Me".to_string())
    );
}

/// A minimal mail account so proxy persistence can be exercised without the
/// rest of the connection metadata mattering.
fn proxy_test_account(conn: &Connection, id: &str, proxy: crate::proxy::ProxyChoice) {
    let creds = proxy_test_creds(proxy);
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: "user@example.com".to_string(),
        display_name: String::new(),
        avatar_url: String::new(),
        sender_name: String::new(),
    };
    upsert_account(conn, id, &meta, &creds).unwrap();
}

fn proxy_test_creds(proxy: crate::proxy::ProxyChoice) -> crate::imap::Creds {
    crate::imap::Creds {
        host: "imap.example.com".to_string(),
        port: 993,
        user: "user@example.com".to_string(),
        password: String::new(),
        tls: true,
        starttls: false,
        smtp_host: "smtp.example.com".to_string(),
        smtp_port: 587,
        smtp_tls: false,
        smtp_starttls: true,
        auth_type: "password".to_string(),
        access_token: None,
        refresh_token: None,
        token_expires_at: 0,
        oauth_client_id: String::new(),
        oauth_client_secret: String::new(),
        oauth_token_url: String::new(),
        oauth_scope: String::new(),
        proxy,
        cert_pin: None,
        smtp_cert_pin: None,
        rejected_grant: None,
    }
}

fn stored_creds(conn: &Connection, id: &str) -> crate::imap::Creds {
    load_accounts(conn)
        .unwrap()
        .into_iter()
        .find(|(account, _)| account == id)
        .map(|(_, creds)| creds)
        .expect("account not found")
}

fn stored_proxy(conn: &Connection, id: &str) -> crate::proxy::ProxyChoice {
    stored_creds(conn, id).proxy
}

/// A sign-in the provider refused has to stay refused across a restart, and
/// saving the account again — which is what reconnecting does — has to lift it.
#[test]
fn rejected_sign_in_persists_until_the_account_is_saved_again() {
    let conn = test_conn();
    let creds = proxy_test_creds(crate::proxy::ProxyChoice::Global);
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: "user@example.com".to_string(),
        display_name: String::new(),
        avatar_url: String::new(),
        sender_name: String::new(),
    };
    upsert_account(&conn, "acct", &meta, &creds).unwrap();
    assert_eq!(stored_creds(&conn, "acct").rejected_grant, None);

    let refused = crate::imap::grant_fingerprint("old-token");
    set_account_rejected_grant(&conn, "acct", &refused).unwrap();
    let mut stored = stored_creds(&conn, "acct");
    assert_eq!(stored.rejected_grant.as_deref(), Some(refused.as_str()));
    // Nothing else in the config moved.
    assert_eq!(stored.host, creds.host);
    assert_eq!(stored.token_expires_at, creds.token_expires_at);

    // The verdict is on that token alone (secrets are applied after loading).
    stored.refresh_token = Some("old-token".to_string());
    assert!(stored.auth_rejected());
    stored.refresh_token = Some("new-token".to_string());
    assert!(!stored.auth_rejected());
    stored.refresh_token = None;
    assert!(!stored.auth_rejected());

    upsert_account(&conn, "acct", &meta, &creds).unwrap();
    assert_eq!(stored_creds(&conn, "acct").rejected_grant, None);
}

/// The pin an account carries for a server whose certificate webpki refuses
/// (Proton Bridge and friends) has to survive a restart, or every launch would
/// re-prompt.
#[test]
fn account_cert_pin_round_trips_and_defaults_to_none() {
    let conn = test_conn();
    let mut creds = proxy_test_creds(crate::proxy::ProxyChoice::Global);
    creds.cert_pin = Some("a".repeat(64));
    creds.smtp_cert_pin = Some("b".repeat(64));
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: "user@example.com".to_string(),
        display_name: String::new(),
        avatar_url: String::new(),
        sender_name: String::new(),
    };
    upsert_account(&conn, "acct", &meta, &creds).unwrap();
    let stored = stored_creds(&conn, "acct");
    assert_eq!(stored.cert_pin.as_deref(), Some("a".repeat(64).as_str()));
    assert_eq!(
        stored.smtp_cert_pin.as_deref(),
        Some("b".repeat(64).as_str())
    );

    // Accounts written before pinning existed have no keys in their config JSON.
    conn.execute(
        "UPDATE accounts SET config = json_remove(config, '$.cert_pin', '$.smtp_cert_pin') WHERE id = 'acct'",
        [],
    )
    .unwrap();
    let stored = stored_creds(&conn, "acct");
    assert_eq!(stored.cert_pin, None);
    assert_eq!(stored.smtp_cert_pin, None);
}

/// Reconnecting resends the setup form, which has no field for the account's
/// proxy or the certificates it accepted. Saving credentials again must not
/// reset settings the form never carried.
#[test]
fn saving_an_account_again_keeps_settings_the_form_does_not_carry() {
    let conn = test_conn();
    let custom = crate::proxy::ProxyChoice::from_json(&json!({
        "mode": "socks5",
        "host": "127.0.0.1",
        "port": 9050,
    }));
    let mut creds = proxy_test_creds(custom.clone());
    creds.cert_pin = Some("a".repeat(64));
    creds.smtp_cert_pin = Some("b".repeat(64));
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: "user@example.com".to_string(),
        display_name: String::new(),
        avatar_url: String::new(),
        sender_name: String::new(),
    };
    upsert_account(&conn, "acct", &meta, &creds).unwrap();

    // What a reconnect builds: servers and credentials, nothing else.
    let mut reconnected = proxy_test_creds(crate::proxy::ProxyChoice::Global);
    let stored = load_account(&conn, "acct").unwrap().expect("account");
    reconnected.carry_over(
        &stored,
        crate::imap::OmittedSettings {
            proxy: true,
            cert_pin: true,
            smtp_cert_pin: true,
            password: true,
        },
    );
    upsert_account(&conn, "acct", &meta, &reconnected).unwrap();

    let after = stored_creds(&conn, "acct");
    assert_eq!(after.proxy, custom);
    assert_eq!(after.cert_pin.as_deref(), Some("a".repeat(64).as_str()));
    assert_eq!(
        after.smtp_cert_pin.as_deref(),
        Some("b".repeat(64).as_str())
    );
}

/// A setting the caller *did* send wins: that is how the user changes it, and
/// how accepting a new certificate replaces the pin.
#[test]
fn a_setting_that_was_sent_is_not_carried_over() {
    let mut creds = proxy_test_creds(crate::proxy::ProxyChoice::Direct);
    creds.cert_pin = Some("a".repeat(64));
    let mut replacement = proxy_test_creds(crate::proxy::ProxyChoice::Global);
    replacement.cert_pin = None;
    replacement.carry_over(
        &creds,
        crate::imap::OmittedSettings {
            proxy: false,
            cert_pin: false,
            smtp_cert_pin: true,
            password: false,
        },
    );

    assert_eq!(replacement.proxy, crate::proxy::ProxyChoice::Global);
    assert_eq!(replacement.cert_pin, None);
}

/// Editing an account's servers from Settings resends the form without a
/// password — the UI never holds one. An omitted password must keep the stored
/// credential rather than blank it, or saving a port change would lock the
/// account out.
#[test]
fn an_omitted_password_is_carried_over() {
    let mut stored = proxy_test_creds(crate::proxy::ProxyChoice::Direct);
    stored.password = "hunter2".to_string();

    let mut edited = proxy_test_creds(crate::proxy::ProxyChoice::Direct);
    edited.password = String::new();
    edited.port = 1143;
    edited.carry_over(
        &stored,
        crate::imap::OmittedSettings {
            proxy: false,
            cert_pin: false,
            smtp_cert_pin: false,
            password: true,
        },
    );

    assert_eq!(edited.password, "hunter2");
    // The edit itself still applies.
    assert_eq!(edited.port, 1143);
}

/// A password the caller *did* send replaces the stored one — that is how the
/// user changes it after a server-side reset.
#[test]
fn a_sent_password_replaces_the_stored_one() {
    let mut stored = proxy_test_creds(crate::proxy::ProxyChoice::Direct);
    stored.password = "old".to_string();

    let mut replacement = proxy_test_creds(crate::proxy::ProxyChoice::Direct);
    replacement.password = "new".to_string();
    replacement.carry_over(
        &stored,
        crate::imap::OmittedSettings {
            proxy: false,
            cert_pin: false,
            smtp_cert_pin: false,
            password: false,
        },
    );

    assert_eq!(replacement.password, "new");
}

/// Editing an existing account (server settings, reconnect) re-runs the upsert
/// with no avatar; the custom one the user picked must survive it.
#[test]
fn upsert_account_keeps_an_existing_avatar() {
    let conn = test_conn();
    proxy_test_account(&conn, "acct", crate::proxy::ProxyChoice::Direct);
    conn.execute(
        "UPDATE accounts SET avatar_url = ?1 WHERE id = ?2",
        rusqlite::params!["/media/avatars/acct.png", "acct"],
    )
    .unwrap();

    proxy_test_account(&conn, "acct", crate::proxy::ProxyChoice::Direct);

    let stored: String = conn
        .query_row(
            "SELECT avatar_url FROM accounts WHERE id = ?1",
            ["acct"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, "/media/avatars/acct.png");
}

/// Accepting a certificate for an account that already exists must not disturb
/// the rest of its connection settings — the user is not re-entering them.
#[test]
fn set_account_cert_pins_replaces_only_the_pins() {
    let conn = test_conn();
    proxy_test_account(&conn, "acct", crate::proxy::ProxyChoice::Direct);
    set_account_cert_pins(&conn, "acct", Some(&"c".repeat(64)), None).unwrap();

    let stored = stored_creds(&conn, "acct");
    assert_eq!(stored.cert_pin.as_deref(), Some("c".repeat(64).as_str()));
    assert_eq!(stored.smtp_cert_pin, None);
    assert_eq!(stored.host, "imap.example.com");
    assert_eq!(stored.smtp_port, 587);
    assert_eq!(stored.proxy, crate::proxy::ProxyChoice::Direct);

    // Clearing is explicit: pass no pin and the account stops trusting it.
    set_account_cert_pins(&conn, "acct", None, None).unwrap();
    assert_eq!(stored_creds(&conn, "acct").cert_pin, None);
}

#[test]
fn account_proxy_round_trips_and_defaults_to_global() {
    let conn = test_conn();
    let custom = crate::proxy::ProxyChoice::from_json(&json!({
        "mode": "socks5",
        "host": "127.0.0.1",
        "port": 9050,
        "username": "u",
        "password": "p",
    }));
    proxy_test_account(&conn, "acct", custom.clone());
    assert_eq!(stored_proxy(&conn, "acct"), custom);

    // An account written before proxy support has no key in its config JSON.
    conn.execute(
        "UPDATE accounts SET config = json_remove(config, '$.proxy') WHERE id = 'acct'",
        [],
    )
    .unwrap();
    assert_eq!(
        stored_proxy(&conn, "acct"),
        crate::proxy::ProxyChoice::Global
    );
}

#[test]
fn set_account_proxy_replaces_only_the_proxy_entry() {
    let conn = test_conn();
    proxy_test_account(&conn, "acct", crate::proxy::ProxyChoice::Global);
    set_account_proxy(&conn, "acct", &crate::proxy::ProxyChoice::Direct).unwrap();

    let (_, creds) = load_accounts(&conn)
        .unwrap()
        .into_iter()
        .find(|(account, _)| account == "acct")
        .unwrap();
    assert_eq!(creds.proxy, crate::proxy::ProxyChoice::Direct);
    // The rest of the connection config survives the targeted update.
    assert_eq!(creds.host, "imap.example.com");
    assert_eq!(creds.smtp_port, 587);
    assert!(creds.smtp_starttls);
}
