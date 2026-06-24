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

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use quick_xml::Reader;
use quick_xml::events::Event;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use crate::store::{self, RssItemExtra, RssMedia};

const RSS_FOLDER_ID: &str = "inbox";
const DEFAULT_RSS_ACCOUNT_TITLE: &str = "RSS";
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// A subscription as needed for sync and presentation.
struct Subscription {
    id: String,
    account: String,
    url: String,
    title: String,
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
        "INSERT INTO accounts(id, engine, provider, display_name, config, created_at, updated_at)
         VALUES(?1, 'rss', 'rss', ?2, '{}', ?3, ?3)
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

// ---- OPML import / export ---------------------------------------------------

/// Serialize a single RSS account's feeds to an OPML 2.0 document. The account
/// becomes one folder `<outline>`, with a child `<outline type="rss">` per
/// enabled subscription. Round-trips with [`import_opml`].
pub fn export_opml(conn: &Connection, account: &str) -> Result<String> {
    let display_name: String = match conn
        .query_row(
            "SELECT display_name FROM accounts WHERE id = ?1 AND engine = 'rss'",
            params![account],
            |row| row.get(0),
        )
        .optional()?
    {
        Some(name) => name,
        None => return Err(anyhow!("unknown RSS account: {account}")),
    };

    let group_title = if display_name.trim().is_empty() {
        DEFAULT_RSS_ACCOUNT_TITLE
    } else {
        display_name.trim()
    };

    let mut sub_stmt = conn.prepare(
        "SELECT title, feed_title, url, site_url FROM subscriptions
         WHERE account = ?1 AND enabled = 1 ORDER BY title COLLATE NOCASE, url",
    )?;
    let feeds = sub_stmt
        .query_map(params![account], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut body = String::new();
    body.push_str(&format!(
        "    <outline text=\"{0}\" title=\"{0}\">\n",
        xml_escape(group_title)
    ));
    for (title, feed_title, url, site_url) in feeds {
        let label = [title.trim(), feed_title.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| feed_host_label(&url));
        body.push_str(&format!(
            "      <outline type=\"rss\" text=\"{0}\" title=\"{0}\" xmlUrl=\"{1}\"",
            xml_escape(&label),
            xml_escape(url.trim()),
        ));
        if !site_url.trim().is_empty() {
            body.push_str(&format!(" htmlUrl=\"{}\"", xml_escape(site_url.trim())));
        }
        body.push_str("/>\n");
    }
    body.push_str("    </outline>\n");

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <opml version=\"2.0\">\n  \
         <head>\n    <title>Meron Feeds</title>\n  </head>\n  \
         <body>\n{body}  </body>\n</opml>\n"
    ))
}

/// Import an OPML document into an existing RSS account. Every feed in the
/// document (flattened across any folder `<outline>`s) is added to `account`.
/// Subscriptions are inserted without fetching (deduped by URL across all
/// accounts) — the next sync pulls their items. Returns the number of feeds
/// actually added (existing feeds are skipped).
pub fn import_opml(db: &Mutex<Connection>, opml: &str, account: &str) -> Result<u32> {
    let groups = parse_opml(opml)?;
    let conn = db.lock().unwrap();

    // The target account must exist and be RSS.
    match store::account_engine(&conn, account)?.as_deref() {
        Some("rss") => {}
        Some(other) => return Err(anyhow!("account {account} is not RSS (engine={other})")),
        None => return Err(anyhow!("unknown account: {account}")),
    }

    let tx = conn.unchecked_transaction()?;
    let mut imported = 0u32;
    let now = now_unix();

    for feed in groups.into_iter().flat_map(|group| group.feeds) {
        let Ok(url) = normalize_feed_url(&feed.xml_url) else {
            continue;
        };
        let sub_id = rss_subscription_id(&url);
        let title = feed.title.trim();
        // INSERT OR IGNORE: the `url` column is UNIQUE, so a feed already
        // subscribed (in this or any account) is skipped, not duplicated.
        let added = tx.execute(
            "INSERT OR IGNORE INTO subscriptions
               (id, account, url, title, site_url, feed_title, enabled, created_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?4, 1, ?6, ?6)",
            params![sub_id, account, url, title, feed.html_url.trim(), now],
        )?;
        imported += added as u32;
    }

    tx.commit()?;
    Ok(imported)
}

/// One OPML folder's feeds. Import flattens groups into the target account, so
/// only the feeds matter (folder titles are ignored).
struct OpmlGroup {
    feeds: Vec<OpmlFeed>,
}

struct OpmlFeed {
    title: String,
    xml_url: String,
    html_url: String,
}

/// A frame on the outline-nesting stack: a folder carries the index of the
/// account group its descendant feeds belong to; a feed carries nothing.
#[derive(Clone, Copy)]
enum OpmlFrame {
    Folder(usize),
    Feed,
}

/// Parse an OPML document into flat folder groups. Feeds nested under a folder
/// (at any depth) join that top-level folder's group; subfolders are flattened
/// since Meron has no nested feed folders. Feeds outside any folder share a
/// lazily-created default group.
fn parse_opml(xml: &str) -> Result<Vec<OpmlGroup>> {
    let mut reader = Reader::from_str(xml);
    let decoder = reader.decoder();
    let mut buf = Vec::new();
    let mut groups: Vec<OpmlGroup> = Vec::new();
    let mut default_group: Option<usize> = None;
    let mut stack: Vec<OpmlFrame> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) if e.name().as_ref() == b"outline" => {
                let (label, xml_url, html_url) = parse_outline_attrs(&e, decoder)?;
                if !xml_url.is_empty() {
                    let gi = feed_group(&stack, &mut groups, &mut default_group);
                    groups[gi].feeds.push(OpmlFeed {
                        title: label,
                        xml_url,
                        html_url,
                    });
                    stack.push(OpmlFrame::Feed);
                } else {
                    // A folder: reuse the enclosing top-level group if nested,
                    // else start a fresh group named after this folder.
                    let gi = match stack.last() {
                        Some(OpmlFrame::Folder(i)) => *i,
                        _ => {
                            groups.push(OpmlGroup { feeds: Vec::new() });
                            groups.len() - 1
                        }
                    };
                    stack.push(OpmlFrame::Folder(gi));
                }
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"outline" => {
                let (label, xml_url, html_url) = parse_outline_attrs(&e, decoder)?;
                // Self-closing outlines are feeds; an empty folder has no feeds.
                if !xml_url.is_empty() {
                    let gi = feed_group(&stack, &mut groups, &mut default_group);
                    groups[gi].feeds.push(OpmlFeed {
                        title: label,
                        xml_url,
                        html_url,
                    });
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"outline" => {
                stack.pop();
            }
            Ok(_) => {}
            Err(e) => return Err(anyhow!("invalid OPML: {e}")),
        }
        buf.clear();
    }
    Ok(groups)
}

/// The group index a feed at the current nesting belongs to: its enclosing
/// top-level folder, or a lazily-created shared default group.
fn feed_group(
    stack: &[OpmlFrame],
    groups: &mut Vec<OpmlGroup>,
    default_group: &mut Option<usize>,
) -> usize {
    if let Some(OpmlFrame::Folder(i)) = stack.last() {
        return *i;
    }
    if let Some(i) = *default_group {
        return i;
    }
    groups.push(OpmlGroup { feeds: Vec::new() });
    let i = groups.len() - 1;
    *default_group = Some(i);
    i
}

/// Pull the display label (`text`, falling back to `title`) and feed/site URLs
/// off an `<outline>` element, with XML entities decoded.
fn parse_outline_attrs(
    e: &quick_xml::events::BytesStart,
    decoder: quick_xml::Decoder,
) -> Result<(String, String, String)> {
    let mut text = String::new();
    let mut title = String::new();
    let mut xml_url = String::new();
    let mut html_url = String::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|err| anyhow!("invalid OPML attribute: {err}"))?;
        let value = attr
            .decode_and_unescape_value(decoder)
            .map_err(|err| anyhow!("invalid OPML attribute value: {err}"))?
            .to_string();
        match attr.key.as_ref() {
            b"xmlUrl" | b"xmlurl" => xml_url = value,
            b"htmlUrl" | b"htmlurl" => html_url = value,
            b"text" => text = value,
            b"title" => title = value,
            _ => {}
        }
    }
    let label = if !text.trim().is_empty() { text } else { title };
    Ok((label.trim().to_string(), xml_url, html_url))
}

/// Escape a string for inclusion in an XML attribute value.
fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

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
           title = excluded.title,
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
/// Returns the number of genuinely new items stored across all of the account's
/// feeds, so the caller can decide whether to raise a "new items" notification.
pub fn sync_account(db: &Mutex<Connection>, account: &str) -> Result<u32> {
    let subs = {
        let conn = db.lock().unwrap();
        load_subscriptions(&conn, account)?
    };
    let mut new_items = 0u32;
    for sub in subs {
        match sync_subscription(db, &sub) {
            Ok(count) => new_items += count,
            Err(e) => {
                let conn = db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE subscriptions SET last_error = ?2, updated_at = ?3 WHERE id = ?1",
                    params![sub.id, format!("{e:#}"), now_unix()],
                );
            }
        }
    }
    prune_feed_media(&crate::parse::media_root());
    Ok(new_items)
}

/// Read an account's resolved "load remote images" preference under a short lock.
fn db_load_remote_images(db: &Mutex<Connection>, account: &str) -> bool {
    let conn = db.lock().unwrap();
    store::load_remote_images(&conn, account).unwrap_or(false)
}

fn sync_subscription(db: &Mutex<Connection>, sub: &Subscription) -> Result<u32> {
    let now = now_unix();
    let fetched = fetch_feed(&sub.url, &sub.etag, &sub.last_modified)?;
    if fetched.not_modified {
        let conn = db.lock().unwrap();
        conn.execute(
            "UPDATE subscriptions SET last_sync_at = ?2, last_error = '', updated_at = ?2 WHERE id = ?1",
            params![sub.id, now],
        )?;
        return Ok(0);
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
    let feed_title = text_content(&feed.title);
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
    let title = if sub.title.is_empty() {
        feed_title.trim().to_string()
    } else {
        sub.title.clone()
    };

    let conn = db.lock().unwrap();
    let tx = conn.unchecked_transaction()?;
    let mut new_items = 0u32;
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
            new_items += 1;
        }
    }
    tx.execute(
        "UPDATE subscriptions
         SET title = ?2, url = ?3, site_url = ?4, feed_title = ?5, json = ?6,
             etag = ?7, last_modified = ?8, last_sync_at = ?9, last_error = '', updated_at = ?9
         WHERE id = ?1",
        params![
            sub.id,
            title,
            fetched.resolved_url,
            site_url,
            feed_title.trim(),
            subscription_json(&icon_url, &icon_key),
            fetched.etag,
            fetched.last_modified,
            now
        ],
    )?;
    tx.commit()?;
    Ok(new_items)
}

/// One bridge `Message` per subscription (subscription = thread), newest first.
pub fn recent(conn: &Connection, account: &str, query: &str, limit: i64) -> Result<Vec<Value>> {
    let limit = if limit <= 0 { 50 } else { limit };
    let q = query.trim().to_lowercase();
    let mut sql = String::from(
        "SELECT s.id, s.account, s.url, s.title,
           COALESCE(MAX(NULLIF(json_extract(m.json,'$.published_at'),0)),
                    MAX(NULLIF(json_extract(m.json,'$.updated_at'),0)),
                    MAX(json_extract(m.json,'$.fetched_at')), 0) AS latest_at,
           COALESCE(SUM(CASE WHEN m.seen = 0 THEN 1 ELSE 0 END), 0) AS unread_count,
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
    if !q.is_empty() {
        sql.push_str(
            " AND (lower(s.title) LIKE ?2 OR lower(m.subject) LIKE ?2 OR lower(json_extract(m.json,'$.summary')) LIKE ?2)",
        );
        args.push(Box::new(format!("%{q}%")));
    }
    sql.push_str(" GROUP BY s.id ORDER BY latest_at DESC, s.title COLLATE NOCASE LIMIT ");
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
            row.get::<_, String>(6)?,                             // latest_title
            row.get::<_, Option<String>>(7)?.unwrap_or_default(), // latest_summary
            row.get::<_, Option<String>>(8)?.unwrap_or_default(), // sub json extras
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (sub_id, acct, url, title, latest_at, unread, latest_title, latest_summary, sub_json) =
            row?;
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
    let unread: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE account = ?1 AND seen = 0",
            params![account],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok(vec![json!({
        "id": RSS_FOLDER_ID,
        "account_id": account,
        "name": "Inbox",
        "role": "inbox",
        "unread": unread,
    })])
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
        "SELECT id, account, url, title, etag, last_modified FROM subscriptions
         WHERE account = ?1 AND enabled = 1 ORDER BY title COLLATE NOCASE, url",
    )?;
    let rows = stmt.query_map(params![account], |row| {
        Ok(Subscription {
            id: row.get(0)?,
            account: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
            etag: row.get(4)?,
            last_modified: row.get(5)?,
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
    if !link.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str("Source: ");
        body.push_str(link);
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
        "subject": subject,
        "preview": first_line(summary),
        "body": body,
        "body_html": body_html,
        "date": ts,
        "unread": unread,
        "starred": starred,
        "has_attachments": !attachments.is_empty(),
        "attachments": attachments,
    })
}

// ---- Feed fetch + parse -----------------------------------------------------

struct FetchedFeed {
    feed: Option<feed_rs::model::Feed>,
    resolved_url: String,
    etag: String,
    last_modified: String,
    not_modified: bool,
}

struct HttpResponse {
    status: u16,
    etag: String,
    last_modified: String,
    body: Vec<u8>,
    content_type: String,
}

fn http_get(url: &str, etag: &str, last_modified: &str) -> Result<HttpResponse> {
    let mut rb = ureq::get(url);
    if !etag.is_empty() {
        rb = rb.header("If-None-Match", etag);
    }
    if !last_modified.is_empty() {
        rb = rb.header("If-Modified-Since", last_modified);
    }
    let mut resp = rb
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(HTTP_TIMEOUT))
        .build()
        .call()
        .with_context(|| format!("HTTP GET {url}"))?;
    let status = resp.status().as_u16();
    let etag = header_str(&resp, "etag");
    let last_modified = header_str(&resp, "last-modified");
    let content_type = header_str(&resp, "content-type");
    let body = if status == 304 {
        Vec::new()
    } else {
        resp.body_mut()
            .read_to_vec()
            .with_context(|| format!("read body {url}"))?
    };
    Ok(HttpResponse {
        status,
        etag,
        last_modified,
        body,
        content_type,
    })
}

fn header_str(resp: &ureq::http::Response<ureq::Body>, name: &str) -> String {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// Fetch and parse a feed. On a parse failure, try discovering a feed `<link>`
/// in the page's HTML and parse that instead.
fn fetch_feed(raw_url: &str, etag: &str, last_modified: &str) -> Result<FetchedFeed> {
    let resp = http_get(raw_url, etag, last_modified)?;
    if resp.status == 304 {
        return Ok(FetchedFeed {
            feed: None,
            resolved_url: raw_url.to_string(),
            etag: etag.to_string(),
            last_modified: last_modified.to_string(),
            not_modified: true,
        });
    }
    if let Ok(feed) = feed_rs::parser::parse(&resp.body[..]) {
        return Ok(FetchedFeed {
            feed: Some(feed),
            resolved_url: raw_url.to_string(),
            etag: resp.etag,
            last_modified: resp.last_modified,
            not_modified: false,
        });
    }
    // Direct parse failed: treat the body as HTML and look for a feed link.
    let html = String::from_utf8_lossy(&resp.body);
    let discovered = discover_feed_url(raw_url, &html)
        .ok_or_else(|| anyhow!("no RSS feed found at {raw_url}"))?;
    let feed_resp = http_get(&discovered, "", "")?;
    let feed = feed_rs::parser::parse(&feed_resp.body[..])
        .with_context(|| format!("parse discovered feed {discovered}"))?;
    let _ = resp.content_type;
    Ok(FetchedFeed {
        feed: Some(feed),
        resolved_url: discovered,
        etag: feed_resp.etag,
        last_modified: feed_resp.last_modified,
        not_modified: false,
    })
}

/// Find a `<link rel="alternate" type="application/rss+xml" ...>` (or similar)
/// in an HTML head and resolve it against the page URL.
fn discover_feed_url(page_url: &str, html: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("link").ok()?;
    let base = url::Url::parse(page_url).ok();
    for el in doc.select(&selector) {
        let rel = el.value().attr("rel").unwrap_or_default();
        let typ = el.value().attr("type").unwrap_or_default();
        let href = el.value().attr("href").unwrap_or_default().trim();
        if href.is_empty() || !rel_has_alternate(rel) || !is_feed_link_type(typ) {
            continue;
        }
        return match &base {
            Some(b) => b.join(href).ok().map(|u| u.to_string()),
            None => Some(href.to_string()),
        };
    }
    None
}

fn parse_items(feed: &feed_rs::model::Feed, fetched_at: i64) -> Vec<ParsedItem> {
    feed.entries
        .iter()
        .filter_map(|entry| parse_item(entry, fetched_at))
        .collect()
}

fn parse_item(entry: &feed_rs::model::Entry, fetched_at: i64) -> Option<ParsedItem> {
    let mut title = text_content(&entry.title).trim().to_string();
    if title.is_empty() {
        title = "(untitled)".to_string();
    }
    let author = entry
        .authors
        .first()
        .map(|p| p.name.trim().to_string())
        .unwrap_or_default();
    let summary_html = text_content(&entry.summary);
    let content_html = entry
        .content
        .as_ref()
        .and_then(|c| c.body.clone())
        .unwrap_or_default();
    let summary = clean_feed_text(&summary_html);
    let content = clean_feed_text(&content_html);
    // Lift inline <img> URLs out of the HTML (stripped by clean_feed_text) so the
    // UI can show them as images. Union content + summary, content first. Keys are
    // filled in later by the image cacher (sync time) for image-enabled accounts.
    let mut image_urls = extract_image_urls(&content_html);
    for url in extract_image_urls(&summary_html) {
        if !image_urls.iter().any(|existing| existing == &url) {
            image_urls.push(url);
        }
    }
    let images = image_urls
        .into_iter()
        .map(|url| RssMedia { url, key: None })
        .collect();
    // Lift inline <video>/<source> URLs out of the HTML the same way. These are
    // rendered straight from their remote URL (no local caching), so they only
    // ever carry a `url`.
    let mut video_urls = extract_video_urls(&content_html);
    for url in extract_video_urls(&summary_html) {
        if !video_urls.iter().any(|existing| existing == &url) {
            video_urls.push(url);
        }
    }
    let videos = video_urls
        .into_iter()
        .map(|url| RssMedia { url, key: None })
        .collect();
    let link = entry
        .links
        .first()
        .map(|l| l.href.trim().to_string())
        .unwrap_or_default();
    let published_at = entry.published.map(|d| d.timestamp()).unwrap_or(0);
    let updated_at = entry.updated.map(|d| d.timestamp()).unwrap_or(0);

    // Keep the raw HTML (content preferred, summary as fallback) for HTML mode.
    let content_html = if !content_html.trim().is_empty() {
        Some(content_html)
    } else if !summary_html.trim().is_empty() {
        Some(summary_html)
    } else {
        None
    };

    let key_source = if !entry.id.trim().is_empty() {
        entry.id.trim().to_string()
    } else if !link.is_empty() {
        link.clone()
    } else {
        format!("{title}|{published_at}|{updated_at}")
    };

    Some(ParsedItem {
        item_key: stable_hash(&key_source),
        title,
        unread: true,
        content_html,
        extra: RssItemExtra {
            author,
            link,
            summary,
            content,
            images,
            videos,
            published_at,
            updated_at,
            fetched_at,
        },
    })
}

// ---- URL + text helpers (match Go RSSStore) ---------------------------------

fn normalize_feed_url(raw: &str) -> Result<String> {
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

fn rss_subscription_id(feed_url: &str) -> String {
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

fn text_content(text: &Option<feed_rs::model::Text>) -> String {
    text.as_ref().map(|t| t.content.clone()).unwrap_or_default()
}

fn clean_feed_text(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    // Replace block-level HTML tags with newlines to preserve structural breaks.
    let normalized = value
        .replace("<p>", "\n")
        .replace("</p>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n")
        .replace("</td>", "\n")
        .replace("</tr>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n")
        .replace("</h4>", "\n")
        .replace("</h5>", "\n")
        .replace("</h6>", "\n")
        .replace("<blockquote>", "\n")
        .replace("</blockquote>", "\n");

    let fragment = scraper::Html::parse_fragment(&normalized);
    // Concatenate text nodes without separators (like goquery's .Text()) so we
    // don't insert spaces before punctuation; normalize spacing and newlines after.
    let text: String = fragment.root_element().text().collect();
    normalize_newlines_and_spaces(&text)
}

fn normalize_newlines_and_spaces(text: &str) -> String {
    let mut result = String::new();
    let mut consecutive_newlines = 0;

    for line in text.lines() {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            consecutive_newlines += 1;
        } else {
            if !result.is_empty() {
                if consecutive_newlines > 1 {
                    result.push_str("\n\n");
                } else {
                    result.push('\n');
                }
            }
            consecutive_newlines = 0;

            // Collapse multiple spaces within the line
            let collapsed_line = trimmed_line
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            result.push_str(&collapsed_line);
        }
    }
    result.trim().to_string()
}

/// Collect `<img src>` URLs from a feed item's HTML, in document order, deduped.
/// Skips `data:` URIs (inline base64 would bloat the cache) and empty srcs.
fn extract_image_urls(html: &str) -> Vec<String> {
    let html = html.trim();
    if html.is_empty() {
        return Vec::new();
    }
    let fragment = scraper::Html::parse_fragment(html);
    let Ok(selector) = scraper::Selector::parse("img") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for el in fragment.select(&selector) {
        let Some(src) = el.value().attr("src") else {
            continue;
        };
        let src = src.trim();
        if src.is_empty() || src.starts_with("data:") {
            continue;
        }

        let mut is_tracker = false;

        // 1. Check width and height attributes
        if let (Some(w), Some(h)) = (el.value().attr("width"), el.value().attr("height")) {
            let w = w.trim();
            let h = h.trim();
            if (w == "1" || w == "0" || w == "2") && (h == "1" || h == "0" || h == "2") {
                is_tracker = true;
            }
        }

        // 2. Check inline style attributes
        if let Some(style) = el.value().attr("style") {
            let s = style.to_lowercase();
            if s.contains("display:none")
                || s.contains("display: none")
                || s.contains("visibility:hidden")
                || s.contains("visibility: hidden")
            {
                is_tracker = true;
            }
            let has_tiny_w = s.contains("width:0px")
                || s.contains("width: 0px")
                || s.contains("width:1px")
                || s.contains("width: 1px")
                || s.contains("width:2px")
                || s.contains("width: 2px");
            let has_tiny_h = s.contains("height:0px")
                || s.contains("height: 0px")
                || s.contains("height:1px")
                || s.contains("height: 1px")
                || s.contains("height:2px")
                || s.contains("height: 2px");
            if has_tiny_w && has_tiny_h {
                is_tracker = true;
            }
        }

        // 3. Check URL pattern heuristics
        let lower_src = src.to_lowercase();
        if lower_src.contains("/open/")
            || lower_src.contains("/track")
            || lower_src.contains("/pixel")
            || lower_src.contains("pixel.gif")
            || lower_src.contains("cleardot.gif")
            || lower_src.contains("spacer.gif")
            || lower_src.contains("/wf/open")
            || lower_src.contains("/open.php")
            || lower_src.contains("utm_")
            || lower_src.contains("bounce")
        {
            is_tracker = true;
        }

        if is_tracker {
            continue;
        }

        if !out.iter().any(|existing| existing == src) {
            out.push(src.to_string());
        }
    }
    out
}

/// Collect inline video URLs from a feed item's HTML, in document order, deduped.
/// Handles both `<video src>` and `<video><source src></video>`. Skips `data:`
/// URIs and empty srcs.
fn extract_video_urls(html: &str) -> Vec<String> {
    let html = html.trim();
    if html.is_empty() {
        return Vec::new();
    }
    let fragment = scraper::Html::parse_fragment(html);
    let Ok(selector) = scraper::Selector::parse("video, video source") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for el in fragment.select(&selector) {
        let Some(src) = el.value().attr("src") else {
            continue;
        };
        let src = src.trim();
        if src.is_empty() || src.starts_with("data:") {
            continue;
        }
        if !out.iter().any(|existing| existing == src) {
            out.push(src.to_string());
        }
    }
    out
}

/// The trailing path segment of an image URL, used as its display filename.
fn image_filename(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut segs| segs.next_back().map(str::to_string))
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "image".to_string())
}

/// Best-effort MIME guess from an image URL's extension (display only).
fn image_mime(url: &str) -> &'static str {
    let lower = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "image/jpeg"
    }
}

/// Best-effort MIME guess from a video URL's extension (display only); defaults
/// to `video/mp4` when the URL has no recognizable extension.
fn video_mime(url: &str) -> &'static str {
    let lower = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".ogv") || lower.ends_with(".ogg") {
        "video/ogg"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else {
        "video/mp4"
    }
}

// ---- Inline image caching ---------------------------------------------------

/// Cap on the total size of cached feed images (overridable via env). Pruned
/// oldest-first after a sync once exceeded.
const FEED_MEDIA_CAP_BYTES: u64 = 1024 * 1024 * 1024;

/// Download each item's inline images into the media dir, filling in their local
/// `key`. Best-effort: a failed fetch leaves `key = None` (the UI falls back to
/// the remote URL for image-enabled accounts). Skips images already on disk so
/// repeat syncs don't re-download. Network only — never holds the DB lock.
fn cache_feed_images(account: &str, sub_id: &str, items: &mut [ParsedItem]) {
    let root = crate::parse::media_root();
    for item in items.iter_mut() {
        for (index, img) in item.extra.images.iter_mut().enumerate() {
            if let Some(key) = existing_image_key(&root, account, sub_id, &item.item_key, index) {
                img.key = Some(key);
                continue;
            }
            let Ok(resp) = http_get(&img.url, "", "") else {
                continue;
            };
            if resp.status != 200 || resp.body.is_empty() {
                continue;
            }
            // Only cache genuine image bytes. Signed feed-image URLs
            // can return HTTP 200 with a text error body like "URL signature expired"
            // once the signature lapses; caching that would shadow the still-valid
            // remote URL with a broken file. Skip it — key stays None and the UI falls
            // back to the live remote URL for image-enabled accounts.
            let Some(ext) = sniff_image_ext(&resp.body) else {
                continue;
            };
            if let Some(key) = write_feed_image(
                &root,
                account,
                sub_id,
                &item.item_key,
                index,
                ext,
                &resp.body,
            ) {
                img.key = Some(key);
            }
        }
    }
}

/// Download and cache a feed's declared icon into the media dir, returning its
/// media key. Best-effort and idempotent: an already-cached icon (same URL) is
/// reused without re-fetching, and any failure (empty/bad URL, non-image bytes)
/// yields `None` so the caller leaves the key empty. Network only — no DB lock.
fn cache_feed_icon(account: &str, sub_id: &str, icon_url: &str) -> Option<String> {
    let icon_url = icon_url.trim();
    if icon_url.is_empty() {
        return None;
    }
    let root = crate::parse::media_root();
    if let Some(key) = existing_icon_key(&root, account, sub_id, icon_url) {
        return Some(key);
    }
    let resp = http_get(icon_url, "", "").ok()?;
    if resp.status != 200 || resp.body.is_empty() {
        return None;
    }
    // Same byte-sniffing guard as inline images: only cache genuine image bytes,
    // never a text error page served with HTTP 200.
    let ext = sniff_image_ext(&resp.body)?;
    write_feed_icon(&root, account, sub_id, icon_url, ext, &resp.body)
}

/// The relative dir holding a subscription's cached feed icon. Shares the
/// `<account>/<sub>` prefix with the per-item image dirs; the icon is a file
/// directly in it (`icon-<urlhash>.<ext>`), so it never collides with the
/// `<item_key>/` subdirectories below it.
fn feed_icon_dir(account: &str, sub_id: &str) -> String {
    use crate::parse::sanitize_segment as san;
    format!("{}/{}", san(account), san(sub_id))
}

/// If the feed icon for `icon_url` is already on disk, return its media key. The
/// filename embeds a hash of the URL so a changed icon misses and re-downloads
/// (the stale file is left for the size-cap pruner). Heals non-image poison.
fn existing_icon_key(root: &Path, account: &str, sub_id: &str, icon_url: &str) -> Option<String> {
    let rel_dir = feed_icon_dir(account, sub_id);
    let prefix = format!("icon-{}.", stable_hash(icon_url));
    for entry in std::fs::read_dir(root.join(&rel_dir)).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("");
        if is_image_ext(ext) {
            return Some(format!("{rel_dir}/{name}"));
        }
        let _ = std::fs::remove_file(entry.path());
    }
    None
}

/// Write feed-icon bytes under `<account>/<sub>/icon-<urlhash>.<ext>`, returning
/// the media key, or `None` if the write fails.
fn write_feed_icon(
    root: &Path,
    account: &str,
    sub_id: &str,
    icon_url: &str,
    ext: &str,
    bytes: &[u8],
) -> Option<String> {
    let key = format!(
        "{}/icon-{}.{}",
        feed_icon_dir(account, sub_id),
        stable_hash(icon_url),
        ext
    );
    let path = root.join(&key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, bytes).ok()?;
    Some(key)
}

/// The relative dir holding one item's cached images.
fn item_media_dir(account: &str, sub_id: &str, item_key: &str) -> String {
    use crate::parse::sanitize_segment as san;
    format!("{}/{}/{}", san(account), san(sub_id), san(item_key))
}

/// If image `index` for an item is already on disk, return its media key (the
/// extension can vary, so match by the `<index>.` filename prefix).
fn existing_image_key(
    root: &Path,
    account: &str,
    sub_id: &str,
    item_key: &str,
    index: usize,
) -> Option<String> {
    let rel_dir = item_media_dir(account, sub_id, item_key);
    let prefix = format!("{index}.");
    for entry in std::fs::read_dir(root.join(&rel_dir)).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("");
        if is_image_ext(ext) {
            return Some(format!("{rel_dir}/{name}"));
        }
        // Heal earlier poison (e.g. ".plain" from an expired signed URL): drop the
        // bad file so this image re-downloads on this sync.
        let _ = std::fs::remove_file(entry.path());
    }
    None
}

/// Write image bytes under `<account>/<sub>/<item>/<index>.<ext>`, returning the
/// media key, or `None` if the write fails.
fn write_feed_image(
    root: &Path,
    account: &str,
    sub_id: &str,
    item_key: &str,
    index: usize,
    ext: &str,
    bytes: &[u8],
) -> Option<String> {
    let key = format!(
        "{}/{}.{}",
        item_media_dir(account, sub_id, item_key),
        index,
        ext
    );
    let path = root.join(&key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, bytes).ok()?;
    Some(key)
}

/// Recognized cached-image file extensions (what `sniff_image_ext` can produce).
fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "png" | "gif" | "webp" | "svg")
}

/// Sniff a common image format from the leading bytes, returning its canonical
/// file extension, or `None` when the bytes aren't a recognized image (e.g. a
/// text error page served with HTTP 200). Content-type is unreliable for signed
/// image URLs, so we trust the bytes instead.
fn sniff_image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    // SVG is text; check the leading bytes for an <svg>/xml prolog.
    let head = &bytes[..bytes.len().min(512)];
    if let Ok(text) = std::str::from_utf8(head) {
        let trimmed = text.trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && text.contains("<svg")) {
            return Some("svg");
        }
    }
    None
}

/// Whether a cached media `key` points to a real image on disk. Guards the read
/// path against poison cached before image validation existed (e.g. ".plain"
/// error bodies) so those keys are dropped in favor of the remote URL.
fn key_is_image(root: &Path, key: &str) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(root.join(key)) else {
        return false;
    };
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf).unwrap_or(0);
    sniff_image_ext(&buf[..n]).is_some()
}

/// Keep the cached-feed-image footprint under the cap by deleting oldest files
/// first. Scoped to `rss-*` account dirs so it never touches mail attachments.
fn prune_feed_media(root: &Path) {
    let cap = std::env::var("MERON_FEED_MEDIA_CAP")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(FEED_MEDIA_CAP_BYTES);

    let mut files: Vec<(std::path::PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let mut total: u64 = 0;
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for account_dir in entries.flatten() {
        if !account_dir
            .file_name()
            .to_string_lossy()
            .starts_with("rss-")
        {
            continue;
        }
        collect_files(&account_dir.path(), &mut files, &mut total);
    }
    if total <= cap {
        return;
    }
    files.sort_by_key(|(_, _, mtime)| *mtime);
    let mut over = total - cap;
    for (path, size, _) in files {
        if over == 0 {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            over = over.saturating_sub(size);
        }
    }
}

/// Recursively gather (path, size, mtime) for every file under `dir`.
fn collect_files(
    dir: &Path,
    out: &mut Vec<(std::path::PathBuf, u64, std::time::SystemTime)>,
    total: &mut u64,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_files(&path, out, total);
        } else if meta.is_file() {
            *total += meta.len();
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            out.push((path, meta.len(), mtime));
        }
    }
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

fn rel_has_alternate(rel: &str) -> bool {
    rel.to_lowercase()
        .split_whitespace()
        .any(|token| token == "alternate")
}

fn is_feed_link_type(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "application/rss+xml"
            | "application/atom+xml"
            | "application/feed+json"
            | "application/json"
            | "application/xml"
            | "text/xml"
    )
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_thread(conn: &Connection, thread_id: &str) -> Result<Vec<Value>> {
        let (items, _) = read_thread_page(conn, thread_id, None, None)?;
        Ok(items)
    }

    fn entry(id: &str, link: &str, title: &str, published: i64) -> feed_rs::model::Entry {
        let mut e = feed_rs::model::Entry::default();
        e.id = id.to_string();
        if !link.is_empty() {
            e.links = vec![feed_rs::model::Link {
                href: link.to_string(),
                rel: None,
                media_type: None,
                href_lang: None,
                title: None,
                length: None,
            }];
        }
        if !title.is_empty() {
            e.title = Some(feed_rs::model::Text {
                content_type: "text/plain".parse().unwrap(),
                src: None,
                content: title.to_string(),
            });
        }
        if published != 0 {
            e.published = chrono::DateTime::from_timestamp(published, 0);
        }
        e
    }

    #[test]
    fn item_identity_prefers_guid_then_link_then_fallback() {
        let with_guid = parse_item(
            &entry("guid-1", "https://example.com/a", "A", 1700000000),
            1,
        );
        let changed_link = parse_item(
            &entry("guid-1", "https://example.com/changed", "A", 1700000000),
            1,
        );
        assert_eq!(
            with_guid.as_ref().unwrap().item_key,
            changed_link.as_ref().unwrap().item_key,
            "GUID should be stable across link changes"
        );

        let link_only = parse_item(&entry("", "https://example.com/a", "A", 0), 1);
        let same_link = parse_item(&entry("", "https://example.com/a", "Different", 0), 1);
        assert_eq!(
            link_only.as_ref().unwrap().item_key,
            same_link.as_ref().unwrap().item_key,
            "link should be stable when GUID is absent"
        );

        let fallback = parse_item(&entry("", "", "A", 1700000000), 1);
        assert!(!fallback.unwrap().item_key.is_empty());
    }

    #[test]
    fn import_opml_accepts_inoreader_export() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-acct', 'rss', 'rss', 'RSS', '{}')",
            [],
        )
        .unwrap();
        let db = Mutex::new(conn);
        let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="1.0">
  <head>
    <title>Feeds exported from Reader</title>
  </head>
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed One" title="Feed One" type="rss" xmlUrl="https://feeds.example.test/one.xml" htmlUrl="https://example.test/one"/>
      <outline text="Feed Two" title="Feed Two" type="rss" xmlUrl="https://feeds.example.test/two.xml" htmlUrl="https://example.test/two"/>
    </outline>
    <outline text="News" title="News">
      <outline text="Feed Three" title="Feed Three" type="rss" xmlUrl="https://feeds.example.test/three.xml" htmlUrl="https://example.test/three"/>
    </outline>
  </body>
</opml>"#;

        assert_eq!(import_opml(&db, opml, "rss-acct").unwrap(), 3);
        let count: i64 = db
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM subscriptions WHERE account = 'rss-acct'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn move_feed_reassigns_subscription_and_preserves_items() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-one', 'rss', 'rss', 'One', '{}'),
                   ('rss-two', 'rss', 'rss', 'Two', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, feed_title, etag, last_modified, enabled)
             VALUES('feed-1', 'rss-one', 'https://example.com/feed', 'Example', 'Example Feed', 'etag-1', 'mod-1', 1)",
            [],
        )
        .unwrap();
        store::upsert_rss_item(
            &conn,
            "rss-one",
            "feed-1",
            "item-1",
            "Post",
            false,
            None,
            &RssItemExtra {
                author: "Author".to_string(),
                link: "https://example.com/post".to_string(),
                summary: "Summary".to_string(),
                content: String::new(),
                images: vec![],
                videos: vec![],
                published_at: 10,
                updated_at: 0,
                fetched_at: 11,
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE messages SET starred = 1 WHERE account = 'rss-one' AND folder = 'feed-1' AND msg_id = 'item-1'",
            [],
        )
        .unwrap();

        let res = move_feed(&conn, "rss-one#rss#feed-1", "rss-two").unwrap();
        assert_eq!(res["thread_id"], "rss-two#rss#feed-1");

        let sub_account: String = conn
            .query_row(
                "SELECT account FROM subscriptions WHERE id = 'feed-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sub_account, "rss-two");

        let (item_account, seen, starred): (String, i64, i64) = conn
            .query_row(
                "SELECT account, seen, starred FROM messages WHERE folder = 'feed-1' AND msg_id = 'item-1'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(item_account, "rss-two");
        assert_eq!(seen, 1);
        assert_eq!(starred, 1);
    }

    #[test]
    fn move_feed_rejects_non_rss_target() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-one', 'rss', 'rss', 'One', '{}'),
                   ('mail-one', 'mail', 'custom', 'Mail', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-one', 'https://example.com/feed', 'Example', 1)",
            [],
        )
        .unwrap();

        let err = move_feed(&conn, "rss-one#rss#feed-1", "mail-one").unwrap_err();
        assert!(err.to_string().contains("is not RSS"));
    }

    #[test]
    fn normalize_adds_scheme_and_requires_host() {
        assert_eq!(
            normalize_feed_url("example.com/feed").unwrap(),
            "https://example.com/feed"
        );
        assert!(normalize_feed_url("   ").is_err());
        assert!(normalize_feed_url("ftp://example.com").is_err());
    }

    #[test]
    fn discovers_feed_link_from_html_head() {
        let html = r#"<html><head><link rel="alternate" type="application/rss+xml" href="/feed.xml"></head><body></body></html>"#;
        assert_eq!(
            discover_feed_url("https://example.com/", html).as_deref(),
            Some("https://example.com/feed.xml")
        );
    }

    #[test]
    fn new_account_ids_are_unique_and_prefixed() {
        let a = new_account_id();
        let b = new_account_id();
        assert!(a.starts_with("rss-"));
        assert!(b.starts_with("rss-"));
        assert_ne!(a, b, "each account gets a distinct id (no name grouping)");
    }

    #[test]
    fn sniff_image_ext_recognizes_images_and_rejects_error_bodies() {
        assert_eq!(
            sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]),
            Some("jpg")
        );
        assert_eq!(sniff_image_ext(b"\x89PNG\r\n\x1a\n....."), Some("png"));
        assert_eq!(sniff_image_ext(b"GIF89a....."), Some("gif"));
        assert_eq!(
            sniff_image_ext(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some("webp")
        );
        assert_eq!(sniff_image_ext(b"<svg xmlns=\"...\">"), Some("svg"));
        // The HTTP-200 text bodies that previously poisoned the cache.
        assert_eq!(sniff_image_ext(b"URL signature expired"), None);
        assert_eq!(sniff_image_ext(b""), None);
    }

    #[test]
    fn clean_feed_text_strips_html() {
        assert_eq!(
            clean_feed_text("<p>Hello <strong>world</strong>.</p>"),
            "Hello world."
        );
        assert_eq!(
            clean_feed_text("<p>Line 1</p><p>Line 2</p>"),
            "Line 1\nLine 2"
        );
        assert_eq!(clean_feed_text("Hello<br>world"), "Hello\nworld");
    }

    #[test]
    fn extract_image_urls_collects_dedupes_and_skips_data_uris() {
        let html = r#"<p>caption</p>
            <img src="https://cdn.example.com/a/1.jpg">
            <img src="https://cdn.example.com/a/2.png">
            <img src="https://cdn.example.com/a/1.jpg">
            <img src="data:image/png;base64,AAAA">
            <img src="https://cdn.example.com/open/rss" width="1" height="1">
            <img src="https://cdn.example.com/open/rss2" width="0" height="0">
            <img>"#;
        assert_eq!(
            extract_image_urls(html),
            vec![
                "https://cdn.example.com/a/1.jpg".to_string(),
                "https://cdn.example.com/a/2.png".to_string(),
            ]
        );
        assert!(extract_image_urls("  ").is_empty());
    }

    #[test]
    fn read_thread_exposes_inline_images_as_url_attachments() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-acct', 'https://example.com/feed', 'Example Feed', 1)",
            [],
        )
        .unwrap();
        store::upsert_rss_item(
            &conn,
            "rss-acct",
            "feed-1",
            "item-1",
            "Post",
            true,
            None,
            &RssItemExtra {
                author: String::new(),
                link: "https://example.com/post".to_string(),
                summary: "Caption".to_string(),
                content: String::new(),
                images: vec![RssMedia {
                    url: "https://cdn.example.com/p/1.jpg".to_string(),
                    key: None,
                }],
                videos: Vec::new(),
                published_at: 1_700_000_000,
                updated_at: 0,
                fetched_at: 1_700_000_100,
            },
        )
        .unwrap();

        let items = read_thread(&conn, "rss-acct#rss#feed-1").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["has_attachments"], true);
        let attachments = items[0]["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0]["url"], "https://cdn.example.com/p/1.jpg");
        assert_eq!(attachments[0]["filename"], "1.jpg");
        assert!(attachments[0]["key"].is_null());
    }

    #[test]
    fn extract_video_urls_handles_src_and_source_children() {
        let html = r#"<p>caption</p>
            <video controls><source src="https://cdn.example.com/v/1.mp4"></video>
            <video src="https://cdn.example.com/v/2.webm" controls></video>
            <video><source src="https://cdn.example.com/v/1.mp4"></video>
            <video><source src="data:video/mp4;base64,AAAA"></video>"#;
        assert_eq!(
            extract_video_urls(html),
            vec![
                "https://cdn.example.com/v/1.mp4".to_string(),
                "https://cdn.example.com/v/2.webm".to_string(),
            ]
        );
        assert!(extract_video_urls("  ").is_empty());
    }

    #[test]
    fn read_thread_exposes_inline_videos_as_remote_attachments() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-acct', 'https://example.com/feed', 'Example Feed', 1)",
            [],
        )
        .unwrap();
        store::upsert_rss_item(
            &conn,
            "rss-acct",
            "feed-1",
            "item-1",
            "Post",
            true,
            None,
            &RssItemExtra {
                author: String::new(),
                link: "https://example.com/post".to_string(),
                summary: "Caption".to_string(),
                content: String::new(),
                images: Vec::new(),
                videos: vec![RssMedia {
                    url: "https://media.example.test/link/abc/media/1".to_string(),
                    key: None,
                }],
                published_at: 1_700_000_000,
                updated_at: 0,
                fetched_at: 1_700_000_100,
            },
        )
        .unwrap();

        let items = read_thread(&conn, "rss-acct#rss#feed-1").unwrap();
        assert_eq!(items.len(), 1);
        let attachments = items[0]["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments[0]["url"],
            "https://media.example.test/link/abc/media/1"
        );
        assert_eq!(attachments[0]["mime"], "video/mp4");
        assert!(attachments[0]["key"].is_null());
    }

    #[test]
    fn starred_items_returns_only_starred_rows_across_subscriptions() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        for (sub, account, url, title) in [
            ("feed-1", "rss-acct", "https://example.com/feed", "Feed One"),
            (
                "feed-2",
                "rss-other",
                "https://other.example/feed",
                "Feed Two",
            ),
        ] {
            conn.execute(
                "INSERT INTO subscriptions(id, account, url, title, enabled)
                 VALUES(?1, ?2, ?3, ?4, 1)",
                params![sub, account, url, title],
            )
            .unwrap();
        }
        let extra = |published_at: i64| RssItemExtra {
            author: String::new(),
            link: "https://example.com/post".to_string(),
            summary: "Caption".to_string(),
            content: String::new(),
            images: Vec::new(),
            videos: Vec::new(),
            published_at,
            updated_at: 0,
            fetched_at: published_at + 100,
        };
        store::upsert_rss_item(
            &conn,
            "rss-acct",
            "feed-1",
            "item-1",
            "Old starred",
            true,
            None,
            &extra(1_700_000_000),
        )
        .unwrap();
        store::upsert_rss_item(
            &conn,
            "rss-acct",
            "feed-1",
            "item-2",
            "Unstarred",
            true,
            None,
            &extra(1_700_000_500),
        )
        .unwrap();
        store::upsert_rss_item(
            &conn,
            "rss-other",
            "feed-2",
            "item-3",
            "New starred",
            true,
            None,
            &extra(1_700_001_000),
        )
        .unwrap();
        store::update_rss_item_starred(&conn, "rss-acct", "feed-1", "item-1", true).unwrap();
        store::update_rss_item_starred(&conn, "rss-other", "feed-2", "item-3", true).unwrap();

        let items = starred_items(&conn, 10).unwrap();
        assert_eq!(
            items
                .iter()
                .map(|m| m["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["rss-other#rss#feed-2#item-3", "rss-acct#rss#feed-1#item-1",]
        );
        assert!(items.iter().all(|m| m["starred"] == true));
        assert_eq!(items[0]["from_name"], "Feed Two");
    }

    #[test]
    fn recent_and_read_thread_build_bridge_messages() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-acct', 'https://example.com/feed', 'Example Feed', 1)",
            [],
        )
        .unwrap();
        store::upsert_rss_item(
            &conn,
            "rss-acct",
            "feed-1",
            "item-1",
            "Hello RSS",
            true,
            None,
            &RssItemExtra {
                author: String::new(),
                link: "https://example.com/post".to_string(),
                summary: "Hello world.".to_string(),
                content: String::new(),
                images: Vec::new(),
                videos: Vec::new(),
                published_at: 1_700_000_000,
                updated_at: 0,
                fetched_at: 1_700_000_100,
            },
        )
        .unwrap();

        let threads = recent(&conn, "rss-acct", "", 10).unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0]["subject"], "Example Feed");
        assert_eq!(threads[0]["from_name"], "Example Feed");
        assert_eq!(threads[0]["preview"], "Hello RSS - Hello world.");
        assert_eq!(threads[0]["unread"], true);

        let thread_id = threads[0]["thread_id"].as_str().unwrap().to_string();
        assert!(parse_thread_id(&thread_id).is_some());

        let items = read_thread(&conn, &thread_id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["subject"], "Hello RSS");
        assert_eq!(
            items[0]["body"],
            "Hello world.\n\nSource: https://example.com/post"
        );
        assert_eq!(items[0]["unread"], true);
    }

    // ---- test helpers for the additions below -------------------------------

    fn rss_conn_with_feed(account: &str, sub: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES(?1, 'rss', 'rss', 'RSS', '{}')",
            params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, feed_title, enabled)
             VALUES(?1, ?2, 'https://example.com/feed', 'Example Feed', 'Example Feed', 1)",
            params![sub, account],
        )
        .unwrap();
        conn
    }

    fn insert_item(conn: &Connection, account: &str, sub: &str, key: &str, published_at: i64) {
        store::upsert_rss_item(
            conn,
            account,
            sub,
            key,
            key,
            true,
            None,
            &RssItemExtra {
                author: String::new(),
                link: "https://example.com/post".to_string(),
                summary: "Caption".to_string(),
                content: String::new(),
                images: Vec::new(),
                videos: Vec::new(),
                published_at,
                updated_at: 0,
                fetched_at: published_at + 100,
            },
        )
        .unwrap();
    }

    // ---- read_thread_page pagination ----------------------------------------

    #[test]
    fn read_thread_page_walks_backwards_with_cursor() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
        insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);
        insert_item(&conn, "rss-acct", "feed-1", "item-3", 300);

        // First page (limit 2): the two newest items, ascending within the page,
        // plus a cursor pointing at the oldest item on the page.
        let (page1, cursor1) =
            read_thread_page(&conn, "rss-acct#rss#feed-1", None, Some(2)).unwrap();
        assert_eq!(
            page1
                .iter()
                .map(|m| m["subject"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["item-2", "item-3"],
        );
        let cursor1 = cursor1.expect("more items remain, cursor must be present");
        assert_eq!(cursor1, "ts:200:item-2");

        // Second page from that cursor: the remaining older item, no further cursor.
        let (page2, cursor2) = read_thread_page(
            &conn,
            "rss-acct#rss#feed-1",
            Some((200, "item-2".into())),
            Some(2),
        )
        .unwrap();
        assert_eq!(
            page2
                .iter()
                .map(|m| m["subject"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["item-1"],
        );
        assert!(cursor2.is_none(), "last page must not yield a cursor");
    }

    #[test]
    fn read_thread_page_unpaginated_returns_all_ascending() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
        insert_item(&conn, "rss-acct", "feed-1", "item-2", 300);
        insert_item(&conn, "rss-acct", "feed-1", "item-3", 200);

        let (items, cursor) = read_thread_page(&conn, "rss-acct#rss#feed-1", None, None).unwrap();
        assert!(cursor.is_none());
        assert_eq!(
            items
                .iter()
                .map(|m| m["subject"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["item-1", "item-3", "item-2"], // ts 100, 200, 300 ascending
        );
    }

    // ---- mark read / starred lifecycle --------------------------------------

    fn seen_starred(conn: &Connection, sub: &str, key: &str) -> (i64, i64) {
        conn.query_row(
            "SELECT seen, starred FROM messages WHERE folder = ?1 AND msg_id = ?2",
            params![sub, key],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn mark_items_then_thread_read() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
        insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

        // Marking one item read leaves the other untouched.
        mark_items_read(&conn, "rss-acct#rss#feed-1", &["item-1".into()], true).unwrap();
        assert_eq!(seen_starred(&conn, "feed-1", "item-1").0, 1);
        assert_eq!(seen_starred(&conn, "feed-1", "item-2").0, 0);

        // Marking the thread read flips the whole feed.
        mark_thread_read(&conn, "rss-acct#rss#feed-1", true).unwrap();
        assert_eq!(seen_starred(&conn, "feed-1", "item-1").0, 1);
        assert_eq!(seen_starred(&conn, "feed-1", "item-2").0, 1);
    }

    #[test]
    fn mark_items_then_thread_starred() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
        insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

        mark_items_starred(&conn, "rss-acct#rss#feed-1", &["item-2".into()], true).unwrap();
        assert_eq!(seen_starred(&conn, "feed-1", "item-1").1, 0);
        assert_eq!(seen_starred(&conn, "feed-1", "item-2").1, 1);

        mark_thread_starred(&conn, "rss-acct#rss#feed-1", true).unwrap();
        assert_eq!(seen_starred(&conn, "feed-1", "item-1").1, 1);
        assert_eq!(seen_starred(&conn, "feed-1", "item-2").1, 1);
    }

    #[test]
    fn mark_rejects_invalid_thread_id() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        assert!(mark_thread_read(&conn, "not-an-rss-id", true).is_err());
        assert!(mark_items_read(&conn, "bad", &["x".into()], true).is_err());
    }

    // ---- remove_feed --------------------------------------------------------

    #[test]
    fn remove_feed_deletes_subscription_and_items() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
        insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

        remove_feed(&conn, "rss-acct#rss#feed-1").unwrap();

        let subs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM subscriptions WHERE id = 'feed-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let msgs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE folder = 'feed-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(subs, 0, "subscription row must be deleted");
        assert_eq!(msgs, 0, "the feed's cached items must be deleted");
    }

    #[test]
    fn remove_feed_rejects_invalid_thread_id() {
        let conn = rss_conn_with_feed("rss-acct", "feed-1");
        assert!(remove_feed(&conn, "garbage").is_err());
    }

    // ---- OPML export round-trips with import --------------------------------

    #[test]
    fn export_opml_round_trips_through_import() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-src', 'rss', 'rss', 'My Feeds', '{}')",
            [],
        )
        .unwrap();
        // Two feeds with an XML-special character in a title to exercise escaping.
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, feed_title, site_url, enabled)
             VALUES('feed-1', 'rss-src', 'https://a.example/feed.xml', 'News & Co', 'News', 'https://a.example', 1),
                   ('feed-2', 'rss-src', 'https://b.example/feed.xml', 'Tech', 'Tech', '', 1)",
            [],
        )
        .unwrap();

        let opml = export_opml(&conn, "rss-src").unwrap();
        assert!(
            opml.contains("News &amp; Co"),
            "title must be XML-escaped: {opml}"
        );
        assert!(opml.contains("xmlUrl=\"https://a.example/feed.xml\""));

        // Import into a *separate* database — the `url` column is globally UNIQUE,
        // so re-importing the same feeds into the same DB is (correctly) a no-op.
        // The real round-trip is export-here, import-on-another-machine.
        let dst = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&dst).unwrap();
        dst.execute(
            "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-dst', 'rss', 'rss', 'Dest', '{}')",
            [],
        )
        .unwrap();
        let db = Mutex::new(dst);
        let imported = import_opml(&db, &opml, "rss-dst").unwrap();
        assert_eq!(
            imported, 2,
            "both feeds must round-trip into the new account"
        );

        let urls: Vec<String> = {
            let conn = db.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT url FROM subscriptions WHERE account = 'rss-dst' ORDER BY url")
                .unwrap();

            stmt.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        assert_eq!(
            urls,
            vec![
                "https://a.example/feed.xml".to_string(),
                "https://b.example/feed.xml".to_string(),
            ]
        );
    }

    #[test]
    fn export_opml_rejects_unknown_account() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        assert!(export_opml(&conn, "nope").is_err());
    }

    // ---- small pure helpers -------------------------------------------------

    #[test]
    fn first_line_collapses_whitespace_and_truncates() {
        assert_eq!(first_line("  hello   world \n again "), "hello world again");
        let long = "x".repeat(300);
        let preview = first_line(&long);
        assert_eq!(preview.chars().count(), 223); // 220 chars + "..."
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn mime_guesses_from_extension() {
        assert_eq!(image_mime("https://x/a.PNG?v=2"), "image/png");
        assert_eq!(image_mime("https://x/a.webp"), "image/webp");
        assert_eq!(image_mime("https://x/a.bin"), "image/jpeg"); // default
        assert_eq!(video_mime("https://x/a.webm"), "video/webm");
        assert_eq!(video_mime("https://x/a"), "video/mp4"); // default
    }

    // ---- end-to-end feed fetch over a real HTTP server ----------------------

    /// Spawn a throwaway HTTP/1.1 server that replies to every request with the
    /// same body. Returns the feed URL. The thread is detached (dies with the
    /// test process). Exercises the real `ureq` fetch path without any network.
    fn serve_feed(body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf); // drain the request line/headers
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        format!("http://{addr}/feed.xml")
    }

    const SAMPLE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Sample Feed</title>
    <link>https://example.com</link>
    <item>
      <title>First post</title>
      <link>https://example.com/1</link>
      <guid>guid-1</guid>
      <description>Hello from the first post.</description>
    </item>
    <item>
      <title>Second post</title>
      <link>https://example.com/2</link>
      <guid>guid-2</guid>
      <description>And the second.</description>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn add_fetches_parses_and_syncs_over_http() {
        // Keep prune_feed_media (called by sync_account) confined to a temp dir
        // instead of the real media cache.
        let media = std::env::temp_dir().join(format!("meron-rss-test-{}", std::process::id()));
        unsafe { std::env::set_var("MERON_MEDIA_DIR", &media) };

        let url = serve_feed(SAMPLE_RSS);
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        let db = Mutex::new(conn);

        // add() fetches the feed, creates the account, and stores both items.
        let account = add(&db, &url, "Sample Feed").unwrap();
        let account_id = account["id"].as_str().unwrap().to_string();
        assert!(account_id.starts_with("rss-"));

        let (subs, msgs): (i64, i64) = {
            let conn = db.lock().unwrap();
            let subs = conn
                .query_row(
                    "SELECT COUNT(*) FROM subscriptions WHERE account = ?1",
                    params![account_id],
                    |r| r.get(0),
                )
                .unwrap();
            let msgs = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE account = ?1",
                    params![account_id],
                    |r| r.get(0),
                )
                .unwrap();
            (subs, msgs)
        };
        assert_eq!(subs, 1, "one subscription created");
        assert_eq!(msgs, 2, "both feed items stored");

        // The parsed items surface as a thread with the feed's titles.
        let threads = {
            let conn = db.lock().unwrap();
            recent(&conn, &account_id, "", 10).unwrap()
        };
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0]["from_name"], "Sample Feed");

        // A re-sync of the unchanged feed finds no new items (guids dedupe).
        let new_items = sync_account(&db, &account_id).unwrap();
        assert_eq!(
            new_items, 0,
            "unchanged feed yields no new items on re-sync"
        );
    }
}
