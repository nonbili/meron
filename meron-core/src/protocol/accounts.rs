use super::*;
use hickory_resolver::Resolver;
use hickory_resolver::proto::rr::RData;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::time::Duration;
use url::form_urlencoded;

#[derive(Clone, Debug, Default)]
struct DiscoveredMailSettings {
    imap_host: String,
    imap_port: u16,
    smtp_host: String,
    smtp_port: u16,
    username: String,
    provider_name: String,
    source: String,
}

#[derive(Default)]
struct AutoconfigServer {
    server_type: String,
    hostname: String,
    port: u16,
    socket_type: String,
    username: String,
}

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

    let discovered = mobile_provider_mail_settings(&email, &domain)
        .or_else(|| discover_mobile_autoconfig(&email, &domain))
        .or_else(|| discover_mobile_srv(&email, &domain))
        .unwrap_or_else(|| DiscoveredMailSettings {
            imap_host: format!("imap.{domain}"),
            imap_port: 993,
            smtp_host: format!("smtp.{domain}"),
            smtp_port: 465,
            username: email.clone(),
            source: "guess".to_string(),
            ..Default::default()
        });
    let mut result = json!({
        "imap_host": discovered.imap_host,
        "imap_port": discovered.imap_port,
        "smtp_host": discovered.smtp_host,
        "smtp_port": discovered.smtp_port,
        "username": discovered.username,
        "source": discovered.source,
    });
    if !discovered.provider_name.is_empty() {
        result["provider_name"] = json!(discovered.provider_name);
    }
    if let Some((provider, url)) =
        mobile_app_password_hint(&domain, result["imap_host"].as_str().unwrap_or_default())
    {
        result["app_password_hint"] = json!({ "provider": provider, "url": url });
    }
    Ok(result)
}

fn mobile_provider_mail_settings(email: &str, domain: &str) -> Option<DiscoveredMailSettings> {
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
    Some(DiscoveredMailSettings {
        provider_name: settings.0.to_string(),
        imap_host: settings.1.to_string(),
        imap_port: settings.2,
        smtp_host: settings.3.to_string(),
        smtp_port: settings.4,
        username: email.to_string(),
        source: "known".to_string(),
    })
}

fn discover_mobile_autoconfig(email: &str, domain: &str) -> Option<DiscoveredMailSettings> {
    let encoded_email: String = form_urlencoded::byte_serialize(email.as_bytes()).collect();
    let urls = [
        (
            format!(
                "https://autoconfig.{domain}/mail/config-v1.1.xml?emailaddress={encoded_email}"
            ),
            "autoconfig",
        ),
        (
            format!(
                "https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml?emailaddress={encoded_email}"
            ),
            "autoconfig",
        ),
        (
            format!("https://autoconfig.thunderbird.net/v1.1/{domain}"),
            "thunderbird",
        ),
    ];
    for (url, source) in urls {
        let Some(xml) = fetch_mobile_autoconfig(&url) else {
            continue;
        };
        let Some(mut settings) = parse_mobile_autoconfig(&xml, email) else {
            continue;
        };
        settings.source = source.to_string();
        return Some(settings);
    }
    None
}

fn fetch_mobile_autoconfig(url: &str) -> Option<String> {
    let mut resp = ureq::get(url)
        .header("User-Agent", "Meron-Mail-Autoconfig")
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(6)))
        .build()
        .call()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_string()
        .ok()
}

fn parse_mobile_autoconfig(xml: &str, email: &str) -> Option<DiscoveredMailSettings> {
    let mut reader = Reader::from_str(xml);
    let decoder = reader.decoder();
    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut display_name = String::new();
    let mut incoming: Vec<AutoconfigServer> = Vec::new();
    let mut outgoing: Vec<AutoconfigServer> = Vec::new();
    let mut current: Option<AutoconfigServer> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "incomingServer" || name == "outgoingServer" {
                    let mut server = AutoconfigServer::default();
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"type" {
                            server.server_type =
                                attr.decode_and_unescape_value(decoder).ok()?.to_string();
                        }
                    }
                    current = Some(server);
                }
                path.push(name);
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().ok()?.trim().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }
                match path.last().map(String::as_str) {
                    Some("displayName") if current.is_none() => display_name = text,
                    Some("hostname") => {
                        if let Some(server) = current.as_mut() {
                            server.hostname = text;
                        }
                    }
                    Some("port") => {
                        if let Some(server) = current.as_mut() {
                            server.port = text.parse::<u16>().unwrap_or(0);
                        }
                    }
                    Some("socketType") => {
                        if let Some(server) = current.as_mut() {
                            server.socket_type = text;
                        }
                    }
                    Some("username") => {
                        if let Some(server) = current.as_mut() {
                            server.username = text;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_vec();
                if name.as_slice() == b"incomingServer" {
                    if let Some(server) = current.take() {
                        incoming.push(server);
                    }
                } else if name.as_slice() == b"outgoingServer" {
                    if let Some(server) = current.take() {
                        outgoing.push(server);
                    }
                }
                path.pop();
            }
            Ok(_) => {}
            Err(_) => return None,
        }
        buf.clear();
    }

    let imap = incoming.into_iter().find(|server| {
        server.server_type.eq_ignore_ascii_case("imap") && !server.hostname.is_empty()
    })?;
    let smtp = outgoing.into_iter().find(|server| {
        server.server_type.eq_ignore_ascii_case("smtp") && !server.hostname.is_empty()
    })?;
    Some(DiscoveredMailSettings {
        imap_host: imap.hostname,
        imap_port: port_for_mobile_autoconfig(imap.port, &imap.socket_type, true),
        smtp_host: smtp.hostname,
        smtp_port: port_for_mobile_autoconfig(smtp.port, &smtp.socket_type, false),
        username: expand_mobile_autoconfig_username(&imap.username, email)
            .unwrap_or_else(|| email.to_string()),
        provider_name: display_name,
        source: String::new(),
    })
}

fn discover_mobile_srv(email: &str, domain: &str) -> Option<DiscoveredMailSettings> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .ok()?;
    let (imap_host, imap_port) = lookup_mobile_srv(&runtime, "imaps", domain)
        .or_else(|| lookup_mobile_srv(&runtime, "imap", domain))?;
    let (smtp_host, smtp_port) = lookup_mobile_srv(&runtime, "submissions", domain)
        .or_else(|| lookup_mobile_srv(&runtime, "submission", domain))?;
    Some(DiscoveredMailSettings {
        imap_host,
        imap_port,
        smtp_host,
        smtp_port,
        username: email.to_string(),
        source: "srv".to_string(),
        ..Default::default()
    })
}

fn lookup_mobile_srv(
    runtime: &tokio::runtime::Runtime,
    service: &str,
    domain: &str,
) -> Option<(String, u16)> {
    runtime.block_on(async {
        let resolver = Resolver::builder_tokio().ok()?.build().ok()?;
        let name = format!("_{service}._tcp.{domain}.");
        let response = tokio::time::timeout(Duration::from_secs(5), resolver.srv_lookup(name))
            .await
            .ok()?
            .ok()?;
        let mut records = response
            .answers()
            .iter()
            .filter_map(|record| match &record.data {
                RData::SRV(srv) => Some(srv),
                _ => None,
            })
            .collect::<Vec<_>>();
        records.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| b.weight.cmp(&a.weight))
        });
        let best = records.first()?;
        let target = best.target.to_utf8().trim_end_matches('.').to_string();
        if target.is_empty() || target == "." {
            return None;
        }
        Some((target, best.port))
    })
}

fn port_for_mobile_autoconfig(port: u16, socket_type: &str, imap: bool) -> u16 {
    if port != 0 {
        return port;
    }
    match socket_type.to_ascii_uppercase().as_str() {
        "STARTTLS" => {
            if imap {
                143
            } else {
                587
            }
        }
        "PLAIN" => {
            if imap {
                143
            } else {
                25
            }
        }
        _ => {
            if imap {
                993
            } else {
                465
            }
        }
    }
}

fn expand_mobile_autoconfig_username(template: &str, email: &str) -> Option<String> {
    if template.is_empty() {
        return None;
    }
    let local = email
        .split_once('@')
        .map(|(local, _)| local)
        .unwrap_or(email);
    Some(
        template
            .replace("%EMAILADDRESS%", email)
            .replace("%EMAILLOCALPART%", local),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_autoconfig_parser_expands_username_and_socket_defaults() {
        let xml = r#"<?xml version="1.0"?>
<clientConfig version="1.1">
  <emailProvider id="example.com">
    <displayName>Example Mail</displayName>
    <incomingServer type="imap">
      <hostname>mail.example.com</hostname>
      <socketType>STARTTLS</socketType>
      <username>%EMAILLOCALPART%</username>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.com</hostname>
      <socketType>STARTTLS</socketType>
      <username>%EMAILADDRESS%</username>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

        let settings = parse_mobile_autoconfig(xml, "alice@example.com").unwrap();
        assert_eq!(settings.provider_name, "Example Mail");
        assert_eq!(settings.imap_host, "mail.example.com");
        assert_eq!(settings.imap_port, 143);
        assert_eq!(settings.smtp_host, "smtp.example.com");
        assert_eq!(settings.smtp_port, 587);
        assert_eq!(settings.username, "alice");
    }

    #[test]
    fn mobile_autoconfig_parser_requires_imap_and_smtp() {
        let xml = r#"<clientConfig><emailProvider><incomingServer type="imap"><hostname>imap.example.com</hostname></incomingServer></emailProvider></clientConfig>"#;
        assert!(parse_mobile_autoconfig(xml, "alice@example.com").is_none());
    }
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

pub(crate) fn set_mobile_account_save_sent_copy(
    data_dir: &str,
    params: &Value,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let value = match params.get("value") {
        Some(Value::Bool(enabled)) => Some(json!(enabled)),
        Some(Value::Null) | None => None,
        _ => return Err("value must be true, false, or null".to_string()),
    };
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref_json(&conn, &id, "save_sent_copy", value)
            .map_err(|err| err.to_string())?;
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
        oauth_client_id: String::new(),
        oauth_client_secret: String::new(),
        oauth_token_url: String::new(),
        oauth_scope: String::new(),
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
