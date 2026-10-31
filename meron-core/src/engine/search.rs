//! Server-side mail search, with cached snapshot pages and starred search.

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Arc;

use crate::search_query::SearchQuery;
use crate::{imap, store};

use super::*;

/// Live hits fetched per batch, newest first by date across the searched
/// folders. IMAP SEARCH answers with the full UID set, and a short or common
/// query ("a", "re") matches most of a mailbox; fetching every header before
/// painting the first page turned one keystroke into a whole-mailbox download.
/// The rest of the set is kept with the snapshot and fetched a batch at a time
/// as paging reaches it.
const LIVE_SEARCH_BATCH: usize = 500;

/// Most cached-only hits one batch appends to a snapshot. Paging past a snapshot
/// that hit this continues through the cache's keyset order, which the snapshot
/// shares.
const SEARCH_SNAPSHOT_MAX: u32 = 2_000;

/// Search `account` for `query` across each of `folders` (typically Inbox +
/// Sent), merging the cached and live-IMAP hits. UIDs are folder-scoped, so
/// results are keyed by (folder, uid) and ordered newest-first by date — the
/// only ordering comparable across mailboxes. Each returned header carries its
/// source `folder` so the bridge can build per-message thread IDs correctly.
///
/// A first page resolves the date of every server hit (from the cache where it
/// can, else a Date-only FETCH), records them with a snapshot, fetches the
/// newest batch by date, and persists the resolved order. Later pages walk the
/// snapshot, fetching further batches when they reach its end, so a
/// sender-controlled Date header cannot move a low UID behind an already-issued
/// cursor and transient disconnects do not invalidate an in-progress search.
pub struct SearchMailPage {
    pub messages: Vec<imap::MessageHeader>,
    pub next_cursor: Option<String>,
    /// The live half failed for at least one folder, so the page may be missing
    /// server-only hits.
    pub incomplete: bool,
}

pub(super) fn cached_search_mail_page(
    conn: &Connection,
    account: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    before_cursor: Option<&crate::thread_list::SearchCursor>,
) -> anyhow::Result<SearchMailPage> {
    let messages =
        store::search_messages_in_folders(conn, account, folders, query, limit, before_cursor)?;
    let next_cursor = store::search_next_cursor(&messages, limit, 0);
    Ok(SearchMailPage {
        messages,
        next_cursor,
        incomplete: false,
    })
}

/// What the local store can say about a later search page.
pub enum SearchContinuation {
    Page(SearchMailPage),
    /// The page runs into server hits not fetched yet. Carries what the store
    /// has, flagged incomplete, for a caller that cannot reach the server.
    NeedsServer(SearchMailPage),
}

/// `SearchCursor::offset` marking a cursor past the end of a snapshot, paging
/// the cache from where the snapshot's last batch stopped taking cached hits.
const CACHE_TAIL_OFFSET: u32 = u32::MAX;

/// A later search page from the local store: the live snapshot the first page
/// recorded, the cached hits a snapshot's cap left out, or keyset paging through
/// cached hits for a cache-only or expired snapshot cursor. Needs no connection
/// or credentials unless it answers [`SearchContinuation::NeedsServer`].
pub fn continue_search_page(
    conn: &Connection,
    account: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    cursor: &crate::thread_list::SearchCursor,
) -> anyhow::Result<SearchContinuation> {
    if let Some(snapshot) = cursor.snapshot.as_deref() {
        if cursor.offset == CACHE_TAIL_OFFSET {
            let mut page = cache_tail_page(conn, account, folders, query, limit, snapshot, cursor)?;
            // A tail entered while the server was unreachable passes over the
            // server hits still pending.
            page.incomplete = store::take_search_pending(conn, snapshot, 0)?.1.is_some();
            return Ok(SearchContinuation::Page(page));
        }
        let page = store::get_search_snapshot_page(
            conn,
            account,
            query,
            folders,
            snapshot,
            cursor.offset,
            limit,
        )?;
        if let Some(page) = page {
            if !page.has_more && page.pending {
                // The offline answer goes on through the cache below where the
                // snapshot stops, so matches already stored stay reachable while
                // the server is not; only server-only hits are missing.
                let resume = offline_cache_resume(conn, snapshot, page.cache_resume.as_deref())?;
                let mut partial = if page.messages.is_empty() {
                    cache_tail_page(conn, account, folders, query, limit, snapshot, &resume)?
                } else {
                    SearchMailPage {
                        messages: page.messages,
                        next_cursor: Some(cache_tail_cursor(snapshot, &resume)),
                        incomplete: false,
                    }
                };
                partial.incomplete = true;
                return Ok(SearchContinuation::NeedsServer(partial));
            }
            if page.has_more {
                let next_cursor =
                    snapshot_next_cursor(&page.messages, true, snapshot, page.next_offset);
                return Ok(SearchContinuation::Page(SearchMailPage {
                    messages: page.messages,
                    next_cursor,
                    incomplete: false,
                }));
            }
            // The snapshot is exhausted. If its last batch cut off cached-only
            // hits, carry on through the cache from where that scan stopped.
            let resume = page
                .cache_resume
                .as_deref()
                .and_then(crate::thread_list::parse_search_cursor);
            let Some(resume) = resume else {
                return Ok(SearchContinuation::Page(SearchMailPage {
                    messages: page.messages,
                    next_cursor: None,
                    incomplete: false,
                }));
            };
            if page.messages.is_empty() {
                return Ok(SearchContinuation::Page(cache_tail_page(
                    conn, account, folders, query, limit, snapshot, &resume,
                )?));
            }
            return Ok(SearchContinuation::Page(SearchMailPage {
                messages: page.messages,
                next_cursor: Some(cache_tail_cursor(snapshot, &resume)),
                incomplete: false,
            }));
        }
    }
    // A cache-only or expired-snapshot cursor resumes with the same keyset
    // ordering rather than accidentally querying the cache's first page.
    Ok(SearchContinuation::Page(cached_search_mail_page(
        conn,
        account,
        folders,
        query,
        limit,
        Some(cursor),
    )?))
}

/// Cached hits after `position` in keyset order, minus those the snapshot
/// already listed (server hits fetched into the cache, and earlier batches'
/// cached hits). The cursor follows the unfiltered page, so a page thinned by
/// the filter still continues.
fn cache_tail_page(
    conn: &Connection,
    account: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    snapshot: &str,
    position: &crate::thread_list::SearchCursor,
) -> anyhow::Result<SearchMailPage> {
    let listed = store::search_snapshot_members(conn, snapshot)?;
    let hits =
        store::search_messages_in_folders(conn, account, folders, query, limit, Some(position))?;
    let next_cursor = hits
        .last()
        .filter(|_| hits.len() == limit as usize)
        .map(|last| cache_tail_cursor(snapshot, &keyset_position(last)));
    Ok(SearchMailPage {
        messages: hits
            .into_iter()
            .filter(|hit| !listed.contains(&(hit.folder.clone(), hit.uid)))
            .collect(),
        next_cursor,
        incomplete: false,
    })
}

/// Where an offline page resumes the cache: every cached hit newer than the
/// newest pending server hit is already listed, unless a batch's cap cut them
/// off first. Hits dated the same as the newest pending one may be either, so the
/// tail starts at that date and relies on its listed filter.
fn offline_cache_resume(
    conn: &Connection,
    snapshot: &str,
    cache_resume: Option<&str>,
) -> anyhow::Result<crate::thread_list::SearchCursor> {
    if let Some(resume) = cache_resume.and_then(crate::thread_list::parse_search_cursor) {
        return Ok(resume);
    }
    let head = store::take_search_pending(conn, snapshot, 0)?.1;
    Ok(crate::thread_list::SearchCursor {
        date: head.map_or(i64::MAX, |head| head.date.saturating_add(1)),
        uid: 0,
        folder: String::new(),
        scanned: 0,
        snapshot: None,
        offset: 0,
    })
}

fn cache_tail_cursor(snapshot: &str, position: &crate::thread_list::SearchCursor) -> String {
    crate::thread_list::format_search_cursor(&crate::thread_list::SearchCursor {
        snapshot: Some(snapshot.to_string()),
        offset: CACHE_TAIL_OFFSET,
        ..position.clone()
    })
}

fn keyset_position(header: &imap::MessageHeader) -> crate::thread_list::SearchCursor {
    crate::thread_list::SearchCursor {
        date: header.date,
        uid: header.uid,
        folder: header.folder.clone(),
        scanned: 0,
        snapshot: None,
        offset: 0,
    }
}

pub(super) fn record_search_folder_result<T>(
    folder: &str,
    result: anyhow::Result<T>,
    successes: &mut Vec<(String, T)>,
    failures: &mut Vec<(String, String)>,
) {
    match result {
        Ok(value) => successes.push((folder.to_string(), value)),
        Err(err) => failures.push((folder.to_string(), format!("{err:#}"))),
    }
}

/// Run `op` on each folder over one shared session, keeping each folder's
/// outcome: a stale/missing Sent folder must not discard a successful Inbox
/// search (or vice versa). `None` when no session could be had at all.
type FolderOp<T> = for<'a> fn(
    &'a mut imap::Session,
    &'a str,
    &'a [u32],
    &'a str,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 'a>,
>;

async fn per_folder_live<T: Send + 'static>(
    engine: &Arc<Engine>,
    account: &str,
    query: &str,
    folders: Vec<(String, Vec<u32>)>,
    op: FolderOp<T>,
) -> Option<Vec<(String, T)>> {
    let live = engine
        .with_read_session(account, |session| {
            let folders = folders.clone();
            let query = query.to_string();
            Box::pin(async move {
                let mut successes = Vec::new();
                let mut failures = Vec::new();
                for (folder, uids) in &folders {
                    let result = op(session, folder, uids, &query).await;
                    record_search_folder_result(folder, result, &mut successes, &mut failures);
                }
                anyhow::Ok((successes, failures))
            })
        })
        .await;
    let (successes, failures) = match live {
        Ok(result) => result,
        Err(err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.search",
                "live search failed for account={account}: {err:#}"
            );
            return None;
        }
    };
    if !failures.is_empty() {
        // The operation deliberately preserved successful folders, so the pool
        // saw an overall success. Do not retain a socket that may have produced
        // an I/O failure partway through the per-folder loop.
        engine.clear_pool(account);
    }
    for (folder, error) in failures {
        crate::mlog!(
            crate::log::Level::Warn,
            "mail.search",
            "live search failed for account={account} folder={folder}: {error}"
        );
    }
    Some(successes)
}

/// Fetch the snapshot's next batch of server hits and append it, together with
/// the cached-only hits it covers, in date order. `Ok(false)` means no pending
/// hit could be placed (every folder's fetch failed, or those that failed hold
/// the newest hits), so no progress was made; `Ok(true)` with `incomplete` set
/// means some folder failed and keeps its pending hits for a later retry.
async fn append_search_batch(
    engine: &Arc<Engine>,
    account: &str,
    folders: &[String],
    query: &str,
    token: &str,
    incomplete: &mut bool,
) -> anyhow::Result<bool> {
    let _snapshot_guard = MessageSyncGuard::begin(engine)?;
    let (batch, next) =
        store::take_search_pending(&engine.db.lock().unwrap(), token, LIVE_SEARCH_BATCH)?;
    let mut requests: Vec<(String, Vec<u32>)> = Vec::new();
    for hit in &batch {
        match requests
            .iter_mut()
            .find(|(folder, _)| *folder == hit.folder)
        {
            Some((_, uids)) => uids.push(hit.uid),
            None => requests.push((hit.folder.clone(), vec![hit.uid])),
        }
    }
    let fetched = if requests.is_empty() {
        Vec::new()
    } else {
        let Some(fetched) = per_folder_live(
            engine,
            account,
            query,
            requests.clone(),
            |session, folder, uids, _| {
                Box::pin(async move {
                    let mut headers = imap::fetch_headers_by_uid(session, folder, uids).await?;
                    for header in &mut headers {
                        header.folder = folder.to_string();
                    }
                    Ok(headers)
                })
            },
        )
        .await
        else {
            *incomplete = true;
            return Ok(false);
        };
        if fetched.is_empty() {
            *incomplete = true;
            return Ok(false);
        }
        if fetched.len() < requests.len() {
            *incomplete = true;
        }
        fetched
    };

    // Each step takes the store lock on its own, so sync, thread reads and flag
    // writes can interleave with a large search instead of queueing behind it.
    for (folder, headers) in &fetched {
        store::upsert_messages(&engine.db.lock().unwrap(), account, folder, headers)?;
    }

    let fetched_folders = fetched
        .iter()
        .map(|(folder, _)| folder.as_str())
        .collect::<std::collections::HashSet<_>>();
    let (frontier, done) = batch_placement(&batch, next.as_ref(), &fetched_folders);
    let done_keys = done
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let mut listed = store::search_snapshot_members(&engine.db.lock().unwrap(), token)?;
    let mut messages = Vec::new();
    let parsed = SearchQuery::parse(query);
    for header in fetched.iter().flat_map(|(_, headers)| headers) {
        let key = (header.folder.clone(), header.uid);
        if done_keys.contains(&key) && live_hit_matches(&parsed, header) && listed.insert(key) {
            messages.push(header.clone());
        }
    }
    // The cached scan's own position when it hits the cap. It is where paging
    // past the snapshot resumes the cache; the snapshot's last row will not do,
    // since a server hit can be older than cached hits the cap skipped.
    let mut cache_resume: Option<String> = None;
    let mut cached_taken = 0u32;
    let mut last_taken: Option<crate::thread_list::SearchCursor> = None;
    let mut cursor: Option<crate::thread_list::SearchCursor> = None;
    'walk: loop {
        const WALK_PAGE: u32 = 500;
        let page = store::search_messages_in_folders(
            &engine.db.lock().unwrap(),
            account,
            folders,
            query,
            WALK_PAGE,
            cursor.as_ref(),
        )?;
        for message in &page {
            if frontier.is_some_and(|frontier| {
                (message.date, message.uid, message.folder.as_str()) < frontier
            }) {
                break 'walk;
            }
            if listed.contains(&(message.folder.clone(), message.uid)) {
                continue;
            }
            if cached_taken >= SEARCH_SNAPSHOT_MAX {
                // Resume right after the last hit taken, so nothing between it
                // and this one is skipped.
                cache_resume = last_taken
                    .as_ref()
                    .map(crate::thread_list::format_search_cursor);
                break 'walk;
            }
            listed.insert((message.folder.clone(), message.uid));
            cached_taken += 1;
            last_taken = Some(keyset_position(message));
            messages.push(message.clone());
        }
        let Some(last) = page.last().filter(|_| page.len() == WALK_PAGE as usize) else {
            break;
        };
        cursor = Some(keyset_position(last));
    }
    if !requests.is_empty() && done.is_empty() {
        // A failed folder holds hits newer than all the fetched ones, so
        // nothing can be placed; retrying would fetch the same batch again.
        return Ok(false);
    }
    store::sort_search_hits_all(&mut messages);
    store::finish_search_batch(
        &engine.db.lock().unwrap(),
        token,
        account,
        &done,
        &messages,
        cache_resume.as_deref(),
    )?;
    Ok(true)
}

/// Whether a server hit really matches: outside Gmail, `has:attachment` is only
/// approximated by SEARCH, and a hit's files are known once it is fetched.
/// `None` means they weren't, so the hit stands. Gmail's own answer is left
/// alone: it reads the whole query, as in `has:attachment OR from:ann`, which
/// [`SearchQuery`] does not.
///
/// A CID image counts as inline until its body is read, as for the attachments
/// filter and the list card's paperclip. BODYSTRUCTURE can't tell one the HTML
/// shows, and keeping them all would list every newsletter with a logo.
pub(super) fn live_hit_matches(query: &SearchQuery, header: &imap::MessageHeader) -> bool {
    !query.has_attachment
        || header.gmail_msg_id.is_some()
        || header.files.as_ref().is_none_or(|files| !files.is_empty())
}

/// Which of a fetched `batch` can be placed, and the frontier below which
/// nothing can be yet. Every pending hit's date is known, so the frontier is
/// exact: the newest hit still pending, being `next` after the batch or any
/// hit of a folder whose fetch failed (not in `fetched`). Batch hits newer than
/// it are done; a fetched hit older than a failed folder's stays pending, to be
/// placed after it. `None` when nothing is left pending.
pub(super) fn batch_placement<'a>(
    batch: &'a [store::PendingSearchHit],
    next: Option<&'a store::PendingSearchHit>,
    fetched: &std::collections::HashSet<&str>,
) -> (Option<store::SearchHitKey<'a>>, Vec<(String, u32)>) {
    let frontier = batch
        .iter()
        .filter(|hit| !fetched.contains(hit.folder.as_str()))
        .chain(next)
        .map(store::PendingSearchHit::key)
        .max();
    let done = batch
        .iter()
        .filter(|hit| fetched.contains(hit.folder.as_str()))
        .filter(|hit| frontier.is_none_or(|frontier| hit.key() > frontier))
        .map(|hit| (hit.folder.clone(), hit.uid))
        .collect();
    (frontier, done)
}

/// Every server hit of `searched` with its date, from the cache where the row
/// is stored and otherwise from a Date-only FETCH. UID order follows mailbox
/// insertion, not the Date header, so an imported or moved message can be the
/// newest match under the lowest UID; ordering batches by date needs the whole
/// set dated first. Hits whose date could not be had are left out and set
/// `incomplete`.
async fn date_search_hits(
    engine: &Arc<Engine>,
    account: &str,
    query: &str,
    searched: &[(String, Vec<u32>)],
    incomplete: &mut bool,
) -> anyhow::Result<Vec<store::PendingSearchHit>> {
    let mut hits = Vec::new();
    let mut undated = Vec::new();
    for (folder, uids) in searched {
        let dates = store::cached_message_dates(&engine.db.lock().unwrap(), account, folder, uids)?;
        let missing = uids
            .iter()
            .copied()
            .filter(|uid| !dates.contains_key(uid))
            .collect::<Vec<_>>();
        hits.extend(
            dates
                .into_iter()
                .map(|(uid, date)| store::PendingSearchHit {
                    folder: folder.clone(),
                    uid,
                    date,
                }),
        );
        if !missing.is_empty() {
            undated.push((folder.clone(), missing));
        }
    }
    if undated.is_empty() {
        return Ok(hits);
    }
    let requested = undated.len();
    let dated = per_folder_live(
        engine,
        account,
        query,
        undated,
        |session, folder, uids, _| Box::pin(imap::fetch_dates_by_uid(session, folder, uids)),
    )
    .await
    .unwrap_or_default();
    if dated.len() < requested {
        *incomplete = true;
    }
    for (folder, dates) in dated {
        hits.extend(
            dates
                .into_iter()
                .map(|(uid, date)| store::PendingSearchHit {
                    folder: folder.clone(),
                    uid,
                    date,
                }),
        );
    }
    Ok(hits)
}

pub async fn search_mail_messages(
    engine: &Arc<Engine>,
    account: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    before_cursor: Option<&crate::thread_list::SearchCursor>,
) -> anyhow::Result<SearchMailPage> {
    let mut incomplete = false;
    let cursor = match before_cursor {
        Some(cursor) => cursor.clone(),
        None => {
            // SEARCH every folder for its full UID set; fetching happens in
            // batches below, so a broad query costs one batch per page.
            let requests = folders
                .iter()
                .map(|folder| (folder.clone(), Vec::new()))
                .collect::<Vec<_>>();
            let searched = per_folder_live(
                engine,
                account,
                query,
                requests,
                |session, folder, _, query| Box::pin(imap::search_uids(session, folder, query)),
            )
            .await
            .unwrap_or_default();
            if searched.is_empty() {
                let db = engine.db.lock().unwrap();
                let mut page = cached_search_mail_page(&db, account, folders, query, limit, None)?;
                page.incomplete = true;
                return Ok(page);
            }
            incomplete = searched.len() < folders.len();
            let pending =
                date_search_hits(engine, account, query, &searched, &mut incomplete).await?;
            let token = store::create_search_snapshot(
                &engine.db.lock().unwrap(),
                account,
                query,
                folders,
                &pending,
            )?;
            // The first batch runs even with nothing pending, to take in the
            // cached-only hits. If its headers cannot be fetched, the snapshot
            // would page as empty; answer from the cache instead, so a failed
            // FETCH does not replace the local matches already on screen.
            if !append_search_batch(engine, account, folders, query, &token, &mut incomplete)
                .await?
            {
                let db = engine.db.lock().unwrap();
                let mut page = cached_search_mail_page(&db, account, folders, query, limit, None)?;
                page.incomplete = true;
                return Ok(page);
            }
            crate::thread_list::SearchCursor {
                date: 0,
                uid: 0,
                folder: String::new(),
                scanned: 0,
                snapshot: Some(token),
                offset: 0,
            }
        }
    };

    // A folder that failed a batch is not retried within this request: its hits
    // bound what later batches can place, so each retry would mostly refetch
    // it just to fail again. The partial page is answered instead.
    let mut batch_failed = false;
    loop {
        let continuation = {
            let db = engine.db.lock().unwrap();
            continue_search_page(&db, account, folders, query, limit, &cursor)?
        };
        match continuation {
            SearchContinuation::Page(mut page) => {
                page.incomplete |= incomplete;
                return Ok(page);
            }
            SearchContinuation::NeedsServer(partial) => {
                if batch_failed {
                    return Ok(partial);
                }
                let token = cursor.snapshot.as_deref().unwrap_or_default();
                let progressed =
                    append_search_batch(engine, account, folders, query, token, &mut batch_failed)
                        .await?;
                incomplete |= batch_failed;
                if !progressed {
                    return Ok(partial);
                }
            }
        }
    }
}

pub(super) fn snapshot_next_cursor(
    messages: &[imap::MessageHeader],
    has_more: bool,
    snapshot: &str,
    offset: u32,
) -> Option<String> {
    let header = has_more.then(|| messages.last()).flatten()?;
    Some(crate::thread_list::format_search_cursor(
        &crate::thread_list::SearchCursor {
            date: header.date,
            uid: header.uid,
            folder: header.folder.clone(),
            scanned: 0,
            snapshot: Some(snapshot.to_string()),
            offset,
        },
    ))
}

/// The mailboxes a starred view reads. Gmail keeps every starred message in
/// its Starred mailbox (special-use `\Flagged`, localized name), so that one
/// folder answers the whole account. Failing that, All Mail holds each message
/// exactly once; scanning every label instead listed a message once per label
/// it carried and SELECTed every folder. Other servers flag per mailbox, so the
/// open one is the answer.
pub async fn starred_search_folders(
    engine: &Arc<Engine>,
    account: &str,
    requested: &str,
) -> Vec<String> {
    let is_gmail = {
        let accounts = engine.accounts.lock().await;
        accounts
            .get(account)
            .map(|creds| creds.auth_type == "gmail_oauth")
            .unwrap_or(false)
    };
    if !is_gmail {
        return vec![requested.to_string()];
    }
    let find_starred = |folders: &[imap::Folder]| {
        folders
            .iter()
            .find(|folder| {
                let name = folder.name.as_str();
                folder.special_use.as_deref() == Some("flagged")
                    || name.eq_ignore_ascii_case("starred")
                    || name.eq_ignore_ascii_case("[gmail]/starred")
                    || name.eq_ignore_ascii_case("[google mail]/starred")
            })
            .map(|folder| folder.name.clone())
    };
    let mut folders = store::get_folders(&engine.db.lock().unwrap(), account).unwrap_or_default();
    if let Some(folder) = find_starred(&folders) {
        return vec![folder];
    }
    // The cache predates special-use `\Flagged` or has never listed folders.
    let fresh = engine
        .with_read_session(account, |session| {
            Box::pin(async move { imap::list_folders(session).await })
        })
        .await;
    match fresh {
        Ok(fresh) => {
            if let Ok(db) = engine.db.lock() {
                let _ = store::upsert_folders(&db, account, &fresh);
            }
            folders = fresh;
            if let Some(folder) = find_starred(&folders) {
                return vec![folder];
            }
        }
        Err(err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.starred",
                "folder LIST failed for account={account}: {err:#}"
            );
        }
    }
    let all_mail = folders
        .into_iter()
        .find(|folder| folder.special_use.as_deref() == Some("all"))
        .map(|folder| folder.name);
    vec![all_mail.unwrap_or_else(|| requested.to_string())]
}

/// Starred messages across `folders`, newest first. The cache answers at once;
/// `refresh` asks the server for each folder's full `\Flagged` set, fetches the
/// newest of those, and drops (and unflags) cached rows the server no longer
/// has flagged — a star removed in another client otherwise lingered until the
/// next flag sync.
pub async fn search_starred_mail_messages(
    engine: &Arc<Engine>,
    account: &str,
    folders: &[String],
    limit: u32,
    refresh: bool,
) -> anyhow::Result<Vec<imap::MessageHeader>> {
    let mut by_key: HashMap<(String, u32), imap::MessageHeader> = HashMap::new();
    for folder in folders {
        for mut message in store::get_starred(&engine.db.lock().unwrap(), account, folder, limit)? {
            message.folder = folder.clone();
            by_key.insert((folder.clone(), message.uid), message);
        }
    }

    if refresh {
        let _snapshot_guard = MessageSyncGuard::begin(engine)?;
        // Best-effort per folder: a single folder's failure is logged and
        // skipped, so the closure always succeeds (no stale-retry needed here).
        let fetch_cap = (limit as usize).max(LIVE_SEARCH_BATCH);
        let server = engine
            .with_read_session(account, |session| {
                let folders = folders.to_vec();
                Box::pin(async move {
                    let mut found = Vec::new();
                    for folder in &folders {
                        let result = async {
                            let flagged = imap::search_starred_uids(session, folder).await?;
                            // Fetch the newest UIDs, then order by date below;
                            // the cap bounds a mailbox with thousands of stars.
                            let newest = &flagged[..flagged.len().min(fetch_cap)];
                            let headers =
                                imap::fetch_headers_by_uid(session, folder, newest).await?;
                            anyhow::Ok((flagged, headers))
                        }
                        .await;
                        match result {
                            Ok((flagged, headers)) => {
                                found.push((folder.clone(), flagged, headers))
                            }
                            Err(err) => crate::mlog!(
                                crate::log::Level::Warn,
                                "mail.starred",
                                "starred search failed for folder={folder}: {err:#}"
                            ),
                        }
                    }
                    anyhow::Ok(found)
                })
            })
            .await;

        match server {
            Ok(found) => {
                for (folder, flagged, mut headers) in found {
                    let flagged: std::collections::HashSet<u32> = flagged.into_iter().collect();
                    let unstarred = by_key
                        .keys()
                        .filter(|(key_folder, uid)| *key_folder == folder && !flagged.contains(uid))
                        .cloned()
                        .collect::<Vec<_>>();
                    for header in &mut headers {
                        header.folder = folder.clone();
                        header.starred = true;
                    }
                    {
                        let db = engine.db.lock().unwrap();
                        for key in &unstarred {
                            store::update_message_starred(&db, account, &folder, key.1, false)?;
                        }
                        store::upsert_messages(&db, account, &folder, &headers)?;
                    }
                    for key in unstarred {
                        by_key.remove(&key);
                    }
                    for header in headers {
                        by_key.insert((folder.clone(), header.uid), header);
                    }
                }
            }
            Err(err) => crate::mlog!(
                crate::log::Level::Warn,
                "mail.starred",
                "starred search connect failed for account={account}: {err:#}"
            ),
        }
    }

    let mut messages = by_key.into_values().collect::<Vec<_>>();
    store::sort_search_hits(&mut messages, limit);
    Ok(messages)
}
