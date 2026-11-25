use super::*;

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
fn mobile_protocol_platform_oauth_update_clears_old_refresh_token() {
    let data_dir = unique_data_dir("oauth-managed-clears-refresh");
    let data_dir_str = data_dir.to_str().unwrap();
    let unique = unique_test_suffix();
    let email = format!("managed-reconnect+{unique}@gmail.com");
    let expired = now_epoch_seconds() - 3600;
    let add_browser = format!(
        r#"{{"id":74,"method":"account.addOAuth","params":{{"email":"{email}","provider":"gmail","display_name":"Gmail","access_token":"browser-access","refresh_token":"browser-refresh","token_expires_at":{expired}}}}}"#
    );
    let added = invoke_mobile_protocol_json(&add_browser, Some(data_dir_str));
    assert_eq!(added["id"], 74, "{added}");

    let managed_expires = now_epoch_seconds() + 3600;
    let update_managed = format!(
        r#"{{"id":75,"method":"account.updateOAuthToken","params":{{"account_id":"{email}","access_token":"managed-access","token_expires_at":{managed_expires}}}}}"#
    );
    let updated = invoke_mobile_protocol_json(&update_managed, Some(data_dir_str));
    assert_eq!(updated["id"], 75, "{updated}");
    assert_eq!(updated["result"]["ok"], true, "{updated}");

    let engine = crate::engine::Engine::new(Box::new(MobileHost {
        data_dir: data_dir_str.to_string(),
    }))
    .expect("engine builds");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let creds = rt
        .block_on(engine.ensure_valid_creds(&email))
        .expect("platform-managed token should not use browser refresh");
    assert_eq!(creds.access_token.as_deref(), Some("managed-access"));
    assert_eq!(creds.refresh_token.as_deref(), None);

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
    let picture_url = one_shot_server("/avatar", "image/png", "png");
    let userinfo_url = one_shot_json_server(
        "/userinfo",
        &format!(
            r#"{{"email":"google.user@gmail.com","name":"Google User","picture":"{picture_url}"}}"#
        ),
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
    // The picture is stored locally, like a chosen avatar.
    let avatar_url = exchanged["result"]["account"]["avatar_url"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        avatar_url.starts_with("/media/avatars/") && avatar_url.ends_with(".png"),
        "{exchanged}"
    );
    assert_eq!(
        std::fs::read(data_dir.join(avatar_url.trim_start_matches('/'))).unwrap(),
        b"png"
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
fn mobile_protocol_exchanges_outlook_code_saves_graph_photo() {
    use base64::Engine;
    use std::io::Write;
    use std::net::TcpListener;

    let data_dir = unique_data_dir("oauth-code-photo");
    let claims = serde_json::json!({ "preferred_username": "photo.user@example.com" });
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    let code_body = format!(
        r#"{{"access_token":"imap","refresh_token":"refresh","expires_in":3600,"id_token":"h.{payload}.s"}}"#
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // Code exchange, Graph token redemption, then the photo itself.
    std::thread::spawn(move || {
        for step in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let request = crate::test_support::read_http_request(&mut stream);
            let (content_type, body): (&str, Vec<u8>) = match step {
                0 => ("application/json", code_body.clone().into_bytes()),
                1 => ("application/json", br#"{"access_token":"graph"}"#.to_vec()),
                _ => {
                    assert!(request.starts_with("GET /photo "), "{request}");
                    assert!(request.contains("Bearer graph"), "{request}");
                    ("image/jpeg", b"\xff\xd8\xff\xe0photo".to_vec())
                }
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let request = format!(
        r#"{{"id":69,"method":"account.exchangeOAuthCode","params":{{"email":"","provider":"outlook","code":"auth-code","client_id":"client","code_verifier":"verifier","token_url":"http://{addr}/token","photo_url":"http://{addr}/photo"}}}}"#
    );

    let exchanged = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    let avatar_url = exchanged["result"]["account"]["avatar_url"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        avatar_url.starts_with("/media/avatars/photo_user_example_com/")
            && avatar_url.ends_with(".jpg"),
        "{exchanged}"
    );
    let saved = std::fs::read(data_dir.join(avatar_url.trim_start_matches('/'))).unwrap();
    assert_eq!(saved, b"\xff\xd8\xff\xe0photo");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_oauth_reconnect_keeps_the_stored_avatar() {
    let data_dir = unique_data_dir("oauth-keeps-avatar");
    let data_dir_str = data_dir.to_str().unwrap();
    let email = format!("keep+{}@gmail.com", unique_test_suffix());
    let add = |avatar: &str| {
        invoke_mobile_protocol_json(
            &format!(
                r#"{{"id":1,"method":"account.addOAuth","params":{{"email":"{email}","provider":"gmail","avatar_url":"{avatar}","access_token":"access","refresh_token":"refresh"}}}}"#
            ),
            Some(data_dir_str),
        )
    };
    let first = add("https://example.com/provider.png");
    assert_eq!(
        first["result"]["account"]["avatar_url"], "https://example.com/provider.png",
        "{first}"
    );
    let set = invoke_mobile_protocol_json(
        &format!(
            r#"{{"id":2,"method":"account.setAvatar","params":{{"id":"{email}","avatar_url":"/media/avatars/custom.png"}}}}"#
        ),
        Some(data_dir_str),
    );
    assert!(set.get("error").is_none(), "{set}");

    // A reconnect offers the provider's photo again; the chosen one stays.
    let again = add("https://example.com/provider.png");
    assert_eq!(
        again["result"]["account"]["avatar_url"], "/media/avatars/custom.png",
        "{again}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn engine_refreshes_outlook_with_the_accounts_client_and_imap_scopes() {
    use std::io::Write;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let token_url = format!("http://{}/token", listener.local_addr().unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = crate::test_support::read_http_request(&mut stream);
        tx.send(request).unwrap();
        let body = r#"{"access_token":"fresh","refresh_token":"refresh-2","expires_in":3600}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });

    let data_dir = unique_data_dir("engine-outlook-refresh");
    let data_dir_str = data_dir.to_str().unwrap();
    let email = format!("refresh+{}@outlook.com", unique_test_suffix());
    let added = invoke_mobile_protocol_json(
        &format!(
            r#"{{"id":1,"method":"account.addOAuth","params":{{"email":"{email}","provider":"outlook","client_id":"own-client","token_url":"{token_url}","access_token":"stale","refresh_token":"refresh","token_expires_at":1}}}}"#
        ),
        Some(data_dir_str),
    );
    assert!(added.get("error").is_none(), "{added}");

    let engine = crate::engine::Engine::new(Box::new(MobileHost {
        data_dir: data_dir_str.to_string(),
    }))
    .expect("engine builds");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let creds = rt
        .block_on(engine.ensure_valid_creds(&email))
        .expect("refreshes");
    assert_eq!(creds.access_token.as_deref(), Some("fresh"));

    let request = rx.recv().unwrap();
    assert!(request.contains("client_id=own-client"), "{request}");
    assert!(request.contains("IMAP.AccessAsUser.All"), "{request}");
    assert!(!request.contains("client_secret"), "{request}");

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

#[test]
fn mobile_oauth_sign_in_keeps_every_service_sharing_the_grant() {
    let answer = invoke_mobile_protocol_json(
        &json!({
            "id": 420,
            "method": "oauth.authorizeUrl",
            "params": {
                "provider": "google_calendar",
                "client_id": "client.apps.example.test",
                "redirect_uri": "test.example:/oauth",
                "state": "s",
                "code_challenge": "c",
                "login_hint": "mira@example.test",
                "scopes": [
                    crate::oauth::GOOGLE_CALENDAR_SCOPE,
                    crate::oauth::GOOGLE_TASKS_SCOPE,
                    "https://mail.google.com/",
                ],
            },
        })
        .to_string(),
        None,
    );
    let url = url::Url::parse(answer["result"]["url"].as_str().unwrap()).unwrap();
    let scope = url
        .query_pairs()
        .find(|(key, _)| key == "scope")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    assert!(
        scope.contains(crate::oauth::GOOGLE_CALENDAR_SCOPE),
        "{scope}"
    );
    assert!(scope.contains(crate::oauth::GOOGLE_TASKS_SCOPE), "{scope}");
    // Only the services that share a grant can be added to a sign-in.
    assert!(!scope.contains("mail.google.com"), "{scope}");
}
