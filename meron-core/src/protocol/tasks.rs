//! Mobile `tasks.*`. The same command vocabulary the desktop sidecar serves,
//! against the same tables — mobile just opens the store per call instead of
//! holding an engine.

use super::*;

use crate::tasks;

/// Route one `tasks.*` request. Mutations emit `tasks.changed` so a screen that
/// isn't the one holding the list re-reads.
pub(crate) fn dispatch_mobile_tasks(
    data_dir: &str,
    method: &str,
    params: &Value,
) -> Option<Result<Value, String>> {
    let mutates = !matches!(method, "tasks.lists" | "tasks.items" | "tasks.forThread");
    let result = match method {
        "tasks.lists" => with_mobile_db(data_dir, |conn| err_str(tasks::lists_payload(&conn))),

        "tasks.items" => {
            let list_id = match req_str(params, "list_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let include_completed = params
                .get("include_completed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::tasks_payload(&conn, &list_id, include_completed))
            })
        }

        "tasks.forThread" => {
            let thread_id = match req_str(params, "thread_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::thread_tasks_payload(&conn, &thread_id))
            })
        }

        "tasks.listCreate" => {
            let title = opt_str(params, "title");
            with_mobile_db(data_dir, |conn| err_str(tasks::create_list(&conn, &title)))
        }

        "tasks.listRename" => {
            let list_id = match req_str(params, "list_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let title = opt_str(params, "title");
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::rename_list(&conn, &list_id, &title))
            })
        }

        "tasks.listDelete" => {
            let list_id = match req_str(params, "list_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |mut conn| {
                err_str(tasks::delete_list(&mut conn, &list_id))
            })
        }

        "tasks.listReorder" => {
            let list_ids = match req_str_array(params, "list_ids") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |mut conn| {
                err_str(tasks::reorder_lists(&mut conn, &list_ids))
            })
        }

        "tasks.create" => {
            let list_id = opt_str(params, "list_id");
            let title = opt_str(params, "title");
            let notes = opt_str(params, "notes");
            let due_at = params.get("due_at").and_then(Value::as_i64).unwrap_or(0);
            let account = opt_str(params, "account");
            let thread_id = opt_str(params, "thread_id");
            let message_id = opt_str(params, "message_id");
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::create_task(
                    &conn,
                    &list_id,
                    &title,
                    &notes,
                    due_at,
                    &account,
                    &thread_id,
                    &message_id,
                ))
            })
        }

        "tasks.update" => {
            let task_id = match req_str(params, "task_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let title = params.get("title").and_then(Value::as_str);
            let notes = params.get("notes").and_then(Value::as_str);
            let due_at = params.get("due_at").and_then(Value::as_i64);
            let list_id = params.get("list_id").and_then(Value::as_str);
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::update_task(
                    &conn, &task_id, title, notes, due_at, list_id,
                ))
            })
        }

        "tasks.setDone" => {
            let task_id = match req_str(params, "task_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let done = params.get("done").and_then(Value::as_bool).unwrap_or(true);
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::set_done(&conn, &task_id, done))
            })
        }

        "tasks.delete" => {
            let task_id = match req_str(params, "task_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::delete_task(&conn, &task_id))
            })
        }

        "tasks.reorder" => {
            let list_id = match req_str(params, "list_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            let task_ids = match req_str_array(params, "task_ids") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |mut conn| {
                err_str(tasks::reorder_tasks(&mut conn, &list_id, &task_ids))
            })
        }

        "tasks.restore" => {
            let payload = params.get("restore").cloned().unwrap_or(Value::Null);
            with_mobile_db(data_dir, |conn| err_str(tasks::restore(&conn, &payload)))
        }

        "tasks.clearCompleted" => {
            let list_id = match req_str(params, "list_id") {
                Ok(value) => value,
                Err(err) => return Some(Err(err)),
            };
            with_mobile_db(data_dir, |conn| {
                err_str(tasks::clear_completed(&conn, &list_id))
            })
        }

        _ => return None,
    };

    if mutates && result.is_ok() {
        crate::ffi::emit_event("tasks.changed", json!({ "method": method }));
    }
    Some(result)
}

/// Empty answers for every `tasks.*` method, for the stub dispatcher that runs
/// before a data dir is bound.
pub(crate) fn stub_mobile_tasks(method: &str) -> Option<Value> {
    Some(match method {
        "tasks.lists" => json!({ "lists": [], "default_list_id": "" }),
        "tasks.items" => json!({ "list_id": "", "tasks": [] }),
        "tasks.forThread" => json!({ "thread_id": "", "tasks": [] }),
        "tasks.listCreate" => json!({ "list": Value::Null }),
        "tasks.create" | "tasks.update" | "tasks.setDone" => json!({ "task": Value::Null }),
        "tasks.clearCompleted" => json!({ "ok": true, "removed": 0, "restore": Value::Null }),
        "tasks.listDelete" | "tasks.delete" => json!({ "ok": true, "restore": Value::Null }),
        "tasks.restore" => json!({ "ok": true, "restored": 0 }),
        "tasks.listRename" | "tasks.listReorder" | "tasks.reorder" => json!({ "ok": true }),
        _ => return None,
    })
}

fn err_str(result: anyhow::Result<Value>) -> Result<Value, String> {
    result.map_err(|err| format!("{err:#}"))
}
