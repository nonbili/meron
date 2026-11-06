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

/// Key prefix for attachments of decrypted mail. They are written under
/// [`secure_media_root`] (tmpfs on Linux) instead of the plain cache, the
/// only place decrypted bytes would otherwise sit on disk outside SQLCipher.
/// After a reboot they are gone and the message is fetched and decrypted
/// again, like any pruned attachment.
pub const SECURE_MEDIA_PREFIX: &str = "e2ee/";

/// Where decrypted attachments go. The bridge sets `MERON_SECURE_MEDIA_DIR`
/// (on Linux `$XDG_RUNTIME_DIR/meron…`); the fallback keeps the sidecar
/// usable when run alone.
pub fn secure_media_root() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_SECURE_MEDIA_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("meron/secure-media");
    }
    media_root().with_file_name("secure-media")
}

/// The file behind a media key, wherever its root is.
pub fn media_file(root: &Path, key: &str) -> PathBuf {
    match key.strip_prefix(SECURE_MEDIA_PREFIX) {
        Some(rest) => secure_media_root().join(rest),
        None => root.join(key),
    }
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

/// The envelope of an outgoing message, as much of it as identifies the copy
/// the server files in Sent. Headers only: the body — and the megabytes of
/// attachments behind it — is never parsed.
///
/// It exists because the `Message-ID` we sent is not always the one that comes
/// back (Proton Bridge replaces it with an id of its own,
/// `@protonmail.internalid`), so recognising our own copy has to fall back to
/// what the provider does keep: same sender, same subject, same recipients,
/// sent at about the same moment. The reader pairs optimistic bubbles to server
/// copies by the same rule.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SentEnvelope {
    /// Normalized `Message-ID`, blank when it could not be read.
    pub message_id: String,
    pub subject: String,
    pub from_addr: String,
    /// Bare `To` + `Cc` addresses, lowercased and sorted so comparison does not
    /// depend on header order. `Bcc` is deliberately absent: the sending server
    /// strips it, so the copy that comes back need not carry it.
    pub recipients: Vec<String>,
    /// `Date` as Unix epoch seconds, 0 when unreadable.
    pub date: i64,
}

/// Read the [`SentEnvelope`] of a raw outgoing message.
pub fn sent_envelope_of(raw: &[u8]) -> SentEnvelope {
    let Ok((headers, _)) = mailparse::parse_headers(raw) else {
        return SentEnvelope::default();
    };
    let (_, from_addr) = split_address(&headers.get_first_value("From").unwrap_or_default());
    let mut recipients = bare_addresses(&headers.get_all_values("To"));
    recipients.extend(bare_addresses(&headers.get_all_values("Cc")));
    SentEnvelope {
        message_id: normalize_msgid(&headers.get_first_value("Message-ID").unwrap_or_default()),
        subject: headers.get_first_value("Subject").unwrap_or_default(),
        from_addr,
        recipients: normalize_addresses(recipients),
        date: parse_date_to_epoch(&headers.get_first_value("Date").unwrap_or_default()),
    }
}

/// Lowercase, de-duplicate and sort a set of addresses so two spellings of the
/// same recipient list compare equal.
pub fn normalize_addresses(addrs: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = addrs
        .into_iter()
        .map(|addr| addr.trim().to_lowercase())
        .filter(|addr| !addr.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The bare addresses of one address-header's values, groups expanded.
fn bare_addresses(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        let Ok(list) = addrparse(value) else { continue };
        for addr in list.iter() {
            match addr {
                mailparse::MailAddr::Single(info) => out.push(info.addr.clone()),
                mailparse::MailAddr::Group(group) => {
                    out.extend(group.addrs.iter().map(|info| info.addr.clone()))
                }
            }
        }
    }
    out
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

/// Decode a bare RFC 2047 header fragment — one that arrives without its field
/// name, so `MailHeaderMap` cannot be used — by letting
/// `mailparse` parse a synthetic header line — its `get_value()` decodes
/// encoded-words and charsets for us.
pub fn decode_words(raw: &str) -> String {
    let line = format!("Subject:{raw}");
    match parse_header(line.as_bytes()) {
        Ok((header, _)) => header.get_value(),
        Err(_) => raw.to_string(),
    }
}

/// Parse an address header like `mailparse::addrparse_header`, but also decode
/// encoded-words `mailparse` leaves verbatim: ones glued to surrounding text
/// (`Acme=?UTF-8?B?...?=`), which RFC 2047 forbids but some senders emit, and
/// ones after non-ASCII text on the same line, where `mailparse` misreads the
/// preceding character. Display and group names holding an encoded-word are
/// decoded here from the raw value and handed back to `mailparse` as one
/// well-formed encoded-word on a line of its own, so it decodes them once and
/// keeps the decoded text opaque to address syntax.
pub fn addrparse_lenient(header: &MailHeader) -> Option<MailAddrList> {
    let raw = header.get_value_raw();
    let raw = match std::str::from_utf8(raw) {
        Ok(text) => text.to_string(),
        Err(_) => raw.iter().map(|&b| b as char).collect(),
    };
    let rewritten = reencode_names(&raw);
    if rewritten == raw {
        return addrparse_header(header).ok();
    }
    let line = format!("{}: {rewritten}", header.get_key_ref());
    let (rewritten_header, _) = parse_header(line.as_bytes()).ok()?;
    addrparse_header(&rewritten_header).ok()
}

/// Replace each display or group name in a raw address list that holds an
/// encoded-word with its decoded text, re-encoded as a single UTF-8 encoded-word
/// on a folded line of its own. Names with quoted strings or comments, and all
/// addresses, are left as they are.
fn reencode_names(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut segment_start = 0;
    let mut quoted = false;
    let mut comment_depth = 0usize;
    let mut in_angle = false;
    let mut escaped = false;
    let mut skip_to = 0;
    for (i, c) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if i < skip_to {
            continue;
        }
        let top_level = !quoted && comment_depth == 0 && !in_angle;
        if top_level && let Some(len) = encoded_word_at(raw, i) {
            // Punctuation inside an encoded-word is text, not address syntax.
            skip_to = i + len;
            continue;
        }
        match c {
            '\\' if quoted || comment_depth > 0 => escaped = true,
            '"' if comment_depth == 0 && !in_angle => quoted = !quoted,
            _ if quoted => {}
            '(' if !in_angle => comment_depth += 1,
            ')' if comment_depth > 0 => comment_depth -= 1,
            _ if comment_depth > 0 => {}
            '>' if in_angle => {
                in_angle = false;
                out.push_str(&raw[segment_start..=i]);
                segment_start = i + 1;
            }
            _ if in_angle => {}
            '<' | ':' => {
                if c == '<' {
                    in_angle = true;
                }
                out.push_str(&reencode_name(&raw[segment_start..i]));
                out.push(c);
                segment_start = i + 1;
            }
            ',' | ';' => {
                out.push_str(&raw[segment_start..=i]);
                segment_start = i + 1;
            }
            _ => {}
        }
    }
    out.push_str(&raw[segment_start..]);
    out
}

fn reencode_name(name: &str) -> String {
    let flat = flatten_name(name);
    let has_word = flat
        .match_indices("=?")
        .any(|(i, _)| encoded_word_len(&flat[i..]).is_some());
    if !has_word {
        return name.to_string();
    }
    let decoded = decode_words_lenient(flat.trim());
    if decoded.is_empty() {
        return name.to_string();
    }
    format!("\r\n =?UTF-8?B?{}?= ", STANDARD.encode(decoded))
}

/// A raw display name as `mailparse` reads it before decoding: folds joined,
/// comments dropped (the whitespace around them kept), quoted strings unwrapped
/// with their escapes resolved. Encoded-words are copied through whole, so
/// punctuation inside one stays text.
fn flatten_name(name: &str) -> String {
    let unfolded = name
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = String::with_capacity(unfolded.len());
    let mut quoted = false;
    let mut comment_depth = 0usize;
    let mut chars = unfolded.char_indices();
    while let Some((i, c)) = chars.next() {
        if comment_depth == 0
            && let Some(len) = encoded_word_at(&unfolded, i)
        {
            out.push_str(&unfolded[i..i + len]);
            while chars.offset() < i + len {
                chars.next();
            }
            continue;
        }
        match c {
            '\\' if quoted || comment_depth > 0 => {
                if let Some((_, next)) = chars.next()
                    && comment_depth == 0
                {
                    out.push(next);
                }
            }
            '"' if comment_depth == 0 => quoted = !quoted,
            '(' if !quoted => comment_depth += 1,
            ')' if comment_depth > 0 => comment_depth -= 1,
            _ if comment_depth > 0 => {}
            _ => out.push(c),
        }
    }
    out
}

/// Byte length of an encoded-word starting at byte `i` of `text`, if one does.
fn encoded_word_at(text: &str, i: usize) -> Option<usize> {
    text[i..]
        .starts_with("=?")
        .then(|| encoded_word_len(&text[i..]))
        .flatten()
}

/// Decode every well-formed encoded-word in `text`, wherever it sits, dropping
/// whitespace between adjacent encoded-words as RFC 2047 requires.
fn decode_words_lenient(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    let mut after_word = false;
    while let Some(start) = rest.find("=?") {
        let decoded = encoded_word_len(&rest[start..]).and_then(|len| {
            let word = &rest[start..start + len];
            let decoded = decode_words(&format!(" {word}"));
            (decoded != word).then_some((len, decoded))
        });
        let Some((len, decoded)) = decoded else {
            out.push_str(&rest[..start + 2]);
            rest = &rest[start + 2..];
            after_word = false;
            continue;
        };
        let gap = &rest[..start];
        if !(after_word && gap.trim().is_empty()) {
            out.push_str(gap);
        }
        out.push_str(&decoded);
        rest = &rest[start + len..];
        after_word = true;
    }
    out.push_str(rest);
    out
}

/// Byte length of the `=?charset?enc?text?=` encoded-word at the start of `s`.
fn encoded_word_len(s: &str) -> Option<usize> {
    let mut parts = s[2..].splitn(3, '?');
    let charset = parts.next()?;
    let enc = parts.next()?;
    let tail = parts.next()?;
    if charset.is_empty() || !matches!(enc, "B" | "b" | "Q" | "q") {
        return None;
    }
    let end = tail.find("?=")?;
    if tail[..end].contains(char::is_whitespace) {
        return None;
    }
    Some(2 + charset.len() + 1 + enc.len() + 1 + end + 2)
}

/// Split a `From`-style value ("Display Name <addr@host>") into name and address.
pub fn split_address(raw: &str) -> (String, String) {
    if let Ok(list) = addrparse(raw)
        && let Some(MailAddr::Single(info)) = list.first()
    {
        let name = info.display_name.clone().unwrap_or_default();
        return (name, info.addr.clone());
    }
    (String::new(), raw.to_string())
}

/// The first `From` address of `headers` as (name, address).
fn from_of(headers: &[MailHeader]) -> (String, String) {
    let Some(header) = headers.get_first_header("From") else {
        return (String::new(), String::new());
    };
    if let Some(list) = addrparse_lenient(header)
        && let Some(MailAddr::Single(info)) = list.first()
    {
        return (
            info.display_name.clone().unwrap_or_default(),
            info.addr.clone(),
        );
    }
    (String::new(), header.get_value())
}

/// The Message-ID of a raw message, headers only — the body (and any megabyte
/// of attachments behind it) is never parsed.
pub fn message_id_of(raw: &[u8]) -> String {
    match mailparse::parse_headers(raw) {
        Ok((headers, _)) => {
            normalize_msgid(&headers.get_first_value("Message-ID").unwrap_or_default())
        }
        Err(_) => String::new(),
    }
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
        Content::Inner { signed_inside, .. } => inner_mail
            .as_ref()
            .map(|m| if *signed_inside { m.subparts.first().unwrap_or(m) } else { m }),
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
    let media = if encrypted { secure_ctx.as_ref() } else { media };
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
        None => (BodySources { plain: None, html: None }, None),
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
    if opened.status.as_ref().is_some_and(|s| s.signed && s.signer.is_some()) {
        attachments.retain(|a| !a.mime.eq_ignore_ascii_case("application/pgp-signature"));
    }
    // The sender's own key, attached so we can record it (Thunderbird does this
    // on every signed message): recorded, so not shown as a file.
    if let Some(part) = content {
        let own_keys = crate::openpgp::mime::sender_key_files(part, &from_addr);
        if !own_keys.is_empty() {
            attachments.retain(|a| !(a.mime.eq_ignore_ascii_case("application/pgp-keys") && own_keys.contains(&a.filename)));
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
        found_keys: FoundKeys { announced: opened.announced, attached: opened.attached },
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
            if parent.file_name().is_some_and(|n| n.to_string_lossy().starts_with("meron")) {
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
        // Ammonia sanitises a *fragment*: `<head>` and its children are dropped
        // as tags, but a dropped tag's text survives. `<title>` is the one head
        // element with text, so without this the subject line reappears as a
        // stray paragraph above the message body.
        .add_clean_content_tags(["title"])
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
        // Apple Mail and Thunderbird mark a reply's quote as
        // `<blockquote type="cite">`; quote folding looks for it (see quote.rs).
        .add_tag_attributes("blockquote", ["type"])
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
            // `meron-*` ids and classes are the frames' own hooks: the injected
            // reader stylesheet, the code-block wrappers, the search marks. A
            // sender that claims one would be read back as ours.
            if matches!(attribute, "id" | "class") && value.to_ascii_lowercase().contains("meron-")
            {
                return None;
            }
            // A link without a scheme resolves against the app's own origin, so
            // following it would load an app page or a cached attachment (a
            // sender's `.html`, or a `cid:` link rewritten to `/media/<key>`)
            // inside the frame with the app's privileges. Only in-page
            // `#fragment` links stay.
            if attribute == "href" && !is_fragment_or_absolute_url(value) {
                return None;
            }
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

/// True for an in-page `#fragment` or a URL that names its own scheme, read the
/// way a browser does: surrounding whitespace and controls trimmed, tabs and
/// newlines removed. Everything else (`/media/…`, `//host/…`, `x.html`) is
/// relative to the document and resolves against the app's origin.
fn is_fragment_or_absolute_url(value: &str) -> bool {
    let value: String = value
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    if value.starts_with('#') {
        return true;
    }
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// What a message declares on its own `<body>` for the whole page.
#[derive(Default)]
struct BodyCanvas {
    background: Option<String>,
    text: Option<String>,
    /// The properties the declaration marked `!important`, named as the frames
    /// restore them: "background-color", "color".
    important: Vec<&'static str>,
}

/// One canvas declaration read off a style attribute, with its importance.
struct CanvasDeclaration {
    value: String,
    important: bool,
}

/// The canvas colors the email declares on its own `<body>`.
///
/// Ammonia sanitises a fragment, so the `<body>` tag (and with it the page colors
/// most HTML mail sets there) never survives into the rendered document. The
/// frames need them: a declared background marks the message as carrying its own
/// design, which is what keeps a dark theme from repainting it, and the text
/// color has to travel with it — restoring a black canvas without the white text
/// that went on it is worse than restoring neither (see `frameTheme.ts`). Both
/// are echoed into the head as `<meta>`s rather than re-applied here, so the
/// frontend stays the single place that decides.
fn declared_body_colors(source: &str) -> BodyCanvas {
    let Some(tag) = body_start_tag(source) else {
        return BodyCanvas::default();
    };
    let style = attribute_value(tag, "style").unwrap_or_default();
    let (css_background, css_text) = css_canvas_colors(&style);
    // Importance travels with the color: the frames restore these as the inline
    // declarations they were, and an important one outranks a sender rule that a
    // normal one loses to.
    let mut important = Vec::new();
    if css_background.as_ref().is_some_and(|held| held.important) {
        important.push("background-color");
    }
    if css_text.as_ref().is_some_and(|held| held.important) {
        important.push("color");
    }

    // A style declaration outranks the presentational attribute, as it does in
    // the cascade — `bgcolor` is only the fallback for mail that predates CSS.
    // A declaration that isn't a color (an image layer, say) still wins, and
    // then drops out here rather than falling back to the attribute.
    let background = match css_background.as_ref().map(|held| held.value.as_str()) {
        Some(value) => safe_css_color(value),
        None => attribute_value(tag, "bgcolor")
            .as_deref()
            .and_then(safe_css_color),
    };
    let text = match css_text.as_ref().map(|held| held.value.as_str()) {
        Some(value) => safe_css_color(value),
        None => attribute_value(tag, "text")
            .as_deref()
            .and_then(safe_css_color),
    };

    BodyCanvas {
        background,
        text,
        important,
    }
}

/// The raw `<body ...>` start tag, skipping any that only appears inside a comment.
fn body_start_tag(source: &str) -> Option<&str> {
    let lower = source.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("<body") {
        let start = from + rel;
        from = start + 5;
        // `<bodyfoo` is a different element; the tag name must end here.
        let after = lower.as_bytes().get(start + 5)?;
        if !matches!(after, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/') {
            continue;
        }
        // A `<body>` written inside a comment isn't the document's body. Every
        // comment that opens before it and hasn't closed yet swallows it.
        let commented = lower[..start]
            .rfind("<!--")
            .is_some_and(|open| !lower[open..start].contains("-->"));
        if commented {
            continue;
        }
        let end = start + lower[start..].find('>')?;
        return Some(&source[start..end]);
    }
    None
}

/// Accept a value only if it is an actual CSS color, and echo it back trimmed.
///
/// The value lands in a quoted attribute the frontend interpolates into CSS, so
/// the grammar is the boundary: a hex triplet, an `rgb()`/`hsl()` function, or a
/// bare keyword. Anything else — a `url()`, a gradient, a second declaration —
/// is dropped rather than escaped.
fn safe_css_color(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 {
        return None;
    }

    let hex = value.strip_prefix('#');
    let is_hex = hex.is_some_and(|digits| {
        matches!(digits.len(), 3 | 4 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
    });
    // A keyword: `white`, `transparent`, `currentcolor`. No digits, so it can't
    // be confused with a length or a bare hex value.
    let is_keyword = value.chars().all(|c| c.is_ascii_alphabetic());
    // `rgb(0,0,0) url(probe.png)` must not slip through on a prefix/suffix check:
    // the arguments are matched whole, and may only be numbers and separators.
    let lower = value.to_ascii_lowercase();
    let is_function = ["rgb", "rgba", "hsl", "hsla"].iter().any(|name| {
        lower
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('('))
            .and_then(|rest| rest.strip_suffix(')'))
            .is_some_and(|args| {
                // Letters are the angle units a hue may carry (`180deg`,
                // `0.5turn`); parentheses stay out, which is what keeps a
                // trailing `url(...)` layer from passing as a color.
                !args.is_empty()
                    && args.chars().all(|c| {
                        c.is_ascii_alphanumeric()
                            || matches!(c, ',' | '.' | '%' | ' ' | '/' | '+' | '-')
                    })
            })
    });

    (is_hex || is_keyword || is_function).then(|| value.to_string())
}

/// Read `name="value"` (or `name=value`) out of a raw start tag, case-insensitively.
fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(name) {
        let at = from + rel;
        from = at + name.len();
        // Must be a whole attribute name: preceded by whitespace, followed by `=`.
        let before_ok = tag[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace);
        let rest = lower[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let value = tag[tag.len() - rest.len() + 1..].trim_start();
        let (quote, value) = match value.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &value[1..]),
            _ => (None, value),
        };
        let end = match quote {
            Some(q) => value.find(q)?,
            None => value.find(char::is_whitespace).unwrap_or(value.len()),
        };
        return Some(value[..end].to_string());
    }
    None
}

/// The canvas colors a style attribute ends up with: `(background, text)`.
///
/// Declarations are read in source order and the winner is chosen the way the
/// cascade chooses it: an `!important` declaration beats a normal one, and among
/// equals the last wins — including the `background` shorthand, which resets the
/// color even when it names an image, so a later `background: url(...)` really
/// does drop an earlier `background-color`.
fn css_canvas_colors(style: &str) -> (Option<CanvasDeclaration>, Option<CanvasDeclaration>) {
    let mut background: Option<CanvasDeclaration> = None;
    let mut text: Option<CanvasDeclaration> = None;

    // Comments can sit anywhere, including between `!` and `important`, and can
    // hold whole declarations of their own.
    let style = strip_css_comments(style);
    for decl in style.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        // `!important` is part of the declaration, not of the color. CSS allows
        // whitespace after the `!`, which minifiers and hand-written mail both use.
        let (value, important) = split_important(value.trim());
        let slot = match name.trim().to_ascii_lowercase().as_str() {
            "background-color" | "background" => &mut background,
            "color" => &mut text,
            _ => continue,
        };
        // A normal declaration can't displace an important one; anything else
        // that comes later can.
        if slot
            .as_ref()
            .is_some_and(|held| held.important && !important)
        {
            continue;
        }
        *slot = Some(CanvasDeclaration {
            value: value.to_string(),
            important,
        });
    }

    (background, text)
}

/// Drop `/* ... */` comments, which are valid anywhere a space is.
fn strip_css_comments(style: &str) -> String {
    let mut out = String::with_capacity(style.len());
    let mut rest = style;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        out.push(' ');
        match rest[open + 2..].find("*/") {
            Some(close) => rest = &rest[open + 2 + close + 2..],
            // An unterminated comment runs to the end of the declaration block.
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Split a declaration value from its `!important` flag, tolerating `! important`.
fn split_important(value: &str) -> (&str, bool) {
    let Some(bang) = value.rfind('!') else {
        return (value, false);
    };
    let flag = value[bang + 1..].trim_start();
    // `get` rather than a slice: the suffix is untrusted UTF-8, and a fixed byte
    // offset can land inside a code point (`!ééééé`), which would panic.
    let important = flag
        .get(.."important".len())
        .is_some_and(|word| word.eq_ignore_ascii_case("important"))
        && flag["important".len()..].trim().is_empty();
    if important {
        (value[..bang].trim_end(), true)
    } else {
        (value, false)
    }
}

/// Wrap stored HTML for safe iframe rendering: inject a CSP `<meta>` that allows
/// same-origin (`/media`), data, and blob images/media always, and remote (`https:`)
/// images/media only when the account opts in. The reader renders this in a `sandbox`ed
/// iframe with `allow-scripts` (needed so the host's link-click listener fires under
/// WebKitGTK); the `default-src 'none'` CSP still blocks all email JS, so the
/// CSP also gates remote image/media loads.
pub fn prepare_html(source: &str, load_remote_images: bool) -> String {
    prepare_message_html(source, load_remote_images, "")
}

/// [`prepare_html`] for a mail with a subject, which is what tells an Outlook
/// forward from a reply when folding the quote (see quote.rs).
pub fn prepare_message_html(source: &str, load_remote_images: bool, subject: &str) -> String {
    // Defence in depth: strip script-bearing markup *before* the CSP `<meta>` is
    // injected, so a CSP bypass alone can't run the email's JS. CSS is kept (the
    // CSP allows `style-src 'unsafe-inline'`); only script vectors are removed.
    let raw_source = source;
    // Marked after sanitizing, which drops every sender `data-*` attribute, so
    // the quote marker the frames fold on is always ours.
    let source = &crate::quote::mark_html_quote(
        &sanitize_email_html(source),
        crate::quote::is_forward_subject(subject),
    );
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
    // The email's own canvas colors, hoisted off the `<body>` ammonia just
    // dropped. The frames read them back to tell a self-styled message (leave its
    // design alone) from one with no colors of its own (safe to render dark).
    let canvas = declared_body_colors(raw_source);
    let mut body_colors = [
        ("meron-body-bg", canvas.background),
        ("meron-body-fg", canvas.text),
    ]
    .into_iter()
    .filter_map(|(name, color)| {
        color.map(|color| format!("<meta name=\"{name}\" content=\"{color}\">"))
    })
    .collect::<String>();
    if !canvas.important.is_empty() {
        body_colors.push_str(&format!(
            "<meta name=\"meron-body-important\" content=\"{}\">",
            canvas.important.join(" ")
        ));
    }
    let head = format!(
        "<meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
         {body_colors}\
         <style>img,video{{max-width:100%;height:auto}}img{{cursor:zoom-in}}</style>"
    );
    inject_head(source, &head)
}

/// Wrap a sanitized message in the document that carries `head_extra`.
///
/// Always a fresh shell, never a search for the message's own `<html>` or
/// `<head>`: ammonia returns a fragment, so those strings can only occur inside
/// its `<style>` text or an attribute value. Injecting there would park the CSP
/// `<meta>` where the browser does not read it. The desktop frame recognises
/// this exact shell (see `bubbleHtml.ts`), so keep the two in step.
fn inject_head(html: &str, head_extra: &str) -> String {
    format!("<!doctype html><html><head>{head_extra}</head><body>{html}</body></html>")
}

/// Render stored HTML back into display markdown. Re-run by the store when a
/// render-version bump cleared `body` but the HTML source survives. (Plain-text
/// messages have no stored HTML — their `body` is never invalidated.)
pub fn render_body(html: &str) -> String {
    html_to_text(html)
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
    write_media_with_cap(ctx, index, filename, mime, bytes, media_cache_cap())
}

fn write_media_with_cap(
    ctx: &MediaCtx,
    index: usize,
    filename: &str,
    mime: &str,
    bytes: &[u8],
    cap: u64,
) -> Option<String> {
    // An individual file larger than the entire cache cannot survive pruning.
    if bytes.len() as u64 > cap {
        return None;
    }
    let ext = sanitize_segment(
        Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.is_empty())
            // The MIME subtype without its structured-syntax suffix (RFC 6838):
            // `image/svg+xml` is an `.svg`, which the desktop's file server
            // serves as an image; `.svg_xml` would go out as text/plain.
            .or_else(|| {
                mime.rsplit('/')
                    .next()
                    .and_then(|sub| sub.split('+').next())
                    .filter(|e| !e.is_empty())
            })
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
/// SQLite, and a key-less attachment whose bytes were never written — a row
/// cached before attachments were persisted to disk, or a write that failed.
/// An `oversized` attachment is key-less by design and is not one of them.
pub fn cached_media_available(root: &Path, message: &Message) -> bool {
    missing_media_count(root, message) == 0
}

/// How many of a cached message's attachments a download could put back on
/// disk: pruned, or never written. Not the ones left off for their size.
pub fn missing_media_count(root: &Path, message: &Message) -> usize {
    message
        .attachments
        .iter()
        .filter(|att| match att.key.as_deref() {
            Some(key) => !media_file(root, key).is_file(),
            None => !att.oversized,
        })
        .count()
}

/// Keep the shared media cache under its configured cap by removing oldest files
/// first. The cap is bytes and can be overridden with `MERON_MEDIA_CAP`.
fn media_cache_cap() -> u64 {
    std::env::var("MERON_MEDIA_CAP")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(MEDIA_CAP_BYTES)
}

/// A media root for tests that fetch bodies: the fetch prunes it, which walks all
/// of it and deletes from it, so it must never be the shared temp dir itself.
#[cfg(test)]
pub(crate) fn test_media_root() -> PathBuf {
    std::env::temp_dir().join("meron-test-media")
}

pub fn prune_media_cache(root: &Path) {
    let cap = media_cache_cap();
    let mut files = Vec::new();
    let mut total = 0u64;
    collect_media_files(root, &mut files, &mut total, true);
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
    skip_document_sessions: bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        // Reader sessions have no refetchable mailbox source and are reclaimed
        // on close/startup. Ordinary cache eviction must leave them intact.
        if skip_document_sessions && entry.file_name() == "_eml" {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_media_files(&path, out, total, false);
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

/// Best-effort removal of cached attachment/feed media for one account.
pub fn remove_account_media(root: &Path, account: &str) {
    let segment = sanitize_segment(account);
    if segment.is_empty() {
        return;
    }
    let _ = std::fs::remove_dir_all(root.join(segment));
}

/// Best-effort removal of one folder's cached attachment files.
pub fn remove_folder_media(root: &Path, account: &str, folder: &str) {
    let (account, folder) = (sanitize_segment(account), sanitize_segment(folder));
    if account.is_empty() || folder.is_empty() {
        return;
    }
    let _ = std::fs::remove_dir_all(root.join(account).join(folder));
}

/// Best-effort removal of one message's cached attachment files.
pub fn remove_message_media(root: &Path, account: &str, folder: &str, uid: u32) {
    let (account, folder) = (sanitize_segment(account), sanitize_segment(folder));
    if account.is_empty() || folder.is_empty() {
        return;
    }
    let _ = std::fs::remove_dir_all(root.join(account).join(folder).join(uid.to_string()));
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
    if markdown.trim().is_empty()
        && let Some(body) = body_inner_html(&clean_html)
    {
        markdown = convert(&format!("<div>{body}</div>"), Some(options))
            .ok()
            .and_then(|result| result.content)
            .unwrap_or_default();
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
    let mut plain = find_text_part(part, "text/plain");
    let html = find_text_part(part, "text/html");
    // Some senders put their HTML into the text/plain alternative as well.
    // Shown as text it is a wall of tags (and a notification snippet reading
    // `<div style=...`), so fall back to rendering the real HTML part. Only
    // then: that part is what the HTML tab shows, so nothing is lost, while a
    // plain-only message is always kept as the sender wrote it.
    if html.is_some() && plain.as_deref().is_some_and(plain_is_markup) {
        plain = None;
    }
    if plain.is_some() || html.is_some() {
        return BodySources { plain, html };
    }

    let ctype = part.ctype.mimetype.to_ascii_lowercase();
    if part.subparts.is_empty() && ctype.starts_with("text/") {
        let content = text_body(part).unwrap_or_default();
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

/// Whether a text/plain part is really an HTML document: it opens and closes
/// on a tag, and tags make up most of it. Prose that quotes tags as examples
/// (`<p>hello</p>` then an explanation) is mostly text and stays plain.
fn plain_is_markup(plain: &str) -> bool {
    let trimmed = plain.trim();
    if !trimmed.starts_with('<') || !trimmed.ends_with('>') {
        return false;
    }
    let (mut markup, mut total) = (0usize, 0usize);
    let mut in_tag = false;
    for c in trimmed.chars() {
        if c == '<' {
            in_tag = true;
        }
        if !c.is_whitespace() {
            total += 1;
            if in_tag {
                markup += 1;
            }
        }
        if c == '>' {
            in_tag = false;
        }
    }
    markup * 2 >= total
}

fn find_text_part(part: &ParsedMail, mime: &str) -> Option<String> {
    if part.ctype.mimetype.eq_ignore_ascii_case(mime) {
        return text_body(part);
    }
    for sub in &part.subparts {
        if let Some(body) = find_text_part(sub, mime) {
            return Some(body);
        }
    }
    None
}

/// Decoded text of a leaf part. A base64 body cut off mid-quantum (a truncated
/// message, a sender that drops the padding) fails to decode as a whole, which
/// would leave the reader with no body at all; decode the complete quanta
/// instead and show what did arrive.
fn text_body(part: &ParsedMail) -> Option<String> {
    if let Ok(body) = part.get_body() {
        return Some(body);
    }
    let Body::Base64(encoded) = part.get_body_encoded() else {
        return None;
    };
    let mut cleaned: Vec<u8> = encoded
        .get_raw()
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    cleaned.truncate(cleaned.len() / 4 * 4);
    // Nothing complete arrived: leave the part out so an HTML alternative can
    // stand in for it.
    if cleaned.is_empty() {
        return None;
    }
    // Re-parse as a part of its own so `mailparse` applies the charset.
    let mut raw = format!(
        "Content-Type: {}; charset={}\r\nContent-Transfer-Encoding: base64\r\n\r\n",
        part.ctype.mimetype, part.ctype.charset
    )
    .into_bytes();
    raw.extend_from_slice(&cleaned);
    parse_mail(&raw).ok()?.get_body().ok()
}

/// Drop the markup that makes a body readable in the reader but noisy in a
/// one-line snippet: image markers and their links, link syntax (the text is
/// worth showing, the URL isn't), emphasis/code markers, and line-leading
/// quote/list/heading markers. Applies to both bodies we converted from HTML
/// and text/plain parts, which carry the same conventions (Gmail writes
/// `[image: alt] <url>`).
fn strip_preview_markup(body: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in body.lines() {
        let mut rest = line.trim();
        loop {
            let stripped = rest
                .strip_prefix("> ")
                .or_else(|| rest.strip_prefix('>'))
                .or_else(|| rest.strip_prefix("- "))
                .or_else(|| rest.strip_prefix("* "))
                .or_else(|| rest.strip_prefix("+ "))
                .or_else(|| {
                    rest.starts_with('#')
                        .then(|| rest.trim_start_matches('#').trim_start())
                });
            match stripped {
                Some(next) if next != rest => rest = next.trim_start(),
                _ => break,
            }
        }
        // Horizontal rules and other separator runs carry nothing in a snippet.
        if !rest.is_empty() && rest.chars().all(|c| matches!(c, '-' | '*' | '_' | '=')) {
            continue;
        }
        let stripped = strip_inline_markup(rest);
        if !stripped.trim().is_empty() {
            lines.push(stripped);
        }
    }
    lines.join(" ")
}

/// Inline half of [`strip_preview_markup`], run per line.
fn strip_inline_markup(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find(['[', '*', '`', '<']) {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        // A backslash-escaped delimiter is literal text, not markup.
        if is_escaped(&rest[..open]) {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
            continue;
        }
        match tail.as_bytes()[0] {
            b'[' => {
                let Some(close) = tail.find(']') else {
                    out.push('[');
                    rest = &tail[1..];
                    continue;
                };
                let label = &tail[1..close];
                let after = &tail[close + 1..];
                // `[text](url)` / `[text]<url>` keeps the text and drops the
                // destination; a bare `[text]` keeps the brackets (it may be a
                // literal like `[Attachment: x]`).
                let (after, linked) = match strip_link_target(after) {
                    Some(next) => (next, true),
                    None => (after, false),
                };
                if let Some(alt) = label.strip_prefix("image: ") {
                    // The marker stands in for a picture; in a snippet even the
                    // alt text is usually chrome ("Company Logo"), so drop it.
                    let _ = alt;
                } else if linked {
                    out.push_str(label);
                } else {
                    out.push('[');
                    out.push_str(label);
                    out.push(']');
                }
                rest = after;
            }
            b'<' => {
                // Autolink: keep the URL, drop the brackets. Anything else stays
                // (`<escaped>` text, `a < b`).
                match tail.find('>') {
                    Some(close) if is_autolink(&tail[1..close]) => {
                        out.push_str(&tail[1..close]);
                        rest = &tail[close + 1..];
                    }
                    _ => {
                        out.push('<');
                        rest = &tail[1..];
                    }
                }
            }
            // Emphasis (`*bold*`, `**x**`) and code spans: strip the delimiters
            // only when they actually pair up. Bodies here are often text/plain,
            // where a lone `*` is literal (`2 * 3`, `*.rs`) and must survive.
            marker => {
                let run = tail.bytes().take_while(|byte| *byte == marker).count();
                let inner = &tail[run..];
                match closing_run(inner, marker, run) {
                    Some(close) if marker == b'`' => {
                        // Code spans are literal: keep the content untouched.
                        out.push_str(&inner[..close]);
                        rest = &inner[close + run..];
                    }
                    Some(close) => {
                        out.push_str(&strip_inline_markup(&inner[..close]));
                        rest = &inner[close + run..];
                    }
                    None => {
                        out.push_str(&tail[..run]);
                        rest = inner;
                    }
                }
            }
        }
    }
    out.push_str(rest);
    out
}

/// Whether the delimiter right after `before` is backslash-escaped, i.e. an odd
/// number of backslashes precedes it (`\\*` is a literal backslash then markup).
fn is_escaped(before: &str) -> bool {
    before
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

/// Byte offset of the delimiter run that closes an emphasis/code span opened by
/// `run` copies of `marker`, or `None` when the opener is unpaired. Follows the
/// CommonMark shape loosely: the span may not start or end on whitespace, and
/// the closing run must be the same length (so `**bold**` doesn't close on the
/// first `*`).
fn closing_run(inner: &str, marker: u8, run: usize) -> Option<usize> {
    let bytes = inner.as_bytes();
    if bytes.first().is_none_or(|byte| byte.is_ascii_whitespace()) {
        return None;
    }
    let mut idx = 1;
    while idx < bytes.len() {
        if bytes[idx] != marker {
            idx += 1;
            continue;
        }
        let len = bytes[idx..]
            .iter()
            .take_while(|byte| **byte == marker)
            .count();
        if len == run && !bytes[idx - 1].is_ascii_whitespace() && !is_escaped(&inner[..idx]) {
            return Some(idx);
        }
        idx += len;
    }
    None
}

/// Consume a link destination that follows a `]`, i.e. `(url)`, `<url>`, or
/// ` <url>`. Returns the remainder when one was there.
fn strip_link_target(after: &str) -> Option<&str> {
    if let Some(paren) = after.strip_prefix('(') {
        // Destinations may nest parens (`.../Foo_(bar)`) or escape them, so match
        // the closer by depth rather than taking the first `)`.
        let bytes = paren.as_bytes();
        let mut depth = 1usize;
        let mut idx = 0;
        while idx < bytes.len() {
            match bytes[idx] {
                b'\\' => idx += 1,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&paren[idx + 1..]);
                    }
                }
                _ => {}
            }
            idx += 1;
        }
        return None;
    }
    let angle = after
        .strip_prefix(" <")
        .or_else(|| after.strip_prefix('<'))?;
    let end = angle.find('>')?;
    is_autolink(&angle[..end]).then(|| &angle[end + 1..])
}

fn is_autolink(inner: &str) -> bool {
    !inner.contains(char::is_whitespace)
        && (inner.contains("://") || inner.starts_with("mailto:") || inner.contains('@'))
}

pub(crate) fn preview_of(body: &str) -> String {
    let stripped = strip_preview_markup(body);
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
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
    fn message_id_reads_the_header_without_the_body() {
        // What waiting for a Sent copy matches on: the angle brackets come off,
        // a folded header still reads, and a message without the header (or one
        // that is not a message at all) yields nothing rather than a guess.
        let raw = b"From: a@example.com\r\nMessage-ID:\r\n <reply-1@meron>\r\nSubject: Re: Lunch\r\n\r\nbody\r\n";
        assert_eq!(message_id_of(raw), "reply-1@meron");
        assert_eq!(message_id_of(b"From: a@example.com\r\n\r\nbody\r\n"), "");
        assert_eq!(message_id_of(b""), "");
    }

    #[test]
    fn preview_drops_markup_noise() {
        // Gmail's text/plain conventions, as sent by the Play Console.
        let body = "Your update is live\n\n[image: Google Play Console Logo] <https://play.google.com/console/>\n\nHello, Your update to Meron is live in the store.\n\n[image: Icon of your app Meron] *Meron*";
        assert_eq!(
            preview_of(body),
            "Your update is live Hello, Your update to Meron is live in the store. Meron"
        );
    }

    #[test]
    fn preview_keeps_link_text_and_plain_punctuation() {
        let body = "# Heading\n\n---\n\n> quoted\n- See [the docs](https://example.com) and `code`.\nWrite to <bob@example.com>. 3 < 4 and [Attachment: a.pdf]";
        assert_eq!(
            preview_of(body),
            "Heading quoted See the docs and code. Write to bob@example.com. 3 < 4 and [Attachment: a.pdf]"
        );
    }

    #[test]
    fn preview_keeps_unpaired_emphasis_and_nested_link_parens() {
        let body = "2 * 3 and *.rs stay; a lone ` too.\nSee [docs](https://example.com/Foo_(bar)) and *this* **too**.";
        assert_eq!(
            preview_of(body),
            "2 * 3 and *.rs stay; a lone ` too. See docs and this too."
        );
    }

    #[test]
    fn preview_leaves_escaped_delimiters_alone() {
        let body = "Use \\*literal\\* and \\`ticks\\`, but \\\\*this* is markup.";
        assert_eq!(
            preview_of(body),
            "Use \\*literal\\* and \\`ticks\\`, but \\\\this is markup."
        );
    }

    #[test]
    fn preview_truncates_long_bodies() {
        let body = "word ".repeat(100);
        let preview = preview_of(&body);
        assert_eq!(preview.chars().count(), 201);
        assert!(preview.ends_with('…'));
    }

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
    fn delivery_headers_mark_a_message_as_received() {
        // Written here (or filed to Sent by the server): no delivery headers.
        let sent = b"From: me@example.com\r\nSubject: x\r\n\r\nhi";
        assert!(!parse_message(sent, None).delivered);
        // The copy delivered to us carries the receiving MTA's headers, and
        // keeps them wherever it is filed afterwards.
        let received =
            b"Return-Path: <them@example.com>\r\nFrom: them@example.com\r\nSubject: x\r\n\r\nhi";
        assert!(parse_message(received, None).delivered);
        let routed =
            b"Delivered-To: me@example.com\r\nFrom: me@example.com\r\nSubject: x\r\n\r\nhi";
        assert!(parse_message(routed, None).delivered);
    }

    #[test]
    fn truncated_base64_body_keeps_what_decodes() {
        // "hello world" is `aGVsbG8gd29ybGQ=`; cut mid-quantum it no longer
        // decodes as a whole.
        let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8gd29y\r\nbGQ\r\n--b--\r\n";
        let msg = parse_message(raw, None);
        assert_eq!(msg.body, "hello wor");
    }

    #[test]
    fn undecodable_base64_plain_part_falls_back_to_html() {
        let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: multipart/alternative; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\naG\r\n--b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>hello world</p>\r\n--b--\r\n";
        let msg = parse_message(raw, None);
        assert!(msg.body_is_rendered);
        assert_eq!(msg.body, "hello world");
    }

    #[test]
    fn plain_text_keeps_no_html_source() {
        let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\nhello world";
        let msg = parse_message(raw, None);
        assert!(msg.body_html.is_none());
        assert_eq!(msg.body, "hello world");
    }

    #[test]
    fn markup_in_the_plain_part_renders_as_html() {
        // A sender that stuffs its HTML into text/plain: the real HTML part
        // renders instead, so neither the reader nor the snippet shows tags.
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
<div style=\"font-family: Arial\"><p>plain markup</p></div>\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p><strong>html part</strong></p>\r\n\
--sep--\r\n";
        let msg = parse_message(raw, None);
        assert_eq!(msg.body, "**html part**");
        assert_eq!(msg.preview, "html part");

        // Prose that opens on a tag example keeps its plain part, tags and all.
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/alternative; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
<p>hello</p>\r\n\
\r\n\
Please replace <p> with <div> in this example.\r\n\
--sep\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n\
<p>&lt;p&gt;hello&lt;/p&gt;</p>\r\n\
--sep--\r\n";
        let msg = parse_message(raw, None);
        assert!(msg.body.starts_with("<p>hello</p>"));
        assert!(msg.body.contains("replace <p> with <div>"));

        // A plain-only message is never reclassified, even when it is all tags.
        let raw = b"From: a@b.com\r\nSubject: x\r\nContent-Type: text/plain\r\n\r\n\
<div style=\"font-family: Arial\"><p>hello</p></div>";
        let msg = parse_message(raw, None);
        assert!(msg.body_html.is_none());
        assert!(msg.body.starts_with("<div style="));
    }

    #[test]
    fn plain_markup_needs_tags_to_dominate() {
        assert!(plain_is_markup(
            "<div style=\"font-family: Arial\">\n<p style=\"color: #1a2b3c\">Connect</p>\n</div>"
        ));
        // Tag examples in prose, even when the message ends on one.
        assert!(!plain_is_markup(
            "<p>hello</p>\n\nPlease replace <p> with <div> in this example."
        ));
        assert!(!plain_is_markup(
            "<p>hello</p>\n\nPlease replace <p> with <div>"
        ));
        assert!(!plain_is_markup("hello <b>world</b>"));
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

    fn lenient_from(raw: &str) -> (String, String) {
        let line = format!("From: {raw}\r\n\r\n");
        let (headers, _) = mailparse::parse_headers(line.as_bytes()).unwrap();
        from_of(&headers)
    }

    #[test]
    fn decodes_encoded_words_glued_to_text() {
        // "GyRCJUYlOSVIGyhC" is ISO-2022-JP "テスト"; "Q2Fmw6k=" is UTF-8 "Café".
        assert_eq!(
            lenient_from("Acme=?iso-2022-jp?B?GyRCJUYlOSVIGyhC?= <news@example.com>"),
            ("Acmeテスト".to_string(), "news@example.com".to_string())
        );
        // Adjacent encoded-words join without the whitespace between them, even
        // when the first is glued to text.
        assert_eq!(
            lenient_from("Acme=?UTF-8?Q?Foo?= =?UTF-8?Q?Bar?= <a@example.com>").0,
            "AcmeFooBar"
        );
        assert_eq!(
            lenient_from("=?UTF-8?B?Q2Fmw6k=?= =?UTF-8?Q?_Bar?= Inc <a@example.com>").0,
            "Café Bar Inc"
        );
        // A name whose decoded text looks like an encoded-word is not decoded twice.
        // "PT9VVEYtOD9RP0NFTz89" is base64 for "=?UTF-8?Q?CEO?=".
        assert_eq!(
            lenient_from("=?UTF-8?B?PT9VVEYtOD9RP0NFTz89?= <a@example.com>").0,
            "=?UTF-8?Q?CEO?="
        );
        // Folded across lines.
        assert_eq!(
            lenient_from("Acme=?UTF-8?Q?Foo?=\r\n =?UTF-8?Q?Bar?= <a@example.com>").0,
            "AcmeFooBar"
        );
        // Glued on both sides, and two glued words back to back.
        assert_eq!(
            lenient_from("Acme=?UTF-8?Q?Foo?=Inc <a@example.com>").0,
            "AcmeFooInc"
        );
        assert_eq!(
            lenient_from("=?UTF-8?Q?Foo?==?UTF-8?Q?Bar?= <a@example.com>").0,
            "FooBar"
        );
        // Punctuation inside an encoded-word stays text, not address syntax.
        assert_eq!(
            lenient_from("=?UTF-8?Q?Doe,_Jane?= <jane@example.com>"),
            ("Doe, Jane".to_string(), "jane@example.com".to_string())
        );
        assert_eq!(
            lenient_from("Acme=?UTF-8?Q?_(News)?= <a@example.com>").0,
            "Acme (News)"
        );
        assert_eq!(
            lenient_from("=?UTF-8?Q?Acme_(News)?= <a@example.com>").0,
            "Acme (News)"
        );
        // Glued words inside a quoted name, or a name with a comment.
        assert_eq!(
            lenient_from("\"Acme=?UTF-8?Q?Foo?=\" <a@example.com>").0,
            "AcmeFoo"
        );
        assert_eq!(
            lenient_from("\"Acme \\\"Q\\\"=?UTF-8?Q?Foo?=\" <a@example.com>").0,
            "Acme \"Q\"Foo"
        );
        assert_eq!(
            lenient_from("Acme=?UTF-8?Q?Foo?= (News) <a@example.com>").0,
            "AcmeFoo"
        );
        assert_eq!(
            lenient_from("Acme=?UTF-8?Q?Foo?= (Ne\\)ws) Inc <a@example.com>").0,
            "AcmeFoo  Inc"
        );
        // Non-ASCII text before the encoded-word, glued or not.
        assert_eq!(
            lenient_from("Café=?UTF-8?Q?Foo?= <a@example.com>").0,
            "CaféFoo"
        );
        assert_eq!(
            lenient_from("Café =?UTF-8?Q?Foo?= <a@example.com>").0,
            "Café Foo"
        );
        // Encoded-word lookalikes inside an address are left alone.
        assert_eq!(
            lenient_from("User <user=?UTF-8?Q?foo?=@example.com>"),
            (
                "User".to_string(),
                "user=?UTF-8?Q?foo?=@example.com".to_string()
            )
        );
        assert_eq!(lenient_from("a =? b ?= c <a@example.com>").0, "a =? b ?= c");
        assert_eq!(lenient_from("Plain Name <a@example.com>").0, "Plain Name");
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

    // An inline image without a filename (Meron's own composer sends them so)
    // is keyed by its MIME type. `image/svg+xml` must give `.svg`, the type the
    // desktop's file server knows: `.svg_xml` was served as text/plain, and a
    // browser doesn't draw an SVG of that type.
    #[test]
    fn an_inline_svg_without_filename_is_keyed_svg() {
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/related; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p><img src=\"cid:logo@x\"></p>\r\n\
--sep\r\n\
Content-Type: image/svg+xml\r\n\
Content-Disposition: inline\r\n\
Content-ID: <logo@x>\r\n\
\r\n\
<svg xmlns=\"http://www.w3.org/2000/svg\"/>\r\n\
--sep--\r\n";
        let root = std::env::temp_dir().join(format!("meron-svg-key-test-{}", std::process::id()));
        let ctx = MediaCtx {
            root: root.clone(),
            account: "acct".to_string(),
            folder: "INBOX".to_string(),
            uid: 7,
        };
        let msg = parse_message(raw, Some(&ctx));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(msg.attachments.len(), 1);
        assert_eq!(
            msg.attachments[0].key.as_deref(),
            Some("acct/INBOX/7/0.svg")
        );
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
    fn prepare_html_keeps_the_csp_ahead_of_markup_posing_as_a_head() {
        // `<style>` text and attribute values survive sanitizing verbatim.
        for source in [
            "<style>/*<html><head>*/</style><img src=\"https://t.example/p.png\">",
            "<img alt=\"<html><head>\" src=\"https://t.example/p.png\">",
        ] {
            let out = prepare_html(source, false);
            let head = "<!doctype html><html><head><meta charset=\"utf-8\">\
                        <meta http-equiv=\"Content-Security-Policy\"";
            assert!(out.starts_with(head), "{out}");
            assert_eq!(out.matches("Content-Security-Policy").count(), 1, "{out}");
        }
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
    fn prepare_html_marks_only_its_own_quotes() {
        // A sender can't fold part of a message by writing the marker itself.
        let forged = prepare_html(r#"<p data-meron-quote="">hidden?</p><p>tail</p>"#, false);
        assert!(!forged.contains("data-meron-quote"), "{forged}");
        let reply = prepare_html(
            r#"<p>Yes</p><div class="gmail_quote">On Mon, Jane wrote:<blockquote>Lunch?</blockquote></div>"#,
            false,
        );
        assert!(
            reply.contains(r#"class="gmail_quote" data-meron-quote="""#),
            "{reply}"
        );
        // Apple Mail / Thunderbird: the `type="cite"` the detector keys on has
        // to survive sanitizing.
        let cite = prepare_html(
            r#"<p>Yes</p><div class="moz-cite-prefix">On 9/1/25 Jane wrote:</div><blockquote type="cite"><p>Lunch?</p></blockquote>"#,
            false,
        );
        assert!(
            cite.contains(r#"<blockquote type="cite" data-meron-quote="">"#),
            "{cite}"
        );
        assert!(
            cite.contains(r#"class="moz-cite-prefix" data-meron-quote="""#),
            "{cite}"
        );
    }

    #[test]
    fn prepare_html_drops_head_title_text() {
        let out = prepare_html(
            "<html><head><title>Secure your schedule</title></head><body><p>hi</p></body></html>",
            false,
        );
        // The subject must not leak into the body as a stray line of text.
        assert!(!out.contains("Secure your schedule"));
        assert!(out.contains("hi"));
    }

    #[test]
    fn prepare_html_hoists_body_colors() {
        let style = prepare_html(
            "<html><body style=\"margin: 0; background-color: #f5f4f2;\"><p>hi</p></body></html>",
            false,
        );
        assert!(style.contains("<meta name=\"meron-body-bg\" content=\"#f5f4f2\">"));

        // A dark canvas travels with the text color that makes it readable.
        let dark = prepare_html(
            "<html><body bgcolor=\"#000000\" text=\"white\"><p>hi</p></body></html>",
            false,
        );
        assert!(dark.contains("<meta name=\"meron-body-bg\" content=\"#000000\">"));
        assert!(dark.contains("<meta name=\"meron-body-fg\" content=\"white\">"));

        // A style declaration outranks the presentational attribute.
        let both = prepare_html(
            "<body bgcolor=\"#ffffff\" style=\"background: #101010\"><p>hi</p></body>",
            false,
        );
        assert!(both.contains("content=\"#101010\""));

        // A `<body>` that only appears inside a comment isn't the document's.
        let commented = prepare_html(
            "<!-- <body bgcolor=\"#123456\"> --><body bgcolor=\"#abcdef\"><p>hi</p></body>",
            false,
        );
        assert!(commented.contains("content=\"#abcdef\""));
        assert!(!commented.contains("#123456"));

        assert!(!prepare_html("<body><p>hi</p></body>", false).contains("meron-body-"));
    }

    #[test]
    fn prepare_html_hoists_colors_not_arbitrary_css() {
        // Only real colors travel: an image layer would become a background-image
        // (and a network fetch) once the frontend interpolates it into CSS.
        for value in [
            "background: url(probe.png)",
            "background: linear-gradient(#fff, #000)",
            "background-color: a&quot;><script>",
            "background-color: expression(alert(1))",
        ] {
            let out = prepare_html(&format!("<body style=\"{value}\"><p>hi</p></body>"), false);
            assert!(!out.contains("meron-body-bg"), "accepted {value}");
        }

        // A second declaration is just another declaration, not an injection.
        let pair = prepare_html(
            "<body style=\"background-color: #fff; color: red\"><p>hi</p></body>",
            false,
        );
        assert!(pair.contains("<meta name=\"meron-body-bg\" content=\"#fff\">"));
        assert!(pair.contains("<meta name=\"meron-body-fg\" content=\"red\">"));

        // `!important` belongs to the declaration, and decides which one wins.
        let important = prepare_html(
            "<body style=\"background: #000000 !important; background: #ffffff; color: white\"><p>hi</p></body>",
            false,
        );
        assert!(important.contains("<meta name=\"meron-body-bg\" content=\"#000000\">"));
        assert!(important.contains("<meta name=\"meron-body-fg\" content=\"white\">"));

        // CSS allows whitespace after the `!`, and a comment anywhere a space is.
        for style in [
            "background: #000000 ! important; background: #ffffff",
            "background: #000000 !IMPORTANT; background: #ffffff",
            "background: #000000 !/* whoops */important; background: #ffffff",
            // Non-ASCII after the `!` must not land mid-code-point.
            "background: #000000; background: #ffffff !ééééé; background: #000000",
            "background: #ffffff; /* background: #ffffff */ background: #000000",
        ] {
            let out = prepare_html(&format!("<body style=\"{style}\"><p>hi</p></body>"), false);
            assert!(
                out.contains("<meta name=\"meron-body-bg\" content=\"#000000\">"),
                "lost the winning declaration in {style}"
            );
        }

        // A trailing layer is not a color, however the value starts.
        for value in [
            "background: rgb(0,0,0) url(probe.png)",
            "background-color: rgb(0 0 0 / url(x))",
            "background: hsl(0, 0%, 0%) no-repeat",
        ] {
            let out = prepare_html(&format!("<body style=\"{value}\"><p>hi</p></body>"), false);
            assert!(!out.contains("meron-body-bg"), "accepted {value}");
        }

        // The last declaration wins, as it does in the cascade: a shorthand that
        // names an image resets an earlier color rather than losing to it.
        let reset = prepare_html(
            "<body style=\"background-color: #fff; background: url(x.png); color: white\"><p>hi</p></body>",
            false,
        );
        assert!(!reset.contains("meron-body-bg"));
        assert!(reset.contains("<meta name=\"meron-body-fg\" content=\"white\">"));

        // Importance travels with the declaration it was written on.
        let important = prepare_html(
            "<body style=\"background: #000 !important; color: white\"><p>hi</p></body>",
            false,
        );
        assert!(
            important.contains("<meta name=\"meron-body-important\" content=\"background-color\">")
        );
        assert!(
            !prepare_html("<body style=\"color: white\"><p>hi</p></body>", false)
                .contains("meron-body-important")
        );

        // `#fff`, `rgb(...)` and a bare keyword are the accepted forms — including
        // the angle units a hue may carry.
        for value in [
            "#fff",
            "#ffffffaa",
            "rgb(255, 0, 0)",
            "white",
            "hsl(180deg 50% 50%)",
            "hsl(0.5turn 50% 50%)",
            "hsl(-90 50% 50%)",
        ] {
            let out = prepare_html(
                &format!("<body style=\"background-color: {value}\"><p>hi</p></body>"),
                false,
            );
            assert!(
                out.contains(&format!("content=\"{value}\"")),
                "dropped {value}"
            );
        }
    }

    #[test]
    fn prepare_html_marks_a_gmail_reply_with_signatures_above_and_below_the_quote() {
        // Gmail's reply with the signature under the quote, which itself holds
        // the quoted message's own signature.
        let out = prepare_html(
            r#"<div dir="ltr">x1</div><br><div class="gmail_quote gmail_quote_container"><div dir="ltr" class="gmail_attr">On Thu, Sep 10, 2026 at 5:11 PM rnons &lt;<a href="mailto:a@b.c">a@b.c</a>&gt; wrote:<br></div><blockquote class="gmail_quote"><div dir="ltr"><div>t2</div><span class="gmail_signature_prefix">-- </span><br><div dir="ltr" class="gmail_signature">2t</div></div></blockquote></div><div><br clear="all"></div><span class="gmail_signature_prefix">-- </span><br><div dir="ltr" class="gmail_signature"><div dir="ltr">s2</div></div>"#,
            false,
        );
        assert!(
            out.contains(r#"<div class="gmail_quote gmail_quote_container" data-meron-quote="">"#),
            "{out}"
        );
        assert!(
            out.contains(r#"<div class="gmail_signature" data-meron-quote="">"#),
            "{out}"
        );
    }

    #[test]
    fn prepare_html_strips_sender_claimed_meron_hooks() {
        // The frames read their own `meron-*` ids and classes back out of the
        // document; a sender must not be able to plant one.
        let out = prepare_html(
            "<style id=\"meron-reader-style\">body{color:#333}</style>\
             <mark class=\"meron-search-hit\">x</mark>\
             <div class=\"card meron-copy-code\">y</div>",
            false,
        );
        assert!(!out.contains("meron-"));
        // Only the hooks are dropped, not the elements or their other markup.
        assert!(out.contains("<style>"));
        assert!(out.contains("<mark>"));
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
    fn prepare_html_strips_links_that_resolve_against_the_app() {
        let out = prepare_html(
            "<a href=\"/media/acct/inbox/1/0.html\">a</a>\
             <a href=\"//wails/media/x.html\">b</a>\
             <a href=\"0.html\">c</a>\
             <a href=\"\t/wails/runtime\">d</a>\
             <map><area href=\"/media/acct/inbox/1/0.html\"></map>\
             <a href=\"#top\">e</a>\
             <a href=\"https://example.com/x\">f</a>\
             <a href=\"mailto:a@example.com\">g</a>\
             <img src=\"/media/acct/inbox/1/1.png\">",
            false,
        );
        assert!(!out.contains(".html"), "{out}");
        assert!(!out.contains("/wails"), "{out}");
        assert!(out.contains("href=\"#top\""), "{out}");
        assert!(out.contains("href=\"https://example.com/x\""), "{out}");
        assert!(out.contains("href=\"mailto:a@example.com\""), "{out}");
        // Inline images still load from the media cache.
        assert!(out.contains("src=\"/media/acct/inbox/1/1.png\""), "{out}");
    }

    #[test]
    fn a_cid_link_to_an_attachment_does_not_survive_rendering() {
        let raw = b"From: a@b.com\r\n\
Subject: x\r\n\
Content-Type: multipart/related; boundary=sep\r\n\
\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
\r\n\
<p><a href=\"cid:page\">open</a></p>\r\n\
--sep\r\n\
Content-Type: text/html\r\n\
Content-ID: <page>\r\n\
Content-Disposition: attachment; filename=\"page.html\"\r\n\
\r\n\
<script src=\"1.js\"></script>\r\n\
--sep--\r\n";
        let root = std::env::temp_dir().join(format!("meron-cid-link-test-{}", std::process::id()));
        let ctx = MediaCtx {
            root: root.clone(),
            account: "acct".to_string(),
            folder: "inbox".to_string(),
            uid: 7,
        };
        let msg = parse_message(raw, Some(&ctx));
        let _ = std::fs::remove_dir_all(&root);
        let html = msg.body_html.as_deref().expect("html kept");
        // The rewrite still points the link at the cached page...
        assert!(html.contains("href=\"/media/acct/inbox/7/"), "{html}");
        // ...and rendering drops it.
        let out = prepare_html(html, false);
        assert!(out.contains(">open</a>"), "{out}");
        assert!(!out.contains("/media/"), "{out}");
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
            oversized: false,
        });
        assert!(cached_media_available(&root, &msg));

        std::fs::remove_file(root.join("acct/inbox/1/0.png")).unwrap();
        assert!(!cached_media_available(&root, &msg));
        assert_eq!(missing_media_count(&root, &msg), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn has_attachments_ignores_images_the_html_shows_inline() {
        let attachment = |key: &str| Attachment {
            filename: "x".to_string(),
            mime: "image/png".to_string(),
            size: 3,
            key: Some(key.to_string()),
            oversized: false,
        };
        let mut msg = Message {
            body_html: Some("<img src=\"/media/acct/inbox/1/0.png\">".to_string()),
            ..Default::default()
        };
        assert!(!msg.has_attachments());

        msg.attachments.push(attachment("acct/inbox/1/0.png"));
        assert!(!msg.has_attachments(), "a cid: logo is not an attachment");

        msg.attachments.push(attachment("acct/inbox/1/1.pdf"));
        assert!(msg.has_attachments());

        // Plain-text mail has no HTML to show anything inline.
        let mut plain = Message::default();
        plain.attachments.push(attachment("acct/inbox/2/0.png"));
        assert!(plain.has_attachments());
    }

    #[test]
    fn named_calendar_alternative_is_not_a_duplicate_attachment() {
        let raw = "Content-Type: multipart/mixed; boundary=outer

--outer
Content-Type: multipart/alternative; boundary=inner

--inner
Content-Type: text/plain; name=body.txt

Invitation
--inner
Content-Type: text/calendar; name=invite.ics
Content-Disposition: inline; filename=invite.ics

BEGIN:VCALENDAR
END:VCALENDAR
--inner--
--outer
Content-Type: text/calendar; name=invite.ics
Content-Disposition: attachment; filename=invite.ics

BEGIN:VCALENDAR
END:VCALENDAR
--outer--
"
        .replace('\n', "\r\n");
        let message = parse_message(raw.as_bytes(), None);
        let files: Vec<_> = message.file_attachments().collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "invite.ics");
        assert_eq!(files[0].mime, "text/calendar");
    }

    #[test]
    fn a_failed_write_is_recovered_and_an_oversized_file_is_not_written() {
        let root = std::env::temp_dir().join(format!("meron-unwritable-{}", std::process::id()));
        let _ = std::fs::remove_file(&root);
        std::fs::write(&root, b"not a directory").unwrap();
        let ctx = MediaCtx {
            root: root.clone(),
            account: "acc".into(),
            folder: "INBOX".into(),
            uid: 1,
        };
        let raw = b"Message-ID: <one@test>\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=file.bin\r\n\r\ndata";
        let message = parse_message(raw, Some(&ctx));
        assert_eq!(message.attachments.len(), 1);
        assert_eq!(message.attachments[0].key, None);
        assert!(!message.attachments[0].oversized);
        assert_eq!(missing_media_count(&root, &message), 1);
        std::fs::remove_file(&root).unwrap();
        assert_eq!(
            write_media_with_cap(&ctx, 0, "file.bin", "application/octet-stream", b"data", 3),
            None
        );
        assert!(!root.exists());
    }

    #[test]
    fn keyless_attachments_trigger_media_recovery_unless_oversized() {
        // A row cached before attachments were persisted has a key-less file;
        // it must report unavailable so the caller refetches and writes it.
        // One left off disk for its size would only be left off again.
        let root = std::env::temp_dir().join(format!("meron-keyless-{}", std::process::id()));
        let mut msg = Message::default();
        msg.attachments.push(Attachment {
            filename: "invite.ics".to_string(),
            mime: "text/calendar".to_string(),
            size: 3072,
            key: None,
            oversized: false,
        });
        assert!(!cached_media_available(&root, &msg));
        assert_eq!(missing_media_count(&root, &msg), 1);
        msg.attachments[0].oversized = true;
        assert!(cached_media_available(&root, &msg));
        assert_eq!(missing_media_count(&root, &msg), 0);
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

    #[test]
    fn reads_the_envelope_of_an_outgoing_message() {
        let raw = concat!(
            "From: Me <Me@Example.com>\r\n",
            "To: \"B\" <b@example.com>, c@example.com\r\n",
            "Cc: A@Example.com\r\n",
            "Bcc: hidden@example.com\r\n",
            "Subject: =?utf-8?q?Re=3A_lunch?=\r\n",
            "Message-ID: <reply-1@meron>\r\n",
            "Date: Tue, 14 Nov 2023 22:13:20 +0000\r\n",
            "\r\n",
            "body\r\n",
        );
        let envelope = sent_envelope_of(raw.as_bytes());
        assert_eq!(envelope.message_id, "reply-1@meron");
        assert_eq!(envelope.subject, "Re: lunch");
        assert_eq!(envelope.from_addr, "Me@Example.com");
        // To + Cc, lowercased and sorted; Bcc stays out — the sending server
        // strips it, so the copy that comes back never carries it.
        assert_eq!(
            envelope.recipients,
            vec![
                "a@example.com".to_string(),
                "b@example.com".to_string(),
                "c@example.com".to_string()
            ]
        );
        assert_eq!(envelope.date, 1_700_000_000);
    }

    #[test]
    fn an_unparseable_message_has_a_blank_envelope() {
        assert_eq!(
            sent_envelope_of(b"\x00not a message"),
            SentEnvelope::default()
        );
    }
}

#[cfg(test)]
mod document_media_cache_tests {
    use super::*;

    #[test]
    fn pruning_preserves_open_document_media() {
        let root =
            std::env::temp_dir().join(format!("meron-reader-prune-{}", uuid::Uuid::new_v4()));
        let document = root.join("_eml/reader/1/attachment.png");
        let cached = root.join("account/INBOX/1/attachment.png");
        for path in [&document, &cached] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        std::fs::write(&document, b"reader attachment").unwrap();
        // A sparse file exceeds the cache cap without allocating a GiB of data.
        std::fs::File::create(&cached)
            .unwrap()
            .set_len(media_cache_cap() + 1)
            .unwrap();
        prune_media_cache(&root);
        assert_eq!(std::fs::read(&document).unwrap(), b"reader attachment");
        assert!(!cached.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
