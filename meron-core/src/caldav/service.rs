//! CalDAV accounts as both apps use them: add one (discovery doubles as the
//! password check), list them with their calendars, pick which calendars to
//! show, remove one, and sync.
//!
//! Every call here may block on the network; the sidecar runs them on a
//! blocking thread and mobile calls them off the main thread already.

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use super::Client;
use super::store::{self, DavAccount, DavCalendar};
use super::sync::{self, Report};
use crate::engine::{Engine, default_oauth_client};
use crate::google_grant::Connected;
use crate::oauth::{self, GOOGLE_CALENDAR_PROVIDER, GOOGLE_CALENDAR_SCOPE};
use crate::secrets::Secrets;

/// Run one `caldav.*` request: the vocabulary desktop and mobile share.
pub fn dispatch(engine: &Engine, method: &str, params: &Value) -> Result<Value> {
    let text = |key: &str| params.get(key).and_then(Value::as_str).unwrap_or_default();
    let required = |key: &str| match text(key) {
        "" => Err(anyhow!("missing {key}")),
        value => Ok(value),
    };
    let flag = |key: &str| params.get(key).and_then(Value::as_bool);
    let conn = || engine.db.lock().map_err(|_| anyhow!("store lock poisoned"));
    match method {
        "caldav.discoverServer" => Ok(json!({
            "server_url": super::discovery::discover_server(required("email")?, super::discovery::Dav::Calendars)
                .map(|url| url.to_string()),
        })),
        "caldav.accounts" => Ok(json!({ "accounts": accounts_payload(&*conn()?)? })),
        "caldav.calendars" => calendars_payload(&*conn()?),
        "caldav.add" => {
            // Left blank, with a mail account on the same login: that
            // account's password, read here so it never reaches the UI.
            let reused = match (text("password"), text("mail_account_id")) {
                ("", mail) if !mail.is_empty() => {
                    Some(mail_account_password(engine, mail, text("username"))?)
                }
                _ => None,
            };
            add_account(
                engine,
                &NewAccount {
                    server_url: text("server_url"),
                    username: text("username"),
                    password: reused.as_deref().unwrap_or(text("password")),
                    display_name: text("display_name"),
                },
            )
        }
        "caldav.addGoogle" => {
            let email = required("email")?.trim().to_ascii_lowercase();
            if !email.contains('@') {
                bail!("Google sign-in did not return an email address");
            }
            let pending = crate::google_grant::pending(engine, &email, GOOGLE_CALENDAR_SCOPE);
            // Saved nowhere: the grant goes unless something else uses it.
            let (added, scopes) =
                match crate::google_grant::connect(engine, &email, GOOGLE_CALENDAR_SCOPE, params) {
                    Ok(Connected::Token(access)) => {
                        match add_google(engine, &email, &access, text("account_id")) {
                            // Google refused the grant's token: sign in afresh.
                            Err(err) => match crate::google_grant::sign_in_after(
                                engine,
                                &email,
                                GOOGLE_CALENDAR_SCOPE,
                                params,
                                &err,
                            )? {
                                Some(scopes) => (Err(err), Some(scopes)),
                                None => (Err(err), None),
                            },
                            added => (added, None),
                        }
                    }
                    Ok(Connected::SignIn(scopes)) => (Err(anyhow!("needs sign-in")), Some(scopes)),
                    Err(err) => (Err(err), None),
                };
            // Failed, or the account was removed meanwhile: the grant goes
            // unless something else uses it.
            crate::google_grant::abandon(pending)?;
            match scopes {
                Some(scopes) => Ok(json!({ "needs_sign_in": true, "scopes": scopes })),
                None => added,
            }
        }
        "caldav.setPassword" => set_password(engine, required("account_id")?, text("password")),
        "caldav.remove" => remove_account(
            engine,
            required("account_id")?,
            flag("discard_unsent").unwrap_or(false),
        ),
        "caldav.setCalendarEnabled" => set_calendar_enabled(
            engine,
            required("calendar_id")?,
            flag("enabled").unwrap_or(true),
            flag("discard_unsent").unwrap_or(false),
        ),
        "caldav.createCalendar" => create_calendar(
            engine,
            required("account_id")?,
            required("name")?,
            text("color"),
        ),
        "caldav.updateCalendar" => update_calendar(
            engine,
            required("calendar_id")?,
            params.get("name").and_then(Value::as_str),
            params.get("color").and_then(Value::as_str),
        ),
        "caldav.deleteCalendar" => delete_calendar(engine, required("calendar_id")?),
        "caldav.renameAccount" => {
            let conn = conn()?;
            let id = required("account_id")?;
            let mut account = store::account(&conn, id)?
                .ok_or_else(|| anyhow!("no such CalDAV account: {id}"))?;
            // An empty name goes back to the server's host, as on adding;
            // a Google account's back to its mailbox's name.
            account.display_name = match text("display_name").trim() {
                "" if account.google => String::new(),
                "" => super::parse_server_url(&account.server_url)?
                    .host_str()
                    .unwrap_or(&account.username)
                    .to_string(),
                name => name.to_string(),
            };
            store::save_account(&conn, &account)?;
            Ok(json!({ "accounts": accounts_payload(&conn)? }))
        }
        "caldav.setSyncInterval" => {
            let minutes = params
                .get("minutes")
                .and_then(Value::as_i64)
                .ok_or_else(|| anyhow!("missing minutes"))?;
            let conn = conn()?;
            store::set_sync_interval(&conn, required("account_id")?, minutes)?;
            Ok(json!({ "accounts": accounts_payload(&conn)? }))
        }
        "caldav.sync" => {
            let account = params.get("account_id").and_then(Value::as_str);
            let automatic = params
                .get("automatic")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let report = sync(engine, account, automatic)?;
            Ok(json!({
                "pulled": report.pulled,
                "pushed": report.pushed,
                "error": report.error,
            }))
        }
        other => Err(anyhow!("unknown method: {other}")),
    }
}

pub struct NewAccount<'a> {
    pub server_url: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub display_name: &'a str,
}

/// The password a mail account signs in with, when it signs in with one
/// and its login is `username`: the same server account often has
/// calendars too, and the same password opens them.
pub(crate) fn mail_account_password(
    engine: &Engine,
    mail_account_id: &str,
    username: &str,
) -> Result<String> {
    // The engine's own copy holds a password kept only in memory (no
    // keychain); the store and keychain cover accounts it has not loaded.
    let loaded = engine
        .accounts
        .try_lock()
        .ok()
        .and_then(|accounts| accounts.get(mail_account_id).cloned());
    let creds = match loaded {
        Some(creds) => Some(creds),
        None => {
            let conn = lock(engine)?;
            crate::store::load_accounts(&conn)?
                .into_iter()
                .find(|(id, _)| id == mail_account_id)
                .map(|(id, mut creds)| {
                    engine.host.apply_secret(&conn, &id, &mut creds);
                    creds
                })
        }
    };
    match creds {
        Some(creds)
            if !creds.is_oauth()
                && !creds.password.is_empty()
                && creds.user.trim().eq_ignore_ascii_case(username.trim()) =>
        {
            Ok(creds.password)
        }
        _ => bail!("no saved password for {}; enter it", username.trim()),
    }
}

fn lock(engine: &Engine) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    engine.db.lock().map_err(|_| anyhow!("store lock poisoned"))
}

/// Connect, find the calendars, and keep the account. Fails without keeping
/// anything when the server, the username or the password is wrong. Adding
/// an account that is already here (a retry after the first attempt timed
/// out in the UI) updates its password instead of adding it twice.
pub fn add_account(engine: &Engine, new: &NewAccount<'_>) -> Result<Value> {
    let username = new.username.trim();
    if username.is_empty() || new.password.is_empty() {
        bail!("a username and password are required");
    }
    let server = super::parse_server_url(new.server_url)?;
    let same_account = |conn: &rusqlite::Connection| -> Result<Option<DavAccount>> {
        Ok(store::accounts(conn)?.into_iter().find(|a| {
            !a.google
                && a.username == username
                && super::parse_server_url(&a.server_url).is_ok_and(|known| known == server)
        }))
    };
    if let Some(existing) = same_account(&*lock(engine)?)? {
        let mut answer = set_password(engine, &existing.id, new.password)?;
        answer["account_id"] = json!(existing.id);
        return Ok(answer);
    }
    let client = client(server.as_str(), username, new.password)?;
    let found = client.discover()?;
    let id = crate::tasks::new_id("dav");
    let display_name = match new.display_name.trim() {
        "" => server.host_str().unwrap_or(username).to_string(),
        name => name.to_string(),
    };
    let id = {
        let conn = lock(engine)?;
        // Another attempt finished first while this one was discovering:
        // keep that account, with the password just checked.
        let id = match same_account(&conn)? {
            Some(existing) => existing.id,
            None => id,
        };
        engine.host.store_secret(
            &conn,
            &id,
            &Secrets {
                password: new.password.to_string(),
                ..Default::default()
            },
        )?;
        if store::account(&conn, &id)?.is_none() {
            store::save_account(
                &conn,
                &DavAccount {
                    id: id.clone(),
                    server_url: new.server_url.trim().to_string(),
                    username: username.to_string(),
                    display_name,
                    home: found.home.clone(),
                    ..Default::default()
                },
            )?;
            store::reconcile_calendars(&conn, &id, &found.calendars)?;
        }
        id
    };
    remember_password(engine, &id, new.password);
    Ok(json!({ "account_id": id, "accounts": accounts_payload(&*lock(engine)?)? }))
}

/// Connect the signed-in user's calendars, authorized by the account's
/// Google grant. An existing account keeps its calendar IDs, local settings,
/// and sync state when reauthorized.
fn add_google(
    engine: &Engine,
    email: &str,
    access_token: &str,
    reconnect_id: &str,
) -> Result<Value> {
    let client = Client::google(email, access_token)?;
    // Verify CalDAV is enabled as well as Calendar List: events use CalDAV.
    let primary = client.primary_calendar()?;
    let listed = client.google_calendars()?;
    let calendar_list_disabled = listed.is_none();
    let mut calendars = listed.unwrap_or_else(|| vec![primary.clone()]);
    for calendar in &mut calendars {
        if calendar.url == client.server.as_str() {
            calendar.url.clone_from(&primary.url);
        }
    }
    let conn = lock(engine)?;
    let existing = store::accounts(&conn)?
        .into_iter()
        .find(|account| account.google && account.username == email);
    if !reconnect_id.is_empty()
        && existing
            .as_ref()
            .is_none_or(|account| account.id != reconnect_id)
    {
        bail!("Sign in with the Google account you are reconnecting");
    }
    let id = if let Some(account) = existing {
        // Connected before grants were shared: drop its own token.
        engine.host.delete_secret(&conn, &account.id)?;
        if let Ok(mut credentials) = engine.dav_credentials.lock() {
            credentials.remove(&account.id);
        }
        store::set_google_calendar_list_disabled(&conn, &account.id, calendar_list_disabled)?;
        account.id
    } else {
        let id = crate::tasks::new_id("dav");
        let transaction = conn.unchecked_transaction()?;
        store::save_account(
            &transaction,
            &DavAccount {
                id: id.clone(),
                server_url: client.server.to_string(),
                username: email.to_string(),
                // Unnamed: it goes by its mailbox's name (see `account_identity`).
                display_name: String::new(),
                google: true,
                ..Default::default()
            },
        )?;
        store::set_google_calendar_list_disabled(&transaction, &id, calendar_list_disabled)?;
        store::reconcile_calendars(&transaction, &id, &calendars)?;
        transaction.commit()?;
        id
    };
    Ok(json!({ "account_id": id, "accounts": accounts_payload(&conn)? }))
}

#[derive(Clone, Default)]
pub(crate) struct Credentials {
    secrets: Secrets,
    expires_at: i64,
}

fn save_google_credentials(
    engine: &Engine,
    conn: &rusqlite::Connection,
    id: &str,
    credentials: &Credentials,
) -> Result<()> {
    engine.host.store_secret(
        conn,
        id,
        &Secrets {
            refresh_token: credentials.secrets.refresh_token.clone(),
            ..Default::default()
        },
    )
}

/// Replace an account's password, after checking the server takes it.
pub fn set_password(engine: &Engine, account_id: &str, password: &str) -> Result<Value> {
    let account = store::account(&*lock(engine)?, account_id)?
        .ok_or_else(|| anyhow!("no such CalDAV account: {account_id}"))?;
    if account.google {
        bail!("Reconnect with Google to update this calendar account");
    }
    let client = client(&account.server_url, &account.username, password)?;
    match account.home.as_str() {
        "" => client.discover().map(drop)?,
        home => client.calendars(home).map(drop)?,
    }
    let conn = lock(engine)?;
    engine.host.store_secret(
        &conn,
        account_id,
        &Secrets {
            password: password.to_string(),
            ..Default::default()
        },
    )?;
    store::record_sync(&conn, account_id, "")?;
    remember_password(engine, account_id, password);
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

/// Edits made here that have not reached the server yet go when the
/// calendar is turned off or its account removed. Try to send them first;
/// what still cannot be sent is only thrown away when the caller says so.
/// Returns how many are left unsent.
/// The caller checks again, under the lock it then deletes with, so an edit
/// made meanwhile is not lost either.
fn flush_unsent(engine: &Engine, account_id: &str, calendar_id: Option<&str>, discard: bool) {
    let unsent = || -> Result<i64> { store::unsent(&*lock(engine)?, account_id, calendar_id) };
    if !discard && unsent().is_ok_and(|n| n > 0) {
        let _ = sync(engine, Some(account_id), false);
    }
}

pub fn remove_account(engine: &Engine, account_id: &str, discard_unsent: bool) -> Result<Value> {
    flush_unsent(engine, account_id, None, discard_unsent);
    // No sync of the account may be halfway through: what it brought back
    // would land on an account that is gone.
    let _sync = SyncGuard::claim(engine, account_id)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    let conn = lock(engine)?;
    let unsent = store::unsent(&conn, account_id, None)?;
    if unsent > 0 && !discard_unsent {
        return Ok(json!({ "unsent": unsent, "accounts": accounts_payload(&conn)? }));
    }
    let account = store::account(&conn, account_id)?;
    store::delete_account(&conn, account_id)?;
    engine.host.delete_secret(&conn, account_id)?;
    if let Ok(mut credentials) = engine.dav_credentials.lock() {
        credentials.remove(account_id);
    }
    if let Some(account) = account.filter(|account| account.google) {
        crate::google_grant::release(engine, &conn, &account.username)?;
    }
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

pub fn set_calendar_enabled(
    engine: &Engine,
    calendar_id: &str,
    enabled: bool,
    discard_unsent: bool,
) -> Result<Value> {
    let calendar = store::calendar(&*lock(engine)?, calendar_id)?
        .ok_or_else(|| anyhow!("no such calendar: {calendar_id}"))?;
    if enabled {
        // A sync already running listed its calendars before this one was
        // on and leaves it alone, so there is nothing to wait for.
        let conn = lock(engine)?;
        store::set_enabled(&conn, calendar_id, true)?;
        return Ok(json!({ "accounts": accounts_payload(&conn)? }));
    }
    flush_unsent(engine, &calendar.account, Some(calendar_id), discard_unsent);
    // Nor halfway through for a calendar being turned off: an upload that
    // finished after would be taken for a deletion to send.
    let _sync = SyncGuard::claim(engine, &calendar.account)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    let conn = lock(engine)?;
    if !enabled && !discard_unsent {
        let unsent = store::unsent(&conn, &calendar.account, Some(calendar_id))?;
        if unsent > 0 {
            return Ok(json!({ "unsent": unsent, "accounts": accounts_payload(&conn)? }));
        }
    }
    store::set_enabled(&conn, calendar_id, enabled)?;
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

/// How long a change to an account's calendars waits for a sync of it to
/// finish: well inside the desktop's timeout for the call.
const MUTATION_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// A client authenticated with the account's current credentials.
fn account_client(engine: &Engine, account: &DavAccount) -> Result<Client> {
    if account.google
        && let Some(access) =
            crate::google_grant::access_token(engine, &account.username, GOOGLE_CALENDAR_SCOPE)?
    {
        return Client::google(&account.username, &access);
    }
    let credentials = credentials(engine, &account.id)?;
    if !account.google {
        if credentials.secrets.password.is_empty() {
            bail!("no password stored for this calendar account; enter it again");
        }
        return client(
            &account.server_url,
            &account.username,
            &credentials.secrets.password,
        );
    }
    if credentials.expires_at > crate::store::now_unix() + 60
        && let Some(access) = &credentials.secrets.access_token
    {
        return Client::google(&account.username, access);
    }
    // Connected before grants were shared: its own token, if it still has one.
    let refresh = credentials
        .secrets
        .refresh_token
        .as_deref()
        .ok_or_else(|| anyhow!("Reconnect Google Calendar to resume syncing"))?;
    let oauth = default_oauth_client(GOOGLE_CALENDAR_PROVIDER);
    let tokens = oauth::refresh_google_dav(&oauth, refresh).map_err(anyhow::Error::msg)?;
    let client = Client::google(&account.username, &tokens.access_token)?;
    let refreshed = Credentials {
        expires_at: crate::store::now_unix().saturating_add(tokens.expires_in.unwrap_or(3600)),
        secrets: Secrets {
            access_token: Some(tokens.access_token),
            refresh_token: Some(tokens.refresh_token.unwrap_or_else(|| refresh.to_string())),
            ..Default::default()
        },
    };
    let conn = lock(engine)?;
    // Reauthorization can finish while the old grant is being refreshed.
    // Keep the newer credentials in that case.
    if cached_credentials(engine, &account.id).is_some_and(|current| {
        current.secrets.refresh_token == credentials.secrets.refresh_token
            && current.secrets.access_token == credentials.secrets.access_token
    }) {
        if refreshed.secrets.refresh_token != credentials.secrets.refresh_token {
            save_google_credentials(engine, &conn, &account.id, &refreshed)?;
        }
        remember_credentials(engine, &account.id, refreshed);
    }
    Ok(client)
}

/// Make a new event calendar on the account's server, and show it here.
pub fn create_calendar(
    engine: &Engine,
    account_id: &str,
    name: &str,
    color: &str,
) -> Result<Value> {
    let name = name.trim();
    if name.is_empty() {
        bail!("a calendar needs a name");
    }
    let account = store::account(&*lock(engine)?, account_id)?
        .ok_or_else(|| anyhow!("no such CalDAV account: {account_id}"))?;
    if account.google {
        bail!("create calendars in Google Calendar");
    }
    let client = account_client(engine, &account)?;
    // No sync of the account in between: one that listed the calendars
    // before this one was made would drop it here when it applied.
    let _sync = SyncGuard::claim_within(engine, account_id, MUTATION_WAIT)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    let home = match account.home.as_str() {
        "" => client.discover()?.home,
        home => home.to_string(),
    };
    let url = client.make_calendar(&home, name, color.trim())?;
    // Made: from here on, nothing may report it as not made, or a retry
    // would make a second. If the server cannot list it right away, it is
    // shown as made and the next sync fills in the rest.
    let made = |c: &super::Calendar| super::Url::parse(&c.url).ok() == super::Url::parse(&url).ok();
    let conn_result = match client.calendars(&home) {
        // A server may list the new calendar only a moment later.
        Ok(listed) if listed.iter().any(made) => {
            store::reconcile_calendars(&*lock(engine)?, account_id, &listed)
        }
        _ => store::add_calendar(
            &*lock(engine)?,
            account_id,
            &super::Calendar {
                url: url.clone(),
                name: name.to_string(),
                color: color.trim().to_string(),
                components: vec!["VEVENT".into()],
                ..Default::default()
            },
        ),
    };
    let conn = lock(engine)?;
    let created = conn_result
        .ok()
        .and_then(|calendars| {
            calendars
                .into_iter()
                .find(|c| super::Url::parse(&c.url).ok() == super::Url::parse(&url).ok())
        })
        .map(|c| c.id)
        .unwrap_or_default();
    Ok(json!({ "calendar_id": created, "accounts": accounts_payload(&conn)? }))
}

/// Rename or recolour a calendar on its server. A colour the server will
/// not take (a shared or subscribed calendar) is kept on this device.
pub fn update_calendar(
    engine: &Engine,
    calendar_id: &str,
    name: Option<&str>,
    color: Option<&str>,
) -> Result<Value> {
    let name = name.map(str::trim);
    if name == Some("") {
        bail!("a calendar needs a name");
    }
    let color = color.map(str::trim);
    let (calendar, account) = {
        let conn = lock(engine)?;
        let calendar = store::calendar(&conn, calendar_id)?
            .ok_or_else(|| anyhow!("no such calendar: {calendar_id}"))?;
        let account = store::account(&conn, &calendar.account)?
            .ok_or_else(|| anyhow!("no such CalDAV account: {}", calendar.account))?;
        (calendar, account)
    };
    // A sync that listed the calendars before this change, applying after
    // it, would put the old name or colour back here.
    let _sync = SyncGuard::claim_within(engine, &account.id, MUTATION_WAIT)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    let refused = if account.google || calendar.read_only {
        true
    } else {
        match account_client(engine, &account)?.update_calendar(&calendar.url, name, color) {
            Ok(()) => false,
            Err(err) if err.is::<super::PropertyRefused>() => true,
            Err(err) => return Err(err),
        }
    };
    let conn = lock(engine)?;
    if refused {
        if name.is_some() {
            bail!("this calendar's server does not let it be renamed");
        }
        store::set_local_color(&conn, calendar_id, color.unwrap_or_default())?;
    } else {
        store::set_calendar_props(&conn, calendar_id, name, color)?;
    }
    Ok(json!({ "accounts": accounts_payload(&conn)?, "local": refused }))
}

/// Delete a calendar on its server, with every event in it, for every
/// device and everyone it is shared with; then forget it here.
pub fn delete_calendar(engine: &Engine, calendar_id: &str) -> Result<Value> {
    let (calendar, account) = {
        let conn = lock(engine)?;
        let calendar = store::calendar(&conn, calendar_id)?
            .ok_or_else(|| anyhow!("no such calendar: {calendar_id}"))?;
        let account = store::account(&conn, &calendar.account)?
            .ok_or_else(|| anyhow!("no such CalDAV account: {}", calendar.account))?;
        (calendar, account)
    };
    if account.google {
        bail!("delete calendars in Google Calendar");
    }
    // A sync halfway through would bring the calendar's events back. Waited
    // for before the server is asked, and not for long, so a busy account
    // fails here rather than after the calendar is already gone.
    let _sync = SyncGuard::claim_within(engine, &account.id, MUTATION_WAIT)
        .ok_or_else(|| anyhow!("a sync of this account is still running; try again"))?;
    match account_client(engine, &account)?.delete_calendar(&calendar.url) {
        Ok(()) => {}
        Err(err) if err.is::<super::PropertyRefused>() => {
            bail!("this calendar's server does not let it be deleted")
        }
        Err(err) => return Err(err),
    }
    let conn = lock(engine)?;
    store::delete_calendar(&conn, calendar_id)?;
    Ok(json!({ "accounts": accounts_payload(&conn)? }))
}

/// Sync one account, or all of them. An account already syncing is waited
/// for and then synced again, so what was changed before this call is sent.
/// The report counts what every account did and carries the first failure.
///
/// `automatic`: a sync no one asked for (the calendar opening) leaves out
/// the accounts set to sync only when asked.
pub fn sync(engine: &Engine, account_id: Option<&str>, automatic: bool) -> Result<Report> {
    let accounts = store::accounts(&*lock(engine)?)?;
    let mut total = Report::default();
    for account in accounts
        .iter()
        .filter(|a| account_id.is_none_or(|id| a.id == id))
        .filter(|a| !automatic || a.sync_interval > 0)
    {
        let report = match SyncGuard::claim(engine, &account.id) {
            None => Report {
                error: "a sync of this account is still running".into(),
                ..Default::default()
            },
            Some(_guard) => {
                // A sync we waited for may have been throttled. Re-read under
                // the guard before making even a token refresh request.
                let Some(account) = store::account(&*lock(engine)?, &account.id)? else {
                    continue;
                };
                if let Some(error) = account.cooldown_error(crate::store::now_unix()) {
                    Report {
                        error,
                        ..Default::default()
                    }
                } else {
                    match account_client(engine, &account) {
                        Ok(client) => sync::sync_account(&*engine.db, &client, &account.id),
                        Err(err) => {
                            let error = format!("{err:#}");
                            let _ = store::record_sync(&*lock(engine)?, &account.id, &error);
                            Report {
                                error,
                                ..Default::default()
                            }
                        }
                    }
                }
            }
        };
        total.pulled += report.pulled;
        total.pushed += report.pushed;
        if total.error.is_empty() && !report.error.is_empty() {
            let name = account_identity(&*lock(engine)?, &account).name;
            total.error = format!("{name}: {}", report.error);
        }
    }
    Ok(total)
}

/// What an account goes by. A Google account the user has not named takes
/// its mailbox's name and picture, or its address without a mailbox.
pub(crate) struct AccountIdentity {
    pub name: String,
    pub avatar_url: String,
}

/// Google connections made before accounts could go unnamed all saved this.
const OLD_GOOGLE_DEFAULT_NAME: &str = "Google Calendar";

pub(crate) fn account_identity(
    conn: &rusqlite::Connection,
    account: &DavAccount,
) -> AccountIdentity {
    let saved = account.display_name.trim();
    if !account.google {
        let name = if saved.is_empty() {
            account.server_url.as_str()
        } else {
            saved
        };
        return AccountIdentity {
            name: name.to_string(),
            avatar_url: String::new(),
        };
    }
    let (mailbox_name, avatar_url) = conn
        .query_row(
            "SELECT display_name, avatar_url FROM accounts
             WHERE lower(trim(email)) = lower(trim(?1)) ORDER BY sort_order LIMIT 1",
            [&account.username],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap_or_default();
    let name = if !saved.is_empty() && saved != OLD_GOOGLE_DEFAULT_NAME {
        saved.to_string()
    } else if !mailbox_name.trim().is_empty() {
        mailbox_name.trim().to_string()
    } else {
        account.username.clone()
    };
    AccountIdentity { name, avatar_url }
}

/// Google's primary calendar is named after the account's address, which
/// reads better as the account's own name.
fn calendar_name(
    account: &DavAccount,
    identity: &AccountIdentity,
    calendar: &DavCalendar,
) -> String {
    if is_primary(account, calendar) {
        identity.name.clone()
    } else {
        calendar.name.clone()
    }
}

/// The calendar Google keeps at the account's own collection.
fn is_primary(account: &DavAccount, calendar: &DavCalendar) -> bool {
    account.google && super::same_google_collection_str(&calendar.url, &account.server_url)
}

/// Every account with its calendars, for settings.
pub fn accounts_payload(conn: &rusqlite::Connection) -> Result<Value> {
    let mut out = Vec::new();
    for account in store::accounts(conn)? {
        let identity = account_identity(conn, &account);
        let calendars: Vec<Value> = store::calendars(conn, &account.id)?
            .iter()
            .map(|calendar| {
                json!({
                    "id": calendar.id,
                    "name": calendar_name(&account, &identity, calendar),
                    "primary": is_primary(&account, calendar),
                    "color": calendar.shown_color(),
                    "read_only": calendar.read_only,
                    "enabled": calendar.enabled,
                    "events": calendar.supports("VEVENT"),
                    "tasks": calendar.supports("VTODO"),
                })
            })
            .collect();
        out.push(json!({
            "id": account.id,
            "server_url": account.server_url,
            "username": account.username,
            "google": account.google,
            "display_name": account.display_name,
            "name": identity.name,
            "avatar_url": identity.avatar_url,
            "last_sync_at": account.last_sync_at,
            "last_error": account.last_error,
            "discovery_warning": if account.google_calendar_list_disabled {
                super::GOOGLE_CALENDAR_LIST_DISABLED
            } else {
                ""
            },
            "sync_interval": account.sync_interval,
            "calendars": calendars,
        }));
    }
    Ok(json!(out))
}

/// The calendars events can be shown from and put in: every enabled event
/// calendar, with `writable` for the editor's picker and `color` for the
/// view.
pub fn calendars_payload(conn: &rusqlite::Connection) -> Result<Value> {
    let accounts: Vec<_> = store::accounts(conn)?
        .into_iter()
        .map(|account| {
            let identity = account_identity(conn, &account);
            (account, identity)
        })
        .collect();
    let calendars: Vec<Value> = store::all_calendars(conn)?
        .iter()
        .filter(|c| c.enabled && c.supports("VEVENT"))
        .map(|calendar| {
            let account = accounts.iter().find(|(a, _)| a.id == calendar.account);
            json!({
                "id": calendar.id,
                "name": account.map_or_else(|| calendar.name.clone(), |(a, i)| calendar_name(a, i, calendar)),
                "color": calendar.shown_color(),
                "writable": !calendar.read_only,
                "account_id": calendar.account,
                "account_name": account.map(|(_, i)| i.name.as_str()).unwrap_or_default(),
                "account_avatar_url": account.map(|(_, i)| i.avatar_url.as_str()).unwrap_or_default(),
                "account_google": account.is_some_and(|(a, _)| a.google),
            })
        })
        .collect();
    Ok(json!({ "calendars": calendars }))
}

fn client(server: &str, username: &str, password: &str) -> Result<Client> {
    Client::new(server, username, password, crate::proxy::global().as_ref())
}

fn remember_password(engine: &Engine, account_id: &str, password: &str) {
    remember_credentials(
        engine,
        account_id,
        Credentials {
            secrets: Secrets {
                password: password.to_string(),
                ..Default::default()
            },
            ..Default::default()
        },
    );
}

fn remember_credentials(engine: &Engine, account_id: &str, credentials: Credentials) {
    if let Ok(mut cache) = engine.dav_credentials.lock() {
        cache.insert(account_id.to_string(), credentials);
    }
}

fn cached_credentials(engine: &Engine, account_id: &str) -> Option<Credentials> {
    engine.dav_credentials.lock().ok()?.get(account_id).cloned()
}

fn credentials(engine: &Engine, account_id: &str) -> Result<Credentials> {
    if let Some(credentials) = cached_credentials(engine, account_id) {
        return Ok(credentials);
    }
    let conn = lock(engine)?;
    if let Some(credentials) = cached_credentials(engine, account_id) {
        return Ok(credentials);
    }
    let credentials = Credentials {
        secrets: engine.host.load_secret(&conn, account_id),
        ..Default::default()
    };
    if !credentials.secrets.is_empty() {
        remember_credentials(engine, account_id, credentials.clone());
    }
    Ok(credentials)
}

struct SyncGuard<'a> {
    engine: &'a Engine,
    account: String,
}

impl<'a> SyncGuard<'a> {
    /// Wait for any sync of `account` in flight, then claim it. `None` when
    /// it is still running after the longest a sync should take; only a
    /// claimed guard releases the account.
    fn claim(engine: &'a Engine, account: &str) -> Option<Self> {
        Self::claim_within(engine, account, std::time::Duration::from_secs(300))
    }

    /// [`claim`](Self::claim), giving up after `wait`.
    fn claim_within(engine: &'a Engine, account: &str, wait: std::time::Duration) -> Option<Self> {
        let deadline = std::time::Instant::now() + wait;
        loop {
            let claimed = engine.dav_syncing.lock().ok()?.insert(account.to_string());
            if claimed {
                return Some(Self {
                    engine,
                    account: account.to_string(),
                });
            }
            if std::time::Instant::now() > deadline {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

impl Drop for SyncGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut syncing) = self.engine.dav_syncing.lock() {
            syncing.remove(&self.account);
        }
    }
}
