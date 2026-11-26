use super::super::*;

const DRAFT_SYNC_LIMIT: u32 = 20;

/// Wait out a Sent copy the provider has not exposed yet, off the send call, and
/// tell the UI once it is cached — the thread card's message count is a message
/// short until then. Its own thread for the same reason the sync tail uses one:
/// the send has to return now, and this can take seconds.
fn watch_for_sent_copy(
    engine: &std::sync::Arc<crate::engine::Engine>,
    account_id: &str,
    copy: crate::engine::SentCopy,
) {
    let engine = engine.clone();
    let account_id = account_id.to_string();
    std::thread::spawn(move || {
        let account = account_id.clone();
        let observed = crate::ffi::engine_block_on(async move {
            anyhow::Ok(crate::engine::await_sent_copy(&engine, &account, &copy).await)
        })
        .unwrap_or(false);
        if observed {
            crate::ffi::emit_event(
                "mail.sentCopyCached",
                crate::engine::sent_copy_cached_detail(&account_id),
            );
        }
    });
}

pub(crate) fn send_mobile_message(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let to = req_str(params, "to")?;
    let cc = opt_str(params, "cc");
    let bcc = opt_str(params, "bcc");
    let subject = opt_str(params, "subject");
    let body = opt_str(params, "body");
    let html = opt_str(params, "html");
    let in_reply_to = opt_str(params, "in_reply_to");
    let references = opt_str(params, "references");
    let reply_to = opt_str(params, "reply_to");
    let message_id = opt_str(params, "message_id");
    let requested_from = opt_str(params, "from");
    let attachments = parse_mobile_attachments(params)?;

    // Route through the shared Engine: reuses the warm session pool while
    // foreground (no per-send TLS+LOGIN) and runs the same Sent finalization as
    // desktop. Provider defaults decide whether this APPENDs or only refreshes
    // the server-created Sent copy.
    let engine = crate::ffi::engine_for(data_dir)?;
    let creds = crate::ffi::engine_block_on(engine.ensure_valid_creds(&account_id))?;
    if account_needs_reconnect(&creds) {
        return Err(format!("account needs reconnect: {account_id}"));
    }
    let (from_addr, sender_name) = {
        let conn = engine.db.lock().unwrap();
        resolve_mobile_send_from(&conn, &account_id, &creds, &requested_from)?
    };
    let (encrypt, implicit) = {
        let conn = engine.db.lock().unwrap();
        crate::openpgp::store::reply_protection(
            &conn,
            &account_id,
            &in_reply_to,
            params
                .get("encrypt")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            params
                .get("plain")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .map_err(|err| format!("{err:#}"))?
    };
    let ring = crate::openpgp::keyring::current();
    let mut undelivered = Vec::new();
    let raw = if encrypt {
        let keys = crate::openpgp::outgoing::resolve(
            &ring,
            &account_id,
            &from_addr,
            &to,
            &cc,
            &bcc,
        )
        .map_err(|err| {
            if implicit {
                format!("this replies to encrypted mail, so it can only be sent encrypted: {err:#}")
            } else {
                format!("{err:#}")
            }
        })?;
        let sent = crate::ffi::engine_block_on(smtp::send_encrypted(
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
        ))?;
        let recipients = [&to, &cc, &bcc]
            .iter()
            .flat_map(|field| crate::openpgp::outgoing::split_recipients(field))
            .collect::<Vec<_>>();
        let used = crate::openpgp::outgoing::chosen_peer_keys(&ring, &recipients);
        // Delivery already succeeded: a bookkeeping error must not invite a resend.
        let _ = crate::openpgp::store::record_used(&engine.db.lock().unwrap(), &used);
        undelivered = sent.undelivered;
        sent.raw
    } else {
        let extra = crate::openpgp::outgoing::autocrypt_for(&ring, &account_id, &from_addr)
            .map(|value| vec![("Autocrypt", value)])
            .unwrap_or_default();
        let sign = params.get("sign").and_then(Value::as_bool);
        let signer = crate::openpgp::outgoing::plain_signer(&ring, &account_id, sign)
            .map_err(|err| format!("{err:#}"))?;
        crate::ffi::engine_block_on(smtp::send(
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
            signer,
        ))?
    };
    let sent_bytes = raw.len();
    finish_mobile_sent(&engine, &account_id, &raw);
    Ok(json!({ "ok": true, "sent_bytes": sent_bytes, "undelivered": undelivered }))
}

/// File the Sent copy of a message that just went out, shared by every
/// mobile send path.
fn finish_mobile_sent(
    engine: &std::sync::Arc<crate::engine::Engine>,
    account_id: &str,
    raw: &[u8],
) {
    match crate::ffi::engine_block_on(crate::engine::append_to_sent(engine, account_id, raw)) {
        // Announced either way. The reload the send itself runs covers the
        // mailbox and the open thread, but not the Kanban board's columns, so a
        // copy already cached still leaves counts there a message short. One
        // the provider has not filed yet lands after that reload entirely, so
        // it is waited for first.
        Ok(copy) => {
            if copy.worth_awaiting() {
                watch_for_sent_copy(engine, account_id, copy);
            } else {
                crate::ffi::emit_event(
                    "mail.sentCopyCached",
                    crate::engine::sent_copy_cached_detail(account_id),
                );
            }
        }
        Err(err) => {
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.send",
                "APPEND to Sent failed for {account_id}: {err}"
            );
        }
    }
}

/// Answer a calendar invitation from its card: see [`crate::calendar::respond`].
pub(crate) fn respond_mobile_invite(data_dir: &str, params: &Value) -> Result<Value, String> {
    let message = req_str(params, "message")?;
    let thread_id = opt_str(params, "thread_id");
    let response = req_str(params, "response")?;
    let response = crate::calendar::Response::parse(&response)
        .ok_or_else(|| format!("unknown invitation response: {response}"))?;
    let engine = crate::ffi::engine_for(data_dir)?;
    let responded = crate::ffi::engine_block_on(crate::calendar::respond(
        &engine,
        &message,
        &thread_id,
        response,
        &std::path::PathBuf::from(data_dir).join("attachments"),
    ))?;
    finish_mobile_sent(&engine, &responded.account, &responded.raw);
    crate::ffi::emit_event("calendar.changed", json!({ "method": "invite.respond" }));
    Ok(json!({
        "ok": true,
        "response": response.as_str(),
        "event_id": responded.event_id,
    }))
}

pub(crate) fn save_mobile_draft(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let to = opt_str(params, "to");
    let cc = opt_str(params, "cc");
    let bcc = opt_str(params, "bcc");
    let subject = opt_str(params, "subject");
    let body = opt_str(params, "body");
    let html = opt_str(params, "html");
    let in_reply_to = opt_str(params, "in_reply_to");
    let references = opt_str(params, "references");
    let reply_to = opt_str(params, "reply_to");
    let draft_id = req_str(params, "draft_id")?;
    let requested_from = opt_str(params, "from");
    let attachments = parse_mobile_attachments(params)?;

    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let (from_addr, sender_name) =
            resolve_mobile_send_from(&conn, &account_id, &creds, &requested_from)?;
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
        )
        .map_err(|err| format!("{err:#}"))?;
        // The composer's sign switch travels with the draft.
        let raw = crate::openpgp::outgoing::with_draft_sign(
            &raw,
            params.get("sign").and_then(Value::as_bool),
        );
        let encrypt = params
            .get("encrypt")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let plain = params
            .get("plain")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let (encrypt, _) = crate::openpgp::store::reply_protection(
            &conn,
            &account_id,
            &in_reply_to,
            encrypt,
            plain,
        )
        .map_err(|err| format!("{err:#}"))?;
        let encrypted_draft = !plain
            && crate::openpgp::store::answers_encrypted(&conn, &account_id, &draft_id)
                .map_err(|err| format!("{err:#}"))?;
        let raw = if encrypt || encrypted_draft {
            let ring = crate::openpgp::keyring::current();
            let own = ring.default_key(&account_id).ok_or_else(|| {
                "can't save this draft encrypted: this account has no encryption key".to_string()
            })?;
            let autocrypt = crate::openpgp::autocrypt::header_value(
                &crate::openpgp::outgoing::bare_addr(&from_addr),
                &own.public,
            )
            .map_err(|err| format!("{err:#}"))?;
            crate::openpgp::outgoing::wrap(&raw, &[&own.public], &own.secret, &autocrypt)
                .map_err(|err| format!("{err:#}"))?
        } else {
            raw
        };
        let saved_id = draft_id.clone();
        let local_draft_id = draft_id.clone();
        let saved_bytes = raw.len();
        let drafts_folder =
            crate::ffi::engine_block_on(engine.with_write_session(&account_id, move |session| {
                let raw = raw.clone();
                let saved_id = saved_id.clone();
                Box::pin(async move {
                    let drafts = imap::find_drafts_folder(session)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                    imap::replace_draft(session, &drafts, &raw, &saved_id).await?;
                    anyhow::Ok(drafts)
                })
            }))?;
        // Read-only refresh, on its own session: the draft is already written
        // and must not be retried, and a draft we cannot parse must not make the
        // save look like it failed — the caller would drop the id it was
        // written under, leaving a copy nothing ever discards.
        let saved = json!({ "ok": true, "draft_id": draft_id, "saved_bytes": saved_bytes });
        let refresh = crate::ffi::engine_block_on(async {
            anyhow::Ok(
                crate::engine::refresh_written_folder(
                    &engine,
                    &account_id,
                    &drafts_folder,
                    DRAFT_SYNC_LIMIT,
                )
                .await,
            )
        })?;
        let Some(refresh) = refresh else {
            return Ok(saved);
        };
        let batch = &refresh.batch;
        store::upsert_messages(&conn, &account_id, &drafts_folder, &batch.messages)
            .map_err(|err| err.to_string())?;
        let keep_uid = batch
            .messages
            .iter()
            .filter(|message| message.message_id.eq_ignore_ascii_case(&local_draft_id))
            .map(|message| message.uid)
            .max();
        if keep_uid.is_some() {
            store::delete_draft_copies(
                &conn,
                &account_id,
                &drafts_folder,
                &local_draft_id,
                keep_uid,
            )
            .map_err(|err| err.to_string())?;
        }
        store::set_folder_state(
            &conn,
            &account_id,
            &drafts_folder,
            batch.uidvalidity,
            batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        Ok(saved)
    })
}

pub(crate) fn discard_mobile_draft(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let draft_id = opt_str(params, "draft_id");
    if draft_id.trim().is_empty() {
        return Ok(json!({ "ok": true, "deleted": 0, "permanent": true }));
    }
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let local_draft_id = draft_id.clone();
        // The LIST that finds the folder changes nothing and is where a dead
        // pooled session gives out — likely right after a long send — so it
        // preflights the delete that follows, as the desktop discard does.
        let drafts_slot: std::sync::Arc<std::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let preflight_slot = std::sync::Arc::clone(&drafts_slot);
        let (drafts, deleted) =
            crate::ffi::engine_block_on(engine.with_preflighted_write_session(
                &account_id,
                move |session| {
                    let slot = std::sync::Arc::clone(&preflight_slot);
                    Box::pin(async move {
                        let drafts = imap::find_drafts_folder(session)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                        *slot.lock().unwrap() = Some(drafts);
                        anyhow::Ok(())
                    })
                },
                move |session| {
                    let draft_id = draft_id.clone();
                    let slot = std::sync::Arc::clone(&drafts_slot);
                    Box::pin(async move {
                        let drafts = { slot.lock().unwrap().clone() }
                            .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
                        let deleted = imap::discard_draft(session, &drafts, &draft_id).await?;
                        anyhow::Ok((drafts, deleted))
                    })
                },
            ))?;
        store::delete_draft_copies(&conn, &account_id, &drafts, &local_draft_id, None)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "deleted": deleted, "permanent": true }))
    })
}
