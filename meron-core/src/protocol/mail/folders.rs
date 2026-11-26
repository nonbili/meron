use super::super::*;
use super::canon_folder;

pub(crate) fn list_mobile_folders(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            let folders = rss::folders(&conn, &account_id).map_err(|err| format!("{err:#}"))?;
            return Ok(json!({ "folders": folders }));
        }
        let folders = store::get_folders(&conn, &account_id).map_err(|err| err.to_string())?;
        let live = store::notify_folder_plan(&conn, &account_id)
            .map_err(|err| err.to_string())?
            .live;
        let folders = folders
            .into_iter()
            .map(|folder| {
                let role = folder.role.clone();
                // `name` stays the wire name — mobile addresses mailboxes by it
                // (SELECT, sync, cached rows) — and `display_name` carries the
                // modified-UTF-7-decoded label for rendering.
                let display_name = if folder.display_name.is_empty() {
                    folder.name.clone()
                } else {
                    folder.display_name.clone()
                };
                json!({
                    "id": folder.name,
                    "account_id": account_id,
                    "name": folder.name,
                    "display_name": display_name,
                    "role": role,
                    "special_use": folder.special_use,
                    "delimiter": folder.delimiter.unwrap_or_default(),
                    "unread": folder.unread,
                    "notify": folder.notify,
                    // Opted in and holding one of the account's IDLE slots;
                    // the host's live push watches these, core polls the rest.
                    "notify_live": live.contains(&folder.name),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "folders": folders }))
    })
}

/// Opt a folder in to (or out of) background sync and new-mail notifications.
/// Answers with the refreshed folder list, like the other folder mutations.
pub(crate) fn set_mobile_folder_notify(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    let folder = canon_folder(&req_str(params, "folder_id")?);
    let enabled = params
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "enabled is required".to_string())?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("RSS accounts do not support folders".to_string());
        }
        if !store::set_folder_notify(&conn, &account_id, &folder, enabled)
            .map_err(|err| err.to_string())?
        {
            return Err(format!("Folder not found: {folder}"));
        }
        Ok(json!({ "ok": true }))
    })?;
    crate::ffi::sync_foreground_notify_watch(data_dir, &account_id, &folder, enabled);
    list_mobile_folders(data_dir, params)
}

pub(crate) fn create_mobile_folder(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    let display_name = req_str(params, "name")?.trim().to_string();
    if display_name.is_empty() {
        return Err("Folder name is required".to_string());
    }
    // Servers without UTF8=ACCEPT address mailboxes in modified UTF-7.
    let name = crate::utf7::encode(&display_name);
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("RSS accounts do not support folders".to_string());
        }
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        // Go through Engine so browser-flow OAuth is refreshed before opening a
        // new IMAP connection and the foreground session pool is reused.
        let folders =
            crate::ffi::engine_block_on(engine.with_write_session(&account_id, move |session| {
                let name = name.clone();
                Box::pin(async move {
                    imap::create_folder(session, &name).await?;
                    imap::list_folders(session).await
                })
            }))?;
        store::upsert_folders(&conn, &account_id, &folders).map_err(|err| err.to_string())?;
        list_mobile_folders(data_dir, params)
    })
}

// Delete a folder and everything nested under it on the server, then drop the
// whole subtree's cache. The special-use gate is re-checked here rather than
// trusted from the caller: the delete takes the folders' mail with it.
pub(crate) fn delete_mobile_folder(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .or_else(|| params.get("name"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .ok_or_else(|| "folder_id is required".to_string())?;
    if account_id == "unified" {
        return Err("Pick a single account to delete a folder".to_string());
    }

    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("RSS accounts do not support folders".to_string());
        }
        crate::mail_model::check_folder_deletable(&conn, &account_id, &folder)?;
        let targets = crate::mail_model::folder_delete_targets(&conn, &account_id, &folder)?;
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let server_result = {
            let targets = targets.clone();
            crate::ffi::engine_block_on_anyhow(engine.with_preflighted_write_session(
                &account_id,
                |session| Box::pin(imap::prepare_folder_delete(session)),
                move |session| {
                    let targets = targets.clone();
                    Box::pin(async move { imap::delete_folders(session, &targets).await })
                },
            ))
        };
        let (removed, warning) = match server_result {
            Ok(removed) => (removed, None),
            Err(err) => match err.downcast::<imap::PartialFolderDelete>() {
                Ok(partial) => {
                    let (removed, warning) = partial.into_parts();
                    (removed, Some(warning))
                }
                Err(err) => return Err(err.to_string()),
            },
        };
        let mut deleted = 0;
        for target in &removed {
            deleted +=
                store::delete_folder(&conn, &account_id, target).map_err(|err| err.to_string())?;
            // Nothing is left to watch; a watcher would only fail to SELECT
            // the mailbox and reconnect forever.
            crate::ffi::forget_deleted_folder_watch(&account_id, target);
        }
        // A deleted opted-in folder may have freed a live slot.
        if !removed.is_empty() {
            crate::ffi::start_live_notify_watches(data_dir, &account_id);
        }
        let mut response = list_mobile_folders(data_dir, params)?;
        response["ok"] = json!(warning.is_none());
        response["folder"] = json!(folder);
        // Every folder that went with it, so the caller can clear the views and
        // caches keyed on a nested folder as well.
        response["removed"] = json!(removed);
        response["deleted"] = json!(deleted);
        response["warning"] = json!(warning);
        Ok(response)
    })
}

// Rename a folder, and everything nested under it, on the server. `name` is the
// folder's new full path as the user reads it. Special-use folders are refused
// here rather than trusted from the caller, like a delete.
pub(crate) fn rename_mobile_folder(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .ok_or_else(|| "folder_id is required".to_string())?;
    let display_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if display_name.is_empty() {
        return Err("Folder name is required".to_string());
    }
    if account_id == "unified" {
        return Err("Pick a single account to rename a folder".to_string());
    }
    // Addressed by its wire form, as `mail.folderCreate` stores it.
    let name = crate::utf7::encode(&display_name);

    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("RSS accounts do not support folders".to_string());
        }
        crate::mail_model::check_folder_renamable(&conn, &account_id, &folder)?;
        if name == folder {
            let mut response = list_mobile_folders(data_dir, params)?;
            response["ok"] = json!(true);
            response["folder"] = json!(folder);
            response["renamed"] = json!([]);
            return Ok(response);
        }
        let delimiter = store::get_folders(&conn, &account_id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|item| item.name == folder)
            .and_then(|item| item.delimiter)
            .filter(|delimiter| !delimiter.is_empty());
        if delimiter.is_some_and(|delimiter| name.starts_with(&format!("{folder}{delimiter}"))) {
            return Err("A folder cannot be moved inside itself".to_string());
        }
        if store::folder_exists(&conn, &account_id, &name).map_err(|err| err.to_string())? {
            return Err(format!("A folder named {display_name} already exists"));
        }
        let pairs = store::folder_rename_pairs(&conn, &account_id, &folder, &name)
            .map_err(|err| err.to_string())?;
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        // EXAMINE is a read-only preflight and may retry on a stale pooled
        // socket. RENAME itself is never retried.
        {
            let (folder, name) = (folder.clone(), name.clone());
            crate::ffi::engine_block_on(engine.with_preflighted_write_session(
                &account_id,
                |session| Box::pin(imap::prepare_folder_delete(session)),
                move |session| {
                    let (folder, name) = (folder.clone(), name.clone());
                    Box::pin(async move { imap::rename_folder(session, &folder, &name).await })
                },
            ))?;
        }
        store::rename_folder(&conn, &account_id, &pairs).map_err(|err| err.to_string())?;
        for (from, _) in &pairs {
            // The old name is gone; a watcher would only fail to SELECT it and
            // reconnect forever.
            crate::ffi::forget_deleted_folder_watch(&account_id, from);
        }
        // Opted-in folders keep their opt-in under the new names.
        crate::ffi::start_live_notify_watches(data_dir, &account_id);
        let mut response = list_mobile_folders(data_dir, params)?;
        response["ok"] = json!(true);
        response["folder"] = json!(folder);
        // Every folder that moved, as [old, new] pairs, so the caller can
        // repoint the views and caches keyed on a nested folder as well.
        response["renamed"] = json!(pairs);
        Ok(response)
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

    if account_id == "unified" {
        let accounts = with_mobile_db(data_dir, |conn| {
            let accounts = store::list_accounts(&conn).map_err(|err| err.to_string())?;
            Ok(json!(
                accounts
                    .into_iter()
                    .filter(|account| account
                        .get("included_in_unified")
                        .and_then(Value::as_bool)
                        .unwrap_or(true))
                    .filter(
                        |account| account.get("auth_type").and_then(Value::as_str) != Some("rss")
                    )
                    .filter_map(|account| account
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string))
                    .collect::<Vec<_>>()
            ))
        })?
        .as_array()
        .cloned()
        .unwrap_or_default();
        let accounts = accounts
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        let account_folders = with_mobile_db(data_dir, |conn| {
            let mut resolved = Vec::new();
            for account in accounts {
                if let Some(account_folder) = store::folder_for_role(&conn, &account, &folder)
                    .map_err(|err| err.to_string())?
                {
                    resolved.push((account, account_folder));
                }
            }
            Ok(json!(resolved))
        })?;
        let account_folders = serde_json::from_value::<Vec<(String, String)>>(account_folders)
            .map_err(|err| format!("{err:#}"))?;
        let mut updated = 0_u64;
        let mut failures = Vec::new();
        let mut folder_unreads = serde_json::Map::new();
        let mut folder_counts = Vec::new();
        for (account, account_folder) in account_folders {
            let account_params = json!({ "account_id": account, "folder_id": account_folder });
            match mark_mobile_folder_all_read(data_dir, &account_params) {
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
                Err(message) => failures.push(json!({ "account_id": account, "message": message })),
            }
        }
        return Ok(json!({
            "ok": failures.is_empty(),
            "updated": updated,
            "failures": failures,
            "folder_unreads": folder_unreads,
            "folder_counts": folder_counts,
        }));
    }

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
            let select_folder = folder.clone();
            crate::ffi::engine_block_on(engine.with_flag_write_session(
                &account_id,
                move |session| {
                    let folder = select_folder.clone();
                    Box::pin(async move { imap::prepare_flag_update(session, &folder).await })
                },
                move |session| {
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::store_seen(session, &uids, true).await?;
                        anyhow::Ok(())
                    })
                },
            ))?;
        }
        store::mark_folder_seen(&conn, &account_id, &folder, true)
            .map_err(|err| err.to_string())?;
        crate::mail_model::mutation_result(
            json!({ "ok": true, "updated": updated, "folder": folder }),
            &conn,
            &account_id,
            "",
            &folder,
            None,
            Some(false),
            None,
            false,
        )
        .map_err(|err| err.to_string())
    })
}

// Permanently delete every message in a folder, server side and in the store.
// Restricted to Trash and Junk, and re-checked here rather than trusting the
// caller: the delete is unrecoverable.
pub(crate) fn empty_mobile_folder(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .ok_or_else(|| "folder_id is required".to_string())?;
    if account_id == "unified" {
        return Err("Only Trash and Junk folders can be emptied".to_string());
    }

    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("Only Trash and Junk folders can be emptied".to_string());
        }
        let role =
            store::folder_role(&conn, &account_id, &folder).map_err(|err| err.to_string())?;
        if role != "trash" && role != "junk" {
            return Err("Only Trash and Junk folders can be emptied".to_string());
        }
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let expunged = {
            let folder = folder.clone();
            crate::ffi::engine_block_on(engine.with_write_session(&account_id, move |session| {
                let folder = folder.clone();
                Box::pin(async move { imap::empty_folder(session, &folder).await })
            }))?
        };
        let deleted = store::delete_folder_messages(&conn, &account_id, &folder)
            .map_err(|err| err.to_string())?;
        crate::mail_model::mutation_result(
            json!({
                "ok": true,
                "deleted": deleted,
                "expunged": expunged,
                "folder": folder,
                "role": role,
            }),
            &conn,
            &account_id,
            "",
            &folder,
            None,
            None,
            None,
            true,
        )
        .map_err(|err| err.to_string())
    })
}
