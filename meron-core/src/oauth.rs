//! OAuth sign-in shared by desktop (sidecar `oauth.*` commands) and mobile
//! (`oauth.authorizeUrl`, `account.exchangeOAuthCode`): the authorization URL,
//! code exchange, the account profile, and its photo. The hosts keep the
//! browser half (opening it, the redirect, PKCE verifiers).

use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

pub const GOOGLE_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
pub const GOOGLE_SCOPES: &str = "openid email profile https://mail.google.com/";
pub const OUTLOOK_AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
pub const OUTLOOK_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
/// Microsoft binds a token to one resource, so these IMAP/SMTP scopes are
/// requested on code exchange and refresh.
pub const OUTLOOK_SCOPES: &str = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid email profile";
/// Consented at authorize time alongside OUTLOOK_SCOPES, but redeemed from the
/// refresh token as a token of its own (see `fetch_photo`).
pub const OUTLOOK_PHOTO_SCOPE: &str = "https://graph.microsoft.com/User.Read";
pub const OUTLOOK_PHOTO_URL: &str = "https://graph.microsoft.com/v1.0/me/photo/$value";

const PHOTO_MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Default, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    pub id_token: Option<String>,
}

/// The client a code exchange or refresh speaks for, and its endpoints. An
/// empty URL means the provider's default.
#[derive(Debug, Default, Clone)]
pub struct Client {
    pub provider: String,
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub photo_url: String,
}

impl Client {
    pub fn token_url(&self) -> &str {
        match (self.token_url.trim(), self.provider.as_str()) {
            ("", "outlook") => OUTLOOK_TOKEN_URL,
            ("", _) => GOOGLE_TOKEN_URL,
            (url, _) => url,
        }
    }

    fn userinfo_url(&self) -> &str {
        match self.userinfo_url.trim() {
            "" => GOOGLE_USERINFO_URL,
            url => url,
        }
    }

    fn photo_url(&self) -> &str {
        match self.photo_url.trim() {
            "" => OUTLOOK_PHOTO_URL,
            url => url,
        }
    }
}

#[derive(Debug, Default)]
pub struct Profile {
    pub email: String,
    pub name: String,
    /// Google's picture URL; Microsoft has none (see `fetch_photo`).
    pub picture: String,
}

pub struct Photo {
    pub ext: &'static str,
    pub bytes: Vec<u8>,
}

/// The provider's authorization URL for a PKCE (S256) sign-in. Microsoft is
/// asked for the photo scope too, so one consent covers both resources.
pub fn authorize_url(
    client: &Client,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
    login_hint: &str,
) -> Result<String, String> {
    let outlook_scopes = format!("{OUTLOOK_SCOPES} {OUTLOOK_PHOTO_SCOPE}");
    let (base, scope) = match client.provider.as_str() {
        "gmail" => (GOOGLE_AUTHORIZE_URL, GOOGLE_SCOPES),
        "outlook" => (OUTLOOK_AUTHORIZE_URL, outlook_scopes.as_str()),
        _ => return Err("unsupported oauth provider".to_string()),
    };
    let mut url = url::Url::parse(base).map_err(|err| err.to_string())?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("client_id", &client.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", scope)
            .append_pair("state", state)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256");
        if client.provider == "gmail" {
            // A refresh token every time, even for an account consented before.
            query
                .append_pair("access_type", "offline")
                .append_pair("prompt", "consent");
        }
        if !login_hint.is_empty() {
            query.append_pair("login_hint", login_hint);
        }
    }
    Ok(url.into())
}

/// Swap an authorization code for tokens.
pub fn exchange_code(
    client: &Client,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<Tokens, String> {
    let mut form: Vec<(&str, &str)> = vec![
        ("client_id", &client.client_id),
        ("code", code),
        ("grant_type", "authorization_code"),
    ];
    if !redirect_uri.is_empty() {
        form.push(("redirect_uri", redirect_uri));
    }
    if !code_verifier.is_empty() {
        form.push(("code_verifier", code_verifier));
    }
    if !client.client_secret.is_empty() {
        form.push(("client_secret", &client.client_secret));
    }
    if client.provider == "outlook" {
        form.push(("scope", OUTLOOK_SCOPES));
    }
    post_token(client.token_url(), form, "oauth code exchange")
}

/// The account's email and name: Google's userinfo endpoint, or the claims of
/// Microsoft's id_token (Graph is a resource the IMAP/SMTP token can't address).
pub fn fetch_profile(client: &Client, tokens: &Tokens) -> Result<Profile, String> {
    if client.provider == "outlook" {
        let (email, name) = parse_id_token_claims(tokens.id_token.as_deref().unwrap_or(""));
        return Ok(Profile {
            email,
            name,
            picture: String::new(),
        });
    }
    #[derive(Deserialize)]
    struct UserInfo {
        email: Option<String>,
        name: Option<String>,
        picture: Option<String>,
    }
    let mut resp = agent()?
        .get(client.userinfo_url())
        .header("Authorization", &format!("Bearer {}", tokens.access_token))
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(8)))
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
    let info: UserInfo = serde_json::from_str(&body)
        .map_err(|err| format!("parse Google userinfo response: {err}"))?;
    let trimmed = |value: Option<String>| value.unwrap_or_default().trim().to_string();
    Ok(Profile {
        email: trimmed(info.email),
        name: trimmed(info.name),
        picture: trimmed(info.picture),
    })
}

/// Download the account's profile photo. Google's is the userinfo picture
/// (`picture` when the caller already has it); Microsoft's comes from Graph,
/// with a token redeemed from the refresh token. Fails when there is none
/// (Graph answers 404) or the grant predates the photo scope.
pub fn fetch_photo(
    client: &Client,
    access_token: &str,
    refresh_token: &str,
    picture: &str,
) -> Result<Photo, String> {
    if client.provider != "outlook" {
        let picture = if picture.is_empty() {
            let tokens = Tokens {
                access_token: access_token.to_string(),
                ..Tokens::default()
            };
            fetch_profile(client, &tokens)?.picture
        } else {
            picture.to_string()
        };
        if picture.is_empty() {
            return Err("Google profile has no picture".to_string());
        }
        return download_photo(&picture, None);
    }
    let tokens = post_token(
        client.token_url(),
        vec![
            ("client_id", &client.client_id),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
            ("scope", OUTLOOK_PHOTO_SCOPE),
        ],
        "Graph token",
    )?;
    download_photo(client.photo_url(), Some(&tokens.access_token))
}

/// Decode the (unverified) payload of an OIDC id_token for the account email
/// and name. The token came over TLS straight from the token endpoint, so its
/// claims are read without re-verifying the signature. Empty on any failure.
pub fn parse_id_token_claims(id_token: &str) -> (String, String) {
    use base64::Engine;

    let Some(claims) = id_token
        .split('.')
        .nth(1)
        .and_then(|payload| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(payload.trim_end_matches('='))
                .ok()
        })
        .and_then(|decoded| serde_json::from_slice::<Value>(&decoded).ok())
    else {
        return (String::new(), String::new());
    };
    let claim = |key: &str| {
        claims
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let email = Some(claim("email"))
        .filter(|email| !email.is_empty())
        .unwrap_or_else(|| claim("preferred_username"));
    let name = Some(claim("name"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            [claim("given_name"), claim("family_name")]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        });
    (email, name)
}

fn agent() -> Result<ureq::Agent, String> {
    crate::proxy::agent().map_err(|err| format!("oauth proxy: {err:#}"))
}

fn post_token(url: &str, form: Vec<(&str, &str)>, what: &str) -> Result<Tokens, String> {
    let mut resp = agent()?
        .post(url)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .send_form(form)
        .map_err(|err| format!("{what} request: {err:#}"))?;
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("read {what} response: {err:#}"))?;
    if !status.is_success() {
        return Err(format!("{what} failed ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(|err| format!("parse {what} response: {err}"))
}

fn download_photo(url: &str, bearer: Option<&str>) -> Result<Photo, String> {
    let mut request = agent()?.get(url);
    if let Some(token) = bearer {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    let mut resp = request
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .call()
        .map_err(|err| format!("photo request: {err:#}"))?;
    if !resp.status().is_success() {
        return Err(format!("photo request failed ({})", resp.status()));
    }
    let mime = resp
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    let ext = crate::protocol::media_extension("", mime.split(';').next().unwrap_or("").trim())?;
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(PHOTO_MAX_BYTES)
        .read_to_vec()
        .map_err(|err| format!("read photo: {err:#}"))?;
    if bytes.is_empty() {
        return Err("photo is empty".to_string());
    }
    Ok(Photo { ext, bytes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    /// Answer each request in turn with `(content type, body)`, handing the
    /// complete requests back for inspection.
    fn serve(
        responses: Vec<(&'static str, Vec<u8>)>,
    ) -> (String, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (content_type, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let request = crate::test_support::read_http_request(&mut stream);
                let _ = tx.send(request);
                let status = if body.is_empty() {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        (base, rx)
    }

    #[test]
    fn authorize_url_carries_pkce_and_provider_scopes() {
        let client = |provider: &str| Client {
            provider: provider.to_string(),
            client_id: "client".to_string(),
            ..Client::default()
        };
        let query = |url: String| {
            url::Url::parse(&url)
                .unwrap()
                .query_pairs()
                .into_owned()
                .collect::<std::collections::HashMap<_, _>>()
        };
        let outlook =
            authorize_url(&client("outlook"), "http://127.0.0.1:1", "st", "ch", "").unwrap();
        assert!(outlook.starts_with(OUTLOOK_AUTHORIZE_URL));
        let outlook = query(outlook);
        assert!(outlook["scope"].contains("IMAP.AccessAsUser.All"));
        assert!(outlook["scope"].contains(OUTLOOK_PHOTO_SCOPE));
        assert_eq!(outlook["code_challenge"], "ch");
        assert_eq!(outlook["code_challenge_method"], "S256");
        assert!(!outlook.contains_key("prompt") && !outlook.contains_key("login_hint"));

        let gmail = query(
            authorize_url(&client("gmail"), "app:/oauth", "st", "ch", "me@gmail.com").unwrap(),
        );
        assert_eq!(gmail["scope"], GOOGLE_SCOPES);
        assert_eq!(gmail["access_type"], "offline");
        assert_eq!(gmail["login_hint"], "me@gmail.com");
        assert_eq!(gmail["redirect_uri"], "app:/oauth");
        assert!(authorize_url(&client("yahoo"), "", "", "", "").is_err());
    }

    #[test]
    fn id_token_claims_fall_back_to_username_and_given_names() {
        use base64::Engine;
        let claims = serde_json::json!({
            "preferred_username": "me@outlook.com",
            "given_name": "Ada",
            "family_name": "Lovelace",
        });
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        assert_eq!(
            parse_id_token_claims(&format!("h.{payload}.s")),
            ("me@outlook.com".to_string(), "Ada Lovelace".to_string())
        );
        assert_eq!(
            parse_id_token_claims("garbage"),
            (String::new(), String::new())
        );
    }

    #[test]
    fn gmail_photo_is_the_userinfo_picture() {
        let picture = b"\x89PNGpicture".to_vec();
        let (pictures, picture_rx) = serve(vec![
            ("image/png", picture.clone()),
            ("image/png", picture.clone()),
        ]);
        let (userinfo, userinfo_rx) = serve(vec![(
            "application/json",
            format!(r#"{{"email":"me@gmail.com","picture":"{pictures}/picture"}}"#).into_bytes(),
        )]);
        let client = Client {
            provider: "gmail".to_string(),
            userinfo_url: format!("{userinfo}/userinfo"),
            ..Client::default()
        };

        // A picture already in hand is downloaded as is.
        let photo = fetch_photo(&client, "access", "", &format!("{pictures}/picture")).unwrap();
        assert_eq!((photo.ext, photo.bytes), ("png", picture.clone()));
        assert!(picture_rx.recv().unwrap().starts_with("GET /picture "));

        // Otherwise userinfo names it.
        let photo = fetch_photo(&client, "access", "", "").unwrap();
        assert_eq!((photo.ext, photo.bytes), ("png", picture));
        assert!(userinfo_rx.recv().unwrap().contains("Bearer access"));
        assert!(picture_rx.recv().unwrap().starts_with("GET /picture "));
    }

    #[test]
    fn outlook_photo_comes_from_graph_with_a_redeemed_token() {
        let (base, rx) = serve(vec![
            ("application/json", br#"{"access_token":"graph"}"#.to_vec()),
            ("image/jpeg", b"\xff\xd8photo".to_vec()),
            ("application/json", br#"{"access_token":"graph"}"#.to_vec()),
            ("image/jpeg", Vec::new()),
        ]);
        let client = Client {
            provider: "outlook".to_string(),
            client_id: "client".to_string(),
            token_url: format!("{base}/token"),
            photo_url: format!("{base}/photo"),
            ..Client::default()
        };
        let photo = fetch_photo(&client, "imap", "refresh", "").unwrap();
        assert_eq!((photo.ext, photo.bytes), ("jpg", b"\xff\xd8photo".to_vec()));
        assert!(rx.recv().unwrap().starts_with("POST /token "));
        assert!(rx.recv().unwrap().contains("Bearer graph"));
        // No photo: Graph answers 404.
        assert!(fetch_photo(&client, "imap", "refresh", "").is_err());
    }
}
