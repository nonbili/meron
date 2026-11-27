use super::super::*;
use super::{
    has_requested_mobile_message_ids, parse_thread_id_with_request_folder, requested_mobile_uids,
};

/// Save a tag change locally, then let the server catch up on its own: a tag
/// needs no connection, and a failed push is retried by the next sync.
pub(crate) fn set_mobile_thread_tags(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let parsed = parse_thread_id_with_request_folder(&thread_id, params)
        .ok_or_else(|| "invalid thread_id".to_string())?;
    let add = crate::tags::parse_tag_list(params.get("add"))?;
    let remove = crate::tags::parse_tag_list(params.get("remove"))?;
    let key = parsed
        .subject_filter
        .as_deref()
        .map(|subject| store::branch_compound_key(&parsed.thread_key, subject))
        .unwrap_or_else(|| parsed.thread_key.clone());
    let result = with_mobile_db(data_dir, |conn| {
        let uids = if has_requested_mobile_message_ids(params) || parsed.uid.is_some() {
            requested_mobile_uids(&conn, &parsed, params)?
        } else {
            Vec::new()
        };
        let targets = store::tag_targets(&conn, &parsed.account, &parsed.folder, &key, &uids)
            .map_err(|err| err.to_string())?;
        let tags = store::retag(&conn, &parsed.account, &targets, &add, &remove, true)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "tags": tags }))
    })?;
    crate::ffi::spawn_tag_push(data_dir, &parsed.account);
    Ok(result)
}

pub(crate) fn list_mobile_tags(data_dir: &str) -> Result<Value, String> {
    with_mobile_db(data_dir, |conn| {
        store::tag_counts(&conn, None)
            .map(store::tag_counts_json)
            .map_err(|err| err.to_string())
    })
}

/// Rename a tag everywhere, or delete it, then let the server catch up on its
/// own, as a tag change on one thread does.
pub(crate) fn replace_mobile_tag(data_dir: &str, params: &Value) -> Result<Value, String> {
    let (tag, to) = crate::tags::parse_tag_replacement(params)?;
    let mut accounts = Vec::new();
    let result = with_mobile_db(data_dir, |conn| {
        let (changed, rules) = store::replace_tag_and_rules(&conn, &tag, to.as_deref())
            .map_err(|err| err.to_string())?;
        accounts = changed;
        let queries = crate::tags::renamed_queries(params, &tag, to.as_deref());
        Ok(json!({ "ok": true, "tag": to, "rules": rules, "queries": queries }))
    })?;
    for account in &accounts {
        crate::ffi::spawn_tag_push(data_dir, account);
    }
    Ok(result)
}

pub(crate) fn mobile_rules(data_dir: &str) -> Result<Value, String> {
    with_mobile_db(data_dir, |conn| {
        let rules = store::rules_for_editing(&conn).map_err(|err| err.to_string())?;
        Ok(json!({ "rules": rules }))
    })
}

pub(crate) fn set_mobile_rules(data_dir: &str, params: &Value) -> Result<Value, String> {
    let rules = serde_json::from_value(params.get("rules").cloned().unwrap_or_default())
        .map_err(|err| err.to_string())?;
    with_mobile_db(data_dir, |conn| {
        if let Some(expected) = params
            .get("expected_rules")
            .filter(|value| !value.is_null())
        {
            let expected = serde_json::from_value::<Vec<store::Rule>>(expected.clone())
                .map_err(|err| err.to_string())?;
            let (rules, saved) = store::set_rules_if_current(&conn, &expected, rules)
                .map_err(|err| err.to_string())?;
            return Ok(json!({ "rules": rules, "conflict": !saved }));
        }
        let rules = store::set_rules(&conn, rules).map_err(|err| err.to_string())?;
        Ok(json!({ "rules": rules }))
    })
}

pub(crate) fn mobile_rule_matches(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_str(params, "id")?;
    with_mobile_db(data_dir, |conn| {
        let count =
            store::count_existing_rule_matches(&conn, &id).map_err(|err| err.to_string())?;
        let without_body =
            store::count_existing_rule_without_body(&conn, &id).map_err(|err| err.to_string())?;
        Ok(json!({ "count": count, "without_body": without_body }))
    })
}

pub(crate) fn mobile_rule_status(data_dir: &str) -> Result<Value, String> {
    with_mobile_db(data_dir, |conn| {
        let log = store::rule_log(&conn).map_err(|err| err.to_string())?;
        let running = store::running_rules(&conn).map_err(|err| err.to_string())?;
        Ok(json!({ "log": log, "running": running }))
    })
}

pub(crate) fn stop_mobile_rule(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_str(params, "id")?;
    with_mobile_db(data_dir, |conn| {
        store::stop_rule(&conn, &id).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn run_mobile_rule(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_str(params, "id")?;
    let engine = crate::ffi::engine_for(data_dir)?;
    let count = crate::ffi::engine_block_on(crate::engine::run_rule_on_existing(&engine, &id))?;
    Ok(json!({ "count": count }))
}
