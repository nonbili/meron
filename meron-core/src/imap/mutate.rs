use super::*;

/// Move `uids` from `source_folder` to `dest_folder`. Returns the target's
/// UIDNEXT from just before the move, when the server reported it.
pub async fn move_to_folder(
    session: &mut Session,
    source_folder: &str,
    dest_folder: &str,
    uids: &[u32],
) -> Result<Option<u32>> {
    if uids.is_empty() {
        return Ok(None);
    }
    // The target's UIDNEXT before the move: every copy the move creates lands
    // at or above it, which is how a caller finds them again — async-imap
    // drops the COPYUID response code. Best-effort; None if STATUS fails.
    let target_uid_next = session
        .status(dest_folder, "(UIDNEXT)")
        .await
        .ok()
        .and_then(|mailbox| mailbox.uid_next);
    session.select(source_folder).await.context("SELECT")?;
    // UID MOVE is the RFC 6851 MOVE extension; servers that don't advertise it
    // (e.g. mailo) reject it with "Unknown command". Fall back to the classic
    // COPY + \Deleted + EXPUNGE sequence, which every IMAP server supports.
    let supports_move = match session.capabilities().await {
        Ok(caps) => caps.has_str("MOVE"),
        Err(_) => false,
    };
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        if supports_move {
            session
                .uid_mv(&uid_set, dest_folder)
                .await
                .context("UID MOVE")?;
        } else {
            session
                .uid_copy(&uid_set, dest_folder)
                .await
                .context("UID COPY")?;
        }
    }
    if !supports_move {
        expunge_uids(session, source_folder, uids)
            .await
            .context("Move copied messages but could not confirm removal from the source folder")?;
    }
    Ok(target_uid_next)
}

/// How far a [`move_to_folder_once`] got.
#[derive(Debug, Default)]
pub struct MoveOutcome {
    /// What has left the source, including what was already gone.
    pub moved: Vec<u32>,
    /// Why the rest of the UIDs are still there.
    pub failed: Option<anyhow::Error>,
}

/// [`move_to_folder`] that may be repeated after being cut off at any point. A
/// server with MOVE does that by itself. Without it a move is a copy and a
/// delete, so what `dest` already holds is not copied again, only removed from
/// the source. `message_ids` lines up with `uids`; a message with `""` can't be
/// told from its copy, so it stays where it is and the outcome says so.
pub async fn move_to_folder_once(
    session: &mut Session,
    source_folder: &str,
    dest_folder: &str,
    uids: &[u32],
    message_ids: &[String],
) -> Result<MoveOutcome> {
    let supports_move = match session.capabilities().await {
        Ok(caps) => caps.has_str("MOVE"),
        Err(_) => false,
    };
    if supports_move {
        move_to_folder(session, source_folder, dest_folder, uids).await?;
        return Ok(MoveOutcome {
            moved: uids.to_vec(),
            failed: None,
        });
    }
    let sizes = message_sizes(session, source_folder, uids).await?;
    session.select(dest_folder).await.context("SELECT")?;
    let (mut copied, mut pending, mut gone, mut unidentified) =
        (Vec::new(), Vec::new(), Vec::new(), 0);
    for (uid, id) in uids.iter().zip(message_ids) {
        // Gone from the source: another client got there, nothing to move.
        let Some(size) = sizes.get(uid) else {
            gone.push(*uid);
            continue;
        };
        if id.is_empty() {
            unidentified += 1;
        } else if holds_copy(session, id, *size).await? {
            copied.push(*uid);
        } else {
            pending.push(*uid);
        }
    }
    move_to_folder(session, source_folder, dest_folder, &pending).await?;
    let mut moved = pending;
    moved.append(&mut gone);
    if let Err(err) = expunge_uids(session, source_folder, &copied).await {
        return Ok(MoveOutcome {
            moved,
            failed: Some(err),
        });
    }
    moved.append(&mut copied);
    let failed = (unidentified > 0).then(|| {
        anyhow::anyhow!(
            "{unidentified} messages have no Message-ID to tell a retry by, so they were not moved"
        )
    });
    Ok(MoveOutcome { moved, failed })
}

/// The UIDVALIDITY `folder` has on the server right now.
pub async fn folder_uidvalidity(session: &mut Session, folder: &str) -> Result<u32> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    Ok(mailbox.uid_validity.unwrap_or(0))
}

pub(super) async fn message_sizes(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
) -> Result<HashMap<u32, u32>> {
    session.select(folder).await.context("SELECT")?;
    let mut sizes = HashMap::new();
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        let mut stream = session
            .uid_fetch(uid_set, "(UID RFC822.SIZE)")
            .await
            .context("UID FETCH size")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH size item")?;
            if let (Some(uid), Some(size)) = (fetch.uid, fetch.size) {
                sizes.insert(uid, size);
            }
        }
    }
    Ok(sizes)
}

/// Whether `folder` holds this very message, byte for byte: a caller may give up
/// the source on a yes. SEARCH only finds candidates, and a resent message can
/// keep its Message-ID and even its size. In doubt this says no, and the price is
/// a duplicate. On a yes the copy gets the source's read and starred state, added
/// to whatever it has, as an APPEND would have given it.
pub async fn folder_holds_message(
    session: &mut Session,
    folder: &str,
    id: &str,
    message: &RawMessageCopy,
) -> Result<bool> {
    session.select(folder).await.context("SELECT")?;
    let hits = session
        .uid_search(format!("UNDELETED HEADER MESSAGE-ID {}", imap_quote(id)))
        .await
        .context("UID SEARCH")?;
    if hits.is_empty() {
        return Ok(false);
    }
    let uid_set = hits
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut found = None;
    {
        let mut stream = session
            .uid_fetch(uid_set, "(UID BODY.PEEK[])")
            .await
            .context("UID FETCH candidate copy")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH candidate copy item")?;
            if fetch.body() == Some(message.raw.as_slice()) {
                found = fetch.uid.or(found);
            }
        }
    }
    let Some(uid) = found else {
        return Ok(false);
    };
    for (wanted, flag) in [(message.seen, "\\Seen"), (message.starred, "\\Flagged")] {
        if wanted {
            let mut stream = session
                .uid_store(uid.to_string(), format!("+FLAGS.SILENT ({flag})"))
                .await
                .context("UID STORE copy flags")?;
            while let Some(item) = stream.next().await {
                item.context("UID STORE item")?;
            }
        }
    }
    Ok(true)
}

/// Whether the selected mailbox holds a copy of a message: the same whole
/// Message-ID at the same size. SEARCH can't say, as it matches substrings, and
/// a resent message can keep its ID, so its hits are read back. In doubt this
/// says no: the price is a duplicate, where a wrong yes would lose the original.
pub(super) async fn holds_copy(session: &mut Session, id: &str, size: u32) -> Result<bool> {
    let hits = session
        .uid_search(format!("HEADER MESSAGE-ID {}", imap_quote(id)))
        .await
        .context("UID SEARCH")?;
    if hits.is_empty() {
        return Ok(false);
    }
    let uid_set = hits
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let wanted = id.trim().to_lowercase();
    let mut found = false;
    let mut stream = session
        .uid_fetch(uid_set, "(UID RFC822.SIZE BODY.PEEK[HEADER])")
        .await
        .context("UID FETCH Message-ID")?;
    while let Some(item) = stream.next().await {
        let fetch = item.context("UID FETCH Message-ID item")?;
        let header = fetch.header().unwrap_or_default();
        found |= fetch.size == Some(size)
            && crate::parse::message_id_of(header).trim().to_lowercase() == wanted;
    }
    Ok(found)
}

#[derive(Clone)]
pub struct RawMessageCopy {
    /// The source UID, or 0 when the server left it out of the FETCH reply.
    pub uid: u32,
    pub raw: Vec<u8>,
    pub seen: bool,
    pub starred: bool,
}

/// Fetch raw RFC822 bytes plus the user-visible flags worth preserving when a
/// message is copied into another mailbox. Uses BODY.PEEK[] so copying never
/// marks unread source mail as read.
pub async fn fetch_raw_messages_for_copy(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<RawMessageCopy>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let mut out = Vec::new();
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        let mut stream = session
            .uid_fetch(uid_set, "(FLAGS BODY.PEEK[])")
            .await
            .context("UID FETCH raw copy")?;
        while let Some(item) = stream.next().await {
            let fetch = item.context("UID FETCH raw copy item")?;
            if let Some(body) = fetch.body() {
                let seen = fetch
                    .flags()
                    .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
                let starred = fetch
                    .flags()
                    .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
                out.push(RawMessageCopy {
                    uid: fetch.uid.unwrap_or(0),
                    raw: body.to_vec(),
                    seen,
                    starred,
                });
            }
        }
    }
    Ok(out)
}

/// async-imap's APPEND expects the first response to be a continuation, but
/// selected pooled sessions may receive unsolicited mailbox updates first.
/// Drain those updates while waiting for the continuation and tagged completion.
/// Cache reconciliation uses separate sync/IDLE sessions, so these updates do
/// not need to be applied here.
pub(super) async fn append_message(
    session: &mut Session,
    folder: &str,
    flags: Option<&str>,
    raw: &[u8],
) -> Result<()> {
    use async_imap::imap_proto::{Response, Status};
    use tokio::io::AsyncWriteExt;

    anyhow::ensure!(
        !folder.contains(['\r', '\n', '\0']),
        "Invalid APPEND mailbox name"
    );
    let mailbox = format!("\"{}\"", folder.replace('\\', "\\\\").replace('"', "\\\""));
    let flags = flags.map(|flags| format!(" {flags}")).unwrap_or_default();
    let id = session
        .run_command(format!("APPEND {mailbox}{flags} {{{}}}", raw.len()))
        .await?;
    let mut sent = false;
    loop {
        let response = session
            .read_response()
            .await?
            .ok_or(async_imap::error::Error::ConnectionLost)?;
        match response.parsed() {
            Response::Continue { .. } if !sent => {
                session.as_mut().write_all(raw).await?;
                session.as_mut().write_all(b"\r\n").await?;
                session.as_mut().flush().await?;
                sent = true;
            }
            Response::Done {
                tag,
                status,
                code,
                information,
            } if tag == &id => match status {
                Status::Ok if sent => return Ok(()),
                Status::No => {
                    return Err(async_imap::error::Error::No(format!(
                        "code: {code:?}, info: {information:?}"
                    ))
                    .into());
                }
                Status::Bad => {
                    return Err(async_imap::error::Error::Bad(format!(
                        "code: {code:?}, info: {information:?}"
                    ))
                    .into());
                }
                _ => anyhow::bail!("Unexpected APPEND completion: {:?}", response.parsed()),
            },
            Response::Data {
                status: Status::Bye,
                ..
            } => {
                return Err(async_imap::error::Error::ConnectionLost.into());
            }
            Response::Continue { .. } => anyhow::bail!("Unexpected APPEND continuation"),
            _ => {}
        }
    }
}

pub async fn append_copied_message(
    session: &mut Session,
    folder: &str,
    message: &RawMessageCopy,
) -> Result<()> {
    let flags = match (message.seen, message.starred) {
        (true, true) => Some("(\\Seen \\Flagged)"),
        (true, false) => Some("(\\Seen)"),
        (false, true) => Some("(\\Flagged)"),
        (false, false) => None,
    };
    append_message(session, folder, flags, &message.raw)
        .await
        .context("IMAP APPEND copied message")?;
    Ok(())
}

/// Copy sequentially so a failure retains the number of acknowledged APPENDs.
/// The failing APPEND may have landed if its completion response was lost.
pub async fn append_copied_messages(
    session: &mut Session,
    folder: &str,
    messages: &[RawMessageCopy],
) -> Result<()> {
    for (copied, message) in messages.iter().enumerate() {
        append_copied_message(session, folder, message)
            .await
            .with_context(|| {
                format!(
                    "Copy progress: {copied}/{} confirmed; message size: {} bytes",
                    messages.len(),
                    message.raw.len(),
                )
            })?;
    }
    Ok(())
}

/// Permanently delete `uids` from `folder`: mark them `\Deleted`, then UID
/// EXPUNGE so they skip Trash entirely. Used for discarding drafts, which are
/// unsent and ephemeral — moving them to Trash leaves confusing `\Draft` copies.
pub async fn expunge_uids(session: &mut Session, folder: &str, uids: &[u32]) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await.context("SELECT")?;
    let uid_sets = uid_set_chunks(uids, MAX_UID_SET_LEN);
    store_deleted(session, &uid_sets).await?;

    let supports_uidplus = match session.capabilities().await {
        Ok(caps) => caps.has_str("UIDPLUS"),
        Err(_) => false,
    };

    if supports_uidplus {
        uid_expunge(session, &uid_sets).await?;
    } else {
        let estream = session.expunge().await.context("EXPUNGE")?;
        futures::pin_mut!(estream);
        while let Some(item) = estream.next().await {
            item.context("EXPUNGE item")?;
        }
    }
    Ok(())
}

/// Permanently delete every message in `folder`: mark the whole mailbox
/// `\Deleted` by sequence set, then EXPUNGE. Used by "empty trash"/"empty junk",
/// which must clear the server folder itself rather than the UIDs we happen to
/// have cached. Returns the message count reported by SELECT.
pub async fn empty_folder(session: &mut Session, folder: &str) -> Result<u32> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let total = mailbox.exists;
    if total == 0 {
        return Ok(0);
    }
    let mut stream = session
        .store("1:*", "+FLAGS.SILENT (\\Deleted)")
        .await
        .context("STORE Deleted")?;
    while let Some(item) = stream.next().await {
        item.context("STORE item")?;
    }
    drop(stream);

    let estream = session.expunge().await.context("EXPUNGE")?;
    futures::pin_mut!(estream);
    while let Some(item) = estream.next().await {
        item.context("EXPUNGE item")?;
    }
    Ok(total)
}

/// Append a raw RFC822 message to `folder` with the `\Seen` flag, so the user's
/// own sent reply appears in Sent without being marked unread. Used after SMTP
/// send so the message threads back into the conversation on next sync.
pub async fn append_to_sent(session: &mut Session, folder: &str, raw: &[u8]) -> Result<()> {
    append_message(session, folder, Some("(\\Seen)"), raw)
        .await
        .context("IMAP APPEND")?;
    Ok(())
}

/// Append a draft to `folder`, then prune older copies that share the same
/// `message_id` so repeated autosaves replace the draft in place instead of
/// piling up duplicates. `message_id` is the bare id (no angle brackets) that
/// `smtp::build_message` embedded as the Message-ID header. Pruning is
/// best-effort — the APPEND is what matters, so search/expunge failures are
/// logged and swallowed.
pub async fn replace_draft(
    session: &mut Session,
    folder: &str,
    raw: &[u8],
    message_id: &str,
) -> Result<()> {
    append_message(session, folder, Some("(\\Draft \\Seen)"), raw)
        .await
        .context("IMAP APPEND to Drafts")?;
    if message_id.trim().is_empty() {
        return Ok(());
    }
    if let Err(err) = prune_old_drafts(session, folder, message_id).await {
        eprintln!("meron-core: prune old drafts failed message_id={message_id}: {err:#}");
    }
    Ok(())
}

/// Mark every draft in `folder` carrying `message_id` as `\Deleted` except the
/// newest (highest UID) and UID EXPUNGE them. Assumes `folder` holds at most one
/// live copy plus stale duplicates from earlier autosaves.
pub(super) async fn prune_old_drafts(
    session: &mut Session,
    folder: &str,
    message_id: &str,
) -> Result<()> {
    let mut uids = draft_uids(session, folder, message_id).await?;
    if uids.len() <= 1 {
        return Ok(());
    }
    // Keep the newest copy (highest UID); the rest are stale autosaves.
    uids.sort_unstable();
    uids.pop();
    expunge_uids(session, folder, &uids).await
}

/// How many of the newest messages in Drafts `draft_uids` reads when the
/// server's own search comes back empty.
pub(super) const DRAFT_SCAN_LIMIT: u32 = 500;

/// The UIDs of every copy of the draft `message_id` in `folder`, which is left
/// selected. Asks the server first (`SEARCH HEADER`), but an empty or failed
/// answer isn't trusted: some servers don't index Message-ID, match it only as
/// whole words, or lag behind an APPEND, and believing them leaves each sent
/// message's draft in Drafts for good. So that case reads the Message-ID of
/// the folder's newest messages and matches here instead.
pub(super) async fn draft_uids(
    session: &mut Session,
    folder: &str,
    message_id: &str,
) -> Result<Vec<u32>> {
    let mailbox = session.select(folder).await.context("SELECT Drafts")?;
    let criteria = format!("HEADER MESSAGE-ID {}", imap_quote(message_id));
    match session.uid_search(criteria).await {
        Ok(set) if !set.is_empty() => return Ok(set.into_iter().collect()),
        Ok(_) => {}
        Err(err) => eprintln!("meron-core: UID SEARCH draft id failed, scanning {folder}: {err:#}"),
    }
    if mailbox.exists == 0 {
        return Ok(Vec::new());
    }
    let first = mailbox.exists.saturating_sub(DRAFT_SCAN_LIMIT - 1).max(1);
    let mut stream = session
        .fetch(
            format!("{first}:*"),
            "(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)])",
        )
        .await
        .context("FETCH draft ids")?;
    let mut uids = Vec::new();
    while let Some(item) = stream.next().await {
        let fetch = item.context("FETCH draft id item")?;
        let Some(uid) = fetch.uid else {
            continue;
        };
        if fetch
            .header()
            .is_some_and(|header| header_is_message(header, message_id))
        {
            uids.push(uid);
        }
    }
    Ok(uids)
}

/// Whether the header block `header` carries `message_id` (bare, no angle
/// brackets) as its Message-ID.
pub(super) fn header_is_message(header: &[u8], message_id: &str) -> bool {
    let wanted = message_id.trim().trim_matches(['<', '>']);
    if wanted.is_empty() {
        return false;
    }
    mailparse::parse_headers(header)
        .ok()
        .and_then(|(headers, _)| headers.get_first_value("Message-ID"))
        .is_some_and(|value| {
            value
                .trim()
                .trim_matches(['<', '>'])
                .eq_ignore_ascii_case(wanted)
        })
}

/// Permanently remove every draft in `folder` carrying `message_id`. Used after
/// the message is sent, because the saved Drafts copy is no longer useful.
pub async fn discard_draft(session: &mut Session, folder: &str, message_id: &str) -> Result<usize> {
    if message_id.trim().is_empty() {
        return Ok(0);
    }
    let uids = draft_uids(session, folder, message_id).await?;
    if uids.is_empty() {
        return Ok(0);
    }
    let deleted = uids.len();
    expunge_uids(session, folder, &uids).await?;
    Ok(deleted)
}

/// MCP must never fall back to folder-wide EXPUNGE: unrelated messages could
/// already carry \Deleted. Validate the mailbox generation before touching flags.
pub async fn expunge_uids_checked(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
    validity: u32,
) -> Result<()> {
    let caps = session.capabilities().await.context("CAPABILITY")?;
    anyhow::ensure!(
        caps.has_str("UIDPLUS"),
        "This operation requires IMAP UIDPLUS; no messages were changed"
    );
    let selected = session.select(folder).await.context("SELECT")?;
    anyhow::ensure!(
        validity != 0 && selected.uid_validity == Some(validity),
        "Mailbox changed; request a new approval"
    );
    expunge_selected_uids(session, uids).await
}

pub(super) async fn expunge_selected_uids(session: &mut Session, uids: &[u32]) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let uid_sets = uid_set_chunks(uids, MAX_UID_SET_LEN);
    store_deleted(session, &uid_sets).await?;
    uid_expunge(session, &uid_sets).await
}

/// Flag every UID in `uid_sets` `\Deleted` in the selected mailbox.
pub(super) async fn store_deleted(session: &mut Session, uid_sets: &[String]) -> Result<()> {
    for uid_set in uid_sets {
        let mut stream = session
            .uid_store(uid_set, "+FLAGS.SILENT (\\Deleted)")
            .await
            .context("UID STORE Deleted")?;
        while let Some(item) = stream.next().await {
            item.context("UID STORE item")?;
        }
    }
    Ok(())
}

/// UID EXPUNGE (RFC 4315) exactly the UIDs in `uid_sets`, leaving any other
/// `\Deleted` message in the mailbox alone.
pub(super) async fn uid_expunge(session: &mut Session, uid_sets: &[String]) -> Result<()> {
    for uid_set in uid_sets {
        let stream = session.uid_expunge(uid_set).await.context("UID EXPUNGE")?;
        futures::pin_mut!(stream);
        while let Some(item) = stream.next().await {
            item.context("UID EXPUNGE item")?;
        }
    }
    Ok(())
}

/// Move-only behavior for MCP organization, including messages in Drafts/Trash.
pub async fn move_to_folder_checked(
    session: &mut Session,
    folder: &str,
    target: &str,
    uids: &[u32],
) -> Result<()> {
    let caps = session.capabilities().await.context("CAPABILITY")?;
    let supports_move = caps.has_str("MOVE");
    anyhow::ensure!(
        supports_move || caps.has_str("UIDPLUS"),
        "Moving through MCP requires IMAP MOVE or UIDPLUS; no messages were changed"
    );
    session.select(folder).await.context("SELECT")?;
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        if supports_move {
            session.uid_mv(&uid_set, target).await.context("UID MOVE")?;
        } else {
            session
                .uid_copy(&uid_set, target)
                .await
                .context("UID COPY")?;
        }
    }
    if !supports_move {
        expunge_selected_uids(session, uids).await?;
    }
    Ok(())
}
