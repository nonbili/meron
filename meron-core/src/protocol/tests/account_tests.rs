use super::*;

#[test]
fn mobile_protocol_allocates_stable_format_message_identity() {
    let data_dir = unique_data_dir("allocate-identity");
    let value = invoke_mobile_protocol_json(
        r#"{"id":44,"method":"mail.allocateIdentity","params":{"account_id":"me@example.com","draft":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let id = value["result"]["message_id"].as_str().unwrap();
    assert!(id.starts_with("meron-draft-"), "{value}");
    assert!(id.ends_with("@example.com"), "{value}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_persists_password_account_metadata() {
    let data_dir = unique_data_dir("account");
    let unique = unique_test_suffix();
    let email = format!("me+{unique}@example.com");
    let request = format!(
        r#"{{"id":60,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Me","sender_name":"Sender","imap_host":"imap.example.com","imap_port":993,"smtp_host":"smtp.example.com","smtp_port":587,"username":"{email}","password":"secret","tls":true}}}}"#
    );
    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(added["id"], 60);
    assert_eq!(added["result"]["account"]["id"], email.as_str());
    assert_eq!(added["result"]["account"]["email"], email.as_str());
    assert_eq!(added["result"]["account"]["display_name"], "Me");
    assert_eq!(added["result"]["account"]["sender_name"], "Sender");
    assert_eq!(added["result"]["account"]["provider"], "custom");
    assert_eq!(added["result"]["account"]["auth_type"], "password");
    assert_eq!(added["result"]["account"]["imap_host"], "imap.example.com");
    assert_eq!(added["result"]["account"]["imap_port"], 993);
    assert_eq!(added["result"]["account"]["smtp_host"], "smtp.example.com");
    assert_eq!(added["result"]["account"]["smtp_port"], 587);
    assert_eq!(added["result"]["account"]["tls"], true);
    assert_eq!(added["result"]["account"]["needs_reconnect"], false);

    let listed = invoke_mobile_protocol_json(
        r#"{"id":61,"method":"account.list","params":{}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(listed["id"], 61);
    assert_eq!(listed["result"]["accounts"][0]["id"], email.as_str());
    assert_eq!(listed["result"]["accounts"][0]["needs_reconnect"], false);

    let _ = std::fs::remove_dir_all(data_dir);
}

/// Reconnecting an account re-runs `account.addPassword` with the setup form's
/// fields. The form has no proxy or certificate entry, so a per-account proxy —
/// and any certificate the user accepted — has to survive the save.
#[test]
fn mobile_protocol_reconnect_keeps_the_account_proxy_and_certificate_pins() {
    let data_dir = unique_data_dir("account-reconnect");
    let dir = data_dir.to_str().unwrap().to_string();
    let unique = unique_test_suffix();
    let email = format!("bridge+{unique}@example.com");
    let add = format!(
        r#"{{"id":70,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"127.0.0.1","imap_port":1143,"smtp_host":"127.0.0.1","smtp_port":1025,"username":"{email}","password":"secret","tls":false,"starttls":true,"smtp_tls":false,"smtp_starttls":true,"cert_pin":"AA11","smtp_cert_pin":"BB22","proxy":{{"mode":"socks5","host":"127.0.0.1","port":9050}}}}}}"#
    );
    let added = invoke_mobile_protocol_json(&add, Some(&dir));
    assert_eq!(added["result"]["account"]["cert_pin"], "aa11", "{added}");
    assert_eq!(added["result"]["account"]["proxy"]["mode"], "socks5");

    // The reconnect: same servers and a fresh password, nothing else.
    let reconnect = format!(
        r#"{{"id":71,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"127.0.0.1","imap_port":1143,"smtp_host":"127.0.0.1","smtp_port":1025,"username":"{email}","password":"new-secret","tls":false,"starttls":true,"smtp_tls":false,"smtp_starttls":true}}}}"#
    );
    let reconnected = invoke_mobile_protocol_json(&reconnect, Some(&dir));
    let account = &reconnected["result"]["account"];
    assert_eq!(account["proxy"]["mode"], "socks5", "{reconnected}");
    assert_eq!(account["proxy"]["port"], 9050, "{reconnected}");
    assert_eq!(account["cert_pin"], "aa11", "{reconnected}");
    assert_eq!(account["smtp_cert_pin"], "bb22", "{reconnected}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_honors_explicit_password_account_security_modes() {
    let data_dir = unique_data_dir("account-security");
    let unique = unique_test_suffix();
    let email = format!("self-hosted+{unique}@example.com");
    let request = format!(
        r#"{{"id":62,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"mail.example.com","imap_port":143,"smtp_host":"mail.example.com","smtp_port":2525,"username":"{email}","password":"secret","tls":false,"starttls":true,"smtp_tls":false,"smtp_starttls":false}}}}"#
    );

    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    let account = &added["result"]["account"];
    assert_eq!(account["tls"], false);
    assert_eq!(account["starttls"], true);
    assert_eq!(account["smtp_tls"], false);
    assert_eq!(account["smtp_starttls"], false);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_only_overrides_inference_with_unambiguous_security_modes() {
    let data_dir = unique_data_dir("account-partial-security");
    let unique = unique_test_suffix();
    let email = format!("partial-security+{unique}@example.com");
    let request = format!(
        r#"{{"id":63,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"mail.example.com","imap_port":143,"smtp_host":"mail.example.com","smtp_port":587,"username":"{email}","password":"secret","tls":true,"starttls":false,"smtp_tls":true}}}}"#
    );

    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    let account = &added["result"]["account"];
    assert_eq!(account["tls"], true);
    assert_eq!(account["starttls"], false);
    assert_eq!(account["smtp_tls"], true);
    assert_eq!(account["smtp_starttls"], false);

    let email = format!("partial-starttls+{unique}@example.com");
    let request = format!(
        r#"{{"id":64,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"mail.example.com","imap_port":993,"smtp_host":"mail.example.com","smtp_port":587,"username":"{email}","password":"secret","tls":true,"smtp_starttls":false}}}}"#
    );
    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    let account = &added["result"]["account"];
    assert_eq!(account["smtp_tls"], false);
    assert_eq!(account["smtp_starttls"], true);

    let email = format!("legacy-tls+{unique}@example.com");
    let request = format!(
        r#"{{"id":65,"method":"account.addPassword","params":{{"email":"{email}","imap_host":"mail.example.com","imap_port":143,"smtp_host":"mail.example.com","smtp_port":587,"username":"{email}","password":"secret","tls":true}}}}"#
    );
    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    let account = &added["result"]["account"];
    assert_eq!(account["tls"], false);
    assert_eq!(account["starttls"], true);
    assert_eq!(account["smtp_tls"], false);
    assert_eq!(account["smtp_starttls"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_updates_account_settings_and_aliases() {
    let data_dir = unique_data_dir("account-settings");
    let unique = unique_test_suffix();
    let email = format!("settings+{unique}@example.com");
    let data_dir_str = data_dir.to_str().unwrap();
    let add_request = format!(
        r#"{{"id":70,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Old","sender_name":"Old Sender","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
    );
    assert!(
        invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
            .get("error")
            .is_none()
    );

    let commands = [
        format!(
            r#"{{"id":71,"method":"account.setName","params":{{"id":"{email}","name":"Personal"}}}}"#
        ),
        format!(
            r#"{{"id":72,"method":"account.setSenderName","params":{{"id":"{email}","name":"Sender"}}}}"#
        ),
        format!(
            r#"{{"id":73,"method":"account.setAvatar","params":{{"id":"{email}","avatar_url":"https://example.com/avatar.png"}}}}"#
        ),
        format!(
            r#"{{"id":74,"method":"account.setChatWallpaper","params":{{"id":"{email}","wallpaper":{{"kind":"preset","presetId":"grid"}}}}}}"#
        ),
        format!(
            r#"{{"id":75,"method":"account.setImages","params":{{"id":"{email}","enabled":true}}}}"#
        ),
        format!(
            r#"{{"id":76,"method":"account.setConversationHtml","params":{{"id":"{email}","enabled":false}}}}"#
        ),
        format!(
            r#"{{"id":77,"method":"account.setUnified","params":{{"id":"{email}","enabled":false}}}}"#
        ),
        format!(
            r#"{{"id":78,"method":"account.setMuted","params":{{"id":"{email}","enabled":true}}}}"#
        ),
        format!(
            r#"{{"id":79,"method":"account.setPaused","params":{{"id":"{email}","enabled":true}}}}"#
        ),
        format!(
            r#"{{"id":80,"method":"account.setRSSSyncInterval","params":{{"id":"{email}","minutes":30}}}}"#
        ),
        format!(
            r##"{{"id":90,"method":"account.setColor","params":{{"id":"{email}","color":"#F59E0B"}}}}"##
        ),
        format!(
            r#"{{"id":81,"method":"account.setAliases","params":{{"id":"{email}","aliases":[{{"email":"alias@example.com","name":"Alias"}},{{"email":"alias@example.com","name":"Duplicate"}},{{"email":"   ","name":"Blank"}}]}}}}"#
        ),
    ];
    for command in commands {
        let value = invoke_mobile_protocol_json(&command, Some(data_dir_str));
        assert!(value.get("error").is_none(), "{value}");
    }

    let listed = invoke_mobile_protocol_json(
        r#"{"id":82,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    let account = &listed["result"]["accounts"][0];
    assert_eq!(account["display_name"], "Personal", "{listed}");
    assert_eq!(account["sender_name"], "Sender", "{listed}");
    assert_eq!(
        account["avatar_url"], "https://example.com/avatar.png",
        "{listed}"
    );
    assert_eq!(account["chat_wallpaper"]["kind"], "preset", "{listed}");
    assert_eq!(account["chat_wallpaper"]["presetId"], "grid", "{listed}");
    assert_eq!(account["color"], "#f59e0b", "{listed}");
    assert_eq!(account["load_remote_images"], true, "{listed}");
    assert_eq!(account["conversation_html"], false, "{listed}");
    assert_eq!(account["included_in_unified"], false, "{listed}");
    assert_eq!(account["muted"], true, "{listed}");
    assert_eq!(account["paused"], true, "{listed}");
    assert_eq!(account["aliases"].as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(
        account["aliases"][0]["email"], "alias@example.com",
        "{listed}"
    );
    assert_eq!(account["aliases"][0]["name"], "Alias", "{listed}");

    let remove = format!(r#"{{"id":83,"method":"account.remove","params":{{"id":"{email}"}}}}"#);
    assert!(
        invoke_mobile_protocol_json(&remove, Some(data_dir_str))
            .get("error")
            .is_none()
    );
    let empty = invoke_mobile_protocol_json(
        r#"{"id":84,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(empty["result"]["accounts"].as_array().unwrap().len(), 0);

    seed_rss_account(&data_dir, "rss-settings", "Feeds");
    let interval = invoke_mobile_protocol_json(
        r#"{"id":85,"method":"account.setRSSSyncInterval","params":{"id":"rss-settings","minutes":30}}"#,
        Some(data_dir_str),
    );
    assert!(interval.get("error").is_none(), "{interval}");
    let rss_listed = invoke_mobile_protocol_json(
        r#"{"id":84,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(
        rss_listed["result"]["accounts"][0]["rss_sync_interval_minutes"], 30,
        "{rss_listed}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_writes_account_media_files() {
    let data_dir = unique_data_dir("account-media");
    let unique = unique_test_suffix();
    let email = format!("media+{unique}@example.com");
    let data_dir_str = data_dir.to_str().unwrap();
    let add_request = format!(
        r#"{{"id":85,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Media","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
    );
    assert!(
        invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
            .get("error")
            .is_none()
    );

    let avatar_request = format!(
        r#"{{"id":86,"method":"account.writeAvatarFile","params":{{"id":"{email}","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}}}}"#
    );
    let avatar = invoke_mobile_protocol_json(&avatar_request, Some(data_dir_str));
    let avatar_url = avatar["result"]["url"].as_str().unwrap();
    assert!(avatar_url.starts_with("/media/avatars/"), "{avatar}");
    let avatar_path = data_dir.join(avatar_url.trim_start_matches('/'));
    assert!(avatar_path.exists(), "{avatar_url}");

    let wallpaper_request = format!(
        r#"{{"id":87,"method":"account.writeChatWallpaperFile","params":{{"id":"{email}","filename":"wallpaper.webp","mime":"image/webp","data":"d2FsbA=="}}}}"#
    );
    let wallpaper = invoke_mobile_protocol_json(&wallpaper_request, Some(data_dir_str));
    let wallpaper_url = wallpaper["result"]["url"].as_str().unwrap();
    assert!(
        wallpaper_url.starts_with("/media/wallpapers/"),
        "{wallpaper}"
    );
    let wallpaper_path = data_dir.join(wallpaper_url.trim_start_matches('/'));
    assert!(wallpaper_path.exists(), "{wallpaper_url}");

    let attachment_dir = data_dir
        .join("attachments")
        .join(crate::parse::sanitize_segment(&email));
    std::fs::create_dir_all(&attachment_dir).unwrap();
    std::fs::write(attachment_dir.join("note.txt"), b"cached").unwrap();

    let set_wallpaper = format!(
        r#"{{"id":88,"method":"account.setChatWallpaper","params":{{"id":"{email}","wallpaper":{{"kind":"custom","url":"{wallpaper_url}"}}}}}}"#
    );
    assert!(
        invoke_mobile_protocol_json(&set_wallpaper, Some(data_dir_str))
            .get("error")
            .is_none()
    );

    let listed = invoke_mobile_protocol_json(
        r#"{"id":89,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    assert_eq!(
        listed["result"]["accounts"][0]["chat_wallpaper"]["url"],
        wallpaper_url
    );

    let remove = format!(r#"{{"id":90,"method":"account.remove","params":{{"id":"{email}"}}}}"#);
    assert!(
        invoke_mobile_protocol_json(&remove, Some(data_dir_str))
            .get("error")
            .is_none()
    );
    assert!(!attachment_dir.exists());
    assert!(!avatar_path.parent().unwrap().exists());
    assert!(!wallpaper_path.parent().unwrap().exists());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reads_cached_attachment_file() {
    let data_dir = unique_data_dir("attachment-read");
    let data_dir_str = data_dir.to_str().unwrap();
    let attachment_dir = data_dir
        .join("attachments")
        .join("acc")
        .join("INBOX")
        .join("1");
    std::fs::create_dir_all(&attachment_dir).unwrap();
    std::fs::write(attachment_dir.join("note.txt"), b"Hi").unwrap();

    let read = invoke_mobile_protocol_json(
        r#"{"id":90,"method":"mail.attachmentRead","params":{"key":"acc/INBOX/1/note.txt"}}"#,
        Some(data_dir_str),
    );
    assert_eq!(read["id"], 90);
    assert_eq!(read["result"]["data"], STANDARD.encode(b"Hi"));

    let desktop_alias_read = invoke_mobile_protocol_json(
        r#"{"id":92,"method":"mail.readAttachment","params":{"key":"acc/INBOX/1/note.txt"}}"#,
        Some(data_dir_str),
    );
    assert_eq!(desktop_alias_read["id"], 92);
    assert_eq!(desktop_alias_read["result"]["data"], STANDARD.encode(b"Hi"));

    let rejected = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"mail.attachmentRead","params":{"key":"../meron.db"}}"#,
        Some(data_dir_str),
    );
    assert!(
        rejected["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid attachment key")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reports_and_clears_storage_cache() {
    let data_dir = unique_data_dir("storage-cache");
    let data_dir_str = data_dir.to_str().unwrap();
    seed_mobile_account(&data_dir, "me@example.com");
    let attachment_dir = data_dir
        .join("attachments")
        .join("acc")
        .join("INBOX")
        .join("1");
    std::fs::create_dir_all(&attachment_dir).unwrap();
    std::fs::write(attachment_dir.join("one.txt"), b"Hi").unwrap();
    std::fs::write(attachment_dir.join("two.txt"), b"There").unwrap();
    let secure_dir = data_dir.join("secure-media");
    std::fs::create_dir_all(&secure_dir).unwrap();
    std::fs::write(secure_dir.join("secret.txt"), b"Secret").unwrap();

    let usage =
        invoke_mobile_protocol_json(r#"{"id":92,"method":"storage.usage"}"#, Some(data_dir_str));
    assert_eq!(usage["id"], 92);
    assert_eq!(usage["result"]["cacheBytes"], 13);
    assert!(usage["result"]["dbBytes"].as_u64().unwrap() > 0, "{usage}");

    let cleared = invoke_mobile_protocol_json(
        r#"{"id":93,"method":"storage.clearCache"}"#,
        Some(data_dir_str),
    );
    assert_eq!(cleared["id"], 93);
    assert_eq!(cleared["result"]["cacheBytes"], 0);
    assert!(
        cleared["result"]["dbBytes"].as_u64().unwrap() > 0,
        "{cleared}"
    );
    assert!(data_dir.join("attachments").exists());
    assert!(!secure_dir.join("secret.txt").exists());
    assert!(data_dir.join("meron.db").exists());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reorders_accounts() {
    let data_dir = unique_data_dir("account-reorder");
    let data_dir_str = data_dir.to_str().unwrap();
    for email in [
        "first@example.com",
        "second@example.com",
        "third@example.com",
    ] {
        let add_request = format!(
            r#"{{"id":90,"method":"account.addPassword","params":{{"email":"{email}","display_name":"{email}","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
        );
        assert!(
            invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
                .get("error")
                .is_none()
        );
    }

    let reorder = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"account.reorder","params":{"accounts":["third@example.com","first@example.com","second@example.com"]}}"#,
        Some(data_dir_str),
    );
    assert_eq!(reorder["result"]["ok"], true, "{reorder}");

    let listed = invoke_mobile_protocol_json(
        r#"{"id":92,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    let accounts = listed["result"]["accounts"].as_array().unwrap();
    let ids: Vec<_> = accounts
        .iter()
        .map(|account| account["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![
            "third@example.com",
            "first@example.com",
            "second@example.com"
        ]
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_suggests_contacts_from_cached_messages() {
    let data_dir = unique_data_dir("contact-suggest");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 1,
                subject: "Hello".to_string(),
                from_name: "Aki".to_string(),
                from_addr: "aki@example.com".to_string(),
                date: 100,
                thread_key: "one".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "Again".to_string(),
                from_name: "Aki".to_string(),
                from_addr: "aki@example.com".to_string(),
                date: 200,
                thread_key: "two".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 3,
                subject: "Plan".to_string(),
                from_name: "Bea".to_string(),
                from_addr: "bea@example.com".to_string(),
                date: 300,
                thread_key: "three".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let suggestions = invoke_mobile_protocol_json(
        r#"{"id":93,"method":"mail.suggestContacts","params":{"account":"me@example.com","query":"","limit":2}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(
        suggestions["result"]["contacts"][0]["addr"], "aki@example.com",
        "{suggestions}"
    );
    assert_eq!(
        suggestions["result"]["contacts"][0]["name"], "Aki",
        "{suggestions}"
    );
    assert_eq!(
        suggestions["result"]["contacts"][1]["addr"], "bea@example.com",
        "{suggestions}"
    );

    let filtered = invoke_mobile_protocol_json(
        r#"{"id":94,"method":"contacts.suggest","params":{"account_id":"me@example.com","query":"bea","limit":8}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(filtered["result"]["contacts"].as_array().unwrap().len(), 1);
    assert_eq!(
        filtered["result"]["contacts"][0]["addr"], "bea@example.com",
        "{filtered}"
    );

    for (sort, first) in [("name", "aki@example.com"), ("recent", "bea@example.com")] {
        let directory = invoke_mobile_protocol_json(
            &json!({
                "id": 95, "method": "mail.suggestContacts",
                "params": {"query": "", "limit": 100, "sort": sort, "directory": true}
            })
            .to_string(),
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(
            directory["result"]["contacts"][0]["addr"], first,
            "{directory}"
        );
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_autodiscover_returns_known_provider() {
    let gmail = invoke_mobile_protocol_json(
        r#"{"id":96,"method":"account.autodiscover","params":{"email":"me@gmail.com"}}"#,
        None,
    );
    assert_eq!(gmail["id"], 96);
    assert_eq!(gmail["result"]["imap_host"], "imap.gmail.com");
    assert_eq!(gmail["result"]["smtp_host"], "smtp.gmail.com");
    assert_eq!(gmail["result"]["username"], "me@gmail.com");
    assert_eq!(gmail["result"]["source"], "known");
}

#[test]
fn engine_loads_password_account_with_secret_from_store() {
    let data_dir = unique_data_dir("engine-load");
    let data_dir_str = data_dir.to_str().unwrap();
    let email = format!("eng+{}@example.com", unique_test_suffix());
    add_password_account(data_dir_str, &email);

    // MobileHost opens the same store and applies the secret onto the creds.
    let engine = crate::engine::Engine::new(Box::new(MobileHost {
        data_dir: data_dir_str.to_string(),
    }))
    .expect("engine builds");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let creds = rt
        .block_on(engine.ensure_valid_creds(&email))
        .expect("account loaded with secret");
    assert_eq!(creds.user, email);
    assert_eq!(creds.password, "secret");
}

#[test]
fn engine_hydrates_account_added_after_construction() {
    let data_dir = unique_data_dir("engine-hydrate");
    let data_dir_str = data_dir.to_str().unwrap();
    // Build the Engine before any account exists, mirroring a long-lived
    // foreground Engine that outlives an out-of-band account add.
    let engine = crate::engine::Engine::new(Box::new(MobileHost {
        data_dir: data_dir_str.to_string(),
    }))
    .expect("engine builds");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let email = format!("late+{}@example.com", unique_test_suffix());
    assert!(
        rt.block_on(engine.ensure_valid_creds(&email)).is_err(),
        "unknown account should error before it exists"
    );

    add_password_account(data_dir_str, &email);

    let creds = rt
        .block_on(engine.ensure_valid_creds(&email))
        .expect("account hydrated from store on next use");
    assert_eq!(creds.password, "secret");
}

#[test]
fn engine_does_not_refresh_platform_managed_oauth_without_refresh_token() {
    let data_dir = unique_data_dir("engine-managed-oauth");
    let data_dir_str = data_dir.to_str().unwrap();
    let email = format!("managed+{}@gmail.com", unique_test_suffix());
    let add = format!(
        r#"{{"id":1,"method":"account.addOAuth","params":{{"email":"{email}","provider":"gmail","display_name":"Gmail","sender_name":"Sender","access_token":"access-only","token_expires_at":1}}}}"#
    );
    let added = invoke_mobile_protocol_json(&add, Some(data_dir_str));
    assert_eq!(
        added["result"]["account"]["needs_reconnect"], false,
        "{added}"
    );

    let engine = crate::engine::Engine::new(Box::new(MobileHost {
        data_dir: data_dir_str.to_string(),
    }))
    .expect("engine builds");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let creds = rt
        .block_on(engine.ensure_valid_creds(&email))
        .expect("access-token-only OAuth account remains usable for platform refresh");
    assert_eq!(creds.access_token.as_deref(), Some("access-only"));
    assert_eq!(creds.refresh_token.as_deref(), None);

    let _ = std::fs::remove_dir_all(data_dir);
}
