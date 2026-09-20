use super::*;
use std::collections::HashMap;
use std::sync::Mutex;

fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    store::run_migrations(&conn).unwrap();
    conn
}

fn no_secrets(_: &str) -> Secrets {
    Secrets::default()
}

/// `BackupData` is deliberately not `Debug` (it can hold a password), so
/// `unwrap_err` is unavailable — pull the message out by hand.
fn parse_err(text: &str, passphrase: Option<&str>) -> String {
    match parse(text, passphrase) {
        Ok(_) => panic!("expected the backup to be rejected"),
        Err(err) => err.to_string(),
    }
}

fn secrets_with(password: &str) -> Secrets {
    Secrets {
        password: password.to_string(),
        access_token: None,
        refresh_token: None,
    }
}

/// A `store_secrets` sink that records what it was handed, standing in for the
/// keychain (desktop) or the keyed DB (mobile).
#[derive(Default)]
struct SecretSink(Mutex<HashMap<String, String>>);

impl SecretSink {
    fn saver(&self) -> impl Fn(&Connection, &str, &Secrets) -> Result<()> + '_ {
        move |_conn, account, secrets| {
            self.0
                .lock()
                .unwrap()
                .insert(account.to_string(), secrets.password.clone());
            Ok(())
        }
    }
}

fn insert_mail_account(conn: &Connection, id: &str, config: Value, prefs: Value) {
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url, sender_name,
                              config, prefs, sort_order, created_at, updated_at)
         VALUES(?1, 'mail', 'custom', ?2, 'Work', '', 'Ada', ?3, ?4, 3, 100, 100)",
        params![id, id, config.to_string(), prefs.to_string()],
    )
    .unwrap();
}

fn insert_rss_account(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config, prefs, sort_order, created_at, updated_at)
         VALUES(?1, 'rss', 'rss', 'Feeds', '{}', '{}', 7, 100, 100)",
        params![id],
    )
    .unwrap();
}

fn insert_subscription(conn: &Connection, account: &str, id: &str, url: &str, title: &str) {
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, site_url, feed_title, enabled, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, 'https://site.example', ?4, 1, 100, 100)",
        params![id, account, url, title],
    )
    .unwrap();
}

fn sample_config() -> Value {
    json!({
        "host": "imap.example.com",
        "port": 993,
        "user": "ada@example.com",
        "tls": true,
        "smtp_host": "smtp.example.com",
        "smtp_port": 465,
        "auth_type": "password",
        "oauth_client_secret": "app-client-secret",
        "proxy": { "mode": "socks5", "host": "127.0.0.1", "port": 1080, "username": "u", "password": "proxy-pass" },
    })
}

// ---- Round trip -------------------------------------------------------------

#[test]
fn plaintext_backup_round_trips_accounts_settings_and_feeds() {
    let source = test_conn();
    insert_mail_account(
        &source,
        "ada@example.com",
        sample_config(),
        json!({ "muted": true, "aliases": [{ "email": "ada@alt.example", "name": "Ada" }] }),
    );
    insert_rss_account(&source, "rss-1");
    insert_subscription(
        &source,
        "rss-1",
        "feed-abc",
        "https://blog.example/feed.xml",
        "Blog",
    );
    store::setting_set(&source, "theme_id", &json!("nord")).unwrap();
    store::setting_set(&source, "font_scale", &json!(115)).unwrap();

    let text = export(&source, false, None, Host::default(), &no_secrets).unwrap();
    let data = parse(&text, None).unwrap();

    let target = test_conn();
    let sink = SecretSink::default();
    let summary = apply(&target, &data, &sink.saver()).unwrap();

    assert_eq!(summary.accounts, 2);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.feeds, 1);
    assert_eq!(summary.settings, 2);
    assert_eq!(summary.secrets, 0);

    // Accounts came back with their metadata and prefs intact.
    let restored = store::list_accounts(&target).unwrap();
    assert_eq!(restored.len(), 2);
    let mail = restored
        .iter()
        .find(|a| a["id"] == "ada@example.com")
        .unwrap();
    assert_eq!(mail["imap_host"], "imap.example.com");
    assert_eq!(mail["smtp_port"], 465);
    assert_eq!(mail["sender_name"], "Ada");
    assert_eq!(mail["muted"], true);
    assert_eq!(mail["aliases"][0]["email"], "ada@alt.example");

    // Settings came back parsed, not as JSON-in-a-string.
    let settings =
        store::settings_get(&target, &["theme_id".to_string(), "font_scale".to_string()]).unwrap();
    assert_eq!(settings["theme_id"], "nord");
    assert_eq!(settings["font_scale"], 115);

    // The feed is attached to the restored RSS account.
    let (account, url): (String, String) = target
        .query_row("SELECT account, url FROM subscriptions", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(account, "rss-1");
    assert_eq!(url, "https://blog.example/feed.xml");
}

#[test]
fn backup_omits_cached_mail() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    source
        .execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, body, date)
             VALUES('ada@example.com', 'INBOX', '1', 1, 'Secret subject', 'body text', 100)",
            [],
        )
        .unwrap();

    let text = export(&source, false, None, Host::default(), &no_secrets).unwrap();
    assert!(!text.contains("Secret subject"));
    assert!(!text.contains("body text"));

    let target = test_conn();
    let sink = SecretSink::default();
    apply(&target, &parse(&text, None).unwrap(), &sink.saver()).unwrap();
    let messages: i64 = target
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(messages, 0);
}

// ---- Secrets and redaction --------------------------------------------------

#[test]
fn plaintext_backup_redacts_proxy_and_oauth_client_secrets() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    store::setting_set(
        &source,
        crate::proxy::SETTING_KEY,
        &json!({ "mode": "http", "host": "proxy.example", "port": 3128, "username": "u", "password": "app-proxy-pass" }),
    )
    .unwrap();

    let text = export(&source, false, None, Host::default(), &no_secrets).unwrap();
    assert!(!text.contains("proxy-pass"));
    assert!(!text.contains("app-proxy-pass"));
    assert!(!text.contains("app-client-secret"));

    // Everything else about the proxy survives, so only the password is retyped.
    let data = parse(&text, None).unwrap();
    assert_eq!(
        data.settings[crate::proxy::SETTING_KEY]["host"],
        "proxy.example"
    );
    assert_eq!(data.settings[crate::proxy::SETTING_KEY]["password"], "");
    assert_eq!(data.accounts[0].config["proxy"]["username"], "u");
    assert_eq!(data.accounts[0].config["proxy"]["password"], "");
    assert!(data.accounts[0].config.get("oauth_client_secret").is_none());
    assert_eq!(data.accounts[0].config["host"], "imap.example.com");
}

#[test]
fn the_envelope_records_which_host_wrote_the_file() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));

    // Desktop and mobile version separately, so the same number can come from
    // either: the platform is what makes it readable.
    let host = Host {
        platform: "android",
        app_version: "0.2.4",
    };
    let text = export(&source, false, None, host, &no_secrets).unwrap();
    assert!(text.contains("\"platform\": \"android\""));
    assert!(text.contains("\"app_version\": \"0.2.4\""));

    // A host with nothing to offer leaves the fields out rather than stamping
    // values that mean nothing.
    let host = Host {
        platform: "  ",
        app_version: "",
    };
    let text = export(&source, false, None, host, &no_secrets).unwrap();
    assert!(!text.contains("app_version"));
    assert!(!text.contains("platform"));
}

#[test]
fn exporting_secrets_without_a_passphrase_is_refused() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));

    let err = export(&source, true, None, Host::default(), &|_| {
        secrets_with("hunter2")
    })
    .unwrap_err();
    assert!(err.to_string().contains("passphrase is required"));

    // An empty passphrase is treated as no passphrase, not as a real one.
    let err = export(&source, true, Some(""), Host::default(), &|_| {
        secrets_with("hunter2")
    })
    .unwrap_err();
    assert!(err.to_string().contains("passphrase is required"));
}

#[test]
fn encrypted_backup_round_trips_secrets_and_hides_them_on_disk() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));

    let text = export(
        &source,
        true,
        Some("correct horse"),
        Host::default(),
        &|_| secrets_with("hunter2"),
    )
    .unwrap();

    // Nothing sensitive is readable in the file, but the envelope still is.
    assert!(!text.contains("hunter2"));
    assert!(!text.contains("imap.example.com"));
    assert!(!text.contains("proxy-pass"));
    assert!(text.contains("\"meron_backup\""));
    assert!(text.contains("\"encrypted\": true"));
    assert!(text.contains("\"has_secrets\": true"));

    let data = parse(&text, Some("correct horse")).unwrap();
    // With secrets included, config keeps the fields a plaintext export strips.
    assert_eq!(data.accounts[0].config["proxy"]["password"], "proxy-pass");
    assert_eq!(
        data.accounts[0].config["oauth_client_secret"],
        "app-client-secret"
    );

    let target = test_conn();
    let sink = SecretSink::default();
    let summary = apply(&target, &data, &sink.saver()).unwrap();
    assert_eq!(summary.accounts, 1);
    assert_eq!(summary.secrets, 1);
    assert_eq!(
        sink.0.lock().unwrap().get("ada@example.com").unwrap(),
        "hunter2"
    );
}

#[test]
fn accounts_with_no_stored_secret_carry_none() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));

    let text = export(&source, true, Some("pw"), Host::default(), &no_secrets).unwrap();
    let data = parse(&text, Some("pw")).unwrap();
    assert!(data.accounts[0].secrets.is_none());

    let target = test_conn();
    let sink = SecretSink::default();
    let summary = apply(&target, &data, &sink.saver()).unwrap();
    assert_eq!(summary.secrets, 0);
    assert!(sink.0.lock().unwrap().is_empty());
}

// ---- Passphrase failures ----------------------------------------------------

#[test]
fn encrypted_backup_without_a_passphrase_asks_for_one() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    let text = export(&source, true, Some("pw"), Host::default(), &|_| {
        secrets_with("hunter2")
    })
    .unwrap();

    let err = parse_err(&text, None);
    assert!(needs_passphrase(&err), "{err}");

    // A blank passphrase is the same as none, so the UI re-prompts.
    let err = parse_err(&text, Some(""));
    assert!(needs_passphrase(&err), "{err}");
}

#[test]
fn wrong_passphrase_is_reported_as_such() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    let text = export(&source, true, Some("pw"), Host::default(), &|_| {
        secrets_with("hunter2")
    })
    .unwrap();

    let err = parse_err(&text, Some("not-pw"));
    assert!(err.contains("wrong passphrase"), "{err}");
    // "Wrong passphrase" must not send the UI back to the prompt-for-one path.
    assert!(!needs_passphrase(&err));
}

#[test]
fn tampering_with_the_ciphertext_fails_authentication() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    let text = export(&source, true, Some("pw"), Host::default(), &|_| {
        secrets_with("hunter2")
    })
    .unwrap();

    let mut envelope: Value = serde_json::from_str(&text).unwrap();
    let ciphertext = envelope["ciphertext"].as_str().unwrap().to_string();
    let mut raw = STANDARD.decode(&ciphertext).unwrap();
    raw[0] ^= 0x01;
    envelope["ciphertext"] = json!(STANDARD.encode(&raw));

    let err = parse_err(&envelope.to_string(), Some("pw"));
    assert!(err.contains("wrong passphrase"), "{err}");
}

#[test]
fn a_passphrase_on_a_plaintext_backup_is_ignored() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    let text = export(&source, false, None, Host::default(), &no_secrets).unwrap();

    let data = parse(&text, Some("unnecessary")).unwrap();
    assert_eq!(data.accounts.len(), 1);
}

// ---- Malformed input --------------------------------------------------------

#[test]
fn non_backup_files_are_rejected() {
    for text in ["", "not json", "{}", r#"{"hello":"world"}"#, "[]"] {
        let err = parse_err(text, None);
        assert!(
            err.contains("not a Meron backup file") || err.contains("no data"),
            "{text:?} gave {err}"
        );
    }
}

#[test]
fn a_newer_format_version_is_refused() {
    let text = json!({ "meron_backup": FORMAT_VERSION + 1, "encrypted": false, "data": {} });
    let err = parse_err(&text.to_string(), None);
    assert!(err.contains("newer version of Meron"), "{err}");
}

/// The iteration count is read from the envelope before the passphrase can be
/// checked, so an absurd one must be refused rather than worked through.
#[test]
fn an_absurd_iteration_count_is_refused_instead_of_performed() {
    for iterations in [
        json!(MAX_KDF_ITERATIONS + 1),
        json!(u64::MAX),
        json!(4_000_000_000u64),
        // 2^32 + our own count: a truncating cast would read this as normal.
        json!(4_294_967_296u64 + KDF_ITERATIONS as u64),
        json!(0),
    ] {
        let text = json!({
            "meron_backup": FORMAT_VERSION,
            "encrypted": true,
            "cipher": "aes-256-gcm",
            "kdf": { "salt": STANDARD.encode([0u8; SALT_LEN]), "iterations": iterations },
            "nonce": STANDARD.encode([0u8; NONCE_LEN]),
            "ciphertext": STANDARD.encode([0u8; 32]),
        });
        let err = parse_err(&text.to_string(), Some("pw"));
        assert!(
            err.contains("unreasonable amount of work"),
            "{iterations}: {err}"
        );
    }
}

#[test]
fn a_malformed_salt_is_refused() {
    for salt in [vec![0u8; 0], vec![0u8; 8], vec![0u8; 64]] {
        let text = json!({
            "meron_backup": FORMAT_VERSION,
            "encrypted": true,
            "cipher": "aes-256-gcm",
            "kdf": { "salt": STANDARD.encode(&salt), "iterations": KDF_ITERATIONS },
            "nonce": STANDARD.encode([0u8; NONCE_LEN]),
            "ciphertext": STANDARD.encode([0u8; 32]),
        });
        let err = parse_err(&text.to_string(), Some("pw"));
        assert!(
            err.contains("malformed `kdf.salt`"),
            "{} bytes: {err}",
            salt.len()
        );
    }
}

/// The count we actually write has to survive the bound that rejects the rest.
#[test]
fn our_own_iteration_count_still_opens() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    let text = export(&source, true, Some("pw"), Host::default(), &|_| {
        secrets_with("hunter2")
    })
    .unwrap();

    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap()["kdf"]["iterations"],
        KDF_ITERATIONS
    );
    let data = parse(&text, Some("pw")).unwrap();
    assert_eq!(data.accounts.len(), 1);
}

#[test]
fn an_encrypted_backup_with_an_unknown_cipher_is_refused() {
    let text = json!({
        "meron_backup": FORMAT_VERSION,
        "encrypted": true,
        "cipher": "rot13",
        "kdf": { "salt": STANDARD.encode([0u8; SALT_LEN]) },
        "nonce": STANDARD.encode([0u8; NONCE_LEN]),
        "ciphertext": STANDARD.encode([0u8; 32]),
    });
    let err = parse_err(&text.to_string(), Some("pw"));
    assert!(err.contains("unsupported backup cipher"), "{err}");
}

// ---- Apply semantics --------------------------------------------------------

#[test]
fn existing_accounts_are_skipped_not_overwritten() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    let data = parse(
        &export(&source, false, None, Host::default(), &no_secrets).unwrap(),
        None,
    )
    .unwrap();

    // The target already has this account, pointed at a different server.
    let target = test_conn();
    insert_mail_account(
        &target,
        "ada@example.com",
        json!({ "host": "live.example.com", "port": 993, "user": "ada" }),
        json!({}),
    );

    let sink = SecretSink::default();
    let summary = apply(&target, &data, &sink.saver()).unwrap();
    assert_eq!(summary.accounts, 0);
    assert_eq!(summary.skipped, 1);

    let host: String = target
        .query_row(
            "SELECT json_extract(config, '$.host') FROM accounts WHERE id = 'ada@example.com'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(host, "live.example.com", "the live account was overwritten");
}

#[test]
fn settings_overwrite_on_restore() {
    let source = test_conn();
    store::setting_set(&source, "theme_id", &json!("nord")).unwrap();
    let data = parse(
        &export(&source, false, None, Host::default(), &no_secrets).unwrap(),
        None,
    )
    .unwrap();

    let target = test_conn();
    store::setting_set(&target, "theme_id", &json!("solarized")).unwrap();
    store::setting_set(&target, "font_scale", &json!(150)).unwrap();

    let sink = SecretSink::default();
    apply(&target, &data, &sink.saver()).unwrap();

    let settings =
        store::settings_get(&target, &["theme_id".to_string(), "font_scale".to_string()]).unwrap();
    assert_eq!(settings["theme_id"], "nord");
    // A key the backup doesn't mention is left alone rather than reset.
    assert_eq!(settings["font_scale"], 150);
}

#[test]
fn a_feed_already_subscribed_is_not_duplicated() {
    let source = test_conn();
    insert_rss_account(&source, "rss-1");
    insert_subscription(
        &source,
        "rss-1",
        "feed-abc",
        "https://blog.example/feed.xml",
        "Blog",
    );
    let data = parse(
        &export(&source, false, None, Host::default(), &no_secrets).unwrap(),
        None,
    )
    .unwrap();

    let target = test_conn();
    insert_rss_account(&target, "rss-other");
    insert_subscription(
        &target,
        "rss-other",
        "feed-existing",
        "https://blog.example/feed.xml",
        "Blog",
    );

    let sink = SecretSink::default();
    let summary = apply(&target, &data, &sink.saver()).unwrap();
    assert_eq!(summary.accounts, 1);
    assert_eq!(summary.feeds, 0, "the duplicate URL was inserted anyway");

    let count: i64 = target
        .query_row("SELECT COUNT(*) FROM subscriptions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn a_failed_import_leaves_the_store_untouched() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    insert_mail_account(&source, "bob@example.com", sample_config(), json!({}));
    let data = parse(
        &export(&source, true, Some("pw"), Host::default(), &|_| {
            secrets_with("s")
        })
        .unwrap(),
        Some("pw"),
    )
    .unwrap();

    // Secret storage failing mid-run (a locked keychain) must not leave half
    // the accounts behind.
    let target = test_conn();
    let summary = apply(&target, &data, &|_, account, _| {
        if account == "bob@example.com" {
            Err(anyhow!("keychain is locked"))
        } else {
            Ok(())
        }
    });
    assert!(summary.is_err());

    let count: i64 = target
        .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn accounts_with_a_blank_id_are_dropped() {
    let data = BackupData {
        accounts: vec![
            BackupAccount {
                id: "   ".to_string(),
                ..Default::default()
            },
            BackupAccount {
                id: "ada@example.com".to_string(),
                engine: "mail".to_string(),
                ..Default::default()
            },
        ],
        settings: Map::new(),
        ..Default::default()
    };

    let target = test_conn();
    let sink = SecretSink::default();
    let summary = apply(&target, &data, &sink.saver()).unwrap();
    assert_eq!(summary.accounts, 1);
    assert_eq!(summary.skipped, 0);
}

#[test]
fn empty_engine_and_provider_fall_back_to_mail_defaults() {
    let data = BackupData {
        accounts: vec![BackupAccount {
            id: "ada@example.com".to_string(),
            ..Default::default()
        }],
        settings: Map::new(),
        ..Default::default()
    };

    let target = test_conn();
    let sink = SecretSink::default();
    apply(&target, &data, &sink.saver()).unwrap();

    let (engine, provider): (String, String) = target
        .query_row(
            "SELECT engine, provider FROM accounts WHERE id = 'ada@example.com'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(engine, "mail");
    assert_eq!(provider, "custom");
}

#[test]
fn unknown_fields_from_a_newer_build_survive_a_round_trip() {
    // `config` and `prefs` are free-form, so a field this build has never heard
    // of must still come back rather than being silently dropped.
    let source = test_conn();
    insert_mail_account(
        &source,
        "ada@example.com",
        json!({ "host": "imap.example.com", "future_field": "keep me" }),
        json!({ "future_pref": 42 }),
    );

    let data = parse(
        &export(&source, false, None, Host::default(), &no_secrets).unwrap(),
        None,
    )
    .unwrap();
    let target = test_conn();
    let sink = SecretSink::default();
    apply(&target, &data, &sink.saver()).unwrap();

    let (config, prefs): (String, String) = target
        .query_row(
            "SELECT config, prefs FROM accounts WHERE id = 'ada@example.com'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&config).unwrap()["future_field"],
        "keep me"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&prefs).unwrap()["future_pref"],
        42
    );
}

// ---- Pre-release `platform` map ---------------------------------------------

/// A development build carried mobile preferences in their own `platform` map
/// before they became `settings` rows. Serde would ignore the field, restoring
/// such a file with every mobile preference silently dropped.
#[test]
fn a_legacy_platform_map_is_folded_into_settings() {
    let text = json!({
        "meron_backup": FORMAT_VERSION,
        "encrypted": false,
        "data": {
            "accounts": [],
            "settings": { "theme_id": "nord" },
            "platform": {
                "app:appearance_mode_v1": "Indigo",
                "app:message_font_scale_v1": 115,
                "app:hidden_navigation_accounts_v1": ["a@example.com"],
                "kanban:kanban_boards_v1": "[{\"id\":\"b1\"}]",
            },
        },
    });

    let data = parse(&text.to_string(), None).unwrap();

    assert_eq!(data.settings["mobile.app.appearance_mode_v1"], "Indigo");
    assert_eq!(data.settings["mobile.app.message_font_scale_v1"], 115);
    assert_eq!(
        data.settings["mobile.app.hidden_navigation_accounts_v1"][0],
        "a@example.com"
    );
    assert_eq!(
        data.settings["mobile.kanban.kanban_boards_v1"],
        "[{\"id\":\"b1\"}]"
    );
    // Existing rows are untouched.
    assert_eq!(data.settings["theme_id"], "nord");
}

#[test]
fn a_legacy_platform_map_never_overwrites_a_real_settings_row() {
    let text = json!({
        "meron_backup": FORMAT_VERSION,
        "encrypted": false,
        "data": {
            "settings": { "mobile.app.appearance_mode_v1": "Rose" },
            "platform": { "app:appearance_mode_v1": "Indigo" },
        },
    });

    let data = parse(&text.to_string(), None).unwrap();

    // The row is the newer home for the same setting, so it wins.
    assert_eq!(data.settings["mobile.app.appearance_mode_v1"], "Rose");
}

#[test]
fn a_legacy_platform_map_survives_encryption() {
    let source = test_conn();
    let text = export(&source, true, Some("pw"), Host::default(), &no_secrets).unwrap();

    // Rebuild the ciphertext around a payload carrying the old shape.
    let mut envelope: Value = serde_json::from_str(&text).unwrap();
    let payload = json!({
        "accounts": [],
        "settings": {},
        "platform": { "app:app_language_v1": "ja" },
    });
    let sealed = encrypt(payload.to_string().as_bytes(), "pw").unwrap();
    envelope["kdf"] = sealed.kdf_json();
    envelope["nonce"] = json!(STANDARD.encode(sealed.nonce));
    envelope["ciphertext"] = json!(STANDARD.encode(&sealed.ciphertext));

    let data = parse(&envelope.to_string(), Some("pw")).unwrap();
    assert_eq!(data.settings["mobile.app.app_language_v1"], "ja");
}

#[test]
fn a_backup_without_a_platform_map_is_unaffected() {
    let source = test_conn();
    insert_mail_account(&source, "ada@example.com", sample_config(), json!({}));
    store::setting_set(&source, "theme_id", &json!("nord")).unwrap();

    let data = parse(
        &export(&source, false, None, Host::default(), &no_secrets).unwrap(),
        None,
    )
    .unwrap();

    assert_eq!(data.settings["theme_id"], "nord");
    assert_eq!(data.settings.len(), 1);
}

/// Tasks have no server copy, so a backup is the only thing standing between a
/// reinstall and losing them. Everything the user typed has to survive the
/// round trip, including the link back to a mail thread.
#[test]
fn backup_round_trips_task_lists() {
    let source = test_conn();
    let list = crate::tasks::ensure_default_list(&source).unwrap();
    let groceries = store::insert_task_list(&source, "list-groceries", "Groceries").unwrap();
    crate::tasks::create_task(
        &source,
        &list,
        "Reply to the landlord",
        "about the boiler",
        1_700_000_000,
        "acct1",
        "acct1#thread#4",
        "<boiler@example.com>",
    )
    .unwrap();
    let done =
        crate::tasks::create_task(&source, &groceries.id, "Milk", "", 0, "", "", "").unwrap();
    crate::tasks::set_done(&source, done["task"]["id"].as_str().unwrap(), true).unwrap();

    let data = collect(&source, false, &no_secrets).unwrap();
    assert_eq!(data.task_lists.len(), 2);

    let target = test_conn();
    let summary = apply(&target, &data, &|_, _, _| Ok(())).unwrap();
    assert_eq!(summary.tasks, 2);

    let restored_lists = store::task_lists(&target).unwrap();
    let titles: Vec<&str> = restored_lists.iter().map(|l| l.title.as_str()).collect();
    assert!(titles.contains(&"Groceries"), "{titles:?}");

    let my_tasks = restored_lists
        .iter()
        .find(|l| l.title == crate::tasks::DEFAULT_LIST_TITLE)
        .unwrap();
    let tasks = store::tasks_for_list(&target, &my_tasks.id, true).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "Reply to the landlord");
    assert_eq!(tasks[0].notes, "about the boiler");
    assert_eq!(tasks[0].due_at, 1_700_000_000);
    assert_eq!(tasks[0].thread_id, "acct1#thread#4");
    assert_eq!(tasks[0].message_id, "<boiler@example.com>");

    let groceries_id = &restored_lists
        .iter()
        .find(|l| l.title == "Groceries")
        .unwrap()
        .id;
    let milk = store::tasks_for_list(&target, groceries_id, true).unwrap();
    assert!(
        milk[0].completed_at > 0,
        "completion survives the round trip"
    );
}

/// Importing the same file twice is something people do — twice through the
/// dialog, or onto a device that already restored it. It must not double the
/// list or its contents.
#[test]
fn importing_a_backup_twice_does_not_duplicate_tasks() {
    let source = test_conn();
    let list = crate::tasks::ensure_default_list(&source).unwrap();
    crate::tasks::create_task(&source, &list, "Book the flight", "", 0, "", "", "").unwrap();
    let data = collect(&source, false, &no_secrets).unwrap();

    let target = test_conn();
    // The destination has already opened Tasks once, so the default list exists
    // and is empty — the case where matching lists by id would lose everything.
    crate::tasks::ensure_default_list(&target).unwrap();

    assert_eq!(apply(&target, &data, &|_, _, _| Ok(())).unwrap().tasks, 1);
    assert_eq!(
        apply(&target, &data, &|_, _, _| Ok(())).unwrap().tasks,
        0,
        "the second import adds nothing"
    );

    let lists = store::task_lists(&target).unwrap();
    assert_eq!(lists.len(), 1, "merged into the existing list");
    assert_eq!(
        store::tasks_for_list(&target, &lists[0].id, true)
            .unwrap()
            .len(),
        1
    );
}

/// Files written before Tasks existed carry no `task_lists` key at all.
#[test]
fn a_backup_without_tasks_still_imports() {
    let conn = test_conn();
    let data: BackupData =
        serde_json::from_str(r#"{"accounts":[],"settings":{"theme_id":"meron-light"}}"#).unwrap();
    let summary = apply(&conn, &data, &|_, _, _| Ok(())).unwrap();
    assert_eq!(summary.tasks, 0);
    assert_eq!(summary.settings, 1);
    assert!(store::task_lists(&conn).unwrap().is_empty());
}
