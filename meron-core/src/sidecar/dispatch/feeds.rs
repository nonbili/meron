use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::protocol::Request;
use meron_core::rss;

use crate::Writer;
use crate::sidecar::params::*;

/// Handle RSS accounts and feeds: `account.addRss`, `feed.*`, `rss.*`.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    _out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        // Add an RSS feed: fetch + parse + persist on the blocking pool (network
        // I/O), returning the bridge Account JSON.
        "account.addRss" => {
            let feed_url = req_str(p, "feed_url")?;
            let display_name = req_str(p, "display_name").unwrap_or_default();
            let engine = engine.clone();
            let account =
                tokio::task::spawn_blocking(move || rss::add(&engine.db, &feed_url, &display_name))
                    .await??;
            Ok(json!({ "account": account }))
        }

        // Add a feed to an existing RSS account (network on the blocking pool).
        "feed.add" => {
            let account = req_str(p, "account")?;
            let feed_url = req_str(p, "feed_url")?;
            let engine = engine.clone();
            let res =
                tokio::task::spawn_blocking(move || rss::add_feed(&engine.db, &account, &feed_url))
                    .await??;
            Ok(res)
        }

        // Remove a single feed (subscription) and its items from an RSS account.
        "feed.remove" => {
            let thread_id = req_str(p, "thread_id")?;
            let res = rss::remove_feed(&engine.db.lock().unwrap(), &thread_id)?;
            Ok(res)
        }

        // Move a feed subscription between RSS accounts without losing cached
        // items or per-item read/starred state.
        "feed.move" => {
            let thread_id = req_str(p, "thread_id")?;
            let target_account = req_str(p, "target_account")?;
            let res = rss::move_feed(&engine.db.lock().unwrap(), &thread_id, &target_account)?;
            Ok(res)
        }

        // Name a feed in the list; an empty title restores the feed's own.
        "feed.rename" => {
            let thread_id = req_str(p, "thread_id")?;
            let title = p
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let res = rss::rename_feed(&engine.db.lock().unwrap(), &thread_id, &title)?;
            Ok(res)
        }

        // Serialize one RSS account's feeds to an OPML 2.0 document.
        "rss.exportOpml" => {
            let account = req_str(p, "account")?;
            let opml = rss::export_opml(&engine.db.lock().unwrap(), &account)?;
            Ok(json!({ "opml": opml }))
        }

        // Import feeds from an OPML document into one RSS account. Returns the
        // number of feeds added; the caller reloads accounts and syncs.
        "rss.importOpml" => {
            let opml = req_str(p, "opml")?;
            let account = req_str(p, "account")?;
            let engine = engine.clone();
            let imported =
                tokio::task::spawn_blocking(move || rss::import_opml(&engine.db, &opml, &account))
                    .await??;
            Ok(json!({ "imported": imported }))
        }

        // RSS thread read: paginated newest-first slice (or full thread when
        // `limit` is omitted), as final Message JSON.
        "rss.thread" => {
            let thread_id = req_str(p, "thread_id")?;
            let limit = p.get("limit").and_then(Value::as_u64).map(|n| n as u32);
            let before_cursor = p
                .get("before_cursor")
                .and_then(Value::as_str)
                .and_then(parse_rss_cursor);
            let (messages, next_cursor) = rss::read_thread_page(
                &engine.db.lock().unwrap(),
                &thread_id,
                before_cursor,
                limit,
            )?;
            let mut out = json!({ "messages": messages });
            if let Some(cursor) = next_cursor {
                out.as_object_mut()
                    .unwrap()
                    .insert("next_cursor".into(), Value::String(cursor));
            }
            Ok(out)
        }

        "rss.markRead" => {
            let thread_id = req_str(p, "thread_id")?;
            // Defaults to read; pass seen:false to mark unread.
            let seen = p.get("seen").and_then(Value::as_bool).unwrap_or(true);
            let item_keys = p
                .get("item_keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if item_keys.is_empty() {
                rss::mark_thread_read(&engine.db.lock().unwrap(), &thread_id, seen)?;
            } else {
                rss::mark_items_read(&engine.db.lock().unwrap(), &thread_id, &item_keys, seen)?;
            }
            Ok(json!({ "ok": true }))
        }

        "rss.markAllRead" => {
            let account = req_str(p, "account")?;
            let updated = rss::mark_account_read(&engine.db.lock().unwrap(), &account)?;
            Ok(json!({
                "ok": true,
                "updated": updated,
                "folder_unreads": { (account): { "inbox": 0 } },
            }))
        }

        "rss.markStarred" => {
            let thread_id = req_str(p, "thread_id")?;
            let starred = p.get("starred").and_then(Value::as_bool).unwrap_or(true);
            let item_keys = p
                .get("item_keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if item_keys.is_empty() {
                rss::mark_thread_starred(&engine.db.lock().unwrap(), &thread_id, starred)?;
            } else {
                rss::mark_items_starred(
                    &engine.db.lock().unwrap(),
                    &thread_id,
                    &item_keys,
                    starred,
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
