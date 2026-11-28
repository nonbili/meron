use super::super::*;
use super::parse_thread_id;

/// Cached threads involving an exact address, across all accounts and folders.
/// The store builds ordinary thread-list cards, including actionable identities.
pub(crate) fn list_mobile_contact_threads(data_dir: &str, params: &Value) -> Result<Value, String> {
    let addr = req_str(params, "addr")?;
    if addr.trim().is_empty() {
        return Err("addr is required".to_string());
    }
    let limit = opt_u32(params, "limit").unwrap_or(50).clamp(1, 200) as usize;
    let cursor = params.get("before_cursor").and_then(Value::as_str);
    with_mobile_db(data_dir, |conn| {
        store::contact_thread_page(&conn, &addr, cursor, limit).map_err(|err| format!("{err:#}"))
    })
}

pub(crate) fn list_mobile_threads(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    if account_id == "unified" {
        return list_mobile_unified_threads(data_dir, params);
    }
    // The attachments toggle narrows a page after it is read, so a page can come
    // back empty while older matches remain: read on through a few such pages
    // (see `ThreadListQuery::next_attachment_cursor`).
    let mut params = params.clone();
    let mut hops = 0;
    loop {
        let page = list_mobile_account_threads(data_dir, &params, &account_id)?;
        let request = thread_list::ThreadListQuery::from_params(&params, "folder_id");
        match request.next_attachment_cursor(&page, hops) {
            Some(cursor) => {
                params["before_cursor"] = Value::String(cursor);
                hops += 1;
            }
            None => return Ok(page),
        }
    }
}

/// One page of a single account's mailbox view.
fn list_mobile_account_threads(
    data_dir: &str,
    params: &Value,
    account_id: &str,
) -> Result<Value, String> {
    let account_id = account_id.to_string();
    let request = thread_list::ThreadListQuery::from_params(params, "folder_id");
    let folder_id = request.folder.clone();
    let limit = request.limit;
    let refresh = params
        .get("refresh")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let is_rss = with_mobile_db(data_dir, |conn| {
        Ok(json!(is_rss_account(&conn, &account_id)?))
    })?
    .as_bool()
    .unwrap_or(false);
    if is_rss {
        return with_mobile_db(data_dir, |conn| {
            thread_list::rss_page(&conn, &account_id, &request).map_err(|err| format!("{err:#}"))
        });
    }

    // Mobile is cache-first: unlike desktop, starred reads the flags the last
    // sync stored so the list still answers offline, and a failed live search
    // falls back to the cached one.
    let mut search_incomplete = false;
    let (messages, next_cursor) = match request.source() {
        thread_list::MailSource::Starred => (
            get_cached_mobile_starred(data_dir, &account_id, &folder_id, limit)?,
            None,
        ),
        thread_list::MailSource::Recent { filter } => get_cached_mobile_mail_page(
            data_dir,
            &account_id,
            &folder_id,
            limit,
            request.before_cursor,
            filter,
        )?,
        thread_list::MailSource::Search => {
            // One folder list for both halves: the live search and the offline
            // fallback must cover the same mailboxes, or losing the network
            // would silently drop the Sent hits from the results.
            let folders = mobile_search_folders(data_dir, &account_id, &folder_id, &request.query)?;
            // Mobile renders indexed local hits first, then repeats the first
            // page with refresh=true off the UI thread. Later pages are local
            // reads (the live snapshot, else cached keyset order), served
            // straight from the store so they need neither the engine nor valid
            // credentials — a paused account keeps paging its snapshot.
            let (mut messages, next_cursor) = if let Some(cursor) =
                request.search_before_cursor.as_ref()
            {
                let page = match continue_mobile_search(
                    data_dir,
                    &account_id,
                    &folders,
                    &request.query,
                    limit,
                    cursor,
                )? {
                    crate::engine::SearchContinuation::Page(page) => page,
                    // The page reaches server hits the first page left for
                    // later; fetching them needs the engine. Offline, show
                    // what is here and say the list is incomplete.
                    crate::engine::SearchContinuation::NeedsServer(partial) => {
                        search_live_mobile_mail_messages(
                            data_dir,
                            &account_id,
                            &folders,
                            &request.query,
                            limit,
                            Some(cursor),
                        )
                        .unwrap_or(partial)
                    }
                };
                search_incomplete = page.incomplete;
                (page.messages, page.next_cursor)
            } else if refresh {
                match search_live_mobile_mail_messages(
                    data_dir,
                    &account_id,
                    &folders,
                    &request.query,
                    limit,
                    None,
                ) {
                    Ok(page) => {
                        search_incomplete = page.incomplete;
                        (page.messages, page.next_cursor)
                    }
                    Err(err) => {
                        crate::mlog!(
                            crate::log::Level::Warn,
                            "mail.search",
                            "live search failed for account={account_id} folder={folder_id}: {err}"
                        );
                        search_incomplete = true;
                        let messages = search_cached_mobile_mail_messages(
                            data_dir,
                            &account_id,
                            &folders,
                            &request.query,
                            limit,
                        )?;
                        let next_cursor = store::search_next_cursor(&messages, limit, 0);
                        (messages, next_cursor)
                    }
                }
            } else {
                let messages = search_cached_mobile_mail_messages(
                    data_dir,
                    &account_id,
                    &folders,
                    &request.query,
                    limit,
                )?;
                let next_cursor = store::search_next_cursor(&messages, limit, 0);
                (messages, next_cursor)
            };
            store::dedupe_search_hits(&mut messages, &folders);
            (messages, next_cursor)
        }
    };
    with_mobile_db(data_dir, |conn| {
        let mut page = thread_list::mail_page(
            &conn,
            &account_id,
            &folder_id,
            messages,
            next_cursor,
            // A mobile mailbox view is always a thread list.
            true,
        )
        .map_err(|err| format!("{err:#}"))?;
        if search_incomplete {
            page.as_object_mut()
                .unwrap()
                .insert("search_incomplete".to_string(), Value::Bool(true));
        }
        request.retain_attachment_threads(&mut page);
        Ok(page)
    })
}

fn list_mobile_unified_threads(data_dir: &str, params: &Value) -> Result<Value, String> {
    let limit = thread_list::ThreadListQuery::from_params(params, "folder_id").limit;
    let account_cursors = params
        .get("before_cursor")
        .and_then(Value::as_str)
        .and_then(crate::unified::decode_cursor)
        .unwrap_or_default();
    let accounts = with_mobile_db(data_dir, |conn| {
        let accounts = store::list_accounts(&conn).map_err(|err| err.to_string())?;
        Ok(json!(
            accounts
                .into_iter()
                .filter(|account| {
                    account
                        .get("included_in_unified")
                        .and_then(Value::as_bool)
                        .unwrap_or(true)
                })
                .filter_map(|account| {
                    account
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        ))
    })?
    .as_array()
    .cloned()
    .unwrap_or_default()
    .into_iter()
    .filter_map(|account| account.as_str().map(str::to_string))
    .filter(|account| account_cursors.is_empty() || account_cursors.contains_key(account))
    .collect::<Vec<_>>();

    // Resolve the requested role to each account's own mailbox; accounts whose
    // server has no such folder drop out silently rather than reporting a
    // failure the view would render as a permanent error. Mirrors the desktop
    // "messages.unifiedRecent" fan-out.
    let role = params
        .get("folder_role")
        .and_then(Value::as_str)
        .unwrap_or("inbox")
        .to_string();
    let folders = with_mobile_db(data_dir, |conn| {
        let mut resolved = Vec::new();
        for account_id in &accounts {
            match store::folder_for_role(&conn, account_id, &role) {
                Ok(Some(folder)) => resolved.push((account_id.clone(), folder)),
                Ok(None) => {}
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(json!(resolved))
    })?;
    let folders = serde_json::from_value::<Vec<(String, String)>>(folders)
        .map_err(|err| format!("{err:#}"))?;

    let mut pages = Vec::new();
    let mut unsynced_folders = Vec::new();
    for (account_id, folder) in folders {
        let mut account_params = params.clone();
        let object = account_params
            .as_object_mut()
            .ok_or_else(|| "params must be an object".to_string())?;
        object.insert("account_id".to_string(), Value::String(account_id.clone()));
        object.insert("folder_id".to_string(), Value::String(folder.clone()));
        let from = account_cursors
            .get(&account_id)
            .cloned()
            .unwrap_or_default();
        match &from.cursor {
            Some(cursor) => {
                object.insert("before_cursor".to_string(), Value::String(cursor.clone()));
            }
            None => {
                object.remove("before_cursor");
            }
        }
        object.insert("limit".to_string(), json!(from.fetch_limit(limit)));
        let result = list_mobile_threads(data_dir, &account_params);
        if result
            .as_ref()
            .is_ok_and(|page| page["folder_synced"] == false)
        {
            unsynced_folders.push(json!({ "account_id": account_id, "folder_id": folder }));
        }
        pages.push(crate::unified::AccountPage {
            account_id,
            from,
            result,
        });
    }
    let mut page = crate::unified::merge_pages(pages, "threads", limit);
    page["unsynced_folders"] = json!(unsynced_folders);
    Ok(page)
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
    filter: store::RecentFilter,
) -> Result<(Vec<MessageHeader>, Option<String>), String> {
    let conn = open_mobile_db(data_dir)?;
    store::get_recent_page(&conn, account_id, folder_id, limit, before_cursor, filter)
        .map_err(|err| err.to_string())
}

fn get_cached_mobile_starred(
    data_dir: &str,
    account_id: &str,
    folder_id: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>, String> {
    let conn = open_mobile_db(data_dir)?;
    store::get_starred(&conn, account_id, folder_id, limit).map_err(|err| err.to_string())
}

/// Resolve the same search scope with or without a live connection.
fn mobile_search_folders(
    data_dir: &str,
    account_id: &str,
    folder_id: &str,
    query: &str,
) -> Result<Vec<String>, String> {
    let conn = open_mobile_db(data_dir)?;
    Ok(crate::engine::query_folders(
        &conn, account_id, folder_id, query,
    ))
}

fn search_cached_mobile_mail_messages(
    data_dir: &str,
    account_id: &str,
    folders: &[String],
    query: &str,
    limit: u32,
) -> Result<Vec<MessageHeader>, String> {
    let conn = open_mobile_db(data_dir)?;
    store::search_messages_in_folders(&conn, account_id, folders, query, limit, None)
        .map_err(|err| err.to_string())
}

fn continue_mobile_search(
    data_dir: &str,
    account_id: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    cursor: &crate::thread_list::SearchCursor,
) -> Result<crate::engine::SearchContinuation, String> {
    let conn = open_mobile_db(data_dir)?;
    crate::engine::continue_search_page(&conn, account_id, folders, query, limit, cursor)
        .map_err(|err| format!("{err:#}"))
}

fn search_live_mobile_mail_messages(
    data_dir: &str,
    account_id: &str,
    folders: &[String],
    query: &str,
    limit: u32,
    before_cursor: Option<&crate::thread_list::SearchCursor>,
) -> Result<crate::engine::SearchMailPage, String> {
    let engine = crate::ffi::engine_for(data_dir)?;
    if engine.is_paused(account_id) {
        return Err(format!("account is paused: {account_id}"));
    }
    let creds = crate::ffi::engine_block_on(engine.ensure_valid_creds(account_id))?;
    if account_needs_reconnect(&creds) {
        return Err(format!("account needs reconnect: {account_id}"));
    }

    crate::ffi::engine_block_on(crate::engine::search_mail_messages(
        &engine,
        account_id,
        folders,
        query,
        limit,
        before_cursor,
    ))
}

pub(crate) fn read_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    if is_rss_thread_id(&thread_id) {
        return read_mobile_rss_thread(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    // Mobile sync stores headers only, so opening a thread routinely finds
    // uncached bodies; the shared reader pulls the newest one inline and fills
    // the rest in the background. `mail.tailSynced` is the body-only refresh
    // signal: the mobile UI answers it by re-reading just the open thread.
    let thread_key = match parsed.uid {
        Some(uid) => format!("uid:{uid}"),
        None => parsed.thread_key.clone(),
    };
    let limit = opt_u32(params, "limit");
    let before_cursor = params
        .get("before_cursor")
        .and_then(Value::as_str)
        .map(str::to_string);
    let on_bodies_fetched: crate::thread_read::BodiesFetchedHook = {
        let account = parsed.account.clone();
        Box::new(move || {
            let _ = crate::ffi::emit_event(
                "mail.tailSynced",
                serde_json::json!({ "account": account }),
            );
        })
    };
    crate::ffi::engine_block_on(crate::thread_read::read_thread_page(
        &engine,
        crate::thread_read::ThreadReadArgs {
            account: &parsed.account,
            folder: &parsed.folder,
            thread_id: &thread_id,
            thread_key: &thread_key,
            subject_filter: parsed.subject_filter.as_deref(),
            limit,
            for_print: params
                .get("for_print")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            before_cursor: before_cursor.as_deref(),
            media_root: std::path::PathBuf::from(data_dir).join("attachments"),
        },
        Some(on_bodies_fetched),
    ))
}

pub(crate) fn list_mobile_starred_items(data_dir: &str, params: &Value) -> Result<Value, String> {
    let limit = opt_u32(params, "limit").unwrap_or(200);
    with_mobile_db(data_dir, |conn| {
        let mut items = crate::mail_model::starred_thread_cards(&conn, 2_000)
            .map_err(|err| format!("{err:#}"))?;
        items.extend(rss::starred_items(&conn, 2_000).map_err(|err| format!("{err:#}"))?);
        Ok(crate::mail_model::starred_page(
            items,
            params
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            params
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or("all"),
            params
                .get("attachments")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            limit as usize,
            params.get("before_cursor").and_then(Value::as_str),
        ))
    })
}
