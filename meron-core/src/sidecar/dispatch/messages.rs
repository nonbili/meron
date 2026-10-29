use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::protocol::Request;
use meron_core::{mail_model, parse, rss, store, thread_list, thread_read, unified};

use crate::sidecar::params::*;
use crate::sidecar::spawn::*;
use crate::{Writer, emit};

/// Handle mailbox and thread reads: listing, sync, thread reads, contacts.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        "messages.unifiedRecent" => {
            let cursors = p
                .get("before_cursor")
                .and_then(Value::as_str)
                .and_then(unified::decode_cursor)
                .unwrap_or_default();
            let accounts = store::list_accounts(&engine.db.lock().unwrap())?
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
                .filter(|account| cursors.is_empty() || cursors.contains_key(account))
                .collect::<Vec<_>>();
            // The unified view switches folders by *role*: each account answers
            // from its own Sent/Archive/Trash/…, and an account whose server has
            // no such mailbox drops out of the merge silently. Surfacing that as
            // a failure would pin a permanent error banner on the view for any
            // account whose provider simply lacks the folder.
            let role = req_str(p, "folder_role").unwrap_or_else(|_| "inbox".to_string());
            let mut folders = Vec::with_capacity(accounts.len());
            {
                let db = engine.db.lock().unwrap();
                for account in accounts {
                    if let Some(folder) = store::folder_for_role(&db, &account, &role)? {
                        folders.push((account, folder));
                    }
                }
            }
            let limit = u32::from(req_u16(p, "limit").unwrap_or(50));
            let mut pages = Vec::with_capacity(folders.len());
            for (account, folder) in folders {
                let from = cursors.get(&account).cloned().unwrap_or_default();
                let mut params = json!({
                    "account": account.clone(),
                    "folder": folder,
                    "query": req_str(p, "query").unwrap_or_default(),
                    "filter": req_str(p, "filter").unwrap_or_default(),
                    "attachments": p.get("attachments").and_then(Value::as_bool).unwrap_or(false),
                    "limit": from.fetch_limit(limit),
                    "refresh": p.get("refresh").and_then(Value::as_bool).unwrap_or(true),
                    "group": true,
                });
                if let Some(cursor) = &from.cursor {
                    params["before_cursor"] = Value::String(cursor.clone());
                }
                let request = Request {
                    id: req.id,
                    method: "messages.recent".to_string(),
                    params,
                };
                let result = Box::pin(dispatch(engine, &request, out))
                    .await
                    .map_err(|err| format!("{err:#}"));
                pages.push(unified::AccountPage {
                    account_id: account,
                    from,
                    result,
                });
            }
            Ok(unified::merge_pages(pages, "threads", limit))
        }

        // RSS returns final thread Message JSON under "threads"; mail returns raw
        // rows under "messages" the bridge groups into threads.
        "messages.recent" => {
            // The attachments toggle narrows a page after it is read (search,
            // starred and feed pages cannot ask for it up front), so a page can
            // come back empty while older matches remain. Read on through a few
            // such pages rather than hand back an empty list — which would show
            // the empty state and no way to page on.
            let mut params = p.clone();
            let mut hops = 0;
            loop {
                let page = messages_recent(engine, &params, out).await?;
                let request = thread_list::ThreadListQuery::from_params(&params, "folder");
                match request.next_attachment_cursor(&page, hops) {
                    Some(cursor) => {
                        params["before_cursor"] = Value::String(cursor);
                        hops += 1;
                    }
                    None => return Ok(page),
                }
            }
        }

        // Every starred item across all accounts, local cache only (the
        // IMAP-backed starred filter keeps mail flags fresh; no round-trip
        // here). Core returns one final, searchable, paginated item model for
        // both mail and RSS so transport adapters do not mint ids or reshape it.
        // Which notified messages are still unread, so the bridge can withdraw
        // notifications for mail read, moved or deleted since.
        "messages.stillUnread" => {
            meron_core::protocol::messages_still_unread(&engine.db.lock().unwrap(), p)
                .map_err(anyhow::Error::msg)
        }
        "starred.items" => {
            let limit = req_u32(p, "limit").unwrap_or(200);
            // Before taking the store lock: opening the archive reads the store.
            let local = crate::sidecar::local::starred_cards(engine, 2_000);
            let db = engine.db.lock().unwrap();
            let mut items = mail_model::starred_thread_cards(&db, 2_000)?;
            items.extend(local);
            items.extend(rss::starred_items(&db, 2_000)?);
            Ok(mail_model::starred_page(
                items,
                &req_str(p, "query").unwrap_or_default(),
                &req_str(p, "filter").unwrap_or_else(|_| "all".to_string()),
                p.get("attachments")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                limit as usize,
                p.get("before_cursor").and_then(Value::as_str),
            ))
        }

        "identity.allocate" => Ok(json!({
            "message_id": mail_model::allocate_message_id(
                &req_str(p, "account_id").unwrap_or_default(),
                p.get("draft").and_then(Value::as_bool).unwrap_or(false),
            )
        })),

        // Recipient autocomplete: distinct correspondents from cached messages,
        // matched against `query` and ranked by frequency/recency.
        "contacts.suggest" => {
            let account = req_str(p, "account").unwrap_or_default();
            let query = req_str(p, "query").unwrap_or_default();
            let limit = req_u32(p, "limit").unwrap_or(8);
            let contacts =
                store::suggest_contacts(&engine.db.lock().unwrap(), &account, &query, limit)?;
            Ok(json!({ "contacts": contacts }))
        }

        // Fire-and-forget background sync; the result arrives via mail.synced.
        "messages.sync" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                spawn_rss_sync(engine.clone(), out.clone(), account);
                return Ok(json!({ "ok": true, "queued": true }));
            }
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let limit = req_u16(p, "limit").unwrap_or(50) as u32;
            spawn_message_sync(engine.clone(), out.clone(), account, folder, limit);
            Ok(json!({ "ok": true, "queued": true }))
        }

        "messages.read" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let uid = req_u32(p, "uid")?;

            let mut message = read_cached_or_fetch(engine, &account, &folder, uid).await?;
            let (mine, ours) = {
                let db = engine.db.lock().unwrap();
                (store::self_addrs(&db, &account), store::all_self_addrs(&db))
            };
            // Shaped like the thread read's, without the calendar text.
            let invite = message
                .invite
                .take()
                .or_else(|| {
                    meron_core::calendar::invite_from_attachments(
                        &parse::media_root(),
                        &message.attachments,
                    )
                })
                .map(|invite| {
                    meron_core::calendar::invite_json_for(
                        &engine.db.lock().unwrap(),
                        &account,
                        &invite,
                        &mine,
                    )
                });
            let outgoing = store::account_message_is_outgoing(
                &account,
                &mine,
                &folder,
                &message.from_addr,
                message.delivered,
            );
            // The same reply rule the thread read ships with every message, so a
            // single-message read seeds a reply identically.
            let reply = meron_core::reply::reply_json(
                &meron_core::reply::ReplyTarget {
                    from_name: &message.from_name,
                    from_addr: &message.from_addr,
                    reply_to: &message.reply_to,
                    to: &message.to,
                    cc: &message.cc,
                },
                &ours,
            );
            Ok(json!({
                "outgoing": outgoing,
                "reply": reply,
                "invite": invite,
                "message": serde_json::to_value(message)?
            }))
        }

        "messages.thread" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            // Card ids carry a branch subject suffix; the store queries use the
            // root key and the branch filter narrows the rows afterwards.
            let (thread_key, subject_filter) = store::split_thread_key(&req_str(p, "thread_key")?);
            // Pagination is opt-in: callers that don't pass `limit` get the
            // full thread (preserves the markRead full-scan path in app.go).
            let limit = p.get("limit").and_then(Value::as_u64).map(|n| n as u32);
            let before_cursor = p.get("before_cursor").and_then(Value::as_str);
            // The bridge passes the frontend's exact thread id so message ids
            // match what the UI keys on; direct callers (tests) may omit it.
            let thread_id = req_str(p, "thread_id")
                .unwrap_or_else(|_| mail_model::format_thread_id(&account, &folder, &thread_key));

            // For UI reads (limit present), pull in any referenced ancestor
            // messages missing from the local cache so the reader shows the
            // full conversation instead of just the synced tail or a lone
            // draft. Runs in the background; if the fill finds anything it
            // emits `mail.synced` and the reader re-reads. The markRead
            // full-scan path (no limit) skips this entirely.
            if limit.is_some() {
                maybe_spawn_fill_thread_gaps(engine, out, &account, &thread_key);
            }

            // The background body fill announces itself with `mail.synced`,
            // which the desktop frontend already answers by re-reading the
            // open thread.
            let on_bodies_fetched: thread_read::BodiesFetchedHook = {
                let out = out.clone();
                let account = account.clone();
                Box::new(move || {
                    let out = out.clone();
                    let account = account.clone();
                    tokio::spawn(async move {
                        emit(
                            &out,
                            "mail.synced",
                            json!({ "account": account, "folder": "inbox", "synced": 0 }),
                        )
                        .await;
                    });
                })
            };
            thread_read::read_thread_page(
                engine,
                thread_read::ThreadReadArgs {
                    account: &account,
                    folder: &folder,
                    thread_id: &thread_id,
                    thread_key: &thread_key,
                    subject_filter: subject_filter.as_deref(),
                    limit,
                    for_print: p.get("for_print").and_then(Value::as_bool).unwrap_or(false),
                    before_cursor,
                    media_root: parse::media_root(),
                },
                Some(on_bodies_fetched),
            )
            .await
        }

        "messages.threadHeaders" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) = store::split_thread_key(&req_str(p, "thread_key")?);
            let headers = {
                let db = engine.db.lock().unwrap();
                store::get_thread_headers(&db, &account, &folder, &thread_key)?
            };
            let headers = headers
                .into_iter()
                .filter(|header| match subject_filter.as_deref() {
                    Some(filter) => store::thread_grouping_subject(&header.subject) == filter,
                    None => true,
                })
                .map(|header| {
                    json!({
                        "uid": header.uid,
                        "folder": folder,
                        "subject": header.subject,
                        "seen": header.seen,
                        "starred": header.starred,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({ "headers": headers }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}

/// One `messages.recent` page: a mailbox view's cards, from the cache or a live
/// search, narrowed by the request's filters.
async fn messages_recent(engine: &Arc<Engine>, p: &Value, out: &Writer) -> anyhow::Result<Value> {
    let account = req_str(p, "account")?;
    let request = thread_list::ThreadListQuery::from_params(p, "folder");
    let refresh = p.get("refresh").and_then(Value::as_bool).unwrap_or(true);
    if is_rss(engine, &account)? {
        let page = thread_list::rss_page(&engine.db.lock().unwrap(), &account, &request)?;
        if refresh {
            spawn_rss_sync(engine.clone(), out.clone(), account);
        }
        return Ok(page);
    }
    let folder = request.folder.clone();
    let limit = request.limit;
    // Capture this before spawning the background refresh. The returned
    // page was read from the pre-refresh cache, so its empty-state
    // metadata must describe that same snapshot.
    let folder_synced_before =
        store::get_folder_state(&engine.db.lock().unwrap(), &account, &folder)?.is_some();
    // Desktop starred reads are online-first. Search first paints the
    // local index with refresh=false, then repeats with refresh=true;
    // snapshot-backed later pages are local even though they travel
    // through the shared search engine.
    let mut search_incomplete = false;
    let (messages, next_cursor) = match request.source() {
        thread_list::MailSource::Starred => {
            let folders = starred_search_folders(engine, &account, &folder).await;
            (
                search_starred_mail_messages(engine, &account, &folders, limit, refresh).await?,
                None,
            )
        }
        thread_list::MailSource::Recent { filter } => store::get_recent_page(
            &engine.db.lock().unwrap(),
            &account,
            &folder,
            limit,
            request.before_cursor,
            filter,
        )?,
        thread_list::MailSource::Search => {
            // Chat-view search spans the selected folder plus Sent, so a
            // lookup surfaces both received and self-sent mail (and old
            // messages filed under Sent), not just the current mailbox.
            let folders = search_folders(&engine.db.lock().unwrap(), &account, &folder);
            if refresh || request.search_before_cursor.as_ref().is_some() {
                let page = search_mail_messages(
                    engine,
                    &account,
                    &folders,
                    &request.query,
                    limit,
                    request.search_before_cursor.as_ref(),
                )
                .await?;
                search_incomplete = page.incomplete;
                (page.messages, page.next_cursor)
            } else {
                let messages = store::search_messages_in_folders(
                    &engine.db.lock().unwrap(),
                    &account,
                    &folders,
                    &request.query,
                    limit,
                    None,
                )?;
                let next_cursor = store::search_next_cursor(&messages, limit, 0);
                (messages, next_cursor)
            }
        }
    };
    if refresh && request.wants_background_sync() {
        spawn_message_sync(
            engine.clone(),
            out.clone(),
            account.clone(),
            folder.clone(),
            limit,
        );
    }
    let mut page = thread_list::mail_page(
        &engine.db.lock().unwrap(),
        &account,
        &folder,
        messages,
        next_cursor,
        p.get("group").and_then(Value::as_bool).unwrap_or(false),
    )?;
    page.as_object_mut().unwrap().insert(
        "folder_synced".to_string(),
        Value::Bool(folder_synced_before),
    );
    if search_incomplete {
        page.as_object_mut()
            .unwrap()
            .insert("search_incomplete".to_string(), Value::Bool(true));
    }
    request.retain_attachment_threads(&mut page);
    Ok(page)
}
