use super::*;
use crate::imap::{Folder, MessageHeader, Recipient};
use crate::store::{RssItemExtra, RssMedia};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn request_defaults_missing_params_to_null() {
    let req: Request = serde_json::from_str(r#"{"id":7,"method":"ping"}"#).unwrap();
    assert_eq!(req.id, 7);
    assert_eq!(req.method, "ping");
    assert_eq!(req.params, Value::Null);
}

#[test]
fn ready_and_ping_expose_protocol_version() {
    assert_eq!(ready_event()["protocol"], PROTOCOL_VERSION);
    assert_eq!(ping_response()["protocol"], PROTOCOL_VERSION);
    assert_eq!(ping_response()["version"], VERSION);
}

#[test]
fn protocol_invoke_wraps_ping_response() {
    let value = invoke_protocol_json(r#"{"id":42,"method":"ping"}"#);
    assert_eq!(value["id"], 42);
    assert_eq!(value["result"]["pong"], true);
    assert_eq!(value["result"]["protocol"], PROTOCOL_VERSION);
}

#[test]
fn protocol_invoke_wraps_empty_account_list() {
    let value = invoke_protocol_json(r#"{"id":43,"method":"account.list","params":{}}"#);
    assert_eq!(value["id"], 43);
    assert_eq!(
        value["result"],
        json!({
            "accounts": []
        })
    );
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
    assert!(
        data_dir.join(avatar_url.trim_start_matches('/')).exists(),
        "{avatar_url}"
    );

    let wallpaper_request = format!(
        r#"{{"id":87,"method":"account.writeChatWallpaperFile","params":{{"id":"{email}","filename":"wallpaper.webp","mime":"image/webp","data":"d2FsbA=="}}}}"#
    );
    let wallpaper = invoke_mobile_protocol_json(&wallpaper_request, Some(data_dir_str));
    let wallpaper_url = wallpaper["result"]["url"].as_str().unwrap();
    assert!(
        wallpaper_url.starts_with("/media/wallpapers/"),
        "{wallpaper}"
    );
    assert!(
        data_dir
            .join(wallpaper_url.trim_start_matches('/'))
            .exists(),
        "{wallpaper_url}"
    );

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

    let usage =
        invoke_mobile_protocol_json(r#"{"id":92,"method":"storage.usage"}"#, Some(data_dir_str));
    assert_eq!(usage["id"], 92);
    assert_eq!(usage["result"]["cacheBytes"], 7);
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

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_persists_oauth_account_metadata() {
    let data_dir = unique_data_dir("oauth-account");
    let unique = unique_test_suffix();
    let email = format!("me+{unique}@gmail.com");
    let request = format!(
        r#"{{"id":63,"method":"account.addOAuth","params":{{"email":"{email}","provider":"gmail","display_name":"Gmail","sender_name":"Sender","access_token":"access","refresh_token":"refresh","token_expires_at":1700000000}}}}"#
    );
    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(added["id"], 63);
    assert_eq!(added["result"]["account"]["id"], email.as_str());
    assert_eq!(added["result"]["account"]["email"], email.as_str());
    assert_eq!(added["result"]["account"]["display_name"], "Gmail");
    assert_eq!(added["result"]["account"]["sender_name"], "Sender");
    assert_eq!(added["result"]["account"]["provider"], "gmail");
    assert_eq!(added["result"]["account"]["auth_type"], "gmail_oauth");
    assert_eq!(added["result"]["account"]["imap_host"], "imap.gmail.com");
    assert_eq!(added["result"]["account"]["imap_port"], 993);
    assert_eq!(added["result"]["account"]["smtp_host"], "smtp.gmail.com");
    assert_eq!(added["result"]["account"]["smtp_port"], 587);
    assert_eq!(added["result"]["account"]["needs_reconnect"], false);

    let listed = invoke_mobile_protocol_json(
        r#"{"id":64,"method":"account.list","params":{}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(listed["id"], 64);
    assert_eq!(listed["result"]["accounts"][0]["id"], email.as_str());
    assert_eq!(listed["result"]["accounts"][0]["needs_reconnect"], false);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_persists_platform_managed_oauth_account_without_refresh_token() {
    let data_dir = unique_data_dir("oauth-managed");
    let unique = unique_test_suffix();
    let email = format!("me+{unique}@gmail.com");
    let expires_at = now_epoch_seconds() + 3600;
    // Android AccountManager Gmail: access token only, no refresh token.
    let request = format!(
        r#"{{"id":70,"method":"account.addOAuth","params":{{"email":"{email}","provider":"gmail","display_name":"Gmail","access_token":"access-1","token_expires_at":{expires_at}}}}}"#
    );

    let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(added["id"], 70, "{added}");
    assert_eq!(added["result"]["account"]["id"], email.as_str(), "{added}");
    assert_eq!(
        added["result"]["account"]["auth_type"], "gmail_oauth",
        "{added}"
    );
    // No refresh token, but a live access token => not flagged for reconnect.
    assert_eq!(
        added["result"]["account"]["needs_reconnect"], false,
        "{added}"
    );

    // Platform pushes a freshly minted token before sync.
    let new_expires = now_epoch_seconds() + 3600;
    let update = format!(
        r#"{{"id":71,"method":"account.updateOAuthToken","params":{{"account_id":"{email}","access_token":"access-2","token_expires_at":{new_expires}}}}}"#
    );
    let updated = invoke_mobile_protocol_json(&update, Some(data_dir.to_str().unwrap()));
    assert_eq!(updated["id"], 71, "{updated}");
    assert_eq!(updated["result"]["ok"], true, "{updated}");
    assert_eq!(
        updated["result"]["account"]["needs_reconnect"], false,
        "{updated}"
    );

    let listed = invoke_mobile_protocol_json(
        r#"{"id":72,"method":"account.list","params":{}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(
        listed["result"]["accounts"][0]["needs_reconnect"], false,
        "{listed}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_update_oauth_token_rejects_unknown_account() {
    let data_dir = unique_data_dir("oauth-managed-missing");
    let update = r#"{"id":73,"method":"account.updateOAuthToken","params":{"account_id":"nobody@gmail.com","access_token":"x"}}"#;
    let updated = invoke_mobile_protocol_json(update, Some(data_dir.to_str().unwrap()));
    assert!(updated["error"]["message"].is_string(), "{updated}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_appends_new_accounts() {
    let data_dir = unique_data_dir("account-append");
    let data_dir_str = data_dir.to_str().unwrap();
    for (email, name) in [
        ("z-first@example.com", "Z First"),
        ("a-second@example.com", "A Second"),
        ("m-third@example.com", "M Third"),
    ] {
        let add_request = format!(
            r#"{{"id":93,"method":"account.addPassword","params":{{"email":"{email}","display_name":"{name}","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
        );
        assert!(
            invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
                .get("error")
                .is_none()
        );
    }
    let listed = invoke_mobile_protocol_json(
        r#"{"id":95,"method":"account.list","params":{}}"#,
        Some(data_dir_str),
    );
    let accounts = listed["result"]["accounts"].as_array().unwrap();
    let labels: Vec<_> = accounts
        .iter()
        .map(|account| account["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(labels, vec!["Z First", "A Second", "M Third"]);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_exchanges_oauth_code_and_persists_account() {
    let data_dir = unique_data_dir("oauth-code");
    let token_url = one_shot_oauth_token_server();
    let userinfo_url = one_shot_json_server(
        "/userinfo",
        r#"{"email":"google.user@gmail.com","name":"Google User","picture":"https://lh3.googleusercontent.com/avatar"}"#,
    );
    let unique = unique_test_suffix();
    let email = format!("me+{unique}@gmail.com");
    let request = format!(
        r#"{{"id":66,"method":"account.exchangeOAuthCode","params":{{"email":"{email}","provider":"gmail","display_name":"","sender_name":"","code":"auth-code","client_id":"client","client_secret":"secret","redirect_uri":"jp.nonbili.meron.oauth://oauth","code_verifier":"verifier","token_url":"{token_url}","userinfo_url":"{userinfo_url}"}}}}"#
    );

    let exchanged = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(exchanged["id"], 66, "{exchanged}");
    assert_eq!(
        exchanged["result"]["account"]["id"],
        email.as_str(),
        "{exchanged}"
    );
    assert_eq!(
        exchanged["result"]["account"]["provider"], "gmail",
        "{exchanged}"
    );
    assert_eq!(
        exchanged["result"]["account"]["auth_type"], "gmail_oauth",
        "{exchanged}"
    );
    assert_eq!(
        exchanged["result"]["account"]["display_name"], "Google User",
        "{exchanged}"
    );
    assert_eq!(
        exchanged["result"]["account"]["sender_name"], "Google User",
        "{exchanged}"
    );
    assert_eq!(
        exchanged["result"]["account"]["avatar_url"], "https://lh3.googleusercontent.com/avatar",
        "{exchanged}"
    );
    assert_eq!(
        exchanged["result"]["account"]["needs_reconnect"], false,
        "{exchanged}"
    );

    let listed = invoke_mobile_protocol_json(
        r#"{"id":67,"method":"account.list","params":{}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(
        listed["result"]["accounts"][0]["id"],
        email.as_str(),
        "{listed}"
    );
    assert_eq!(
        listed["result"]["accounts"][0]["needs_reconnect"], false,
        "{listed}"
    );
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    let config: String = conn
        .query_row(
            "SELECT config FROM accounts WHERE id = ?1",
            [&email],
            |row| row.get(0),
        )
        .unwrap();
    let config: serde_json::Value = serde_json::from_str(&config).unwrap();
    assert_eq!(config["oauth_client_id"], "client", "{config}");
    assert_eq!(config["oauth_client_secret"], "secret", "{config}");
    assert_eq!(config["oauth_token_url"], token_url.as_str(), "{config}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_exchanges_oauth_code_uses_id_token_claims() {
    let data_dir = unique_data_dir("oauth-code-claims");
    let token_url =
        one_shot_oauth_token_server_with_id_token("outlook.user@example.com", "Outlook User");
    let request = format!(
        r#"{{"id":68,"method":"account.exchangeOAuthCode","params":{{"email":"","provider":"outlook","display_name":"","sender_name":"","code":"auth-code","client_id":"client","redirect_uri":"msauth://jp.nonbili.meron/example","code_verifier":"verifier","token_url":"{token_url}"}}}}"#
    );

    let exchanged = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(exchanged["id"], 68, "{exchanged}");
    let account = &exchanged["result"]["account"];
    assert_eq!(account["id"], "outlook.user@example.com", "{exchanged}");
    assert_eq!(account["provider"], "outlook", "{exchanged}");
    assert_eq!(account["display_name"], "Outlook User", "{exchanged}");
    assert_eq!(account["sender_name"], "Outlook User", "{exchanged}");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    let config: String = conn
        .query_row(
            "SELECT config FROM accounts WHERE id = ?1",
            ["outlook.user@example.com"],
            |row| row.get(0),
        )
        .unwrap();
    let config: serde_json::Value = serde_json::from_str(&config).unwrap();
    assert_eq!(config["oauth_client_id"], "client", "{config}");
    assert_eq!(config["oauth_token_url"], token_url.as_str(), "{config}");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_rejects_unsupported_oauth_provider() {
    let data_dir = unique_data_dir("oauth-provider");
    let value = invoke_mobile_protocol_json(
        r#"{"id":65,"method":"account.addOAuth","params":{"email":"me@example.com","provider":"other","refresh_token":"refresh"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 65);
    assert_eq!(value["error"]["message"], "unsupported oauth provider");
    let _ = std::fs::remove_dir_all(data_dir);
}

fn one_shot_oauth_token_server() -> String {
    one_shot_oauth_token_server_with_body(
        r#"{"access_token":"access-from-code","refresh_token":"refresh-from-code","expires_in":3600}"#
            .to_string(),
    )
}

fn one_shot_oauth_token_server_with_id_token(email: &str, name: &str) -> String {
    use base64::Engine;

    let claims = serde_json::json!({
        "preferred_username": email,
        "name": name,
    });
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    let id_token = format!("header.{payload}.signature");
    one_shot_oauth_token_server_with_body(format!(
        r#"{{"access_token":"access-from-code","refresh_token":"refresh-from-code","expires_in":3600,"id_token":"{id_token}"}}"#
    ))
}

fn one_shot_oauth_token_server_with_body(body: String) -> String {
    one_shot_json_server("/token", &body)
}

fn one_shot_json_server(path: &str, body: &str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let body = body.to_string();
    let path = path.to_string();
    let server_path = path.clone();
    std::thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buf).unwrap_or(0);
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..read]);
                let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }
            if request.is_empty() {
                continue;
            }
            let request_line = String::from_utf8_lossy(&request);
            let expected = format!(" {server_path} ");
            if !request_line
                .lines()
                .next()
                .unwrap_or("")
                .contains(&expected)
            {
                continue;
            }
            write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            stream.flush().unwrap();
            return;
        }
    });
    format!("http://{addr}{path}")
}

#[test]
fn mobile_protocol_send_requires_usable_account_secret() {
    let data_dir = unique_data_dir("send-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");

    let value = invoke_mobile_protocol_json(
        r#"{"id":83,"method":"mail.send","params":{"account_id":"me@example.com","to":"you@example.com","subject":"Hi","body":"Hello"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 83);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_sync_requires_usable_account_secret() {
    let data_dir = unique_data_dir("sync-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");

    let value = invoke_mobile_protocol_json(
        r#"{"id":84,"method":"mail.sync","params":{"account_id":"me@example.com","folder_id":"inbox","limit":10}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 84);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let desktop_name = invoke_mobile_protocol_json(
        r#"{"id":85,"method":"messages.sync","params":{"account":"me@example.com","folder":"INBOX","limit":10}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(desktop_name["id"], 85);
    assert!(
        desktop_name["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_rejects_invalid_password_account() {
    let data_dir = unique_data_dir("invalid-account");
    let value = invoke_mobile_protocol_json(
        r#"{"id":62,"method":"account.addPassword","params":{"email":"missing-at","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"me"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 62);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid email")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_lists_folders_from_store() {
    let data_dir = unique_data_dir("folders");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                unread: 0,
            },
            Folder {
                name: "Archive".to_string(),
                delimiter: Some("/".to_string()),
                unread: 0,
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 1,
            subject: "Unread".to_string(),
            seen: false,
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":63,"method":"mail.folderList","params":{"account_id":"me@example.com"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 63);
    assert_eq!(value["result"]["folders"][0]["id"], "Archive");
    assert_eq!(value["result"]["folders"][0]["role"], "archive");
    assert_eq!(value["result"]["folders"][1]["id"], "INBOX");
    assert_eq!(value["result"]["folders"][1]["role"], "inbox");
    assert_eq!(value["result"]["folders"][1]["unread"], 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_folder_create_requires_usable_account_secret() {
    let data_dir = unique_data_dir("folder-create-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");

    let value = invoke_mobile_protocol_json(
        r#"{"id":66,"method":"mail.folderCreate","params":{"account_id":"me@example.com","name":"Work"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 66);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_move_requires_usable_account_secret() {
    let data_dir = unique_data_dir("move-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "Move me".to_string(),
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":67,"method":"mail.move","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_folder_id":"Work"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 67);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_copy_requires_usable_account_secret() {
    let data_dir = unique_data_dir("copy-needs-secret");
    seed_mobile_account(&data_dir, "me@example.com");
    seed_mobile_account(&data_dir, "target@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "target@example.com", "Archive").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "Copy me".to_string(),
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":68,"method":"mail.copy","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_account_id":"target@example.com","target_folder_id":"Archive"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 68);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_lists_threads_from_store() {
    let data_dir = unique_data_dir("threads");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 7,
                subject: "Newest".to_string(),
                from_name: "Ada".to_string(),
                from_addr: "ada@example.com".to_string(),
                date: 200,
                seen: false,
                starred: true,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 3,
                subject: "Older".to_string(),
                from_name: "Bea".to_string(),
                from_addr: "bea@example.com".to_string(),
                date: 100,
                seen: true,
                starred: false,
                thread_key: "older-topic".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":64,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 64);
    let first = &value["result"]["threads"][0];
    assert_eq!(first["account_id"], "me@example.com");
    assert_eq!(first["folder_id"], "INBOX");
    assert_eq!(first["subject"], "Newest");
    assert_eq!(first["from_name"], "Ada");
    assert_eq!(first["unread"], true);
    assert_eq!(first["unread_count"], 1);
    assert_eq!(first["starred"], true);
    assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");

    let unread = invoke_mobile_protocol_json(
        r#"{"id":65,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"unread"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(unread["result"]["threads"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_keeps_gmail_thread_id_atomic_across_subject_drift() {
    let data_dir = unique_data_dir("gmail-thread-id");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 10,
                subject: "[nonbili/Nora] Profiles bug (Issue #295)".to_string(),
                from_name: "NightStars".to_string(),
                date: 100,
                seen: true,
                thread_key: "gmthrid:123".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 11,
                subject: "[nonbili/Nora] Profiles bug [Linux Flatpak] (Issue #295)".to_string(),
                from_name: "NightStars".to_string(),
                date: 200,
                seen: false,
                thread_key: "gmthrid:123".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":166,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let threads = value["result"]["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1, "{value}");
    assert_eq!(
        threads[0]["thread_id"],
        "me@example.com#INBOX#t.Z210aHJpZDoxMjM"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_inherits_parent_thread_key_for_sent_reply_chain() {
    let data_dir = unique_data_dir("sent-reply-chain");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "me@example.com", "Sent").unwrap();

    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 7,
            subject: "test mailo2".to_string(),
            from_name: "Gmail".to_string(),
            from_addr: "sender@gmail.com".to_string(),
            date: 100,
            seen: true,
            thread_key: "root@mail.gmail.com".to_string(),
            message_id: "root@mail.gmail.com".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "INBOX",
        7,
        &Message {
            subject: "test mailo2".to_string(),
            from_name: "Gmail".to_string(),
            from_addr: "sender@gmail.com".to_string(),
            to: "Me <me@example.com>".to_string(),
            message_id: "root@mail.gmail.com".to_string(),
            date: 100,
            body: "root".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    // Reproduces a fresh account add / Sent sync: newest rows can be processed
    // before older parents. The second sent reply has no usable References root,
    // so its computed key fell back to In-Reply-To (the first sent reply), even
    // though that parent appears later in the same batch.
    store::upsert_messages(
        &conn,
        "me@example.com",
        "Sent",
        &[
            MessageHeader {
                uid: 9,
                subject: "Re: test mailo2".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 300,
                seen: true,
                thread_key: "first-reply@mailo.com".to_string(),
                message_id: "second-reply@mailo.com".to_string(),
                in_reply_to: "first-reply@mailo.com".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 8,
                subject: "Re: test mailo2".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 200,
                seen: true,
                thread_key: "root@mail.gmail.com".to_string(),
                message_id: "first-reply@mailo.com".to_string(),
                in_reply_to: "root@mail.gmail.com".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Sent",
        8,
        &Message {
            subject: "Re: test mailo2".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            to: "sender@gmail.com".to_string(),
            message_id: "first-reply@mailo.com".to_string(),
            references: "root@mail.gmail.com".to_string(),
            date: 200,
            body: "first".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Sent",
        9,
        &Message {
            subject: "Re: test mailo2".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            to: "sender@gmail.com".to_string(),
            message_id: "second-reply@mailo.com".to_string(),
            references: "root@mail.gmail.com first-reply@mailo.com".to_string(),
            date: 300,
            body: "second".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let sent = invoke_mobile_protocol_json(
        r#"{"id":167,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"Sent","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let sent_threads = sent["result"]["threads"].as_array().unwrap();
    assert_eq!(sent_threads.len(), 1, "{sent}");
    assert_eq!(
        sent_threads[0]["thread_id"],
        "me@example.com#Sent#t.cm9vdEBtYWlsLmdtYWlsLmNvbQ"
    );

    let read = invoke_mobile_protocol_json(
        r#"{"id":168,"method":"mail.threadRead","params":{"thread_id":"me@example.com#INBOX#t.cm9vdEBtYWlsLmdtYWlsLmNvbQ"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let messages = read["result"]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "{read}");
    assert_eq!(messages[0]["message_id"], "root@mail.gmail.com");
    assert_eq!(messages[1]["message_id"], "first-reply@mailo.com");
    assert_eq!(messages[2]["message_id"], "second-reply@mailo.com");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reads_cached_thread_messages_from_store() {
    let data_dir = unique_data_dir("thread-read");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 9,
            subject: "Cached subject".to_string(),
            from_name: "Ada".to_string(),
            from_addr: "ada@example.com".to_string(),
            date: 300,
            seen: false,
            starred: true,
            thread_key: "topic".to_string(),
            to: vec![Recipient {
                name: "Me".to_string(),
                addr: "me@example.com".to_string(),
            }],
            ..Default::default()
        }],
    )
    .unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "INBOX",
        9,
        &Message {
            subject: "Cached subject".to_string(),
            from_name: "Ada".to_string(),
            from_addr: "ada@example.com".to_string(),
            to: "Me <me@example.com>".to_string(),
            message_id: "topic".to_string(),
            references: "root".to_string(),
            date: 300,
            body: "Hello from cache".to_string(),
            body_html: Some("<p>Hello from cache</p>".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":66,"method":"mail.threadRead","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 66);
    let first = &value["result"]["messages"][0];
    assert_eq!(first["id"], "me@example.com#INBOX#t.dG9waWM#9");
    assert_eq!(first["account_id"], "me@example.com");
    assert_eq!(first["folder_id"], "INBOX");
    assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");
    assert_eq!(first["from_name"], "Ada");
    assert_eq!(first["to"], "Me <me@example.com>");
    assert_eq!(first["message_id"], "topic");
    assert_eq!(first["references"], "root");
    assert_eq!(first["subject"], "Cached subject");
    assert_eq!(first["body"], "Hello from cache");
    assert_eq!(first["body_html"], "<p>Hello from cache</p>");
    assert_eq!(first["unread"], true);
    assert_eq!(first["starred"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_reads_uid_thread_message_from_store() {
    let data_dir = unique_data_dir("uid-thread-read");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Drafts",
        4,
        &Message {
            subject: "Draft".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            date: 400,
            body: "Draft body".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":67,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 67);
    assert_eq!(value["result"]["messages"][0]["subject"], "Draft");
    assert_eq!(value["result"]["messages"][0]["body"], "Draft body");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_mark_read_persists_locally_before_server_sync() {
    let data_dir = unique_data_dir("thread-actions");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 10,
                subject: "First".to_string(),
                seen: false,
                starred: false,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 11,
                subject: "Second".to_string(),
                seen: false,
                starred: false,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let read = invoke_mobile_protocol_json(
        r#"{"id":68,"method":"mail.markRead","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","seen":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["id"], 68);
    assert!(
        read["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let starred = invoke_mobile_protocol_json(
        r#"{"id":69,"method":"mail.markStarred","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","starred":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(starred["id"], 69);
    assert!(
        starred["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let mark_all = invoke_mobile_protocol_json(
        r#"{"id":72,"method":"mail.markAllRead","params":{"account_id":"me@example.com","folder_id":"INBOX"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(mark_all["id"], 72);
    assert_eq!(mark_all["result"]["ok"], true);

    let threads = invoke_mobile_protocol_json(
        r#"{"id":70,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let first = &threads["result"]["threads"][0];
    assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");
    assert_eq!(first["unread"], false);
    assert_eq!(first["unread_count"], 0);
    assert_eq!(first["starred"], false);

    let unread = invoke_mobile_protocol_json(
        r#"{"id":71,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"unread"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(unread["result"]["threads"].as_array().unwrap().len(), 0);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_marks_uid_thread_read_locally_before_server_sync() {
    let data_dir = unique_data_dir("uid-thread-actions");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Drafts",
        4,
        &Message {
            subject: "Draft".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            date: 400,
            body: "Draft body".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":72,"method":"mail.markRead","params":{"thread_id":"me@example.com#Drafts#4","seen":true}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let read = invoke_mobile_protocol_json(
        r#"{"id":73,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["result"]["messages"][0]["unread"], false);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_lists_starred_items_from_store() {
    let data_dir = unique_data_dir("starred-items");
    seed_mobile_account(&data_dir, "a1@example.com");
    seed_mobile_account(&data_dir, "a2@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "a1@example.com", "INBOX").unwrap();
    store::ensure_folder(&conn, "a2@example.com", "Sent").unwrap();
    store::upsert_messages(
        &conn,
        "a1@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 1,
                subject: "Older starred".to_string(),
                from_name: "Ada".to_string(),
                from_addr: "ada@example.com".to_string(),
                date: 100,
                seen: false,
                starred: true,
                thread_key: "older".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 2,
                subject: "Not starred".to_string(),
                date: 300,
                seen: false,
                starred: false,
                thread_key: "skip".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "a2@example.com",
        "Sent",
        &[MessageHeader {
            uid: 7,
            subject: "Newest starred".to_string(),
            from_name: "Bea".to_string(),
            from_addr: "bea@example.com".to_string(),
            date: 200,
            seen: true,
            starred: true,
            thread_key: "newer".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":74,"method":"mail.starredItems","params":{"limit":2}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 74);
    let items = value["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["account_id"], "a2@example.com");
    assert_eq!(items[0]["folder_id"], "Sent");
    assert_eq!(items[0]["subject"], "Newest starred");
    assert_eq!(items[0]["thread_id"], "a2@example.com#Sent#t.bmV3ZXI");
    assert_eq!(items[0]["id"], "a2@example.com#Sent#t.bmV3ZXI#7");
    assert_eq!(items[0]["unread"], false);
    assert_eq!(items[0]["starred"], true);
    assert_eq!(items[1]["account_id"], "a1@example.com");
    assert_eq!(items[1]["subject"], "Older starred");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_routes_rss_accounts_through_mail_shapes() {
    let data_dir = unique_data_dir("rss-mail-shapes");
    seed_rss_account(&data_dir, "rss-account", "News");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    seed_rss_feed(&conn, "rss-account", "feed-a", "Feed A");
    seed_rss_item(
        &conn,
        "rss-account",
        "feed-a",
        "item-old",
        "Old item",
        100,
        true,
    );
    seed_rss_item(
        &conn,
        "rss-account",
        "feed-a",
        "item-new",
        "New item",
        200,
        true,
    );
    drop(conn);

    let folders = invoke_mobile_protocol_json(
        r#"{"id":86,"method":"mail.folderList","params":{"account_id":"rss-account"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(folders["id"], 86);
    assert_eq!(folders["result"]["folders"][0]["id"], "inbox");
    assert_eq!(folders["result"]["folders"][0]["role"], "inbox");
    assert_eq!(folders["result"]["folders"][0]["unread"], 2);

    let threads = invoke_mobile_protocol_json(
        r#"{"id":87,"method":"mail.threadList","params":{"account_id":"rss-account","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(threads["id"], 87);
    let first = &threads["result"]["threads"][0];
    assert_eq!(first["thread_id"], "rss-account#rss#feed-a");
    assert_eq!(first["account_id"], "rss-account");
    assert_eq!(first["folder_id"], "inbox");
    assert_eq!(first["from_name"], "Feed A");
    assert_eq!(first["subject"], "Feed A");
    assert_eq!(first["unread"], true);
    assert_eq!(first["unread_count"], 2);

    let read = invoke_mobile_protocol_json(
        r#"{"id":88,"method":"mail.threadRead","params":{"thread_id":"rss-account#rss#feed-a","limit":1}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["id"], 88);
    assert_eq!(
        read["result"]["messages"][0]["id"],
        "rss-account#rss#feed-a#item-new"
    );
    assert_eq!(read["result"]["messages"][0]["subject"], "New item");
    assert_eq!(
        read["result"]["messages"][0]["body"],
        "Body for New item\n\nSource: https://example.com/item-new"
    );
    assert_eq!(read["result"]["messages"][0]["unread"], true);
    assert_eq!(read["result"]["next_cursor"], "ts:200:item-new");

    let mark_read = invoke_mobile_protocol_json(
        r#"{"id":89,"method":"mail.markRead","params":{"thread_id":"rss-account#rss#feed-a","seen":true,"message_ids":["rss-account#rss#feed-a#item-new"]}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(mark_read["result"]["ok"], true);

    let mark_starred = invoke_mobile_protocol_json(
        r#"{"id":90,"method":"mail.markStarred","params":{"thread_id":"rss-account#rss#feed-a","starred":true,"item_keys":["item-new"]}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(mark_starred["result"]["ok"], true);

    let reread = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"rss.thread","params":{"thread_id":"rss-account#rss#feed-a"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let messages = reread["result"]["messages"].as_array().unwrap();
    let new_item = messages
        .iter()
        .find(|message| message["id"] == "rss-account#rss#feed-a#item-new")
        .unwrap();
    assert_eq!(new_item["unread"], false);
    assert_eq!(new_item["starred"], true);

    let starred = invoke_mobile_protocol_json(
        r#"{"id":92,"method":"mail.starredItems","params":{"limit":5}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(
        starred["result"]["items"][0]["id"],
        "rss-account#rss#feed-a#item-new"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_exports_and_imports_opml() {
    let data_dir = unique_data_dir("rss-opml-mobile");
    seed_rss_account(&data_dir, "rss-source", "Source feeds");
    seed_rss_account(&data_dir, "rss-target", "Target feeds");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    seed_rss_feed(&conn, "rss-source", "feed-a", "Feed A");
    drop(conn);

    let exported = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"rss.exportOpml","params":{"account":"rss-source"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(exported["id"], 91);
    let opml = exported["result"]["opml"].as_str().unwrap();
    assert!(opml.contains("<opml version=\"2.0\">"));
    assert!(opml.contains("Feed A"));

    let request = serde_json::json!({
            "id": 92,
            "method": "rss.importOpml",
            "params": {
                "account": "rss-target",
                "opml": r#"<?xml version="1.0"?><opml version="2.0"><body><outline text="Imported"><outline type="rss" text="Imported Feed" xmlUrl="https://import.example/feed.xml"/></outline></body></opml>"#,
            },
        })
        .to_string();
    let imported = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(imported["id"], 92);
    assert_eq!(imported["result"]["imported"], 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_syncs_empty_rss_account_without_mail_credentials() {
    let data_dir = unique_data_dir("rss-sync-empty");
    seed_rss_account(&data_dir, "rss-empty", "RSS");

    let value = invoke_mobile_protocol_json(
        r#"{"id":93,"method":"mail.sync","params":{"account_id":"rss-empty"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 93);
    assert_eq!(value["result"]["ok"], true);
    assert_eq!(value["result"]["account"], "rss-empty");
    assert_eq!(value["result"]["synced"], 0);
    assert_eq!(value["result"]["rss"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_delete_requires_server_credentials() {
    let data_dir = unique_data_dir("thread-delete");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 20,
                subject: "Delete me".to_string(),
                date: 300,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 21,
                subject: "Delete me too".to_string(),
                date: 200,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 22,
                subject: "Keep me".to_string(),
                date: 100,
                thread_key: "other".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":75,"method":"mail.delete","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 75);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let threads = invoke_mobile_protocol_json(
        r#"{"id":76,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let rows = threads["result"]["threads"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["subject"], "Delete me");
    assert_eq!(rows[1]["subject"], "Keep me");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_delete_uid_thread_requires_server_credentials() {
    let data_dir = unique_data_dir("uid-thread-delete");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::save_cached_message(
        &conn,
        "me@example.com",
        "Drafts",
        4,
        &Message {
            subject: "Draft".to_string(),
            from_name: "Me".to_string(),
            from_addr: "me@example.com".to_string(),
            date: 400,
            body: "Draft body".to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":77,"method":"mail.delete","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let read = invoke_mobile_protocol_json(
        r#"{"id":78,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["result"]["messages"].as_array().unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_archive_requires_server_credentials() {
    let data_dir = unique_data_dir("thread-archive");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::upsert_folders(
        &conn,
        "me@example.com",
        &[
            Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                unread: 0,
            },
            Folder {
                name: "Archive".to_string(),
                delimiter: Some("/".to_string()),
                unread: 0,
            },
        ],
    )
    .unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 31,
                subject: "Archive me".to_string(),
                date: 300,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 32,
                subject: "Archive me too".to_string(),
                date: 200,
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 33,
                subject: "Keep me".to_string(),
                date: 100,
                thread_key: "other".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":79,"method":"mail.archive","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 79);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("account needs reconnect")
    );

    let inbox = invoke_mobile_protocol_json(
        r#"{"id":80,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let inbox_rows = inbox["result"]["threads"].as_array().unwrap();
    assert_eq!(inbox_rows.len(), 2);
    assert_eq!(inbox_rows[0]["subject"], "Archive me");
    assert_eq!(inbox_rows[1]["subject"], "Keep me");

    let archive = invoke_mobile_protocol_json(
        r#"{"id":81,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"Archive","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let archive_rows = archive["result"]["threads"].as_array().unwrap();
    assert_eq!(archive_rows.len(), 0);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_archive_requires_cached_archive_folder() {
    let data_dir = unique_data_dir("thread-archive-missing-folder");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[MessageHeader {
            uid: 41,
            subject: "Archive me".to_string(),
            thread_key: "topic".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();
    drop(conn);

    let value = invoke_mobile_protocol_json(
        r#"{"id":82,"method":"mail.archive","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(value["id"], 82);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Archive folder not found")
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn protocol_invoke_wraps_empty_mail_collections() {
    let cases = [
        (
            r#"{"id":44,"method":"mail.folderList","params":{"account_id":"acc"}}"#,
            json!({ "folders": [] }),
        ),
        (
            r#"{"id":45,"method":"mail.threadList","params":{"account_id":"acc","folder_id":"inbox"}}"#,
            json!({ "threads": [] }),
        ),
        (
            r#"{"id":46,"method":"mail.threadRead","params":{"thread_id":"thread"}}"#,
            json!({ "messages": [] }),
        ),
        (
            r#"{"id":47,"method":"mail.starredItems","params":{}}"#,
            json!({ "items": [] }),
        ),
        (
            r#"{"id":48,"method":"rss.exportOpml","params":{"account":"rss"}}"#,
            json!({ "opml": "" }),
        ),
    ];

    for (request, expected_result) in cases {
        let value = invoke_protocol_json(request);
        assert_eq!(value["result"], expected_result);
        assert!(value.get("error").is_none());
    }
}

#[test]
fn protocol_invoke_wraps_noop_mail_mutations() {
    let cases = [
        (
            r#"{"id":48,"method":"mail.send","params":{"account_id":"acc","to":"you@example.com"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":71,"method":"mail.saveDraft","params":{"account_id":"acc","draft_id":"draft@example.com"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":72,"method":"mail.discardDraft","params":{"account_id":"acc","draft_id":"draft@example.com"}}"#,
            json!({ "ok": true, "deleted": 0, "permanent": true }),
        ),
        (
            r#"{"id":54,"method":"rss.importOpml","params":{"account":"rss","opml":"<opml/>"}}"#,
            json!({ "imported": 0 }),
        ),
        (
            r#"{"id":73,"method":"account.addRSS","params":{"feed_url":"https://example.com/feed.xml","display_name":"News"}}"#,
            json!({ "account": Value::Null }),
        ),
        (
            r#"{"id":74,"method":"rss.addFeed","params":{"account_id":"rss","feed_url":"https://example.com/feed.xml"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":75,"method":"rss.removeFeed","params":{"thread_id":"rss#rss#feed"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":76,"method":"rss.moveFeed","params":{"thread_id":"rss#rss#feed","target_account_id":"rss2"}}"#,
            json!({ "ok": true, "moved": 0 }),
        ),
        (
            r#"{"id":77,"method":"mail.readAttachment","params":{"key":"acc/INBOX/1/note.txt"}}"#,
            json!({ "data": "" }),
        ),
        (
            r#"{"id":55,"method":"account.remove","params":{"id":"acc"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":56,"method":"account.setName","params":{"id":"acc","name":"Personal"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":57,"method":"account.setSenderName","params":{"id":"acc","name":"Sender"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":58,"method":"account.setAvatar","params":{"id":"acc","avatar_url":"https://example.com/avatar.png"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":59,"method":"account.setChatWallpaper","params":{"id":"acc","wallpaper":{"kind":"preset","presetId":"grid"}}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":60,"method":"account.writeAvatarFile","params":{"id":"acc","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}}"#,
            json!({ "url": "" }),
        ),
        (
            r#"{"id":61,"method":"account.writeChatWallpaperFile","params":{"id":"acc","filename":"wallpaper.png","mime":"image/png","data":"aGVsbG8="}}"#,
            json!({ "url": "" }),
        ),
        (
            r#"{"id":62,"method":"account.setImages","params":{"id":"acc","enabled":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":63,"method":"account.setConversationHtml","params":{"id":"acc","enabled":false}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":64,"method":"account.setUnified","params":{"id":"acc","enabled":false}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":65,"method":"account.setMuted","params":{"id":"acc","enabled":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":66,"method":"account.setPaused","params":{"id":"acc","enabled":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":67,"method":"account.setRSSSyncInterval","params":{"id":"acc","minutes":30}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":68,"method":"account.setAliases","params":{"id":"acc","aliases":[]}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":69,"method":"account.reorder","params":{"accounts":["b","a"]}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":70,"method":"mail.suggestContacts","params":{"account":"acc","query":"a","limit":8}}"#,
            json!({ "contacts": [] }),
        ),
        (
            r#"{"id":49,"method":"mail.archive","params":{"thread_id":"thread"}}"#,
            json!({ "ok": true, "moved": 0 }),
        ),
        (
            r#"{"id":50,"method":"mail.delete","params":{"thread_id":"thread"}}"#,
            json!({ "ok": true, "deleted": 0 }),
        ),
        (
            r#"{"id":51,"method":"mail.markRead","params":{"thread_id":"thread","seen":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":52,"method":"mail.markAllRead","params":{"account_id":"acc","folder_id":"INBOX"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":53,"method":"mail.markStarred","params":{"thread_id":"thread","starred":true}}"#,
            json!({ "ok": true }),
        ),
    ];

    for (request, expected_result) in cases {
        let value = invoke_protocol_json(request);
        assert_eq!(value["result"], expected_result);
        assert!(value.get("error").is_none());
    }
}

#[test]
fn protocol_invoke_wraps_unknown_method_error() {
    let value = invoke_protocol_json(r#"{"id":9,"method":"missing"}"#);
    assert_eq!(value["id"], 9);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown method")
    );
}

fn unique_data_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("meron-mobile-{label}-{}", unique_test_suffix()))
}

fn unique_test_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    let count = TEST_UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{pid}-{nanos}-{count}")
}

fn seed_mobile_account(data_dir: &std::path::Path, email: &str) {
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    let creds = Creds {
        host: "imap.example.com".to_string(),
        port: 993,
        user: email.to_string(),
        password: String::new(),
        tls: true,
        starttls: false,
        smtp_host: "smtp.example.com".to_string(),
        smtp_port: 465,
        smtp_tls: true,
        smtp_starttls: false,
        auth_type: "password".to_string(),
        access_token: None,
        refresh_token: None,
        token_expires_at: 0,
        oauth_client_id: String::new(),
        oauth_client_secret: String::new(),
        oauth_token_url: String::new(),
        oauth_scope: String::new(),
    };
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: email.to_string(),
        display_name: String::new(),
        avatar_url: String::new(),
        sender_name: String::new(),
    };
    store::upsert_account(&conn, email, &meta, &creds).unwrap();
}

fn seed_rss_account(data_dir: &std::path::Path, account: &str, display_name: &str) {
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, email, display_name, config)
             VALUES(?1, 'rss', 'rss', ?2, ?3, '{}')",
        rusqlite::params![account, format!("{account}.local"), display_name],
    )
    .unwrap();
}

fn seed_rss_feed(conn: &rusqlite::Connection, account: &str, sub_id: &str, title: &str) {
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, feed_title, enabled)
             VALUES(?1, ?2, ?3, ?4, ?4, 1)",
        rusqlite::params![
            sub_id,
            account,
            format!("https://example.com/{sub_id}"),
            title
        ],
    )
    .unwrap();
}

fn seed_rss_item(
    conn: &rusqlite::Connection,
    account: &str,
    sub_id: &str,
    item_key: &str,
    title: &str,
    ts: i64,
    unread: bool,
) {
    store::upsert_rss_item(
        conn,
        account,
        sub_id,
        item_key,
        title,
        unread,
        Some(&format!("<p>Body for {title}</p>")),
        &RssItemExtra {
            author: "Author".to_string(),
            link: format!("https://example.com/{item_key}"),
            summary: format!("Summary for {title}"),
            content: String::new(),
            images: Vec::<RssMedia>::new(),
            videos: Vec::<RssMedia>::new(),
            published_at: ts,
            updated_at: 0,
            fetched_at: ts,
        },
    )
    .unwrap();
}

#[test]
fn requested_mobile_uids_uses_message_ids_without_thread_fallback() {
    let data_dir = unique_data_dir("targeted-uids");
    seed_mobile_account(&data_dir, "me@example.com");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
    store::upsert_messages(
        &conn,
        "me@example.com",
        "INBOX",
        &[
            MessageHeader {
                uid: 7,
                subject: "One".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            },
            MessageHeader {
                uid: 8,
                subject: "Two".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            },
        ],
    )
    .unwrap();
    let parsed = ParsedThreadId {
        account: "me@example.com".to_string(),
        folder: "INBOX".to_string(),
        thread_key: "topic".to_string(),
        uid: None,
    };

    let targeted = requested_mobile_uids(
        &conn,
        &parsed,
        &json!({ "message_ids": ["me@example.com#INBOX#t.dG9waWM#8"] }),
    )
    .unwrap();
    assert_eq!(targeted, vec![8]);

    let malformed =
        requested_mobile_uids(&conn, &parsed, &json!({ "message_ids": ["bad"] })).unwrap();
    assert!(malformed.is_empty());

    let fallback = requested_mobile_uids(&conn, &parsed, &json!({})).unwrap();
    assert_eq!(fallback, vec![7, 8]);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn parse_thread_id_with_request_folder_overrides_encoded_folder() {
    let parsed = parse_thread_id_with_request_folder(
        "me@example.com#INBOX#t.dG9waWM",
        &json!({ "folder": "Trash" }),
    )
    .unwrap();

    assert_eq!(parsed.account, "me@example.com");
    assert_eq!(parsed.folder, "Trash");
    assert_eq!(parsed.thread_key, "topic");
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

fn add_password_account(data_dir_str: &str, email: &str) {
    let add = format!(
        r#"{{"id":1,"method":"account.addPassword","params":{{"email":"{email}","display_name":"E","sender_name":"E","imap_host":"imap.example.com","imap_port":993,"smtp_host":"smtp.example.com","smtp_port":587,"username":"{email}","password":"secret","tls":true}}}}"#
    );
    let added = invoke_mobile_protocol_json(&add, Some(data_dir_str));
    assert_eq!(added["result"]["account"]["id"], email);
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
