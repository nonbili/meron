//! Local folders in the sidecar: requests for the local account, and moves
//! between it and server accounts.
//!
//! The local account's mail lives in the archive (`store::archive`), behind an
//! [`Engine`] of its own that has no server accounts. Reads go through the same
//! list and thread code as server mail, pointed at that engine; writes never
//! leave the machine, except an upload back to a server account.

use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

use meron_core::engine::{Engine, EngineHost};
use meron_core::protocol::Request;
use meron_core::store::LOCAL_ACCOUNT_ID;
use meron_core::{engine, imap, mail_model, parse, secrets, store, thread_list, thread_read};

use crate::Writer;
use crate::sidecar::params::*;

/// Messages fetched, archived and removed from the server per round, so a large
/// move holds a bounded amount of mail in memory and an interruption leaves
/// every finished round complete.
const ARCHIVE_BATCH: usize = 25;

/// Methods that act on one account's mail. Addressed to the local account,
/// they are served here; everything else about it (preferences, order) is an
/// ordinary account row in the store.
const LOCAL_MAIL_METHODS: &[&str] = &[
    "folders.list",
    "folders.create",
    "folders.delete",
    "folders.rename",
    "folders.setNotify",
    "folders.archive",
    "messages.emptyFolder",
    "messages.recent",
    "messages.sync",
    "messages.read",
    "messages.thread",
    "messages.threadHeaders",
    "messages.markRead",
    "messages.markStarred",
    "messages.setTags",
    "tags.list",
    "messages.markAllRead",
    "messages.delete",
    "messages.move",
    "messages.copy",
    "messages.exportUids",
    "messages.saveRaw",
    "messages.saveRawBatch",
    "watch.start",
    "watch.stop",
];

struct ArchiveHost;

impl EngineHost for ArchiveHost {
    fn open_db(&self) -> anyhow::Result<rusqlite::Connection> {
        store::open_archive()
    }

    fn apply_secret(&self, _: &rusqlite::Connection, _: &str, _: &mut imap::Creds) {}

    fn store_secret(
        &self,
        _: &rusqlite::Connection,
        _: &str,
        _: &secrets::Secrets,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

/// The archive engine, opened on first use so a profile without local folders
/// never creates `archive.db`.
static ARCHIVE: Mutex<Option<Arc<Engine>>> = Mutex::new(None);

fn archive(main: &Arc<Engine>) -> anyhow::Result<Arc<Engine>> {
    let mut slot = ARCHIVE.lock().unwrap();
    if let Some(archive) = slot.as_ref() {
        return Ok(archive.clone());
    }
    // Checked under the lock removal holds, so a request racing the removal of
    // local folders cannot bring back an empty archive behind it.
    if !store::has_local_account(&main.db.lock().unwrap())? {
        anyhow::bail!("Local folders are not set up");
    }
    let archive = Arc::new(Engine::for_archive(Box::new(ArchiveHost))?);
    {
        let db = archive.db.lock().unwrap();
        match store::repair_local_index(&db, &parse::media_root()) {
            Ok(0) => {}
            Ok(repaired) => eprintln!("meron-core: reindexed {repaired} archived messages"),
            Err(err) => eprintln!("meron-core: archive index repair failed: {err:#}"),
        }
        store::sync_local_identity(&main.db.lock().unwrap(), &db)?;
    }
    *slot = Some(archive.clone());
    Ok(archive)
}

/// Whether this request is for local folders.
pub(crate) fn handles(req: &Request) -> bool {
    let p = &req.params;
    let account = p.get("account").and_then(Value::as_str);
    match req.method.as_str() {
        "account.addLocal"
        | "account.localStats"
        | "account.localBackup"
        | "account.localRestore" => true,
        "account.remove" => {
            account.or_else(|| p.get("id").and_then(Value::as_str)) == Some(LOCAL_ACCOUNT_ID)
        }
        "messages.move" | "messages.copy"
            if p.get("target_account").and_then(Value::as_str) == Some(LOCAL_ACCOUNT_ID) =>
        {
            true
        }
        method => account == Some(LOCAL_ACCOUNT_ID) && LOCAL_MAIL_METHODS.contains(&method),
    }
}

pub(crate) async fn dispatch(
    main: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        // Create the local account (once) and its archive.
        "account.addLocal" => {
            let name = req_str(p, "name")
                .ok()
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "Local Folders".to_string());
            let created = store::add_local_account(&main.db.lock().unwrap(), &name)?;
            let archive = archive(main)?;
            // Start with one folder, so there is somewhere to move mail and a
            // folder to open. The UI names it in the user's language.
            if created {
                let folder = req_str(p, "folder")
                    .ok()
                    .filter(|folder| !folder.trim().is_empty())
                    .unwrap_or_else(|| "Archive".to_string());
                let db = archive.db.lock().unwrap();
                if store::get_folders(&db, LOCAL_ACCOUNT_ID)?.is_empty() {
                    store::create_local_folder(&db, &folder)?;
                }
            }
            Ok(json!({ "ok": true, "created": created, "id": LOCAL_ACCOUNT_ID }))
        }

        "account.localStats" => {
            let archive = archive(main)?;
            let mut stats = store::local_storage_stats(&archive.db.lock().unwrap())?;
            stats["path"] = json!(archive_file().display().to_string());
            Ok(stats)
        }

        "account.localBackup" => {
            let path = req_str(p, "path")?;
            let passphrase = req_str(p, "passphrase")?;
            let archive = archive(main)?;
            let db = archive.db.lock().unwrap();
            store::backup_archive_to(&db, std::path::Path::new(&path), &passphrase)?;
            Ok(json!({ "ok": true, "path": path }))
        }

        // Merge a backup into the local folders, creating them if needed.
        "account.localRestore" => {
            let path = req_str(p, "path")?;
            let passphrase = req_str(p, "passphrase")?;
            store::add_local_account(&main.db.lock().unwrap(), "Local Folders")?;
            let archive = archive(main)?;
            let restored = {
                let archive = archive.clone();
                tokio::task::spawn_blocking(move || {
                    store::restore_local_backup(
                        &archive.db.lock().unwrap(),
                        std::path::Path::new(&path),
                        &passphrase,
                        &parse::media_root(),
                    )
                })
                .await??
            };
            Ok(json!({ "ok": true, "restored": restored }))
        }

        // Deletes the archive and every message in it. The UI asks first, and
        // says it cannot be undone. The account goes only once the file is
        // gone: otherwise adding local folders again would bring the mail back.
        "account.remove" => {
            // Held to the end, so nothing reopens the archive while it goes.
            let mut slot = ARCHIVE.lock().unwrap();
            // A request still holding the engine (a long move) keeps the
            // database open and could write to it after it is deleted.
            if slot
                .as_ref()
                .is_some_and(|archive| Arc::strong_count(archive) > 1)
            {
                anyhow::bail!("Local folders are busy. Try again once the current move finishes.");
            }
            drop(slot.take());
            let path = archive_file();
            for suffix in ["", "-wal", "-shm"] {
                let mut file = path.clone().into_os_string();
                file.push(suffix);
                match std::fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        return Err(anyhow::anyhow!(
                            "Could not delete {}: {err}",
                            std::path::Path::new(&file).display()
                        ));
                    }
                }
            }
            store::delete_account(&main.db.lock().unwrap(), LOCAL_ACCOUNT_ID)?;
            drop(slot);
            parse::remove_account_media(&parse::media_root(), LOCAL_ACCOUNT_ID);
            Ok(json!({ "ok": true }))
        }

        "messages.move" | "messages.copy"
            if req_str(p, "account").ok().as_deref() != Some(LOCAL_ACCOUNT_ID) =>
        {
            archive_from_server(main, req).await
        }

        _ => {
            let archive = archive(main)?;
            local_mail(main, &archive, req, out).await
        }
    }
}

fn archive_file() -> std::path::PathBuf {
    std::env::var("MERON_ARCHIVE_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| store::app_dir().join("archive.db"))
}

/// The UIDs a mail request names: explicit `uids`, one `uid`, or every message
/// of `thread_key` in `folder`.
fn request_uids(conn: &rusqlite::Connection, p: &Value, folder: &str) -> anyhow::Result<Vec<u32>> {
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
    store::resolve_message_uids(
        conn,
        p.get("account")
            .and_then(Value::as_str)
            .unwrap_or(LOCAL_ACCOUNT_ID),
        folder,
        &thread_key,
        subject_filter.as_deref(),
        uid,
        &explicit_uids,
    )
}

/// The thread id a mutation reports as changed, built as the server path does.
fn changed_thread_id(p: &Value, account: &str, folder: &str) -> String {
    let (thread_key, subject_filter) =
        store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
    if thread_key.is_empty() {
        return p
            .get("uid")
            .and_then(Value::as_u64)
            .map(|uid| format!("{account}#{folder}#{uid}"))
            .unwrap_or_default();
    }
    let key = subject_filter
        .as_deref()
        .map(|subject| store::branch_compound_key(&thread_key, subject))
        .unwrap_or(thread_key);
    mail_model::format_thread_id(account, folder, &key)
}

fn request_folder(p: &Value) -> anyhow::Result<String> {
    let folder = req_str(p, "folder").or_else(|_| req_str(p, "name"))?;
    store::normalize_local_folder_name(&folder)
}

async fn local_mail(
    main: &Arc<Engine>,
    archive: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    let account = LOCAL_ACCOUNT_ID;
    let media_root = parse::media_root();
    match req.method.as_str() {
        "folders.list" => {
            let folders = store::get_folders(&archive.db.lock().unwrap(), account)?;
            Ok(json!({ "folders": serde_json::to_value(folders)? }))
        }

        "folders.create" => {
            let folder =
                store::create_local_folder(&archive.db.lock().unwrap(), &req_str(p, "name")?)?;
            Ok(json!({ "folders": serde_json::to_value(vec![folder])? }))
        }

        "folders.delete" => {
            let folder = request_folder(p)?;
            let db = archive.db.lock().unwrap();
            let (removed, deleted) = store::delete_local_folder(&db, &folder)?;
            for target in &removed {
                parse::remove_folder_media(&media_root, account, target);
            }
            Ok(json!({
                "ok": true,
                "folder": folder,
                "removed": removed,
                "deleted": deleted,
                "folders": serde_json::to_value(store::get_folders(&db, account)?)?,
            }))
        }

        "folders.rename" => {
            let folder = request_folder(p)?;
            let db = archive.db.lock().unwrap();
            let renamed =
                store::rename_local_folder(&db, &folder, &req_str(p, "name")?, &media_root)?;
            Ok(json!({
                "ok": true,
                "folder": folder,
                "renamed": renamed,
                "folders": serde_json::to_value(store::get_folders(&db, account)?)?,
            }))
        }

        "folders.setNotify" => Err(anyhow::anyhow!(
            "Local folders get no new mail to notify about"
        )),
        "folders.archive" => Err(anyhow::anyhow!("Local folders have no Archive folder")),
        "messages.emptyFolder" => Err(anyhow::anyhow!(
            "Only Trash and Junk folders can be emptied"
        )),
        // .eml export: the kept originals, byte for byte.
        "messages.exportUids" => {
            let folder = canon_local(p);
            let db = archive.db.lock().unwrap();
            let mut uids = if p.get("all").and_then(Value::as_bool).unwrap_or(false) {
                store::local_folder_uids(&db, &folder)?
            } else {
                request_uids(&db, p, &folder)?
            };
            uids.sort_unstable();
            uids.dedup();
            Ok(json!({ "uids": uids }))
        }

        "messages.saveRaw" => {
            let folder = canon_local(p);
            let uid = req_u32(p, "uid")?;
            let path = req_str(p, "path")?;
            let raw = store::raw_message(&archive.db.lock().unwrap(), &folder, uid)?
                .ok_or_else(|| anyhow::anyhow!("message {uid} not found in {folder}"))?;
            write_private(std::path::Path::new(&path), &raw)?;
            Ok(json!({ "saved": true, "size": raw.len() }))
        }

        "messages.saveRawBatch" => {
            let folder = canon_local(p);
            let dir = std::path::PathBuf::from(req_str(p, "dir")?);
            let uids = p
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
            let db = archive.db.lock().unwrap();
            let mut saved = Vec::with_capacity(uids.len());
            for uid in uids {
                let Some(raw) = store::raw_message(&db, &folder, uid)? else {
                    continue;
                };
                write_private(&dir.join(format!("{uid}.eml")), &raw)?;
                saved.push(uid);
            }
            Ok(json!({ "saved": saved }))
        }

        // Nothing to sync or watch: a local folder changes only through Meron.
        "messages.sync" | "watch.start" | "watch.stop" => Ok(json!({ "ok": true })),

        "messages.recent" => {
            let request = thread_list::ThreadListQuery::from_params(p, "folder");
            let folder = request.folder.clone();
            let db = archive.db.lock().unwrap();
            let (messages, next_cursor) = match request.source() {
                thread_list::MailSource::Starred => (
                    store::get_starred(&db, account, &folder, request.limit)?,
                    None,
                ),
                thread_list::MailSource::Recent { filter } => store::get_recent_page(
                    &db,
                    account,
                    &folder,
                    request.limit,
                    request.before_cursor,
                    filter,
                )?,
                thread_list::MailSource::Search => {
                    let folders = engine::query_folders(&db, account, &folder, &request.query);
                    let messages = store::search_messages_in_folders(
                        &db,
                        account,
                        &folders,
                        &request.query,
                        request.limit,
                        request.search_before_cursor.as_ref(),
                    )?;
                    let next_cursor = store::search_next_cursor(&messages, request.limit, 0);
                    (messages, next_cursor)
                }
            };
            let mut page = thread_list::mail_page(
                &db,
                account,
                &folder,
                messages,
                next_cursor,
                p.get("group").and_then(Value::as_bool).unwrap_or(false),
            )?;
            page["folder_synced"] = Value::Bool(true);
            request.retain_attachment_threads(&mut page);
            Ok(page)
        }

        "messages.read" | "messages.threadHeaders" => {
            if req.method == "messages.read" {
                let folder = canon_local(p);
                let uid = req_u32(p, "uid")?;
                store::restore_local_media(
                    &archive.db.lock().unwrap(),
                    &[(folder, uid)],
                    &media_root,
                )?;
            }
            refresh_identity(main, archive);
            Box::pin(crate::sidecar::dispatch::messages::dispatch(
                archive, req, out,
            ))
            .await
        }

        "messages.thread" => {
            let folder = canon_local(p);
            let (thread_key, subject_filter) = store::split_thread_key(&req_str(p, "thread_key")?);
            let limit = p.get("limit").and_then(Value::as_u64).map(|n| n as u32);
            let thread_id = req_str(p, "thread_id")
                .unwrap_or_else(|_| mail_model::format_thread_id(account, &folder, &thread_key));
            // Only the page being read: an old conversation may be long.
            {
                let db = archive.db.lock().unwrap();
                let (headers, _) = thread_read::thread_page_headers(
                    &db,
                    account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    limit,
                    p.get("before_cursor").and_then(Value::as_str),
                )?;
                let messages = headers
                    .into_iter()
                    .map(|header| {
                        let folder = if header.folder.is_empty() {
                            folder.clone()
                        } else {
                            header.folder
                        };
                        (folder, header.uid)
                    })
                    .collect::<Vec<_>>();
                store::restore_local_media(&db, &messages, &media_root)?;
            }
            refresh_identity(main, archive);
            thread_read::read_thread_page(
                archive,
                thread_read::ThreadReadArgs {
                    account,
                    folder: &folder,
                    thread_id: &thread_id,
                    thread_key: &thread_key,
                    subject_filter: subject_filter.as_deref(),
                    limit,
                    for_print: p.get("for_print").and_then(Value::as_bool).unwrap_or(false),
                    before_cursor: p.get("before_cursor").and_then(Value::as_str),
                    media_root,
                },
                None,
            )
            .await
        }

        "messages.markRead" | "messages.markStarred" => {
            let folder = canon_local(p);
            let read = req.method == "messages.markRead";
            let value = p
                .get(if read { "seen" } else { "starred" })
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let db = archive.db.lock().unwrap();
            let explicit = p.get("uids").is_some() || p.get("uid").is_some();
            let (thread_key, subject_filter) =
                store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
            // A whole thread is flagged in every local folder it spans.
            let by_folder = if !explicit && !thread_key.is_empty() {
                store::thread_uids_by_folder(
                    &db,
                    account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    false,
                )?
            } else {
                std::collections::BTreeMap::from([(folder.clone(), request_uids(&db, p, &folder)?)])
            };
            for (marked_folder, uids) in &by_folder {
                let (seen, starred) = if read {
                    (Some(value), None)
                } else {
                    (None, Some(value))
                };
                store::set_local_flags(&db, marked_folder, uids, seen, starred)?;
            }
            let mut result = mail_model::mutation_result(
                json!({ "ok": true }),
                &db,
                account,
                &changed_thread_id(p, account, &folder),
                &folder,
                None,
                read.then_some(!value),
                (!read).then_some(value),
                false,
            )?;
            mail_model::add_folder_unreads(&mut result, &db, account, by_folder.keys())?;
            Ok(result)
        }

        "tags.list" => {
            let counts = store::tag_counts(&archive.db.lock().unwrap(), Some(account))?;
            Ok(store::tag_counts_json(counts))
        }

        "messages.setTags" => {
            let folder = canon_local(p);
            let add = meron_core::tags::parse_tag_list(p.get("add")).map_err(anyhow::Error::msg)?;
            let remove =
                meron_core::tags::parse_tag_list(p.get("remove")).map_err(anyhow::Error::msg)?;
            let db = archive.db.lock().unwrap();
            let targets = store::tag_targets(
                &db,
                account,
                &folder,
                &req_str(p, "thread_key").unwrap_or_default(),
                &tag_uids(p),
            )?;
            let tags = store::retag(&db, account, &targets, &add, &remove, false)?;
            Ok(json!({ "ok": true, "tags": tags }))
        }

        "messages.markAllRead" => {
            let folder = canon_local(p);
            let db = archive.db.lock().unwrap();
            let updated = store::set_local_folder_seen(&db, &folder, true)?;
            mail_model::mutation_result(
                json!({ "ok": true, "updated": updated, "folder": folder }),
                &db,
                account,
                "",
                &folder,
                None,
                Some(false),
                None,
                false,
            )
        }

        // Local folders have no Trash: a delete is permanent, and the UI says so.
        "messages.delete" => {
            let folder = canon_local(p);
            let db = archive.db.lock().unwrap();
            let uids = request_uids(&db, p, &folder)?;
            let deleted = store::delete_local_messages(&db, &folder, &uids)?;
            for uid in &uids {
                parse::remove_message_media(&media_root, account, &folder, *uid);
            }
            mail_model::mutation_result(
                json!({ "ok": true, "deleted": deleted, "permanent": true }),
                &db,
                account,
                &changed_thread_id(p, account, &folder),
                &folder,
                None,
                None,
                None,
                true,
            )
        }

        "messages.move" | "messages.copy" => {
            let moving = req.method == "messages.move";
            let folder = canon_local(p);
            let target_account =
                req_str(p, "target_account").unwrap_or_else(|_| account.to_string());
            let target_folder = req_str(p, "target_folder")?;
            let thread_id = changed_thread_id(p, account, &folder);
            if target_account == account {
                let target_folder = store::normalize_local_folder_name(&target_folder)?;
                let db = archive.db.lock().unwrap();
                let uids = request_uids(&db, p, &folder)?;
                if target_folder == folder {
                    return Ok(json!({ "ok": true, "moved": 0 }));
                }
                let target_uids = if moving {
                    store::move_local_messages(&db, &folder, &target_folder, &uids, &media_root)?
                } else {
                    store::copy_local_messages(&db, &folder, &target_folder, &uids, &media_root)?
                };
                if moving {
                    for uid in &uids {
                        parse::remove_message_media(&media_root, account, &folder, *uid);
                    }
                }
                let count = target_uids.len();
                return mail_model::mutation_result(
                    json!({
                        "ok": true,
                        "moved": if moving { count } else { 0 },
                        "copied": if moving { 0 } else { count },
                        "source_folder": folder,
                        "target_account": account,
                        "target_folder": target_folder,
                        "target_uids": target_uids,
                    }),
                    &db,
                    account,
                    &thread_id,
                    &folder,
                    Some(&target_folder),
                    None,
                    None,
                    moving,
                );
            }

            // Back to a server: upload the originals, then forget the local
            // copies only once every APPEND was acknowledged.
            let target_folder = engine::canon_folder(&target_folder);
            let (uids, messages) = {
                let db = archive.db.lock().unwrap();
                let uids = request_uids(&db, p, &folder)?;
                let messages = store::raw_messages_for_copy(&db, &folder, &uids)?;
                (uids, messages)
            };
            engine::require_complete_fetch(&uids, &messages)?;
            if messages.is_empty() {
                return Ok(json!({ "ok": true, "moved": 0, "copied": 0 }));
            }
            main.with_write_session(&target_account, |session| {
                let target_folder = target_folder.clone();
                let messages = messages.clone();
                Box::pin(async move {
                    imap::append_copied_messages(session, &target_folder, &messages).await
                })
            })
            .await?;
            let copies = std::collections::BTreeMap::from([(folder.clone(), uids.clone())]);
            let tags = store::carried_tags(&archive.db.lock().unwrap(), account, &copies)?;
            store::apply_carried_tags(&main.db.lock().unwrap(), &target_account, tags, true)?;
            if moving {
                let db = archive.db.lock().unwrap();
                store::delete_local_messages(&db, &folder, &uids)?;
                for uid in &uids {
                    parse::remove_message_media(&media_root, account, &folder, *uid);
                }
            }
            // Best effort: the uploads are done either way.
            if let Some(refresh) = engine::refresh_written_folder(
                main,
                &target_account,
                &target_folder,
                50.max(messages.len() as u32),
            )
            .await
            {
                let batch = &refresh.batch;
                let db = main.db.lock().unwrap();
                let _ = store::ensure_folder(&db, &target_account, &target_folder)
                    .and_then(|_| {
                        store::upsert_messages(
                            &db,
                            &target_account,
                            &target_folder,
                            &batch.messages,
                        )
                    })
                    .and_then(|_| {
                        store::set_folder_state(
                            &db,
                            &target_account,
                            &target_folder,
                            batch.uidvalidity,
                            batch.uid_next,
                        )
                    });
            }
            let count = messages.len();
            mail_model::mutation_result(
                json!({
                    "ok": true,
                    "moved": if moving { count } else { 0 },
                    "copied": if moving { 0 } else { count },
                    "source_folder": folder,
                    "target_account": target_account,
                    "target_folder": target_folder,
                }),
                &archive.db.lock().unwrap(),
                account,
                &thread_id,
                &folder,
                None,
                None,
                None,
                moving,
            )
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}

/// Write an exported message readable by the user only, as the server export does.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut output = options
        .open(path)
        .map_err(|err| anyhow::anyhow!("open message export {}: {err}", path.display()))?;
    output
        .write_all(bytes)
        .map_err(|err| anyhow::anyhow!("write message export {}: {err}", path.display()))
}

/// The folder a local request names, in the form local folders are stored
/// under (see `store::normalize_local_folder_name`).
fn canon_local(p: &Value) -> String {
    store::canon_local_folder(&req_str(p, "folder").unwrap_or_default())
}

/// Tag counts in local folders, for the tag list.
pub(crate) fn tag_counts(main: &Arc<Engine>) -> std::collections::BTreeMap<String, u32> {
    let counts = archive(main)
        .and_then(|archive| store::tag_counts(&archive.db.lock().unwrap(), Some(LOCAL_ACCOUNT_ID)));
    counts.unwrap_or_default()
}

/// Rename or delete a tag in local folders. See [`store::replace_tag`]. Nothing
/// to do without local folders, and the archive is not created for it.
pub(crate) fn replace_tag(main: &Arc<Engine>, tag: &str, to: Option<&str>) -> anyhow::Result<()> {
    if !store::has_local_account(&main.db.lock().unwrap())? {
        return Ok(());
    }
    let archive = archive(main)?;
    store::replace_tag(&archive.db.lock().unwrap(), tag, to, false)?;
    Ok(())
}

/// Starred threads kept in local folders, for the cross-account Starred view.
/// Empty when there are no local folders; the archive is not created for it.
pub(crate) fn starred_cards(main: &Arc<Engine>, max_threads: u32) -> Vec<Value> {
    let has_local = store::has_local_account(&main.db.lock().unwrap()).unwrap_or(false);
    if !has_local {
        return Vec::new();
    }
    let cards = archive(main).and_then(|archive| {
        mail_model::starred_thread_cards(&archive.db.lock().unwrap(), max_threads)
    });
    cards.unwrap_or_else(|err| {
        eprintln!("meron-core: local starred threads: {err:#}");
        Vec::new()
    })
}

/// Keep the archive's view of the user's addresses and the local account's
/// preferences current before a thread read renders with them.
fn refresh_identity(main: &Arc<Engine>, archive: &Arc<Engine>) {
    let result = store::sync_local_identity(&main.db.lock().unwrap(), &archive.db.lock().unwrap());
    if let Err(err) = result {
        eprintln!("meron-core: local identity sync failed: {err:#}");
    }
}

/// Move (or copy) server mail into a local folder: download each original, keep
/// it in the archive and read it back, and only then move the server copy to
/// Trash. Runs in rounds of [`ARCHIVE_BATCH`], each complete before the next.
async fn archive_from_server(main: &Arc<Engine>, req: &Request) -> anyhow::Result<Value> {
    let p = &req.params;
    let moving = req.method == "messages.move";
    let account = req_str(p, "account")?;
    let folder =
        engine::canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
    let target_folder = store::normalize_local_folder_name(&req_str(p, "target_folder")?)?;
    let thread_id = changed_thread_id(p, &account, &folder);
    let archive = archive(main)?;
    if !store::folder_exists(
        &archive.db.lock().unwrap(),
        LOCAL_ACCOUNT_ID,
        &target_folder,
    )? {
        return Err(anyhow::anyhow!("Folder not found: {target_folder}"));
    }
    let uids = request_uids(&main.db.lock().unwrap(), p, &folder)?;
    let source = store::archive_source_label(&main.db.lock().unwrap(), &account, &folder);
    let media_root = parse::media_root();

    let mut target_uids = Vec::with_capacity(uids.len());
    let mut trash_folder = None;
    for chunk in uids.chunks(ARCHIVE_BATCH) {
        let messages = main
            .with_read_session(&account, |session| {
                let folder = folder.clone();
                let uids = chunk.to_vec();
                Box::pin(
                    async move { imap::fetch_raw_messages_for_copy(session, &folder, &uids).await },
                )
            })
            .await?;
        engine::require_complete_fetch(chunk, &messages)?;
        let inputs = {
            let db = main.db.lock().unwrap();
            messages
                .iter()
                .map(|message| {
                    let (thread_key, date) =
                        store::cached_thread_key_and_date(&db, &account, &folder, message.uid)?;
                    Ok((message, thread_key, date))
                })
                .collect::<anyhow::Result<Vec<_>>>()?
        };
        let stored = {
            let archive = archive.clone();
            let target_folder = target_folder.clone();
            let source = source.clone();
            let media_root = media_root.clone();
            let inputs = inputs
                .into_iter()
                .map(|(message, thread_key, date)| {
                    (
                        message.raw.clone(),
                        message.seen,
                        message.starred,
                        thread_key,
                        date,
                    )
                })
                .collect::<Vec<_>>();
            tokio::task::spawn_blocking(move || {
                let inputs = inputs
                    .iter()
                    .map(
                        |(raw, seen, starred, thread_key, date)| store::ArchiveInput {
                            raw,
                            seen: *seen,
                            starred: *starred,
                            thread_key: thread_key.clone(),
                            date_fallback: *date,
                            source: source.clone(),
                        },
                    )
                    .collect::<Vec<_>>();
                store::archive_messages(
                    &archive.db.lock().unwrap(),
                    &target_folder,
                    &inputs,
                    &media_root,
                )
            })
            .await??
        };
        target_uids.extend(stored);
        let copies = std::collections::BTreeMap::from([(folder.clone(), chunk.to_vec())]);
        let tags = store::carried_tags(&main.db.lock().unwrap(), &account, &copies)?;
        store::apply_carried_tags(&archive.db.lock().unwrap(), LOCAL_ACCOUNT_ID, tags, false)?;
        if moving {
            let removed = engine::delete_to_trash(main, &account, &folder, chunk)
                .await
                .map_err(|err| {
                    err.context("Kept the messages in the local folder but could not remove them from the server")
                })?;
            store::delete_messages_by_uid(&main.db.lock().unwrap(), &account, &folder, chunk)?;
            if let Some(trash) = removed {
                trash_folder = Some(trash.folder);
            }
        }
    }

    // The Trash copies are on the server now; show them without a full sync.
    if let Some(trash) = &trash_folder
        && let Some(refresh) =
            engine::refresh_written_folder(main, &account, trash, 50.max(uids.len() as u32)).await
    {
        let batch = &refresh.batch;
        let db = main.db.lock().unwrap();
        let _ = store::ensure_folder(&db, &account, trash)
            .and_then(|_| store::upsert_messages(&db, &account, trash, &batch.messages))
            .and_then(|_| {
                store::set_folder_state(&db, &account, trash, batch.uidvalidity, batch.uid_next)
            });
    }

    let count = target_uids.len();
    let mut result = mail_model::mutation_result(
        json!({
            "ok": true,
            "moved": if moving { count } else { 0 },
            "copied": if moving { 0 } else { count },
            "source_folder": folder,
            "target_account": LOCAL_ACCOUNT_ID,
            "target_folder": target_folder,
        }),
        &main.db.lock().unwrap(),
        &account,
        &thread_id,
        &folder,
        trash_folder.as_deref(),
        None,
        None,
        moving,
    )?;
    let local_unread = store::get_folder_unread(
        &archive.db.lock().unwrap(),
        LOCAL_ACCOUNT_ID,
        &target_folder,
    )?;
    result["folder_unreads"][LOCAL_ACCOUNT_ID] = json!({ target_folder.clone(): local_unread });
    if let Some(counts) = result["folder_counts"].as_array_mut() {
        counts.push(json!({
            "account_id": LOCAL_ACCOUNT_ID,
            "folder_id": target_folder,
            "unread": local_unread,
        }));
    }
    Ok(result)
}
