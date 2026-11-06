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

/// The optional `uids` array of a request, ignoring anything that isn't a UID.
fn opt_uids(params: &Value) -> Vec<u32> {
    params
        .get("uids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_u64)
                .map(|n| n as u32)
                .collect()
        })
        .unwrap_or_default()
}

/// Finalize the Sent view after a send. For Gmail/Outlook defaults this only
/// refreshes the provider-created copy; other accounts get Meron's best-effort
/// APPEND plus refresh. The mail already left via SMTP, so none of this may
/// hold up the reply: an APPEND re-uploads the whole message, and a host that
/// times out waiting on it would report a delivered message as failed — and
/// offer to send it again.
/// Whether a send or draft is encrypted (`openpgp::outgoing::protection`),
/// and whether only because it answers encrypted mail (nothing was asked).
fn reply_protection(
    engine: &Arc<Engine>,
    p: &Value,
    account: &str,
    in_reply_to: &str,
) -> anyhow::Result<(bool, bool)> {
    let encrypt = p.get("encrypt").and_then(Value::as_bool).unwrap_or(false);
    let plain = p.get("plain").and_then(Value::as_bool).unwrap_or(false);
    meron_core::openpgp::store::reply_protection(
        &engine.db.lock().unwrap(),
        account,
        in_reply_to,
        encrypt,
        plain,
    )
}

fn finish_sent(engine: &Arc<Engine>, out: &Writer, account: String, raw: Vec<u8>) {
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
                    || (copy.worth_awaiting() && await_sent_copy(&engine, &account, &copy).await)
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
}

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
            // End-to-end encryption is decided in the composer; here it is
            // enforced: an encrypted send never goes out unencrypted, and a reply
            // to encrypted mail is encrypted unless the caller explicitly asked
            // for plain (MCP, or a composer path that lost the flag).
            let (encrypt, implicit) = reply_protection(engine, p, &account, &in_reply_to)?;
            let creds = engine.ensure_valid_creds(&account).await?;
            let (from_addr, sender_name) =
                resolve_send_from(engine, &account, &creds, &requested_from)?;
            let ring = meron_core::openpgp::keyring::current();
            let mut undelivered = Vec::new();
            let raw = if encrypt {
                let keys = meron_core::openpgp::outgoing::resolve(&ring, &account, &from_addr, &to, &cc, &bcc)
                    .map_err(|err| {
                        if implicit {
                            anyhow::anyhow!("this replies to encrypted mail, so it can only be sent encrypted: {err:#}")
                        } else {
                            err
                        }
                    })?;
                let sent = smtp::send_encrypted(
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
                    &keys,
                )
                .await?;
                // The keys it went to are now the ones the composer compares
                // against. The mail is out: a failure here only
                // means the next send asks again.
                let recipients: Vec<String> = [&to, &cc, &bcc]
                    .iter()
                    .flat_map(|field| meron_core::openpgp::outgoing::split_recipients(field))
                    .collect();
                let used = meron_core::openpgp::outgoing::chosen_peer_keys(&ring, &recipients);
                if let Err(err) =
                    meron_core::openpgp::store::record_used(&engine.db.lock().unwrap(), &used)
                {
                    eprintln!("openpgp: couldn't record the keys used: {err:#}");
                }
                undelivered = sent.undelivered;
                sent.raw
            } else {
                let extra: Vec<(&str, String)> =
                    meron_core::openpgp::outgoing::autocrypt_for(&ring, &account, &from_addr)
                        .map(|v| vec![("Autocrypt", v)])
                        .unwrap_or_default();
                smtp::send(
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
                    &extra,
                )
                .await?
            };
            finish_sent(engine, out, account, raw);
            // Delivered to someone, so not an error (a resend would duplicate
            // it), but the composer says who didn't get it.
            Ok(json!({ "ok": true, "undelivered": undelivered }))
        }

        "invite.respond" => {
            let message = req_str(p, "message")?;
            let thread_id = req_str(p, "thread_id").unwrap_or_default();
            let response = req_str(p, "response")?;
            let response = meron_core::calendar::Response::parse(&response)
                .ok_or_else(|| anyhow::anyhow!("unknown invitation response: {response}"))?;
            let responded = meron_core::calendar::respond(
                engine,
                &message,
                &thread_id,
                response,
                &meron_core::parse::media_root(),
            )
            .await?;
            finish_sent(engine, out, responded.account, responded.raw);
            emit(out, "calendar.changed", json!({ "method": req.method })).await;
            Ok(json!({
                "ok": true,
                "response": response.as_str(),
                "event_id": responded.event_id,
            }))
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
            // A draft of an encrypted message is itself encrypted (to us
            // only), or its text would sit on the server in the clear. So is a
            // draft replying to encrypted mail that didn't say "plain"; that
            // needs only our own key, even with encryption turned off.
            let (encrypt, _) = reply_protection(engine, p, &account, &in_reply_to)?;
            let raw = if encrypt {
                let ring = meron_core::openpgp::keyring::current();
                let own = ring.default_key(&account).context(
                    "can't save this draft encrypted: this account has no encryption key",
                )?;
                let autocrypt = meron_core::openpgp::autocrypt::header_value(
                    &meron_core::openpgp::outgoing::bare_addr(&from_addr),
                    &own.public,
                )?;
                meron_core::openpgp::outgoing::wrap(&raw, &[&own.public], &own.secret, &autocrypt)?
            } else {
                raw
            };
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

        // Resolve what a bulk .eml export covers into source UIDs: a thread
        // (from the local cache, like copy/move) or, with `all`, every message
        // the server holds in the folder — not just the cached window.
        "messages.exportUids" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let mut uids = if p.get("all").and_then(Value::as_bool).unwrap_or(false) {
                engine
                    .with_read_session(&account, |session| {
                        let folder = folder.clone();
                        Box::pin(async move { imap::list_all_uids(session, &folder).await })
                    })
                    .await?
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                let (thread_key, subject_filter) =
                    store::split_thread_key(&req_str(p, "thread_key").unwrap_or_default());
                let uid = p.get("uid").and_then(Value::as_u64).map(|n| n as u32);
                store::resolve_message_uids(
                    &engine.db.lock().unwrap(),
                    &account,
                    &folder,
                    &thread_key,
                    subject_filter.as_deref(),
                    uid,
                    &opt_uids(p),
                )?
            };
            uids.sort_unstable();
            uids.dedup();
            Ok(json!({ "uids": uids }))
        }

        // Fetch a small batch of messages and write each one's RFC822 bytes to
        // `<dir>/<uid>.eml`. The caller packs them into the export archive and
        // batches the UIDs, so one slow or missing message costs one batch and
        // a whole batch never has to fit in a JSON response. UIDs the server no
        // longer has are simply absent from `saved`.
        "messages.saveRawBatch" => {
            let account = req_str(p, "account")?;
            let folder =
                canon_folder(&req_str(p, "folder").unwrap_or_else(|_| "INBOX".to_string()));
            let uids = opt_uids(p);
            let dir = std::path::PathBuf::from(req_str(p, "dir")?);

            let raw_messages = engine
                .with_read_session(&account, |session| {
                    let folder = folder.clone();
                    let uids = uids.clone();
                    Box::pin(async move {
                        imap::fetch_raw_messages_for_copy(session, &folder, &uids).await
                    })
                })
                .await?;
            let mut saved = Vec::with_capacity(raw_messages.len());
            for message in raw_messages {
                // Only UIDs that were asked for become file names.
                if !uids.contains(&message.uid) {
                    continue;
                }
                let path = dir.join(format!("{}.eml", message.uid));
                std::fs::write(&path, &message.raw)
                    .with_context(|| format!("write message export {}", path.display()))?;
                saved.push(message.uid);
            }
            Ok(json!({ "saved": saved }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
