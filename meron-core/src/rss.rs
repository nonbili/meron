//! RSS/Atom engine: feed fetch (blocking `ureq`), parsing (`feed-rs`), HTML feed
//! discovery + text cleaning (`scraper`), and persistence into the shared unified
//! tables (`accounts`, `subscriptions`, `messages`).
//!
//! Ported from the bridge's former Go `RSSStore`. Identity hashing (account id,
//! subscription id, item key) mirrors the Go implementation byte-for-byte —
//! sha256, first 12 bytes, URL-safe base64 without padding — so feeds and items
//! imported from the old `rss.sqlite` keep stable keys and don't duplicate on the
//! next sync.
//!
//! Read paths return final bridge-shaped `Message`/`Folder` JSON (the desktop
//! bridge passes them straight through), unlike the mail path which returns raw
//! rows the bridge formats.

use anyhow::{Result, anyhow};
use base64::Engine as _;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

use crate::search_query::{SearchQuery, local_midnight};
use crate::store::{self, RssItemExtra};

mod fetch;
#[cfg(test)]
use fetch::discover_feed_url;
use fetch::fetch_feed;
mod parse;
#[cfg(test)]
use parse::{clean_feed_text, extract_image_urls, extract_video_urls, parse_item};
use parse::{image_filename, image_mime, parse_items, text_content, video_mime};
mod media;
#[cfg(test)]
use media::sniff_image_ext;
use media::{cache_feed_icon, cache_feed_images, key_is_image, prune_feed_media};

const RSS_FOLDER_ID: &str = "inbox";
const DEFAULT_RSS_ACCOUNT_TITLE: &str = "RSS";

/// A subscription as needed for sync and presentation.
struct Subscription {
    id: String,
    account: String,
    url: String,
    etag: String,
    last_modified: String,
}

struct ParsedItem {
    item_key: String,
    title: String,
    unread: bool,
    /// Raw HTML body of the entry (content, else summary), for the reader's HTML
    /// mode. `None` when the feed gave no HTML. Stored in message `json`.
    content_html: Option<String>,
    extra: RssItemExtra,
}

/// A feed entry that landed in the store for the first time during a sync.
/// Carries everything a "new items" notification needs, so callers never have to
/// guess from the store which item triggered the sync — an item's `date` is its
/// *published* date, so the newest arrival is often not the newest-dated row.
pub struct NewItem {
    /// Subscription id, which is both the item's `folder` and the feed thread's
    /// key (`<account>#rss#<sub>`) that a notification tap should open.
    pub subscription_id: String,
    /// The entry's stable key (its guid/link hash). A feed is one thread, so the
    /// subscription id does not tell two of its entries apart — this does, and
    /// it is what keeps two same-titled arrivals from sharing a notification.
    pub item_key: String,
    /// The feed's display title, used as the notification's "sender".
    pub feed_title: String,
    pub title: String,
    /// Body snippet for the notification, empty when the entry carried no text.
    pub preview: String,
    /// Item date as epoch seconds, matching the stored `date` column.
    pub date: i64,
}

// ---- Public API -------------------------------------------------------------

/// Create a new RSS account (fresh stable id) and, if a feed URL is supplied,
/// add it as the account's first feed. Returns the bridge `Account` JSON. Each
/// call creates a *distinct* account — no name-based grouping — so users can have
/// multiple RSS accounts (even same-named) and rename them later.
pub fn add(db: &Mutex<Connection>, feed_url: &str, display_name: &str) -> Result<Value> {
    let account_title = {
        let t = display_name.trim();
        if t.is_empty() {
            DEFAULT_RSS_ACCOUNT_TITLE.to_string()
        } else {
            t.to_string()
        }
    };
    let account_id = new_account_id();
    let feed_url = feed_url.trim();
    // Fetch the first feed (if any) *before* writing, so a bad URL doesn't leave
    // an empty account behind.
    let parsed = if feed_url.is_empty() {
        None
    } else {
        Some(fetch_parsed(feed_url)?)
    };
    // A fresh RSS account defaults to images-on; we defer caching images to the
    // background sync so the account addition is fast and doesn't block/timeout.

    let now = now_unix();
    let conn = db.lock().unwrap();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config, created_at, updated_at, sort_order)
         VALUES(?1, 'rss', 'rss', ?2, '{}', ?3, ?3, COALESCE((SELECT MAX(sort_order) + 1 FROM accounts), 0))
         ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name, updated_at = excluded.updated_at",
        params![account_id, account_title, now],
    )?;
    if let Some(parsed) = parsed {
        store_parsed_feed(&tx, &account_id, &parsed)?;
    }
    tx.commit()?;
    Ok(account_json(&account_id, &account_title))
}

/// Add a feed to an existing RSS account. Fetches + parses (with HTML discovery
/// fallback) before writing the subscription and its items. Returns the resolved
/// feed title.
pub fn add_feed(db: &Mutex<Connection>, account: &str, feed_url: &str) -> Result<Value> {
    {
        let conn = db.lock().unwrap();
        match store::account_engine(&conn, account)?.as_deref() {
            Some("rss") => {}
            Some(other) => return Err(anyhow!("account {account} is not RSS (engine={other})")),
            None => return Err(anyhow!("unknown account: {account}")),
        }
    }
    let parsed = fetch_parsed(feed_url)?;
    let feed_title = parsed.feed_title.clone();
    // Defer caching images to the background sync so the feed addition is fast
    // and doesn't block/timeout.
    let conn = db.lock().unwrap();
    let tx = conn.unchecked_transaction()?;
    store_parsed_feed(&tx, account, &parsed)?;
    tx.commit()?;
    Ok(json!({ "ok": true, "feed_title": feed_title }))
}

/// Remove a single feed (subscription) and its cached items from an RSS account.
/// Takes the subscription's thread id ("<account>#rss#<sub>"), the same id the
/// UI carries for the feed's thread row. The account itself is left in place.
pub fn remove_feed(conn: &Connection, thread_id: &str) -> Result<Value> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    let tx = conn.unchecked_transaction()?;
    // RSS items are stored with the subscription id as their `folder`.
    tx.execute(
        "DELETE FROM messages WHERE account = ?1 AND folder = ?2",
        params![account, sub_id],
    )?;
    tx.execute(
        "DELETE FROM subscriptions WHERE id = ?1 AND account = ?2",
        params![sub_id, account],
    )?;
    tx.commit()?;
    Ok(json!({ "ok": true }))
}

/// Move a feed subscription and its cached items to another RSS account. This is
/// a metadata reassignment, not a delete/re-add, so read/starred item state and
/// sync metadata stay intact.
pub fn move_feed(conn: &Connection, thread_id: &str, target_account: &str) -> Result<Value> {
    let (source_account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    let target_account = target_account.trim();
    if target_account.is_empty() {
        return Err(anyhow!("target account required"));
    }

    match store::account_engine(conn, target_account)?.as_deref() {
        Some("rss") => {}
        Some(other) => {
            return Err(anyhow!(
                "account {target_account} is not RSS (engine={other})"
            ));
        }
        None => return Err(anyhow!("unknown account: {target_account}")),
    }

    let exists = conn
        .query_row(
            "SELECT 1 FROM subscriptions WHERE id = ?1 AND account = ?2",
            params![sub_id, source_account],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists {
        return Err(anyhow!("unknown feed: {thread_id}"));
    }

    if source_account == target_account {
        return Ok(json!({
            "ok": true,
            "moved": 0,
            "thread_id": thread_id,
            "source_account_id": source_account,
            "target_account_id": target_account
        }));
    }

    let tx = conn.unchecked_transaction()?;
    let now = now_unix();
    tx.execute(
        "UPDATE subscriptions SET account = ?3, updated_at = ?4 WHERE id = ?1 AND account = ?2",
        params![sub_id, source_account, target_account, now],
    )?;
    let moved = tx.execute(
        "UPDATE messages SET account = ?3 WHERE account = ?1 AND folder = ?2",
        params![source_account, sub_id, target_account],
    )?;
    tx.commit()?;

    Ok(json!({
        "ok": true,
        "moved": moved,
        "thread_id": format_thread_id(target_account, &sub_id),
        "source_account_id": source_account,
        "target_account_id": target_account
    }))
}

/// Give a feed the name it is listed under, or with an empty `title` hand the
/// name back to the feed's own. A sync only fills in a missing title, so the
/// name set here stays until it is changed again.
pub fn rename_feed(conn: &Connection, thread_id: &str, title: &str) -> Result<Value> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    // One line: the name is a list label, and a pasted line break would
    // otherwise end up in notifications and OPML exports.
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let (url, feed_title): (String, String) = conn
        .query_row(
            "SELECT url, feed_title FROM subscriptions WHERE id = ?1 AND account = ?2",
            params![sub_id, account],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("unknown feed: {thread_id}"))?;
    // Restoring falls back to the host label, as adding a feed does, so a feed
    // with no title of its own is never left without a name.
    let title = if !title.is_empty() {
        title
    } else if !feed_title.trim().is_empty() {
        feed_title.trim().to_string()
    } else {
        feed_host_label(&url)
    };
    conn.execute(
        "UPDATE subscriptions SET title = ?3, updated_at = ?4 WHERE id = ?1 AND account = ?2",
        params![sub_id, account, title, now_unix()],
    )?;
    Ok(json!({ "ok": true, "title": title }))
}

mod opml;

pub use opml::{export_opml, import_opml};

/// A fetched + parsed feed, ready to persist (no DB lock held during the fetch).
struct FetchedParsed {
    resolved_url: String,
    feed_title: String,
    site_url: String,
    /// Feed-supplied icon/logo URL (favicon preferred over banner logo), empty
    /// when the feed declared neither. Stored in the subscription `json`.
    icon_url: String,
    items: Vec<ParsedItem>,
}

/// The best icon URL a feed declares for itself: prefer `<icon>` (a square
/// favicon, what an avatar wants) over `<logo>` (a wide banner), empty when
/// neither is present.
fn feed_icon_url(feed: &feed_rs::model::Feed) -> String {
    feed.icon
        .as_ref()
        .or(feed.logo.as_ref())
        .map(|img| img.uri.trim().to_string())
        .unwrap_or_default()
}

/// Serialize a subscription's `json` extras column: the feed-declared icon URL
/// and, once downloaded, its cached media key. New per-feed metadata can be
/// added as fields here without a schema migration.
fn subscription_json(icon_url: &str, icon_key: &str) -> String {
    json!({ "icon_url": icon_url, "icon_key": icon_key }).to_string()
}

fn fetch_parsed(feed_url: &str) -> Result<FetchedParsed> {
    let normalized = normalize_feed_url(feed_url)?;
    let fetched = fetch_feed(&normalized, "", "")?;
    let feed = fetched
        .feed
        .ok_or_else(|| anyhow!("feed returned no content"))?;
    let resolved = fetched.resolved_url;
    let feed_title = {
        let t = text_content(&feed.title);
        if t.trim().is_empty() {
            feed_host_label(&resolved)
        } else {
            t.trim().to_string()
        }
    };
    let site_url = feed
        .links
        .first()
        .map(|l| l.href.clone())
        .unwrap_or_default();
    let icon_url = feed_icon_url(&feed);
    let items = parse_items(&feed, now_unix());
    Ok(FetchedParsed {
        resolved_url: resolved,
        feed_title,
        site_url,
        icon_url,
        items,
    })
}

fn store_parsed_feed(tx: &Connection, account: &str, parsed: &FetchedParsed) -> Result<()> {
    let sub_id = rss_subscription_id(&parsed.resolved_url);
    let now = now_unix();
    tx.execute(
        "INSERT INTO subscriptions(id, account, url, title, site_url, feed_title, json, enabled, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)
         ON CONFLICT(id) DO UPDATE SET
           account = excluded.account,
           url = excluded.url,
           -- A name the user gave the feed survives adding it again; a title
           -- that was only ever the feed's own follows the fresh one.
           title = CASE WHEN subscriptions.title = '' OR subscriptions.title = subscriptions.feed_title
                        THEN excluded.title ELSE subscriptions.title END,
           site_url = excluded.site_url,
           feed_title = excluded.feed_title,
           json = excluded.json,
           enabled = 1,
           updated_at = excluded.updated_at",
        params![
            sub_id,
            account,
            parsed.resolved_url,
            parsed.feed_title,
            parsed.site_url,
            parsed.feed_title,
            subscription_json(&parsed.icon_url, ""),
            now
        ],
    )?;
    for item in &parsed.items {
        // Initial feed load: store items, but don't treat them as "new" arrivals.
        store::upsert_rss_item(
            tx,
            account,
            &sub_id,
            &item.item_key,
            &item.title,
            item.unread,
            item.content_html.as_deref(),
            &item.extra,
        )?;
    }
    Ok(())
}

/// Re-fetch every enabled subscription of an account (conditional GET via
/// stored ETag / Last-Modified), upserting new items and refreshing sub state.
/// Returns the genuinely new items stored across all of the account's feeds,
/// newest first, so the caller can raise a "new items" notification that names
/// the entries that actually arrived.
pub fn sync_account(db: &Mutex<Connection>, account: &str) -> Result<Vec<NewItem>> {
    let subs = {
        let conn = crate::log::timed_db_lock(db, "rss.sync.load_subscriptions");
        load_subscriptions(&conn, account)?
    };
    let mut new_items: Vec<NewItem> = Vec::new();
    for sub in subs {
        match sync_subscription(db, &sub) {
            Ok(items) => new_items.extend(items),
            Err(e) => {
                let conn = crate::log::timed_db_lock(db, "rss.sync.save_error");
                let _ = conn.execute(
                    "UPDATE subscriptions SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
                    params![sub.id, format!("{e:#}"), now_unix()],
                );
            }
        }
    }
    prune_feed_media(&crate::parse::media_root());
    new_items.sort_by_key(|item| std::cmp::Reverse(item.date));
    Ok(new_items)
}

/// The `mail.newMessages` payload for a batch of feed arrivals, shaped exactly
/// like the mail one ([`crate::mail_model::new_messages_detail`]): the top-level
/// `from`/`subject`/`preview`/`threadKey` describe the newest arrival for
/// single-notification clients (desktop), and `messages` carries one entry per
/// arrival for clients that post a notification each. `items` must be newest
/// first, as [`sync_account`] returns them.
pub fn new_items_detail(
    account: &str,
    account_name: &str,
    muted: bool,
    items: &[NewItem],
) -> Option<Value> {
    let latest = items.first()?;
    let messages: Vec<Value> = items
        .iter()
        .take(crate::mail_model::NEW_MESSAGES_DETAIL_MAX)
        .map(|item| {
            json!({
                // RSS rows carry no IMAP uid; `itemKey` is what identifies one,
                // since a feed's entries all share its thread key.
                "uid": 0,
                "itemKey": item.item_key,
                "from": item.feed_title,
                "subject": item.title,
                "preview": item.preview,
                "threadKey": item.subscription_id,
                "date": item.date,
            })
        })
        .collect();
    Some(json!({
        "account": account,
        "accountName": account_name,
        "folder": RSS_FOLDER_ID,
        "count": items.len(),
        "muted": muted,
        "from": latest.feed_title,
        "subject": latest.title,
        "preview": latest.preview,
        // The subscription id: a feed is one thread, so this opens the feed the
        // newest arrival landed in.
        "threadKey": latest.subscription_id,
        "messages": messages,
    }))
}

/// Notification snippet for an item: its summary, else its content body.
fn item_preview(extra: &RssItemExtra) -> String {
    if !extra.summary.trim().is_empty() {
        first_line(&extra.summary)
    } else {
        first_line(&extra.content)
    }
}

/// Read an account's resolved "load remote images" preference under a short lock.
fn db_load_remote_images(db: &Mutex<Connection>, account: &str) -> bool {
    let conn = db.lock().unwrap();
    store::load_remote_images(&conn, account).unwrap_or(false)
}

fn sync_subscription(db: &Mutex<Connection>, sub: &Subscription) -> Result<Vec<NewItem>> {
    let now = now_unix();
    let fetched = fetch_feed(&sub.url, &sub.etag, &sub.last_modified)?;
    if fetched.not_modified {
        let conn = crate::log::timed_db_lock(db, "rss.sync.not_modified");
        conn.execute(
            "UPDATE subscriptions SET last_sync_at = ?2, last_error = '', updated_at = ?2 WHERE id = ?1",
            params![sub.id, now],
        )?;
        return Ok(Vec::new());
    }
    let feed = fetched
        .feed
        .ok_or_else(|| anyhow!("feed returned no content"))?;
    let mut items = parse_items(&feed, now);
    // Download inline images (off the DB lock) for image-enabled accounts so they
    // render from disk and never re-fetch from the origin on each open.
    if db_load_remote_images(db, &sub.account) {
        cache_feed_images(&sub.account, &sub.id, &mut items);
    }
    // The host label stands in for a missing feed title, as when the feed was
    // added, so restoring a renamed feed's own name never leaves it blank.
    let feed_title = match text_content(&feed.title).trim() {
        "" => feed_host_label(&fetched.resolved_url),
        t => t.to_string(),
    };
    let site_url = feed
        .links
        .first()
        .map(|l| l.href.clone())
        .unwrap_or_default();
    let icon_url = feed_icon_url(&feed);
    // Cache the feed's own icon to disk (off the DB lock) so the avatar renders
    // from /media with no remote fetch. Unconditional: it's first-party branding,
    // not the third-party content the remote-images gate is meant to hold back.
    let icon_key = cache_feed_icon(&sub.account, &sub.id, &icon_url).unwrap_or_default();
    let conn = crate::log::timed_db_lock(db, "rss.sync.persist_items");
    let tx = conn.unchecked_transaction()?;
    // Only a missing title is filled in. The row's current title is kept as it
    // is, not the copy read before the fetch, so a rename made while this sync
    // was fetching is not written over.
    let Some(title): Option<String> = tx
        .query_row(
            "UPDATE subscriptions
         SET title = CASE WHEN title = '' THEN ?2 ELSE title END, url = ?3, site_url = ?4,
             feed_title = ?2, json = ?5, etag = ?6, last_modified = ?7, last_sync_at = ?8,
             last_error = '', updated_at = ?8
         WHERE id = ?1
         RETURNING title",
            params![
                sub.id,
                feed_title,
                fetched.resolved_url,
                site_url,
                subscription_json(&icon_url, &icon_key),
                fetched.etag,
                fetched.last_modified,
                now
            ],
            |row| row.get(0),
        )
        .optional()?
    else {
        // Removed while it was being fetched: nothing left to store items under.
        return Ok(Vec::new());
    };
    let mut new_items = Vec::new();
    for item in &items {
        if store::upsert_rss_item(
            &tx,
            &sub.account,
            &sub.id,
            &item.item_key,
            &item.title,
            item.unread,
            item.content_html.as_deref(),
            &item.extra,
        )? {
            new_items.push(NewItem {
                subscription_id: sub.id.clone(),
                item_key: item.item_key.clone(),
                // The user's own title for the feed when they renamed it, so the
                // notification matches the thread row they will land on.
                feed_title: title.clone(),
                title: item.title.clone(),
                preview: item_preview(&item.extra),
                date: store::item_date_epoch(
                    item.extra.published_at,
                    item.extra.updated_at,
                    item.extra.fetched_at,
                ),
            });
        }
    }
    tx.commit()?;
    Ok(new_items)
}

/// One bridge `Message` per subscription (subscription = thread), newest first.
/// `filter` mirrors the mail thread list: "unread" keeps feeds with an unread
/// item, "starred" keeps feeds with a starred item, anything else keeps all.
///
/// `query` keeps feeds with an item that matches it, operators included, as
/// they read for a feed: `from:` is the feed or the item's author, `subject:`
/// the item's title. A feed item is addressed to no one, so `to:` matches none.
pub fn recent(
    conn: &Connection,
    account: &str,
    query: &str,
    filter: &str,
    limit: i64,
) -> Result<Vec<Value>> {
    let limit = if limit <= 0 { 50 } else { limit };
    let query = SearchQuery::parse(query);
    let mut sql = String::from(
        "SELECT s.id, s.account, s.url, s.title,
           COALESCE(MAX(NULLIF(json_extract(m.json,'$.published_at'),0)),
                    MAX(NULLIF(json_extract(m.json,'$.updated_at'),0)),
                    MAX(json_extract(m.json,'$.fetched_at')), 0) AS latest_at,
           COALESCE(SUM(CASE WHEN m.seen = 0 THEN 1 ELSE 0 END), 0) AS unread_count,
           EXISTS(SELECT 1 FROM messages sm
                    WHERE sm.account = s.account AND sm.folder = s.id AND sm.starred <> 0)
             AS has_starred_items,
           COALESCE((SELECT subject FROM messages lm WHERE lm.account = s.account AND lm.folder = s.id
              ORDER BY COALESCE(NULLIF(json_extract(lm.json,'$.published_at'),0),
                                NULLIF(json_extract(lm.json,'$.updated_at'),0),
                                json_extract(lm.json,'$.fetched_at')) DESC LIMIT 1), '') AS latest_title,
           COALESCE((SELECT json_extract(lm.json,'$.summary') FROM messages lm WHERE lm.account = s.account AND lm.folder = s.id
              ORDER BY COALESCE(NULLIF(json_extract(lm.json,'$.published_at'),0),
                                NULLIF(json_extract(lm.json,'$.updated_at'),0),
                                json_extract(lm.json,'$.fetched_at')) DESC LIMIT 1), '') AS latest_summary,
           s.json AS sub_json
         FROM subscriptions s
         LEFT JOIN messages m ON m.account = s.account AND m.folder = s.id
         WHERE s.account = ?1 AND s.enabled = 1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(account.to_string())];
    // Each text condition: any of `columns` holds any of `values`.
    let mut like = |columns: &[&str], values: &[String]| {
        let mut clauses = Vec::new();
        for value in values {
            let pattern = format!("%{}%", value.to_lowercase());
            for column in columns {
                args.push(Box::new(pattern.clone()));
                clauses.push(format!("lower({column}) LIKE ?"));
            }
        }
        sql.push_str(&format!(" AND ({})", clauses.join(" OR ")));
    };
    for text in &query.text {
        like(
            &["s.title", "m.subject", "json_extract(m.json,'$.summary')"],
            std::slice::from_ref(text),
        );
    }
    for from in &query.from {
        like(&["s.title", "m.from_name"], from);
    }
    for subject in &query.subject {
        like(&["m.subject"], subject);
    }
    if !query.to.is_empty() {
        sql.push_str(" AND 0");
    }
    if query.read {
        sql.push_str(" AND m.seen <> 0");
    }
    if query.unread {
        sql.push_str(" AND m.seen = 0");
    }
    if query.starred {
        sql.push_str(" AND m.starred <> 0");
    }
    // Tags are for mail: no feed item carries one.
    if !query.tags.is_empty() {
        sql.push_str(" AND 0");
    }
    // What an item's `has_attachments` counts: its inline images and videos.
    if query.has_attachment {
        sql.push_str(
            " AND (json_array_length(m.json,'$.images') > 0
                   OR json_array_length(m.json,'$.videos') > 0)",
        );
    }
    if let Some(after) = query.after {
        sql.push_str(" AND m.date >= ?");
        args.push(Box::new(local_midnight(after)));
    }
    if let Some(before) = query.before {
        sql.push_str(" AND m.date < ?");
        args.push(Box::new(local_midnight(before)));
    }
    sql.push_str(" GROUP BY s.id");
    match filter {
        "unread" => sql.push_str(" HAVING unread_count > 0"),
        "starred" => sql.push_str(" HAVING has_starred_items"),
        _ => {}
    }
    sql.push_str(" ORDER BY latest_at DESC, s.title COLLATE NOCASE LIMIT ");
    sql.push_str(&limit.to_string());

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter().map(|b| b.as_ref())), |row| {
        Ok((
            row.get::<_, String>(0)?,                             // sub id
            row.get::<_, String>(1)?,                             // account
            row.get::<_, String>(2)?,                             // url
            row.get::<_, String>(3)?,                             // title
            row.get::<_, i64>(4)?,                                // latest_at
            row.get::<_, i64>(5)?,                                // unread_count
            row.get::<_, i64>(6)? != 0,                           // has starred items
            row.get::<_, String>(7)?,                             // latest_title
            row.get::<_, Option<String>>(8)?.unwrap_or_default(), // latest_summary
            row.get::<_, Option<String>>(9)?.unwrap_or_default(), // sub json extras
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (
            sub_id,
            acct,
            url,
            title,
            latest_at,
            unread,
            has_starred_items,
            latest_title,
            latest_summary,
            sub_json,
        ) = row?;
        let icon_key = serde_json::from_str::<Value>(&sub_json)
            .ok()
            .and_then(|v| v["icon_key"].as_str().map(str::to_string))
            .unwrap_or_default();
        out.push(thread_message(
            &acct,
            &sub_id,
            &title,
            &url,
            latest_at,
            unread,
            has_starred_items,
            &latest_title,
            &latest_summary,
            &icon_key,
        ));
    }
    Ok(out)
}

/// Paginated newest-first slice of an RSS thread, returned ascending by
/// (sort_ts, item_key). `before_cursor` is `Some((ts, item_key))` to fetch the
/// next older page; `None` returns the most recent page. When `limit` is
/// `None`, the full thread is returned (caller did not opt into pagination).
pub fn read_thread_page(
    conn: &Connection,
    thread_id: &str,
    before_cursor: Option<(i64, String)>,
    limit: Option<u32>,
) -> Result<(Vec<Value>, Option<String>)> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    let sub_title: String = conn
        .query_row(
            "SELECT title FROM subscriptions WHERE id = ?1 AND account = ?2",
            params![sub_id, account],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default();

    let load_remote_images = store::load_remote_images(conn, &account).unwrap_or(false);

    // Newest-first when paginating so the cursor walks backwards in time;
    // unpaginated callers still want oldest-first to preserve old behavior.
    let (sql, ascending) = match limit {
        Some(_) => (
            "SELECT msg_id, subject, seen, json,
                    COALESCE(NULLIF(json_extract(json,'$.published_at'),0),
                             NULLIF(json_extract(json,'$.updated_at'),0),
                             json_extract(json,'$.fetched_at')) AS sort_ts,
                    starred
               FROM messages
              WHERE account = ?1 AND folder = ?2
                AND (?3 IS NULL
                     OR sort_ts < ?3
                     OR (sort_ts = ?3 AND msg_id < ?4))
              ORDER BY sort_ts DESC, msg_id DESC
              LIMIT ?5",
            false,
        ),
        None => (
            "SELECT msg_id, subject, seen, json,
                    COALESCE(NULLIF(json_extract(json,'$.published_at'),0),
                             NULLIF(json_extract(json,'$.updated_at'),0),
                             json_extract(json,'$.fetched_at')) AS sort_ts,
                    starred
               FROM messages
              WHERE account = ?1 AND folder = ?2
                AND (?3 IS NULL
                     OR sort_ts < ?3
                     OR (sort_ts = ?3 AND msg_id < ?4))
              ORDER BY sort_ts ASC, msg_id ASC",
            true,
        ),
    };

    let mut stmt = conn.prepare(sql)?;
    let cursor_ts = before_cursor.as_ref().map(|(t, _)| *t);
    let cursor_key = before_cursor
        .as_ref()
        .map(|(_, k)| k.as_str())
        .unwrap_or("");
    let probe_limit = limit
        .map(|n| n.saturating_add(1) as i64)
        .unwrap_or(i64::MAX);

    let mut map_row = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get::<_, String>(0)?,                   // item key
            row.get::<_, String>(1)?,                   // subject
            row.get::<_, i64>(2)?,                      // seen
            row.get::<_, String>(3)?,                   // json
            row.get::<_, Option<i64>>(4)?.unwrap_or(0), // sort_ts
            row.get::<_, i64>(5)? != 0,                 // starred
        ))
    };
    let rows = match limit {
        Some(_) => stmt.query_map(
            params![account, sub_id, cursor_ts, cursor_key, probe_limit],
            &mut map_row,
        )?,
        None => stmt.query_map(
            params![account, sub_id, cursor_ts, cursor_key],
            &mut map_row,
        )?,
    };

    let mut rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = match limit {
        Some(n) if rows.len() > n as usize => {
            rows.truncate(n as usize);
            true
        }
        _ => false,
    };

    // After truncation, the oldest item in this newest-first page is the
    // cursor for the next (older) page.
    let next_cursor = if has_more {
        rows.last()
            .map(|(item_key, _, _, _, ts, _)| format!("ts:{}:{}", ts, item_key))
    } else {
        None
    };

    if !ascending {
        rows.reverse();
    }

    let mut out = Vec::with_capacity(rows.len());
    for (item_key, subject, seen, json, _ts, starred) in rows {
        let body_html = serde_json::from_str::<Value>(&json)
            .ok()
            .and_then(|value| value["body_html"].as_str().map(str::to_string));
        out.push(item_message(
            &account,
            &sub_id,
            &sub_title,
            &item_key,
            &subject,
            seen == 0,
            starred,
            &json,
            body_html.as_deref(),
            load_remote_images,
        ));
    }
    Ok((out, next_cursor))
}

/// Final bridge-shaped `Message` for every starred RSS item across all
/// accounts/subscriptions, newest first (same sort_ts ordering as
/// `read_thread_page`).
pub fn starred_items(conn: &Connection, limit: i64) -> Result<Vec<Value>> {
    let limit = if limit <= 0 { 200 } else { limit };
    let mut stmt = conn.prepare(
        "SELECT m.account, m.folder, s.title, m.msg_id, m.subject, m.seen, m.json
           FROM messages m
           JOIN subscriptions s ON s.account = m.account AND s.id = m.folder
          WHERE m.starred <> 0
          ORDER BY COALESCE(NULLIF(json_extract(m.json,'$.published_at'),0),
                            NULLIF(json_extract(m.json,'$.updated_at'),0),
                            json_extract(m.json,'$.fetched_at')) DESC, m.msg_id DESC
          LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok((
            row.get::<_, String>(0)?, // account
            row.get::<_, String>(1)?, // sub id
            row.get::<_, String>(2)?, // sub title
            row.get::<_, String>(3)?, // item key
            row.get::<_, String>(4)?, // subject
            row.get::<_, i64>(5)?,    // seen
            row.get::<_, String>(6)?, // json
        ))
    })?;

    let mut remote_images: std::collections::HashMap<String, bool> = Default::default();
    let mut out = Vec::new();
    for row in rows {
        let (account, sub_id, sub_title, item_key, subject, seen, json) = row?;
        let load_remote_images = *remote_images
            .entry(account.clone())
            .or_insert_with(|| store::load_remote_images(conn, &account).unwrap_or(false));
        let body_html = serde_json::from_str::<Value>(&json)
            .ok()
            .and_then(|value| value["body_html"].as_str().map(str::to_string));
        out.push(item_message(
            &account,
            &sub_id,
            &sub_title,
            &item_key,
            &subject,
            seen == 0,
            true,
            &json,
            body_html.as_deref(),
            load_remote_images,
        ));
    }
    Ok(out)
}

pub fn mark_thread_read(conn: &Connection, thread_id: &str, seen: bool) -> Result<()> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    if !seen {
        // Marking a feed unread flags its newest item only — see
        // store::update_rss_newest_item_seen.
        return store::update_rss_newest_item_seen(conn, &account, &sub_id, seen);
    }
    store::update_rss_thread_seen(conn, &account, &sub_id, seen)
}

pub fn mark_items_read(
    conn: &Connection,
    thread_id: &str,
    item_keys: &[String],
    seen: bool,
) -> Result<()> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    for item_key in item_keys {
        store::update_rss_item_seen(conn, &account, &sub_id, item_key, seen)?;
    }
    Ok(())
}

pub fn mark_account_read(conn: &Connection, account: &str) -> Result<usize> {
    if !account.starts_with("rss-") {
        return Err(anyhow!("invalid RSS account id: {account}"));
    }
    Ok(conn.execute(
        "UPDATE messages SET seen = 1 WHERE account = ?1 AND seen = 0",
        params![account],
    )?)
}

pub fn mark_thread_starred(conn: &Connection, thread_id: &str, starred: bool) -> Result<()> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    store::update_rss_thread_starred(conn, &account, &sub_id, starred)
}

pub fn mark_items_starred(
    conn: &Connection,
    thread_id: &str,
    item_keys: &[String],
    starred: bool,
) -> Result<()> {
    let (account, sub_id) =
        parse_thread_id(thread_id).ok_or_else(|| anyhow!("invalid RSS thread id: {thread_id}"))?;
    for item_key in item_keys {
        store::update_rss_item_starred(conn, &account, &sub_id, item_key, starred)?;
    }
    Ok(())
}

/// The single synthetic Inbox folder for an RSS account, with its unread count.
pub fn folders(conn: &Connection, account: &str) -> Result<Vec<Value>> {
    let unread = unread_count(conn, account)?;
    Ok(vec![json!({
        "id": RSS_FOLDER_ID,
        "account_id": account,
        "name": "Inbox",
        "role": "inbox",
        "unread": unread,
    })])
}

pub fn unread_count(conn: &Connection, account: &str) -> Result<i64> {
    Ok(conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE account = ?1 AND seen = 0",
            params![account],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Bridge `Account` JSON for an RSS account.
pub fn account_json(id: &str, title: &str) -> Value {
    json!({
        "id": id,
        "email": format!("{id}.local"),
        "display_name": title,
        "provider": "rss",
        "auth_type": "rss",
        "imap_host": "",
        "imap_port": 0,
        "smtp_host": "",
        "smtp_port": 0,
        "tls": false,
    })
}

// ---- Persistence helpers ----------------------------------------------------

fn load_subscriptions(conn: &Connection, account: &str) -> Result<Vec<Subscription>> {
    let mut stmt = conn.prepare(
        "SELECT id, account, url, etag, last_modified FROM subscriptions
         WHERE account = ?1 AND enabled = 1 ORDER BY title COLLATE NOCASE, url",
    )?;
    let rows = stmt.query_map(params![account], |row| {
        Ok(Subscription {
            id: row.get(0)?,
            account: row.get(1)?,
            url: row.get(2)?,
            etag: row.get(3)?,
            last_modified: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---- Presentation (final bridge JSON) ---------------------------------------

#[allow(clippy::too_many_arguments)]
fn thread_message(
    account: &str,
    sub_id: &str,
    title: &str,
    url: &str,
    latest_at: i64,
    unread: i64,
    has_starred_items: bool,
    latest_title: &str,
    latest_summary: &str,
    icon_key: &str,
) -> Value {
    let mut preview = latest_title.trim().to_string();
    if !latest_summary.is_empty() {
        if !preview.is_empty() {
            preview.push_str(" - ");
        }
        preview.push_str(&first_line(latest_summary));
    }
    let thread_id = format_thread_id(account, sub_id);
    json!({
        "id": thread_id,
        "account_id": account,
        "folder_id": RSS_FOLDER_ID,
        "thread_id": thread_id,
        "from_name": title,
        "from_addr": feed_host_label(url),
        "feed_url": url,
        "feed_icon": icon_key,
        "subject": title,
        "preview": preview,
        "date": latest_at,
        "unread": unread > 0,
        "unread_count": unread,
        // A feed is not itself starred; this only describes its contained items.
        "has_starred_items": has_starred_items,
    })
}

fn item_message(
    account: &str,
    sub_id: &str,
    sub_title: &str,
    item_key: &str,
    subject: &str,
    unread: bool,
    starred: bool,
    extra_json: &str,
    body_html: Option<&str>,
    load_remote_images: bool,
) -> Value {
    let json: Value = serde_json::from_str(extra_json).unwrap_or_else(|_| json!({}));
    let summary = json["summary"].as_str().unwrap_or_default();
    let content = json["content"].as_str().unwrap_or_default();
    let link = json["link"].as_str().unwrap_or_default();
    // Inline images as attachments. A cached image carries a media `key` (served
    // from disk, no third-party fetch); an uncached one carries only its `url`,
    // which the UI gates on the account's "load remote images" preference.
    let mut attachments: Vec<Value> = json["images"]
        .as_array()
        .map(|arr| {
            let root = crate::parse::media_root();
            arr.iter()
                .filter_map(|img| {
                    let url = img["url"].as_str()?;
                    // Trust the cached key only if it's a real image still on disk;
                    // otherwise fall back to the remote URL. Keys persisted before
                    // image validation can point at poison (".plain" error bodies
                    // from expired signed URLs), which would render as broken images.
                    let key = img["key"].as_str().filter(|k| key_is_image(&root, k));
                    Some(json!({
                        "filename": image_filename(url),
                        "mime": image_mime(url),
                        "size": 0,
                        "key": key,
                        "url": url,
                    }))
                })
                .collect()
        })
        .unwrap_or_default();
    // Inline videos as remote-only attachments (no cached key). The UI keys off
    // the `video/*` mime to render a player instead of an image.
    if let Some(arr) = json["videos"].as_array() {
        for vid in arr {
            let Some(url) = vid["url"].as_str() else {
                continue;
            };
            attachments.push(json!({
                "filename": image_filename(url),
                "mime": video_mime(url),
                "size": 0,
                "key": Value::Null,
                "url": url,
            }));
        }
    }
    let published = json["published_at"].as_i64().unwrap_or(0);
    let updated = json["updated_at"].as_i64().unwrap_or(0);
    let fetched = json["fetched_at"].as_i64().unwrap_or(0);

    let mut body = String::new();
    if let Some(html) = body_html {
        body = crate::parse::html_to_text(html);
    }
    if body.trim().is_empty() {
        body = summary.trim().to_string();
        if !content.trim().is_empty() {
            body = content.to_string();
        }
    }
    let ts = if published != 0 {
        published
    } else if updated != 0 {
        updated
    } else {
        fetched
    };
    // Iframe-ready original HTML for the reader's HTML mode (remote images gated on
    // the account setting, like mail). Omitted when the feed gave no HTML.
    let body_html = body_html
        .filter(|html| !html.trim().is_empty())
        .map(|html| crate::parse::prepare_html(html, load_remote_images));

    let thread_id = format_thread_id(account, sub_id);
    json!({
        "id": format!("{thread_id}#{item_key}"),
        "account_id": account,
        "folder_id": RSS_FOLDER_ID,
        "thread_id": thread_id,
        "from_name": sub_title,
        "from_addr": feed_host_label(link),
        // The item's original page; the reader offers to open or copy it.
        "link": link,
        "subject": subject,
        "preview": first_line(summary),
        "body_links": crate::linkify::plain_body_links(&body),
        "body": body,
        "body_html": body_html,
        "date": ts,
        "unread": unread,
        "starred": starred,
        "has_attachments": !attachments.is_empty(),
        "attachments": attachments,
    })
}

// ---- URL + text helpers (match Go RSSStore) ---------------------------------

pub(crate) fn normalize_feed_url(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(anyhow!("feed URL required"));
    }
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let mut parsed = url::Url::parse(&with_scheme).map_err(|_| anyhow!("invalid feed URL"))?;
    if parsed.host_str().unwrap_or("").is_empty() {
        return Err(anyhow!("invalid feed URL"));
    }
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(anyhow!("feed URL must use http or https"));
    }
    parsed.set_fragment(None);
    Ok(parsed.to_string())
}

/// A fresh, stable, name-independent RSS account id. Carries the "rss-" prefix
/// the bridge routes on, and is random so accounts can be renamed and even share
/// a display name.
fn new_account_id() -> String {
    format!("rss-{}", uuid::Uuid::new_v4().simple())
}

pub(crate) fn rss_subscription_id(feed_url: &str) -> String {
    format!("feed-{}", stable_hash(feed_url))
}

fn format_thread_id(account: &str, sub_id: &str) -> String {
    format!("{account}#rss#{sub_id}")
}

fn parse_thread_id(thread_id: &str) -> Option<(String, String)> {
    let (account, rest) = thread_id.split_once("#rss#")?;
    if account.is_empty() || rest.is_empty() {
        return None;
    }
    let sub_id = rest.split('#').next().unwrap_or("");
    if sub_id.is_empty() {
        return None;
    }
    Some((account.to_string(), sub_id.to_string()))
}

fn stable_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let sum = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&sum[..12])
}

fn first_line(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview: String = collapsed.chars().take(220).collect();
    if collapsed.chars().count() > 220 {
        preview.push_str("...");
    }
    preview
}

fn feed_host_label(raw: &str) -> String {
    url::Url::parse(raw)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_string())
        })
        .unwrap_or_default()
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
