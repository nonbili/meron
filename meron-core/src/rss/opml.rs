use anyhow::{Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::Event;
use rusqlite::{Connection, OptionalExtension, params};
use std::sync::Mutex;

use crate::store;

use super::{
    DEFAULT_RSS_ACCOUNT_TITLE, feed_host_label, normalize_feed_url, now_unix, rss_subscription_id,
};

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
