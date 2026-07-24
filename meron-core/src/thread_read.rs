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

use serde_json::{Value, json};

use crate::engine::{Engine, attach_html};
use crate::{imap, parse, store};

/// Called after a background body fetch stored at least one new message, so
/// the platform can tell its UI to re-read the open thread.
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
    /// Opaque `uid:N` cursor from a previous page's `next_cursor`.
    pub before_cursor: Option<&'a str>,
    /// Attachment/media directory (env-derived on desktop, `data_dir` on mobile).
    pub media_root: PathBuf,
    /// Bake the account's remote-image CSP into `body_html` (the desktop
    /// reader renders it in an iframe as-is). Mobile passes false: its WebView
    /// applies the remote-image policy at render time and wants raw HTML.
    pub bake_html_policy: bool,
}

/// A header's resolved body state: the cached message (if any) and whether the
/// cache can serve it in full — threading headers extracted and inline media
/// still on disk. Incomplete slots are candidates for a refetch but still
/// render from whatever the cache has.
struct Slot {
    folder: String,
    cached: Option<parse::Message>,
    complete: bool,
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
        before_cursor,
        media_root,
        bake_html_policy,
    } = args;

    // Thread view spans folders within the account so the user's own Sent
    // replies appear alongside the inbox messages they thread with.
    //
    // Exception: a synthetic `uid:N` key (a message with no real threading
    // headers — e.g. a freshly-saved draft) is folder-local, because UIDs are
    // folder-scoped. Spanning folders would match an unrelated message that
    // happens to share UID N elsewhere and render the wrong thread.
    let mut headers = if thread_key.starts_with("uid:") {
        let db = engine.db.lock().unwrap();
        let mut headers = store::get_thread_headers(&db, account, folder, thread_key)?;
        // A message opened straight from a notification can have a cached body
        // before its header row is synced; synthesize a header so the read
        // still renders instead of coming back empty.
        if headers.is_empty()
            && let Ok(uid) = thread_key["uid:".len()..].parse::<u32>()
            && store::get_cached_message(&db, account, folder, uid)
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
        let db = engine.db.lock().unwrap();
        store::get_thread_headers_all_folders(&db, account, thread_key)?
    };
    if let Some(filter) = subject_filter {
        headers.retain(|header| store::thread_grouping_subject(&header.subject) == filter);
    }
    let headers = {
        let db = engine.db.lock().unwrap();
        store::collapse_thread_draft_headers(&db, account, folder, headers)?
    };

    // `before_cursor` / `limit` are honored as a date-ordered slice so the
    // cross-folder query stays consistent with the old per-folder pagination
    // contract. The cursor format ("uid:N") indexes into the cross-folder
    // result list rather than a per-folder UID space.
    let before_uid = before_cursor
        .and_then(|s| s.strip_prefix("uid:"))
        .and_then(|s| s.parse::<u32>().ok());
    let (headers, next_cursor) = if let Some(limit) = limit {
        let mut headers = headers;
        if let Some(cursor) = before_uid
            && let Some(idx) = headers.iter().position(|h| h.uid == cursor)
        {
            headers.truncate(idx);
        }
        let total = headers.len();
        let start = total.saturating_sub(limit as usize);
        let page = headers[start..].to_vec();
        let next_cursor = if start > 0 {
            page.first().map(|h| format!("uid:{}", h.uid))
        } else {
            None
        };
        (page, next_cursor)
    } else {
        (headers, None)
    };

    let mut slots = {
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
                // Rows cached before the threading-header extraction landed
                // have an empty `message_id`; refetch them so
                // reply_to/cc/references populate. Real-world mail almost
                // always carries a Message-ID, so emptiness is a reliable
                // "pre-extraction cache" signal.
                let complete = cached.as_ref().is_some_and(|message| {
                    !message.message_id.is_empty()
                        && parse::cached_media_available(&media_root, message)
                });
                Slot {
                    folder: msg_folder,
                    cached,
                    complete,
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
        if limit.is_none() {
            // Full-scan path (markRead, compose threading): every message must
            // be present — a reply built off a body-less copy has no
            // Message-ID and would orphan the thread on the recipient's side.
            // Fetch all gaps before answering.
            if let Err(err) =
                fetch_into_slots(engine, account, &headers, &mut slots, &missing, &media_root).await
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
            let had_any = slots.iter().any(|slot| slot.cached.is_some());
            match fetch_into_slots(
                engine,
                account,
                &headers,
                &mut slots,
                &[newest],
                &media_root,
            )
            .await
            {
                Ok(()) => {}
                // With cached messages to show, a partial thread beats failing
                // the whole read; the background fetch retries.
                Err(err) if had_any => {
                    crate::mlog!(
                        crate::log::Level::Warn,
                        "mail",
                        "inline thread body fetch for {account} uid {}: {err:#}",
                        headers[newest].uid
                    );
                }
                Err(err) => return Err(err),
            }
            let background: Vec<(String, u32)> = missing
                .iter()
                .filter(|&&idx| !slots[idx].complete)
                .map(|&idx| (slots[idx].folder.clone(), headers[idx].uid))
                .collect();
            if !background.is_empty() {
                spawn_fill_thread_bodies(
                    engine,
                    account,
                    thread_key,
                    background,
                    media_root.clone(),
                    on_bodies_fetched,
                );
            }
        }
    }

    let (mine, load_remote_images) = {
        let db = engine.db.lock().unwrap();
        (
            store::self_addrs(&db, account),
            bake_html_policy && store::load_remote_images(&db, account).unwrap_or(false),
        )
    };
    let mut seen_message_ids = HashSet::new();
    let mut messages = Vec::with_capacity(headers.len());
    for (header, slot) in headers.iter().zip(slots) {
        let mut cached = slot.cached;
        if bake_html_policy && let Some(message) = cached.as_mut() {
            attach_html(message, load_remote_images);
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
        messages.push(thread_message_json(
            account,
            thread_id,
            &slot.folder,
            header,
            cached.as_ref(),
            &mine,
        ));
    }

    let mut out = json!({ "messages": messages });
    if let Some(cursor) = next_cursor {
        out.as_object_mut()
            .unwrap()
            .insert("next_cursor".into(), Value::String(cursor));
    }
    Ok(out)
}

/// Fetch the bodies for `indices` from IMAP (grouped by folder so each mailbox
/// is SELECTed once), cache them, and mark their slots complete.
async fn fetch_into_slots(
    engine: &Arc<Engine>,
    account: &str,
    headers: &[imap::MessageHeader],
    slots: &mut [Slot],
    indices: &[usize],
    media_root: &std::path::Path,
) -> anyhow::Result<()> {
    let mut by_folder: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for &idx in indices {
        by_folder
            .entry(slots[idx].folder.clone())
            .or_default()
            .push(headers[idx].uid);
    }
    let fetched =
        fetch_thread_bodies(engine, account, &by_folder, media_root.to_path_buf()).await?;
    let db = engine.db.lock().unwrap();
    for (folder, uid, message) in fetched {
        let _ = store::save_cached_message(&db, account, &folder, uid, &message);
        if let Some(&idx) = indices
            .iter()
            .find(|&&idx| slots[idx].folder == folder && headers[idx].uid == uid)
        {
            slots[idx].cached = Some(message);
            slots[idx].complete = true;
        }
    }
    Ok(())
}

async fn fetch_thread_bodies(
    engine: &Arc<Engine>,
    account: &str,
    by_folder: &BTreeMap<String, Vec<u32>>,
    media_root: PathBuf,
) -> anyhow::Result<Vec<(String, u32, parse::Message)>> {
    engine
        .with_read_session(account, |session| {
            let by_folder = by_folder.clone();
            let media_root = media_root.clone();
            let account = account.to_string();
            Box::pin(async move {
                let mut all = Vec::new();
                for (folder, uids) in &by_folder {
                    let bodies =
                        imap::fetch_bodies(session, folder, uids, media_root.clone(), &account)
                            .await?;
                    for (uid, message) in bodies {
                        all.push((folder.clone(), uid, message));
                    }
                }
                anyhow::Ok(all)
            })
        })
        .await
}

/// Fetch a thread's uncached message bodies in the background, then run the
/// platform hook so the open reader re-reads and the bodies appear. Keeps the
/// per-message IMAP fetches off the read's request path. Deduped per thread
/// via `engine.body_fetches`.
fn spawn_fill_thread_bodies(
    engine: &Arc<Engine>,
    account: &str,
    thread_key: &str,
    missing: Vec<(String, u32)>,
    media_root: PathBuf,
    on_bodies_fetched: Option<BodiesFetchedHook>,
) {
    let key = format!("{account}|{thread_key}");
    if !engine.body_fetches.lock().unwrap().insert(key.clone()) {
        return;
    }
    let engine = engine.clone();
    let account = account.to_string();
    tokio::spawn(async move {
        let mut by_folder: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for (folder, uid) in missing {
            by_folder.entry(folder).or_default().push(uid);
        }
        let mut fetched_any = false;
        match fetch_thread_bodies(&engine, &account, &by_folder, media_root).await {
            Ok(fetched) => {
                let db = engine.db.lock().unwrap();
                for (folder, uid, message) in fetched {
                    let _ = store::save_cached_message(&db, &account, &folder, uid, &message);
                    fetched_any = true;
                }
            }
            Err(err) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail",
                    "background thread body fetch for {account}: {err:#}"
                );
            }
        }
        engine.body_fetches.lock().unwrap().remove(&key);
        if fetched_any && let Some(notify) = on_bodies_fetched {
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
) -> Value {
    let id = format!("{thread_id}#{}", header.uid);
    let from_addr = cached
        .map(|message| message.from_addr.as_str())
        .unwrap_or(header.from_addr.as_str());
    json!({
        "id": id,
        "account_id": account_id,
        "folder_id": folder,
        "thread_id": thread_id,
        // Classified in the core (own address *or* Sent-folder provenance) so
        // both frontends render alias-sent mail as outgoing without knowing
        // the account's aliases.
        "outgoing": store::is_outgoing(mine, folder, from_addr),
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
        // No cached body means the on-demand IMAP fetch failed (auth/network)
        // or is still filling in the background — not that the message is
        // empty. Clients offer a retry / re-read for these.
        "body_missing": cached.is_none(),
        "date": cached.map(|message| message.date).unwrap_or(header.date),
        "unread": !header.seen,
        "starred": header.starred,
        "has_attachments": cached.map(|message| !message.attachments.is_empty()).unwrap_or(false),
        "attachments": cached
            .map(|message| serde_json::to_value(&message.attachments).unwrap_or_else(|_| json!([])))
            .unwrap_or_else(|| json!([])),
    })
}

#[cfg(test)]
mod tests {
    use super::thread_message_json;
    use crate::imap::MessageHeader;
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

    #[test]
    fn shapes_ids_and_flags_from_header() {
        let value = thread_message_json(
            "acc",
            "tid",
            "INBOX",
            &header(10, false),
            None,
            &HashSet::new(),
        );
        assert_eq!(value["id"], "tid#10");
        assert_eq!(value["thread_id"], "tid");
        assert_eq!(value["folder_id"], "INBOX");
        assert_eq!(value["unread"], true);
        assert_eq!(value["subject"], "Hi");
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
        );
        assert_eq!(value["body_missing"], true);
        assert_eq!(value["body"], "");
        assert_eq!(value["unread"], false);
    }
}
