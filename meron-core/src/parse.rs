//! MIME parsing helpers built on `mailparse`.
//!
//! Two jobs: decode RFC 2047 encoded-words in header fragments (used for
//! ENVELOPE-derived list views, where async-imap hands us raw bytes), and turn
//! a full RFC822 message into a readable summary + body for the reader view.

use html_to_markdown_rs::{ConversionOptions, convert};
use mailparse::{DispositionType, MailHeaderMap, ParsedMail, addrparse, parse_header, parse_mail};
use std::path::{Path, PathBuf};

/// Default cap for files served from the media cache. The desktop bridge stores
/// this under the user's cache directory, so it must not grow without bound.
const MEDIA_CAP_BYTES: u64 = 1024 * 1024 * 1024;

/// Where inline images are written to disk and the message identity used to build
/// their on-disk path / served key (`account/folder/uid/index.ext`). The desktop
/// bridge serves these files at `/media/<key>`, so image bytes never travel back
/// through the JSON sidecar protocol. Built per `parse_message` call.
pub struct MediaCtx {
    pub root: PathBuf,
    pub account: String,
    pub folder: String,
    pub uid: u32,
}

/// Root directory for on-disk attachment files. The bridge sets `MERON_MEDIA_DIR`
/// so both sides agree; the fallback only matters when running the sidecar alone.
pub fn media_root() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_MEDIA_DIR") {
        return PathBuf::from(path);
    }
    let base = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.cache")
    });
    PathBuf::from(base).join("meron/attachments")
}

/// Parse an email `Date` header to Unix epoch seconds, returning 0 when it is
/// empty or unparseable. Mail dates are RFC 2822 (the IMAP envelope and the
/// `Date:` header both use it); RFC 3339/ISO is accepted as a fallback for the
/// occasional non-conformant sender. Storing an epoch lets the DB sort by date
/// directly and the frontend format it in local time.
pub fn parse_date_to_epoch(raw: &str) -> i64 {
    let raw = raw.trim();
    if raw.is_empty() {
        return 0;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(raw) {
        return dt.timestamp();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return dt.timestamp();
    }
    0
}

/// A single message rendered for the conversation/reader view.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    /// Comma-separated `To` recipients (each "Name <addr>" or "addr"), empty when
    /// unknown. Used to detect which of the user's addresses (alias) a message was
    /// delivered to, so replies can default the From accordingly.
    #[serde(default)]
    pub to: String,
    /// Comma-separated `Reply-To` addresses (each "Name <addr>" or "addr"),
    /// empty when the header is absent. Replies should prefer this over From.
    #[serde(default)]
    pub reply_to: String,
    /// Comma-separated `Cc` addresses from the original message, empty when none.
    /// Replies preserve this so other recipients stay on the thread.
    #[serde(default)]
    pub cc: String,
    /// Comma-separated `Bcc` addresses, present only on outgoing copies (Sent /
    /// Drafts) — the sending server strips `Bcc` before delivery, so received
    /// messages never carry it. Empty otherwise.
    #[serde(default)]
    pub bcc: String,
    /// Normalized `Message-ID` ("id@host", no angle brackets). Replies use this
    /// for `In-Reply-To` and the `References` chain so the thread holds together
    /// on the recipient side.
    #[serde(default)]
    pub message_id: String,
    /// Normalized `References` chain (space-separated bare ids, oldest → newest),
    /// when present. Empty when the header is absent (likely a root message).
    #[serde(default)]
    pub references: String,
    /// Send time as Unix epoch seconds (0 when the `Date` header is absent or
    /// unparseable). The frontend formats it for display in local time.
    pub date: i64,
    pub body: String,
    /// The message's original HTML, or `None` when it was plain text (in which case
    /// `body` already *is* the source). Persisted to the store so a render-logic
    /// change can re-derive `body` without an IMAP refetch, and so the reader's
    /// "HTML mode" tab can show the original email. At parse time it holds the raw
    /// HTML with `cid:` inline images rewritten to `/media/<key>`; the bridge
    /// handler injects a remote-image CSP `<meta>` on read (see `prepare_html`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,
    #[serde(default)]
    pub body_is_rendered: bool,
    pub preview: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub mime: String,
    pub size: usize,
    /// Relative media key (`account/folder/uid/index.ext`) for the bytes written
    /// to disk and served at `/media/<key>`; null only when no media context was
    /// provided (tests, previews) or the write failed.
    pub key: Option<String>,
}

/// Decode an RFC 2047 header fragment (e.g. an ENVELOPE subject) by letting
/// `mailparse` parse a synthetic header line — its `get_value()` decodes
/// encoded-words and charsets for us.
pub fn decode_words(raw: &str) -> String {
    let line = format!("Subject:{raw}");
    match parse_header(line.as_bytes()) {
        Ok((header, _)) => header.get_value(),
        Err(_) => raw.to_string(),
    }
}

/// Split a `From`-style value ("Display Name <addr@host>") into name and address.
pub fn split_address(raw: &str) -> (String, String) {
    if let Ok(list) = addrparse(raw) {
        if let Some(mailparse::MailAddr::Single(info)) = list.first() {
            let name = info.display_name.clone().unwrap_or_default();
            return (name, info.addr.clone());
        }
    }
    (String::new(), raw.to_string())
}

/// Parse a full RFC822 message into a reader-view summary. Inline images are
/// written under `media` (when provided) and referenced by key; passing `None`
/// (tests, previews) skips disk writes and leaves every attachment key null.
pub fn parse_message(raw: &[u8], media: Option<&MediaCtx>) -> Message {
    let mail = match parse_mail(raw) {
        Ok(mail) => mail,
        Err(_) => return Message::default(),
    };
    let headers = &mail.headers;
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    let (from_name, from_addr) =
        split_address(&headers.get_first_value("From").unwrap_or_default());
    let to = collect_address_list(&headers.get_all_values("To"));
    let reply_to = collect_address_list(&headers.get_all_values("Reply-To"));
    let cc = collect_address_list(&headers.get_all_values("Cc"));
    let bcc = collect_address_list(&headers.get_all_values("Bcc"));
    let message_id = normalize_msgid(&headers.get_first_value("Message-ID").unwrap_or_default());
    let references =
        normalize_references(&headers.get_first_value("References").unwrap_or_default());
    let date = parse_date_to_epoch(&headers.get_first_value("Date").unwrap_or_default());
    let sources = body_sources(&mail);

    let mut attachments = Vec::new();
    let mut cid_keys: Vec<(String, String)> = Vec::new();
    collect_attachments(&mail, &mut attachments, &mut cid_keys, media);
    if let Some(ctx) = media {
        prune_media_cache(&ctx.root);
    }

    // Prefer the MIME text/plain alternative for the conversation and Plain
    // reader view. Keep HTML separately for the HTML reader; when a message is
    // HTML-only, fall back to converting HTML so there is still readable text.
    let (body, body_html, body_is_rendered) = match (sources.plain, sources.html) {
        (Some(plain), Some(html)) => {
            let plain = normalize_text(&plain);
            let html = rewrite_cid_refs(&html, &cid_keys);
            (plain, Some(html), false)
        }
        (None, Some(html)) => {
            let html = rewrite_cid_refs(&html, &cid_keys);
            (render_body(&html), Some(html), true)
        }
        (Some(plain), None) => {
            let plain = normalize_text(&plain);
            (plain, None, false)
        }
        (None, None) => (String::new(), None, false),
    };
    let preview = preview_of(&body);

    Message {
        subject,
        from_name,
        from_addr,
        to,
        reply_to,
        cc,
        bcc,
        message_id,
        references,
        date,
        body,
        body_html,
        body_is_rendered,
        preview,
        attachments,
    }
}

/// Strip angle brackets / whitespace from a single Message-ID header value,
/// leaving the bare `id@host`. Returns empty for malformed input.
fn normalize_msgid(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string()
}

/// Normalize a `References:` header into space-separated bare ids (no angle
/// brackets), preserving order. The header is whitespace-separated `<id>` tokens.
fn normalize_references(raw: &str) -> String {
    raw.split_whitespace()
        .map(|tok| tok.trim_start_matches('<').trim_end_matches('>').trim())
        .filter(|tok| !tok.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Flatten one or more header values (e.g. multiple `Cc:` lines) into a single
/// comma-separated address list, with RFC 2047 encoded-words decoded. Group
/// syntax and nested groups are flattened to their member addresses; if parsing
/// fails entirely the raw header value is preserved so the user still sees it.
fn collect_address_list(values: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match addrparse(trimmed) {
            Ok(list) => {
                for entry in list.iter() {
                    push_addr(entry, &mut out);
                }
            }
            Err(_) => out.push(trimmed.to_string()),
        }
    }
    out.join(", ")
}

fn push_addr(entry: &mailparse::MailAddr, out: &mut Vec<String>) {
    match entry {
        mailparse::MailAddr::Single(info) => {
            let formatted = match info.display_name.as_deref() {
                Some(name) if !name.trim().is_empty() => {
                    format!("{} <{}>", name.trim(), info.addr)
                }
                _ => info.addr.clone(),
            };
            if !formatted.is_empty() {
                out.push(formatted);
            }
        }
        mailparse::MailAddr::Group(group) => {
            for addr in &group.addrs {
                let formatted = match addr.display_name.as_deref() {
                    Some(name) if !name.trim().is_empty() => {
                        format!("{} <{}>", name.trim(), addr.addr)
                    }
                    _ => addr.addr.clone(),
                };
                if !formatted.is_empty() {
                    out.push(formatted);
                }
            }
        }
    }
}

/// Rewrite `cid:<id>` references (e.g. `src="cid:logo"`) to the `/media/<key>` path
/// the matching inline image was written to, so HTML mode can load them.
fn rewrite_cid_refs(html: &str, cid_keys: &[(String, String)]) -> String {
    let mut out = html.to_string();
    for (cid, key) in cid_keys {
        if cid.is_empty() {
            continue;
        }
        out = out.replace(&format!("cid:{cid}"), &format!("/media/{key}"));
    }
    out
}

/// Strip script-bearing markup from email HTML while preserving layout/styling.
/// This is the independent second boundary behind the iframe's `default-src 'none'`
/// CSP: even if the CSP fails to apply, the markup itself carries no `<script>`,
/// no `on*` handlers, no `javascript:`/`vbscript:` URLs, and no embedding tags
/// (`<iframe>`/`<object>`/`<embed>`/`<form>`/`<meta>`/`<base>`/`<link>`), so the
/// `allow-same-origin allow-scripts` iframe can't be turned into an XSS vector.
///
/// CSS is intentionally preserved — both `<style>` blocks and inline `style=`
/// attributes — because the CSP already permits `style-src 'unsafe-inline'` and
/// email layout depends on it; CSS carries no script execution in WebKit. Inline
/// `data:image/...` sources are kept on `<img>` only; `data:` is stripped from
/// every other slot so a `data:text/html` href can't navigate the frame to an
/// un-CSP'd document. `cid:` refs are already rewritten to `/media/<key>` by the
/// time this runs, so relative URLs must pass through untouched.
fn sanitize_email_html(source: &str) -> String {
    use std::borrow::Cow;

    let mut builder = ammonia::Builder::default();
    builder
        // Tags ammonia drops by default that email layout still leans on.
        .add_tags(["style", "font", "center"])
        // By default ammonia also empties `<style>` content; keep the CSS.
        .rm_clean_content_tags(["style"])
        // Presentational attributes allowed on any element; styling must survive.
        .add_generic_attributes([
            "style",
            "class",
            "id",
            "align",
            "valign",
            "bgcolor",
            "background",
            "color",
            "face",
            "size",
            "width",
            "height",
            "border",
            "cellpadding",
            "cellspacing",
            "nowrap",
        ])
        // `data:` is allowed through the scheme filter but then constrained to
        // image sources by the attribute filter below.
        .url_schemes(
            ["http", "https", "mailto", "tel", "data"]
                .into_iter()
                .collect(),
        )
        // `/media/<key>` inline-image refs are relative and must pass through.
        .url_relative(ammonia::UrlRelative::PassThrough)
        .attribute_filter(|element, attribute, value| {
            if value.trim_start().to_ascii_lowercase().starts_with("data:") {
                // Keep `data:image/...` only on `<img src>`; drop it anywhere else
                // (e.g. an `href`) so it can't become a navigable script document.
                let ok = element == "img"
                    && attribute == "src"
                    && value
                        .trim_start()
                        .to_ascii_lowercase()
                        .starts_with("data:image/");
                return if ok { Some(Cow::Borrowed(value)) } else { None };
            }
            Some(Cow::Borrowed(value))
        });
    builder.clean(source).to_string()
}

/// Wrap stored HTML for safe iframe rendering: inject a CSP `<meta>` that allows
/// same-origin (`/media`), data, and blob images/media always, and remote (`https:`)
/// images/media only when the account opts in. The reader renders this in a `sandbox`ed
/// iframe with `allow-scripts` (needed so the host's link-click listener fires under
/// WebKitGTK); the `default-src 'none'` CSP still blocks all email JS, so the
/// CSP also gates remote image/media loads.
pub fn prepare_html(source: &str, load_remote_images: bool) -> String {
    // Defence in depth: strip script-bearing markup *before* the CSP `<meta>` is
    // injected, so a CSP bypass alone can't run the email's JS. CSS is kept (the
    // CSP allows `style-src 'unsafe-inline'`); only script vectors are removed.
    let source = &sanitize_email_html(source);
    let (img, media) = if load_remote_images {
        (
            "'self' data: http: https:",
            "'self' data: blob: http: https:",
        )
    } else {
        ("'self' data:", "'self' data: blob:")
    };
    // `script-src`/`object-src`/`frame-src 'none'` are explicit for robustness
    // (they already inherit from `default-src`); `base-uri` and `form-action` do
    // NOT fall back to `default-src`, so they must be set to neutralise a `<base>`
    // hijack or a form posting out of the frame.
    let csp = format!(
        "default-src 'none'; script-src 'none'; object-src 'none'; frame-src 'none'; \
         base-uri 'none'; form-action 'none'; \
         img-src {img}; media-src {media}; style-src 'unsafe-inline'; font-src 'self' data:;"
    );
    // Keep oversized inline images within the reader width (preserving aspect), and
    // hint that they open the gallery. `style-src 'unsafe-inline'` covers this block.
    let head = format!(
        "<meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         <style>img,video{{max-width:100%;height:auto}}img{{cursor:zoom-in}}</style>"
    );
    inject_head(source, &head)
}

/// Insert `head_extra` into the document head: after an existing `<head ...>`, or
/// after `<html ...>` as a new head, or as a prepended document otherwise.
fn inject_head(html: &str, head_extra: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(head) = lower.find("<head") {
        if let Some(rel) = lower[head..].find('>') {
            let at = head + rel + 1;
            return format!("{}{head_extra}{}", &html[..at], &html[at..]);
        }
    }
    if let Some(htmltag) = lower.find("<html") {
        if let Some(rel) = lower[htmltag..].find('>') {
            let at = htmltag + rel + 1;
            return format!("{}<head>{head_extra}</head>{}", &html[..at], &html[at..]);
        }
    }
    format!("<!doctype html><html><head>{head_extra}</head><body>{html}</body></html>")
}

/// Render stored HTML back into display markdown. Re-run by the store when a
/// render-version bump cleared `body` but the HTML source survives. (Plain-text
/// messages have no stored HTML — their `body` is never invalidated.)
pub fn render_body(html: &str) -> String {
    html_to_text(html)
}

/// Walk the MIME tree collecting non-text leaf parts (images and attachments).
/// Image bytes are written to disk under `media` and referenced by key so they
/// never round-trip through the JSON bridge; non-images stay metadata-only.
fn collect_attachments(
    part: &ParsedMail,
    out: &mut Vec<Attachment>,
    cid_keys: &mut Vec<(String, String)>,
    media: Option<&MediaCtx>,
) {
    if !part.subparts.is_empty() {
        for sub in &part.subparts {
            collect_attachments(sub, out, cid_keys, media);
        }
        return;
    }

    let mime = part.ctype.mimetype.to_ascii_lowercase();
    let disposition = part.get_content_disposition();
    let is_attachment = disposition.disposition == DispositionType::Attachment;
    let is_image = mime.starts_with("image/");

    // Skip the text/html/plain body parts unless explicitly an attachment.
    if mime.starts_with("text/") && !is_attachment {
        return;
    }
    if !is_attachment && !is_image {
        return;
    }

    let bytes = match part.get_body_raw() {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let size = bytes.len();
    let filename = disposition
        .params
        .get("filename")
        .cloned()
        .or_else(|| part.ctype.params.get("name").cloned())
        .unwrap_or_else(|| "attachment".to_string());

    let index = out.len();
    // Persist every attachment (images and files alike) so the bridge can serve
    // it at `/media/<key>` and the user can download it. Without media context
    // (tests, previews) the key stays null and only metadata is kept.
    let key = media.and_then(|ctx| write_media(ctx, index, &filename, &mime, &bytes));

    // Map this inline image's Content-ID to its served key so HTML mode can
    // rewrite `cid:` references (`<foo@host>` headers — strip the angle brackets).
    if let Some(key) = &key {
        if let Some(cid) = part.headers.get_first_value("Content-ID") {
            let cid = cid
                .trim()
                .trim_start_matches('<')
                .trim_end_matches('>')
                .trim();
            if !cid.is_empty() {
                cid_keys.push((cid.to_string(), key.clone()));
            }
        }
    }

    out.push(Attachment {
        filename,
        mime,
        size,
        key,
    });
}

/// Write one attachment's bytes to `<root>/<account>/<folder>/<uid>/<index>.<ext>`
/// and return the relative key, or `None` if the write fails (the part is then
/// just dropped from the rendered view rather than failing the whole message).
/// The extension comes from the original filename when present (so downloads keep
/// a sensible type), falling back to the MIME subtype.
fn write_media(
    ctx: &MediaCtx,
    index: usize,
    filename: &str,
    mime: &str,
    bytes: &[u8],
) -> Option<String> {
    let ext = sanitize_segment(
        Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.is_empty())
            .or_else(|| mime.rsplit('/').next().filter(|e| !e.is_empty()))
            .unwrap_or("bin"),
    );
    let key = format!(
        "{}/{}/{}/{}.{}",
        sanitize_segment(&ctx.account),
        sanitize_segment(&ctx.folder),
        ctx.uid,
        index,
        ext
    );
    let path = ctx.root.join(&key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, bytes).ok()?;
    Some(key)
}

/// True when every attachment's bytes are still on disk. Used before serving
/// cached mail rows; when false the caller refetches and re-parses the message.
/// Two cases force a refetch: a keyed file that pruning removed independently of
/// SQLite, and a key-less attachment — a row cached before attachments were
/// persisted to disk, whose bytes were never written and must be fetched now.
pub fn cached_media_available(root: &Path, message: &Message) -> bool {
    message
        .attachments
        .iter()
        .all(|att| match att.key.as_deref() {
            Some(key) => root.join(key).is_file(),
            None => false,
        })
}

/// Keep the shared media cache under its configured cap by removing oldest files
/// first. The cap is bytes and can be overridden with `MERON_MEDIA_CAP`.
pub fn prune_media_cache(root: &Path) {
    let cap = std::env::var("MERON_MEDIA_CAP")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(MEDIA_CAP_BYTES);

    let mut files = Vec::new();
    let mut total = 0u64;
    collect_media_files(root, &mut files, &mut total);
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

fn collect_media_files(
    dir: &Path,
    out: &mut Vec<(PathBuf, u64, std::time::SystemTime)>,
    total: &mut u64,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_media_files(&path, out, total);
        } else if meta.is_file() {
            *total += meta.len();
            out.push((
                path,
                meta.len(),
                meta.modified().unwrap_or(std::time::UNIX_EPOCH),
            ));
        }
    }
}

/// Reduce a path segment to filesystem- and URL-safe characters so account names
/// and IMAP folder paths (which may contain `/`, brackets, spaces) can't escape
/// the media root or break the served URL.
pub(crate) fn sanitize_segment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn html_to_text(html: &str) -> String {
    let options = ConversionOptions::builder()
        .extract_metadata(false)
        .skip_images(false)
        // Render bare links as `[text](url)` rather than `<url>` autolinks — the
        // reader's link tokenizer handles the former, but leaves the angle
        // brackets of the latter as stray text.
        .autolinks(false)
        .strip_tags(vec!["style".to_string(), "script".to_string()])
        .build();
    let clean_html = flatten_tables(&strip_invisible_html(html));
    let mut markdown = convert(&clean_html, Some(options.clone()))
        .ok()
        .and_then(|result| result.content)
        .unwrap_or_default();
    if markdown.trim().is_empty() {
        if let Some(body) = body_inner_html(&clean_html) {
            markdown = convert(&format!("<div>{body}</div>"), Some(options))
                .ok()
                .and_then(|result| result.content)
                .unwrap_or_default();
        }
    }
    normalize_text(&compact_image_links(&strip_heading_markers(
        &rewrite_image_markdown(&markdown),
    )))
}

fn is_whitespace_or_invisible(c: char) -> bool {
    c.is_whitespace()
        || c == '\u{200b}' // zero-width space
        || c == '\u{200c}' // zero-width non-joiner
        || c == '\u{200d}' // zero-width joiner
        || c == '\u{feff}' // zero-width no-break space
}

fn normalize_text(text: &str) -> String {
    let mut final_lines = Vec::new();
    let mut consecutive_empty = 0;

    for line in text.lines() {
        let trimmed = line.trim_end_matches(is_whitespace_or_invisible);
        if trimmed.is_empty() {
            consecutive_empty += 1;
            if consecutive_empty <= 1 {
                final_lines.push("");
            }
        } else {
            consecutive_empty = 0;
            final_lines.push(trimmed);
        }
    }

    // Trim leading/trailing empty lines
    let mut start = 0;
    while start < final_lines.len() && final_lines[start].is_empty() {
        start += 1;
    }
    let mut end = final_lines.len();
    while end > start && final_lines[end - 1].is_empty() {
        end -= 1;
    }

    final_lines[start..end].join("\n").trim().to_string()
}

fn rewrite_image_markdown(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find("![") {
        out.push_str(&rest[..start]);
        let after_marker = &rest[start + 2..];
        let Some(alt_end) = after_marker.find("](") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let alt = &after_marker[..alt_end];
        let after_alt = &after_marker[alt_end + 2..];
        let Some(url_end) = after_alt.find(')') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let label = alt.trim();
        // Drop decorative/icon images (no alt text) entirely — e.g. GitHub's
        // issue-type icon or tracking pixels. Keeping them produced noise, and
        // when they sit inside an anchor (`[![](icon) text](url)`) they broke the
        // surrounding link. Named images become a plain `[image: alt]` marker
        // (Gmail's plain-text convention) rather than a link — we don't auto-load
        // remote images, and inline ones already show as thumbnails below.
        let tail = &after_alt[url_end + 1..];
        if label.is_empty() {
            if out.ends_with('[') && tail.starts_with("](") {
                // This is an empty image that is the sole content of a link: `[![](src)](url)`.
                // We want to drop the entire link since it has no readable text.
                out.pop(); // Pop the leading `[`
                if let Some(anchor_url_end) = tail[2..].find(')') {
                    rest = &tail[2 + anchor_url_end + 1..];
                } else {
                    rest = tail;
                }
            } else {
                // Normal decorative/icon image: just drop it.
                // Also swallow a single trailing space the icon left behind, so a
                // dropped leading icon doesn't open its link with `[ text`.
                rest = tail.strip_prefix(' ').unwrap_or(tail);
            }
        } else if out.ends_with('[') && tail.starts_with("](") {
            // Linked image (`[![alt](src)](url)`): the image is the sole content
            // of an anchor (e.g. a logo that links somewhere). Emit the marker as
            // the link's text — the surrounding `[` ... `](url)` already wrap it —
            // so the reader renders a clickable `[image: alt](url)` link instead
            // of dropping the destination.
            out.push_str(&format!("image: {label}"));
            rest = tail;
        } else {
            out.push_str(&format!("[image: {label}]"));
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn strip_heading_markers(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                trimmed.trim_start_matches('#').trim_start()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compact_image_links(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut previous_image_link = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        let image_link = trimmed.starts_with("[image: ");
        if image_link && previous_image_link {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            out.push_str(trimmed);
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
        previous_image_link = image_link;
    }
    out
}

fn strip_invisible_html(html: &str) -> String {
    strip_html_block(&strip_html_block(html, "style"), "script")
}

/// Neutralize HTML table layout so the markdown converter doesn't render it as a
/// markdown table. Emails (Google, newsletters, Outlook) use tables purely for
/// layout; rendering them as tables crams whole paragraphs into single cells,
/// strips block structure (text runs together), and emits `| --- |` noise the
/// reader can't render. Cells/rows become `<div>` to keep block separation;
/// table containers are dropped. Tag-only rewrite — inner content is preserved.
fn flatten_tables(html: &str) -> String {
    const DROP: &[&str] = &[
        "table", "thead", "tbody", "tfoot", "colgroup", "col", "caption",
    ];
    const BLOCK: &[&str] = &["tr", "td", "th"];

    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let inner = &after[1..gt];
        let closing = inner.trim_start().starts_with('/');
        let name: String = inner
            .trim_start()
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();

        if DROP.contains(&name.as_str()) {
            // drop the tag, keep content
        } else if BLOCK.contains(&name.as_str()) {
            out.push_str(if closing { "</div>" } else { "<div>" });
        } else {
            out.push_str(&after[..=gt]);
        }
        rest = &after[gt + 1..];
    }
    out.push_str(rest);
    out
}

fn body_inner_html(html: &str) -> Option<&str> {
    let lower = html.to_ascii_lowercase();
    let body_start = lower.find("<body")?;
    let open_end = lower[body_start..].find('>')? + body_start + 1;
    let close_start = lower[open_end..].find("</body>")? + open_end;
    Some(&html[open_end..close_start])
}

fn strip_html_block(html: &str, tag: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find(&open_pattern) else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..start]);
        let Some(end_rel) = lower[start..].find(&close_pattern) else {
            return out;
        };
        let end = start + end_rel + close_pattern.len();
        rest = &rest[end..];
    }
}

#[derive(Default)]
struct BodySources {
    plain: Option<String>,
    html: Option<String>,
}

/// Pick body sources, walking multipart trees. Multipart/alternative usually
/// carries both text/plain and text/html; the plain part is the source for the
/// conversation/plain reader, and HTML is kept for the HTML reader.
fn body_sources(part: &mailparse::ParsedMail) -> BodySources {
    let plain = find_text_part(part, "text/plain");
    let html = find_text_part(part, "text/html");
    if plain.is_some() || html.is_some() {
        return BodySources { plain, html };
    }

    let ctype = part.ctype.mimetype.to_ascii_lowercase();
    if part.subparts.is_empty() && ctype.starts_with("text/") {
        let content = part.get_body().unwrap_or_default();
        if content.contains("<html")
            || content.contains("<body")
            || content.contains("<p>")
            || content.contains("</div>")
        {
            return BodySources {
                plain: None,
                html: Some(content),
            };
        }
        return BodySources {
            plain: Some(content),
            html: None,
        };
    }
    BodySources::default()
}

fn find_text_part(part: &ParsedMail, mime: &str) -> Option<String> {
    if part.ctype.mimetype.eq_ignore_ascii_case(mime) {
        return part.get_body().ok();
    }
    for sub in &part.subparts {
        if let Some(body) = find_text_part(sub, mime) {
            return Some(body);
        }
    }
    None
}

fn preview_of(body: &str) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview: String = collapsed.chars().take(200).collect();
    if collapsed.chars().count() > 200 {
        preview.push('…');
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_html_to_clean_text() {
        let html = "<html><head><style>body { color: red; }</style></head><body><h1>Hello World</h1><p>This is a <a href=\"https://example.com\">link</a>.<br>New line &amp; &lt;escaped&gt;.</p><div>Another block</div></body></html>";
        let text = html_to_text(html);
        assert_eq!(
            text,
            "Hello World\n\nThis is a [link](https://example.com).\nNew line & <escaped>.\n\nAnother block"
        );
    }

    #[test]
    fn converts_html_images_to_image_markers() {
        let html = r#"<p><strong>Alice Example</strong> created an issue <a href="https://example.com/i">(example/project#266)</a></p><p><img alt="Screenshot.png" src="https://example.com/Screenshot.png"></p>"#;
        let text = html_to_text(html);
        assert_eq!(
            text,
            "**Alice Example** created an issue [(example/project#266)](https://example.com/i)\n\n[image: Screenshot.png]"
        );
    }

    #[test]
    fn keeps_link_on_named_linked_image() {
        // A named image that is the sole content of an anchor (e.g. a Google logo
        // linking to a destination) keeps its link as `[image: alt](url)`.
        let html = r#"<p><a href="https://google.com"><img alt="Google" src="cid:logo"></a></p>"#;
        let text = html_to_text(html);
        assert_eq!(text, "[image: Google](https://google.com)");
    }

    #[test]
    fn drops_decorative_icon_images() {
        // Empty-alt image (e.g. GitHub's issue-type icon) is dropped, leaving the
        // surrounding link intact rather than a stray "Image (view on web)".
        let html = r#"<p><a href="https://x/i"><img alt="" src="https://x/icon.png"> <strong>Alice Example</strong> created an issue (example/project#266)</a></p>"#;
        let text = html_to_text(html);
        assert_eq!(
            text,
            "[**Alice Example** created an issue (example/project#266)](https://x/i)"
        );
    }

    #[test]
    fn captures_html_source_for_rerender() {
        let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/html\r\n\r\n<p><strong>hi</strong></p>";
        let msg = parse_message(raw, None);
        let html = msg.body_html.as_deref().expect("html kept");
        assert!(html.contains("<strong>hi</strong>"));
        // Re-rendering the stored HTML reproduces the body.
        assert_eq!(render_body(html), msg.body);
        assert_eq!(msg.body, "**hi**");
    }

    #[test]
    fn plain_text_keeps_no_html_source() {
        let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\nhello world";
        let msg = parse_message(raw, None);
        assert!(msg.body_html.is_none());
        assert_eq!(msg.body, "hello world");
    }

    #[test]
    fn multipart_alternative_prefers_plain_body_and_keeps_html() {
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
plain part\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p><strong>html part</strong></p>\r\n\
--sep--\r\n";
        let msg = parse_message(raw, None);
        assert_eq!(msg.body, "plain part");
        assert!(
            msg.body_html
                .as_deref()
                .unwrap_or_default()
                .contains("<strong>html part</strong>")
        );
    }

    #[test]
    fn strips_style_blocks_with_attributes() {
        let html = "<html><head><style type=\"text/css\">body { color: red; }</style></head><body>Hello World</body></html>";
        let text = html_to_text(html);
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn decodes_rfc2047_subject() {
        // "=?UTF-8?B?SGVsbMO2?=" is base64 for "Hellö".
        assert_eq!(decode_words(" =?UTF-8?B?SGVsbMO2?="), "Hellö");
        assert_eq!(decode_words(" Plain subject"), "Plain subject");
    }

    #[test]
    fn splits_display_name_and_addr() {
        let (name, addr) = split_address("Maya Chen <maya@example.com>");
        assert_eq!(name, "Maya Chen");
        assert_eq!(addr, "maya@example.com");

        let (name, addr) = split_address("bare@example.com");
        assert_eq!(name, "");
        assert_eq!(addr, "bare@example.com");
    }

    #[test]
    fn writes_inline_image_to_disk_and_keys_it() {
        // "AQID" is base64 for the bytes [1, 2, 3].
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/mixed; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p>hi</p>\r\n\
--sep\r\n\
Content-Type: image/png\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-Disposition: inline; filename=\"a.png\"\r\n\
\r\n\
AQID\r\n\
--sep--\r\n";
        let root = std::env::temp_dir().join(format!("meron-media-test-{}", std::process::id()));
        let ctx = MediaCtx {
            root: root.clone(),
            account: "acct@host".to_string(),
            folder: "[Gmail]/All Mail".to_string(),
            uid: 42,
        };
        let msg = parse_message(raw, Some(&ctx));
        assert_eq!(msg.attachments.len(), 1);
        let key = msg.attachments[0].key.as_deref().expect("image keyed");
        // Unsafe path chars in account/folder are flattened to `_`.
        assert_eq!(key, "acct_host/_Gmail__All_Mail/42/0.png");
        let bytes = std::fs::read(root.join(key)).expect("file written");
        assert_eq!(bytes, vec![1, 2, 3]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rewrites_cid_refs_in_html_source_to_media_keys() {
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/related; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p><img src=\"cid:logo123\"></p>\r\n\
--sep\r\n\
Content-Type: image/png\r\n\
Content-Transfer-Encoding: base64\r\n\
Content-ID: <logo123>\r\n\
Content-Disposition: inline; filename=\"a.png\"\r\n\
\r\n\
AQID\r\n\
--sep--\r\n";
        let root = std::env::temp_dir().join(format!("meron-cid-test-{}", std::process::id()));
        let ctx = MediaCtx {
            root: root.clone(),
            account: "acct".to_string(),
            folder: "inbox".to_string(),
            uid: 7,
        };
        let msg = parse_message(raw, Some(&ctx));
        let html = msg.body_html.as_deref().expect("html kept");
        assert!(html.contains("/media/acct/inbox/7/0.png"));
        assert!(!html.contains("cid:logo123"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prepare_html_gates_remote_images_via_csp() {
        let off = prepare_html("<p>hi</p>", false);
        assert!(off.contains("Content-Security-Policy"));
        assert!(off.contains("img-src 'self' data:;"));
        assert!(off.contains("media-src 'self' data: blob:;"));
        assert!(!off.contains("https:"));

        let on = prepare_html("<html><head></head><body>hi</body></html>", true);
        assert!(on.contains("img-src 'self' data: http: https:;"));
        assert!(on.contains("media-src 'self' data: blob: http: https:;"));
    }

    #[test]
    fn prepare_html_strips_script_vectors_keeps_styling() {
        let out = prepare_html(
            "<style>p{color:red}</style>\
             <p style=\"font-weight:bold\" onclick=\"steal()\">hi</p>\
             <script>alert(1)</script>\
             <a href=\"javascript:alert(2)\">x</a>\
             <a href=\"https://example.com\">ok</a>\
             <iframe src=\"https://evil\"></iframe>\
             <form action=\"https://evil\"></form>",
            false,
        );
        // Script-bearing markup is gone.
        assert!(!out.contains("<script"));
        assert!(!out.contains("onclick"));
        assert!(!out.contains("javascript:"));
        assert!(!out.contains("<iframe"));
        assert!(!out.contains("<form"));
        // Styling and safe links survive.
        assert!(out.contains("<style>"));
        assert!(out.contains("color:red"));
        assert!(out.contains("font-weight:bold"));
        assert!(out.contains("https://example.com"));
    }

    #[test]
    fn prepare_html_keeps_data_image_but_strips_data_href() {
        let out = prepare_html(
            "<img src=\"data:image/png;base64,AAAA\">\
             <a href=\"data:text/html,<script>alert(1)</script>\">x</a>\
             <img src=\"/media/acct/inbox/1/0.png\">",
            true,
        );
        assert!(out.contains("data:image/png;base64,AAAA"));
        assert!(out.contains("/media/acct/inbox/1/0.png"));
        // The data:text/html navigation vector is dropped, href and all.
        assert!(!out.contains("data:text/html"));
    }

    #[test]
    fn cached_media_available_requires_keyed_files() {
        let root = std::env::temp_dir().join(format!(
            "meron-media-availability-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("acct/inbox/1")).unwrap();
        std::fs::write(root.join("acct/inbox/1/0.png"), [1, 2, 3]).unwrap();

        let mut msg = Message::default();
        msg.attachments.push(Attachment {
            filename: "a.png".to_string(),
            mime: "image/png".to_string(),
            size: 3,
            key: Some("acct/inbox/1/0.png".to_string()),
        });
        assert!(cached_media_available(&root, &msg));

        std::fs::remove_file(root.join("acct/inbox/1/0.png")).unwrap();
        assert!(!cached_media_available(&root, &msg));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cached_media_unavailable_for_keyless_attachment() {
        // A row cached before attachments were persisted has a key-less file;
        // it must report unavailable so the caller refetches and writes it.
        let root = std::env::temp_dir().join(format!("meron-keyless-{}", std::process::id()));
        let mut msg = Message::default();
        msg.attachments.push(Attachment {
            filename: "invite.ics".to_string(),
            mime: "text/calendar".to_string(),
            size: 3072,
            key: None,
        });
        assert!(!cached_media_available(&root, &msg));
    }

    #[test]
    fn parses_multipart_message_summary() {
        let raw = b"From: Maya Chen <maya@example.com>\r\n\
Subject: =?UTF-8?B?TGF1bmNoIG5vdGVz?=\r\n\
Date: Mon, 18 May 2026 09:42:00 +0000\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
The launch notes are ready for review.\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p><strong>The launch notes</strong> are ready for review.</p>\r\n\
--sep--\r\n";
        let msg = parse_message(raw, None);
        assert_eq!(msg.subject, "Launch notes");
        assert_eq!(msg.from_name, "Maya Chen");
        assert_eq!(msg.from_addr, "maya@example.com");
        assert_eq!(msg.body, "The launch notes are ready for review.");
        assert!(
            msg.body_html
                .as_deref()
                .unwrap_or_default()
                .contains("<strong>The launch notes</strong>")
        );
        assert!(msg.preview.starts_with("The launch notes are ready"));
    }

    #[test]
    fn flattens_layout_tables_into_blocks() {
        // Email layout tables must not become markdown tables: paragraphs in a
        // cell stay separate blocks, and there are no `|`/`---` table artifacts.
        let html =
            "<table><tr><td><p>First paragraph.</p><p>Second paragraph.</p></td></tr></table>";
        let text = html_to_text(html);
        assert_eq!(text, "First paragraph.\n\nSecond paragraph.");
        assert!(!text.contains('|'));
    }

    #[test]
    fn normalizes_whitespace_and_thin_spaces() {
        let input = "ZHANG XINGYAO\n\n\u{200a}\n\nAmount\n\n6,383.25 USD\n\n\u{200b}\n\nDate";
        let normalized = normalize_text(input);
        assert_eq!(
            normalized,
            "ZHANG XINGYAO\n\nAmount\n\n6,383.25 USD\n\nDate"
        );
    }

    #[test]
    fn drops_empty_linked_images() {
        // A linked image with no alt text (e.g. social media icon links in Airwallex emails)
        // should have the entire empty link dropped rather than leaving a stray `[](url)`.
        let html = r#"<p><a href="https://facebook.com/airwallex"><img alt="" src="https://example.com/Facebook.png"></a></p>"#;
        let text = html_to_text(html);
        assert_eq!(text, "");
    }
}
