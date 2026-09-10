//! Constrained mail mutations used by the desktop MCP adapter. Approval lives
//! in the desktop host; this layer preserves the exact approved IMAP UID set.
use crate::{engine::Engine, imap, mail_model, store};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

const MAX_MESSAGES: usize = 1000;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub account: String,
    pub folder: String,
    #[serde(default)]
    pub uids: Vec<u32>,
    #[serde(default)]
    pub empty_trash: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct DeleteSnapshot {
    pub account: String,
    pub folder: String,
    pub uidvalidity: u32,
    pub uids: Vec<u32>,
    pub messages: Vec<Value>,
}

fn validate_uids(uids: &[u32]) -> Result<()> {
    ensure!(
        !uids.is_empty() && uids.len() <= MAX_MESSAGES,
        "Select 1–1000 messages"
    );
    let unique: std::collections::HashSet<_> = uids.iter().collect();
    ensure!(
        !uids.contains(&0) && unique.len() == uids.len(),
        "Invalid or duplicate message UID"
    );
    Ok(())
}

pub async fn prepare_delete(engine: &Arc<Engine>, value: Value) -> Result<Value> {
    let selection: Selection = serde_json::from_value(value)?;
    ensure!(
        !selection.account.is_empty() && !selection.folder.is_empty(),
        "Account and folder are required"
    );
    if !selection.empty_trash {
        validate_uids(&selection.uids)?;
    }
    let account = selection.account.clone();
    let snapshot = engine.with_read_session(&account, |session| {
        let selection = selection.clone();
        Box::pin(async move {
            let Selection { account, folder, mut uids, empty_trash } = selection;
            if empty_trash {
                ensure!(imap::find_trash_folder(session).await?.as_deref() == Some(folder.as_str()), "Only Trash can be emptied");
                uids = imap::list_all_uids(session, &folder).await?.into_iter().collect();
                uids.sort_unstable();
                if !uids.is_empty() { validate_uids(&uids)?; }
            }
            ensure!(session.capabilities().await?.has_str("UIDPLUS"), "Permanent deletion through MCP requires IMAP UIDPLUS");
            let validity = session.select(&folder).await?.uid_validity.context("Missing UIDVALIDITY")?;
            ensure!(validity != 0, "Invalid UIDVALIDITY");
            let headers = imap::fetch_headers_by_uid(session, &folder, &uids).await?;
            let after = session.select(&folder).await?.uid_validity;
            ensure!(after == Some(validity), "Mailbox changed; request a new preview");
            ensure!(headers.len() == uids.len(), "Some messages no longer exist; request a new preview");
            let messages = headers.into_iter().map(|m| json!({"uid":m.uid,"subject":m.subject,"from":m.from_addr,"date":m.date})).collect();
            Ok(DeleteSnapshot {account,folder,uidvalidity:validity,uids,messages})
        })
    }).await?;
    Ok(serde_json::to_value(snapshot)?)
}

pub async fn delete(engine: &Arc<Engine>, value: Value) -> Result<Value> {
    let snapshot: DeleteSnapshot = serde_json::from_value(value)?;
    if snapshot.uids.is_empty() {
        return Ok(json!({"ok":true,"deleted":0}));
    }
    validate_uids(&snapshot.uids)?;
    ensure!(snapshot.uidvalidity != 0, "Missing UIDVALIDITY");
    let account = snapshot.account.clone();
    engine
        .with_write_session(&account, |session| {
            let snapshot = snapshot.clone();
            Box::pin(async move {
                imap::expunge_uids_checked(
                    session,
                    &snapshot.folder,
                    &snapshot.uids,
                    snapshot.uidvalidity,
                )
                .await
            })
        })
        .await?;
    let db = engine.db.lock().unwrap();
    store::delete_messages_by_uid(&db, &account, &snapshot.folder, &snapshot.uids)?;
    mail_model::mutation_result(
        json!({"ok":true,"deleted":snapshot.uids.len(),"permanent":true}),
        &db,
        &account,
        "",
        &snapshot.folder,
        None,
        None,
        None,
        true,
    )
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Organize {
    account: String,
    folder: String,
    uids: Vec<u32>,
    action: String,
    #[serde(default)]
    target_folder: String,
}

pub async fn organize(engine: &Arc<Engine>, value: Value) -> Result<Value> {
    let input: Organize = serde_json::from_value(value)?;
    validate_uids(&input.uids)?;
    ensure!(
        !input.account.is_empty() && !input.folder.is_empty(),
        "Account and folder are required"
    );
    ensure!(
        matches!(
            input.action.as_str(),
            "mark_read" | "mark_unread" | "star" | "unstar" | "archive" | "trash" | "move"
        ),
        "Unknown organization action"
    );
    let account = input.account.clone();
    let target = engine
        .with_write_session(&account, |session| {
            let input = input.clone();
            Box::pin(async move {
                match input.action.as_str() {
                    "mark_read" | "mark_unread" => {
                        imap::prepare_flag_update(session, &input.folder).await?;
                        imap::store_seen(session, &input.uids, input.action == "mark_read").await?;
                        Ok(None)
                    }
                    "star" | "unstar" => {
                        imap::prepare_flag_update(session, &input.folder).await?;
                        imap::store_starred(session, &input.uids, input.action == "star").await?;
                        Ok(None)
                    }
                    _ => {
                        let target = match input.action.as_str() {
                            "archive" => imap::find_archive_folder(session)
                                .await?
                                .context("Archive folder not found")?,
                            "trash" => imap::find_trash_folder(session)
                                .await?
                                .context("Trash folder not found")?,
                            "move" if !input.target_folder.is_empty() => input.target_folder,
                            _ => bail!("Target folder is required"),
                        };
                        ensure!(
                            target != input.folder,
                            "Messages are already in the target folder"
                        );
                        imap::move_to_folder_checked(session, &input.folder, &target, &input.uids)
                            .await?;
                        Ok(Some(target))
                    }
                }
            })
        })
        .await?;
    if target.is_none() {
        let db = engine.db.lock().unwrap();
        let seen = match input.action.as_str() {
            "mark_read" => Some(true),
            "mark_unread" => Some(false),
            _ => None,
        };
        let starred = match input.action.as_str() {
            "star" => Some(true),
            "unstar" => Some(false),
            _ => None,
        };
        for uid in &input.uids {
            if let Some(seen) = seen {
                store::update_message_seen(&db, &account, &input.folder, *uid, seen)?;
            }
            if let Some(starred) = starred {
                store::update_message_starred(&db, &account, &input.folder, *uid, starred)?;
            }
        }
        return mail_model::mutation_result(
            json!({"ok":true,"changed":input.uids.len()}),
            &db,
            &account,
            "",
            &input.folder,
            None,
            seen,
            starred,
            true,
        );
    }
    // Refresh the move destination on a read session. The write is never retried.
    let refresh_folder = target.as_deref().unwrap_or(&input.folder);
    let refresh = crate::engine::fetch_recent_resilient(
        engine,
        &account,
        refresh_folder,
        50.max(input.uids.len() as u32),
    )
    .await;
    let db = engine.db.lock().unwrap();
    if let Some(ref target) = target {
        store::delete_messages_by_uid(&db, &account, &input.folder, &input.uids)?;
        store::ensure_folder(&db, &account, target)?;
    }
    let warning = match refresh {
        Ok(batch) => {
            store::upsert_messages(&db, &account, refresh_folder, &batch.messages)?;
            store::set_folder_state(
                &db,
                &account,
                refresh_folder,
                batch.uidvalidity,
                batch.uid_next,
            )?;
            None
        }
        Err(_) => {
            Some("The operation completed but the mailbox refresh failed. Do not retry the write.")
        }
    };
    mail_model::mutation_result(
        json!({"ok":true,"changed":input.uids.len(),"target_folder":target,"warning":warning}),
        &db,
        &account,
        "",
        &input.folder,
        target.as_deref(),
        None,
        None,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_selections_are_bounded_and_unique() {
        assert!(validate_uids(&[]).is_err());
        assert!(validate_uids(&[0]).is_err());
        assert!(validate_uids(&[1, 1]).is_err());
        assert!(validate_uids(&(1..=1001).collect::<Vec<_>>()).is_err());
        assert!(validate_uids(&[1, 4, 9]).is_ok());
    }
}
