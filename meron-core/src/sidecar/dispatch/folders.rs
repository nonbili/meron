use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::protocol::Request;
use meron_core::{imap, mail_model, rss, store};

use crate::Writer;
use crate::sidecar::idle::{
    set_view_watch, start_notify_folder_watches, stop_idle_watch, view_watches,
};
use crate::sidecar::params::*;
use crate::sidecar::spawn::*;

/// Handle folder listing and management: `folders.*`, `messages.emptyFolder`.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        // Cache-only (instant). When refresh != false, also kicks a background
        // sync that emits mail.synced; event-driven reloads pass refresh:false to
        // avoid a sync→event→reload loop.
        // RSS accounts return final Folder JSON (one synthetic Inbox); mail
        // returns raw rows the bridge formats. Routed by the account's engine.
        "folders.list" => {
            let account = req_str(p, "account")?;
            let read_account = account.clone();
            let (folders, rss_account) =
                crate::sidecar::db::run(&engine.db, "folders.list", move |db| {
                    let rss_account =
                        store::account_engine(db, &read_account)?.as_deref() == Some("rss");
                    let folders = if rss_account {
                        serde_json::to_value(rss::folders(db, &read_account)?)?
                    } else {
                        serde_json::to_value(store::get_folders(db, &read_account)?)?
                    };
                    Ok((folders, rss_account))
                })
                .await?;
            if !rss_account && p.get("refresh").and_then(Value::as_bool).unwrap_or(true) {
                spawn_folder_sync(engine.clone(), out.clone(), account);
            }
            Ok(json!({ "folders": folders }))
        }

        "folders.create" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                return Err(anyhow::anyhow!("RSS accounts do not support folders"));
            }
            let display_name = req_str(p, "name")?.trim().to_string();
            if display_name.is_empty() {
                return Err(anyhow::anyhow!("Folder name is required"));
            }
            // The user types UTF-8; servers without UTF8=ACCEPT want modified
            // UTF-7, and the wire form is what we store and address it by.
            let name = meron_core::utf7::encode(&display_name);

            engine
                .with_write_session(&account, |session| {
                    let name = name.clone();
                    Box::pin(async move { imap::create_folder(session, &name).await })
                })
                .await?;

            let folder = imap::Folder {
                name,
                display_name,
                delimiter: None,
                ..Default::default()
            };
            {
                let db = engine.db.lock().unwrap();
                store::upsert_folders(&db, &account, std::slice::from_ref(&folder))?;
            }
            Ok(json!({ "folders": serde_json::to_value(vec![folder])? }))
        }

        // Opt a folder in to (or out of) live sync and new-mail notifications.
        // The first few opted in are watched over IDLE and the rest polled
        // (`store::notify_folder_plan`). Disabling stops the folder's watch
        // unless a view (a kanban column showing it) still wants it, and may
        // promote a polled folder into the freed live slot.
        "folders.setNotify" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                return Err(anyhow::anyhow!("RSS accounts do not support folders"));
            }
            let folder = canon_folder(&req_str(p, "folder")?);
            let enabled = req_bool(p, "enabled")?;
            let folders = {
                let db = engine.db.lock().unwrap();
                if !store::set_folder_notify(&db, &account, &folder, enabled)? {
                    return Err(anyhow::anyhow!("Folder not found: {folder}"));
                }
                store::get_folders(&db, &account)?
            };
            // INBOX is always watched, whatever the flag says.
            if !enabled && !folder.eq_ignore_ascii_case("INBOX") && !view_watches(&account, &folder)
            {
                stop_idle_watch(engine, &account, &folder);
            }
            if !engine.is_paused(&account) {
                start_notify_folder_watches(engine, out, &account);
            }
            Ok(json!({ "ok": true, "folders": serde_json::to_value(folders)? }))
        }

        // Delete a folder and everything nested under it on the server, then
        // forget the whole subtree's cache. Unrecoverable, so the special-use
        // gate is re-checked here rather than trusted from the caller.
        "folders.delete" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                return Err(anyhow::anyhow!("RSS accounts do not support folders"));
            }
            let folder = canon_folder(&req_str(p, "folder").or_else(|_| req_str(p, "name"))?);
            let targets = {
                let db = engine.db.lock().unwrap();
                mail_model::check_folder_deletable(&db, &account, &folder)
                    .map_err(anyhow::Error::msg)?;
                mail_model::folder_delete_targets(&db, &account, &folder)
                    .map_err(anyhow::Error::msg)?
            };

            // EXAMINE is a read-only preflight and may retry on a stale pooled
            // socket. DELETE itself starts only after that succeeds and is
            // never retried.
            let server_result = engine
                .with_preflighted_write_session(
                    &account,
                    |session| Box::pin(imap::prepare_folder_delete(session)),
                    |session| {
                        let targets = targets.clone();
                        Box::pin(async move { imap::delete_folders(session, &targets).await })
                    },
                )
                .await;
            let (removed, warning) = match server_result {
                Ok(removed) => (removed, None),
                Err(err) => match err.downcast::<imap::PartialFolderDelete>() {
                    Ok(partial) => {
                        let (removed, warning) = partial.into_parts();
                        (removed, Some(warning))
                    }
                    Err(err) => return Err(err),
                },
            };

            let (deleted, folders) = {
                let db = engine.db.lock().unwrap();
                let mut deleted = 0;
                for target in &removed {
                    deleted += store::delete_folder(&db, &account, target)?;
                    // Nothing is left to watch; a watcher would only fail to
                    // SELECT the mailbox and reconnect forever.
                    set_view_watch(&account, target, false);
                    stop_idle_watch(engine, &account, target);
                }
                (deleted, store::get_folders(&db, &account)?)
            };
            // A deleted opted-in folder may have freed a live slot.
            if !removed.is_empty() && !engine.is_paused(&account) {
                start_notify_folder_watches(engine, out, &account);
            }
            Ok(json!({
                "ok": warning.is_none(),
                "folder": folder,
                // Every folder that went with it, so the caller can clear the
                // views and caches keyed on a nested folder as well.
                "removed": removed,
                "deleted": deleted,
                "folders": serde_json::to_value(folders)?,
                "warning": warning,
            }))
        }

        // Rename a folder, and everything nested under it, on the server. `name`
        // is the folder's new full path as the user reads it. Special-use
        // folders are refused here rather than trusted from the caller.
        "folders.rename" => {
            let account = req_str(p, "account")?;
            if is_rss(engine, &account)? {
                return Err(anyhow::anyhow!("RSS accounts do not support folders"));
            }
            let folder = canon_folder(&req_str(p, "folder")?);
            let display_name = req_str(p, "name")?.trim().to_string();
            if display_name.is_empty() {
                return Err(anyhow::anyhow!("Folder name is required"));
            }
            // Addressed by its wire form, as `folders.create` stores it.
            let name = meron_core::utf7::encode(&display_name);
            let pairs = {
                let db = engine.db.lock().unwrap();
                mail_model::check_folder_renamable(&db, &account, &folder)
                    .map_err(anyhow::Error::msg)?;
                if name == folder {
                    return Ok(json!({
                        "ok": true,
                        "folder": folder,
                        "renamed": [],
                        "folders": serde_json::to_value(store::get_folders(&db, &account)?)?,
                    }));
                }
                let delimiter = store::get_folders(&db, &account)?
                    .into_iter()
                    .find(|item| item.name == folder)
                    .and_then(|item| item.delimiter)
                    .filter(|delimiter| !delimiter.is_empty());
                if delimiter
                    .is_some_and(|delimiter| name.starts_with(&format!("{folder}{delimiter}")))
                {
                    return Err(anyhow::anyhow!("A folder cannot be moved inside itself"));
                }
                if store::folder_exists(&db, &account, &name)? {
                    return Err(anyhow::anyhow!(
                        "A folder named {display_name} already exists"
                    ));
                }
                store::folder_rename_pairs(&db, &account, &folder, &name)?
            };

            // EXAMINE is a read-only preflight and may retry on a stale pooled
            // socket. RENAME itself is never retried.
            engine
                .with_preflighted_write_session(
                    &account,
                    |session| Box::pin(imap::prepare_folder_delete(session)),
                    |session| {
                        let (folder, name) = (folder.clone(), name.clone());
                        Box::pin(async move { imap::rename_folder(session, &folder, &name).await })
                    },
                )
                .await?;

            let folders = {
                let db = engine.db.lock().unwrap();
                store::rename_folder(&db, &account, &pairs)?;
                for (from, _) in &pairs {
                    // The old name is gone; a watcher would only fail to
                    // SELECT it and reconnect forever.
                    set_view_watch(&account, from, false);
                    stop_idle_watch(engine, &account, from);
                }
                store::get_folders(&db, &account)?
            };
            if !engine.is_paused(&account) {
                start_notify_folder_watches(engine, out, &account);
            }
            // Pick up what the server made of the new names.
            spawn_folder_sync(engine.clone(), out.clone(), account);
            Ok(json!({
                "ok": true,
                "folder": folder,
                // Every folder that moved, so the caller can repoint the views
                // and caches keyed on a nested folder as well.
                "renamed": pairs,
                "folders": serde_json::to_value(folders)?,
            }))
        }

        // Permanently delete every message in a folder, server side and in the
        // store. Restricted to Trash and Junk: the operation is unrecoverable,
        // so an arbitrary folder must never reach it even if a caller asks.
        "messages.emptyFolder" => {
            let account = req_str(p, "account")?;
            let folder = canon_folder(&req_str(p, "folder")?);
            let role = {
                let db = engine.db.lock().unwrap();
                store::folder_role(&db, &account, &folder)?
            };
            if role != "trash" && role != "junk" {
                return Err(anyhow::anyhow!(
                    "Only Trash and Junk folders can be emptied"
                ));
            }

            // Mutating, so it never auto-retries.
            let expunged = engine
                .with_write_session(&account, |session| {
                    let folder = folder.clone();
                    Box::pin(async move { imap::empty_folder(session, &folder).await })
                })
                .await?;

            let deleted = {
                let db = engine.db.lock().unwrap();
                store::delete_folder_messages(&db, &account, &folder)?
            };
            mail_model::mutation_result(
                json!({
                    "ok": true,
                    "deleted": deleted,
                    "expunged": expunged,
                    "folder": folder,
                    "role": role,
                }),
                &engine.db.lock().unwrap(),
                &account,
                "",
                &folder,
                None,
                None,
                None,
                true,
            )
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
