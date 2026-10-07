//! Folder and message sync against the server, including companion folders.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::{imap, store};

use super::*;

/// Reconnect to IMAP, list folders, and refresh the store.
pub async fn sync_folders(
    engine: &Arc<Engine>,
    account: &str,
) -> anyhow::Result<Vec<imap::Folder>> {
    let folders = engine
        .with_read_session(account, |session| {
            Box::pin(async move { imap::list_folders(session).await })
        })
        .await?;
    crate::mlog!(
        crate::log::Level::Debug,
        "mail.sync",
        "folders account={account}: {} listed [{}]",
        folders.len(),
        folders
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let db = engine.db.lock().unwrap();
    store::upsert_folders(&db, account, &folders)?;
    Ok(folders)
}

/// Update every physical copy selected by a star action. Commit each mailbox to
/// the cache only after its server write succeeds, including on partial failure.
pub async fn mark_starred_copies(
    engine: &Engine,
    account: &str,
    targets: &std::collections::BTreeMap<String, Vec<u32>>,
    starred: bool,
) -> anyhow::Result<()> {
    for (folder, uids) in targets {
        engine
            .with_flag_write_session(
                account,
                |session| {
                    let folder = folder.clone();
                    Box::pin(async move { imap::prepare_flag_update(session, &folder).await })
                },
                |session| {
                    let uids = uids.clone();
                    Box::pin(async move { imap::store_starred(session, &uids, starred).await })
                },
            )
            .await?;
        let db = engine.db.lock().unwrap();
        for uid in uids {
            store::update_message_starred(&db, account, folder, *uid, starred)?;
        }
    }
    Ok(())
}

/// Where [`delete_to_trash`] moved messages, and the Trash folder's UIDNEXT
/// from just before the move (see [`refresh_moved_copies`]).
pub struct Trashed {
    pub folder: String,
    pub uid_next_before: Option<u32>,
}

/// Delete messages on the server. Returns `None` for a permanent expunge
/// (Drafts, or items already in Trash), or `Some` when moved to Trash.
pub async fn delete_to_trash(
    engine: &Engine,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> anyhow::Result<Option<Trashed>> {
    engine
        .with_write_session(account, |session| {
            let folder = folder.to_string();
            let uids = uids.to_vec();
            Box::pin(async move {
                let drafts = imap::find_drafts_folder(session).await?;
                if drafts.as_deref() == Some(folder.as_str()) {
                    imap::expunge_uids(session, &folder, &uids).await?;
                    return anyhow::Ok(None);
                }
                let trash = imap::find_trash_folder(session)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Trash folder not found for this account"))?;
                if trash == folder {
                    imap::expunge_uids(session, &folder, &uids).await?;
                    return anyhow::Ok(None);
                }
                let uid_next_before = imap::move_to_folder(session, &folder, &trash, &uids).await?;
                anyhow::Ok(Some(Trashed {
                    folder: trash,
                    uid_next_before,
                }))
            })
        })
        .await
}

/// The target folder's recent window, re-read after a move into it, and the
/// UIDs of the copies that move created there.
pub struct MovedCopies {
    /// `None` when the re-read failed; the move itself still stands.
    pub batch: Option<imap::RecentBatch>,
    pub uids: Vec<u32>,
    _snapshot_guard: Option<MessageSyncGuard>,
}

impl MovedCopies {
    /// Cache the re-read target window, if there is one.
    pub fn store(
        &self,
        conn: &rusqlite::Connection,
        account: &str,
        target: &str,
    ) -> anyhow::Result<()> {
        let Some(batch) = &self.batch else {
            return Ok(());
        };
        store::ensure_folder(conn, account, target)?;
        store::upsert_messages(conn, account, target, &batch.messages)?;
        store::set_folder_state(conn, account, target, batch.uidvalidity, batch.uid_next)?;
        Ok(())
    }
}

/// The lowercased Message-ID of each message about to be moved, `""` for one
/// without, so [`refresh_moved_copies`] can tell its copies from other mail in
/// the target. `cached` comes from `store::cached_message_ids`; what the cache
/// lacks is read from the server. `None` when a message can't be described
/// there either — then no copies are picked.
///
/// Read-only, on its own session, and run before the move: best-effort, since
/// failing here costs only the exact undo, never the move.
pub async fn moved_message_ids(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uids: &[u32],
    cached: Vec<Option<String>>,
) -> Option<Vec<String>> {
    let missing: Vec<u32> = uids
        .iter()
        .zip(&cached)
        .filter(|(_, id)| id.is_none())
        .map(|(uid, _)| *uid)
        .collect();
    let fetched: HashMap<u32, String> = if missing.is_empty() {
        HashMap::new()
    } else {
        match fetch_headers_isolating_unparseable(engine, account, folder, missing).await {
            Ok((headers, _)) => headers
                .into_iter()
                .map(|header| (header.uid, header.message_id.trim().to_lowercase()))
                .collect(),
            Err(err) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail.sync",
                    "account={account} folder={folder}: reading messages before move failed: {err:#}"
                );
                return None;
            }
        }
    };
    uids.iter()
        .zip(cached)
        .map(|(uid, id)| id.or_else(|| fetched.get(uid).cloned()))
        .collect()
}

/// The UIDs of the copies a move created, among the target's messages at or
/// above its pre-move UIDNEXT (`arrivals`). `moved` holds the lowercased
/// Message-ID of each moved message, `""` for one without.
///
/// Arrivals whose Message-ID the move didn't carry are other mail delivered
/// meanwhile, and are passed over. Each Message-ID the move did carry — the
/// empty one included — must account for exactly as many arrivals as moved
/// messages: fewer means a copy is missing, more means a concurrent delivery
/// is indistinguishable from it. Either way the answer would be wrong, so none
/// is given, and no Undo is offered.
pub(super) fn moved_copy_uids(arrivals: &[imap::MessageHeader], moved: &[String]) -> Vec<u32> {
    let mut expected: HashMap<&str, usize> = HashMap::new();
    for id in moved {
        *expected.entry(id.as_str()).or_default() += 1;
    }
    let mut found: HashMap<&str, Vec<u32>> = HashMap::new();
    for arrival in arrivals {
        let id = arrival.message_id.trim().to_lowercase();
        if let Some((&key, _)) = expected.get_key_value(id.as_str()) {
            found.entry(key).or_default().push(arrival.uid);
        }
    }
    let mut uids = Vec::with_capacity(moved.len());
    for (id, count) in expected {
        match found.remove(id) {
            Some(copies) if copies.len() == count => uids.extend(copies),
            _ => return Vec::new(),
        }
    }
    uids.sort_unstable();
    uids
}

/// Re-read `target` after moving `count` messages into it, and pick out the
/// copies the move created (see [`moved_copy_uids`]): every message at or
/// above the target's UIDNEXT from before the move is read, not just the
/// recent window, so a large move can't lose its earliest copies. Undo moves
/// exactly these back, rather than resolving the thread key in the target —
/// which would also take older mail of the same conversation, and finds
/// nothing for a `uid:` key, as UIDs change on a move.
///
/// `moved` is `None` when the moved messages couldn't all be described, and
/// then no copies are picked, as when the pre-move UIDNEXT is unknown.
///
/// Read-only and best-effort, on its own sessions: the MOVE has landed and
/// must not be retried or reported failed because a re-read went wrong.
pub async fn refresh_moved_copies(
    engine: &Arc<Engine>,
    account: &str,
    target: &str,
    count: usize,
    uid_next_before: Option<u32>,
    moved: Option<&[String]>,
) -> MovedCopies {
    let guard = match MessageSyncGuard::begin(engine) {
        Ok(guard) => guard,
        Err(err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "cannot guard moved-copy refresh: {err:#}"
            );
            return MovedCopies {
                batch: None,
                uids: Vec::new(),
                _snapshot_guard: None,
            };
        }
    };
    let batch = match fetch_recent_resilient(engine, account, target, 50.max(count as u32)).await {
        Ok(batch) => Some(batch),
        Err(err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "account={account} folder={target}: refresh after move failed: {err:#}"
            );
            None
        }
    };
    let uids = match (uid_next_before.filter(|uid_next| *uid_next > 0), moved) {
        (Some(floor), Some(moved)) => match moved_arrivals(engine, account, target, floor).await {
            Ok(arrivals) => moved_copy_uids(&arrivals, moved),
            Err(err) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail.sync",
                    "account={account} folder={target}: locating moved copies failed: {err:#}"
                );
                Vec::new()
            }
        },
        _ => Vec::new(),
    };
    MovedCopies {
        batch,
        uids,
        _snapshot_guard: Some(guard),
    }
}

/// Every message in `target` at or above `floor`. Fails rather than return a
/// partial set, which [`moved_copy_uids`] would take for a missing copy.
async fn moved_arrivals(
    engine: &Arc<Engine>,
    account: &str,
    target: &str,
    floor: u32,
) -> anyhow::Result<Vec<imap::MessageHeader>> {
    let uids = engine
        .with_read_session(account, |session| {
            let target = target.to_string();
            Box::pin(async move { imap::uids_from(session, &target, floor).await })
        })
        .await?;
    let (headers, skipped) =
        fetch_headers_isolating_unparseable(engine, account, target, uids.clone()).await?;
    anyhow::ensure!(
        skipped.is_empty() && headers.len() == uids.len(),
        "read {} of {} messages",
        headers.len(),
        uids.len()
    );
    Ok(headers)
}

/// Reconnect to IMAP, fetch the most recent `limit` messages of a folder into
/// the store, resetting cached messages if the server's UIDVALIDITY changed.
pub struct SyncMessagesResult {
    pub count: usize,
    pub messages: Vec<imap::MessageHeader>,
    pub arrivals: Vec<imap::MessageHeader>,
}

/// Rebuild what the batched sync would have produced, for a folder holding at
/// least one message whose FETCH response cannot be parsed. Establishes the
/// window with [`recover_recent_batch`], then re-runs the best-effort flag
/// reconciliation and UID listing that the failed attempt never reached.
pub(super) async fn sync_state_isolating_unparseable(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
    prior_modseq: u64,
    prior_validity: u32,
) -> anyhow::Result<(
    imap::RecentBatch,
    Option<imap::FlagSync>,
    Option<std::collections::HashSet<u32>>,
    Option<std::collections::HashSet<u32>>,
)> {
    let batch = recover_recent_batch(engine, account, folder, limit).await?;
    let uidvalidity = batch.uidvalidity;
    let (flag_sync, server_uids, server_unseen) = engine
        .with_read_session(account, |session| {
            let folder = folder.to_string();
            Box::pin(async move {
                let validity_matches = prior_validity != 0 && prior_validity == uidvalidity;
                let flag_sync = imap::sync_flags(session, &folder, prior_modseq, validity_matches)
                    .await
                    .ok();
                let server_uids = imap::list_all_uids(session, &folder).await.ok();
                let server_unseen = imap::list_unseen_uids(session, &folder).await.ok();
                anyhow::Ok((flag_sync, server_uids, server_unseen))
            })
        })
        .await
        .unwrap_or((None, None, None));
    Ok((batch, flag_sync, server_uids, server_unseen))
}

/// [`imap::fetch_recent`] for one folder, recovering from messages whose FETCH
/// response cannot be parsed instead of failing the whole batch. Prefer this to
/// calling `imap::fetch_recent` inside a session closure.
///
/// Read-only, so it must not share a session with a mutating command: a wedged
/// connection has to be discarded, and a mutation that already reached the
/// server must never be retried. Run the mutation first, then call this.
pub async fn fetch_recent_resilient(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> anyhow::Result<imap::RecentBatch> {
    let attempt = engine
        .with_read_session(account, |session| {
            let folder = folder.to_string();
            Box::pin(async move { imap::fetch_recent(session, &folder, limit).await })
        })
        .await;
    match attempt {
        Ok(batch) => Ok(batch),
        Err(err) if imap::is_unparseable_response(&err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "account={account} folder={folder}: unparseable FETCH response, \
                 re-reading the window message by message: {err:#}"
            );
            recover_recent_batch(engine, account, folder, limit).await
        }
        Err(err) => Err(err),
    }
}

/// Re-read the recent window of `folder` while working around messages whose
/// FETCH response cannot be parsed. Establishes the window as UIDs (a
/// `UID SEARCH` reply is immune to the failure), then reads it in ranges that
/// exclude the offending messages.
pub(super) async fn recover_recent_batch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> anyhow::Result<imap::RecentBatch> {
    let (uidvalidity, uid_next, uids) = engine
        .with_read_session(account, |session| {
            let folder = folder.to_string();
            Box::pin(async move { imap::recent_uids(session, &folder, limit).await })
        })
        .await?;
    let (messages, skipped) =
        fetch_headers_isolating_unparseable(engine, account, folder, uids).await?;
    if !skipped.is_empty() {
        crate::mlog!(
            crate::log::Level::Warn,
            "mail.sync",
            "account={account} folder={folder}: {} of {} messages skipped as unreadable: {skipped:?}",
            skipped.len(),
            messages.len() + skipped.len()
        );
    }
    Ok(imap::RecentBatch {
        uidvalidity,
        uid_next,
        messages,
    })
}

/// Ceiling on FETCH attempts one poison-message recovery may spend. A run with
/// a single bad message costs about `2 * log2(window)` attempts; this only
/// bites when a folder is riddled with them, where the right answer is to store
/// what we have rather than reconnect dozens of times.
pub(super) const ISOLATION_ATTEMPT_LIMIT: usize = 32;

/// Fetch headers for `uids`, working around messages whose FETCH response the
/// IMAP parser rejects (see [`imap::is_unparseable_response`]).
///
/// Such a response wedges the connection it arrived on, so it cannot be skipped
/// mid-stream — the session has to go and the range has to be re-fetched
/// without it. This halves any failing range until the offending message sits
/// alone, drops just that one, and keeps its neighbours. Every attempt runs
/// through `with_read_session`, which discards the failed session and hands the
/// retry a fresh connection.
///
/// Returns the headers it could read plus the UIDs it gave up on. A genuine
/// network or server error aborts, since retrying narrower ranges would not
/// help.
pub(super) async fn fetch_headers_isolating_unparseable(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uids: Vec<u32>,
) -> anyhow::Result<(Vec<imap::MessageHeader>, Vec<u32>)> {
    fetch_headers_isolating_until(engine, account, folder, uids, None).await
}

/// A catch-up deadline returns completed headers and isolated poison UIDs;
/// cancelled or unread chunks remain eligible for the next sync.
async fn fetch_headers_isolating_until(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    uids: Vec<u32>,
    deadline: Option<tokio::time::Instant>,
) -> anyhow::Result<(Vec<imap::MessageHeader>, Vec<u32>)> {
    let mut pending = vec![uids];
    let mut out: Vec<imap::MessageHeader> = Vec::new();
    let mut skipped: Vec<u32> = Vec::new();
    let mut attempts = 0usize;

    while let Some(chunk) = pending.pop() {
        if chunk.is_empty() {
            continue;
        }
        if attempts >= ISOLATION_ATTEMPT_LIMIT {
            let abandoned: usize = pending.iter().map(Vec::len).sum::<usize>() + chunk.len();
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "account={account} folder={folder}: giving up isolating unparseable \
                 messages after {attempts} fetches, {abandoned} UIDs left unread"
            );
            break;
        }
        attempts += 1;
        let operation = engine.with_read_session(account, |session| {
            let folder = folder.to_string();
            let chunk = chunk.clone();
            Box::pin(async move { imap::fetch_headers_by_uid(session, &folder, &chunk).await })
        });
        let result = if let Some(deadline) = deadline {
            match tokio::time::timeout_at(deadline, operation).await {
                Ok(result) => result,
                Err(_) => break,
            }
        } else {
            operation.await
        };
        match result {
            Ok(headers) => out.extend(headers),
            Err(err) if !imap::is_unparseable_response(&err) => return Err(err),
            Err(err) if chunk.len() == 1 => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail.sync",
                    "account={account} folder={folder}: skipping uid={}, its FETCH \
                     response could not be parsed: {err:#}",
                    chunk[0]
                );
                skipped.push(chunk[0]);
            }
            Err(_) => {
                // Push the tail first so the halves are attempted in UID order.
                let mid = chunk.len() / 2;
                pending.push(chunk[mid..].to_vec());
                pending.push(chunk[..mid].to_vec());
            }
        }
    }

    out.sort_unstable_by_key(|header| std::cmp::Reverse(header.uid));
    Ok((out, skipped))
}

pub async fn sync_messages(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> anyhow::Result<SyncMessagesResult> {
    sync_messages_with_policy(
        engine,
        account,
        folder,
        limit,
        false,
        ATTACHMENT_CATCHUP_BUDGET,
    )
    .await
}

/// Automatic desktop refresh: bounded network work, respecting pause state.
/// IDLE recovery and explicit mobile refresh retain their original policy.
pub async fn sync_background_messages(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> anyhow::Result<SyncMessagesResult> {
    sync_messages_with_policy(
        engine,
        account,
        folder,
        limit,
        true,
        ATTACHMENT_CATCHUP_BUDGET,
    )
    .await
}

/// A cache refresh after a successful server write. Keep the guard until the
/// returned batch has been persisted, so cleanup cannot release stale UIDs.
pub struct WrittenFolderRefresh {
    pub batch: imap::RecentBatch,
    _guard: MessageSyncGuard,
}

/// Cache warming must not turn a completed APPEND/save into a failed write.
/// Registration and network failures skip the refresh; callers retain success.
pub async fn refresh_written_folder(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> Option<WrittenFolderRefresh> {
    let result = async {
        let guard = MessageSyncGuard::begin(engine)?;
        let batch = fetch_recent_resilient(engine, account, folder, limit).await?;
        anyhow::Ok(WrittenFolderRefresh {
            batch,
            _guard: guard,
        })
    }
    .await;
    match result {
        Ok(refresh) => Some(refresh),
        Err(err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "account={account} folder={folder}: post-write refresh skipped: {err:#}"
            );
            None
        }
    }
}

// Keep registration alive through persistence, including when the caller drops
// its future after spawn_blocking has started. Drop also covers network errors
// and cancellation without leaving markers protected by a dead snapshot.
pub struct MessageSyncGuard {
    engine: Arc<Engine>,
    id: i64,
}

impl MessageSyncGuard {
    pub fn begin(engine: &Arc<Engine>) -> anyhow::Result<Self> {
        let db = engine.db.lock().unwrap();
        let (id, _) = store::begin_message_sync(&db)?;
        Ok(Self {
            engine: engine.clone(),
            id,
        })
    }
}

impl Drop for MessageSyncGuard {
    fn drop(&mut self) {
        let db = self.engine.db.lock().unwrap();
        if let Err(err) = store::end_message_sync(&db, self.id) {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "removal marker cleanup failed: {err:#}"
            );
        }
    }
}

const ATTACHMENT_CATCHUP_BUDGET: Duration = Duration::from_secs(5);

async fn sync_messages_with_policy(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
    background: bool,
    catchup_budget: Duration,
) -> anyhow::Result<SyncMessagesResult> {
    // Read the prior sync position before any network I/O so we can ask the
    // server for only the flag changes since then (CONDSTORE CHANGEDSINCE).
    // Callers must await this operation rather than timing out the whole sync;
    // only the network phase below is cancellable by the sync budget.
    let prepare_engine = engine.clone();
    let prepare_account = account.to_string();
    let prepare_folder = folder.to_string();
    let (prior_modseq, prior_validity, removed_epoch, removal_guard, missing_files) =
        tokio::task::spawn_blocking(move || {
            let engine = prepare_engine;
            let account = prepare_account.as_str();
            let folder = prepare_folder.as_str();
            let db = crate::log::timed_db_lock(&engine.db, "sync_messages.prepare/backfill");
            if background && store::account_paused(&db, account)? {
                return Err(anyhow::Error::new(BackgroundSyncCancelled));
            }
            let modseq = store::get_folder_modseq(&db, account, folder)?;
            let validity = store::get_folder_state(&db, account, folder)?
                .map(|(v, _)| v)
                .unwrap_or(0);
            // Removals after this point (an archive that lands while the fetch is
            // still on the wire) get a higher epoch. This snapshot must not write
            // those UIDs back.
            let missing_files =
                store::missing_attachment_metadata_uids(&db, account, folder, limit)?;
            let (id, removed_epoch) = store::begin_message_sync(&db)?;
            let guard = MessageSyncGuard {
                engine: engine.clone(),
                id,
            };
            anyhow::Ok((modseq, validity, removed_epoch, guard, missing_files))
        })
        .await??;

    let fetch = || async {
        let attempt = engine
            .with_read_session(account, |session| {
                let folder = folder.to_string();
                Box::pin(async move {
                    let batch = imap::fetch_recent(session, &folder, limit).await?;
                    // Reconcile \Seen across the whole folder (catches reads on other
                    // devices, even for messages older than the recent window).
                    // Best-effort: a no-op when there's no baseline, UIDVALIDITY changed,
                    // or the server lacks CONDSTORE.
                    let validity_matches =
                        prior_validity != 0 && prior_validity == batch.uidvalidity;
                    let flag_sync =
                        imap::sync_flags(session, &folder, prior_modseq, validity_matches)
                            .await
                            .ok();
                    // Server-side UID set so we can drop locally cached messages another
                    // client moved or deleted. Best-effort: a failure here skips the prune.
                    let server_uids = imap::list_all_uids(session, &folder).await.ok();
                    // Unread mail older than the window still counts toward the
                    // folder's unread total. Best-effort like the prune above.
                    let server_unseen = imap::list_unseen_uids(session, &folder).await.ok();
                    anyhow::Ok((batch, flag_sync, server_uids, server_unseen))
                })
            })
            .await;
        // A message whose FETCH response we cannot parse takes the whole batch down
        // with it, and does so again on every later sync. Re-read the window a
        // narrower range at a time so the rest of the folder still syncs.
        let state = match attempt {
            Ok(state) => Ok(state),
            Err(err) if imap::is_unparseable_response(&err) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail.sync",
                    "account={account} folder={folder}: unparseable FETCH response, \
                 re-reading the window message by message: {err:#}"
                );
                sync_state_isolating_unparseable(
                    engine,
                    account,
                    folder,
                    limit,
                    prior_modseq,
                    prior_validity,
                )
                .await
            }
            Err(err) => Err(err),
        }?;
        Ok(state)
    };
    let (batch, flag_sync, server_uids, server_unseen) = if background {
        retry_background_sync(&format!("sync {folder} for {account}"), || true, fetch).await?
    } else {
        fetch().await?
    };
    let synced_validity = batch.uidvalidity;
    let catchup_uids: Vec<_> = if prior_validity != 0 && prior_validity == synced_validity {
        missing_files
            .iter()
            .copied()
            .filter(|uid| !batch.messages.iter().any(|header| header.uid == *uid))
            .collect()
    } else {
        Vec::new()
    };
    let attempted: Vec<_> = missing_files
        .into_iter()
        .filter(|uid| batch.messages.iter().any(|header| header.uid == *uid))
        .collect();
    let catchup_engine = engine.clone();
    let catchup_account = account.to_string();
    let catchup_folder = folder.to_string();

    let synced_messages = batch.messages.clone();
    let count = synced_messages.len();
    crate::mlog!(
        crate::log::Level::Debug,
        "mail.sync",
        "messages account={account} folder={folder}: fetched={count} \
         uidvalidity={} uid_next={} prior_validity={prior_validity} server_uids={}",
        batch.uidvalidity,
        batch.uid_next,
        server_uids.as_ref().map_or(-1, |u| u.len() as i64)
    );
    let engine = engine.clone();
    let account = account.to_string();
    let folder = folder.to_string();
    // Outside the network timeout: always observe the committed result.
    let (result, removal_guard) = tokio::task::spawn_blocking(move || {
    let account = account.as_str();
    let folder = folder.as_str();
    let db = crate::log::timed_db_lock(&engine.db, "sync_messages.persist");
    let current = store::get_folder_state(&db, account, folder)?;
    let arrivals = if current.is_some_and(|(validity, _)| validity == batch.uidvalidity) && store::folder_notifies(&db, account, folder)? {
        store::classify_arrivals(&db, account, folder, current.unwrap().1, batch.uid_next, &batch.messages)?
    } else { Vec::new() };
    let persist_started = std::time::Instant::now();
    let phase_started = std::time::Instant::now();
    if prior_validity != 0 && prior_validity != batch.uidvalidity {
        store::clear_folder_messages(&db, account, folder)?;
    }
    let clear_time = phase_started.elapsed();
    let phase_started = std::time::Instant::now();
    let fetched_uids: Vec<u32> = batch.messages.iter().map(|message| message.uid).collect();
    store::release_removed_messages(&db, account, folder, &fetched_uids, removed_epoch)?;
    store::upsert_messages(&db, account, folder, &batch.messages)?;
    store::mark_attachment_metadata_attempted(&db, account, folder, &attempted)?;
    let upsert_time = phase_started.elapsed();
    // Make sure the folder is represented in the folders table so its unread
    // count surfaces (tray dot / badges) even before a full folder LIST sync —
    // which, in the unified view, may never run for this account.
    let phase_started = std::time::Instant::now();
    store::ensure_folder(&db, account, folder)?;
    let ensure_time = phase_started.elapsed();
    let phase_started = std::time::Instant::now();
    let mut pruned = 0;
    if let Some(uids) = server_uids.as_ref() {
        let validity_ok = prior_validity == 0 || prior_validity == batch.uidvalidity;
        // An empty UID set means "the server holds nothing here" only if the
        // fetch also came back empty. Having just read `count` messages out of
        // this same mailbox, an empty SEARCH result contradicts itself — the
        // connection lied (see the CONDSTORE desync in imap::sync_flags) — and
        // pruning against it would delete the very messages just stored.
        let uids_credible = !uids.is_empty() || count == 0;
        if validity_ok && uids_credible {
            pruned = store::prune_missing_messages(&db, account, folder, uids)?;
        } else if !uids_credible {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "skipping prune for account={account} folder={folder}: \
                 UID SEARCH returned no UIDs but {count} messages were fetched"
            );
        }
    }
    let prune_time = phase_started.elapsed();
    let flag_count = flag_sync.as_ref().map_or(0, |fs| fs.changes.len());
    let phase_started = std::time::Instant::now();
    if let Some(fs) = flag_sync {
        for &(uid, seen, starred) in &fs.changes {
            store::update_message_seen(&db, account, folder, uid, seen)?;
            store::update_message_starred(&db, account, folder, uid, starred)?;
        }
        if fs.highest_modseq > 0 {
            store::set_folder_modseq(&db, account, folder, fs.highest_modseq)?;
        }
    }
    let flags_time = phase_started.elapsed();
    if let Some(unseen) = server_unseen.as_ref() {
        store::set_uncached_unseen(&db, account, folder, unseen)?;
    }
    let phase_started = std::time::Instant::now();
    store::set_folder_state(&db, account, folder, batch.uidvalidity, batch.uid_next)?;
    let state_time = phase_started.elapsed();
    let total = persist_started.elapsed();
    drop(db);
    if total.as_millis() >= 100 {
        crate::mlog!(
            crate::log::Level::Warn,
            "sync.persist.timing",
            "account={account} folder={folder:?} fetched={count} server_uids={} pruned={pruned} flag_changes={flag_count} clear_ms={} upsert_ms={} ensure_ms={} prune_ms={} flags_ms={} state_ms={} total_ms={}",
            server_uids.as_ref().map_or(0, |uids| uids.len()),
            clear_time.as_millis(),
            upsert_time.as_millis(),
            ensure_time.as_millis(),
            prune_time.as_millis(),
            flags_time.as_millis(),
            state_time.as_millis(),
            total.as_millis()
        );
    }
    Ok::<_, anyhow::Error>((SyncMessagesResult {
        count, messages: synced_messages, arrivals,
    }, removal_guard))
    }).await??;

    // The main sync is already committed. Catch-up gets its own small budget;
    // an exhausted deadline cannot discard new mail, flags, or sync markers.
    if !catchup_uids.is_empty() {
        let deadline = tokio::time::Instant::now() + catchup_budget;
        match fetch_headers_isolating_until(
            &catchup_engine,
            &catchup_account,
            &catchup_folder,
            catchup_uids,
            Some(deadline),
        )
        .await
        {
            Ok((headers, skipped)) => {
                let persist = tokio::task::spawn_blocking(move || {
                    let _removal_guard = removal_guard;
                    let db = catchup_engine.db.lock().unwrap();
                    if store::get_folder_state(&db, &catchup_account, &catchup_folder)?
                        .map(|(validity, _)| validity)
                        != Some(synced_validity)
                    {
                        return anyhow::Ok(());
                    }
                    let attempted: Vec<_> = headers
                        .iter()
                        .map(|header| header.uid)
                        .chain(skipped)
                        .collect();
                    store::upsert_messages(&db, &catchup_account, &catchup_folder, &headers)?;
                    store::mark_attachment_metadata_attempted(
                        &db,
                        &catchup_account,
                        &catchup_folder,
                        &attempted,
                    )?;
                    anyhow::Ok(())
                })
                .await;
                if let Err(err) = persist
                    .map_err(anyhow::Error::from)
                    .and_then(|result| result)
                {
                    crate::mlog!(
                        crate::log::Level::Warn,
                        "mail.sync",
                        "attachment metadata persistence skipped: {err:#}"
                    );
                }
            }
            Err(err) => crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "attachment metadata catch-up skipped: {err:#}"
            ),
        }
    }
    Ok(result)
}

pub(super) const DEFAULT_BACKGROUND_SYNC_TIMEOUT_SECS: u64 = 30;
pub(super) const MAX_BACKGROUND_SYNC_TIMEOUT_SECS: u64 = 24 * 60 * 60;

pub(super) fn parse_background_sync_timeout(value: Option<&str>) -> Duration {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|&secs| (1..=MAX_BACKGROUND_SYNC_TIMEOUT_SECS).contains(&secs))
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_BACKGROUND_SYNC_TIMEOUT_SECS))
}

/// Per-folder sync budget, shared by background syncs and each piggybacked
/// companion sync so one slow mailbox can't hold the post-sync tail (and the
/// emit that follows it) indefinitely. The 30s default suits direct IMAP
/// servers; slow gateways (e.g. DavMail bridging Exchange EWS) can need more,
/// so it can be overridden with `MERON_SYNC_TIMEOUT` (seconds, up to one day).
pub fn background_sync_timeout() -> Duration {
    static TIMEOUT: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *TIMEOUT.get_or_init(|| {
        let value = std::env::var("MERON_SYNC_TIMEOUT").ok();
        parse_background_sync_timeout(value.as_deref())
    })
}

/// The companion mailboxes (Sent, then Drafts) to piggyback onto a sync of
/// another folder, each paired with its role label for the caller's logs.
/// Callers resolve the names via `cached_sent_folder`/`cached_drafts_folder`,
/// which already exclude the folder being synced; this only guards against the
/// two roles resolving to the same mailbox.
pub(super) fn companion_folders(
    sent: Option<String>,
    drafts: Option<String>,
) -> Vec<(&'static str, String)> {
    let mut companions = Vec::new();
    if let Some(sent) = sent {
        companions.push(("Sent", sent));
    }
    if let Some(drafts) = drafts
        && !companions
            .iter()
            .any(|(_, existing)| existing.eq_ignore_ascii_case(&drafts))
    {
        companions.push(("Drafts", drafts));
    }
    companions
}

/// Outcome of one companion-folder sync from [`sync_companion_folders`]:
/// `role` is "Sent" or "Drafts", `folder` the resolved mailbox name.
pub struct CompanionSync {
    pub role: &'static str,
    pub folder: String,
    pub result: anyhow::Result<SyncMessagesResult>,
}

/// Piggyback Sent and Drafts envelope syncs onto a completed sync of `folder`,
/// so messages sent or drafted from another client surface in the cross-folder
/// conversation view straight from the local store — the per-folder recent
/// sync only ever covers the open folder, and thread-gap filling only fetches
/// referenced *ancestors*, so nothing else ever pulls in a reply another
/// client added to Sent. Shared by desktop and mobile so the two can't drift.
/// Failures are returned per folder, never propagated: a companion hiccup must
/// not fail the primary sync that already succeeded.
pub async fn sync_companion_folders(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    limit: u32,
) -> Vec<CompanionSync> {
    let sent = cached_sent_folder(engine, account, folder);
    let drafts = cached_drafts_folder(engine, account, folder);
    let mut outcomes = Vec::new();
    for (role, companion) in companion_folders(sent, drafts) {
        let result = sync_background_messages(engine, account, &companion, limit).await;
        outcomes.push(CompanionSync {
            role,
            folder: companion,
            result,
        });
    }
    outcomes
}

#[cfg(test)]
mod removal_guard_tests {
    use super::*;

    struct TestHost;

    impl EngineHost for TestHost {
        fn open_db(&self) -> anyhow::Result<rusqlite::Connection> {
            store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &rusqlite::Connection, _: &str, _: &mut imap::Creds) {}
        fn store_secret(
            &self,
            _: &rusqlite::Connection,
            _: &str,
            _: &crate::secrets::Secrets,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    async fn metadata_test_server(
        total: u32,
        poison_uids: Vec<u32>,
    ) -> (
        Arc<Engine>,
        tokio::task::JoinHandle<()>,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        metadata_test_server_with_stall(total, poison_uids, None).await
    }

    async fn metadata_test_server_with_stall(
        total: u32,
        poison_uids: Vec<u32>,
        stalled_uid: Option<u32>,
    ) -> (
        Arc<Engine>,
        tokio::task::JoinHandle<()>,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use std::sync::atomic::AtomicUsize;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let commands_after_poison = Arc::new(AtomicUsize::new(0));
        let server_counter = commands_after_poison.clone();
        let server = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let counter = server_counter.clone();
                let poison_uids = poison_uids.clone();
                tokio::spawn(async move {
                    let (reader, mut writer) = socket.into_split();
                    writer.write_all(b"* OK IMAP4rev1 ready\r\n").await.unwrap();
                    let mut lines = tokio::io::BufReader::new(reader).lines();
                    let mut poisoned = false;
                    while let Ok(Some(line)) = lines.next_line().await {
                        if poisoned {
                            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        let (tag, command) = line.split_once(' ').unwrap();
                        let mut reply = if command.starts_with("SELECT") {
                            format!(
                                "* {total} EXISTS\r\n* OK [UIDVALIDITY 42] valid\r\n* OK [UIDNEXT {}] next\r\n",
                                total + 1
                            )
                        } else if command.starts_with("CAPABILITY") {
                            "* CAPABILITY IMAP4rev1\r\n".to_string()
                        } else if command.starts_with("UID SEARCH") {
                            if command.contains("UNSEEN") {
                                "* SEARCH\r\n".to_string()
                            } else {
                                format!(
                                    "* SEARCH {}\r\n",
                                    (1..=total)
                                        .map(|uid| uid.to_string())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                )
                            }
                        } else if command.starts_with("UID FETCH") {
                            let set = command.split_whitespace().nth(2).unwrap();
                            let mut uids = Vec::new();
                            for segment in set.split(',') {
                                if let Some((lo, hi)) = segment.split_once(':') {
                                    uids.extend(
                                        lo.parse::<u32>().unwrap()..=hi.parse::<u32>().unwrap(),
                                    );
                                } else {
                                    uids.push(segment.parse::<u32>().unwrap());
                                }
                            }
                            if let Some(uid) = uids.iter().find(|uid| poison_uids.contains(uid)) {
                                poisoned = true;
                                format!(
                                    "* {uid} FETCH (UID {uid} BODYSTRUCTURE (\"APPLICATION\" \"café\" NIL NIL NIL \"7BIT\" 1))\r\n"
                                )
                            } else if stalled_uid.is_some_and(|uid| uids.contains(&uid)) {
                                // Hold a partial FETCH open until cancellation drops
                                // the connection. No later command may reuse it.
                                writer.write_all(b"* 1 FETCH (UID 1 ").await.unwrap();
                                if let Ok(Some(_)) = lines.next_line().await {
                                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                }
                                break;
                            } else {
                                uids.iter().map(|uid| {
                                    if *uid == 2 { format!("* 2 FETCH (UID 2 FLAGS (\\Seen) BODYSTRUCTURE (\"APPLICATION\" \"PDF\" (\"NAME\" \"older.pdf\") NIL NIL \"BASE64\" 4))\r\n") }
                                    else { format!("* {uid} FETCH (UID {uid} FLAGS (\\Seen))\r\n") }
                                }).collect()
                            }
                        } else if command.starts_with("FETCH") {
                            format!(
                                "* {total} FETCH (UID {total} FLAGS (\\Seen) BODYSTRUCTURE (\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 4 1))\r\n"
                            )
                        } else {
                            String::new()
                        };
                        reply.push_str(&format!("{tag} OK done\r\n"));
                        if writer.write_all(reply.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        let engine = Arc::new(Engine::new(Box::new(TestHost)).unwrap());
        let mut creds = {
            let db = engine.db.lock().unwrap();
            let config = serde_json::json!({"host":"127.0.0.1", "port":port, "tls":false, "user":"u", "proxy":{"mode":"direct"}}).to_string();
            db.execute(
                "INSERT INTO accounts(id, config) VALUES('acct', ?1)",
                [&config],
            )
            .unwrap();
            store::set_folder_state(&db, "acct", "INBOX", 42, total + 1).unwrap();
            let headers: Vec<_> = (1..=total)
                .map(|uid| imap::MessageHeader {
                    uid,
                    date: uid as i64,
                    seen: true,
                    files: (uid == total).then(Vec::new),
                    ..Default::default()
                })
                .collect();
            store::upsert_messages(&db, "acct", "INBOX", &headers).unwrap();
            store::load_account(&db, "acct").unwrap().unwrap()
        };
        creds.password = "p".into();
        engine.accounts.lock().await.insert("acct".into(), creds);
        (engine, server, commands_after_poison)
    }

    /// UID 4 poisons every FETCH containing it, UID 3 omits BODYSTRUCTURE,
    /// and UID 2 carries a file. Catch-up must reach UID 2 on the next sync.
    #[tokio::test]
    async fn catchup_discards_poison_sessions_advances_and_preserves_sync_count() {
        use std::sync::atomic::Ordering;
        let (engine, server, commands_after_poison) = metadata_test_server(5, vec![4]).await;
        for expected_missing in [vec![2, 1], vec![]] {
            let result = tokio::time::timeout(
                Duration::from_secs(10),
                sync_messages(&engine, "acct", "INBOX", 2),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(result.count, 1);
            assert_eq!(result.messages.len(), 1);
            let db = engine.db.lock().unwrap();
            assert_eq!(
                store::missing_attachment_metadata_uids(&db, "acct", "INBOX", 10).unwrap(),
                expected_missing
            );
            let count: i64 = db
                .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 5, "pruning must retain all live cached rows");
        }
        assert_eq!(commands_after_poison.load(Ordering::SeqCst), 0);
        assert_eq!(
            store::get_recent_page(
                &engine.db.lock().unwrap(),
                "acct",
                "INBOX",
                50,
                None,
                store::RecentFilter {
                    unread_only: false,
                    attachments_only: true
                }
            )
            .unwrap()
            .0[0]
                .uid,
            2
        );
        server.abort();
    }

    #[tokio::test]
    async fn catchup_leaves_unread_chunks_eligible_at_isolation_limit() {
        let (engine, server, commands_after_poison) =
            metadata_test_server(51, (26..=50).collect()).await;
        let sync = || {
            tokio::time::timeout(
                Duration::from_secs(15),
                sync_messages(&engine, "acct", "INBOX", 50),
            )
        };
        assert_eq!(sync().await.unwrap().unwrap().count, 1);
        let missing = store::missing_attachment_metadata_uids(
            &engine.db.lock().unwrap(),
            "acct",
            "INBOX",
            100,
        )
        .unwrap();
        assert!(
            missing.contains(&1),
            "healthy UID in an unread chunk must stay eligible"
        );
        assert!(
            missing.len() < 50,
            "isolated poison UIDs should advance progress"
        );
        // Further bounded syncs finish the poison range and reach the healthy file.
        for _ in 0..5 {
            if store::missing_attachment_metadata_uids(
                &engine.db.lock().unwrap(),
                "acct",
                "INBOX",
                100,
            )
            .unwrap()
            .is_empty()
            {
                break;
            }
            sync().await.unwrap().unwrap();
        }
        let db = engine.db.lock().unwrap();
        assert!(
            store::missing_attachment_metadata_uids(&db, "acct", "INBOX", 100)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store::get_recent_page(
                &db,
                "acct",
                "INBOX",
                50,
                None,
                store::RecentFilter {
                    unread_only: false,
                    attachments_only: true
                }
            )
            .unwrap()
            .0[0]
                .uid,
            2
        );
        assert_eq!(
            commands_after_poison.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        server.abort();
    }

    #[tokio::test]
    async fn catchup_timeout_preserves_main_sync_and_completed_progress() {
        let (engine, server, commands_after_failure) =
            metadata_test_server_with_stall(5, vec![4], Some(1)).await;
        // The poison UID is isolated and UID 3 completes before the healthy
        // [1,2] chunk stalls. Both completed rows must survive the deadline.
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            sync_messages_with_policy(&engine, "acct", "INBOX", 4, true, Duration::from_secs(2)),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(
            store::missing_attachment_metadata_uids(
                &engine.db.lock().unwrap(),
                "acct",
                "INBOX",
                10
            )
            .unwrap(),
            vec![2, 1]
        );
        for _ in 0..2 {
            {
                let db = engine.db.lock().unwrap();
                db.execute("DELETE FROM messages WHERE uid = 5", [])
                    .unwrap();
                store::set_folder_state(&db, "acct", "INBOX", 42, 5).unwrap();
            }
            // Repeated catch-up timeouts must not hold new mail or sync state
            // hostage, nor mark cancelled UIDs as attempted.
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                sync_messages_with_policy(
                    &engine,
                    "acct",
                    "INBOX",
                    4,
                    true,
                    Duration::from_millis(150),
                ),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(result.count, 1);
            let db = engine.db.lock().unwrap();
            assert_eq!(
                store::get_folder_state(&db, "acct", "INBOX").unwrap(),
                Some((42, 6))
            );
            let seen: bool = db
                .query_row("SELECT seen FROM messages WHERE uid = 5", [], |r| r.get(0))
                .unwrap();
            assert!(
                seen,
                "new recent headers must be persisted despite catch-up timeout"
            );
            assert_eq!(
                store::missing_attachment_metadata_uids(&db, "acct", "INBOX", 10).unwrap(),
                vec![2, 1]
            );
        }
        assert_eq!(
            commands_after_failure.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        server.abort();
    }

    #[tokio::test]
    async fn post_write_refresh_skips_a_failed_guard_registration() {
        let engine = Arc::new(Engine::new(Box::new(TestHost)).unwrap());
        // Force the registration INSERT to fail before any network request.
        engine
            .db
            .lock()
            .unwrap()
            .execute_batch("CREATE TEMP TABLE active_message_syncs (invalid_column INTEGER);")
            .unwrap();
        assert!(
            refresh_written_folder(&engine, "acct", "Drafts", 20)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn cancelling_a_snapshot_releases_its_removal_markers() {
        let engine = Arc::new(Engine::new(Box::new(TestHost)).unwrap());
        let worker_engine = engine.clone();
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _guard = MessageSyncGuard::begin(&worker_engine).unwrap();
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        {
            let db = engine.db.lock().unwrap();
            store::delete_messages_by_uid(&db, "acct", "INBOX", &[7]).unwrap();
            let count: i64 = db
                .query_row("SELECT count(*) FROM removed_message_uids", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(count, 1);
        }
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let db = engine.db.lock().unwrap();
        let count: i64 = db
            .query_row("SELECT count(*) FROM removed_message_uids", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
