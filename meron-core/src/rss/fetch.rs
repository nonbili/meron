use anyhow::{Context, Result, anyhow};
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

pub(super) struct FetchedFeed {
    pub(super) feed: Option<feed_rs::model::Feed>,
    pub(super) resolved_url: String,
    pub(super) etag: String,
    pub(super) last_modified: String,
    pub(super) not_modified: bool,
}

pub(super) struct HttpResponse {
    pub(super) status: u16,
    pub(super) etag: String,
    pub(super) last_modified: String,
    pub(super) body: Vec<u8>,
    pub(super) content_type: String,
}

pub(super) fn http_get(url: &str, etag: &str, last_modified: &str) -> Result<HttpResponse> {
    let mut rb = crate::proxy::agent()?.get(url);
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
pub(super) fn fetch_feed(raw_url: &str, etag: &str, last_modified: &str) -> Result<FetchedFeed> {
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
pub(super) fn discover_feed_url(page_url: &str, html: &str) -> Option<String> {
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
