//! APPENDing sent copies and drafts, and waiting for the sent copy to land.

use rusqlite::Connection;
use std::sync::Arc;
use std::time::Duration;

use crate::{imap, parse, store};

use super::*;

pub fn should_append_sent_copy(
    auth_type: &str,
    smtp_host: &str,
    override_pref: Option<bool>,
) -> bool {
    override_pref.unwrap_or_else(|| {
        let host = smtp_host.trim_end_matches('.').to_ascii_lowercase();
        let provider_saves_sent = matches!(auth_type, "gmail_oauth" | "outlook_oauth")
            || matches!(
                host.as_str(),
                "smtp.gmail.com"
                    | "smtp.googlemail.com"
                    | "smtp-mail.outlook.com"
                    | "smtp.office365.com"
                    | "smtp.live.com"
                    | "smtp.hotmail.com"
            );
        !provider_saves_sent
    })
}

/// The `mail.sentCopyCached` detail a host emits once the Sent copy of a message
/// the user just sent is in the local cache — see [`SentCopy`].
///
/// Caching that copy changes what the thread list shows for the conversation:
/// a card's message count spans the account's folders, so the reply is one more
/// message behind the row. Nothing else announces it — the IDLE loop watches
/// INBOX, where a reply the user sent usually never lands — so the list would
/// keep its stale count until the user navigated away and back.
///
/// It is its own event rather than a folderless `mail.synced` because it is not
/// a mailbox sync finishing: the row that changed sits in whichever mailbox
/// holds the conversation (the inbox, an archive, a label), which the sender
/// does not know, so this says "re-read this account's cache" and names no
/// folder. Consumers that key off a synced folder would either drop it (as
/// unrelated to the mailbox on screen) or, worse, mistake it for the completion
/// of a folder sync still in flight. Nor does it carry new-mail semantics: no
/// notification or "new mail" toast fires for the user's own reply.
///
/// Both hosts emit it for every cached copy, immediate or awaited. The mobile
/// hosts do reload the thread list themselves once `mail.send` returns, which
/// covers a copy cached inside that call — but only for the mailbox and the
/// open thread, not for the Kanban board's columns, which read the same counts
/// and would otherwise stay a message behind.
pub fn sent_copy_cached_detail(account: &str) -> serde_json::Value {
    serde_json::json!({ "account": account })
}

/// What [`append_to_sent`] left in the cache for a message the user just sent.
///
/// Some copy is always expected: either Meron APPENDs one, or the account is
/// set to leave it to a provider that files its own — which is what turning
/// `save_sent_copy` off means, rather than "no Sent copy will ever exist".
pub struct SentCopy {
    /// The account's Sent folder.
    pub folder: String,
    /// The envelope of the message that was sent, for recognising the server's
    /// copy of it in the folder — see [`sent_copy_landed`].
    pub envelope: parse::SentEnvelope,
    /// Whether the copy is cached *now*. A provider that files its own can
    /// expose it over IMAP seconds after SMTP returns, so `false` does not mean
    /// it was lost — [`await_sent_copy`] waits it out.
    pub observed: bool,
}

impl SentCopy {
    /// Whether [`await_sent_copy`] could still turn this into a cached copy.
    pub fn worth_awaiting(&self) -> bool {
        !self.observed
    }
}

/// Gaps between re-reads of the Sent folder while waiting for a provider to
/// expose its own copy. Bounded on purpose: an ordinary sync of the folder
/// picks up whatever is still missing, and a send must not leave a session
/// polling for it forever.
pub(super) const SENT_COPY_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];

/// Keep re-reading the Sent folder until `copy` shows up there. True once it is
/// cached, false once the attempts run out.
pub async fn await_sent_copy(engine: &Arc<Engine>, account: &str, copy: &SentCopy) -> bool {
    for delay in SENT_COPY_RETRY_DELAYS {
        tokio::time::sleep(delay).await;
        match refresh_sent_copy(engine, account, copy).await {
            Ok(true) => return true,
            Ok(false) => {}
            // A refresh that fails (a dropped session, a network blip) has not
            // ruled the copy out, so the remaining attempts still run.
            Err(err) => eprintln!("meron-core: Sent refresh for {account}: {err:#}"),
        }
    }
    false
}

/// Re-read the tail of the Sent folder into the cache, reporting whether the
/// copy is now there.
pub(super) async fn refresh_sent_copy(
    engine: &Arc<Engine>,
    account: &str,
    copy: &SentCopy,
) -> anyhow::Result<bool> {
    let folder = copy.folder.as_str();
    let batch = fetch_recent_resilient(engine, account, folder, 20).await?;
    let db = engine.db.lock().unwrap();
    store_folder_tail(&db, account, folder, &batch)?;
    Ok(batch
        .messages
        .iter()
        .any(|candidate| sent_copy_landed(&copy.envelope, candidate)))
}

/// Persist a partial re-read of a folder — the tail reads that follow a send or
/// a draft save, rather than a full sync of it.
///
/// It has to answer a UIDVALIDITY change itself. Recording the new validity
/// while leaving the previous generation's rows in place would make the next
/// ordinary sync see a validity that already matches its own and skip the reset
/// it would otherwise do: the two UID generations would stay mixed in the cache
/// for good, and flag reconciliation would keep asking for changes since a
/// modseq that belongs to a mailbox the server no longer has. Dropping the rows
/// and the modseq here leaves the folder exactly as a first sync finds it.
pub(super) fn store_folder_tail(
    db: &Connection,
    account: &str,
    folder: &str,
    batch: &imap::RecentBatch,
) -> anyhow::Result<()> {
    let prior_validity = store::get_folder_state(db, account, folder)?
        .map(|(validity, _)| validity)
        .unwrap_or(0);
    if prior_validity != 0 && prior_validity != batch.uidvalidity {
        store::clear_folder_messages(db, account, folder)?;
        store::set_folder_modseq(db, account, folder, 0)?;
    }
    store::upsert_messages(db, account, folder, &batch.messages)?;
    store::set_folder_state(db, account, folder, batch.uidvalidity, batch.uid_next)?;
    Ok(())
}

/// How far the copy's `Date` may sit from the one we sent and still be the same
/// message: enough for a slow submission plus modest clock skew, short enough
/// not to swallow a genuinely later reply into the same conversation.
pub(super) const SENT_COPY_MATCH_WINDOW_SECS: i64 = 600;

/// Whether `candidate`, read out of the Sent folder, is the server's copy of the
/// message described by `sent`.
///
/// The id we sent answers directly when it comes back, but it does not always:
/// Proton Bridge replaces it with an id of its own (`@protonmail.internalid`),
/// which is why the reader pairs optimistic bubbles to server copies by
/// envelope instead. The same rule applies here — same sender, same subject,
/// same recipients, sent at about the same moment.
///
/// A folder-growth test would be cheaper, but it cannot tell this send's copy
/// from anything else the folder gained: another client's send in the same
/// seconds would end the wait early, leaving the copy the user is waiting on
/// unannounced, and a folder with no prior state offers no baseline at all —
/// which for a rewritten id meant the copy could be cached and never reported.
pub(super) fn sent_copy_landed(
    sent: &parse::SentEnvelope,
    candidate: &imap::MessageHeader,
) -> bool {
    if !sent.message_id.is_empty() && candidate.message_id.eq_ignore_ascii_case(&sent.message_id) {
        return true;
    }
    if !candidate.from_addr.eq_ignore_ascii_case(&sent.from_addr)
        || candidate.subject.trim() != sent.subject.trim()
        || candidate_recipients(candidate) != sent.recipients
    {
        return false;
    }
    // Both dates are needed: a message whose Date could not be read (0) would
    // otherwise sit a lifetime away from every send, or match one exactly.
    sent.date != 0
        && candidate.date != 0
        && (candidate.date - sent.date).abs() <= SENT_COPY_MATCH_WINDOW_SECS
}

/// A cached message's To + Cc, normalized the way [`parse::SentEnvelope`] holds
/// the addresses it was sent to, so the two compare directly.
pub(super) fn candidate_recipients(candidate: &imap::MessageHeader) -> Vec<String> {
    parse::normalize_addresses(
        candidate
            .to
            .iter()
            .chain(candidate.cc.iter())
            .map(|recipient| recipient.addr.clone())
            .collect(),
    )
}

pub async fn append_to_sent(
    engine: &Arc<Engine>,
    account: &str,
    raw: &[u8],
) -> anyhow::Result<SentCopy> {
    let (auth_type, smtp_host, override_pref) = {
        let (auth_type, smtp_host) = engine
            .accounts
            .lock()
            .await
            .get(account)
            .map(|creds| (creds.auth_type.clone(), creds.smtp_host.clone()))
            .unwrap_or_else(|| ("password".to_string(), String::new()));
        let override_pref = store::save_sent_copy_pref(&engine.db.lock().unwrap(), account)?;
        (auth_type, smtp_host, override_pref)
    };
    let should_append = should_append_sent_copy(&auth_type, &smtp_host, override_pref);

    // APPEND is mutating, so this never auto-retries (a drop after the server
    // accepted the message must not re-APPEND a duplicate copy).
    let sent = engine
        .with_write_session(account, |session| {
            let raw = raw.to_vec();
            Box::pin(async move {
                let sent = imap::find_sent_folder(session)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("no Sent folder found"))?;
                if should_append {
                    imap::append_to_sent(session, &sent, &raw).await?;
                }
                anyhow::Ok(sent)
            })
        })
        .await?;
    // Refresh local Sent-folder envelopes so the new row is queryable by the
    // cross-folder thread view immediately. For Gmail/Outlook defaults, this
    // picks up the provider-created Sent copy instead of uploading a duplicate.
    // Read-only, so it runs on its own session: the APPEND above has already
    // landed and must not be retried alongside it.
    //
    // Our own APPEND is on the server before this reads, so it comes back in
    // this pass; a provider-filed copy may not exist yet, which is what
    // `observed` reports and [`await_sent_copy`] waits out.
    let mut copy = SentCopy {
        folder: sent,
        envelope: parse::sent_envelope_of(raw),
        observed: false,
    };
    // A refresh that fails is not a failed send, and must not be reported as
    // one: the message is on its way and the copy is merely unobserved, which
    // is exactly the state the caller's watcher is for.
    match refresh_sent_copy(engine, account, &copy).await {
        Ok(observed) => copy.observed = observed,
        Err(err) => eprintln!("meron-core: Sent refresh for {account}: {err:#}"),
    }

    Ok(copy)
}

pub async fn append_to_drafts(
    engine: &Arc<Engine>,
    account: &str,
    raw: &[u8],
    message_id: &str,
) -> anyhow::Result<()> {
    // replace_draft APPENDs the new draft and expunges the prior copy; mutating,
    // so it never auto-retries. Finding the folder is a plain LIST, and a pooled
    // session the server has dropped fails on it before anything is written, so
    // it runs as the retryable preflight — an autosave used to surface that dead
    // socket to the user as "Draft autosave failed: LIST: Broken pipe".
    let drafts_slot: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
    let drafts = engine
        .with_preflighted_write_session(
            account,
            |session| {
                let slot = Arc::clone(&drafts_slot);
                Box::pin(async move {
                    let drafts = imap::find_drafts_folder(session)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                    *slot.lock().unwrap() = Some(drafts);
                    anyhow::Ok(())
                })
            },
            |session| {
                let raw = raw.to_vec();
                let message_id = message_id.to_string();
                let slot = Arc::clone(&drafts_slot);
                Box::pin(async move {
                    let drafts = { slot.lock().unwrap().clone() }
                        .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                    imap::replace_draft(session, &drafts, &raw, &message_id).await?;
                    anyhow::Ok(drafts)
                })
            },
        )
        .await?;
    // Refresh local Drafts envelopes so an autosaved reply appears in the
    // existing cross-folder thread view immediately. Read-only, and the draft
    // is already written, so it runs on its own session — and a refresh that
    // fails must not report the save as failed: the caller would drop the id
    // it just wrote under, and nothing would ever discard that copy.
    let batch = match fetch_recent_resilient(engine, account, &drafts, 20).await {
        Ok(batch) => batch,
        Err(err) => {
            eprintln!("meron-core: Drafts refresh for {account}: {err:#}");
            return Ok(());
        }
    };
    {
        let db = engine.db.lock().unwrap();
        store_folder_tail(&db, account, &drafts, &batch)?;
        // replace_draft expunged the prior server copy, but its locally cached
        // row (older UID) survives the upsert; drop every copy of this draft
        // except the one the batch just brought in, or the thread view shows
        // duplicate draft bubbles until the next full Drafts sync.
        let keep_uid = batch
            .messages
            .iter()
            .filter(|m| m.message_id.eq_ignore_ascii_case(message_id))
            .map(|m| m.uid)
            .max();
        if keep_uid.is_some() {
            store::delete_draft_copies(&db, account, &drafts, message_id, keep_uid)?;
        }
    }
    Ok(())
}
