use super::*;

pub(crate) fn list_mobile_accounts(data_dir: &str) -> Result<Value, String> {
    with_mobile_db(data_dir, |conn| {
        let mut accounts = store::list_accounts(&conn).map_err(|err| err.to_string())?;
        let creds_by_id = store::load_accounts(&conn)
            .map_err(|err| err.to_string())?
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        for account in &mut accounts {
            if account
                .get("auth_type")
                .and_then(Value::as_str)
                .is_some_and(|auth_type| auth_type == "rss")
            {
                continue;
            }
            let needs_reconnect = account
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| {
                    let mut creds = creds_by_id.get(id)?.clone();
                    load_mobile_secret(&conn, id).apply_to(&mut creds);
                    Some(account_needs_reconnect(&creds))
                })
                .unwrap_or(true);
            if let Some(obj) = account.as_object_mut() {
                obj.insert("needs_reconnect".to_string(), json!(needs_reconnect));
            }
        }
        Ok(json!({ "accounts": accounts }))
    })
}

pub(crate) fn req_account_pref_id(params: &Value) -> Result<String, String> {
    req_str(params, "account")
        .or_else(|_| req_str(params, "account_id"))
        .or_else(|_| req_str(params, "id"))
}

pub(crate) fn remove_mobile_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    with_mobile_db(data_dir, |conn| {
        store::delete_account(&conn, &id).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn autodiscover_mobile_account(params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?.trim().to_string();
    let Some((_, domain)) = email.rsplit_once('@') else {
        return Err("invalid email".to_string());
    };
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return Err("invalid email".to_string());
    }
    let (provider, imap_host, imap_port, smtp_host, smtp_port, source) =
        mobile_provider_mail_settings(&domain).unwrap_or_else(|| {
            (
                String::new(),
                format!("imap.{domain}"),
                993,
                format!("smtp.{domain}"),
                465,
                "guess".to_string(),
            )
        });
    let mut result = json!({
        "imap_host": imap_host,
        "imap_port": imap_port,
        "smtp_host": smtp_host,
        "smtp_port": smtp_port,
        "username": email,
        "source": source,
    });
    if !provider.is_empty() {
        result["provider_name"] = json!(provider);
    }
    if let Some((provider, url)) =
        mobile_app_password_hint(&domain, result["imap_host"].as_str().unwrap_or_default())
    {
        result["app_password_hint"] = json!({ "provider": provider, "url": url });
    }
    Ok(result)
}

pub(crate) fn mobile_provider_mail_settings(
    domain: &str,
) -> Option<(String, String, u16, String, u16, String)> {
    let settings = match domain {
        "gmail.com" | "googlemail.com" => ("Gmail", "imap.gmail.com", 993, "smtp.gmail.com", 465),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => (
            "Outlook",
            "outlook.office365.com",
            993,
            "smtp.office365.com",
            587,
        ),
        "icloud.com" | "me.com" | "mac.com" => {
            ("iCloud", "imap.mail.me.com", 993, "smtp.mail.me.com", 587)
        }
        "yahoo.com" | "ymail.com" | "rocketmail.com" => (
            "Yahoo",
            "imap.mail.yahoo.com",
            993,
            "smtp.mail.yahoo.com",
            465,
        ),
        "aol.com" => ("AOL", "imap.aol.com", 993, "smtp.aol.com", 465),
        "fastmail.com" | "fastmail.fm" => (
            "Fastmail",
            "imap.fastmail.com",
            993,
            "smtp.fastmail.com",
            465,
        ),
        "proton.me" | "protonmail.com" => {
            ("Proton Mail Bridge", "127.0.0.1", 1143, "127.0.0.1", 1025)
        }
        _ => return None,
    };
    Some((
        settings.0.to_string(),
        settings.1.to_string(),
        settings.2,
        settings.3.to_string(),
        settings.4,
        "known".to_string(),
    ))
}

pub(crate) fn mobile_app_password_hint(
    domain: &str,
    imap_host: &str,
) -> Option<(&'static str, &'static str)> {
    let host = imap_host.to_ascii_lowercase();
    let contains = |needle: &str| host.contains(needle) || domain.contains(needle);
    if contains("mail.me.com")
        || contains("icloud.com")
        || contains("me.com")
        || contains("mac.com")
    {
        Some(("iCloud", "https://account.apple.com/account/manage"))
    } else if contains("fastmail") {
        Some((
            "Fastmail",
            "https://app.fastmail.com/settings/security/apppassword",
        ))
    } else if contains("yahoo") || contains("ymail") || contains("rocketmail") {
        Some((
            "Yahoo",
            "https://login.yahoo.com/account/security/app-passwords",
        ))
    } else if contains("aol.com") {
        Some((
            "AOL",
            "https://login.aol.com/account/security/app-passwords",
        ))
    } else {
        None
    }
}

pub(crate) fn reorder_mobile_accounts(data_dir: &str, params: &Value) -> Result<Value, String> {
    let ids = req_str_array(params, "accounts")?;
    with_mobile_db(data_dir, |conn| {
        store::reorder_accounts(&conn, &ids).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn suggest_mobile_contacts(data_dir: &str, params: &Value) -> Result<Value, String> {
    let mut account = opt_str(params, "account");
    if account.is_empty() {
        account = opt_str(params, "account_id");
    }
    if account.is_empty() {
        account = opt_str(params, "id");
    }
    let query = opt_str(params, "query");
    let limit = opt_u32(params, "limit").unwrap_or(8);
    with_mobile_db(data_dir, |conn| {
        let contacts = store::suggest_contacts(&conn, &account, &query, limit)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "contacts": contacts }))
    })
}

pub(crate) fn set_mobile_account_name(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let name = opt_str(params, "name");
    with_mobile_db(data_dir, |conn| {
        conn.execute(
            "UPDATE accounts SET display_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
            rusqlite::params![name, id],
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn set_mobile_account_sender_name(
    data_dir: &str,
    params: &Value,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let name = opt_str(params, "name");
    with_mobile_db(data_dir, |conn| {
        conn.execute(
            "UPDATE accounts SET sender_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
            rusqlite::params![name, id],
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn set_mobile_account_avatar(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let avatar_url = opt_str(params, "avatar_url");
    with_mobile_db(data_dir, |conn| {
        conn.execute(
            "UPDATE accounts SET avatar_url = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
            rusqlite::params![avatar_url, id],
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn set_mobile_account_chat_wallpaper(
    data_dir: &str,
    params: &Value,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let wallpaper = match params.get("wallpaper") {
        Some(Value::Null) | None => None,
        Some(value) => {
            let obj = value
                .as_object()
                .ok_or_else(|| "wallpaper must be an object".to_string())?;
            match obj.get("kind").and_then(Value::as_str).unwrap_or_default() {
                "preset" => {
                    let preset_id = obj
                        .get("presetId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if preset_id.is_empty() {
                        return Err("preset wallpaper requires presetId".to_string());
                    }
                    Some(json!({ "kind": "preset", "presetId": preset_id }))
                }
                "custom" => {
                    let url = obj
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if !url.starts_with("/media/wallpapers/") {
                        return Err("custom wallpaper URL must be a Meron wallpaper".to_string());
                    }
                    Some(json!({ "kind": "custom", "url": url }))
                }
                _ => return Err("unknown wallpaper kind".to_string()),
            }
        }
    };
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref_json(&conn, &id, "chat_wallpaper", wallpaper)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn set_mobile_account_images(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let enabled = req_bool(params, "enabled")?;
    with_mobile_db(data_dir, |conn| {
        store::set_load_remote_images(&conn, &id, enabled).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn set_mobile_account_bool_pref(
    data_dir: &str,
    params: &Value,
    key: &str,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let enabled = req_bool(params, "enabled")?;
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref(&conn, &id, key, enabled).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn set_mobile_account_rss_sync_interval(
    data_dir: &str,
    params: &Value,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let minutes = params
        .get("minutes")
        .and_then(Value::as_u64)
        .unwrap_or(60)
        .clamp(5, 1440);
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref_u64(&conn, &id, "rss_sync_interval_minutes", minutes)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "minutes": minutes }))
    })
}

pub(crate) fn set_mobile_account_aliases(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let mut aliases: Vec<store::Alias> = params
        .get("aliases")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| "aliases must be an array".to_string())?
        .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    aliases.retain_mut(|alias| {
        alias.email = alias.email.trim().to_string();
        alias.name = alias.name.trim().to_string();
        !alias.email.is_empty() && seen.insert(alias.email.to_lowercase())
    });
    with_mobile_db(data_dir, |conn| {
        store::set_account_aliases(&conn, &id, &aliases).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "aliases": aliases.len() }))
    })
}

pub(crate) fn add_mobile_password_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?;
    if !email.contains('@') {
        return Err("invalid email".to_string());
    }
    let imap_host = req_str(params, "imap_host")?;
    let smtp_host = req_str(params, "smtp_host")?;
    if imap_host.trim().is_empty() || smtp_host.trim().is_empty() {
        return Err("server required".to_string());
    }
    let username = req_str(params, "username")?;
    if username.trim().is_empty() {
        return Err("username required".to_string());
    }

    let imap_port = req_u16(params, "imap_port").unwrap_or(993);
    let smtp_port = req_u16(params, "smtp_port").unwrap_or(465);
    let tls = params.get("tls").and_then(Value::as_bool).unwrap_or(true);
    let (imap_tls, imap_starttls) = tls_mode(tls, imap_port);
    let (smtp_tls, smtp_starttls) = tls_mode(tls, smtp_port);
    let id = account_id(&email);
    let password = opt_str(params, "password");
    let creds = Creds {
        host: imap_host,
        port: imap_port,
        user: username,
        password,
        tls: imap_tls,
        starttls: imap_starttls,
        smtp_host,
        smtp_port,
        smtp_tls,
        smtp_starttls,
        auth_type: "password".to_string(),
        access_token: None,
        refresh_token: None,
        token_expires_at: 0,
    };
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: email.clone(),
        display_name: opt_str(params, "display_name"),
        avatar_url: String::new(),
        sender_name: opt_str(params, "sender_name"),
    };

    with_mobile_db(data_dir, |conn| {
        store::upsert_account(&conn, &id, &meta, &creds).map_err(|err| err.to_string())?;
        store_mobile_secret(&conn, &id, &creds)?;
        let mut account = store::list_accounts(&conn)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .ok_or_else(|| "account not found after save".to_string())?;
        if let Some(obj) = account.as_object_mut() {
            obj.insert(
                "needs_reconnect".to_string(),
                json!(account_needs_reconnect(&creds)),
            );
        }
        Ok(json!({ "account": account }))
    })
}
