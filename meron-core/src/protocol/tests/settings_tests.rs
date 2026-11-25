use super::*;

#[test]
fn mobile_protocol_persists_app_and_account_proxies() {
    let _guard = crate::proxy::TEST_GLOBAL_LOCK
        .write()
        .unwrap_or_else(|err| err.into_inner());
    let data_dir = unique_data_dir("proxy");
    let unique = unique_test_suffix();
    let email = format!("proxy+{unique}@example.com");
    let data_dir_str = data_dir.to_str().unwrap();
    let add_request = format!(
        r#"{{"id":90,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
    );
    assert!(
        invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
            .get("error")
            .is_none()
    );

    // A fresh install has no proxy, and a new account follows the app-wide one.
    let empty = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"app.proxyGet","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(empty["result"]["proxy"], serde_json::Value::Null);
    let listed = invoke_mobile_protocol_json(
        r#"{"id":92,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(listed["result"]["accounts"][0]["proxy"]["mode"], "global");

    let set = invoke_mobile_protocol_json(
        r#"{"id":93,"method":"app.proxySet","params":{"proxy":{"mode":"socks5","host":"127.0.0.1","port":9050,"username":"u","password":"p"}}}"#,
        Some(data_dir_str),
    );
    assert_eq!(set["result"]["ok"], true);
    let stored = invoke_mobile_protocol_json(
        r#"{"id":94,"method":"app.proxyGet","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(stored["result"]["proxy"]["mode"], "socks5");
    assert_eq!(stored["result"]["proxy"]["port"], 9050);
    // Saving publishes it, so connections made right after use it without a restart.
    assert_eq!(
        crate::proxy::global().map(|cfg| cfg.host),
        Some("127.0.0.1".to_string())
    );

    let account_set = format!(
        r#"{{"id":95,"method":"account.setProxy","params":{{"id":"{email}","proxy":{{"mode":"direct"}}}}}}"#
    );
    assert_eq!(
        invoke_mobile_protocol_json(&account_set, Some(data_dir_str))["result"]["ok"],
        true
    );
    let listed = invoke_mobile_protocol_json(
        r#"{"id":96,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(listed["result"]["accounts"][0]["proxy"]["mode"], "direct");

    crate::proxy::set_global(None);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_round_trips_a_plaintext_backup() {
    let source_dir = unique_data_dir("backup-source");
    let unique = unique_test_suffix();
    let email = format!("backup+{unique}@example.com");
    let source = source_dir.to_str().unwrap();
    let add = format!(
        r#"{{"id":80,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Work","sender_name":"Ada","imap_host":"imap.example.com","imap_port":993,"smtp_host":"smtp.example.com","smtp_port":465,"username":"{email}","password":"pa55w0rd-plain","tls":true}}}}"#
    );
    assert!(
        invoke_mobile_protocol_json(&add, Some(source))
            .get("error")
            .is_none()
    );

    let exported = invoke_mobile_protocol_json(
        r#"{"id":81,"method":"backup.export","params":{}}"#,
        Some(source),
    );
    assert!(exported.get("error").is_none(), "{exported}");
    let text = exported["result"]["backup"].as_str().unwrap().to_string();
    // Without secrets the password never reaches the file.
    assert!(!text.contains("pa55w0rd-plain"), "{text}");

    // Restore into a second, empty data dir.
    let target_dir = unique_data_dir("backup-target");
    let target = target_dir.to_str().unwrap();
    let import = json!({
        "id": 82,
        "method": "backup.import",
        "params": { "backup": text },
    });
    let imported = invoke_mobile_protocol_json(&import.to_string(), Some(target));
    assert!(imported.get("error").is_none(), "{imported}");
    assert_eq!(imported["result"]["accounts"], 1);
    assert_eq!(imported["result"]["skipped"], 0);
    assert_eq!(imported["result"]["secrets"], 0);

    let listed = invoke_mobile_protocol_json(
        r#"{"id":83,"method":"account.list","params":{}}"#,
        Some(target),
    );
    assert_eq!(listed["result"]["accounts"][0]["id"], email.as_str());
    assert_eq!(
        listed["result"]["accounts"][0]["imap_host"],
        "imap.example.com"
    );
    // No secret came across, so the restored account has to be reconnected.
    assert_eq!(listed["result"]["accounts"][0]["needs_reconnect"], true);

    let _ = std::fs::remove_dir_all(source_dir);
    let _ = std::fs::remove_dir_all(target_dir);
}

/// Mobile settings are rows in the core `settings` table, the same as desktop;
/// the platform store is only a cache in front of them.
#[test]
fn mobile_protocol_reads_back_settings_it_wrote() {
    let dir = unique_data_dir("mobile-prefs");
    let data_dir = dir.to_str().unwrap();

    for (key, value) in [
        (r#""mobile.app.appearance_mode_v1""#, r#""Indigo""#),
        (r#""mobile.app.message_font_scale_v1""#, "115"),
        (r#""mobile.app.background_sync_enabled_v1""#, "false"),
        (
            r#""mobile.app.hidden_navigation_accounts_v1""#,
            r#"["a@example.com"]"#,
        ),
    ] {
        let request = format!(
            r#"{{"id":1,"method":"app.prefsSet","params":{{"key":{key},"value":{value}}}}}"#
        );
        let value = invoke_mobile_protocol_json(&request, Some(data_dir));
        assert!(value.get("error").is_none(), "{value}");
    }

    let get = json!({
        "id": 2,
        "method": "app.prefsGet",
        "params": {
            "keys": [
                "mobile.app.appearance_mode_v1",
                "mobile.app.message_font_scale_v1",
                "mobile.app.background_sync_enabled_v1",
                "mobile.app.hidden_navigation_accounts_v1",
                "mobile.app.never_set_v1",
            ],
        },
    });
    let value = invoke_mobile_protocol_json(&get.to_string(), Some(data_dir));
    assert!(value.get("error").is_none(), "{value}");
    let prefs = &value["result"]["prefs"];
    // Types survive the round trip rather than all becoming strings.
    assert_eq!(prefs["mobile.app.appearance_mode_v1"], "Indigo");
    assert_eq!(prefs["mobile.app.message_font_scale_v1"], 115);
    assert_eq!(prefs["mobile.app.background_sync_enabled_v1"], false);
    assert_eq!(
        prefs["mobile.app.hidden_navigation_accounts_v1"][0],
        "a@example.com"
    );
    // A key never written is absent, not defaulted — that is what lets the host
    // keep its own cached value instead of being reset to a default.
    assert!(prefs.get("mobile.app.never_set_v1").is_none(), "{prefs}");

    let _ = std::fs::remove_dir_all(dir);
}

/// Settings restored from a backup land in the same table the host hydrates
/// from, so a restore reaches mobile preferences with no extra channel.
#[test]
fn mobile_settings_restored_from_a_backup_are_readable_as_prefs() {
    let source_dir = unique_data_dir("mobile-prefs-backup-source");
    let source = source_dir.to_str().unwrap();
    let set = json!({
        "id": 1,
        "method": "app.prefsSet",
        "params": { "key": "mobile.app.app_language_v1", "value": "ja" },
    });
    assert!(
        invoke_mobile_protocol_json(&set.to_string(), Some(source))
            .get("error")
            .is_none()
    );

    let exported = invoke_mobile_protocol_json(
        r#"{"id":2,"method":"backup.export","params":{}}"#,
        Some(source),
    );
    let text = exported["result"]["backup"].as_str().unwrap().to_string();

    let target_dir = unique_data_dir("mobile-prefs-backup-target");
    let target = target_dir.to_str().unwrap();
    let import = json!({ "id": 3, "method": "backup.import", "params": { "backup": text } });
    let imported = invoke_mobile_protocol_json(&import.to_string(), Some(target));
    assert!(imported.get("error").is_none(), "{imported}");

    let get = json!({
        "id": 4,
        "method": "app.prefsGet",
        "params": { "keys": ["mobile.app.app_language_v1"] },
    });
    let value = invoke_mobile_protocol_json(&get.to_string(), Some(target));
    assert_eq!(value["result"]["prefs"]["mobile.app.app_language_v1"], "ja");

    let _ = std::fs::remove_dir_all(source_dir);
    let _ = std::fs::remove_dir_all(target_dir);
}

#[test]
fn mobile_protocol_round_trips_an_encrypted_backup_with_secrets() {
    let source_dir = unique_data_dir("backup-enc-source");
    let unique = unique_test_suffix();
    let email = format!("enc+{unique}@example.com");
    let source = source_dir.to_str().unwrap();
    let add = format!(
        r#"{{"id":84,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Work","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"hunter2"}}}}"#
    );
    assert!(
        invoke_mobile_protocol_json(&add, Some(source))
            .get("error")
            .is_none()
    );

    let export = json!({
        "id": 85,
        "method": "backup.export",
        "params": { "include_secrets": true, "passphrase": "correct horse" },
    });
    let exported = invoke_mobile_protocol_json(&export.to_string(), Some(source));
    assert!(exported.get("error").is_none(), "{exported}");
    let text = exported["result"]["backup"].as_str().unwrap().to_string();
    assert!(!text.contains("hunter2"), "{text}");

    let target_dir = unique_data_dir("backup-enc-target");
    let target = target_dir.to_str().unwrap();

    // No passphrase: a prompt request, not a failure.
    let bare = json!({ "id": 86, "method": "backup.import", "params": { "backup": text } });
    let value = invoke_mobile_protocol_json(&bare.to_string(), Some(target));
    assert!(value.get("error").is_none(), "{value}");
    assert_eq!(value["result"]["needs_passphrase"], true);

    // Wrong passphrase: a real error.
    let wrong = json!({
        "id": 87,
        "method": "backup.import",
        "params": { "backup": text, "passphrase": "nope" },
    });
    let value = invoke_mobile_protocol_json(&wrong.to_string(), Some(target));
    let message = value["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("wrong passphrase"), "{value}");

    // Right passphrase: the secret comes back, so the account is usable.
    let right = json!({
        "id": 88,
        "method": "backup.import",
        "params": { "backup": text, "passphrase": "correct horse" },
    });
    let value = invoke_mobile_protocol_json(&right.to_string(), Some(target));
    assert!(value.get("error").is_none(), "{value}");
    assert_eq!(value["result"]["accounts"], 1);
    assert_eq!(value["result"]["secrets"], 1);

    let listed = invoke_mobile_protocol_json(
        r#"{"id":89,"method":"account.list","params":{}}"#,
        Some(target),
    );
    assert_eq!(listed["result"]["accounts"][0]["needs_reconnect"], false);

    let _ = std::fs::remove_dir_all(source_dir);
    let _ = std::fs::remove_dir_all(target_dir);
}
