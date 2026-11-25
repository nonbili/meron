use super::*;

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
pub(super) fn bare_addresses(values: &[String]) -> Vec<String> {
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
pub(super) fn reencode_names(raw: &str) -> String {
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

pub(super) fn reencode_name(name: &str) -> String {
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
pub(super) fn flatten_name(name: &str) -> String {
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
pub(super) fn encoded_word_at(text: &str, i: usize) -> Option<usize> {
    text[i..]
        .starts_with("=?")
        .then(|| encoded_word_len(&text[i..]))
        .flatten()
}

/// Decode every well-formed encoded-word in `text`, wherever it sits, dropping
/// whitespace between adjacent encoded-words as RFC 2047 requires.
pub(super) fn decode_words_lenient(text: &str) -> String {
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
pub(super) fn encoded_word_len(s: &str) -> Option<usize> {
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
pub(super) fn from_of(headers: &[MailHeader]) -> (String, String) {
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
