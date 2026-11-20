//! Address book sync and online contact edits, separate from mail and local notes.
mod client;
mod editing;

use anyhow::{Context, Result, anyhow, ensure};
use calcard::vcard::{VCard, VCardProperty, VCardValue};
use client::Client;
use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::MutexGuard,
};

use crate::{
    engine::{Engine, default_oauth_client},
    oauth::{self, GOOGLE_CONTACTS_PROVIDER},
    secrets::Secrets,
};

struct Card {
    url: String,
    etag: String,
    name: String,
    emails: BTreeSet<String>,
    invalid: bool,
}

impl Card {
    fn parse(url: &str, etag: &str, text: &str) -> Result<Self> {
        ensure!(
            text.trim()
                .lines()
                .last()
                .is_some_and(|line| line.eq_ignore_ascii_case("END:VCARD")),
            "server returned an incomplete vCard"
        );
        let vcard = VCard::parse(text).map_err(|_| anyhow!("server returned an invalid vCard"))?;
        let name = display_name(vcard.entries.iter());
        let mut emails = BTreeSet::new();
        for entry in vcard.entries {
            for value in entry.values {
                let VCardValue::Text(value) = value else {
                    continue;
                };
                let value = value.trim();
                match entry.name {
                    VCardProperty::Email if editing::valid_email(value) => {
                        if !emails
                            .iter()
                            .any(|email: &String| email.to_lowercase() == value.to_lowercase())
                        {
                            emails.insert(value.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(Self {
            url: url.into(),
            etag: etag.into(),
            name,
            emails,
            invalid: false,
        })
    }

    fn invalid(url: &str, etag: &str) -> Self {
        Self {
            url: url.into(),
            etag: etag.into(),
            name: String::new(),
            emails: BTreeSet::new(),
            invalid: true,
        }
    }
}

fn display_name<'a>(entries: impl IntoIterator<Item = &'a calcard::vcard::VCardEntry>) -> String {
    entries
        .into_iter()
        .filter(|entry| entry.name == VCardProperty::Fn)
        .flat_map(|entry| &entry.values)
        .filter_map(|value| match value {
            VCardValue::Text(value) => Some(value.split_whitespace().collect::<Vec<_>>().join(" ")),
            _ => None,
        })
        .find(|name| !name.is_empty())
        .unwrap_or_default()
}

#[derive(Clone)]
struct Book {
    url: String,
    name: String,
}

#[derive(Serialize)]
struct SavedBook {
    id: String,
    #[serde(flatten)]
    book: BookLabel,
    enabled: bool,
    skipped: i64,
    #[serde(skip)]
    sync_token: String,
}

#[derive(Serialize)]
struct BookLabel {
    name: String,
    #[serde(skip)]
    url: String,
}

#[derive(Serialize)]
struct Account {
    id: String,
    server: String,
    username: String,
    google: bool,
    books: Vec<SavedBook>,
}

fn accounts(db: &Connection) -> Result<Vec<Account>> {
    let mut stmt = db.prepare(
        "SELECT id, server, username, google FROM carddav_accounts ORDER BY username, server",
    )?;
    let mut books = db.prepare(
        "SELECT id, url, name, enabled, sync_token, (SELECT count(*) FROM carddav_cards WHERE book=carddav_books.id AND invalid) FROM carddav_books WHERE account=?1 ORDER BY name, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Account {
            id: r.get(0)?,
            server: r.get(1)?,
            username: r.get(2)?,
            google: r.get(3)?,
            books: Vec::new(),
        })
    })?;
    rows.map(|row| {
        let mut row = row?;
        row.books = books
            .query_map([&row.id], |r| {
                Ok(SavedBook {
                    id: r.get(0)?,
                    book: BookLabel {
                        url: r.get(1)?,
                        name: r.get(2)?,
                    },
                    enabled: r.get(3)?,
                    sync_token: r.get(4)?,
                    skipped: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(row)
    })
    .collect()
}

fn lock(engine: &Engine) -> Result<MutexGuard<'_, Connection>> {
    engine.db.lock().map_err(|_| anyhow!("store lock poisoned"))
}

pub fn dispatch(engine: &Engine, method: &str, p: &Value) -> Result<Value> {
    let required = |key: &str| {
        p.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .with_context(|| format!("missing {key}"))
    };
    if method == "carddav.accounts" {
        return Ok(json!({"accounts": accounts(&*lock(engine)?)?}));
    }
    if method == "carddav.contacts" {
        return editing::list(&*lock(engine)?, required("book_id")?, p);
    }
    // One mutation at a time; never hold the database mutex during a DAV request.
    let _guard = engine
        .carddav_operation
        .try_lock()
        .map_err(|_| anyhow!("address book operation already in progress"))?;
    let result = match method {
        "carddav.add" => {
            let server = client::server_url(required("server_url")?)?;
            ensure!(
                server.as_str() != client::GOOGLE_DISCOVERY,
                "Use Connect Google Contacts for this server"
            );
            let username = required("username")?.trim();
            ensure!(
                !username.is_empty() && !username.contains(':'),
                "invalid CardDAV username"
            );
            let password = required("password")?;
            let client = Client::basic(server, username, password)?;
            let found = client.discover()?;
            let id = save_account(
                engine,
                &client,
                username,
                Secrets {
                    password: password.into(),
                    ..Default::default()
                },
                &found,
                false,
            )?;
            json!({"account_id": id})
        }
        "carddav.addGoogle" => {
            let email = required("email")?.trim().to_ascii_lowercase();
            ensure!(
                email.contains('@'),
                "Google sign-in did not return an email address"
            );
            if let Some(id) = p["account_id"].as_str().filter(|id| !id.is_empty()) {
                ensure!(
                    accounts(&*lock(engine)?)?
                        .iter()
                        .any(|account| account.id == id
                            && account.google
                            && account.username == email),
                    "Sign in with the Google Contacts account you are reconnecting"
                );
            }
            let client = Client::google(required("access_token")?)?;
            let refresh = required("refresh_token")?;
            let found = client.discover()?;
            let id = save_account(
                engine,
                &client,
                &email,
                Secrets {
                    refresh_token: Some(refresh.into()),
                    ..Default::default()
                },
                &found,
                true,
            )?;
            let account = accounts(&*lock(engine)?)?
                .into_iter()
                .find(|a| a.id == id)
                .context("address book account not found")?;
            let warning = sync_account(engine, &client, account)
                .err()
                .map(|err| format!("{err:#}"));
            json!({"account_id": id, "sync_error": warning})
        }
        "carddav.remove" => {
            let id = required("account_id")?;
            let db = lock(engine)?;
            ensure!(
                db.execute("DELETE FROM carddav_accounts WHERE id=?1", [id])? == 1,
                "address book account not found"
            );
            engine.host.delete_secret(&db, id)?;
            engine.carddav_secrets.lock().unwrap().remove(id);
            json!({})
        }
        "carddav.setBookEnabled" => {
            let db = lock(engine)?;
            let enabled = p["enabled"].as_bool().context("missing enabled")?;
            ensure!(
                db.execute(
                    "UPDATE carddav_books SET enabled=?1 WHERE id=?2",
                    params![enabled, required("book_id")?]
                )? == 1,
                "address book not found"
            );
            json!({})
        }
        "carddav.sync" => {
            let requested = required("account_id")?;
            let account = accounts(&*lock(engine)?)?
                .into_iter()
                .find(|a| a.id == requested)
                .context("address book account not found")?;
            let client = account_client(engine, &account)?;
            let changed = sync_account(engine, &client, account)?;
            json!({"changed": changed})
        }
        "carddav.createContact" | "carddav.updateContact" | "carddav.deleteContact" => {
            editing::mutate(engine, required("book_id")?, method, p)?
        }
        _ => return Err(anyhow!("unknown address book command")),
    };
    Ok(result)
}

fn account_client(engine: &Engine, account: &Account) -> Result<Client> {
    let requested = account.id.as_str();
    let secret = {
        let db = lock(engine)?;
        let cached = engine
            .carddav_secrets
            .lock()
            .unwrap()
            .get(requested)
            .cloned();
        cached.unwrap_or_else(|| engine.host.load_secret(&db, requested))
    };
    if account.google {
        let refresh = secret
            .refresh_token
            .as_deref()
            .context("Reconnect Google Contacts to resume syncing")?;
        let oauth = default_oauth_client(GOOGLE_CONTACTS_PROVIDER);
        let tokens = oauth::refresh_google_dav(&oauth, refresh).map_err(|err| {
            if err.contains("invalid_grant") {
                anyhow!("Reconnect Google Contacts to resume syncing")
            } else {
                anyhow!("Could not refresh Google Contacts authorization. Try again.")
            }
        })?;
        if let Some(token) = tokens
            .refresh_token
            .filter(|token| !token.is_empty() && token != refresh)
        {
            let secret = Secrets {
                refresh_token: Some(token),
                ..Default::default()
            };
            engine
                .host
                .store_secret(&*lock(engine)?, requested, &secret)?;
            engine
                .carddav_secrets
                .lock()
                .unwrap()
                .insert(requested.into(), secret);
        }
        Client::google(&tokens.access_token)
    } else {
        ensure!(
            !secret.password.is_empty(),
            "CardDAV password is missing; connect the account again"
        );
        Client::basic(
            client::server_url(&account.server).map_err(|err| anyhow!("{err}. For a saved HTTP account, enable HTTPS on the server and connect using its HTTPS URL."))?,
            &account.username,
            &secret.password,
        )
    }
}

fn save_account(
    engine: &Engine,
    client: &Client,
    username: &str,
    secrets: Secrets,
    books: &[Book],
    google: bool,
) -> Result<String> {
    let mut db = lock(engine)?;
    let tx = db.transaction()?;
    let id: String = tx.query_row(
        "INSERT INTO carddav_accounts(id, server, username, google) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(server, username) DO UPDATE SET google=excluded.google RETURNING id",
        params![
            crate::tasks::new_id("carddav"),
            client.server.as_str(),
            username,
            google,
        ],
        |r| r.get(0),
    )?;
    for book in books {
        tx.execute(
            "INSERT INTO carddav_books(id, account, url, name) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(account, url) DO UPDATE SET name=excluded.name",
            params![crate::tasks::new_id("addressbook"), id, book.url, book.name],
        )?;
    }
    engine.host.store_secret(&tx, &id, &secrets)?;
    tx.commit()?;
    engine
        .carddav_secrets
        .lock()
        .unwrap()
        .insert(id.clone(), secrets);
    Ok(id)
}

fn sync_account(engine: &Engine, client: &Client, account: Account) -> Result<usize> {
    let mut changed = 0;
    for book in account.books.into_iter().filter(|book| book.enabled) {
        changed += sync_book(engine, client, &book, account.google)?;
    }
    Ok(changed)
}

fn sync_book(engine: &Engine, client: &Client, saved: &SavedBook, google: bool) -> Result<usize> {
    let known: BTreeMap<String, String> = {
        let db = lock(engine)?;
        let mut stmt = db.prepare("SELECT url, etag FROM carddav_cards WHERE book=?1")?;
        stmt.query_map([&saved.id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let book = Book {
        url: saved.book.url.clone(),
        name: saved.book.name.clone(),
    };
    let incremental = if google && !saved.sync_token.is_empty() {
        client.incremental(&book, &saved.sync_token, &known)?
    } else {
        None
    };
    let (versions, cards, token) = match incremental {
        Some(delta) => delta,
        None => {
            // Take the token first, so changes racing the full listing will
            // still be replayed by the next sync-collection report.
            let token = if google {
                client.sync_token(&book)?
            } else {
                String::new()
            };
            let (versions, cards) = client.snapshot(&book, &known)?;
            (versions, cards, token)
        }
    };
    let mut db = lock(engine)?;
    let tx = db.transaction()?;
    let mut count = cards.len();
    for url in known.keys().filter(|url| !versions.contains_key(*url)) {
        count += tx.execute(
            "DELETE FROM carddav_cards WHERE book=?1 AND url=?2",
            params![saved.id, url],
        )?;
    }
    for card in cards {
        tx.execute("INSERT INTO carddav_cards(book, url, etag, name, emails, invalid) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(book, url) DO UPDATE SET etag=excluded.etag, name=excluded.name, emails=excluded.emails, invalid=excluded.invalid",
            params![saved.id, card.url, card.etag, card.name, serde_json::to_string(&card.emails)?, card.invalid])?;
    }
    tx.execute(
        "UPDATE carddav_books SET sync_token=?1 WHERE id=?2",
        params![token, saved.id],
    )?;
    tx.commit()?;
    Ok(count)
}

pub(crate) fn contacts(db: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = db.prepare(
        "SELECT card.name, email.value FROM carddav_cards card
        JOIN carddav_books book ON book.id=card.book, json_each(card.emails) email
        WHERE book.enabled ORDER BY card.name, email.value",
    )?;
    Ok(stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}
