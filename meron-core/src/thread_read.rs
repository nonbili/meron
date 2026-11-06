//! Shared thread reading for both frontends. Desktop (`messages.thread` in the
//! sidecar) and mobile (`mail.threadRead` over FFI) resolve a thread the same
//! way: select and paginate its headers, serve message bodies cache-first, pull
//! the newest uncached body inline, hand older gaps to a deduped background
//! fetch, and shape the final bridge-ready message JSON. Platform callers only
//! parse their own id formats and decide how a finished background fetch is
//! announced (sidecar event vs FFI event callback).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};

use crate::engine::{Engine, MessageSyncGuard, TransferLimits, attach_html, message_key};
use crate::reply::ReplyTarget;
use crate::{imap, mail_model, parse, quote, reply, store};

/// Called when a background body fetch has something for the UI: bodies or
/// attachment files that arrived, or a body that could not be downloaded.
pub type BodiesFetchedHook = Box<dyn Fn() + Send + Sync + 'static>;

pub struct ThreadReadArgs<'a> {
    pub account: &'a str,
    /// Canonical nominal folder; headers from the cross-folder query carry
    /// their own source folder and fall back to this one.
    pub folder: &'a str,
    /// Full bridge thread id, used verbatim for the messages' `id`/`thread_id`
    /// fields so they match what the requesting UI keys on.
    pub thread_id: &'a str,
    /// Root thread key, already split from any branch-subject suffix.
    pub thread_key: &'a str,
    pub subject_filter: Option<&'a str>,
    /// Present for UI reads (page size); absent for full-scan reads (markRead,
    /// compose threading), which fetch every missing body synchronously.
    pub limit: Option<u32>,
    /// Bounded best-effort body fetch for printing; never fail for missing bodies.
    pub for_print: bool,
    /// Opaque cursor from a previous page's `next_cursor`: `message:<base64
    /// folder>:<uid>`, since UIDs are only unique within one mailbox. Legacy
    /// `uid:N` cursors are still accepted, but match on UID alone and so can
    /// resolve to the wrong message in a cross-folder thread — never hand-build
    /// one.
    pub before_cursor: Option<&'a str>,
    /// Attachment/media directory (env-derived on desktop, `data_dir` on mobile).
    pub media_root: PathBuf,
}

/// A header's resolved body state: the cached message (if any) and whether the
/// cache can serve it in full — threading headers extracted and inline media
/// still on disk. Incomplete slots are candidates for a refetch but still
/// render from whatever the cache has.
struct Slot {
    folder: String,
    cached: Option<parse::Message>,
    complete: bool,
    /// Attachment files of `cached` that are not on disk, counted once when the
    /// message is loaded or fetched rather than on every question about it.
    media_missing: usize,
}

/// The headers one page of a thread shows, oldest first, and the cursor of the
/// page before it: the thread across folders (or one folder for a `uid:` key),
/// narrowed to the subject branch, drafts collapsed, then sliced by
/// `before_cursor` and `limit`.
pub fn thread_page_headers(
    db: &rusqlite::Connection,
    account: &str,
    folder: &str,
    thread_key: &str,
    subject_filter: Option<&str>,
    limit: Option<u32>,
    before_cursor: Option<&str>,
) -> anyhow::Result<(Vec<imap::MessageHeader>, Option<String>)> {
    // Thread view spans folders within the account so the user's own Sent
    // replies appear alongside the inbox messages they thread with.
    //
    // Exception: a synthetic `uid:N` key (a message with no real threading
    // headers — e.g. a freshly-saved draft) is folder-local, because UIDs are
    // folder-scoped. Spanning folders would match an unrelated message that
    // happens to share UID N elsewhere and render the wrong thread.
    let mut headers = if thread_key.starts_with("uid:") {
        let mut headers = store::get_thread_headers(db, account, folder, thread_key)?;
        // A message opened straight from a notification can have a cached body
        // before its header row is synced; synthesize a header so the read
        // still renders instead of coming back empty.
        if headers.is_empty()
            && let Ok(uid) = thread_key["uid:".len()..].parse::<u32>()
            && store::get_cached_message(db, account, folder, uid)
                .ok()
                .flatten()
                .is_some()
        {
            headers.push(imap::MessageHeader {
                uid,
                folder: folder.to_string(),
                thread_key: thread_key.to_string(),
                ..Default::default()
            });
        }
        headers
    } else {
        store::get_thread_headers_all_folders(db, account, thread_key)?
    };
    if let Some(filter) = subject_filter {
        headers.retain(|header| store::subject_in_branch(&header.subject, filter));
    }
    let headers = store::collapse_thread_draft_headers(db, account, folder, headers)?;

    // `before_cursor` / `limit` are honored as a date-ordered slice so the
    // cross-folder query stays consistent with the old per-folder pagination
    // contract. New cursors name both the source folder and UID because UIDs
    // are only unique within one mailbox. Legacy uid-only cursors remain
    // readable for pagination requests minted by an older app process.
    let before_location = before_cursor.and_then(parse_thread_cursor);
    Ok(if let Some(limit) = limit {
        let mut headers = headers;
        if let Some(cursor) = before_location
            && let Some(idx) = thread_cursor_position(&headers, &cursor)
        {
            headers.truncate(idx);
        }
        let total = headers.len();
        let start = total.saturating_sub(limit as usize);
        let page = headers[start..].to_vec();
        let next_cursor = if start > 0 {
            page.first().map(thread_cursor)
        } else {
            None
        };
        (page, next_cursor)
    } else {
        (headers, None)
    })
}

/// Read one page of a thread and return `{ "messages": [...], "next_cursor"? }`
/// with final bridge-shaped message JSON (see [`thread_message_json`]).
pub async fn read_thread_page(
    engine: &Arc<Engine>,
    args: ThreadReadArgs<'_>,
    on_bodies_fetched: Option<BodiesFetchedHook>,
) -> anyhow::Result<Value> {
    let ThreadReadArgs {
        account,
        folder,
        thread_id,
        thread_key,
        subject_filter,
        limit,
        for_print,
        before_cursor,
        media_root,
    } = args;

    let (headers, next_cursor) = {
        let db = engine.db.lock().unwrap();
        thread_page_headers(
            &db,
            account,
            folder,
            thread_key,
            subject_filter,
            limit,
            before_cursor,
        )?
    };

    let cached_messages: Vec<(String, Option<parse::Message>)> = {
        let db = engine.db.lock().unwrap();
        headers
            .iter()
            .map(|header| {
                let msg_folder = if header.folder.is_empty() {
                    folder.to_string()
                } else {
                    header.folder.clone()
                };
                let cached = store::get_cached_message(&db, account, &msg_folder, header.uid)
                    .ok()
                    .flatten();
                (msg_folder, cached)
            })
            .collect()
    };
    // Paged reads return cached text immediately; full scans and print snapshots
    // wait for missing attachment files as well.
    let wait_for_media = limit.is_none() || for_print;
    // The files are looked for here, with no lock held.
    let counted: Vec<(String, Option<parse::Message>, usize)> = cached_messages
        .into_iter()
        .map(|(msg_folder, cached)| {
            let media_missing = cached.as_ref().map_or(0, |message| {
                parse::missing_media_count(&media_root, message)
            });
            (msg_folder, cached, media_missing)
        })
        .collect();
    let mut slots = {
        counted
            .into_iter()
            .map(|(msg_folder, cached, media_missing)| {
                let media_settled = !wait_for_media || media_missing == 0;
                // Rows cached before the threading-header extraction landed
                // have an empty `message_id`; refetch them so
                // reply_to/cc/references populate. Real-world mail almost
                // always carries a Message-ID, so emptiness is a reliable
                // "pre-extraction cache" signal.
                let complete = cached
                    .as_ref()
                    .is_some_and(|message| !message.message_id.is_empty() && media_settled);
                Slot {
                    folder: msg_folder,
                    cached,
                    complete,
                    media_missing,
                }
            })
            .collect::<Vec<_>>()
    };

    let missing: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| !slot.complete)
        .map(|(idx, _)| idx)
        .collect();
    if !missing.is_empty() {
        if for_print {
            // Persist each completed body before starting the next one, so the
            // page deadline cannot discard progress and retries converge.
            // Leave room under the desktop's 30s bridge deadline.
            // Do not start background work for this independent print snapshot.
            let result = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                for &idx in &missing {
                    fetch_into_slots(
                        engine,
                        account,
                        &headers,
                        &mut slots,
                        &[idx],
                        &media_root,
                        TransferLimits::within(crate::engine::INLINE_BODY_BUDGET),
                    )
                    .await?;
                }
                anyhow::Ok(())
            })
            .await;
            if !matches!(result, Ok(Ok(()))) {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail",
                    "print body fetch incomplete for {account}"
                );
            }
        } else if limit.is_none() {
            // Full-scan path (markRead, compose threading): every message must
            // be present — a reply built off a body-less copy has no
            // Message-ID and would orphan the thread on the recipient's side.
            // Fetch all gaps before answering.
            //
            // The allowance grows with the count, up to a ceiling a caller can
            // sit through. What arrived before it ran out is cached, so a
            // thread too long for one attempt is shorter on the next.
            if let Err(err) = fetch_into_slots(
                engine,
                account,
                &headers,
                &mut slots,
                &missing,
                &media_root,
                thread_budget(missing.len(), FULL_SCAN_BODY_BUDGET),
            )
            .await
            {
                // Degrade to the stale cached bodies when every message still
                // has one (e.g. a local draft with no Message-ID that a
                // refetch can't improve on); fail only when the thread would
                // otherwise be missing content.
                if slots.iter().any(|slot| slot.cached.is_none()) {
                    return Err(err);
                }
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail",
                    "thread body refetch for {account}: {err:#}"
                );
            }
        } else {
            // UI page: it must answer within the caller's timeout budget even
            // on a slow or dead connection, so fetch only the newest gap
            // inline — usually the very message the user tapped, since sync
            // stores envelopes only — and hand older gaps to the background
            // fill below.
            let newest = *missing.last().unwrap();
            // A fill that just failed announced itself, and this is the read
            // that shows it: none of its messages is downloaded again here,
            // or an unreachable server would be asked on every notification.
            // Taking the failures leaves the read after this one free to retry.
            let failures: Vec<(usize, String)> = missing
                .iter()
                .filter_map(|&idx| {
                    let key = message_key(account, &slots[idx].folder, headers[idx].uid);
                    Some((idx, engine.take_body_failure(&key)?))
                })
                .collect();
            let failed = failures
                .iter()
                .find(|(idx, _)| *idx == newest)
                .map(|(_, error)| error);
            if let Some(error) = failed {
                if slots.iter().all(|slot| slot.cached.is_none()) {
                    return Err(anyhow::anyhow!(error.clone()));
                }
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail",
                    "background thread body fetch for {account}: {error}"
                );
            }
            // Re-reads triggered by notifications must not start a second
            // interactive download beside the same message's background fill.
            let filling = engine.body_fetches.lock().unwrap().contains(&message_key(
                account,
                &slots[newest].folder,
                headers[newest].uid,
            ));
            let inline_error = if filling || failed.is_some() {
                None
            } else {
                match fetch_into_slots(
                    engine,
                    account,
                    &headers,
                    &mut slots,
                    &[newest],
                    &media_root,
                    TransferLimits::within(crate::engine::INLINE_BODY_BUDGET),
                )
                .await
                {
                    Ok(()) => None,
                    // FETCH may deliver a body before command completion fails.
                    // Show any bodies now available and retry remaining gaps
                    // in the background.
                    Err(err) if slots.iter().any(|slot| slot.cached.is_some()) => {
                        crate::mlog!(
                            crate::log::Level::Warn,
                            "mail",
                            "inline thread body fetch for {account} uid {}: {err:#}",
                            headers[newest].uid
                        );
                        None
                    }
                    Err(err) => Some(err),
                }
            };
            let mut background = pending_body_recovery(
                &engine.media_recovery_hold.lock().unwrap(),
                account,
                &slots,
                &headers,
                &missing,
            );
            // Keep cached messages visible; a later read can retry the rest.
            background.retain(|(folder, uid)| {
                !failures
                    .iter()
                    .any(|(idx, _)| slots[*idx].folder == *folder && headers[*idx].uid == *uid)
            });
            // Bodies we can already show, whose attachment bytes are gone.
            for item in messages_missing_media(
                &engine.media_recovery_hold.lock().unwrap(),
                account,
                &slots,
                &headers,
            ) {
                if !background.contains(&item) {
                    background.push(item);
                }
            }
            if !background.is_empty() {
                spawn_fill_thread_bodies(
                    engine,
                    account,
                    background,
                    media_root.clone(),
                    on_bodies_fetched,
                );
            }
            // Even a first-ever body that exceeds the interactive budget gets
            // a longer background attempt, while the caller receives its error.
            if let Some(err) = inline_error {
                return Err(err);
            }
        }
    } else if limit.is_some() && !for_print {
        // Every body is cached, but a cleared attachment cache still leaves
        // keys pointing at files that are gone. Refetch those without holding
        // the body the UI already has.
        let background = messages_missing_media(
            &engine.media_recovery_hold.lock().unwrap(),
            account,
            &slots,
            &headers,
        );
        if !background.is_empty() {
            spawn_fill_thread_bodies(
                engine,
                account,
                background,
                media_root.clone(),
                on_bodies_fetched,
            );
        }
    }

    let (mine, ours, remote_policy) = {
        let db = engine.db.lock().unwrap();
        (
            store::self_addrs(&db, account),
            store::all_self_addrs(&db),
            store::remote_image_policy(&db, account).unwrap_or_default(),
        )
    };
    let mut seen_message_ids = HashSet::new();
    let mut messages = Vec::with_capacity(headers.len());
    // Looked at after any fill above was started, so a placeholder this read
    // returns says whether its body is on the way.
    let filling = engine.body_fetches.lock().unwrap().clone();
    for (header, slot) in headers.iter().zip(slots) {
        let mut cached = slot.cached;
        if let Some(message) = cached.as_mut() {
            attach_html(message, &remote_policy);
        }
        // Newly synced envelope rows do not have json.message_id yet, so the
        // SQL-level cross-folder dedupe cannot collapse a self-addressed
        // Sent/Inbox pair on the first thread read. Once the body fetch has
        // cached the full message, collapse later copies before returning.
        let message_id_key = cached
            .as_ref()
            .map(|message| message.message_id.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if !message_id_key.is_empty() && !seen_message_ids.insert(message_id_key) {
            continue;
        }
        let mut message = thread_message_json(
            account,
            thread_id,
            &slot.folder,
            header,
            cached.as_ref(),
            &mine,
            &ours,
        );
        message["tags"] = json!(store::message_tags(
            &engine.db.lock().unwrap(),
            account,
            &slot.folder,
            header.uid,
        )?);
        // A paged read answers before pruned attachment files are back. The
        // body and its `/media` URLs are the same before and after, so this is
        // what tells a client its pictures are worth asking for again. A count,
        // not a flag: a recovery that restores one file and not another still
        // has to show.
        if slot.media_missing > 0 {
            message["media_missing"] = json!(slot.media_missing);
        }
        if let Some(cached) = cached.as_mut() {
            if cached.invite.is_none() {
                cached.invite =
                    crate::calendar::invite_from_attachments(&media_root, &cached.attachments);
            }
            if let Some(invite) = &cached.invite {
                let db = engine.db.lock().unwrap();
                // Covers an update downloaded before updates were applied on
                // download, or before what it revises was filed.
                let _ = crate::calendar::events::apply_invite_update(
                    &db,
                    account,
                    &cached.from_addr,
                    &cached.message_id,
                    invite,
                );
                message["invite"] = crate::calendar::invite_json_for(&db, account, invite, &mine);
            }
        }
        // Tells a body still downloading in the background from one that
        // could not be downloaded: only the second is worth a retry button.
        if cached.is_none() && filling.contains(&message_key(account, &slot.folder, header.uid)) {
            message["body_loading"] = json!(true);
        }
        messages.push(message);
    }

    let mut out = json!({ "messages": messages });
    if let Some(cursor) = next_cursor {
        out.as_object_mut()
            .unwrap()
            .insert("next_cursor".into(), Value::String(cursor));
    }
    Ok(out)
}

// Attachment holds never block a first body download or its longer background
// retry. Only already-cached bodies participate in media recovery suppression.
fn pending_body_recovery(
    hold: &MediaRecoveryHold,
    account: &str,
    slots: &[Slot],
    headers: &[imap::MessageHeader],
    indices: &[usize],
) -> Vec<(String, u32)> {
    indices
        .iter()
        .copied()
        .filter(|&idx| {
            !slots[idx].complete
                && (!slots[idx]
                    .cached
                    .as_ref()
                    .is_some_and(|message| !message.message_id.is_empty())
                    || !media_recovery_held(
                        hold,
                        &message_key(account, &slots[idx].folder, headers[idx].uid),
                    ))
        })
        .map(|idx| (slots[idx].folder.clone(), headers[idx].uid))
        .collect()
}

/// Cached messages whose attachment files are not on disk: pruned, never
/// written, or cached before their keys were kept.
///
/// A message is left out while its recovery is on hold (see
/// `Engine::media_recovery_hold`), so the reader is not notified and asked
/// again for as long as the thread stays open.
fn messages_missing_media(
    hold: &MediaRecoveryHold,
    account: &str,
    slots: &[Slot],
    headers: &[imap::MessageHeader],
) -> Vec<(String, u32)> {
    slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.complete && slot.media_missing > 0)
        .map(|(idx, slot)| (slot.folder.clone(), headers[idx].uid))
        .filter(|(folder, uid)| !media_recovery_held(hold, &message_key(account, folder, *uid)))
        .collect()
}

fn thread_cursor(header: &imap::MessageHeader) -> String {
    format!(
        "message:{}:{}",
        URL_SAFE_NO_PAD.encode(header.folder.as_bytes()),
        header.uid
    )
}

fn parse_thread_cursor(cursor: &str) -> Option<(Option<String>, u32)> {
    if let Some(rest) = cursor.strip_prefix("message:") {
        let (folder, uid) = rest.split_once(':')?;
        let folder = String::from_utf8(URL_SAFE_NO_PAD.decode(folder).ok()?).ok()?;
        return Some((Some(folder), uid.parse().ok()?));
    }
    let uid = cursor.strip_prefix("uid:")?.parse().ok()?;
    Some((None, uid))
}

fn thread_cursor_position(
    headers: &[imap::MessageHeader],
    cursor: &(Option<String>, u32),
) -> Option<usize> {
    let (folder, uid) = cursor;
    headers.iter().position(|header| {
        header.uid == *uid && folder.as_deref().is_none_or(|f| header.folder == f)
    })
}

/// The most a full-scan read (reply, markRead) spends downloading a thread.
const FULL_SCAN_BODY_BUDGET: std::time::Duration = std::time::Duration::from_secs(120);

/// The background fill. Nobody waits on it, so a large message gets the time
/// it needs to land, but it holds a session, the sync guard and its
/// messages' places in `body_fetches` while it runs: a silent socket has to
/// end it long before the total does.
const BACKGROUND_BODY_LIMITS: TransferLimits = TransferLimits {
    idle: std::time::Duration::from_secs(120),
    total: std::time::Duration::from_secs(600),
};

/// One on-demand allowance per message, up to `ceiling`: for a caller that is
/// waiting on all of them.
fn thread_budget(messages: usize, ceiling: std::time::Duration) -> TransferLimits {
    TransferLimits::within(
        (crate::engine::INLINE_BODY_BUDGET * messages.max(1) as u32).min(ceiling),
    )
}

type MediaRecoveryHold = std::collections::HashMap<String, Option<std::time::Instant>>;

/// How long readers stop waiting on a message's attachment files after a
/// download did not deliver it: long enough that an offline or struggling
/// connection does not stall every open of a thread that is otherwise cached.
const MEDIA_RECOVERY_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(120);

fn media_recovery_held(hold: &MediaRecoveryHold, key: &str) -> bool {
    match hold.get(key) {
        Some(None) => true,
        Some(Some(until)) => std::time::Instant::now() < *until,
        None => false,
    }
}

/// Record what the already-cached bodies from a thread download mean for each
/// message's attachment recovery (see `Engine::media_recovery_hold`):
///
/// - arrived with its files: no hold;
/// - arrived and files are still missing: held until restart, since another
///   download is not known to help and a reader that keeps re-reading would
///   otherwise be sent to fetch it without end;
/// - already cached, asked for in a folder the download got to, and not
///   delivered: held for
///   [`MEDIA_RECOVERY_COOLDOWN`] after a failure, or until restart when a
///   successful FETCH left it out (an expunged UID);
/// - uncached bodies or a folder the download never reached: no hold.
///
/// Returns each delivered message with its count of missing files.
fn store_thread_bodies(
    engine: &Engine,
    account: &str,
    asked: &BTreeMap<String, Vec<u32>>,
    fetched: Vec<(String, u32, parse::Message)>,
    attempted: &HashSet<String>,
    succeeded: bool,
    media_root: &std::path::Path,
) -> Vec<(String, u32, parse::Message, usize)> {
    // The files are looked for before any lock is taken.
    let delivered: Vec<(String, u32, parse::Message, usize)> = fetched
        .into_iter()
        .map(|(folder, uid, message)| {
            let media_missing = parse::missing_media_count(media_root, &message);
            (folder, uid, message, media_missing)
        })
        .collect();
    let cached_bodies = {
        let db = engine.db.lock().unwrap();
        asked
            .iter()
            .flat_map(|(folder, uids)| {
                uids.iter().filter_map(|uid| {
                    store::get_cached_message(&db, account, folder, *uid)
                        .ok()
                        .flatten()
                        .filter(|message| !message.message_id.is_empty())
                        .map(|_| message_key(account, folder, *uid))
                })
            })
            .collect::<HashSet<_>>()
    };
    let now = std::time::Instant::now();
    let mut hold = engine.media_recovery_hold.lock().unwrap();
    hold.retain(|_, until| until.is_none_or(|until| now < until));
    let mut arrived: HashSet<String> = HashSet::new();
    for (folder, uid, _, media_missing) in &delivered {
        let key = message_key(account, folder, *uid);
        if *media_missing == 0 {
            hold.remove(&key);
        } else {
            hold.insert(key.clone(), None);
        }
        arrived.insert(key);
    }
    for (folder, uids) in asked {
        if !attempted.contains(folder) {
            continue;
        }
        for uid in uids {
            let key = message_key(account, folder, *uid);
            // Holds are only for attachment recovery, never first-time bodies.
            if !cached_bodies.contains(&key) {
                hold.remove(&key);
                continue;
            }
            // A hold until restart is not shortened by a later miss.
            if !arrived.contains(&key) && hold.get(&key) != Some(&None) {
                hold.insert(
                    key,
                    if succeeded {
                        None
                    } else {
                        Some(now + MEDIA_RECOVERY_COOLDOWN)
                    },
                );
            }
        }
    }
    delivered
}

/// Fetch the bodies for `indices` from IMAP on one session (each mailbox is
/// SELECTed once), cache them, and mark their slots complete. `limits` are for
/// all of them; what arrived before a failure is cached and marked all the
/// same.
async fn fetch_into_slots(
    engine: &Arc<Engine>,
    account: &str,
    headers: &[imap::MessageHeader],
    slots: &mut [Slot],
    indices: &[usize],
    media_root: &std::path::Path,
    limits: TransferLimits,
) -> anyhow::Result<()> {
    let _snapshot_guard = MessageSyncGuard::begin(engine)?;
    let mut by_folder: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for &idx in indices {
        by_folder
            .entry(slots[idx].folder.clone())
            .or_default()
            .push(headers[idx].uid);
    }
    let fetched = crate::engine::fetch_bodies_keeping_partial(
        engine, account, &by_folder, media_root, limits,
    )
    .await;
    let delivered = store_thread_bodies(
        engine,
        account,
        &by_folder,
        fetched.messages,
        &fetched.attempted,
        fetched.result.is_ok(),
        media_root,
    );
    for (folder, uid, message, media_missing) in delivered {
        if let Some(&idx) = indices
            .iter()
            .find(|&&idx| slots[idx].folder == folder && headers[idx].uid == uid)
        {
            slots[idx].media_missing = media_missing;
            slots[idx].cached = Some(message);
            slots[idx].complete = true;
        }
    }
    fetched.result
}

/// Fetch a thread's uncached message bodies in the background, then run the
/// platform hook so the open reader re-reads and the bodies appear. Keeps the
/// per-message IMAP fetches off the read's request path. Deduped per message
/// via `engine.body_fetches`.
pub(crate) fn spawn_fill_thread_bodies(
    engine: &Arc<Engine>,
    account: &str,
    missing: Vec<(String, u32)>,
    media_root: PathBuf,
    on_bodies_fetched: Option<BodiesFetchedHook>,
) {
    let missing: Vec<_> = {
        let mut filling = engine.body_fetches.lock().unwrap();
        missing
            .into_iter()
            .filter(|(folder, uid)| filling.insert(message_key(account, folder, *uid)))
            .collect()
    };
    if missing.is_empty() {
        return;
    }
    let keys: Vec<_> = missing
        .iter()
        .map(|(folder, uid)| message_key(account, folder, *uid))
        .collect();
    let engine = engine.clone();
    let account = account.to_string();
    tokio::spawn(async move {
        let mut by_folder: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for (folder, uid) in missing {
            by_folder.entry(folder).or_default().push(uid);
        }
        let mut delivered = false;
        let fill = async {
            let _guard = MessageSyncGuard::begin(&engine)?;
            let fetched = crate::engine::fetch_bodies_keeping_partial(
                &engine,
                &account,
                &by_folder,
                &media_root,
                BACKGROUND_BODY_LIMITS,
            )
            .await;
            delivered = !store_thread_bodies(
                &engine,
                &account,
                &by_folder,
                fetched.messages,
                &fetched.attempted,
                fetched.result.is_ok(),
                &media_root,
            )
            .is_empty();
            fetched.result
        };
        let result = fill.await;
        if let Err(err) = &result {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail",
                "background thread body fetch for {account}: {err:#}"
            );
        }
        let error = result.as_ref().err().map(|err| format!("{err:#}"));
        let uncached: Vec<(&String, u32)> = {
            let db = engine.db.lock().unwrap();
            by_folder
                .iter()
                .flat_map(|(folder, uids)| uids.iter().map(move |uid| (folder, *uid)))
                .filter(|(folder, uid)| {
                    !store::has_cached_body(&db, &account, folder, *uid).unwrap_or(false)
                })
                .collect()
        };
        for (folder, uid) in &uncached {
            engine.record_body_failure(
                message_key(&account, folder, *uid),
                error.clone().unwrap_or_else(|| {
                    format!("message uid {uid} not returned by FETCH in {folder}")
                }),
            );
        }
        {
            let mut filling = engine.body_fetches.lock().unwrap();
            for key in keys {
                filling.remove(&key);
            }
        }
        engine.body_fetch_done.notify_waiters();
        // Wake readers that returned placeholders while this fill was pending:
        // the next read shows what arrived, or consumes a recorded failure
        // instead of starting a loop. A fill that changed nothing a reader can
        // see (attachment files still missing, their bodies cached all along)
        // stays quiet, or the re-read would only start it again.
        if (delivered || !uncached.is_empty())
            && let Some(notify) = on_bodies_fetched
        {
            notify();
        }
    });
}

/// Final bridge-shaped message JSON, rendered from the header row plus the
/// cached body when one exists. A missing body (on-demand fetch failed or
/// still in flight) sets `body_missing` so clients can show a placeholder
/// with a retry instead of dropping the message.
pub fn thread_message_json(
    account_id: &str,
    thread_id: &str,
    folder: &str,
    header: &imap::MessageHeader,
    cached: Option<&parse::Message>,
    mine: &HashSet<String>,
    ours: &HashSet<String>,
) -> Value {
    // A conversation spans folders, but IMAP UIDs do not: INBOX/42 and
    // Sent/42 are two different messages. Key the message by its own mailbox
    // location rather than by the conversation's nominal folder so clients
    // do not collapse equal numeric UIDs from different folders. The ordinary
    // uid-style thread-id format also keeps the id understood by the existing
    // per-message action parsers.
    let id = mail_model::format_thread_id(account_id, folder, &format!("uid:{}", header.uid));
    let from_addr = cached
        .map(|message| message.from_addr.as_str())
        .unwrap_or(header.from_addr.as_str());
    // Who a reply addresses and who it copies is settled here, from the same
    // headers, so every frontend and the MCP tools agree on it (see
    // [`crate::reply`]).
    let reply = reply::reply_json(
        &ReplyTarget {
            from_name: cached
                .map(|message| message.from_name.as_str())
                .unwrap_or(header.from_name.as_str()),
            from_addr,
            reply_to: cached
                .map(|message| message.reply_to.as_str())
                .unwrap_or(""),
            to: cached.map(|message| message.to.as_str()).unwrap_or(""),
            cc: cached.map(|message| message.cc.as_str()).unwrap_or(""),
        },
        ours,
    );
    json!({
        "id": id,
        "account_id": account_id,
        "folder_id": folder,
        "thread_id": thread_id,
        // Classified in the core (own address *or* Sent-folder provenance) so
        // both frontends render alias-sent mail as outgoing without knowing
        // the account's aliases.
        "outgoing": store::account_message_is_outgoing(
            account_id,
            mine,
            folder,
            from_addr,
            cached.is_some_and(|message| message.delivered),
        ),
        "from_name": cached.map(|message| message.from_name.as_str()).unwrap_or(header.from_name.as_str()),
        "from_addr": cached.map(|message| message.from_addr.as_str()).unwrap_or(header.from_addr.as_str()),
        "to": cached.map(|message| message.to.as_str()).unwrap_or(""),
        "reply_to": cached.map(|message| message.reply_to.as_str()).unwrap_or(""),
        "cc": cached.map(|message| message.cc.as_str()).unwrap_or(""),
        "bcc": cached.map(|message| message.bcc.as_str()).unwrap_or(""),
        "message_id": cached.map(|message| message.message_id.as_str()).unwrap_or(""),
        "in_reply_to": header.in_reply_to,
        "references": cached.map(|message| message.references.as_str()).unwrap_or(""),
        "subject": cached.map(|message| message.subject.as_str()).unwrap_or(header.subject.as_str()),
        "preview": cached.map(|message| message.preview.as_str()).unwrap_or(""),
        "body": cached.map(|message| message.body.as_str()).unwrap_or(""),
        "body_html": cached.and_then(|message| message.body_html.as_deref()).unwrap_or(""),
        // Where the plain body's quoted tail starts (UTF-16 offset), so clients
        // can fold it; null when there is none. HTML bodies mark theirs inline.
        "body_quote_start": cached.and_then(|message| quote::plain_quote_start(&message.body)),
        // No cached body means the on-demand IMAP fetch failed (auth/network)
        // or is still filling in the background — not that the message is
        // empty. Clients offer a retry / re-read for these.
        "body_missing": cached.is_none(),
        "date": cached.map(|message| message.date).unwrap_or(header.date),
        "unread": !header.seen,
        "starred": header.starred,
        "has_attachments": cached.is_some_and(|message| message.has_attachments()),
        "attachments": cached
            .map(|message| serde_json::to_value(&message.attachments).unwrap_or_else(|_| json!([])))
            .unwrap_or_else(|| json!([])),
        "reply": reply,
        "crypto": cached.and_then(|message| message.crypto.as_ref()).map(|status| {
            let outgoing = store::account_message_is_outgoing(
                account_id,
                mine,
                folder,
                from_addr,
                cached.is_some_and(|message| message.delivered),
            );
            crate::openpgp::mime::view_json(status, &crate::openpgp::keyring::current(), account_id, from_addr, outgoing)
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Slot, message_key, messages_missing_media, parse_thread_cursor, thread_cursor,
        thread_cursor_position, thread_message_json,
    };
    use crate::imap::MessageHeader;
    use crate::parse::{Attachment, Message, missing_media_count};
    use std::collections::HashSet;

    fn header(uid: u32, seen: bool) -> MessageHeader {
        MessageHeader {
            uid,
            subject: "Hi".to_string(),
            from_addr: "ann@x.com".to_string(),
            seen,
            ..Default::default()
        }
    }

    struct Host;
    impl crate::engine::EngineHost for Host {
        fn open_db(&self) -> anyhow::Result<rusqlite::Connection> {
            crate::store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &rusqlite::Connection, _: &str, _: &mut crate::imap::Creds) {}
        fn store_secret(
            &self,
            _: &rusqlite::Connection,
            _: &str,
            _: &crate::secrets::Secrets,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn failed_fill_notifies_a_pending_reader_and_surfaces_its_error_once() {
        let (engine, server) =
            crate::engine::slow_fetch_engine(1, std::time::Duration::from_millis(150), true).await;
        let (notify, notified) = tokio::sync::oneshot::channel();
        let notify = std::sync::Mutex::new(Some(notify));
        super::spawn_fill_thread_bodies(
            &engine,
            "acc",
            vec![("INBOX".into(), 1)],
            crate::parse::test_media_root(),
            Some(Box::new(move || {
                if let Some(notify) = notify.lock().unwrap().take() {
                    let _ = notify.send(());
                }
            })),
        );
        let read = || {
            super::read_thread_page(
                &engine,
                super::ThreadReadArgs {
                    account: "acc",
                    folder: "INBOX",
                    thread_id: "acc#INBOX#uid:1",
                    thread_key: "uid:1",
                    subject_filter: None,
                    limit: Some(20),
                    for_print: false,
                    before_cursor: None,
                    media_root: crate::parse::test_media_root(),
                },
                None,
            )
        };
        let page = read().await.unwrap();
        assert_eq!(page["messages"][0]["body_missing"], true);
        // Still downloading: a placeholder, not a failure to retry.
        assert_eq!(page["messages"][0]["body_loading"], true);
        tokio::time::timeout(std::time::Duration::from_secs(3), notified)
            .await
            .unwrap()
            .unwrap();
        assert!(engine.body_fetches.lock().unwrap().is_empty());
        let err = read().await.unwrap_err();
        assert!(format!("{err:#}").contains("UID FETCH"), "{err:#}");
        assert!(engine.body_fetches.lock().unwrap().is_empty());
        assert!(engine.body_fetch_failures.lock().unwrap().is_empty());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn failed_fill_keeps_other_cached_messages_visible_without_an_automatic_retry() {
        let engine = std::sync::Arc::new(crate::engine::Engine::new(Box::new(Host)).unwrap());
        let headers: Vec<_> = [1, 2, 3]
            .into_iter()
            .map(|uid| MessageHeader {
                thread_key: "same-thread".into(),
                date: uid as i64,
                ..header(uid, true)
            })
            .collect();
        {
            let db = engine.db.lock().unwrap();
            crate::store::upsert_messages(&db, "acc", "INBOX", &headers).unwrap();
            crate::store::save_cached_message(
                &db,
                "acc",
                "INBOX",
                1,
                &Message {
                    message_id: "cached@test".into(),
                    body: "Cached body".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        // One fill failed for both the older and the newest uncached message.
        for uid in [2, 3] {
            engine.record_body_failure(message_key("acc", "INBOX", uid), "download failed".into());
        }
        let page = super::read_thread_page(
            &engine,
            super::ThreadReadArgs {
                account: "acc",
                folder: "INBOX",
                thread_id: "acc#INBOX#same-thread",
                thread_key: "same-thread",
                subject_filter: None,
                limit: Some(20),
                for_print: false,
                before_cursor: None,
                media_root: crate::parse::test_media_root(),
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(page["messages"].as_array().unwrap().len(), 3);
        assert_eq!(page["messages"][0]["body"], "Cached body");
        assert_eq!(page["messages"][1]["body_missing"], true);
        assert_eq!(page["messages"][2]["body_missing"], true);
        // Failed, not on the way.
        assert!(page["messages"][1].get("body_loading").is_none());
        assert!(page["messages"][2].get("body_loading").is_none());
        // Neither is downloaded again by the read its failure announced.
        assert!(engine.body_fetches.lock().unwrap().is_empty());
        assert!(engine.body_fetch_failures.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn paged_read_shows_first_body_when_fetch_completion_fails() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (reader, mut writer) = socket.into_split();
            writer.write_all(b"* OK ready\r\n").await.unwrap();
            let mut lines = tokio::io::BufReader::new(reader).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                let (tag, command) = line.split_once(' ').unwrap();
                if command.starts_with("UID FETCH") {
                    let body = "Message-ID: <one@test>\r\nSubject: One\r\n\r\nFirst body";
                    writer
                        .write_all(
                            format!(
                                "* 1 FETCH (UID 1 BODY[] {{{}}}\r\n{body})\r\n{tag} NO completion failed\r\n",
                                body.len()
                            )
                            .as_bytes(),
                        )
                        .await
                        .unwrap();
                    return;
                }
                if command.starts_with("SELECT") {
                    writer.write_all(b"* 1 EXISTS\r\n").await.unwrap();
                }
                writer
                    .write_all(format!("{tag} OK done\r\n").as_bytes())
                    .await
                    .unwrap();
            }
        });
        let engine = std::sync::Arc::new(crate::engine::Engine::new(Box::new(Host)).unwrap());
        let creds = {
            let db = engine.db.lock().unwrap();
            let config = serde_json::json!({
                "host": "127.0.0.1", "port": port, "tls": false,
                "user": "u", "proxy": {"mode": "direct"}
            })
            .to_string();
            db.execute(
                "INSERT INTO accounts(id, config) VALUES('acc', ?1)",
                [&config],
            )
            .unwrap();
            crate::store::upsert_messages(&db, "acc", "INBOX", &[header(1, true)]).unwrap();
            crate::store::load_account(&db, "acc").unwrap().unwrap()
        };
        engine.accounts.lock().await.insert("acc".into(), creds);
        let page = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            super::read_thread_page(
                &engine,
                super::ThreadReadArgs {
                    account: "acc",
                    folder: "INBOX",
                    thread_id: "acc#INBOX#uid:1",
                    thread_key: "uid:1",
                    subject_filter: None,
                    limit: Some(20),
                    for_print: false,
                    before_cursor: None,
                    media_root: crate::parse::test_media_root(),
                },
                None,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(page["messages"].as_array().unwrap().len(), 1);
        assert_eq!(page["messages"][0]["body_missing"], false);
        assert_eq!(
            page["messages"][0]["body"].as_str().unwrap().trim(),
            "First body"
        );
        assert!(engine.body_fetches.lock().unwrap().is_empty());
        assert!(
            crate::store::has_cached_body(&engine.db.lock().unwrap(), "acc", "INBOX", 1).unwrap()
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn paged_read_does_not_restart_a_body_already_filling_in_background() {
        let engine = std::sync::Arc::new(crate::engine::Engine::new(Box::new(Host)).unwrap());
        crate::store::upsert_messages(
            &engine.db.lock().unwrap(),
            "acc",
            "INBOX",
            &[header(1, true)],
        )
        .unwrap();
        engine
            .body_fetches
            .lock()
            .unwrap()
            .insert(message_key("acc", "INBOX", 1));
        // No credentials are configured: an accidental inline attempt fails.
        let page = super::read_thread_page(
            &engine,
            super::ThreadReadArgs {
                account: "acc",
                folder: "INBOX",
                thread_id: "acc#INBOX#uid:1",
                thread_key: "uid:1",
                subject_filter: None,
                limit: Some(20),
                for_print: false,
                before_cursor: None,
                media_root: crate::parse::test_media_root(),
            },
            None,
        )
        .await
        .unwrap();
        assert_eq!(page["messages"].as_array().unwrap().len(), 1);
        assert_eq!(page["messages"][0]["body_missing"], true);
        assert!(
            engine
                .body_fetches
                .lock()
                .unwrap()
                .contains(&message_key("acc", "INBOX", 1))
        );
    }

    #[tokio::test]
    async fn an_active_fill_does_not_block_a_new_message_in_the_same_thread() {
        let engine = std::sync::Arc::new(crate::engine::Engine::new(Box::new(Host)).unwrap());
        let headers: Vec<_> = [1, 2]
            .into_iter()
            .map(|uid| MessageHeader {
                thread_key: "same-thread".into(),
                date: uid as i64,
                ..header(uid, true)
            })
            .collect();
        crate::store::upsert_messages(&engine.db.lock().unwrap(), "acc", "INBOX", &headers)
            .unwrap();
        let older = message_key("acc", "INBOX", 1);
        let newest = message_key("acc", "INBOX", 2);
        engine.body_fetches.lock().unwrap().insert(older.clone());
        // The missing credentials prove that the new UID gets an inline attempt
        // rather than being silently skipped because an older UID is filling.
        let err = super::read_thread_page(
            &engine,
            super::ThreadReadArgs {
                account: "acc",
                folder: "INBOX",
                thread_id: "acc#INBOX#same-thread",
                thread_key: "same-thread",
                subject_filter: None,
                limit: Some(1),
                for_print: false,
                before_cursor: None,
                media_root: crate::parse::test_media_root(),
            },
            None,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("account needs reconnect"),
            "{err:#}"
        );
        let filling = engine.body_fetches.lock().unwrap();
        assert!(filling.contains(&older));
        // Its longer background retry is claimed independently of the old fill.
        assert!(filling.contains(&newest));
    }

    #[test]
    fn successful_fetch_omissions_are_held_until_restart_but_failures_expire() {
        let engine = crate::engine::Engine::new(Box::new(Host)).unwrap();
        let asked = std::collections::BTreeMap::from([("INBOX".to_string(), vec![1, 2])]);
        let attempted = HashSet::from(["INBOX".to_string()]);
        let root = crate::parse::test_media_root();
        let cached = Message {
            message_id: "cached-1".into(),
            body: "Cached text".into(),
            ..Default::default()
        };
        crate::store::save_cached_message(&engine.db.lock().unwrap(), "acc", "INBOX", 1, &cached)
            .unwrap();
        super::store_thread_bodies(&engine, "acc", &asked, vec![], &attempted, false, &root);
        let key = message_key("acc", "INBOX", 1);
        assert!(engine.media_recovery_hold.lock().unwrap()[&key].is_some());
        let slots = (0..3)
            .map(|idx| Slot {
                folder: "INBOX".into(),
                cached: if idx == 0 {
                    Some(Message {
                        message_id: "cached-1".into(),
                        ..Default::default()
                    })
                } else {
                    None
                },
                complete: false,
                media_missing: 0,
            })
            .collect::<Vec<_>>();
        let headers = [header(1, true), header(2, true), header(3, true)];
        let pending = || {
            super::pending_body_recovery(
                &engine.media_recovery_hold.lock().unwrap(),
                "acc",
                &slots,
                &headers,
                &[0, 1, 2],
            )
        };
        assert_eq!(pending(), vec![("INBOX".into(), 2), ("INBOX".into(), 3)]);
        assert!(
            !engine
                .media_recovery_hold
                .lock()
                .unwrap()
                .contains_key(&message_key("acc", "INBOX", 2))
        );
        // A later successful FETCH omitting the same UID proves it is gone.
        super::store_thread_bodies(&engine, "acc", &asked, vec![], &attempted, true, &root);
        assert_eq!(engine.media_recovery_hold.lock().unwrap()[&key], None);
        assert_eq!(pending(), vec![("INBOX".into(), 2), ("INBOX".into(), 3)]);
        assert!(
            !engine
                .media_recovery_hold
                .lock()
                .unwrap()
                .contains_key(&message_key("acc", "INBOX", 2))
        );
        // Even a leftover permanent media hold cannot block an uncached body.
        engine
            .media_recovery_hold
            .lock()
            .unwrap()
            .insert(message_key("acc", "INBOX", 2), None);
        assert_eq!(pending(), vec![("INBOX".into(), 2), ("INBOX".into(), 3)]);
        // A subsequent connection failure must not shorten that permanent hold.
        super::store_thread_bodies(&engine, "acc", &asked, vec![], &attempted, false, &root);
        assert_eq!(engine.media_recovery_hold.lock().unwrap()[&key], None);
    }

    #[test]
    fn an_old_body_failure_does_not_stand_in_for_a_new_attempt() {
        let engine = crate::engine::Engine::new(Box::new(Host)).unwrap();
        let key = message_key("acc", "INBOX", 1);
        engine.record_body_failure(key.clone(), "download failed".into());
        assert_eq!(
            engine.take_body_failure(&key).as_deref(),
            Some("download failed")
        );
        assert_eq!(engine.take_body_failure(&key), None);
        if let Some(long_ago) =
            std::time::Instant::now().checked_sub(std::time::Duration::from_secs(3600))
        {
            engine
                .body_fetch_failures
                .lock()
                .unwrap()
                .insert(key.clone(), ("download failed".into(), long_ago));
            assert_eq!(engine.take_body_failure(&key), None);
        }
    }

    #[test]
    fn media_recovery_only_refetches_referenced_files() {
        let root =
            std::env::temp_dir().join(format!("meron-media-recovery-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("acc/INBOX/3")).unwrap();
        std::fs::write(root.join("acc/INBOX/3/0.png"), [1]).unwrap();
        let slot = |key: Option<&str>| {
            let mut message = Message::default();
            message.attachments.push(Attachment {
                filename: "a.png".to_string(),
                mime: "image/png".to_string(),
                size: 1,
                key: key.map(str::to_string),
                oversized: false,
            });
            Slot {
                folder: "INBOX".to_string(),
                media_missing: missing_media_count(&root, &message),
                cached: Some(message),
                complete: true,
            }
        };
        // Pruned, key-less, and present.
        let slots = [
            slot(Some("acc/INBOX/1/0.png")),
            slot(None),
            slot(Some("acc/INBOX/3/0.png")),
        ];
        let headers = [header(1, true), header(2, true), header(3, true)];

        let mut hold = super::MediaRecoveryHold::new();
        assert_eq!(
            messages_missing_media(&hold, "acc", &slots, &headers),
            vec![("INBOX".to_string(), 1), ("INBOX".to_string(), 2)]
        );

        // A refetch that did not bring uid 2's file back is not repeated.
        hold.insert(message_key("acc", "INBOX", 2), None);
        assert_eq!(
            messages_missing_media(&hold, "acc", &slots, &headers),
            vec![("INBOX".to_string(), 1)]
        );

        // A download that failed holds uid 1 for a while, not for good.
        let now = std::time::Instant::now();
        let key = message_key("acc", "INBOX", 1);
        hold.insert(key.clone(), Some(now + std::time::Duration::from_secs(60)));
        assert!(messages_missing_media(&hold, "acc", &slots, &headers).is_empty());
        hold.insert(key, now.checked_sub(std::time::Duration::from_secs(1)));
        assert_eq!(
            messages_missing_media(&hold, "acc", &slots, &headers),
            vec![("INBOX".to_string(), 1)]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shapes_ids_and_flags_from_header() {
        let value = thread_message_json(
            "acc",
            "tid",
            "INBOX",
            &header(10, false),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert_eq!(value["id"], "acc#INBOX#10");
        assert_eq!(value["thread_id"], "tid");
        assert_eq!(value["folder_id"], "INBOX");
        assert_eq!(value["unread"], true);
        assert_eq!(value["subject"], "Hi");
        // The reply rule travels with the message, even before its body caches.
        assert_eq!(value["reply"]["to"], "ann@x.com");
        assert_eq!(value["reply"]["all_adds_recipients"], false);
    }

    #[test]
    fn missing_body_is_flagged_not_dropped() {
        let value = thread_message_json(
            "acc",
            "tid",
            "Sent",
            &header(11, true),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        assert_eq!(value["body_missing"], true);
        assert_eq!(value["body"], "");
        assert_eq!(value["unread"], false);
    }

    #[test]
    fn equal_uids_in_different_folders_have_distinct_ids() {
        let inbox = thread_message_json(
            "acc",
            "tid",
            "INBOX",
            &header(10, false),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );
        let sent = thread_message_json(
            "acc",
            "tid",
            "Sent",
            &header(10, true),
            None,
            &HashSet::new(),
            &HashSet::new(),
        );

        assert_eq!(inbox["id"], "acc#INBOX#10");
        assert_eq!(sent["id"], "acc#Sent#10");
        assert_ne!(inbox["id"], sent["id"]);
        assert_eq!(inbox["thread_id"], sent["thread_id"]);
    }

    #[test]
    fn cursor_distinguishes_equal_uids_in_different_folders() {
        let inbox = MessageHeader {
            uid: 7,
            folder: "INBOX".into(),
            ..Default::default()
        };
        let sent = MessageHeader {
            uid: 7,
            folder: "Sent".into(),
            ..Default::default()
        };

        assert_ne!(thread_cursor(&inbox), thread_cursor(&sent));
        assert_eq!(
            parse_thread_cursor(&thread_cursor(&sent)),
            Some((Some("Sent".into()), 7))
        );
        assert_eq!(parse_thread_cursor("uid:7"), Some((None, 7)));

        let headers = vec![
            inbox,
            MessageHeader {
                uid: 9,
                folder: "Archive".into(),
                ..Default::default()
            },
            sent,
        ];
        let cursor = parse_thread_cursor(&thread_cursor(&headers[2])).unwrap();
        assert_eq!(thread_cursor_position(&headers, &cursor), Some(2));
    }
}
