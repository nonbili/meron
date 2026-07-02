use super::*;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

const MOBILE_BODY_PREFETCH_DAYS: u32 = 7;
const MOBILE_BODY_PREFETCH_MAX: usize = 20;
const MOBILE_BODY_PREFETCH_TIMEOUT_SECS: u64 = 20;

fn prefetch_mobile_bodies(
    data_dir: &str,
    engine: &std::sync::Arc<crate::engine::Engine>,
    account_id: &str,
    folder: &str,
) {
    let options = crate::engine::BodyPrefetchOptions {
        days: MOBILE_BODY_PREFETCH_DAYS,
        max_count: Some(MOBILE_BODY_PREFETCH_MAX),
        media_root: std::path::PathBuf::from(data_dir).join("attachments"),
    };
    let result = crate::ffi::engine_block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(MOBILE_BODY_PREFETCH_TIMEOUT_SECS),
            crate::engine::prefetch_bodies_with_options(engine, account_id, folder, options),
        )
        .await
        .map_err(|_| anyhow::anyhow!("timed out"))?
    });
    match result {
        Ok(fetched) => crate::mlog!(
            crate::log::Level::Info,
            "mail.sync",
            "Prefetched {fetched} message bodies for {account_id}/{folder}"
        ),
        Err(err) => crate::mlog!(
            crate::log::Level::Warn,
            "mail.sync",
            "Body prefetch failed for {account_id}/{folder}: {err}"
        ),
    }
}

/// Inbox syncs whose non-gating tail (`finish_mobile_sync`) is still running on
/// a detached thread, keyed by `account/folder`, so overlapping pull-to-refresh
/// calls don't stack a second prefetch behind the first.
static DEFERRED_SYNC_TAILS: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Claim the deferred-tail slot for `account/folder`. Returns false when a
/// tail for that mailbox is already running, in which case the caller skips.
fn claim_sync_tail(key: &str) -> bool {
    DEFERRED_SYNC_TAILS.lock().unwrap().insert(key.to_string())
}

fn release_sync_tail(key: &str) {
    DEFERRED_SYNC_TAILS.lock().unwrap().remove(key);
}

/// Post-sync work that doesn't gate the inbox list: body prefetch plus Sent and
/// Drafts header syncs (so cross-folder conversation views read from the local
/// store).
fn finish_mobile_sync(
    data_dir: &str,
    engine: &std::sync::Arc<crate::engine::Engine>,
    account_id: &str,
    folder: &str,
    resolved_sent: Option<String>,
    limit: u32,
) {
    prefetch_mobile_bodies(data_dir, engine, account_id, folder);

    // Also pull the Sent folder so outgoing messages — including ones sent from
    // another client — surface in the cross-folder conversation view. The
    // per-folder recent sync only ever covers the open folder.
    if let Some(sent) = resolved_sent
        && !sent.eq_ignore_ascii_case(folder)
    {
        match crate::ffi::engine_block_on(crate::engine::sync_messages(
            engine, account_id, &sent, limit,
        )) {
            Ok(sent_count) => crate::mlog!(
                crate::log::Level::Info,
                "mail.sync",
                "Sent sync account={account_id} sent={sent} synced={sent_count}"
            ),
            Err(err) => crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "Sent sync failed for {account_id}: {err}"
            ),
        }
    }

    // Same for Drafts: keeping its headers cached means opening a thread can
    // surface a reply drafted on another client from the local store alone,
    // with no per-thread network check on the read path.
    if let Some(drafts) = crate::engine::cached_drafts_folder(engine, account_id, folder) {
        match crate::ffi::engine_block_on(crate::engine::sync_messages(
            engine, account_id, &drafts, limit,
        )) {
            Ok(drafts_count) => crate::mlog!(
                crate::log::Level::Info,
                "mail.sync",
                "Drafts sync account={account_id} drafts={drafts} synced={drafts_count}"
            ),
            Err(err) => crate::mlog!(
                crate::log::Level::Warn,
                "mail.sync",
                "Drafts sync failed for {account_id}: {err}"
            ),
        }
    }
}

/// Run the non-gating sync tail. When the caller opted in via `defer_tail`
/// (the foreground UI, whose process outlives the call) it runs on a detached
/// thread so `mail.sync` returns as soon as the inbox headers are stored — the
/// tail costs body prefetch alone up to 20s — and emits `mail.synced` when done
/// so the UI re-reads the store. Everything else (background workers, the IDLE
/// watcher) keeps it inline: an OS-granted execution window ends when the
/// worker returns, so a detached thread could be frozen mid-fetch.
fn run_mobile_sync_tail(
    data_dir: &str,
    engine: &std::sync::Arc<crate::engine::Engine>,
    account_id: &str,
    folder: &str,
    resolved_sent: Option<String>,
    limit: u32,
    defer_tail: bool,
) {
    if !defer_tail {
        finish_mobile_sync(data_dir, engine, account_id, folder, resolved_sent, limit);
        return;
    }
    let key = format!("{account_id}/{folder}");
    if !claim_sync_tail(&key) {
        return;
    }
    let data_dir = data_dir.to_string();
    let engine = engine.clone();
    let account_id = account_id.to_string();
    let folder = folder.to_string();
    std::thread::spawn(move || {
        finish_mobile_sync(&data_dir, &engine, &account_id, &folder, resolved_sent, limit);
        release_sync_tail(&key);
        // Not `mail.synced`: that handler reloads the thread list (resetting
        // pagination), and the tail never changes the open folder's headers —
        // only bodies and Sent/Drafts, which affect the open thread at most.
        crate::ffi::emit_event(
            "mail.tailSynced",
            json!({ "account": account_id, "folder": folder }),
        );
    });
}

pub(crate) fn send_mobile_message(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let to = req_str(params, "to")?;
    let cc = opt_str(params, "cc");
    let bcc = opt_str(params, "bcc");
    let subject = opt_str(params, "subject");
    let body = opt_str(params, "body");
    let html = opt_str(params, "html");
    let in_reply_to = opt_str(params, "in_reply_to");
    let references = opt_str(params, "references");
    let reply_to = opt_str(params, "reply_to");
    let message_id = opt_str(params, "message_id");
    let requested_from = opt_str(params, "from");
    let attachments = parse_mobile_attachments(params)?;

    // Route through the shared Engine: reuses the warm session pool while
    // foreground (no per-send TLS+LOGIN) and runs the same Sent finalization as
    // desktop. Provider defaults decide whether this APPENDs or only refreshes
    // the server-created Sent copy.
    let engine = crate::ffi::engine_for(data_dir)?;
    let creds = crate::ffi::engine_block_on(engine.ensure_valid_creds(&account_id))?;
    if account_needs_reconnect(&creds) {
        return Err(format!("account needs reconnect: {account_id}"));
    }
    let (from_addr, sender_name) = {
        let conn = engine.db.lock().unwrap();
        resolve_mobile_send_from(&conn, &account_id, &creds, &requested_from)
    };
    let raw = crate::ffi::engine_block_on(smtp::send(
        &creds,
        &from_addr,
        &sender_name,
        &to,
        &cc,
        &bcc,
        &subject,
        &body,
        &html,
        &attachments,
        &in_reply_to,
        &references,
        &reply_to,
        &message_id,
    ))?;
    let sent_bytes = raw.len();
    if let Err(err) =
        crate::ffi::engine_block_on(crate::engine::append_to_sent(&engine, &account_id, &raw))
    {
        crate::mlog!(
            crate::log::Level::Warn,
            "mail.send",
            "APPEND to Sent failed for {account_id}: {err}"
        );
    }
    Ok(json!({ "ok": true, "sent_bytes": sent_bytes }))
}

pub(crate) fn save_mobile_draft(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let to = opt_str(params, "to");
    let cc = opt_str(params, "cc");
    let bcc = opt_str(params, "bcc");
    let subject = opt_str(params, "subject");
    let body = opt_str(params, "body");
    let html = opt_str(params, "html");
    let in_reply_to = opt_str(params, "in_reply_to");
    let references = opt_str(params, "references");
    let reply_to = opt_str(params, "reply_to");
    let draft_id = req_str(params, "draft_id")?;
    let requested_from = opt_str(params, "from");
    let attachments = parse_mobile_attachments(params)?;

    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let (from_addr, sender_name) =
            resolve_mobile_send_from(&conn, &account_id, &creds, &requested_from);
        let raw = smtp::build_message(
            &sender_name,
            &from_addr,
            &to,
            &cc,
            &bcc,
            true,
            &subject,
            &body,
            &html,
            &attachments,
            &in_reply_to,
            &references,
            &reply_to,
            &draft_id,
        )
        .map_err(|err| format!("{err:#}"))?;
        let saved_id = draft_id.clone();
        let saved_bytes = raw.len();
        crate::ffi::engine_block_on(engine.with_write_session(&account_id, move |session| {
            let raw = raw.clone();
            let saved_id = saved_id.clone();
            Box::pin(async move {
                let drafts = imap::find_drafts_folder(session)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                imap::replace_draft(session, &drafts, &raw, &saved_id).await?;
                anyhow::Ok(())
            })
        }))?;
        Ok(json!({ "ok": true, "draft_id": draft_id, "saved_bytes": saved_bytes }))
    })
}

pub(crate) fn discard_mobile_draft(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let draft_id = opt_str(params, "draft_id");
    if draft_id.trim().is_empty() {
        return Ok(json!({ "ok": true, "deleted": 0, "permanent": true }));
    }
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let deleted =
            crate::ffi::engine_block_on(engine.with_write_session(&account_id, move |session| {
                let draft_id = draft_id.clone();
                Box::pin(async move {
                    let drafts = imap::find_drafts_folder(session)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                    let deleted = imap::discard_draft(session, &drafts, &draft_id).await?;
                    anyhow::Ok(deleted)
                })
            }))?;
        Ok(json!({ "ok": true, "deleted": deleted, "permanent": true }))
    })
}

pub(crate) fn sync_mobile_mail(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .unwrap_or_else(|| "INBOX".to_string());
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(50)
        .clamp(1, 500);
    let sync_folders = params
        .get("folders")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let defer_tail = params
        .get("defer_tail")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // RSS accounts sync through the feed pipeline, not the IMAP engine.
    let is_rss = with_mobile_db(data_dir, |conn| {
        Ok(json!(is_rss_account(&conn, &account_id)?))
    })?;
    if is_rss.as_bool() == Some(true) {
        return with_mobile_db(data_dir, |conn| sync_mobile_rss_with_conn(conn, account_id));
    }

    // Route IMAP sync through the shared Engine: reuses the warm session pool
    // while foreground and runs the same ops as desktop (sync_folders +
    // sync_messages do fetch/upsert/prune/flag-reconcile into the store), so the
    // two platforms can't drift.
    let engine = crate::ffi::engine_for(data_dir)?;
    let started = std::time::Instant::now();
    let creds = crate::ffi::engine_block_on(engine.ensure_valid_creds(&account_id))?;
    if account_needs_reconnect(&creds) {
        return Err(format!("account needs reconnect: {account_id}"));
    }
    let creds_ms = started.elapsed().as_millis();

    let folders_started = std::time::Instant::now();
    let folders_count = if sync_folders {
        crate::ffi::engine_block_on(crate::engine::sync_folders(&engine, &account_id))?.len()
    } else {
        0
    };
    let folders_ms = folders_started.elapsed().as_millis();
    let messages_started = std::time::Instant::now();
    let count = crate::ffi::engine_block_on(crate::engine::sync_messages(
        &engine,
        &account_id,
        &folder,
        limit,
    ))?;
    let messages_ms = messages_started.elapsed().as_millis();
    let resolved_sent = crate::engine::cached_sent_folder(&engine, &account_id, &folder);
    crate::mlog!(
        crate::log::Level::Info,
        "mail.sync",
        "account={account_id} folder={folder} synced={count} folders={folders_count} sent={resolved_sent:?} creds_ms={creds_ms} folders_ms={folders_ms} messages_ms={messages_ms}"
    );
    run_mobile_sync_tail(
        data_dir,
        &engine,
        &account_id,
        &folder,
        resolved_sent,
        limit,
        defer_tail,
    );
    Ok(json!({
        "ok": true,
        "account": account_id,
        "folder": folder,
        "synced": count,
        "folders": folders_count,
    }))
}

pub(crate) fn list_mobile_folders(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            let folders = rss::folders(&conn, &account_id).map_err(|err| format!("{err:#}"))?;
            return Ok(json!({ "folders": folders }));
        }
        let folders = store::get_folders(&conn, &account_id).map_err(|err| err.to_string())?;
        let folders = folders
            .into_iter()
            .map(|folder| {
                let role = if folder.name.eq_ignore_ascii_case("INBOX") {
                    "inbox"
                } else if looks_like_archive_folder(&folder.name) {
                    "archive"
                } else {
                    "folder"
                };
                json!({
                    "id": folder.name,
                    "account_id": account_id,
                    "name": folder.name,
                    "role": role,
                    "delimiter": folder.delimiter.unwrap_or_default(),
                    "unread": folder.unread,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "folders": folders }))
    })
}

pub(crate) fn create_mobile_folder(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    let name = req_str(params, "name")?.trim().to_string();
    if name.is_empty() {
        return Err("Folder name is required".to_string());
    }
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("RSS accounts do not support folders".to_string());
        }
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let folders = run_mobile_async(async {
            let mut session = imap::connect(&creds).await?;
            imap::create_folder(&mut session, &name).await?;
            let folders = imap::list_folders(&mut session).await?;
            let _ = session.logout().await;
            anyhow::Ok(folders)
        })?;
        store::upsert_folders(&conn, &account_id, &folders).map_err(|err| err.to_string())?;
        list_mobile_folders(data_dir, params)
    })
}

pub(crate) fn list_mobile_threads(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    let folder_id = params
        .get("folder_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(canon_folder)
        .unwrap_or_else(|| "INBOX".to_string());
    let query = params
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let filter = params
        .get("filter")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let before_cursor = params
        .get("before_cursor")
        .and_then(Value::as_str)
        .and_then(parse_mail_cursor);

    let is_rss = with_mobile_db(data_dir, |conn| {
        Ok(json!(is_rss_account(&conn, &account_id)?))
    })?
    .as_bool()
    .unwrap_or(false);
    if is_rss {
        return with_mobile_db(data_dir, |conn| {
            let threads =
                rss::recent(&conn, &account_id, &query, 50).map_err(|err| format!("{err:#}"))?;
            Ok(json!({ "threads": threads }))
        });
    }

    let (mut messages, next_cursor) = if query.is_empty() {
        get_cached_mobile_mail_page(
            data_dir,
            &account_id,
            &folder_id,
            50,
            before_cursor,
            filter == "unread",
        )?
    } else {
        match search_live_mobile_mail_messages(data_dir, &account_id, &folder_id, &query, 50) {
            Ok(messages) => (messages, None),
            Err(err) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail.search",
                    "live search failed for account={account_id} folder={folder_id}: {err}"
                );
                (
                    search_cached_mobile_mail_messages(
                        data_dir,
                        &account_id,
                        &folder_id,
                        &query,
                        50,
                    )?,
                    None,
                )
            }
        }
    };
    with_mobile_db(data_dir, |conn| {
        store::apply_card_identity(&conn, &account_id, &mut messages);
        let threads = thread_cards_json(&account_id, &folder_id, messages);
        let mut out = json!({ "threads": threads });
        if let Some(cursor) = next_cursor {
            out.as_object_mut()
                .unwrap()
                .insert("next_cursor".to_string(), Value::String(cursor));
        }
        Ok(out)
    })
}

fn open_mobile_db(data_dir: &str) -> Result<rusqlite::Connection, String> {
    let data_dir = data_dir.trim();
    if data_dir.is_empty() {
        return Err("mobile core is not initialized".to_string());
    }
    let db_path = Path::new(data_dir).join("meron.db");
    match mobile_db_key() {
        Some(key) => store::open_at_keyed(&db_path, &key),
        None => store::open_at(&db_path),
    }
    .map_err(|err| err.to_string())
}

fn get_cached_mobile_mail_page(
    data_dir: &str,
    account_id: &str,
    folder_id: &str,
    limit: u32,
    before_cursor: Option<(i64, u32)>,
    unread_only: bool,
) -> Result<(Vec<MessageHeader>, Option<String>), String> {
    let conn = open_mobile_db(data_dir)?;
    store::get_recent_page(
        &conn,
        account_id,
        folder_id,
        limit,
        before_cursor,
        unread_only,
    )
    .map_err(|err| err.to_string())
}

fn search_cached_mobile_mail_messages(
    data_dir: &str,
    account_id: &str,
    folder_id: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>, String> {
    let conn = open_mobile_db(data_dir)?;
    store::search_messages(&conn, account_id, folder_id, query, limit)
        .map_err(|err| err.to_string())
}

fn search_live_mobile_mail_messages(
    data_dir: &str,
    account_id: &str,
    folder_id: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>, String> {
    let engine = crate::ffi::engine_for(data_dir)?;
    if engine.is_paused(account_id) {
        return Err(format!("account is paused: {account_id}"));
    }
    let creds = crate::ffi::engine_block_on(engine.ensure_valid_creds(account_id))?;
    if account_needs_reconnect(&creds) {
        return Err(format!("account needs reconnect: {account_id}"));
    }

    let mut folders = vec![folder_id.to_string()];
    if let Some(sent) = crate::engine::cached_sent_folder(&engine, account_id, folder_id) {
        folders.push(sent);
    }
    crate::ffi::engine_block_on(crate::engine::search_mail_messages(
        &engine, account_id, &folders, query, limit,
    ))
}

pub(crate) fn read_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    if is_rss_thread_id(&thread_id) {
        return read_mobile_rss_thread(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let headers = if let Some(uid) = parsed.uid {
            let header = store::get_thread_headers(
                &conn,
                &parsed.account,
                &parsed.folder,
                &format!("uid:{uid}"),
            )
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|header| header.uid == uid);
            match header {
                Some(header) => vec![header],
                None => {
                    let cached =
                        store::get_cached_message(&conn, &parsed.account, &parsed.folder, uid)
                            .map_err(|err| err.to_string())?;
                    if cached.is_some() {
                        vec![MessageHeader {
                            uid,
                            folder: parsed.folder.clone(),
                            thread_key: format!("uid:{uid}"),
                            ..Default::default()
                        }]
                    } else {
                        Vec::new()
                    }
                }
            }
        } else {
            store::get_thread_headers_all_folders(&conn, &parsed.account, &parsed.thread_key)
                .map_err(|err| err.to_string())?
        };
        // Mobile sync stores headers only, so the first time a thread is opened
        // its message bodies aren't cached yet. Fetch them on demand from IMAP
        // (grouped by folder), cache them, then render. Best-effort: a fetch
        // failure still renders whatever is cached.
        let mut missing: std::collections::BTreeMap<String, Vec<u32>> =
            std::collections::BTreeMap::new();
        for header in &headers {
            if header.uid == 0 {
                continue;
            }
            let folder = if header.folder.is_empty() {
                parsed.folder.clone()
            } else {
                header.folder.clone()
            };
            let cached = store::has_cached_body(&conn, &parsed.account, &folder, header.uid)
                .unwrap_or(false);
            if !cached {
                missing.entry(folder).or_default().push(header.uid);
            }
        }
        if !missing.is_empty() {
            // Fetch missing bodies through the Engine's warm session pool (with
            // in-core OAuth refresh + stale-retry). Best-effort: a fetch failure
            // still renders whatever is cached.
            let media_root = std::path::PathBuf::from(data_dir).join("attachments");
            let account = parsed.account.clone();
            let fetched = crate::ffi::engine_block_on(engine.with_read_session(
                &parsed.account,
                move |session| {
                    let missing = missing.clone();
                    let media_root = media_root.clone();
                    let account = account.clone();
                    Box::pin(async move {
                        let mut all = Vec::new();
                        for (folder, uids) in &missing {
                            let bodies = imap::fetch_bodies(
                                session,
                                folder,
                                uids,
                                media_root.clone(),
                                &account,
                            )
                            .await?;
                            for (uid, message) in bodies {
                                all.push((folder.clone(), uid, message));
                            }
                        }
                        anyhow::Ok(all)
                    })
                },
            ));
            if let Ok(fetched) = fetched {
                for (folder, uid, message) in fetched {
                    let _ =
                        store::save_cached_message(&conn, &parsed.account, &folder, uid, &message);
                }
            }
        }

        let messages = headers
            .into_iter()
            .filter_map(|header| {
                let folder = if header.folder.is_empty() {
                    parsed.folder.as_str()
                } else {
                    header.folder.as_str()
                };
                let cached =
                    store::get_cached_message(&conn, &parsed.account, folder, header.uid).ok()?;
                Some(message_json(
                    &parsed.account,
                    &thread_id,
                    folder,
                    &header,
                    cached.as_ref(),
                ))
            })
            .collect::<Vec<_>>();
        Ok(json!({ "messages": messages }))
    })
}

pub(crate) fn list_mobile_starred_items(data_dir: &str, params: &Value) -> Result<Value, String> {
    let limit = opt_u32(params, "limit").unwrap_or(200);
    with_mobile_db(data_dir, |conn| {
        let mut items = store::get_starred_all_accounts(&conn, limit)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|(account, header)| starred_item_json(&account, &header))
            .collect::<Vec<_>>();
        items
            .extend(rss::starred_items(&conn, i64::from(limit)).map_err(|err| format!("{err:#}"))?);
        items.sort_by(|a, b| {
            b.get("date")
                .and_then(Value::as_i64)
                .cmp(&a.get("date").and_then(Value::as_i64))
        });
        items.truncate(limit as usize);
        Ok(json!({ "items": items }))
    })
}

pub(crate) fn delete_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let parsed = parse_thread_id_with_request_folder(&thread_id, params)
        .ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        if uids.is_empty() {
            return Ok(json!({ "ok": true, "deleted": 0 }));
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let folder = parsed.folder.clone();
        let op_uids = uids.clone();
        let delete_result = crate::ffi::engine_block_on(engine.with_write_session(
            &parsed.account,
            move |session| {
                let folder = folder.clone();
                let uids = op_uids.clone();
                Box::pin(async move {
                    let drafts = imap::find_drafts_folder(session).await?;
                    let trash = if drafts.as_deref() == Some(folder.as_str()) {
                        imap::expunge_uids(session, &folder, &uids).await?;
                        None
                    } else {
                        let trash = imap::find_trash_folder(session).await?.ok_or_else(|| {
                            anyhow::anyhow!("Trash folder not found for this account")
                        })?;
                        if trash == folder {
                            imap::expunge_uids(session, &folder, &uids).await?;
                            None
                        } else {
                            imap::move_to_folder(session, &folder, &trash, &uids).await?;
                            Some(trash)
                        }
                    };
                    anyhow::Ok(trash)
                })
            },
        ))?;
        let deleted = store::delete_messages_by_uid(&conn, &parsed.account, &parsed.folder, &uids)
            .map_err(|err| err.to_string())?;
        match delete_result {
            None => Ok(json!({ "ok": true, "deleted": deleted, "permanent": true })),
            Some(trash) => Ok(json!({ "ok": true, "deleted": deleted, "trash": trash })),
        }
    })
}

pub(crate) fn archive_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let target_folder = find_cached_archive_folder(&conn, &parsed.account)?;
        if parsed.folder == target_folder {
            return Ok(json!({
                "ok": true,
                "moved": 0,
                "folder": target_folder,
                "thread_id": thread_id,
            }));
        }
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            return Ok(json!({
                "ok": true,
                "moved": 0,
                "folder": target_folder,
                "thread_id": thread_id,
            }));
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let op_folder = parsed.folder.clone();
        let op_target = target_folder.clone();
        let op_uids = uids.clone();
        let target_batch = crate::ffi::engine_block_on(engine.with_write_session(
            &parsed.account,
            move |session| {
                let source = op_folder.clone();
                let target = op_target.clone();
                let uids = op_uids.clone();
                Box::pin(async move {
                    imap::move_to_folder(session, &source, &target, &uids).await?;
                    let batch =
                        imap::fetch_recent(session, &target, 50.max(uids.len() as u32)).await?;
                    anyhow::Ok(batch)
                })
            },
        ))?;
        store::ensure_folder(&conn, &parsed.account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::upsert_messages(
            &conn,
            &parsed.account,
            &target_folder,
            &target_batch.messages,
        )
        .map_err(|err| err.to_string())?;
        store::set_folder_state(
            &conn,
            &parsed.account,
            &target_folder,
            target_batch.uidvalidity,
            target_batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        let moved = store::move_messages_by_uid(
            &conn,
            &parsed.account,
            &parsed.folder,
            &target_folder,
            &uids,
        )
        .map_err(|err| err.to_string())?;
        let moved_thread_id = format_thread_id(&parsed.account, &target_folder, &parsed.thread_key);
        Ok(json!({
            "ok": true,
            "moved": moved,
            "folder": target_folder,
            "thread_id": moved_thread_id,
        }))
    })
}

pub(crate) fn move_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_folder = canon_folder(&req_str(params, "target_folder_id")?);
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    if parsed.folder == target_folder {
        return Ok(
            json!({ "ok": true, "moved": 0, "folder": target_folder, "thread_id": thread_id }),
        );
    }
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            return Ok(
                json!({ "ok": true, "moved": 0, "folder": target_folder, "thread_id": thread_id }),
            );
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let op_folder = parsed.folder.clone();
        let op_target = target_folder.clone();
        let op_uids = uids.clone();
        let target_batch = crate::ffi::engine_block_on(engine.with_write_session(
            &parsed.account,
            move |session| {
                let source = op_folder.clone();
                let target = op_target.clone();
                let uids = op_uids.clone();
                Box::pin(async move {
                    imap::move_to_folder(session, &source, &target, &uids).await?;
                    let batch =
                        imap::fetch_recent(session, &target, 50.max(uids.len() as u32)).await?;
                    anyhow::Ok(batch)
                })
            },
        ))?;
        store::ensure_folder(&conn, &parsed.account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::upsert_messages(
            &conn,
            &parsed.account,
            &target_folder,
            &target_batch.messages,
        )
        .map_err(|err| err.to_string())?;
        store::set_folder_state(
            &conn,
            &parsed.account,
            &target_folder,
            target_batch.uidvalidity,
            target_batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        let moved = store::move_messages_by_uid(
            &conn,
            &parsed.account,
            &parsed.folder,
            &target_folder,
            &uids,
        )
        .map_err(|err| err.to_string())?;
        let moved_thread_id = format_thread_id(&parsed.account, &target_folder, &parsed.thread_key);
        Ok(json!({
            "ok": true,
            "moved": moved,
            "folder": target_folder,
            "thread_id": moved_thread_id,
        }))
    })
}

pub(crate) fn copy_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_account = req_str(params, "target_account_id")?;
    let target_folder = canon_folder(&req_str(params, "target_folder_id")?);
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            return Ok(json!({
                "ok": true,
                "copied": 0,
                "source_folder": parsed.folder,
                "target_account": target_account,
                "target_folder": target_folder,
            }));
        }
        let source_creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&source_creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let target_creds = load_mobile_account_creds(&conn, &target_account)?;
        if account_needs_reconnect(&target_creds) {
            return Err(format!("account needs reconnect: {target_account}"));
        }
        let source_folder = parsed.folder.clone();
        let fetch_uids = uids.clone();
        let raw_messages = crate::ffi::engine_block_on(engine.with_read_session(
            &parsed.account,
            move |session| {
                let folder = source_folder.clone();
                let uids = fetch_uids.clone();
                Box::pin(
                    async move { imap::fetch_raw_messages_for_copy(session, &folder, &uids).await },
                )
            },
        ))?;
        let copied = raw_messages.len();
        let target_folder_op = target_folder.clone();
        let batch = crate::ffi::engine_block_on(engine.with_write_session(
            &target_account,
            move |session| {
                let target = target_folder_op.clone();
                let raw_messages = raw_messages.clone();
                Box::pin(async move {
                    for message in &raw_messages {
                        imap::append_copied_message(session, &target, message).await?;
                    }
                    let batch =
                        imap::fetch_recent(session, &target, 50.max(raw_messages.len() as u32))
                            .await?;
                    anyhow::Ok(batch)
                })
            },
        ))?;
        store::ensure_folder(&conn, &target_account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::upsert_messages(&conn, &target_account, &target_folder, &batch.messages)
            .map_err(|err| err.to_string())?;
        store::set_folder_state(
            &conn,
            &target_account,
            &target_folder,
            batch.uidvalidity,
            batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({
            "ok": true,
            "copied": copied,
            "source_folder": parsed.folder,
            "target_account": target_account,
            "target_folder": target_folder,
        }))
    })
}

pub(crate) fn mark_mobile_thread_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let seen = req_bool(params, "seen")?;
    if is_rss_thread_id(&thread_id) {
        return mark_mobile_rss_thread_read(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        update_mobile_read_state(&conn, &parsed, params, &uids, seen)?;
        if !uids.is_empty() {
            let creds = load_mobile_account_creds(&conn, &parsed.account)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {}", parsed.account));
            }
            let folder = parsed.folder.clone();
            crate::ffi::engine_block_on(engine.with_write_session(
                &parsed.account,
                move |session| {
                    let folder = folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::set_seen(session, &folder, &uids, seen).await?;
                        anyhow::Ok(())
                    })
                },
            ))?;
        }
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn mark_mobile_folder_all_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .unwrap_or_else(|| "INBOX".to_string());

    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Ok(json!({ "ok": true, "updated": 0, "rss": true }));
        }
        let uids =
            store::get_unseen_uids(&conn, &account_id, &folder).map_err(|err| err.to_string())?;
        let updated = uids.len();
        if !uids.is_empty() {
            let creds = load_mobile_account_creds(&conn, &account_id)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {account_id}"));
            }
            let folder = folder.clone();
            crate::ffi::engine_block_on(engine.with_write_session(&account_id, move |session| {
                let folder = folder.clone();
                let uids = uids.clone();
                Box::pin(async move {
                    imap::set_seen(session, &folder, &uids, true).await?;
                    anyhow::Ok(())
                })
            }))?;
        }
        store::mark_folder_seen(&conn, &account_id, &folder, true)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "updated": updated, "folder": folder }))
    })
}

pub(crate) fn mark_mobile_thread_starred(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let starred = req_bool(params, "starred")?;
    if is_rss_thread_id(&thread_id) {
        return mark_mobile_rss_thread_starred(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        if !uids.is_empty() {
            let creds = load_mobile_account_creds(&conn, &parsed.account)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {}", parsed.account));
            }
            let folder = parsed.folder.clone();
            let uids = uids.clone();
            crate::ffi::engine_block_on(engine.with_write_session(
                &parsed.account,
                move |session| {
                    let folder = folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::set_starred(session, &folder, &uids, starred).await?;
                        anyhow::Ok(())
                    })
                },
            ))?;
        }
        if has_requested_mobile_message_ids(params) {
            for uid in uids {
                store::update_message_starred(&conn, &parsed.account, &parsed.folder, uid, starred)
                    .map_err(|err| err.to_string())?;
            }
        } else if let Some(uid) = parsed.uid {
            store::update_message_starred(&conn, &parsed.account, &parsed.folder, uid, starred)
                .map_err(|err| err.to_string())?;
        } else {
            store::update_thread_starred(
                &conn,
                &parsed.account,
                &parsed.folder,
                &parsed.thread_key,
                starred,
            )
            .map_err(|err| err.to_string())?;
        }
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn cached_thread_uids(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
) -> Result<Vec<u32>, String> {
    if let Some(uid) = parsed.uid {
        return Ok(vec![uid]);
    }
    Ok(
        store::get_thread_headers(conn, &parsed.account, &parsed.folder, &parsed.thread_key)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|header| header.uid)
            .collect(),
    )
}

pub(crate) fn has_requested_mobile_message_ids(params: &Value) -> bool {
    params
        .get("message_ids")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str().is_some_and(|value| !value.trim().is_empty()))
        })
}

pub(crate) fn requested_mobile_uids(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
    params: &Value,
) -> Result<Vec<u32>, String> {
    let uids = params
        .get("message_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(mobile_message_id_uid)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if has_requested_mobile_message_ids(params) {
        return Ok(uids);
    }
    cached_thread_uids(conn, parsed)
}

pub(crate) fn update_mobile_read_state(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
    params: &Value,
    uids: &[u32],
    seen: bool,
) -> Result<(), String> {
    if has_requested_mobile_message_ids(params) {
        for uid in uids {
            store::update_message_seen(conn, &parsed.account, &parsed.folder, *uid, seen)
                .map_err(|err| err.to_string())?;
        }
    } else if let Some(uid) = parsed.uid {
        store::update_message_seen(conn, &parsed.account, &parsed.folder, uid, seen)
            .map_err(|err| err.to_string())?;
    } else {
        store::update_thread_seen(
            conn,
            &parsed.account,
            &parsed.folder,
            &parsed.thread_key,
            seen,
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub(crate) fn mobile_message_id_uid(value: &str) -> Option<u32> {
    value
        .trim()
        .rsplit('#')
        .next()
        .and_then(|uid| uid.parse::<u32>().ok())
}

pub(crate) fn find_cached_archive_folder(
    conn: &rusqlite::Connection,
    account: &str,
) -> Result<String, String> {
    store::get_folders(conn, account)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|folder| looks_like_archive_folder(&folder.name))
        .map(|folder| folder.name)
        .ok_or_else(|| "Archive folder not found for this account".to_string())
}

pub(crate) fn canon_folder(folder: &str) -> String {
    if folder.eq_ignore_ascii_case("inbox") {
        "INBOX".to_string()
    } else {
        folder.to_string()
    }
}

pub(crate) fn looks_like_archive_folder(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "archive"
            | "archives"
            | "all mail"
            | "inbox.archive"
            | "inbox.archives"
            | "[gmail]/all mail"
            | "[google mail]/all mail"
    )
}

pub(crate) fn parse_mail_cursor(cursor: &str) -> Option<(i64, u32)> {
    let parts = cursor.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "date" {
        return None;
    }
    Some((parts[1].parse().ok()?, parts[2].parse().ok()?))
}

pub(crate) fn thread_cards_json(
    account_id: &str,
    folder_id: &str,
    messages: Vec<MessageHeader>,
) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();
    for message in messages {
        let thread_key = if message.thread_key.is_empty() {
            format!("uid:{}", message.uid)
        } else {
            message.thread_key.clone()
        };
        if !seen_keys.insert(thread_key.clone()) {
            continue;
        }
        let message_folder = if message.folder.is_empty() {
            folder_id.to_string()
        } else {
            message.folder.clone()
        };
        let thread_id = format_thread_id(account_id, &message_folder, &thread_key);
        out.push(json!({
            "id": thread_id,
            "account_id": account_id,
            "folder_id": message_folder,
            "thread_id": thread_id,
            "from_name": message.from_name,
            "from_addr": message.from_addr,
            "to": "",
            "subject": message.subject,
            "preview": "",
            "body": "",
            "date": message.date,
            "unread": !message.seen,
            "unread_count": if message.seen { 0 } else { 1 },
            "starred": message.starred,
            "has_attachments": false,
            "recipient_overflow": message.recipient_overflow,
        }));
    }
    out
}

pub(crate) fn format_thread_id(account_id: &str, folder: &str, thread_key: &str) -> String {
    if let Some(uid) = thread_key.strip_prefix("uid:") {
        return format!("{account_id}#{}#{uid}", canon_folder(folder));
    }
    let encoded = URL_SAFE_NO_PAD.encode(thread_key.as_bytes());
    format!("{account_id}#{}#t.{encoded}", canon_folder(folder))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedThreadId {
    pub(crate) account: String,
    pub(crate) folder: String,
    pub(crate) thread_key: String,
    pub(crate) uid: Option<u32>,
}

pub(crate) fn parse_thread_id(thread_id: &str) -> Option<ParsedThreadId> {
    let first = thread_id.find('#')?;
    let last = thread_id.rfind('#')?;
    if first == 0 || last <= first {
        return None;
    }
    let account = thread_id[..first].to_string();
    let folder = thread_id[first + 1..last].to_string();
    let key = &thread_id[last + 1..];
    if let Some(encoded) = key.strip_prefix("t.") {
        let decoded = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).ok()?;
        let thread_key = String::from_utf8(decoded).ok()?;
        return Some(ParsedThreadId {
            account,
            folder,
            thread_key,
            uid: None,
        });
    }
    let uid = key.parse::<u32>().ok()?;
    Some(ParsedThreadId {
        account,
        folder,
        thread_key: format!("uid:{uid}"),
        uid: Some(uid),
    })
}

pub(crate) fn parse_thread_id_with_request_folder(
    thread_id: &str,
    params: &Value,
) -> Option<ParsedThreadId> {
    let mut parsed = parse_thread_id(thread_id)?;
    if let Some(folder) = params
        .get("folder")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parsed.folder = folder.to_string();
    }
    Some(parsed)
}

pub(crate) fn message_json(
    account_id: &str,
    thread_id: &str,
    folder: &str,
    header: &MessageHeader,
    cached: Option<&Message>,
) -> Value {
    let id = format!("{thread_id}#{}", header.uid);
    json!({
        "id": id,
        "account_id": account_id,
        "folder_id": folder,
        "thread_id": thread_id,
        "from_name": cached.map(|message| message.from_name.as_str()).unwrap_or(header.from_name.as_str()),
        "from_addr": cached.map(|message| message.from_addr.as_str()).unwrap_or(header.from_addr.as_str()),
        "to": cached.map(|message| message.to.as_str()).unwrap_or(""),
        "reply_to": cached.map(|message| message.reply_to.as_str()).unwrap_or(""),
        "cc": cached.map(|message| message.cc.as_str()).unwrap_or(""),
        "bcc": cached.map(|message| message.bcc.as_str()).unwrap_or(""),
        "message_id": cached.map(|message| message.message_id.as_str()).unwrap_or(""),
        "references": cached.map(|message| message.references.as_str()).unwrap_or(""),
        "subject": cached.map(|message| message.subject.as_str()).unwrap_or(header.subject.as_str()),
        "preview": cached.map(|message| message.preview.as_str()).unwrap_or(""),
        "body": cached.map(|message| message.body.as_str()).unwrap_or(""),
        "body_html": cached.and_then(|message| message.body_html.as_deref()).unwrap_or(""),
        // No cached body means the on-demand IMAP fetch failed (auth/network),
        // not that the message is empty — the client offers a retry for these.
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

pub(crate) fn starred_item_json(account_id: &str, header: &MessageHeader) -> Value {
    let folder = if header.folder.is_empty() {
        "INBOX"
    } else {
        header.folder.as_str()
    };
    let thread_key = if header.thread_key.is_empty() {
        format!("uid:{}", header.uid)
    } else {
        header.thread_key.clone()
    };
    let thread_id = format_thread_id(account_id, folder, &thread_key);
    json!({
        "id": format!("{thread_id}#{}", header.uid),
        "account_id": account_id,
        "folder_id": folder,
        "thread_id": thread_id,
        "from_name": header.from_name.as_str(),
        "from_addr": header.from_addr.as_str(),
        "to": "",
        "reply_to": "",
        "cc": "",
        "bcc": "",
        "message_id": "",
        "references": "",
        "subject": header.subject.as_str(),
        "preview": "",
        "body": "",
        "body_html": "",
        "date": header.date,
        "unread": !header.seen,
        "starred": true,
        "has_attachments": false,
        "attachments": [],
    })
}

#[cfg(test)]
mod sync_tail_tests {
    use super::{claim_sync_tail, release_sync_tail};

    #[test]
    fn sync_tail_claim_is_exclusive_per_mailbox() {
        assert!(claim_sync_tail("tail-test@example.com/INBOX"));
        // A second sync for the same mailbox must not stack another tail.
        assert!(!claim_sync_tail("tail-test@example.com/INBOX"));
        // A different mailbox is independent.
        assert!(claim_sync_tail("tail-test@example.com/Archive"));
        release_sync_tail("tail-test@example.com/INBOX");
        assert!(claim_sync_tail("tail-test@example.com/INBOX"));
        release_sync_tail("tail-test@example.com/INBOX");
        release_sync_tail("tail-test@example.com/Archive");
    }
}
