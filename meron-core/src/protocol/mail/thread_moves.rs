use super::super::*;
use super::{
    ParsedThreadId, cached_thread_uids, canon_folder, find_cached_archive_folder, format_thread_id,
    parse_thread_id, parse_thread_id_with_request_folder, requested_mobile_uids,
};

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
        let cached_ids = store::cached_message_ids(&conn, &parsed.account, &parsed.folder, &uids)
            .map_err(|err| err.to_string())?;
        let moved_ids = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::moved_message_ids(
                    &engine,
                    &parsed.account,
                    &parsed.folder,
                    &uids,
                    cached_ids,
                )
                .await,
            )
        })?;
        let delete_result = crate::ffi::engine_block_on(crate::engine::delete_to_trash(
            &engine,
            &parsed.account,
            &parsed.folder,
            &uids,
        ))?;
        // Undo moves exactly these copies back out of Trash.
        let trash_copies = match &delete_result {
            Some(trashed) => {
                let copies = crate::ffi::engine_block_on(async {
                    anyhow::Ok(
                        crate::engine::refresh_moved_copies(
                            &engine,
                            &parsed.account,
                            &trashed.folder,
                            uids.len(),
                            trashed.uid_next_before,
                            moved_ids.as_deref(),
                        )
                        .await,
                    )
                })?;
                copies
                    .store(&conn, &parsed.account, &trashed.folder)
                    .map_err(|err| err.to_string())?;
                copies.uids
            }
            None => Vec::new(),
        };
        // Discarding a draft must also drop hidden local copies sharing its
        // Message-ID (stale autosaves the pane deduped away), or the thread
        // card keeps its has_draft badge until the next full sync.
        if store::folder_role(&conn, &parsed.account, &parsed.folder)
            .map_err(|err| err.to_string())?
            == "drafts"
        {
            store::delete_draft_sibling_copies(&conn, &parsed.account, &parsed.folder, &uids)
                .map_err(|err| err.to_string())?;
        }
        store::delete_messages_by_uid(&conn, &parsed.account, &parsed.folder, &uids)
            .map_err(|err| err.to_string())?;
        // Report the successful server operation, not the racy local row count.
        let deleted = uids.len();
        match delete_result {
            None => crate::mail_model::mutation_result(
                json!({ "ok": true, "deleted": deleted, "permanent": true }),
                &conn,
                &parsed.account,
                &thread_id,
                &parsed.folder,
                None,
                None,
                None,
                true,
            ),
            Some(crate::engine::Trashed { folder: trash, .. }) => {
                let moved_thread_id = format_thread_id(&parsed.account, &trash, &parsed.thread_key);
                crate::mail_model::mutation_result(
                    json!({
                        "ok": true,
                        "deleted": deleted,
                        "trash": trash,
                        "thread_id": moved_thread_id,
                        "target_uids": trash_copies,
                    }),
                    &conn,
                    &parsed.account,
                    &thread_id,
                    &parsed.folder,
                    Some(&trash),
                    None,
                    None,
                    true,
                )
            }
        }
        .map_err(|err| err.to_string())
    })
}

pub(crate) fn archive_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let target_folder = find_cached_archive_folder(&conn, &parsed.account)?;
        let (source_folder, in_place) = match crate::mail_model::archive_plan(
            &parsed.folder,
            &parsed.thread_key,
            &target_folder,
        ) {
            crate::mail_model::ArchivePlan::AlreadyArchived => {
                return Ok(crate::mail_model::already_archived_result(
                    &target_folder,
                    &thread_id,
                ));
            }
            crate::mail_model::ArchivePlan::Move {
                source_folder,
                in_place,
            } => (source_folder, in_place),
        };
        // Archived in place, the thread is still listed under the id it came
        // with; the messages leave the same thread in the Inbox.
        let in_folder = |folder: &str| {
            crate::mail_model::thread_id_in_folder(&thread_id, folder)
                .ok_or_else(|| "invalid thread_id".to_string())
        };
        let (source_thread_id, moved_thread_id) = if in_place {
            (in_folder(&source_folder)?, thread_id.clone())
        } else {
            (thread_id.clone(), in_folder(&target_folder)?)
        };
        let parsed = ParsedThreadId {
            folder: source_folder,
            ..parsed.clone()
        };
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            let thread_id = if in_place {
                &moved_thread_id
            } else {
                &thread_id
            };
            return Ok(crate::mail_model::archive_result(
                json!({ "ok": true, "moved": 0 }),
                &target_folder,
                thread_id,
                in_place,
            ));
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let cached_ids = store::cached_message_ids(&conn, &parsed.account, &parsed.folder, &uids)
            .map_err(|err| err.to_string())?;
        let moved_ids = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::moved_message_ids(
                    &engine,
                    &parsed.account,
                    &parsed.folder,
                    &uids,
                    cached_ids,
                )
                .await,
            )
        })?;
        let op_folder = parsed.folder.clone();
        let op_target = target_folder.clone();
        let op_uids = uids.clone();
        let uid_next_before = crate::ffi::engine_block_on(engine.with_write_session(
            &parsed.account,
            move |session| {
                let source = op_folder.clone();
                let target = op_target.clone();
                let uids = op_uids.clone();
                Box::pin(
                    async move { imap::move_to_folder(session, &source, &target, &uids).await },
                )
            },
        ))?;
        let copies = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::refresh_moved_copies(
                    &engine,
                    &parsed.account,
                    &target_folder,
                    uids.len(),
                    uid_next_before,
                    moved_ids.as_deref(),
                )
                .await,
            )
        })?;
        copies
            .store(&conn, &parsed.account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::move_messages_by_uid(
            &conn,
            &parsed.account,
            &parsed.folder,
            &target_folder,
            &uids,
        )
        .map_err(|err| err.to_string())?;
        // The server MOVE succeeded for all resolved UIDs. A concurrent sync
        // can remove a source cache row before this local reconciliation runs.
        let moved = uids.len();
        let result = crate::mail_model::mutation_result(
            json!({
                "ok": true,
                "moved": moved,
                "source_folder": parsed.folder,
                "target_uids": copies.uids,
            }),
            &conn,
            &parsed.account,
            &source_thread_id,
            &parsed.folder,
            Some(&target_folder),
            None,
            None,
            true,
        )
        .map_err(|err| err.to_string())?;
        Ok(crate::mail_model::archive_result(
            result,
            &target_folder,
            &moved_thread_id,
            in_place,
        ))
    })
}

pub(crate) fn move_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_folder = canon_folder(&req_str(params, "target_folder_id")?);
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    let target_account = params
        .get("target_account_id")
        .and_then(Value::as_str)
        .unwrap_or(&parsed.account);
    if target_account != parsed.account {
        let engine = crate::ffi::engine_for(data_dir)?;
        return with_mobile_db(data_dir, |conn| {
            let uids = requested_mobile_uids(&conn, &parsed, params)?;
            if !uids.is_empty() {
                for account in [parsed.account.as_str(), target_account] {
                    let creds = load_mobile_account_creds(&conn, account)?;
                    if account_needs_reconnect(&creds) {
                        return Err(format!("account needs reconnect: {account}"));
                    }
                }
            }
            crate::ffi::engine_block_on(crate::engine::move_between_accounts(
                &engine,
                &parsed.account,
                &parsed.folder,
                &thread_id,
                target_account,
                &target_folder,
                &uids,
            ))
        });
    }
    if parsed.folder == target_folder {
        return Ok(
            json!({ "ok": true, "moved": 0, "folder": target_folder, "thread_id": thread_id }),
        );
    }
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        if uids.is_empty() {
            return Ok(
                json!({ "ok": true, "moved": 0, "folder": target_folder, "thread_id": thread_id }),
            );
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let cached_ids = store::cached_message_ids(&conn, &parsed.account, &parsed.folder, &uids)
            .map_err(|err| err.to_string())?;
        let moved_ids = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::moved_message_ids(
                    &engine,
                    &parsed.account,
                    &parsed.folder,
                    &uids,
                    cached_ids,
                )
                .await,
            )
        })?;
        let op_folder = parsed.folder.clone();
        let op_target = target_folder.clone();
        let op_uids = uids.clone();
        let uid_next_before = crate::ffi::engine_block_on(engine.with_write_session(
            &parsed.account,
            move |session| {
                let source = op_folder.clone();
                let target = op_target.clone();
                let uids = op_uids.clone();
                Box::pin(
                    async move { imap::move_to_folder(session, &source, &target, &uids).await },
                )
            },
        ))?;
        let copies = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::refresh_moved_copies(
                    &engine,
                    &parsed.account,
                    &target_folder,
                    uids.len(),
                    uid_next_before,
                    moved_ids.as_deref(),
                )
                .await,
            )
        })?;
        copies
            .store(&conn, &parsed.account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::move_messages_by_uid(
            &conn,
            &parsed.account,
            &parsed.folder,
            &target_folder,
            &uids,
        )
        .map_err(|err| err.to_string())?;
        // Report the successful server operation, not the racy local row count.
        let moved = uids.len();
        let moved_thread_id = format_thread_id(&parsed.account, &target_folder, &parsed.thread_key);
        crate::mail_model::mutation_result(
            json!({
                "ok": true,
                "moved": moved,
                "folder": target_folder,
                "thread_id": moved_thread_id,
                "target_uids": copies.uids,
            }),
            &conn,
            &parsed.account,
            &thread_id,
            &parsed.folder,
            Some(&target_folder),
            None,
            None,
            true,
        )
        .map_err(|err| err.to_string())
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
        crate::ffi::engine_block_on(engine.with_write_session(&target_account, move |session| {
            let target = target_folder_op.clone();
            let raw_messages = raw_messages.clone();
            Box::pin(
                async move { imap::append_copied_messages(session, &target, &raw_messages).await },
            )
        }))?;
        let copies = std::collections::BTreeMap::from([(parsed.folder.clone(), uids.clone())]);
        store::carried_tags(&conn, &parsed.account, &copies)
            .and_then(|tags| store::apply_carried_tags(&conn, &target_account, tags, true))
            .map_err(|err| err.to_string())?;
        // Read-only refresh, on its own session; see the move handlers above.
        let copied_result = json!({
            "ok": true,
            "copied": copied,
            "source_folder": parsed.folder,
            "target_account": target_account,
            "target_folder": target_folder,
        });
        let refresh = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::refresh_written_folder(
                    &engine,
                    &target_account,
                    &target_folder,
                    50.max(copied as u32),
                )
                .await,
            )
        })?;
        let Some(refresh) = refresh else {
            return Ok(copied_result);
        };
        let batch = &refresh.batch;
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
        Ok(copied_result)
    })
}
