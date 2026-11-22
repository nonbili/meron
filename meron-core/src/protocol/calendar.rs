//! Mobile `calendar.*`: the vocabulary the desktop sidecar serves, against the
//! same tables, with the store opened per call.

use super::*;

use crate::calendar::events;

/// Route one `calendar.*` request. Mutations emit `calendar.changed` so a
/// screen that isn't the one showing the calendar re-reads.
pub(crate) fn dispatch_mobile_calendar(
    data_dir: &str,
    method: &str,
    params: &Value,
) -> Option<Result<Value, String>> {
    let err_str = |result: anyhow::Result<Value>| result.map_err(|err| format!("{err:#}"));
    let result = match method {
        "calendar.events" => {
            let from = params.get("from").and_then(Value::as_i64).unwrap_or(0);
            let to = params.get("to").and_then(Value::as_i64).unwrap_or(0);
            let include_tasks = params
                .get("include_tasks")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return Some(with_mobile_db(data_dir, |conn| {
                err_str(events::events_payload(&conn, from, to, include_tasks))
            }));
        }
        "calendar.get" => {
            let event_id = match req_str(params, "event_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            return Some(with_mobile_db(data_dir, |conn| {
                err_str(events::event_payload(&conn, &event_id))
            }));
        }
        "calendar.create" => {
            let input = events::EventInput::from_params(params);
            with_mobile_db(data_dir, |conn| {
                err_str(events::create_event(&conn, &input))
            })
        }
        "calendar.update" => {
            let event_id = match req_str(params, "event_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let input = events::EventInput::from_params(params);
            with_mobile_db(data_dir, |conn| {
                err_str(events::update_event(&conn, &event_id, &input))
            })
        }
        "calendar.delete" => {
            let event_id = match req_str(params, "event_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |conn| {
                err_str(events::delete_event(&conn, &event_id))
            })
        }
        "invite.addToCalendar" => {
            let message = match req_str(params, "message") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let thread_id = opt_str(params, "thread_id");
            let media_root = std::path::PathBuf::from(data_dir).join("attachments");
            with_mobile_db(data_dir, |conn| {
                err_str(
                    crate::calendar::add_to_calendar(&conn, &message, &thread_id, &media_root)
                        .map(|event_id| json!({ "ok": true, "event_id": event_id })),
                )
            })
        }
        "invite.removeFromCalendar" => {
            let message = match req_str(params, "message") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let media_root = std::path::PathBuf::from(data_dir).join("attachments");
            with_mobile_db(data_dir, |conn| {
                err_str(
                    crate::calendar::remove_from_calendar(&conn, &message, &media_root)
                        .map(|()| json!({ "ok": true })),
                )
            })
        }
        // The host reads the file (a picker's) and saves the export (a share
        // sheet or save dialog); the core only turns iCalendar text into
        // events and back. A preview says what an import would bring in.
        "calendar.previewImport" | "calendar.importIcs" => {
            let ics = match req_str(params, "ics") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let preview = method == "calendar.previewImport";
            let result = with_mobile_db(data_dir, |mut conn| {
                err_str(
                    crate::calendar::transfer::import(&mut conn, &ics, preview)
                        .map_err(anyhow::Error::from)
                        .and_then(|summary| Ok(serde_json::to_value(summary)?)),
                )
            });
            // Nothing changed for a preview, or an import that took nothing.
            let imported = result
                .as_ref()
                .ok()
                .and_then(|summary| summary.get("imported"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if preview || imported == 0 {
                return Some(result);
            }
            result
        }
        "calendar.exportIcs" => {
            let event_id = opt_str(params, "event_id");
            let calendar = opt_str(params, "calendar");
            return Some(with_mobile_db(data_dir, |conn| {
                err_str(
                    crate::calendar::transfer::export(
                        &conn,
                        (!event_id.is_empty()).then_some(event_id.as_str()),
                        &calendar,
                    )
                    .map(|ics| json!({ "ics": ics })),
                )
            }));
        }
        "calendar.restore" => {
            let restore = params.get("restore").cloned().unwrap_or(Value::Null);
            with_mobile_db(data_dir, |conn| {
                err_str(events::restore_event(&conn, &restore))
            })
        }
        _ => return None,
    };
    if let Ok(value) = &result {
        crate::ffi::emit_event("calendar.changed", json!({ "method": method }));
        // An edit to a synced event goes to its server straight away: only
        // the calendars it touched.
        let calendars = [
            &value["event"]["calendar"],
            &value["restore"]["calendar"],
            &value["moved_from"],
        ]
        .iter()
        .filter_map(|calendar| calendar.as_str())
        .filter(|calendar| !calendar.is_empty())
        .map(str::to_string)
        .collect();
        push_soon(data_dir, calendars, false);
    }
    Some(result)
}

/// Empty answers for every `calendar.*` method, for the stub dispatcher that
/// runs before a data dir is bound.
pub(crate) fn stub_mobile_calendar(method: &str) -> Option<Value> {
    Some(match method {
        "calendar.events" => json!({ "events": [] }),
        "calendar.get" | "calendar.create" | "calendar.update" | "calendar.restore" => {
            json!({ "event": Value::Null })
        }
        "calendar.delete" => json!({ "ok": true, "restore": Value::Null }),
        "calendar.previewImport" | "calendar.importIcs" => {
            json!({ "imported": 0, "duplicates": 0, "skipped": 0 })
        }
        "calendar.exportIcs" => json!({ "ics": "" }),
        "invite.removeFromCalendar" | "invite.addToCalendar" => json!({ "ok": true }),
        _ => return None,
    })
}
