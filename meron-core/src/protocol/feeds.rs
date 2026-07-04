use super::*;

pub(crate) fn add_mobile_rss_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let feed_url = opt_str(params, "feed_url");
    let display_name = opt_str(params, "display_name");
    with_mobile_db_mutex(data_dir, |db| {
        let account = rss::add(&db, &feed_url, &display_name).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "account": account }))
    })
}

pub(crate) fn add_mobile_rss_feed(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account = req_account_id(params)?;
    let feed_url = req_str(params, "feed_url")?;
    with_mobile_db_mutex(data_dir, |db| {
        rss::add_feed(&db, &account, &feed_url).map_err(|err| format!("{err:#}"))
    })
}

pub(crate) fn remove_mobile_rss_feed(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    with_mobile_db(data_dir, |conn| {
        rss::remove_feed(&conn, &thread_id).map_err(|err| format!("{err:#}"))
    })
}

pub(crate) fn move_mobile_rss_feed(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_account =
        req_str(params, "target_account").or_else(|_| req_str(params, "target_account_id"))?;
    with_mobile_db(data_dir, |conn| {
        rss::move_feed(&conn, &thread_id, &target_account).map_err(|err| format!("{err:#}"))
    })
}

pub(crate) fn export_mobile_opml(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account = req_account_id(params)?;
    with_mobile_db(data_dir, |conn| {
        let opml = rss::export_opml(&conn, &account).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "opml": opml }))
    })
}

pub(crate) fn import_mobile_opml(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account = req_account_id(params)?;
    let opml = req_str(params, "opml")?;
    with_mobile_db_mutex(data_dir, |db| {
        let imported = rss::import_opml(&db, &opml, &account).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "imported": imported }))
    })
}

pub(crate) fn sync_mobile_rss(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    with_mobile_db(data_dir, |conn| sync_mobile_rss_with_conn(conn, account_id))
}

pub(crate) fn sync_mobile_rss_with_conn(
    conn: rusqlite::Connection,
    account_id: String,
) -> Result<Value, String> {
    if !is_rss_account(&conn, &account_id)? {
        return Err(format!("account is not RSS: {account_id}"));
    }
    let db = Mutex::new(conn);
    let synced = rss::sync_account(&db, &account_id).map_err(|err| format!("{err:#}"))?;
    Ok(json!({
        "ok": true,
        "account": account_id,
        "synced": synced,
        "rss": true,
    }))
}

pub(crate) fn read_mobile_rss_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let limit = opt_u32(params, "limit");
    let before_cursor = params
        .get("before_cursor")
        .and_then(Value::as_str)
        .and_then(parse_rss_cursor);
    with_mobile_db(data_dir, |conn| {
        let (messages, next_cursor) =
            rss::read_thread_page(&conn, &thread_id, before_cursor, limit)
                .map_err(|err| format!("{err:#}"))?;
        let mut out = json!({ "messages": messages });
        if let Some(cursor) = next_cursor {
            out.as_object_mut()
                .unwrap()
                .insert("next_cursor".to_string(), Value::String(cursor));
        }
        Ok(out)
    })
}

pub(crate) fn mark_mobile_rss_thread_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let seen = params.get("seen").and_then(Value::as_bool).unwrap_or(true);
    let item_keys = rss_item_keys(params, &thread_id);
    with_mobile_db(data_dir, |conn| {
        if item_keys.is_empty() {
            rss::mark_thread_read(&conn, &thread_id, seen).map_err(|err| format!("{err:#}"))?;
        } else {
            rss::mark_items_read(&conn, &thread_id, &item_keys, seen)
                .map_err(|err| format!("{err:#}"))?;
        }
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn mark_mobile_rss_thread_starred(
    data_dir: &str,
    params: &Value,
) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let starred = params
        .get("starred")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let item_keys = rss_item_keys(params, &thread_id);
    with_mobile_db(data_dir, |conn| {
        if item_keys.is_empty() {
            rss::mark_thread_starred(&conn, &thread_id, starred)
                .map_err(|err| format!("{err:#}"))?;
        } else {
            rss::mark_items_starred(&conn, &thread_id, &item_keys, starred)
                .map_err(|err| format!("{err:#}"))?;
        }
        Ok(json!({ "ok": true }))
    })
}

pub(crate) fn parse_rss_cursor(cursor: &str) -> Option<(i64, String)> {
    let rest = cursor.strip_prefix("ts:")?;
    let (ts, key) = rest.split_once(':')?;
    Some((ts.parse().ok()?, key.to_string()))
}

pub(crate) fn is_rss_thread_id(thread_id: &str) -> bool {
    thread_id.contains("#rss#")
}

pub(crate) fn rss_item_keys(params: &Value, thread_id: &str) -> Vec<String> {
    params
        .get("item_keys")
        .or_else(|| params.get("message_ids"))
        .and_then(Value::as_array)
        .map(|items| {
            let item_prefix = format!("{thread_id}#");
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|raw| {
                    let value = raw.trim();
                    if value.is_empty() {
                        None
                    } else {
                        Some(
                            value
                                .strip_prefix(&item_prefix)
                                .unwrap_or(value)
                                .to_string(),
                        )
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
