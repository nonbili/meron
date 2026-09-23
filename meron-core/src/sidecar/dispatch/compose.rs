use anyhow::Context as _;
use serde_json::{Value, json};
use std::io::Write as _;
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::protocol::Request;
use meron_core::{imap, smtp, store};

use crate::sidecar::params::*;
use crate::{Writer, emit};

/// Handle outgoing mail: send, drafts, and saving raw messages.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        "send" => {
            let account = req_str(p, "account")?;
            let to = req_str(p, "to")?;
            let cc = req_str(p, "cc").unwrap_or_default();
            let bcc = req_str(p, "bcc").unwrap_or_default();
            let subject = req_str(p, "subject").unwrap_or_default();
            let body = req_str(p, "body").unwrap_or_default();
            let html = req_str(p, "html").unwrap_or_default();
            let in_reply_to = req_str(p, "in_reply_to").unwrap_or_default();
            let references = req_str(p, "references").unwrap_or_default();
            let reply_to = req_str(p, "reply_to").unwrap_or_default();
            // Client-generated Message-ID so the optimistic bubble and a quick
            // follow-up reply share the id the Sent copy will carry.
            let message_id = req_str(p, "message_id").unwrap_or_default();
            let attachments = opt_attachments(p)?;
            let requested_from = req_str(p, "from").unwrap_or_default();
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from)?;
            let raw = smtp::send(
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
            )
            .await?;
            // Finalize the Sent view. For Gmail/Outlook defaults this only
            // refreshes the provider-created copy; other accounts get Meron's
            // best-effort APPEND plus refresh. The mail already left via SMTP,
            // so none of this may hold up the reply: an APPEND re-uploads the
            // whole message, and a host that times out waiting on it would
            // report a delivered message as failed — and offer to send it again.
            let engine = engine.clone();
            let out = out.clone();
            tokio::spawn(async move {
                match append_to_sent(&engine, &account, &raw).await {
                    // The Sent copy is cached, so tell the UI to re-read the
                    // store — see `sent_copy_cached_detail`. When the provider
                    // has not exposed its own copy yet, wait for it rather than
                    // firing at an unchanged cache: no later sync of the Sent
                    // folder is guaranteed to follow.
                    Ok(copy) => {
                        if copy.observed
                            || (copy.worth_awaiting()
                                && await_sent_copy(&engine, &account, &copy).await)
                        {
                            emit(
                                &out,
                                "mail.sentCopyCached",
                                sent_copy_cached_detail(&account),
                            )
                            .await;
                        }
                    }
                    Err(err) => {
                        eprintln!("meron-core: APPEND to Sent failed for {account}: {err:#}");
                    }
                }
            });
            Ok(json!({ "ok": true }))
        }

        "save_draft" => {
            let account = req_str(p, "account")?;
            let to = req_str(p, "to").unwrap_or_default();
            let cc = req_str(p, "cc").unwrap_or_default();
            let bcc = req_str(p, "bcc").unwrap_or_default();
            let subject = req_str(p, "subject").unwrap_or_default();
            let body = req_str(p, "body").unwrap_or_default();
            let html = req_str(p, "html").unwrap_or_default();
            let in_reply_to = req_str(p, "in_reply_to").unwrap_or_default();
            let references = req_str(p, "references").unwrap_or_default();
            let reply_to = req_str(p, "reply_to").unwrap_or_default();
            let attachments = opt_attachments(p)?;
            let requested_from = req_str(p, "from").unwrap_or_default();
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from)?;
            // Stable per-draft Message-ID: each autosave reuses it so the IMAP
            // layer can find and prune the prior copy instead of piling up dups.
            let draft_id = req_str(p, "draft_id").unwrap_or_default();
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
            )?;
            append_to_drafts(engine, &account, &raw, &draft_id).await?;
            Ok(json!({ "ok": true }))
        }

        "discard_draft" => {
            let account = req_str(p, "account")?;
            let draft_id = req_str(p, "draft_id").unwrap_or_default();
            if draft_id.trim().is_empty() {
                return Ok(json!({ "ok": true, "deleted": 0 }));
            }
            // The LIST that finds the folder changes nothing and is where a dead
            // pooled session gives out, so it preflights the delete that follows.
            let drafts_slot: Arc<std::sync::Mutex<Option<String>>> =
                Arc::new(std::sync::Mutex::new(None));
            let (drafts, deleted) = engine
                .with_preflighted_write_session(
                    &account,
                    |session| {
                        let slot = Arc::clone(&drafts_slot);
                        Box::pin(async move {
                            let drafts = imap::find_drafts_folder(session)
                                .await?
                                .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                            *slot.lock().unwrap() = Some(drafts);
                            anyhow::Ok(())
                        })
                    },
                    |session| {
                        let draft_id = draft_id.clone();
                        let slot = Arc::clone(&drafts_slot);
                        Box::pin(async move {
                            let drafts = { slot.lock().unwrap().clone() }
                                .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                            let deleted = imap::discard_draft(session, &drafts, &draft_id).await?;
                            anyhow::Ok((drafts, deleted))
                        })
                    },
                )
                .await?;
            // Drop the locally cached copies too, or the discarded draft keeps
            // showing in the thread view until the next full Drafts sync.
            store::delete_draft_copies(
                &engine.db.lock().unwrap(),
                &account,
                &drafts,
                &draft_id,
                None,
            )?;
            if let Ok(thread_key) = req_str(p, "thread_key") {
                store::delete_quick_reply_drafts_in_thread(
                    &engine.db.lock().unwrap(),
                    &account,
                    &drafts,
                    &thread_key,
                )?;
            }
            Ok(json!({ "ok": true, "deleted": deleted, "permanent": true }))
        }

        // Fetch one message's original RFC822 bytes and write them directly to
        // the path selected by the desktop save dialog. Keeping the bytes out
        // of the JSON response avoids its bounded line size; BODY.PEEK[] keeps
        // an unread message unread.
        "messages.saveRaw" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let uid = req_u32(p, "uid")?;
            let path = req_str(p, "path")?;

            let raw_messages = engine
                .with_read_session(&account, |session| {
                    let folder = folder.clone();
                    Box::pin(async move {
                        imap::fetch_raw_messages_for_copy(session, &folder, &[uid]).await
                    })
                })
                .await?;
            let message = raw_messages
                .into_iter()
                .next()
                .with_context(|| format!("message {uid} not found in {folder}"))?;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            let mut output = options
                .open(&path)
                .with_context(|| format!("open message export {path}"))?;
            output
                .write_all(&message.raw)
                .with_context(|| format!("write message export {path}"))?;
            Ok(json!({ "saved": true, "size": message.raw.len() }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
