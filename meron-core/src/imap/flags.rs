use super::*;

/// Read-state changes reconciled via CONDSTORE: which messages flipped \Seen or \Flagged (starred),
/// plus the folder's current HIGHESTMODSEQ to persist for the next incremental
/// fetch.
pub struct FlagSync {
    pub highest_modseq: u64,
    pub changes: Vec<(u32, bool, bool, Vec<String>)>, // (uid, seen, starred, tags)
    /// Every message's tags, when the folder still needed that first full read.
    pub tag_baseline: Option<Vec<(u32, Vec<String>)>>,
}

/// Reconcile \Seen and \Flagged across an entire folder using CONDSTORE so updates made
/// on other devices are reflected even for messages older than the recent
/// window. Selects with `(CONDSTORE)` to learn HIGHESTMODSEQ, then fetches only
/// the messages whose flags changed since `since_modseq` (CHANGEDSINCE).
///
/// Returns no changes (but still reports HIGHESTMODSEQ) when there's no prior
/// baseline or UIDVALIDITY changed, and is a no-op on servers that do not
/// advertise CONDSTORE — the capability must be checked rather than probed.
/// Proton Mail Bridge answers `SELECT "INBOX" (CONDSTORE)` with an *untagged*
/// `BAD` (" BAD [Error offset=17]: expected CR"), so the client never matches a
/// response to its tag: instead of erroring, every later command on that
/// connection reads the previous command's reply. The desynced session then
/// reports empty mailboxes and an empty folder LIST while still looking
/// healthy, so it goes back into the pool and poisons the account's syncs.
pub async fn sync_flags(
    session: &mut Session,
    folder: &str,
    since_modseq: u64,
    validity_matches: bool,
) -> Result<FlagSync> {
    if !supports_condstore(session).await {
        session.select(folder).await.context("SELECT")?;
        return Ok(FlagSync {
            highest_modseq: 0,
            changes: Vec::new(),
            tag_baseline: None,
        });
    }
    let mailbox = session
        .select_condstore(folder)
        .await
        .context("SELECT CONDSTORE")?;
    let highest_modseq = mailbox.highest_modseq.unwrap_or(0);

    if since_modseq == 0
        || !validity_matches
        || highest_modseq == 0
        || highest_modseq <= since_modseq
    {
        return Ok(FlagSync {
            highest_modseq,
            changes: Vec::new(),
            tag_baseline: None,
        });
    }

    let query = format!("(FLAGS) (CHANGEDSINCE {since_modseq})");
    let mut stream = session
        .uid_fetch("1:*", query)
        .await
        .context("UID FETCH CHANGEDSINCE")?;
    let mut changes = Vec::new();
    while let Some(item) = stream.next().await {
        let fetch = item.context("flag fetch item")?;
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
        let tags = fetch_tags(&fetch);
        changes.push((uid, seen, starred, tags));
    }
    drop(stream);
    Ok(FlagSync {
        highest_modseq,
        changes,
        tag_baseline: None,
    })
}

/// Every message's tags in the selected mailbox. CONDSTORE only reports changes,
/// so mail cached before tags existed has to be read once in full.
pub async fn fetch_all_tags(session: &mut Session) -> Result<Vec<(u32, Vec<String>)>> {
    let mut stream = session
        .uid_fetch("1:*", "(FLAGS)")
        .await
        .context("UID FETCH tag baseline")?;
    let mut tags = Vec::new();
    while let Some(item) = stream.next().await {
        let fetch = item.context("tag baseline item")?;
        if let Some(uid) = fetch.uid {
            tags.push((uid, fetch_tags(&fetch)));
        }
    }
    Ok(tags)
}

/// SELECT `folder` as the preflight of a flag update. Pooled sessions die
/// silently, and the SELECT is the first command that notices, so running it
/// as a separate phase lets a stale connection be replaced before any STORE
/// reaches the server (see `Engine::with_flag_write_session`).
pub async fn prepare_flag_update(session: &mut Session, folder: &str) -> Result<()> {
    session.select(folder).await.context("SELECT")?;
    Ok(())
}

/// Select `folder` for a write of `tag`'s keyword, and say whether it can hold
/// it: it must allow new keywords, or already list this one as permanent.
pub async fn prepare_tag_update(session: &mut Session, folder: &str, tag: &str) -> Result<bool> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let keyword = crate::tags::tag_to_keyword(tag);
    Ok(mailbox.permanent_flags.iter().any(|flag| match flag {
        async_imap::types::Flag::MayCreate => true,
        async_imap::types::Flag::Custom(known) => known.eq_ignore_ascii_case(&keyword),
        _ => false,
    }))
}

/// The tags among a FETCH response's keywords, sorted.
pub(super) fn fetch_tags(fetch: &async_imap::types::Fetch) -> Vec<String> {
    let mut tags: Vec<String> = fetch
        .flags()
        .filter_map(|flag| match flag {
            async_imap::types::Flag::Custom(keyword) => crate::tags::keyword_to_tag(&keyword),
            _ => None,
        })
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

/// Longest UID set sent in one command. RFC 7162 asks clients to keep command
/// lines under 8192 octets; this leaves room for the tag and the flag list.
pub(super) const MAX_UID_SET_LEN: usize = 7000;

/// Compact `uids` into IMAP sequence sets (`1:5,9,12:20`), split so no set is
/// longer than `max_len`. A folder-wide "mark all read" can cover tens of
/// thousands of UIDs, which as a flat list overruns server line limits.
pub(super) fn uid_set_chunks(uids: &[u32], max_len: usize) -> Vec<String> {
    let mut sorted = uids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while i < sorted.len() {
        let start = sorted[i];
        let mut end = start;
        while i + 1 < sorted.len() && sorted[i + 1] == end + 1 {
            i += 1;
            end = sorted[i];
        }
        i += 1;
        let range = if start == end {
            start.to_string()
        } else {
            format!("{start}:{end}")
        };
        if !current.is_empty() && current.len() + 1 + range.len() > max_len {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(',');
        }
        current.push_str(&range);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

pub(super) async fn store_flag(session: &mut Session, uids: &[u32], op: &str) -> Result<()> {
    for uid_set in uid_set_chunks(uids, MAX_UID_SET_LEN) {
        let mut stream = session
            .uid_store(uid_set, op)
            .await
            .context("UID STORE FLAGS.SILENT")?;
        while let Some(item) = stream.next().await {
            item.context("UID STORE item")?;
        }
    }
    Ok(())
}

/// Add or remove a tag's keyword on a set of UIDs already in the selected
/// mailbox. The caller must first run [`prepare_tag_update`] on the session.
pub async fn store_tag(session: &mut Session, uids: &[u32], tag: &str, add: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let sign = if add { '+' } else { '-' };
    let keyword = crate::tags::tag_to_keyword(tag);
    store_flag(session, uids, &format!("{sign}FLAGS.SILENT ({keyword})")).await
}

/// Add or remove the `\Seen` flag on a set of UIDs already in the selected
/// mailbox. The caller must first run [`prepare_flag_update`] on the session.
pub async fn store_seen(session: &mut Session, uids: &[u32], seen: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let op = if seen {
        "+FLAGS.SILENT (\\Seen)"
    } else {
        "-FLAGS.SILENT (\\Seen)"
    };
    store_flag(session, uids, op).await
}

/// Add or remove the `\Flagged` flag on a set of UIDs already in the selected
/// mailbox. The caller must first run [`prepare_flag_update`] on the session.
pub async fn store_starred(session: &mut Session, uids: &[u32], starred: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let op = if starred {
        "+FLAGS.SILENT (\\Flagged)"
    } else {
        "-FLAGS.SILENT (\\Flagged)"
    };
    store_flag(session, uids, op).await
}
