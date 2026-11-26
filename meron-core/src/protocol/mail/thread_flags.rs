use super::super::*;
use super::{
    has_requested_mobile_message_ids, parse_thread_id_with_request_folder, requested_mobile_uids,
    update_mobile_read_state,
};

pub(crate) fn mark_mobile_thread_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let seen = req_bool(params, "seen")?;
    if is_rss_thread_id(&thread_id) {
        return mark_mobile_rss_thread_read(data_dir, params);
    }
    let parsed = parse_thread_id_with_request_folder(&thread_id, params)
        .ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let explicit_ids = has_requested_mobile_message_ids(params);
        // Marking a whole thread read reaches every folder it spans and only the
        // messages still unread; everything else acts on the requested folder.
        let whole_thread_read = seen && !explicit_ids && parsed.uid.is_none();
        let by_folder = if whole_thread_read {
            store::thread_uids_by_folder(
                &conn,
                &parsed.account,
                &parsed.folder,
                &parsed.thread_key,
                parsed.subject_filter.as_deref(),
                true,
            )
            .map_err(|err| err.to_string())?
        } else {
            let uids = if !seen && !explicit_ids && parsed.uid.is_none() {
                // Marking a whole thread unread flags its newest message only.
                store::newest_thread_uids(
                    &conn,
                    &parsed.account,
                    &parsed.folder,
                    &parsed.thread_key,
                    parsed.subject_filter.as_deref(),
                )
                .map_err(|err| err.to_string())?
            } else {
                requested_mobile_uids(&conn, &parsed, params)?
            };
            std::collections::BTreeMap::from([(parsed.folder.clone(), uids)])
        };
        // The server first, the store once it took the change — as markStarred
        // and markAllRead do. A store marked ahead of a failed write keeps the
        // messages read on the next reload although the server still has them
        // unread, and leaves a retry nothing to send.
        if by_folder.values().any(|uids| !uids.is_empty()) {
            let creds = load_mobile_account_creds(&conn, &parsed.account)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {}", parsed.account));
            }
            crate::ffi::engine_block_on(engine.store_seen_by_folder(
                &parsed.account,
                &by_folder,
                seen,
            ))?;
        }
        for (folder, uids) in by_folder
            .iter()
            .filter(|(folder, _)| **folder != parsed.folder)
        {
            for uid in uids {
                store::update_message_seen(&conn, &parsed.account, folder, *uid, seen)
                    .map_err(|err| err.to_string())?;
            }
        }
        let own_uids = by_folder
            .get(&parsed.folder)
            .map(Vec::as_slice)
            .unwrap_or_default();
        update_mobile_read_state(&conn, &parsed, params, own_uids, seen)?;
        let mut result = crate::mail_model::mutation_result(
            json!({ "ok": true }),
            &conn,
            &parsed.account,
            &thread_id,
            &parsed.folder,
            None,
            Some(!seen),
            None,
            false,
        )
        .map_err(|err| err.to_string())?;
        crate::mail_model::add_folder_unreads(
            &mut result,
            &conn,
            &parsed.account,
            by_folder.keys(),
        )
        .map_err(|err| err.to_string())?;
        Ok(result)
    })
}

pub(crate) fn mark_mobile_thread_starred(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let starred = req_bool(params, "starred")?;
    if is_rss_thread_id(&thread_id) {
        return mark_mobile_rss_thread_starred(data_dir, params);
    }
    let parsed = parse_thread_id_with_request_folder(&thread_id, params)
        .ok_or_else(|| "invalid thread_id".to_string())?;
    let engine = crate::ffi::engine_for(data_dir)?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        let key = parsed
            .subject_filter
            .as_deref()
            .map(|subject| store::branch_compound_key(&parsed.thread_key, subject))
            .unwrap_or_else(|| parsed.thread_key.clone());
        let message_scoped = has_requested_mobile_message_ids(params) || parsed.uid.is_some();
        let targets = store::starred_mutation_targets(
            &conn,
            &parsed.account,
            &parsed.folder,
            (!message_scoped).then_some(key.as_str()),
            &uids,
            starred,
        )
        .map_err(|err| err.to_string())?;
        if !targets.is_empty() {
            let creds = load_mobile_account_creds(&conn, &parsed.account)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {}", parsed.account));
            }
            crate::ffi::engine_block_on(crate::engine::mark_starred_copies(
                &engine,
                &parsed.account,
                &targets,
                starred,
            ))?;
        }
        crate::mail_model::mutation_result(
            json!({ "ok": true }),
            &conn,
            &parsed.account,
            &thread_id,
            &parsed.folder,
            None,
            None,
            Some(starred),
            false,
        )
        .map_err(|err| err.to_string())
    })
}
