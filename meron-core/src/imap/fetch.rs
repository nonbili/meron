use super::*;

/// SELECT `folder` and return `(UIDVALIDITY, UIDNEXT, uids)`, where `uids` holds
/// the most recent `limit` messages newest first — the same window
/// [`fetch_recent`] covers, but established without fetching any message.
///
/// Used by the poison-message recovery path, which needs the window as UIDs it
/// can subdivide. A `UID SEARCH` reply is a list of bare numbers, so it cannot
/// itself hit the parse failure that sent us down this path.
pub async fn recent_uids(
    session: &mut Session,
    folder: &str,
    limit: u32,
) -> Result<(u32, u32, Vec<u32>)> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let uidvalidity = mailbox.uid_validity.unwrap_or(0);
    let uid_next = mailbox.uid_next.unwrap_or(0);
    let set: HashSet<u32> = session.uid_search("ALL").await.context("UID SEARCH ALL")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    uids.truncate(limit.max(1) as usize);
    Ok((uidvalidity, uid_next, uids))
}

/// Fetch the most recent `limit` messages in `folder` as envelope summaries,
/// newest first. This is the capability Delta Chat core refuses to give us:
/// reading mail that already existed before setup.
pub async fn fetch_recent(session: &mut Session, folder: &str, limit: u32) -> Result<RecentBatch> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let uidvalidity = mailbox.uid_validity.unwrap_or(0);
    let uid_next = mailbox.uid_next.unwrap_or(0);
    let total = mailbox.exists;
    if total == 0 {
        return Ok(RecentBatch {
            uidvalidity,
            uid_next,
            messages: Vec::new(),
        });
    }
    let limit = limit.max(1);
    let start = total.saturating_sub(limit).saturating_add(1).max(1);
    let set = format!("{start}:{total}");

    let gmail = supports_gmail_ext(session).await;
    let mut out = Vec::new();
    let mut stream = session
        .fetch(set, fetch_items(gmail, false))
        .await
        .context("FETCH")?;
    while let Some(item) = stream.next().await {
        let fetch = item.context("FETCH item")?;
        let uid = match fetch.uid {
            Some(uid) => uid,
            None => continue,
        };
        let seen = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
        let starred = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
        let ef = fetch.header().map(header_fields).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            ef.encrypted,
            &ef.subject,
            &ef.message_id,
            &ef.in_reply_to,
            &ef.references_root,
            uid,
        );
        out.push(MessageHeader {
            uid,
            encrypted: ef.encrypted,
            subject: ef.subject,
            from_name: ef.from_name,
            from_addr: ef.from_addr,
            date: message_date(ef.date, &fetch),
            seen,
            starred,
            server_tags: Some(fetch_tags(&fetch)),
            thread_key,
            message_id: ef.message_id,
            gmail_msg_id: fetch.gmail_msg_id().copied(),
            in_reply_to: ef.in_reply_to,
            folder: String::new(),
            to: ef.to,
            cc: ef.cc,
            recipient_overflow: 0,
            files: fetch.bodystructure().map(attachment_files),
        });
    }
    drop(stream);
    out.reverse();
    Ok(RecentBatch {
        uidvalidity,
        uid_next,
        messages: out,
    })
}

/// A message located by Message-ID and fully fetched: its header row (with a
/// computed `thread_key` and source `folder` populated) plus the parsed body.
/// Used by the on-demand ancestor fetch that fills thread gaps when a reply or
/// draft references messages outside the locally-synced window.
pub struct FetchedMessage {
    pub header: MessageHeader,
    pub message: parse::Message,
}

/// Locate messages in `folder` whose `Message-ID` matches any id in `ids` (bare,
/// no angle brackets) and fetch flags + envelope + full body for each match in a
/// single round-trip. Returns one `FetchedMessage` per located UID; ids with no
/// match here are simply absent from the result so the caller can try the next
/// folder. The `thread_key` is computed exactly as the recent-sync path does, so
/// a fetched ancestor groups into the same thread as the reply that referenced it.
pub async fn fetch_by_message_ids(
    session: &mut Session,
    folder: &str,
    ids: &[String],
    media_root: &std::path::Path,
    account: &str,
) -> Result<Vec<FetchedMessage>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let set: HashSet<u32> = session
        .uid_search(message_id_search_criteria(ids))
        .await
        .context("UID SEARCH Message-ID")?;
    if set.is_empty() {
        return Ok(Vec::new());
    }
    let uids = set.into_iter().collect::<Vec<_>>();
    let gmail = supports_gmail_ext(session).await;
    let mut out = Vec::new();
    for uid_set in uid_set_chunks(&uids, MAX_UID_SET_LEN) {
        let mut stream = session
            .uid_fetch(uid_set, fetch_items(gmail, true))
            .await
            .context("UID FETCH Message-ID matches")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH item")?;
            let uid = match fetch.uid {
                Some(uid) => uid,
                None => continue,
            };
            let raw = match fetch.body() {
                Some(body) => body.to_vec(),
                None => continue,
            };
            let seen = fetch
                .flags()
                .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
            let starred = fetch
                .flags()
                .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
            let ef = fetch.header().map(header_fields).unwrap_or_default();
            let thread_key = thread_key(
                fetch.gmail_thread_id().copied(),
                ef.encrypted,
                &ef.subject,
                &ef.message_id,
                &ef.in_reply_to,
                &ef.references_root,
                uid,
            );
            let media = parse::MediaCtx {
                root: media_root.to_path_buf(),
                account: account.to_string(),
                folder: folder.to_string(),
                uid,
            };
            let mut message = parse::parse_message(&raw, Some(&media));
            let date = message_date(ef.date, &fetch);
            if message.date == 0 {
                message.date = date;
            }
            out.push(FetchedMessage {
                header: MessageHeader {
                    uid,
                    encrypted: ef.encrypted,
                    folder: folder.to_string(),
                    subject: ef.subject,
                    from_name: ef.from_name,
                    from_addr: ef.from_addr,
                    date,
                    seen,
                    starred,
                    server_tags: Some(fetch_tags(&fetch)),
                    thread_key,
                    message_id: ef.message_id,
                    gmail_msg_id: fetch.gmail_msg_id().copied(),
                    in_reply_to: ef.in_reply_to,
                    to: ef.to,
                    cc: ef.cc,
                    recipient_overflow: 0,
                    files: fetch.bodystructure().map(attachment_files),
                },
                message,
            });
        }
    }
    Ok(out)
}

/// Build a SEARCH criteria matching any `HEADER MESSAGE-ID` in `ids`. IMAP SEARCH
/// has no native set membership, so terms are chained with the binary prefix
/// `OR` operator (`OR a b`, then `OR (OR a b) c`, …).
pub(super) fn message_id_search_criteria(ids: &[String]) -> String {
    let mut terms = ids
        .iter()
        .map(|id| format!("HEADER MESSAGE-ID {}", imap_quote(id)));
    let Some(first) = terms.next() else {
        return "ALL".to_string();
    };
    terms.fold(first, |expr, term| format!("OR {expr} {term}"))
}

pub async fn fetch_headers_by_uid(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<MessageHeader>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let gmail = supports_gmail_ext(session).await;
    let mut out = Vec::new();
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        let mut stream = session
            .uid_fetch(uid_set, fetch_items(gmail, false))
            .await
            .context("UID FETCH search headers")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH search header item")?;
            let uid = match fetch.uid {
                Some(uid) => uid,
                None => continue,
            };
            let seen = fetch
                .flags()
                .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
            let starred = fetch
                .flags()
                .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
            let ef = fetch.header().map(header_fields).unwrap_or_default();
            let thread_key = thread_key(
                fetch.gmail_thread_id().copied(),
                ef.encrypted,
                &ef.subject,
                &ef.message_id,
                &ef.in_reply_to,
                &ef.references_root,
                uid,
            );
            out.push(MessageHeader {
                uid,
                encrypted: ef.encrypted,
                subject: ef.subject,
                from_name: ef.from_name,
                from_addr: ef.from_addr,
                date: message_date(ef.date, &fetch),
                seen,
                starred,
                server_tags: Some(fetch_tags(&fetch)),
                thread_key,
                message_id: ef.message_id,
                gmail_msg_id: fetch.gmail_msg_id().copied(),
                in_reply_to: ef.in_reply_to,
                folder: String::new(),
                to: ef.to,
                cc: ef.cc,
                recipient_overflow: 0,
                files: fetch.bodystructure().map(attachment_files),
            });
        }
    }
    out.sort_unstable_by(|a, b| b.uid.cmp(&a.uid));
    Ok(out)
}

/// The list header of a message held as raw RFC822 rather than on a server
/// (local folders), read the same way a header FETCH is, so stored mail threads
/// and lists exactly like synced mail.
pub fn header_from_raw(raw: &[u8], uid: u32, seen: bool, starred: bool) -> MessageHeader {
    let ef = header_fields(raw);
    let thread_key = thread_key(
        None,
        ef.encrypted,
        &ef.subject,
        &ef.message_id,
        &ef.in_reply_to,
        &ef.references_root,
        uid,
    );
    MessageHeader {
        uid,
        encrypted: ef.encrypted,
        subject: ef.subject,
        from_name: ef.from_name,
        from_addr: ef.from_addr,
        date: ef.date,
        seen,
        starred,
        thread_key,
        message_id: ef.message_id,
        in_reply_to: ef.in_reply_to,
        to: ef.to,
        cc: ef.cc,
        ..Default::default()
    }
}

/// Each of `uids`' Date header as epoch seconds (INTERNALDATE when absent or
/// unparseable), the same date [`fetch_headers_by_uid`] reports. Fetches only
/// that one header field, so a search can order a large hit set by date without
/// downloading every header. UIDs the server no longer has are left out.
pub async fn fetch_dates_by_uid(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<(u32, i64)>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let mut out = Vec::with_capacity(uids.len());
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        let mut stream = session
            .uid_fetch(
                uid_set,
                "(UID INTERNALDATE BODY.PEEK[HEADER.FIELDS (DATE)])",
            )
            .await
            .context("UID FETCH search dates")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH search date item")?;
            let Some(uid) = fetch.uid else {
                continue;
            };
            let date = fetch
                .header()
                .and_then(|header| mailparse::parse_headers(header).ok())
                .and_then(|(headers, _)| headers.get_first_value("Date"))
                .map(|raw| parse::parse_date_to_epoch(&raw))
                .unwrap_or_default();
            out.push((uid, message_date(date, &fetch)));
        }
    }
    Ok(out)
}

/// Fetch and parse one full message by UID, assuming `folder` is already
/// selected on `session`. Shared by the on-demand reader (which selects then
/// calls this once) and the body prefetcher (which selects once and pulls many
/// UIDs over a single connection).
///
/// `peek` selects `BODY.PEEK[]` over `RFC822`: both return the full message, but
/// `RFC822` implicitly sets `\Seen` server-side while `BODY.PEEK[]` does not. The
/// prefetcher must peek so background warming never marks unread mail as read.
pub async fn fetch_full_message(
    session: &mut Session,
    uid: u32,
    media: &parse::MediaCtx,
    peek: bool,
) -> Result<Option<parse::Message>> {
    let item = if peek {
        "(INTERNALDATE BODY.PEEK[])"
    } else {
        "(INTERNALDATE RFC822)"
    };
    let mut stream = session
        .uid_fetch(uid.to_string(), item)
        .await
        .context("UID FETCH")?;
    let mut message: Option<parse::Message> = None;
    while let Some(item) = stream.next().await {
        let fetch = item.context("UID FETCH item")?;
        if let Some(body) = fetch.body() {
            let mut parsed = parse::parse_message(body, Some(media));
            parsed.date = message_date(parsed.date, &fetch);
            message = Some(parsed);
            break;
        }
    }
    drop(stream);
    Ok(message)
}

/// How many messages one `UID FETCH` pulls. One round trip per message dominates
/// when the bodies themselves are small; a short batch still bounds how long a
/// caller holds the connection before an interactive read can use it.
pub const BODY_FETCH_BATCH: usize = 4;

/// Select `folder` and fetch full bodies for `uids`, parsing each into a
/// `Message`. Used by the thread reader and the body prefetcher. `peek` so
/// reading doesn't flip server-side `\Seen`.
///
/// Each message goes to `on_message` as it arrives rather than into a returned
/// list, so a caller keeps what was downloaded before an error or a deadline
/// cut the rest short.
pub async fn fetch_bodies(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
    media_root: std::path::PathBuf,
    account: &str,
    on_message: &mut (dyn FnMut(u32, parse::Message) + Send),
) -> Result<()> {
    // One recursive walk per batch, including one cut short by an error, a
    // deadline or cancellation: what arrived before that is already on disk.
    let mut prune = PruneMediaOnDrop(Some(media_root.clone()));
    let result =
        fetch_bodies_unpruned(session, folder, uids, media_root, account, on_message).await;
    // A fetch that ran to its end waits for the prune, so a caller counting
    // attachment files afterwards sees the ones that are going to stay.
    if let Some(root) = prune.0.take() {
        let _ = tokio::task::spawn_blocking(move || parse::prune_media_cache(&root)).await;
    }
    result
}

/// Prunes the media cache when a body fetch is dropped where it stands, by a
/// caller's timeout or cancellation, and so never reaches its own prune.
pub(super) struct PruneMediaOnDrop(Option<std::path::PathBuf>);

impl Drop for PruneMediaOnDrop {
    fn drop(&mut self) {
        let Some(root) = self.0.take() else {
            return;
        };
        // On a blocking worker when there is a runtime to lend one.
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => drop(runtime.spawn_blocking(move || parse::prune_media_cache(&root))),
            Err(_) => parse::prune_media_cache(&root),
        }
    }
}

pub(super) async fn fetch_bodies_unpruned(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
    media_root: std::path::PathBuf,
    account: &str,
    on_message: &mut (dyn FnMut(u32, parse::Message) + Send),
) -> Result<()> {
    session.select(folder).await.context("SELECT")?;
    for chunk in uids.chunks(BODY_FETCH_BATCH) {
        if chunk.is_empty() {
            continue;
        }
        let set = chunk
            .iter()
            .map(|uid| uid.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut stream = session
            .uid_fetch(set, "(UID INTERNALDATE BODY.PEEK[])")
            .await
            .context("UID FETCH")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH item")?;
            let Some(uid) = fetch.uid else { continue };
            let Some(body) = fetch.body() else { continue };
            let media = parse::MediaCtx {
                root: media_root.clone(),
                account: account.to_string(),
                folder: folder.to_string(),
                uid,
            };
            let mut parsed = parse::parse_message_unpruned(body, Some(&media));
            parsed.date = message_date(parsed.date, &fetch);
            on_message(uid, parsed);
        }
        drop(stream);
    }
    Ok(())
}

/// Message fields parsed out of the fetched RFC822 header. A named struct
/// rather than a tuple because there are enough of them (and the recipient
/// lists) that positional destructuring at the call sites would be error-prone.
#[derive(Default)]
pub(super) struct HeaderFields {
    pub(super) subject: String,
    pub(super) from_name: String,
    pub(super) from_addr: String,
    pub(super) date: i64,
    pub(super) message_id: String,
    pub(super) in_reply_to: String,
    /// First id of the `References` header, i.e. the thread root.
    pub(super) references_root: String,
    pub(super) to: Vec<Recipient>,
    pub(super) cc: Vec<Recipient>,
    /// `Content-Type: multipart/encrypted` (PGP/MIME).
    pub(super) encrypted: bool,
}

/// Everything the message list needs, parsed from `RFC822.HEADER`.
///
/// The IMAP `ENVELOPE` carries the same fields, but imap-proto rejects any
/// quoted string holding a byte outside `0x01..=0x7F`, so a single message whose
/// server emitted a raw 8-bit display name (rather than a literal or an RFC 2047
/// encoded-word) fails to parse. async-imap leaves that response in its buffer
/// unconsumed, wedging the connection, and the next sync refetches the same
/// range and wedges again — one message stalls the whole folder forever. The
/// header arrives as a literal, copied out by length with no parsing, so it
/// cannot hit that. It is also what we already trusted over the envelope for the
/// subject: imap-proto hands back quoted-string contents without unescaping, so
/// an envelope subject containing double quotes arrived as `\"...\"` and, being
/// part of the compound thread key, split the conversation into a duplicate
/// thread.
pub(super) fn header_fields(header: &[u8]) -> HeaderFields {
    let Ok((headers, _)) = mailparse::parse_headers(header) else {
        return HeaderFields::default();
    };
    let headers = headers.as_slice();
    let mut from = header_addrs(headers, "From");
    let (from_name, from_addr) = if from.is_empty() {
        (String::new(), String::new())
    } else {
        let first = from.remove(0);
        (first.name, first.addr)
    };
    HeaderFields {
        subject: headers.get_first_value("Subject").unwrap_or_default(),
        from_name,
        from_addr,
        date: headers
            .get_first_value("Date")
            .map(|raw| parse::parse_date_to_epoch(&raw))
            .unwrap_or_default(),
        message_id: headers
            .get_first_value("Message-ID")
            .map(|raw| normalize_message_id(&raw))
            .unwrap_or_default(),
        in_reply_to: headers
            .get_first_value("In-Reply-To")
            .as_deref()
            .and_then(first_message_id)
            .unwrap_or_default(),
        references_root: headers
            .get_first_value("References")
            .as_deref()
            .and_then(first_message_id)
            .unwrap_or_default(),
        to: header_addrs(headers, "To"),
        cc: header_addrs(headers, "Cc"),
        encrypted: headers
            .get_first_value("Content-Type")
            .is_some_and(|value| {
                mailparse::parse_content_type(&value)
                    .mimetype
                    .eq_ignore_ascii_case("multipart/encrypted")
            }),
    }
}

/// Flatten one address header into recipients, expanding RFC 5322 groups and
/// dropping entries that yield no email address. A header that fails to parse
/// contributes nothing rather than failing the message.
pub(super) fn header_addrs(headers: &[mailparse::MailHeader<'_>], key: &str) -> Vec<Recipient> {
    let Some(header) = headers.get_first_header(key) else {
        return Vec::new();
    };
    let Some(list) = parse::addrparse_lenient(header) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in list.iter() {
        match entry {
            mailparse::MailAddr::Single(info) => push_recipient(&mut out, info),
            mailparse::MailAddr::Group(group) => {
                for info in &group.addrs {
                    push_recipient(&mut out, info);
                }
            }
        }
    }
    out
}

pub(super) fn push_recipient(out: &mut Vec<Recipient>, info: &mailparse::SingleInfo) {
    if info.addr.is_empty() {
        return;
    }
    out.push(Recipient {
        name: info.display_name.clone().unwrap_or_default(),
        addr: info.addr.clone(),
    });
}

/// The message's send time: its `Date` header when that parses, otherwise the
/// server's INTERNALDATE (arrival time). Some senders omit the header or write
/// one chrono rejects; without the fallback such a message is dated 1970 and
/// sinks to the bottom of every date-ordered list.
pub(super) fn message_date(header_date: i64, fetch: &async_imap::types::Fetch) -> i64 {
    if header_date != 0 {
        return header_date;
    }
    fetch
        .internal_date()
        .map(|date| date.timestamp())
        .unwrap_or_default()
}

/// Attachment metadata without downloading the MIME payload. Inline CID images
/// are left to the cached body's more precise HTML-reference check.
pub(super) fn attachment_files(
    body: &async_imap::imap_proto::types::BodyStructure<'_>,
) -> Vec<crate::store::CardFile> {
    use async_imap::imap_proto::types::BodyStructure;
    let (common, other) = match body {
        BodyStructure::Multipart { bodies, .. } => {
            return bodies.iter().flat_map(attachment_files).collect();
        }
        BodyStructure::Basic { common, other, .. }
        | BodyStructure::Text { common, other, .. }
        | BodyStructure::Message { common, other, .. } => (common, other),
    };
    let disposition = common.disposition.as_ref();
    let explicit_attachment = disposition.is_some_and(|d| d.ty.eq_ignore_ascii_case("attachment"));
    let filename = disposition
        .and_then(|d| body_parameter(&d.params, "filename"))
        .or_else(|| body_parameter(&common.ty.params, "name"));
    if !explicit_attachment && common.ty.ty.eq_ignore_ascii_case("image") && other.id.is_some() {
        return Vec::new();
    }
    let mime = format!("{}/{}", common.ty.ty, common.ty.subtype).to_ascii_lowercase();
    if !parse::is_attachment_part(&mime, explicit_attachment, filename.is_some()) {
        return Vec::new();
    }
    vec![crate::store::CardFile {
        filename: filename.unwrap_or_else(|| "attachment".to_string()),
        mime,
    }]
}

/// Reuse the MIME parser's RFC 2231 charset/continuation handling, and decode
/// the RFC 2047 words some servers return in otherwise ordinary name params.
pub(super) fn body_parameter(
    params: &async_imap::imap_proto::types::BodyParams<'_>,
    name: &str,
) -> Option<String> {
    let params = params.as_ref()?;
    if let Some((_, value)) = params
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
    {
        return Some(parse::decode_words(&format!(" {value}")));
    }
    let prefix = format!("{name}*");
    let mut header = String::from("attachment");
    for (key, value) in params {
        if key.to_ascii_lowercase().starts_with(&prefix) {
            header.push_str(&format!("; {key}=\"{value}\""));
        }
    }
    mailparse::parse_content_disposition(&header)
        .params
        .get(name)
        .map(|value| parse::decode_words(&format!(" {value}")))
}

/// FETCH item list. Adds `X-GM-THRID` on Gmail so messages thread by Gmail's
/// server-side thread id, and `BODY.PEEK[]` when the full message is wanted.
pub(super) fn fetch_items(gmail: bool, body: bool) -> &'static str {
    match (gmail, body) {
        (true, true) => {
            "(UID FLAGS INTERNALDATE RFC822.HEADER BODYSTRUCTURE X-GM-MSGID X-GM-THRID BODY.PEEK[])"
        }
        (true, false) => {
            "(UID FLAGS INTERNALDATE RFC822.HEADER BODYSTRUCTURE X-GM-MSGID X-GM-THRID)"
        }
        (false, true) => "(UID FLAGS INTERNALDATE RFC822.HEADER BODYSTRUCTURE BODY.PEEK[])",
        (false, false) => "(UID FLAGS INTERNALDATE RFC822.HEADER BODYSTRUCTURE)",
    }
}

/// Whether the server advertises Gmail's `X-GM-EXT-1` extension, i.e. supports
/// `X-GM-THRID`. Covers Gmail/Workspace via both OAuth and app passwords. Issues
/// one CAPABILITY round-trip; negligible next to the connect-time TLS + auth.
pub(super) async fn supports_gmail_ext(session: &mut Session) -> bool {
    match session.capabilities().await {
        Ok(caps) => caps.has_str("X-GM-EXT-1"),
        Err(_) => false,
    }
}

/// Whether the server advertises CONDSTORE (RFC 7162), i.e. accepts
/// `SELECT ... (CONDSTORE)` and `FETCH ... (CHANGEDSINCE n)`. Absent it, the
/// flag reconciliation in [`sync_flags`] must not be attempted at all: a server
/// that rejects the parameter without a properly tagged response leaves the
/// connection unusable (see the note there).
pub(super) async fn supports_condstore(session: &mut Session) -> bool {
    match session.capabilities().await {
        Ok(caps) => caps.has_str("CONDSTORE"),
        Err(_) => false,
    }
}

pub(super) fn thread_key(
    gmail_thrid: Option<u64>,
    encrypted: bool,
    subject: &str,
    message_id: &str,
    in_reply_to: &str,
    references_root: &str,
    uid: u32,
) -> String {
    // Gmail computes threads server-side (References + In-Reply-To + subject
    // similarity), matching what the user sees in the Gmail UI. Prefer it when
    // the server exposes X-GM-THRID; namespace it so it never collides with a
    // Message-ID-based key.
    // Not for encrypted mail: Gmail only sees the placeholder subject, the same
    // for every encrypted message, so it merges unrelated conversations between
    // the same people (even a new message). Those thread by their own headers.
    // A plain message that happens to be titled `...` is not one of them.
    let gmail_thrid = gmail_thrid
        .filter(|_| !(encrypted && crate::store::is_encrypted_placeholder_subject(subject)));
    if let Some(thrid) = gmail_thrid {
        return format!("gmthrid:{thrid}");
    }
    if !references_root.is_empty() {
        return references_root.to_string();
    }
    if !in_reply_to.is_empty() {
        return in_reply_to.to_string();
    }
    if !message_id.is_empty() {
        return message_id.to_string();
    }
    format!("uid:{uid}")
}

/// Strip angle brackets and whitespace from an RFC Message-ID, preserving the
/// original casing. Message-IDs are case-sensitive opaque tokens (RFC 5322) and
/// this value is echoed back on the wire in reply `In-Reply-To`/`References`
/// headers, where a case change breaks the recipient's threading (e.g. Gmail
/// matches ids byte-for-byte). Store lookups that compare ids wrap both sides
/// in `lower()` instead.
pub(super) fn normalize_message_id(value: &str) -> String {
    if let Some(start) = value.find('<')
        && let Some(end) = value[start..].find('>')
    {
        let id = &value[start + 1..start + end];
        return id.trim().to_string();
    }
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string()
}

/// The first RFC message id in a header value that may hold several, such as
/// `References` (thread root first) or an `In-Reply-To` carrying more than one.
///
/// Angle brackets decide where the id starts, not whitespace: RFC 822 allowed a
/// leading phrase, and senders still emit `In-Reply-To: Your message of Tuesday
/// <parent@example.com>`. Splitting on whitespace first would take `Your` as the
/// id and, absent `References`, thread every such message together. Only a value
/// with no brackets at all falls back to its first whitespace-separated token.
pub(super) fn first_message_id(value: &str) -> Option<String> {
    if value.contains('<') {
        let id = normalize_message_id(value);
        return (!id.is_empty()).then_some(id);
    }
    value
        .split_whitespace()
        .map(normalize_message_id)
        .find(|id| !id.is_empty())
}
