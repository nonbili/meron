use super::*;

const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
const OUTLOOK_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const OUTLOOK_SCOPES: &str = "offline_access openid email profile https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send";

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
    };
    let id = account_id(&email);
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider,
        email: email.clone(),
        display_name: opt_str(params, "display_name"),
        avatar_url: opt_str(params, "avatar_url"),
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

    with_mobile_db(data_dir, |conn| {
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
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthCodeTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    id_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
}

pub(crate) fn exchange_mobile_oauth_code(data_dir: &str, params: &Value) -> Result<Value, String> {
    let requested_email = opt_str(params, "email");
    let provider = req_str(params, "provider")?.to_lowercase();
    let code = req_str(params, "code")?;
    let client_id = req_str(params, "client_id")?;
    let redirect_uri = opt_str(params, "redirect_uri");
    let code_verifier = opt_str(params, "code_verifier");
    let client_secret = opt_str(params, "client_secret");
    let token_url = params
        .get("token_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| match provider.as_str() {
            "gmail" => GOOGLE_TOKEN_URL.to_string(),
            "outlook" => OUTLOOK_TOKEN_URL.to_string(),
            _ => String::new(),
        });
    if token_url.is_empty() {
        return Err("unsupported oauth provider".to_string());
    }

    let token = exchange_oauth_code_blocking(
        &token_url,
        &provider,
        &code,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code_verifier,
    )?;
    let refresh_token = token
        .refresh_token
        .or_else(|| {
            params
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| "oauth response missing refresh_token".to_string())?;
    let token_expires_at = now_epoch_seconds() + token.expires_in.unwrap_or(3600);
    let (claim_email, claim_name) =
        parse_oauth_id_token_claims(token.id_token.as_deref().unwrap_or(""));
    let profile_missing = requested_email.is_empty()
        || opt_str(params, "display_name").is_empty()
        || opt_str(params, "sender_name").is_empty()
        || opt_str(params, "avatar_url").is_empty();
    let google_profile = if provider == "gmail" && profile_missing {
        let userinfo_url = params
            .get("userinfo_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(GOOGLE_USERINFO_URL);
        fetch_google_userinfo_blocking(userinfo_url, &token.access_token).unwrap_or_default()
    } else {
        GoogleUserInfo::default()
    };
    let profile_email = google_profile
        .email
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let profile_name = google_profile
        .name
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let profile_picture = google_profile
        .picture
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let email = if requested_email.contains('@') {
        requested_email.clone()
    } else if profile_email.contains('@') {
        profile_email
    } else {
        claim_email
    };
    if !email.contains('@') {
        return Err("invalid email".to_string());
    }

    let mut add_params = params.clone();
    let display_name_missing = opt_str(&add_params, "display_name").is_empty();
    let sender_name_missing = opt_str(&add_params, "sender_name").is_empty();
    let obj = add_params
        .as_object_mut()
        .ok_or_else(|| "params must be an object".to_string())?;
    obj.insert("email".to_string(), Value::String(email.clone()));
    obj.insert("provider".to_string(), Value::String(provider.clone()));
    if display_name_missing {
        obj.insert(
            "display_name".to_string(),
            Value::String(if !profile_name.is_empty() {
                profile_name.clone()
            } else if !claim_name.is_empty() {
                claim_name.clone()
            } else {
                email.clone()
            }),
        );
    }
    if sender_name_missing {
        let sender_name = if !profile_name.is_empty() {
            profile_name
        } else {
            claim_name
        };
        if !sender_name.is_empty() {
            obj.insert("sender_name".to_string(), Value::String(sender_name));
        }
    }
    if opt_str(params, "avatar_url").is_empty() && !profile_picture.is_empty() {
        obj.insert("avatar_url".to_string(), Value::String(profile_picture));
    }
    obj.insert(
        "access_token".to_string(),
        Value::String(token.access_token),
    );
    obj.insert("refresh_token".to_string(), Value::String(refresh_token));
    obj.insert("token_expires_at".to_string(), json!(token_expires_at));
    add_mobile_oauth_account(data_dir, &add_params)
}

fn fetch_google_userinfo_blocking(
    userinfo_url: &str,
    access_token: &str,
) -> Result<GoogleUserInfo, String> {
    if access_token.trim().is_empty() {
        return Ok(GoogleUserInfo::default());
    }
    let mut resp = ureq::get(userinfo_url)
        .header("Authorization", &format!("Bearer {access_token}"))
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(8)))
        .build()
        .call()
        .map_err(|err| format!("Google userinfo request: {err:#}"))?;
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("read Google userinfo response: {err:#}"))?;
    if !status.is_success() {
        return Err(format!("Google userinfo failed ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(|err| format!("parse Google userinfo response: {err}"))
}

fn parse_oauth_id_token_claims(id_token: &str) -> (String, String) {
    let Some(payload) = id_token.split('.').nth(1) else {
        return (String::new(), String::new());
    };
    let decoded = match base64_url_decode(payload) {
        Ok(bytes) => bytes,
        Err(_) => return (String::new(), String::new()),
    };
    let claims: Value = match serde_json::from_slice(&decoded) {
        Ok(value) => value,
        Err(_) => return (String::new(), String::new()),
    };
    let email = claims
        .get("email")
        .and_then(Value::as_str)
        .or_else(|| claims.get("preferred_username").and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string();
    let name = claims
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let name = if name.is_empty() {
        let given_name = claims
            .get("given_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let family_name = claims
            .get("family_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        [given_name, family_name]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        name
    };
    (email, name)
}

fn base64_url_decode(value: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;

    let mut padded = value.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    base64::engine::general_purpose::URL_SAFE.decode(padded)
}

pub(crate) fn exchange_oauth_code_blocking(
    token_url: &str,
    provider: &str,
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<OAuthCodeTokenResponse, String> {
    let mut form: Vec<(&str, &str)> = vec![
        ("client_id", client_id),
        ("code", code),
        ("grant_type", "authorization_code"),
    ];
    if !redirect_uri.is_empty() {
        form.push(("redirect_uri", redirect_uri));
    }
    if !code_verifier.is_empty() {
        form.push(("code_verifier", code_verifier));
    }
    if !client_secret.is_empty() {
        form.push(("client_secret", client_secret));
    }
    if provider == "outlook" {
        form.push(("scope", OUTLOOK_SCOPES));
    }

    let mut resp = ureq::post(token_url)
        .config()
        .http_status_as_error(false)
        .build()
        .send_form(form)
        .map_err(|err| format!("oauth code exchange request: {err:#}"))?;
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("read oauth code exchange response: {err:#}"))?;
    if !status.is_success() {
        return Err(format!("oauth code exchange failed ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(|err| format!("parse oauth code exchange response: {err}"))
}
