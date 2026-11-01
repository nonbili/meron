use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::protocol::Request;
use meron_core::tags::{parse_tag_list, parse_tag_replacement, renamed_queries};
use meron_core::{imap, mail_model, store};

use crate::Writer;
use crate::sidecar::params::*;

/// Handle message flag and placement changes: read, starred, delete, move, copy.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        "messages.markRead" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            // Defaults to true (mark read); pass seen:false to mark unread.
            let seen = p.get("seen").and_then(Value::as_bool).unwrap_or(true);

            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // Marking a whole thread read reaches every folder it spans and only
            // the messages still unread; everything else acts on this folder.
            let whole_thread_read = explicit_uids.is_empty() && !thread_key.is_empty() && seen;
            let by_folder = if whole_thread_read {
                let db = engine.db.lock().unwrap();
                // The store is only updated after the server write below, so
                // skipping messages it already has read is safe to retry.
                store::thread_uids_by_folder(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    true,
                )?
            } else {
                let uids = if !explicit_uids.is_empty() {
                    explicit_uids
                } else if thread_key.is_empty() {
                    uid.into_iter().collect::<Vec<_>>()
                } else {
                    // Marking a whole thread unread flags its newest message only.
                    let db = engine.db.lock().unwrap();
                    store::newest_thread_uids(
                        &db,
                        &account,
                        &folder,
                        &thread_key,
                        subject_filter.as_deref(),
                    )?
                };
                std::collections::BTreeMap::from([(folder.clone(), uids)])
            };

            engine
                .store_seen_by_folder(&account, &by_folder, seen)
                .await?;

            {
                let db = engine.db.lock().unwrap();
                // Per uid: a whole-thread update would flip sibling subject
                // branches sharing the root thread_key, and marking unread only
                // flagged the newest message.
                for (marked_folder, uids) in &by_folder {
                    for marked_uid in uids {
                        store::update_message_seen(
                            &db,
                            &account,
                            marked_folder,
                            *marked_uid,
                            seen,
                        )?;
                    }
                }
                if whole_thread_read && subject_filter.is_none() {
                    store::update_thread_seen(&db, &account, &folder, &thread_key, seen)?;
                }
            }
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };
            let db = engine.db.lock().unwrap();
            let mut result = mail_model::mutation_result(
                json!({ "ok": true }),
                &db,
                &account,
                &changed_thread_id,
                &folder,
                None,
                Some(!seen),
                None,
                false,
            )?;
            mail_model::add_folder_unreads(&mut result, &db, &account, by_folder.keys())?;
            Ok(result)
        }

        "messages.markStarred" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let full_key = req_str(p, "thread_key").unwrap_or_default();
            let (thread_key, subject_filter) = store::split_thread_key(&full_key);
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            // Defaults to true (mark starred); pass starred:false to unstar.
            let starred = p.get("starred").and_then(Value::as_bool).unwrap_or(true);

            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let targets = {
                let db = engine.db.lock().unwrap();
                let message_scoped = p.get("uids").is_some() || full_key.is_empty();
                let uids = if p.get("uids").is_some() {
                    explicit_uids
                } else {
                    uid.into_iter().collect()
                };
                store::starred_mutation_targets(
                    &db,
                    &account,
                    &folder,
                    (!message_scoped).then_some(full_key.as_str()),
                    &uids,
                    starred,
                )?
            };
            mark_starred_copies(&engine, &account, &targets, starred).await?;
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };
            mail_model::mutation_result(
                json!({ "ok": true }),
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                None,
                None,
                Some(starred),
                false,
            )
        }

        "messages.setTags" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let add = parse_tag_list(p.get("add")).map_err(anyhow::Error::msg)?;
            let remove = parse_tag_list(p.get("remove")).map_err(anyhow::Error::msg)?;
            let tags = {
                let db = engine.db.lock().unwrap();
                let targets = store::tag_targets(
                    &db,
                    &account,
                    &folder,
                    &req_str(p, "thread_key").unwrap_or_default(),
                    &tag_uids(p),
                )?;
                store::retag(&db, &account, &targets, &add, &remove, true)?
            };
            spawn_tag_push(&engine, &account);
            Ok(json!({ "ok": true, "tags": tags }))
        }

        // Rename a tag everywhere, or delete it. The archive goes first, as
        // opening it reads the store.
        "tags.replace" => {
            let (tag, to) = parse_tag_replacement(p).map_err(anyhow::Error::msg)?;
            crate::sidecar::local::replace_tag(engine, &tag, to.as_deref())?;
            let (accounts, rules) =
                store::replace_tag_and_rules(&engine.db.lock().unwrap(), &tag, to.as_deref())?;
            for account in &accounts {
                spawn_tag_push(&engine, account);
            }
            let queries = renamed_queries(p, &tag, to.as_deref());
            Ok(json!({ "ok": true, "tag": to, "rules": rules, "queries": queries }))
        }

        "messages.delete" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };

            let uids = {
                let db = engine.db.lock().unwrap();
                store::resolve_message_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &explicit_uids,
                )?
            };

            if uids.is_empty() {
                return mail_model::mutation_result(
                    json!({ "ok": true, "deleted": 0 }),
                    &engine.db.lock().unwrap(),
                    &account,
                    &changed_thread_id,
                    &folder,
                    None,
                    None,
                    None,
                    false,
                );
            }

            let cached_ids =
                store::cached_message_ids(&engine.db.lock().unwrap(), &account, &folder, &uids)?;
            let moved_ids = moved_message_ids(engine, &account, &folder, &uids, cached_ids).await;
            // Mutating, so it never auto-retries.
            let trashed = delete_to_trash(engine, &account, &folder, &uids).await?;
            // Undo moves exactly these copies back out of Trash.
            let copies = match &trashed {
                Some(trash) => {
                    let copies = refresh_moved_copies(
                        engine,
                        &account,
                        &trash.folder,
                        uids.len(),
                        trash.uid_next_before,
                        moved_ids.as_deref(),
                    )
                    .await;
                    copies.store(&engine.db.lock().unwrap(), &account, &trash.folder)?;
                    Some(copies)
                }
                None => None,
            };
            let trash_folder = trashed.as_ref().map(|trash| trash.folder.as_str());

            {
                let db = engine.db.lock().unwrap();
                // Discarding a draft must also drop hidden local copies sharing
                // its Message-ID (stale autosaves the pane deduped away), or the
                // thread card keeps its has_draft badge until the next full sync.
                if store::folder_role(&db, &account, &folder)? == "drafts" {
                    store::delete_draft_sibling_copies(&db, &account, &folder, &uids)?;
                }
                store::delete_messages_by_uid(&db, &account, &folder, &uids)?;
            }
            // The server delete/move-to-Trash completed for every resolved UID.
            // A concurrent source refresh may already have pruned the cache rows.
            let deleted = uids.len();
            let result = match (trash_folder, &copies) {
                (Some(trash), Some(copies)) => json!({
                    "ok": true,
                    "deleted": deleted,
                    "trash": trash,
                    "target_uids": copies.uids,
                }),
                _ => json!({ "ok": true, "deleted": deleted, "permanent": true }),
            };
            mail_model::mutation_result(
                result,
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                trash_folder,
                None,
                None,
                true,
            )
        }

        "messages.move" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let target_folder = canon_folder(&req_str(p, "target_folder")?);
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let changed_thread_id = if thread_key.is_empty() {
                uid.map(|uid| format!("{account}#{folder}#{uid}"))
                    .unwrap_or_default()
            } else {
                let key = subject_filter
                    .as_deref()
                    .map(|subject| store::branch_compound_key(&thread_key, subject))
                    .unwrap_or_else(|| thread_key.clone());
                mail_model::format_thread_id(&account, &folder, &key)
            };

            let target_account = req_str(p, "target_account").unwrap_or_else(|_| account.clone());
            if account == target_account && folder == target_folder {
                return mail_model::mutation_result(
                    json!({ "ok": true, "moved": 0, "source_folder": folder, "target_folder": target_folder }),
                    &engine.db.lock().unwrap(),
                    &account,
                    &changed_thread_id,
                    &folder,
                    Some(&target_folder),
                    None,
                    None,
                    false,
                );
            }

            let uids = {
                let db = engine.db.lock().unwrap();
                store::resolve_message_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &explicit_uids,
                )?
            };

            if uids.is_empty() {
                return mail_model::mutation_result(
                    json!({ "ok": true, "moved": 0, "source_folder": folder, "target_folder": target_folder }),
                    &engine.db.lock().unwrap(),
                    &account,
                    &changed_thread_id,
                    &folder,
                    Some(&target_folder),
                    None,
                    None,
                    false,
                );
            }

            if target_account != account {
                return move_between_accounts(
                    engine,
                    &account,
                    &folder,
                    &changed_thread_id,
                    &target_account,
                    &target_folder,
                    &uids,
                )
                .await;
            }

            let copies =
                move_within_account(engine, &account, &folder, &target_folder, &uids).await?;
            // The IMAP MOVE above completed for every resolved UID. A concurrent
            // source-folder refresh may already have pruned those rows locally,
            // so the cache DELETE count is not the number moved on the server.
            let moved = uids.len();
            mail_model::mutation_result(
                json!({
                    "ok": true,
                    "moved": moved,
                    "source_folder": folder,
                    "target_folder": target_folder,
                    "target_uids": copies.uids,
                }),
                &engine.db.lock().unwrap(),
                &account,
                &changed_thread_id,
                &folder,
                Some(&target_folder),
                None,
                None,
                true,
            )
        }

        "messages.copy" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let target_account = req_str(p, "target_account")?;
            let target_folder = canon_folder(&req_str(p, "target_folder")?);
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
            let explicit_uids = p
                .get("uids")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .map(|n| n as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let uids = {
                let db = engine.db.lock().unwrap();
                store::resolve_message_uids(
                    &db,
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &explicit_uids,
                )?
            };

            if uids.is_empty() {
                return Ok(json!({
                    "ok": true,
                    "copied": 0,
                    "source_folder": folder,
                    "target_account": target_account,
                    "target_folder": target_folder
                }));
            }

            let raw_messages = engine
                .with_read_session(&account, |session| {
                    let folder = folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::fetch_raw_messages_for_copy(session, &folder, &uids).await
                    })
                })
                .await?;

            if raw_messages.is_empty() {
                return Ok(json!({
                    "ok": true,
                    "copied": 0,
                    "source_folder": folder,
                    "target_account": target_account,
                    "target_folder": target_folder
                }));
            }

            let copied = raw_messages.len();
            engine
                .with_write_session(&target_account, |session| {
                    let target_folder = target_folder.clone();
                    let raw_messages = raw_messages.clone();
                    Box::pin(async move {
                        imap::append_copied_messages(session, &target_folder, &raw_messages).await
                    })
                })
                .await?;
            {
                let db = engine.db.lock().unwrap();
                let copies = std::collections::BTreeMap::from([(folder.clone(), uids.clone())]);
                let tags = store::carried_tags(&db, &account, &copies)?;
                store::apply_carried_tags(&db, &target_account, tags, true)?;
            }
            // Read-only refresh, on its own session; see the move handler above.
            let copied_result = json!({
                "ok": true,
                "copied": copied,
                "source_folder": folder,
                "target_account": target_account,
                "target_folder": target_folder
            });
            let Some(refresh) = meron_core::engine::refresh_written_folder(
                engine,
                &target_account,
                &target_folder,
                50.max(raw_messages.len() as u32),
            )
            .await
            else {
                return Ok(copied_result);
            };
            let target_batch = &refresh.batch;

            {
                let db = engine.db.lock().unwrap();
                store::ensure_folder(&db, &target_account, &target_folder)?;
                store::upsert_messages(
                    &db,
                    &target_account,
                    &target_folder,
                    &target_batch.messages,
                )?;
                store::set_folder_state(
                    &db,
                    &target_account,
                    &target_folder,
                    target_batch.uidvalidity,
                    target_batch.uid_next,
                )?;
            }

            Ok(copied_result)
        }

        // Mark every message in a folder as read: set \Seen on the server for the
        // currently-unseen UIDs, then flip the whole folder seen in the store.
        "messages.markAllRead" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));

            let uids = {
                let db = engine.db.lock().unwrap();
                store::get_unseen_uids(&db, &account, &folder)?
            };

            if !uids.is_empty() {
                engine
                    .with_flag_write_session(
                        &account,
                        |session| {
                            let folder = folder.clone();
                            Box::pin(
                                async move { imap::prepare_flag_update(session, &folder).await },
                            )
                        },
                        |session| {
                            let uids = uids.clone();
                            Box::pin(async move { imap::store_seen(session, &uids, true).await })
                        },
                    )
                    .await?;
            }

            {
                let db = engine.db.lock().unwrap();
                store::mark_folder_seen(&db, &account, &folder, true)?;
            }
            mail_model::mutation_result(
                json!({ "ok": true, "updated": uids.len(), "folder": folder }),
                &engine.db.lock().unwrap(),
                &account,
                "",
                &folder,
                None,
                Some(false),
                None,
                false,
            )
        }

        "messages.markAllReadUnified" => {
            let role = req_str(p, "folder").unwrap_or_else(|_| "inbox".to_string());
            let account_folders = {
                let db = engine.db.lock().unwrap();
                store::list_accounts(&db)?
                    .into_iter()
                    .filter(|account| {
                        account
                            .get("included_in_unified")
                            .and_then(Value::as_bool)
                            .unwrap_or(true)
                    })
                    .filter(|account| {
                        account.get("auth_type").and_then(Value::as_str) != Some("rss")
                    })
                    .filter_map(|account| {
                        account
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                    .map(|account| {
                        let folder = store::folder_for_role(&db, &account, &role)?;
                        Ok(folder.map(|folder| (account, folder)))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            };
            let mut updated = 0_u64;
            let mut failures = Vec::new();
            let mut folder_unreads = serde_json::Map::new();
            let mut folder_counts = Vec::new();
            for (account, folder) in account_folders {
                let request = Request {
                    id: req.id,
                    method: "messages.markAllRead".to_string(),
                    params: json!({ "account": account, "folder": folder }),
                };
                match Box::pin(dispatch(engine, &request, out)).await {
                    Ok(result) => {
                        updated += result
                            .get("updated")
                            .and_then(Value::as_u64)
                            .unwrap_or_default();
                        if let Some(counts) = result
                            .get("folder_unreads")
                            .and_then(|all| all.get(&account))
                        {
                            folder_unreads.insert(account.clone(), counts.clone());
                        }
                        folder_counts.extend(
                            result
                                .get("folder_counts")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                    Err(err) => failures
                        .push(json!({ "account_id": account, "message": format!("{err:#}") })),
                }
            }
            Ok(json!({
                "ok": failures.is_empty(),
                "updated": updated,
                "failures": failures,
                "folder_unreads": folder_unreads,
                "folder_counts": folder_counts,
            }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
