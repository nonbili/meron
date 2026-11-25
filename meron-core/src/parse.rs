//! MIME parsing helpers built on `mailparse`.
//!
//! Two jobs: decode RFC 2047 encoded-words in bare header fragments, and turn
//! a full RFC822 message into a readable summary + body for the reader view.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use html_to_markdown_rs::{ConversionOptions, convert};
use mailparse::body::Body;
use mailparse::{
    DispositionType, MailAddr, MailAddrList, MailHeader, MailHeaderMap, ParsedMail, addrparse,
    addrparse_header, parse_header, parse_mail,
};
use std::path::{Path, PathBuf};

mod address;
mod html;
mod media;
mod text;
pub use address::*;
pub use html::*;
pub use media::*;
pub use text::*;

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
    /// Whether the message carries headers a receiving MTA adds at delivery
    /// (`Return-Path`, `Delivered-To`, `X-Original-To`) — i.e. this copy was
    /// delivered to us rather than written by us. Direction can't be read off
    /// the From address alone: a shared address configured here as a send-as
    /// alias is also used by colleagues, and their mail matches it. Unlike the
    /// mailbox it currently sits in, this travels with the message when it is
    /// archived or moved. Defaults to false for rows cached before it existed,
    /// which reads as "unknown" and leaves the address match in charge.
    #[serde(default)]
    pub delivered: bool,
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
    /// The calendar event a `text/calendar` part carries, when the message is
    /// an invitation (or an update, cancellation or reply to one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite: Option<crate::calendar::Invite>,
    /// OpenPGP facts (encrypted, signed, problems), when there are any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crypto: Option<crate::openpgp::mime::CryptoStatus>,
    /// The composer's sign switch as a saved draft recorded it
    /// (`openpgp::outgoing::DRAFT_SIGN_HEADER`); only read when a draft is
    /// reopened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_sign: Option<bool>,
    /// Keys this message announced or carried, for the store to record.
    /// Never persisted: the store keeps them in its own tables.
    #[serde(skip)]
    pub found_keys: FoundKeys,
}

/// Keys found while parsing (Autocrypt headers, attached keys).
#[derive(Debug, Default, Clone)]
pub struct FoundKeys {
    pub announced: Vec<crate::openpgp::autocrypt::Announced>,
    pub attached: Vec<(String, pgp::composed::SignedPublicKey)>,
}

impl FoundKeys {
    pub fn is_empty(&self) -> bool {
        self.announced.is_empty() && self.attached.is_empty()
    }
}

impl Message {
    /// The parts the user would call attachments: every one except those its
    /// HTML body already shows inline (`cid:` logos and signature images,
    /// rewritten to `/media/<key>` at parse time). The reader draws the same
    /// line, and the store's v11 migration mirrors it for rows cached before
    /// the list was stored.
    pub fn file_attachments(&self) -> impl Iterator<Item = &Attachment> {
        let html = self.body_html.as_deref().unwrap_or_default();
        self.attachments
            .iter()
            .filter(move |attachment| match &attachment.key {
                Some(key) => !html.contains(&format!("/media/{key}")),
                None => true,
            })
    }

    pub fn has_attachments(&self) -> bool {
        self.file_attachments().next().is_some()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub mime: String,
    pub size: usize,
    /// Relative media key (`account/folder/uid/index.ext`) for the bytes written
    /// to disk and served at `/media/<key>`; null only when no media context was
    /// provided (tests, previews), the write failed, or the file is
    /// `oversized`.
    pub key: Option<String>,
    /// Left off disk on purpose: larger than the whole media cache, so it
    /// could not outlive a prune. Downloading the message again would not
    /// change that, which is what tells it from a key-less attachment whose
    /// bytes were never written.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub oversized: bool,
}

/// Parse a full RFC822 message into a reader-view summary. Inline images are
/// written under `media` (when provided) and referenced by key; passing `None`
/// (tests, previews) skips disk writes and leaves every attachment key null.
pub fn parse_message(raw: &[u8], media: Option<&MediaCtx>) -> Message {
    let message = parse_message_unpruned(raw, media);
    if let Some(ctx) = media {
        prune_media_cache(&ctx.root);
    }
    message
}

/// Batch fetches prune once off the async runtime after parsing their messages.
pub(crate) fn parse_message_unpruned(raw: &[u8], media: Option<&MediaCtx>) -> Message {
    parse_message_with(raw, media, &crate::openpgp::keyring::current())
}

/// [`parse_message_unpruned`] with explicit OpenPGP keys (tests pass their own).
pub(crate) fn parse_message_with(
    raw: &[u8],
    media: Option<&MediaCtx>,
    ring: &crate::openpgp::keyring::Keyring,
) -> Message {
    use crate::openpgp::mime::Content;
    let mail = match parse_mail(raw) {
        Ok(mail) => mail,
        Err(_) => return Message::default(),
    };
    let headers = &mail.headers;
    let subject = headers.get_first_value("Subject").unwrap_or_default();
    let (from_name, from_addr) = from_of(headers);
    // OpenPGP first: it decides which MIME tree may be shown at all.
    let opened = crate::openpgp::mime::open(&mail, &from_addr, ring);
    let inner_mail = match &opened.content {
        Content::Inner { bytes, .. } => parse_mail(bytes).ok(),
        _ => None,
    };
    let content: Option<&ParsedMail> = match &opened.content {
        Content::Outer => Some(&mail),
        Content::SignedPart => mail.subparts.first(),
        Content::Inner { signed_inside, .. } => inner_mail.as_ref().map(|m| {
            if *signed_inside {
                m.subparts.first().unwrap_or(m)
            } else {
                m
            }
        }),
        Content::Nothing => None,
    };
    let encrypted = opened.status.as_ref().is_some_and(|s| s.encrypted);
    let subject = opened.inner_subject.clone().unwrap_or(subject);
    let secure_ctx = media.filter(|_| encrypted).map(|ctx| MediaCtx {
        root: secure_media_root(),
        account: ctx.account.clone(),
        folder: ctx.folder.clone(),
        uid: ctx.uid,
    });
    let media = if encrypted {
        secure_ctx.as_ref()
    } else {
        media
    };
    let to = collect_address_list(&headers.get_all_headers("To"));
    let reply_to = collect_address_list(&headers.get_all_headers("Reply-To"));
    let cc = collect_address_list(&headers.get_all_headers("Cc"));
    let bcc = collect_address_list(&headers.get_all_headers("Bcc"));
    let message_id = normalize_msgid(&headers.get_first_value("Message-ID").unwrap_or_default());
    let references =
        normalize_references(&headers.get_first_value("References").unwrap_or_default());
    let date = parse_date_to_epoch(&headers.get_first_value("Date").unwrap_or_default());
    let delivered = ["Return-Path", "Delivered-To", "X-Original-To"]
        .iter()
        .any(|name| headers.get_first_value(name).is_some());
    let (sources, invite) = match content {
        Some(part) => (body_sources(part), calendar_invite(part)),
        None => (
            BodySources {
                plain: None,
                html: None,
            },
            None,
        ),
    };

    let mut attachments = Vec::new();
    let mut cid_keys: Vec<(String, String)> = Vec::new();
    if let Some(part) = content {
        collect_attachments(part, &mut attachments, &mut cid_keys, media);
    }
    if encrypted {
        if let Some(root) = media.map(|ctx| ctx.root.clone()) {
            restrict_to_owner(&root);
        }
        for att in &mut attachments {
            if let Some(key) = &mut att.key {
                *key = format!("{SECURE_MEDIA_PREFIX}{key}");
            }
        }
        for (_, key) in &mut cid_keys {
            *key = format!("{SECURE_MEDIA_PREFIX}{key}");
        }
    }
    // A verified signature's `signature.asc` is the signature itself, not a file.
    if opened
        .status
        .as_ref()
        .is_some_and(|s| s.signed && s.signer.is_some())
    {
        attachments.retain(|a| !a.mime.eq_ignore_ascii_case("application/pgp-signature"));
    }
    // The sender's own key, attached so we can record it (Thunderbird does this
    // on every signed message): recorded, so not shown as a file.
    if let Some(part) = content {
        let own_keys = crate::openpgp::mime::sender_key_files(part, &from_addr);
        if !own_keys.is_empty() {
            attachments.retain(|a| {
                !(a.mime.eq_ignore_ascii_case("application/pgp-keys")
                    && own_keys.contains(&a.filename))
            });
        }
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
        delivered,
        date,
        body,
        body_html,
        body_is_rendered,
        preview,
        attachments,
        invite,
        crypto: opened.status,
        // An encrypted draft keeps it inside the encryption.
        draft_sign: inner_mail
            .as_ref()
            .and_then(|inner| crate::openpgp::outgoing::draft_sign(&inner.headers))
            .or_else(|| crate::openpgp::outgoing::draft_sign(headers)),
        found_keys: FoundKeys {
            announced: opened.announced,
            attached: opened.attached,
        },
    }
}

/// Decrypted attachments are readable by the user only.
fn restrict_to_owner(root: &Path) {
    let _ = std::fs::create_dir_all(root);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
        if let Some(parent) = root.parent() {
            if parent
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("meron"))
            {
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
    }
}

/// The event of the message's calendar part. iMIP puts it in a `text/calendar`
/// alternative (Outlook: that alone; Google: that plus an `application/ics`
/// copy as a file), so look at every calendar leaf, and take one that names
/// its iTIP method on the Content-Type first — that is the one the sender
/// means to be acted on.
fn calendar_invite(mail: &ParsedMail) -> Option<crate::calendar::Invite> {
    let mut parts = Vec::new();
    collect_calendar_parts(mail, &mut parts);
    parts.sort_by_key(|part| !part.ctype.params.contains_key("method"));
    parts
        .into_iter()
        .filter_map(|part| text_body(part))
        .find_map(|ics| crate::calendar::parse_invite(&ics))
}

fn collect_calendar_parts<'a>(part: &'a ParsedMail<'a>, out: &mut Vec<&'a ParsedMail<'a>>) {
    if !part.subparts.is_empty() {
        for sub in &part.subparts {
            collect_calendar_parts(sub, out);
        }
        return;
    }
    if is_calendar_mime(&part.ctype.mimetype) {
        out.push(part);
    }
}

pub(crate) fn is_calendar_mime(mime: &str) -> bool {
    mime.eq_ignore_ascii_case("text/calendar") || mime.eq_ignore_ascii_case("application/ics")
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
fn collect_address_list(headers: &[&MailHeader]) -> String {
    let mut out: Vec<String> = Vec::new();
    for header in headers {
        let value = header.get_value();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        match addrparse_lenient(header) {
            Some(list) => {
                for entry in list.iter() {
                    push_addr(entry, &mut out);
                }
            }
            None => out.push(trimmed.to_string()),
        }
    }
    out.join(", ")
}

fn push_addr(entry: &MailAddr, out: &mut Vec<String>) {
    match entry {
        MailAddr::Single(info) => {
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
        MailAddr::Group(group) => {
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

/// Shared by BODYSTRUCTURE metadata and the full MIME reader. Text parts need
/// an attachment disposition: a name alone can describe the body or an inline
/// calendar alternative. Named non-text parts and images can be files.
pub(crate) fn is_attachment_part(mime: &str, explicit: bool, named: bool) -> bool {
    explicit || (!mime.starts_with("text/") && (named || mime.starts_with("image/")))
}

/// Walk the MIME tree collecting file and image leaf parts.
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
    let filename = disposition
        .params
        .get("filename")
        .or_else(|| part.ctype.params.get("name"));
    if !is_attachment_part(&mime, is_attachment, filename.is_some()) {
        return;
    }

    let bytes = match part.get_body_raw() {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let size = bytes.len();
    let filename = filename
        .map(|name| decode_words(&format!(" {name}")))
        .unwrap_or_else(|| "attachment".to_string());

    let index = out.len();
    // Persist every attachment (images and files alike) so the bridge can serve
    // it at `/media/<key>` and the user can download it. Without media context
    // (tests, previews) the key stays null and only metadata is kept.
    let oversized = media.is_some() && size as u64 > media_cache_cap();
    let key = media
        .filter(|_| !oversized)
        .and_then(|ctx| write_media(ctx, index, &filename, &mime, &bytes));

    // Map this inline image's Content-ID to its served key so HTML mode can
    // rewrite `cid:` references (`<foo@host>` headers — strip the angle brackets).
    if let Some(key) = &key
        && let Some(cid) = part.headers.get_first_value("Content-ID")
    {
        let cid = cid
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim();
        if !cid.is_empty() {
            cid_keys.push((cid.to_string(), key.clone()));
        }
    }

    out.push(Attachment {
        filename,
        mime,
        size,
        key,
        oversized,
    });
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod document_media_cache_tests;
