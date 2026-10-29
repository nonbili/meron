use super::*;

use crate::oauth;

pub(crate) fn add_mobile_oauth_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?;
    if !email.contains('@') {
        return Err("invalid email".to_string());
    }
    let provider = req_str(params, "provider")?.to_lowercase();
    let (auth_type, default_imap_host, default_smtp_host, default_smtp_port) =
        match provider.as_str() {
            "gmail" => ("gmail_oauth", "imap.gmail.com", "smtp.gmail.com", 587),
            "outlook" => (
                "outlook_oauth",
                "outlook.office365.com",
                "smtp.office365.com",
                587,
            ),
            _ => return Err("unsupported oauth provider".to_string()),
        };
    let username = opt_str(params, "username");
    let access_token = opt_str(params, "access_token");
    // `refresh_token` is optional: platform-managed accounts (e.g. Android
    // Gmail via AccountManager) have no refresh token because the OS re-mints
    // short-lived access tokens and pushes them in via `account.updateOAuthToken`.
    let refresh_token = opt_str(params, "refresh_token");
    let oauth_client_id = opt_str(params, "client_id");
    let oauth_client_secret = opt_str(params, "client_secret");
    let oauth_token_url = opt_str(params, "token_url");
    let oauth_scope = opt_str(params, "scope");
    let imap_host = params
        .get("imap_host")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_imap_host)
        .to_string();
    let smtp_host = params
        .get("smtp_host")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_smtp_host)
        .to_string();
    let imap_port = req_u16(params, "imap_port").unwrap_or(993);
    let smtp_port = req_u16(params, "smtp_port").unwrap_or(default_smtp_port);
    let token_expires_at = params
        .get("token_expires_at")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let creds = Creds {
        host: imap_host,
        port: imap_port,
        user: if username.is_empty() {
            email.clone()
        } else {
            username
        },
        password: String::new(),
        tls: true,
        starttls: false,
        smtp_host,
        smtp_port,
        smtp_tls: false,
        smtp_starttls: true,
        auth_type: auth_type.to_string(),
        access_token: if access_token.is_empty() {
            None
        } else {
            Some(access_token)
        },
        refresh_token: if refresh_token.is_empty() {
            None
        } else {
            Some(refresh_token)
        },
        token_expires_at,
        oauth_client_id,
        oauth_client_secret,
        oauth_token_url,
        oauth_scope,
        proxy: crate::proxy::ProxyChoice::from_json(params.get("proxy").unwrap_or(&Value::Null)),
        cert_pin: None,
        smtp_cert_pin: None,
        rejected_grant: None,
    };
    let id = account_id(&email);
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider,
        email: email.clone(),
        display_name: opt_str(params, "display_name"),
        // An avatar already set, possibly chosen by the user, outlives a
        // reconnect: an empty one here keeps it.
        avatar_url: if stored_avatar_url(data_dir, &id).is_empty() {
            opt_str(params, "avatar_url")
        } else {
            String::new()
        },
        sender_name: opt_str(params, "sender_name"),
    };

    let result = with_mobile_db(data_dir, |conn| {
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
    })?;
    crate::ffi::evict_engine_account(&id);
    Ok(result)
}

/// Refresh the stored access token for a platform-managed OAuth account.
///
/// Used by Android Gmail (AccountManager), where the OS — not `meron-core` —
/// holds the long-lived credential and mints short-lived access tokens. The
/// platform calls this right before a sync to push the freshly minted token.
pub(crate) fn update_mobile_oauth_token(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_id(params)?;
    let access_token = req_str(params, "access_token")?;
    if access_token.trim().is_empty() {
        return Err("access_token is required".to_string());
    }
    let token_expires_at = params
        .get("token_expires_at")
        .and_then(Value::as_i64)
        .unwrap_or_else(|| now_epoch_seconds() + 3600);

    let result = with_mobile_db(data_dir, |conn| {
        let mut creds = load_mobile_account_creds(&conn, &id)?;
        if !creds.is_oauth() {
            return Err(format!("account is not oauth: {id}"));
        }
        creds.access_token = Some(access_token.clone());
        // This command is used by platform-managed OAuth hosts such as Android
        // AccountManager. Once the host owns refresh, any old browser-flow
        // refresh token must not be reused by core.
        creds.refresh_token = None;
        creds.token_expires_at = token_expires_at;
        creds.rejected_grant = None;
        store::save_account_config(&conn, &id, &creds).map_err(|err| err.to_string())?;
        store_mobile_secret(&conn, &id, &creds)?;
        let mut account = store::list_accounts(&conn)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .ok_or_else(|| "account not found after update".to_string())?;
        if let Some(obj) = account.as_object_mut() {
            obj.insert(
                "needs_reconnect".to_string(),
                json!(account_needs_reconnect(&creds)),
            );
        }
        Ok(json!({ "ok": true, "account": account }))
    })?;
    crate::ffi::evict_engine_account(&id);
    Ok(result)
}

/// `{provider, client_id, redirect_uri, state, code_challenge, login_hint}` →
/// `{url}`, the authorization URL the host opens.
pub(crate) fn mobile_oauth_authorize_url(params: &Value) -> Result<Value, String> {
    let client = oauth::Client {
        provider: req_str(params, "provider")?.to_lowercase(),
        client_id: req_str(params, "client_id")?,
        ..oauth::Client::default()
    };
    let url = oauth::authorize_url(
        &client,
        &req_str(params, "redirect_uri")?,
        &req_str(params, "state")?,
        &req_str(params, "code_challenge")?,
        &opt_str(params, "login_hint"),
    )?;
    Ok(json!({ "url": url }))
}

pub(crate) fn exchange_mobile_oauth_code(data_dir: &str, params: &Value) -> Result<Value, String> {
    let requested_email = opt_str(params, "email");
    let provider = req_str(params, "provider")?.to_lowercase();
    if provider != "gmail" && provider != "outlook" {
        return Err("unsupported oauth provider".to_string());
    }
    let code = req_str(params, "code")?;
    let client = oauth::Client {
        provider: provider.clone(),
        client_id: req_str(params, "client_id")?,
        client_secret: opt_str(params, "client_secret"),
        token_url: opt_str(params, "token_url"),
        userinfo_url: opt_str(params, "userinfo_url"),
        photo_url: opt_str(params, "photo_url"),
    };

    let token = oauth::exchange_code(
        &client,
        &code,
        &opt_str(params, "redirect_uri"),
        &opt_str(params, "code_verifier"),
    )?;
    let refresh_token = token
        .refresh_token
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some(opt_str(params, "refresh_token")).filter(|value| !value.is_empty()))
        .ok_or_else(|| "oauth response missing refresh_token".to_string())?;
    let token_expires_at = now_epoch_seconds() + token.expires_in.unwrap_or(3600);
    let (claim_email, claim_name) =
        oauth::parse_id_token_claims(token.id_token.as_deref().unwrap_or(""));
    let profile_missing = requested_email.is_empty()
        || opt_str(params, "display_name").is_empty()
        || opt_str(params, "sender_name").is_empty()
        || opt_str(params, "avatar_url").is_empty();
    let profile = if provider == "gmail" && profile_missing {
        oauth::fetch_profile(&client, &token).unwrap_or_default()
    } else {
        oauth::Profile::default()
    };
    let email = if requested_email.contains('@') {
        requested_email.clone()
    } else if profile.email.contains('@') {
        profile.email.clone()
    } else {
        claim_email
    };
    if !email.contains('@') {
        return Err("invalid email".to_string());
    }
    let name = if !profile.name.is_empty() {
        profile.name.clone()
    } else {
        claim_name
    };

    let mut add_params = params.clone();
    let obj = add_params
        .as_object_mut()
        .ok_or_else(|| "params must be an object".to_string())?;
    obj.insert("email".to_string(), Value::String(email.clone()));
    obj.insert("provider".to_string(), Value::String(provider.clone()));
    if opt_str(params, "display_name").is_empty() {
        let display_name = if name.is_empty() {
            email.clone()
        } else {
            name.clone()
        };
        obj.insert("display_name".to_string(), Value::String(display_name));
    }
    if opt_str(params, "sender_name").is_empty() && !name.is_empty() {
        obj.insert("sender_name".to_string(), Value::String(name));
    }
    let id = account_id(&email);
    if opt_str(params, "avatar_url").is_empty() && stored_avatar_url(data_dir, &id).is_empty() {
        // The provider's photo, stored locally. Should the download fail,
        // Google's picture is still usable remotely; no photo leaves the initials.
        let avatar_url = oauth::fetch_photo(
            &client,
            &token.access_token,
            &refresh_token,
            &profile.picture,
        )
        .and_then(|photo| {
            store_mobile_account_media(data_dir, &id, "avatars", photo.ext, &photo.bytes)
        })
        .unwrap_or(profile.picture);
        if !avatar_url.is_empty() {
            obj.insert("avatar_url".to_string(), Value::String(avatar_url));
        }
    }
    obj.insert(
        "access_token".to_string(),
        Value::String(token.access_token),
    );
    obj.insert("refresh_token".to_string(), Value::String(refresh_token));
    obj.insert("token_expires_at".to_string(), json!(token_expires_at));
    add_mobile_oauth_account(data_dir, &add_params)
}

/// The avatar an account already has, or "" for none (or no account yet).
fn stored_avatar_url(data_dir: &str, id: &str) -> String {
    with_mobile_db(data_dir, |conn| {
        Ok(conn
            .query_row(
                "SELECT avatar_url FROM accounts WHERE id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .map(Value::String)
            .unwrap_or(Value::Null))
    })
    .ok()
    .and_then(|value| value.as_str().map(str::to_string))
    .unwrap_or_default()
}
