use super::*;

const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const OUTLOOK_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const OUTLOOK_SCOPES: &str = "offline_access openid email https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send";

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
    let refresh_token = req_str(params, "refresh_token")?;
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
        refresh_token: Some(refresh_token),
        token_expires_at,
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

#[derive(Debug, Deserialize)]
pub(crate) struct OAuthCodeTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

pub(crate) fn exchange_mobile_oauth_code(data_dir: &str, params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?;
    let provider = req_str(params, "provider")?.to_lowercase();
    let code = req_str(params, "code")?;
    let client_id = req_str(params, "client_id")?;
    let redirect_uri = req_str(params, "redirect_uri")?;
    let code_verifier = req_str(params, "code_verifier")?;
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

    let mut add_params = params.clone();
    let obj = add_params
        .as_object_mut()
        .ok_or_else(|| "params must be an object".to_string())?;
    obj.insert("email".to_string(), Value::String(email));
    obj.insert("provider".to_string(), Value::String(provider));
    obj.insert(
        "access_token".to_string(),
        Value::String(token.access_token),
    );
    obj.insert("refresh_token".to_string(), Value::String(refresh_token));
    obj.insert("token_expires_at".to_string(), json!(token_expires_at));
    add_mobile_oauth_account(data_dir, &add_params)
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
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
    ];
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
