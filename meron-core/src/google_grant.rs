//! One Google authorization per Google account, shared by the services that
//! sync with it (Calendar, Contacts, Tasks). Installed apps get no incremental
//! authorization, so a sign-in asks for every scope a service of that account
//! uses (or is connecting) along with the new one, and replaces the grant only
//! when all of them were allowed. A service whose scope the grant already
//! covers connects without a browser sign-in, including one reconnected while
//! another service keeps the grant alive.
//!
//! The `google_grants` table maps the address to the secret id the refresh
//! token is kept under, and to the scopes Google last reported for it.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::engine::{Engine, default_oauth_client};
use crate::oauth::{
    self, GOOGLE_CALENDAR_SCOPE, GOOGLE_CONTACTS_SCOPE, GOOGLE_TASKS_SCOPE, Tokens,
};
use crate::secrets::Secrets;

#[derive(Clone, Default)]
pub(crate) struct Grant {
    /// The secret id its refresh token is kept under.
    id: String,
    refresh_token: String,
    access_token: String,
    expires_at: i64,
    /// Unknown until a token response names them.
    scopes: Option<HashSet<String>>,
}

/// Grants by address; `None` when the store was checked and holds none.
pub(crate) type Grants = HashMap<String, Option<Grant>>;

/// The scopes a grant may carry, with the service each one is for.
const SERVICES: [(&str, &str); 3] = [
    (GOOGLE_CALENDAR_SCOPE, "Google Calendar"),
    (GOOGLE_CONTACTS_SCOPE, "Google Contacts"),
    (GOOGLE_TASKS_SCOPE, "Google Tasks"),
];

fn key(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn parse_scopes(scope: Option<&str>) -> Option<HashSet<String>> {
    scope
        .map(|scope| {
            scope
                .split_whitespace()
                .map(str::to_string)
                .collect::<HashSet<_>>()
        })
        .filter(|scopes| !scopes.is_empty())
}

fn join_scopes(scopes: &Option<HashSet<String>>) -> String {
    let mut scopes: Vec<&str> = scopes.iter().flatten().map(String::as_str).collect();
    scopes.sort_unstable();
    scopes.join(" ")
}

fn lock(engine: &Engine) -> Result<std::sync::MutexGuard<'_, Connection>> {
    engine.db.lock().map_err(|_| anyhow!("store lock poisoned"))
}

// Lock order everywhere: the store, then the grants, then the connects.
fn grants(engine: &Engine) -> Result<std::sync::MutexGuard<'_, Grants>> {
    engine
        .google_grants
        .lock()
        .map_err(|_| anyhow!("Google grant lock poisoned"))
}

fn connecting(
    engine: &Engine,
) -> Result<std::sync::MutexGuard<'_, HashMap<String, Vec<&'static str>>>> {
    engine
        .google_connecting
        .lock()
        .map_err(|_| anyhow!("Google grant lock poisoned"))
}

/// The grant held for `email`, read from the store once per run.
fn load(engine: &Engine, email: &str) -> Result<Option<Grant>> {
    if let Some(known) = grants(engine)?.get(&key(email)) {
        return Ok(known.clone());
    }
    load_locked(engine, &*lock(engine)?, email)
}

fn load_locked(engine: &Engine, conn: &Connection, email: &str) -> Result<Option<Grant>> {
    let email = key(email);
    if let Some(known) = grants(engine)?.get(&email) {
        return Ok(known.clone());
    }
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT id, scopes FROM google_grants WHERE email=?1",
            [&email],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((id, scopes)) = row else {
        return Ok(grants(engine)?.entry(email).or_insert(None).clone());
    };
    // Without its refresh token a grant is as good as none. Not remembered
    // as absent: the keychain may only be unavailable for now.
    let Some(refresh_token) = engine
        .host
        .load_secret(conn, &id)
        .refresh_token
        .filter(|token| !token.is_empty())
    else {
        return Ok(None);
    };
    let stored = Grant {
        id,
        refresh_token,
        scopes: parse_scopes(Some(&scopes)),
        ..Grant::default()
    };
    Ok(grants(engine)?.entry(email).or_insert(Some(stored)).clone())
}

/// Keep `grant` for `email`: its row, its refresh token when `previous`
/// (the one stored) differs, and the cache.
fn save(
    engine: &Engine,
    conn: &Connection,
    email: &str,
    grant: Grant,
    previous: Option<&str>,
) -> Result<()> {
    // The secret first: scopes recorded against a token that was never
    // stored would claim more than the stored one covers.
    if previous != Some(grant.refresh_token.as_str()) {
        engine.host.store_secret(
            conn,
            &grant.id,
            &Secrets {
                refresh_token: Some(grant.refresh_token.clone()),
                ..Default::default()
            },
        )?;
    }
    conn.execute(
        "INSERT INTO google_grants(email, id, scopes) VALUES (?1, ?2, ?3)
         ON CONFLICT(email) DO UPDATE SET id=excluded.id, scopes=excluded.scopes",
        params![key(email), grant.id, join_scopes(&grant.scopes)],
    )?;
    grants(engine)?.insert(key(email), Some(grant));
    Ok(())
}

fn delete(engine: &Engine, conn: &Connection, email: &str, id: &str) -> Result<()> {
    engine.host.delete_secret(conn, id)?;
    conn.execute("DELETE FROM google_grants WHERE email=?1", [key(email)])?;
    grants(engine)?.insert(key(email), None);
    Ok(())
}

/// A current access token for `email` that covers `scope`, refreshing the
/// grant when needed. `None` when no grant is held, Google no longer honors
/// it, or it does not cover `scope`: a browser sign-in is needed then.
pub(crate) fn access_token(engine: &Engine, email: &str, scope: &str) -> Result<Option<String>> {
    access_token_with(engine, &client(), email, scope, false)
}

/// Any Google DAV provider's client: they differ only in the scopes asked.
fn client() -> oauth::Client {
    default_oauth_client(oauth::GOOGLE_CALENDAR_PROVIDER)
}

/// `fresh` skips a cached access token: Google may have rejected it before
/// it expired, which is why a service is being reconnected.
fn access_token_with(
    engine: &Engine,
    client: &oauth::Client,
    email: &str,
    scope: &str,
    fresh: bool,
) -> Result<Option<String>> {
    // A sign-in that replaced the grant while it was being refreshed saved
    // one that may well cover `scope`: use that one instead.
    for _ in 0..3 {
        if let Refresh::Done(token) = try_access_token(engine, client, email, scope, fresh)? {
            return Ok(token);
        }
    }
    Ok(None)
}

enum Refresh {
    Done(Option<String>),
    /// The grant was replaced or removed while being refreshed.
    Superseded,
}

fn try_access_token(
    engine: &Engine,
    client: &oauth::Client,
    email: &str,
    scope: &str,
    fresh: bool,
) -> Result<Refresh> {
    let Some(grant) = load(engine, email)? else {
        return Ok(Refresh::Done(None));
    };
    if let Some(scopes) = &grant.scopes {
        if !scopes.contains(scope) {
            return Ok(Refresh::Done(None));
        }
        if !fresh
            && !grant.access_token.is_empty()
            && grant.expires_at > crate::store::now_unix() + 60
        {
            return Ok(Refresh::Done(Some(grant.access_token)));
        }
    }
    let tokens = match oauth::refresh_google_dav(client, &grant.refresh_token) {
        Ok(tokens) => tokens,
        // Revoked, or expired unused: only a new sign-in brings it back.
        Err(err) if err.contains("invalid_grant") => {
            let conn = lock(engine)?;
            if !is_current(engine, email, &grant)? {
                return Ok(Refresh::Superseded);
            }
            delete(engine, &conn, email, &grant.id)?;
            return Ok(Refresh::Done(None));
        }
        Err(_) => bail!("Could not refresh Google authorization. Try again."),
    };
    let refreshed = Grant {
        id: grant.id.clone(),
        refresh_token: tokens
            .refresh_token
            .filter(|token| !token.is_empty())
            .unwrap_or_else(|| grant.refresh_token.clone()),
        access_token: tokens.access_token,
        expires_at: crate::store::now_unix().saturating_add(tokens.expires_in.unwrap_or(3600)),
        // Left out when unchanged: keep the ones known.
        scopes: parse_scopes(tokens.scope.as_deref()).or_else(|| grant.scopes.clone()),
    };
    let covered = refreshed
        .scopes
        .as_ref()
        .is_some_and(|scopes| scopes.contains(scope));
    let access = refreshed.access_token.clone();
    // Kept unless a sign-in or a disconnect replaced the grant meanwhile;
    // then its token is not handed out either, as a service would save
    // itself against a grant that is gone.
    let conn = lock(engine)?;
    if !is_current(engine, email, &grant)? {
        return Ok(Refresh::Superseded);
    }
    save(engine, &conn, email, refreshed, Some(&grant.refresh_token))?;
    Ok(Refresh::Done(covered.then_some(access)))
}

fn is_current(engine: &Engine, email: &str, grant: &Grant) -> Result<bool> {
    Ok(matches!(
        grants(engine)?.get(&key(email)),
        Some(Some(current)) if current.id == grant.id && current.refresh_token == grant.refresh_token
    ))
}

/// Google refused a DAV request's authorization (HTTP 401) though the grant
/// refreshed: only a new browser sign-in can fix that.
#[derive(Debug)]
pub(crate) struct Rejected(pub &'static str);

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Rejected {}

/// Whether a connect that used the grant already held (no fresh sign-in in
/// `params`) failed only because Google refused that authorization: then a
/// browser sign-in is due, for these scopes, rather than the error.
pub(crate) fn sign_in_after(
    engine: &Engine,
    email: &str,
    scope: &str,
    params: &Value,
    err: &anyhow::Error,
) -> Result<Option<Vec<&'static str>>> {
    let reused = params
        .get("access_token")
        .and_then(Value::as_str)
        .is_none_or(|token| token.trim().is_empty());
    if !reused || !err.is::<Rejected>() {
        return Ok(None);
    }
    Ok(Some(needed_scopes(engine, &*lock(engine)?, email, scope)?))
}

/// What connecting a service found.
#[derive(Debug)]
pub(crate) enum Connected {
    /// The access token to connect with.
    Token(String),
    /// The host must sign in through the browser for these scopes, then call
    /// again with the result.
    SignIn(Vec<&'static str>),
}

/// Connect a service of `email` that needs `scope`. If `params` carry a fresh
/// browser sign-in (`access_token`, `refresh_token`, `expires_in`, `scope`),
/// it becomes the account's grant. Otherwise the grant already held is used,
/// with a freshly refreshed access token.
pub(crate) fn connect(
    engine: &Engine,
    email: &str,
    scope: &str,
    params: &Value,
) -> Result<Connected> {
    connect_with(engine, &client(), email, scope, params)
}

fn connect_with(
    engine: &Engine,
    client: &oauth::Client,
    email: &str,
    scope: &str,
    params: &Value,
) -> Result<Connected> {
    let text = |key: &str| {
        params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    let Some(access_token) = text("access_token") else {
        // Any failure to refresh (a revoked grant, or one Google will not
        // refresh for this client any more) leaves a browser sign-in, which
        // replaces the grant, rather than an error every retry repeats.
        return Ok(
            match access_token_with(engine, client, email, scope, true) {
                Ok(Some(token)) => Connected::Token(token),
                Ok(None) | Err(_) => {
                    Connected::SignIn(needed_scopes(engine, &*lock(engine)?, email, scope)?)
                }
            },
        );
    };
    let tokens = Tokens {
        access_token,
        refresh_token: text("refresh_token"),
        expires_in: params.get("expires_in").and_then(Value::as_i64),
        id_token: None,
        scope: text("scope"),
    };
    save_sign_in(engine, email, &tokens, scope)?;
    Ok(Connected::Token(tokens.access_token))
}

/// `scope`, and every scope a service of `email` uses or is connecting
/// with: a sign-in replaces the grant, so it must cover them all.
fn needed_scopes(
    engine: &Engine,
    conn: &Connection,
    email: &str,
    scope: &str,
) -> Result<Vec<&'static str>> {
    let email = key(email);
    let calendar = crate::caldav::store::accounts(conn)?
        .iter()
        .any(|account| account.google && account.username == email);
    let contacts: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM carddav_accounts WHERE google AND username=?1)",
        [&email],
        |row| row.get(0),
    )?;
    let tasks = crate::google_tasks::has_account(conn, &email)?;
    let connecting = connecting(engine)?.get(&email).cloned().unwrap_or_default();
    Ok(SERVICES
        .iter()
        .map(|(known, _)| *known)
        .filter(|known| {
            *known == scope
                || connecting.contains(known)
                || (*known == GOOGLE_CALENDAR_SCOPE && calendar)
                || (*known == GOOGLE_CONTACTS_SCOPE && contacts)
                || (*known == GOOGLE_TASKS_SCOPE && tasks)
        })
        .collect())
}

fn save_sign_in(engine: &Engine, email: &str, tokens: &Tokens, scope: &str) -> Result<()> {
    let scopes = parse_scopes(tokens.scope.as_deref()).unwrap_or_default();
    let conn = lock(engine)?;
    // A scope left unticked on Google's consent screen: keep the grant the
    // services already connected use, rather than one that would cut them off.
    let missing: Vec<&str> = needed_scopes(engine, &conn, email, scope)?
        .into_iter()
        .filter(|needed| !scopes.contains(*needed))
        .filter_map(|needed| SERVICES.iter().find(|(known, _)| *known == needed))
        .map(|(_, service)| *service)
        .collect();
    if !missing.is_empty() {
        bail!(
            "Allow Meron to access {} when signing in to Google",
            missing.join(" and ")
        );
    }
    let previous = load_locked(engine, &conn, email)?;
    let refresh_token = tokens
        .refresh_token
        .clone()
        .or_else(|| previous.as_ref().map(|grant| grant.refresh_token.clone()))
        .ok_or_else(|| anyhow!("Google sign-in did not return a refresh token"))?;
    let grant = Grant {
        // A row whose secret could not be read keeps its id: a new one
        // would leave the old secret behind.
        id: match &previous {
            Some(grant) => grant.id.clone(),
            None => conn
                .query_row(
                    "SELECT id FROM google_grants WHERE email=?1",
                    [key(email)],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or_else(|| crate::tasks::new_id("google-grant")),
        },
        refresh_token,
        access_token: tokens.access_token.clone(),
        expires_at: crate::store::now_unix().saturating_add(tokens.expires_in.unwrap_or(3600)),
        scopes: Some(scopes),
    };
    let previous = previous.map(|grant| grant.refresh_token);
    save(engine, &conn, email, grant, previous.as_deref())
}

/// A connect of a service of `email` in flight: until it is dropped, the
/// grant is kept, and a sign-in must cover its scope too, even with no
/// account saved yet to use them.
pub(crate) struct Pending<'a> {
    engine: &'a Engine,
    email: String,
    scope: &'static str,
}

pub(crate) fn pending<'a>(engine: &'a Engine, email: &str, scope: &'static str) -> Pending<'a> {
    let email = key(email);
    // Under the store lock `release` checks with, so it cannot miss this.
    let conn = engine.db.lock();
    if let Ok(mut connecting) = connecting(engine) {
        connecting.entry(email.clone()).or_default().push(scope);
    }
    drop(conn);
    Pending {
        engine,
        email,
        scope,
    }
}

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Ok(mut connecting) = connecting(self.engine)
            && let Some(scopes) = connecting.get_mut(&self.email)
        {
            if let Some(at) = scopes.iter().position(|scope| *scope == self.scope) {
                scopes.swap_remove(at);
            }
            if scopes.is_empty() {
                connecting.remove(&self.email);
            }
        }
    }
}

/// End a connect, dropping the grant if no service uses it now: the connect
/// saved no account (it failed, or needs a browser sign-in first), or the
/// account was removed while it finished.
pub(crate) fn abandon(pending: Pending<'_>) -> Result<()> {
    let engine = pending.engine;
    let email = pending.email.clone();
    drop(pending);
    release(engine, &*lock(engine)?, &email)
}

/// Drop the grant for `email` once no service syncs with it any more: a
/// later connect signs in again, as after removing the account everywhere.
/// Checked under the store lock connects save their account with.
pub(crate) fn release(engine: &Engine, conn: &Connection, email: &str) -> Result<()> {
    // The empty scope matches no service: only those used or connecting are listed.
    if !needed_scopes(engine, conn, email, "")?.is_empty() {
        return Ok(());
    }
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM google_grants WHERE email=?1",
            [key(email)],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => delete(engine, conn, email, &id),
        None => {
            grants(engine)?.insert(key(email), None);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::{GOOGLE_CALENDAR_SCOPE, GOOGLE_CONTACTS_SCOPE};
    use serde_json::json;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Secrets kept in memory, so the test can see what reached the store.
    #[derive(Clone, Default)]
    struct Host(Arc<Mutex<HashMap<String, Secrets>>>);

    impl crate::engine::EngineHost for Host {
        fn open_db(&self) -> Result<rusqlite::Connection> {
            crate::store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &rusqlite::Connection, _: &str, _: &mut crate::imap::Creds) {}
        fn store_secret(
            &self,
            _: &rusqlite::Connection,
            id: &str,
            secrets: &Secrets,
        ) -> Result<()> {
            self.0.lock().unwrap().insert(id.into(), secrets.clone());
            Ok(())
        }
        fn load_secret(&self, _: &rusqlite::Connection, id: &str) -> Secrets {
            self.0.lock().unwrap().get(id).cloned().unwrap_or_default()
        }
        fn delete_secret(&self, _: &rusqlite::Connection, id: &str) -> Result<()> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }

    /// A token endpoint answering each request in turn with `(status, body)`.
    fn token_server(
        responses: Vec<(&'static str, String)>,
    ) -> (oauth::Client, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/token", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let _ = tx.send(crate::test_support::read_http_request(&mut stream));
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let client = oauth::Client {
            provider: oauth::GOOGLE_CALENDAR_PROVIDER.into(),
            client_id: "client".into(),
            token_url: url,
            ..Default::default()
        };
        (client, rx)
    }

    fn sign_in(scopes: &[&str]) -> Value {
        json!({
            "access_token": "fresh-access",
            "refresh_token": "fresh-refresh",
            "expires_in": 3600,
            "scope": format!("openid email {}", scopes.join(" ")),
        })
    }

    /// The refresh token kept for `email`, through its row's secret id.
    fn stored(engine: &Engine, host: &Host, email: &str) -> Option<String> {
        let id: String = engine
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT id FROM google_grants WHERE email=?1",
                [email],
                |row| row.get(0),
            )
            .ok()?;
        host.0.lock().unwrap().get(&id)?.refresh_token.clone()
    }

    fn token(connected: Connected) -> Option<String> {
        match connected {
            Connected::Token(token) => Some(token),
            Connected::SignIn(_) => None,
        }
    }

    fn sign_in_scopes(connected: Connected) -> Vec<&'static str> {
        match connected {
            Connected::Token(_) => panic!("expected a sign-in"),
            Connected::SignIn(scopes) => scopes,
        }
    }

    fn add_contacts_account(engine: &Engine, email: &str) {
        engine
            .db
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO carddav_accounts(id, server, username, google) VALUES ('carddav-1', 'https://www.googleapis.com/carddav/v1/principals/', ?1, 1)",
                [email],
            )
            .unwrap();
    }

    #[test]
    fn a_sign_in_covers_every_connected_service_and_reconnects_refresh() {
        let host = Host::default();
        let engine = Engine::new(Box::new(host.clone())).unwrap();
        let email = "cora@example.test";
        let (client, requests) = token_server(vec![(
            "200 OK",
            format!(
                r#"{{"access_token":"refreshed","expires_in":3600,"scope":"{GOOGLE_CONTACTS_SCOPE}"}}"#
            ),
        )]);
        let connect = |scope, params: Value| connect_with(&engine, &client, email, scope, &params);

        // Nothing held yet: sign in for Contacts alone.
        assert_eq!(
            sign_in_scopes(connect(GOOGLE_CONTACTS_SCOPE, json!({})).unwrap()),
            [GOOGLE_CONTACTS_SCOPE]
        );
        assert_eq!(
            token(connect(GOOGLE_CONTACTS_SCOPE, sign_in(&[GOOGLE_CONTACTS_SCOPE])).unwrap())
                .as_deref(),
            Some("fresh-access")
        );
        assert_eq!(
            stored(&engine, &host, email).as_deref(),
            Some("fresh-refresh")
        );
        add_contacts_account(&engine, email);

        // Reconnecting Contacts needs no browser, but does not trust the
        // cached access token: Google may have rejected it.
        assert_eq!(
            token(connect(GOOGLE_CONTACTS_SCOPE, json!({})).unwrap()).as_deref(),
            Some("refreshed")
        );
        assert!(
            requests
                .recv()
                .unwrap()
                .contains("refresh_token=fresh-refresh")
        );

        // Calendar is not covered: its sign-in asks for Contacts again, as
        // the grant it replaces must keep serving it.
        assert_eq!(
            sign_in_scopes(connect(GOOGLE_CALENDAR_SCOPE, json!({})).unwrap()),
            [GOOGLE_CALENDAR_SCOPE, GOOGLE_CONTACTS_SCOPE]
        );
        // Contacts unticked on the consent screen: refused, the grant kept.
        let err = connect(GOOGLE_CALENDAR_SCOPE, sign_in(&[GOOGLE_CALENDAR_SCOPE])).unwrap_err();
        assert!(err.to_string().contains("Google Contacts"), "{err}");
        assert_eq!(
            access_token(&engine, email, GOOGLE_CALENDAR_SCOPE).unwrap(),
            None
        );
        assert_eq!(
            access_token(&engine, email, GOOGLE_CONTACTS_SCOPE)
                .unwrap()
                .as_deref(),
            Some("refreshed")
        );

        // Both allowed: one grant serves both.
        let both = json!({
            "access_token": "both-access",
            "refresh_token": "both-refresh",
            "expires_in": 3600,
            "scope": format!("openid email {GOOGLE_CALENDAR_SCOPE} {GOOGLE_CONTACTS_SCOPE}"),
        });
        connect(GOOGLE_CALENDAR_SCOPE, both).unwrap();
        for scope in [GOOGLE_CALENDAR_SCOPE, GOOGLE_CONTACTS_SCOPE] {
            assert_eq!(
                access_token(&engine, email, scope).unwrap().as_deref(),
                Some("both-access")
            );
        }
        assert_eq!(
            stored(&engine, &host, email).as_deref(),
            Some("both-refresh")
        );
    }

    #[test]
    fn a_connected_tasks_account_keeps_its_grant_and_scope() {
        let host = Host::default();
        let engine = Engine::new(Box::new(host.clone())).unwrap();
        let email = "tasks@example.test";
        save_sign_in(
            &engine,
            email,
            &Tokens {
                access_token: "fixture-access".into(),
                refresh_token: Some("fixture-refresh".into()),
                scope: Some(GOOGLE_TASKS_SCOPE.into()),
                expires_in: Some(3600),
                id_token: None,
            },
            GOOGLE_TASKS_SCOPE,
        )
        .unwrap();
        {
            let conn = engine.db.lock().unwrap();
            conn.execute(
                "INSERT INTO google_task_accounts (email) VALUES (?1)",
                [email],
            )
            .unwrap();
            assert_eq!(
                needed_scopes(&engine, &conn, email, GOOGLE_CALENDAR_SCOPE).unwrap(),
                [GOOGLE_CALENDAR_SCOPE, GOOGLE_TASKS_SCOPE]
            );
            release(&engine, &conn, email).unwrap();
        }
        assert_eq!(
            stored(&engine, &host, email).as_deref(),
            Some("fixture-refresh")
        );
    }

    #[test]
    fn a_stored_grant_keeps_its_scopes_and_is_dropped_when_revoked() {
        let host = Host::default();
        host.0.lock().unwrap().insert(
            "google-grant-1".into(),
            Secrets {
                refresh_token: Some("stored".into()),
                ..Default::default()
            },
        );
        let engine = Engine::new(Box::new(host.clone())).unwrap();
        engine
            .db
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO google_grants(email, id, scopes) VALUES ('cora@example.test', 'google-grant-1', '')",
                [],
            )
            .unwrap();
        let (client, requests) = token_server(vec![
            (
                "200 OK",
                format!(
                    r#"{{"access_token":"a1","expires_in":3600,"scope":"{GOOGLE_CALENDAR_SCOPE}"}}"#
                ),
            ),
            (
                "200 OK",
                r#"{"access_token":"a2","expires_in":3600}"#.into(),
            ),
            ("400 Bad Request", r#"{"error":"invalid_grant"}"#.into()),
        ]);
        let token =
            |scope| access_token_with(&engine, &client, "Cora@example.test", scope, false).unwrap();
        let expire = || {
            if let Some(Some(grant)) = engine.google_grants.lock().unwrap().values_mut().next() {
                grant.expires_at = 0;
            }
        };

        // Scopes not recorded yet: a refresh names them, and they are kept.
        assert_eq!(token(GOOGLE_CONTACTS_SCOPE), None);
        assert!(requests.recv().unwrap().contains("refresh_token=stored"));
        let scopes: String = engine
            .db
            .lock()
            .unwrap()
            .query_row("SELECT scopes FROM google_grants", [], |row| row.get(0))
            .unwrap();
        assert_eq!(scopes, GOOGLE_CALENDAR_SCOPE);
        // Current: no further request.
        assert_eq!(token(GOOGLE_CALENDAR_SCOPE).as_deref(), Some("a1"));
        assert_eq!(token(GOOGLE_CONTACTS_SCOPE), None);

        // A refresh that leaves the scopes out keeps the ones known.
        expire();
        assert_eq!(token(GOOGLE_CALENDAR_SCOPE).as_deref(), Some("a2"));
        assert!(requests.recv().is_ok());

        // Revoked: the grant is forgotten, row and secret.
        expire();
        assert_eq!(token(GOOGLE_CALENDAR_SCOPE), None);
        assert!(requests.recv().is_ok());
        assert!(host.0.lock().unwrap().is_empty());
        let rows: i64 = engine
            .db
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM google_grants", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn a_sign_in_covers_services_still_connecting() {
        let host = Host::default();
        let engine = Engine::new(Box::new(host.clone())).unwrap();
        let email = "cora@example.test";
        // Calendar signed in and is still discovering its calendars.
        let calendar = pending(&engine, email, GOOGLE_CALENDAR_SCOPE);
        connect(
            &engine,
            email,
            GOOGLE_CALENDAR_SCOPE,
            &sign_in(&[GOOGLE_CALENDAR_SCOPE]),
        )
        .unwrap();
        // Contacts' sign-in would replace the grant Calendar is connecting with.
        let contacts = pending(&engine, email, GOOGLE_CONTACTS_SCOPE);
        let err = connect(
            &engine,
            email,
            GOOGLE_CONTACTS_SCOPE,
            &sign_in(&[GOOGLE_CONTACTS_SCOPE]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Google Calendar"), "{err}");
        abandon(contacts).unwrap();
        // Calendar fails too, and nothing is left using the grant.
        assert!(stored(&engine, &host, email).is_some());
        abandon(calendar).unwrap();
        assert!(stored(&engine, &host, email).is_none());
        assert!(host.0.lock().unwrap().is_empty());
    }

    #[test]
    fn a_grant_google_will_not_refresh_leads_to_a_sign_in() {
        let host = Host::default();
        let engine = Engine::new(Box::new(host.clone())).unwrap();
        let email = "cora@example.test";
        let (client, _requests) = token_server(vec![(
            "401 Unauthorized",
            r#"{"error":"unauthorized_client"}"#.into(),
        )]);
        connect(
            &engine,
            email,
            GOOGLE_CONTACTS_SCOPE,
            &sign_in(&[GOOGLE_CONTACTS_SCOPE]),
        )
        .unwrap();
        // Reconnecting asks for a sign-in rather than failing every retry,
        // and keeps the grant until that sign-in replaces it.
        assert_eq!(
            sign_in_scopes(
                connect_with(&engine, &client, email, GOOGLE_CONTACTS_SCOPE, &json!({})).unwrap()
            ),
            [GOOGLE_CONTACTS_SCOPE]
        );
        assert!(stored(&engine, &host, email).is_some());
    }

    #[test]
    fn only_a_rejected_reused_grant_asks_for_a_sign_in() {
        let engine = Engine::new(Box::new(Host::default())).unwrap();
        let email = "cora@example.test";
        add_contacts_account(&engine, email);
        let rejected = anyhow::Error::from(Rejected("Google Contacts rejected authorization."));
        let after = |params: Value, err: &anyhow::Error| {
            sign_in_after(&engine, email, GOOGLE_CALENDAR_SCOPE, &params, err).unwrap()
        };
        // Reused grant refused: sign in for Calendar and the Contacts kept.
        assert_eq!(
            after(json!({}), &rejected),
            Some(vec![GOOGLE_CALENDAR_SCOPE, GOOGLE_CONTACTS_SCOPE])
        );
        // A fresh sign-in refused, or any other failure, is reported as is.
        assert_eq!(after(sign_in(&[GOOGLE_CALENDAR_SCOPE]), &rejected), None);
        assert_eq!(after(json!({}), &anyhow::anyhow!("network down")), None);
    }

    #[test]
    fn release_keeps_the_grant_while_a_service_uses_it_or_connects() {
        let host = Host::default();
        let engine = Engine::new(Box::new(host.clone())).unwrap();
        let email = "cora@example.test";
        let release = || release(&engine, &engine.db.lock().unwrap(), email).unwrap();
        connect(
            &engine,
            email,
            GOOGLE_CONTACTS_SCOPE,
            &sign_in(&[GOOGLE_CONTACTS_SCOPE]),
        )
        .unwrap();

        // A connect still discovering, its account not saved yet.
        let connecting = pending(&engine, "Cora@Example.test", GOOGLE_CONTACTS_SCOPE);
        release();
        assert!(!host.0.lock().unwrap().is_empty());
        add_contacts_account(&engine, email);
        drop(connecting);
        release();
        assert!(
            access_token(&engine, email, GOOGLE_CONTACTS_SCOPE)
                .unwrap()
                .is_some()
        );

        engine
            .db
            .lock()
            .unwrap()
            .execute("DELETE FROM carddav_accounts", [])
            .unwrap();
        release();
        assert!(host.0.lock().unwrap().is_empty());
        assert_eq!(
            access_token(&engine, email, GOOGLE_CONTACTS_SCOPE).unwrap(),
            None
        );
    }
}
