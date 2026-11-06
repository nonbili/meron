//! Reading cached messages and filling missing thread ancestors.

use std::collections::HashSet;
use std::sync::Arc;

use crate::{imap, parse, store};

use super::*;

pub(super) fn push_unique_folder(folders: &mut Vec<String>, folder: Option<String>) {
    if let Some(folder) = folder
        && !folders
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&folder))
    {
        folders.push(folder);
    }
}

pub(super) fn thread_gap_search_folders(
    sent: Option<String>,
    drafts: Option<String>,
    archive: Option<String>,
) -> Vec<String> {
    let mut folders = vec!["INBOX".to_string()];
    push_unique_folder(&mut folders, sent);
    push_unique_folder(&mut folders, drafts);
    push_unique_folder(&mut folders, archive);
    folders
}

/// Fetch a thread's referenced-but-unsynced ancestor messages from the server
/// and cache them, so the reader shows the whole conversation rather than the
/// locally-synced tail. No-op when nothing is missing (the common case), so a
/// normal thread open pays no network cost. Searches INBOX and Sent first —
/// where the recent-sync assigns stable UIDs, so re-finding an already-cached
/// message updates it in place rather than duplicating it — then Drafts for
/// remote saved replies, then the all-mail / archive folder for messages that
/// live only there.
/// Fetch any referenced-but-missing ancestor messages for a thread over IMAP and
/// persist them. Returns `true` if at least one message was newly stored, so the
/// caller can refresh the open thread. Opens a network connection — call it off
/// the read path (see [`maybe_spawn_fill_thread_gaps`]).
pub async fn fill_thread_gaps(
    engine: &Arc<Engine>,
    account: &str,
    thread_key: &str,
) -> anyhow::Result<bool> {
    let remaining = {
        let db = engine.db.lock().unwrap();
        store::get_thread_reference_gaps(&db, account, thread_key)?
    };
    if remaining.is_empty() {
        return Ok(false);
    }

    let _snapshot_guard = MessageSyncGuard::begin(engine)?;

    // IMAP-read-only (SEARCH + FETCH); upserts are idempotent, so the whole loop
    // is safe to retry on a stale pooled connection. `remaining` is cloned per
    // invocation so a retry starts from the full gap set.
    engine
        .with_read_session(account, |session| {
            let engine = engine.clone();
            let account = account.to_string();
            let mut remaining = remaining.clone();
            Box::pin(async move {
                let mut persisted = false;
                let sent = imap::find_sent_folder(session).await.ok().flatten();
                let drafts = imap::find_drafts_folder(session).await.ok().flatten();
                let archive = imap::find_archive_folder(session).await.ok().flatten();
                let folders = thread_gap_search_folders(sent, drafts, archive);

                let media_root = parse::media_root();
                for folder in folders {
                    if remaining.is_empty() {
                        break;
                    }
                    let found = match imap::fetch_by_message_ids(
                        session,
                        &folder,
                        &remaining,
                        &media_root,
                        &account,
                    )
                    .await
                    {
                        Ok(found) => found,
                        Err(err) => {
                            eprintln!("meron-core: fetch_by_message_ids folder={folder}: {err:#}");
                            continue;
                        }
                    };
                    if found.is_empty() {
                        continue;
                    }
                    let mut found_ids: HashSet<String> = HashSet::new();
                    {
                        let db = engine.db.lock().unwrap();
                        for fm in &found {
                            // Header row first (writes thread_key etc.), then the
                            // body row (writes body/json) — both keyed on
                            // (account, folder, uid).
                            let _ = store::upsert_messages(
                                &db,
                                &account,
                                &folder,
                                std::slice::from_ref(&fm.header),
                            );
                            let _ = store::save_cached_message(
                                &db,
                                &account,
                                &folder,
                                fm.header.uid,
                                &fm.message,
                            );
                            persisted = true;
                            let mid = fm.message.message_id.trim().to_ascii_lowercase();
                            if !mid.is_empty() {
                                found_ids.insert(mid);
                            }
                        }
                    }
                    remaining.retain(|id| !found_ids.contains(id));
                }
                anyhow::Ok(persisted)
            })
        })
        .await
}

/// Cached-only variant of [`read_cached_or_fetch`]: returns the message when
/// the local cache can serve it in full — body row present, threading headers
/// extracted, inline media on disk — and `None` otherwise. Never touches the
/// network, so the thread view can answer instantly from cache and leave
/// misses to a background fetch.
pub fn read_cached(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uid: u32,
) -> Option<parse::Message> {
    let media_root = parse::media_root();
    let (cached, remote_policy) = {
        let db = engine.db.lock().unwrap();
        let cached = store::get_cached_message(&db, account, folder, uid);
        let policy = store::remote_image_policy(&db, account).unwrap_or_default();
        (cached, policy)
    };
    let mut msg = cached.ok().flatten()?;
    // Rows cached before the threading-header extraction landed have an
    // empty `message_id`; refetch them so reply_to/cc/references populate.
    // Real-world mail almost always carries a Message-ID, so emptiness is a
    // reliable "pre-extraction cache" signal.
    if msg.message_id.is_empty() || !parse::cached_media_available(&media_root, &msg) {
        return None;
    }
    attach_html(&mut msg, &remote_policy);
    Some(msg)
}

pub async fn read_cached_or_fetch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uid: u32,
) -> anyhow::Result<parse::Message> {
    read_cached_or_fetch_with_budget(engine, account, folder, uid, INLINE_BODY_BUDGET).await
}

async fn read_cached_or_fetch_with_budget(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uid: u32,
    budget: std::time::Duration,
) -> anyhow::Result<parse::Message> {
    if let Some(msg) = read_cached(engine, account, folder, uid) {
        return Ok(msg);
    }
    let media_root = parse::media_root();
    let remote_policy = {
        let db = engine.db.lock().unwrap();
        store::remote_image_policy(&db, account).unwrap_or_default()
    };

    // A background fill already has this message: wait for it instead of
    // downloading the same body beside it. The wait comes out of the same
    // budget as the download below, which is for the whole call.
    let started = tokio::time::Instant::now();
    let key = message_key(account, folder, uid);
    let filled = tokio::time::timeout(budget, async {
        loop {
            let done = engine.body_fetch_done.notified();
            tokio::pin!(done);
            done.as_mut().enable();
            if !engine.body_fetches.lock().unwrap().contains(&key) {
                break;
            }
            done.await;
        }
    })
    .await;
    if filled.is_err() {
        anyhow::bail!("message body download is still in progress");
    }
    if let Some(msg) = read_cached(engine, account, folder, uid) {
        return Ok(msg);
    }
    // This read asks the server itself, so what a fill found earlier is moot.
    engine.take_body_failure(&key);
    let _snapshot_guard = MessageSyncGuard::begin(engine)?;
    let wanted = std::collections::BTreeMap::from([(folder.to_string(), vec![uid])]);
    let fetched = fetch_bodies_keeping_partial(
        engine,
        account,
        &wanted,
        &media_root,
        TransferLimits::within(budget.saturating_sub(started.elapsed())),
    )
    .await;
    let mut message = match fetched
        .messages
        .into_iter()
        .find(|(f, id, _)| f == folder && *id == uid)
    {
        Some((_, _, message)) => message,
        None => {
            let error = fetched
                .result
                .err()
                .unwrap_or_else(|| anyhow::anyhow!("message uid {uid} not found in {folder}"));
            // Only a download that ran out of time is helped by the longer
            // background allowance; the next read waits on it.
            if error.is::<TransferTimedOut>() {
                crate::thread_read::spawn_fill_thread_bodies(
                    engine,
                    account,
                    vec![(folder.to_string(), uid)],
                    media_root,
                    None,
                );
            }
            return Err(error);
        }
    };

    attach_html(&mut message, &remote_policy);
    Ok(message)
}

/// How long an on-demand body download may take, reconnect included. Inside
/// the desktop bridge's 30s budget, and long enough that a slow server is not
/// abandoned and fetched a second time.
pub const INLINE_BODY_BUDGET: std::time::Duration = std::time::Duration::from_secs(25);

/// Turn the stored HTML source into the iframe-ready `body_html` the reader's HTML
/// mode renders: inject the remote-image CSP, allowed when the account loads
/// remote content or the user allowed this message's sender. Plain messages have
/// no HTML source, so this is a no-op for them.
pub fn attach_html(message: &mut parse::Message, policy: &store::RemoteImagePolicy) {
    // Decrypted mail never loads remote content, whatever the account or
    // sender setting (EFAIL; Thunderbird removed the option too).
    let encrypted = message.crypto.as_ref().is_some_and(|c| c.encrypted);
    let allowed = !encrypted && policy.allows(&message.from_addr);
    if let Some(html) = message.body_html.take() {
        message.body_html = Some(parse::prepare_message_html(
            &html,
            allowed,
            &message.subject,
        ));
    }
}

#[cfg(test)]
mod body_read_tests {
    use super::*;
    #[tokio::test]
    async fn timed_out_single_read_finishes_in_background_without_duplicate_retries() {
        let (engine, server) = super::super::prefetch::transfer_clock_tests::slow_fetch_engine(
            2,
            std::time::Duration::from_millis(25),
            false,
        )
        .await;
        let err = read_cached_or_fetch_with_budget(
            &engine,
            "acc",
            "INBOX",
            1,
            std::time::Duration::from_millis(100),
        )
        .await
        .unwrap_err();
        assert!(err.is::<TransferTimedOut>());
        assert!(
            engine
                .body_fetches
                .lock()
                .unwrap()
                .contains(&message_key("acc", "INBOX", 1))
        );
        // Too short to see the fill through: reported, not fetched again.
        let err = read_cached_or_fetch_with_budget(
            &engine,
            "acc",
            "INBOX",
            1,
            std::time::Duration::from_millis(10),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("still in progress"));
        // The next read waits for the fill and returns what it cached.
        let message = read_cached_or_fetch_with_budget(
            &engine,
            "acc",
            "INBOX",
            1,
            std::time::Duration::from_secs(3),
        )
        .await
        .unwrap();
        assert_eq!(message.body.trim(), "Slow body");
        server.await.unwrap();
    }
}
