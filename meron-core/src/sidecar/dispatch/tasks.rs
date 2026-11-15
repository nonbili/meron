use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::protocol::Request;
use meron_core::tasks;

use crate::sidecar::params::*;
use crate::{Writer, emit};

/// SQLite and lock waits run in the blocking pool; events are emitted only
/// after the database guard has been released.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let method = req.method.clone();
    let params = req.params.clone();
    let result = crate::sidecar::db::run(&engine.db, "tasks.dispatch", move |db| {
        execute(db, &method, &params)
    })
    .await?;
    if !matches!(
        req.method.as_str(),
        "tasks.lists" | "tasks.items" | "tasks.forThread"
    ) {
        emit(out, "tasks.changed", json!({ "method": req.method })).await;
    }
    Ok(result)
}

fn execute(db: &mut rusqlite::Connection, method: &str, p: &Value) -> anyhow::Result<Value> {
    let result = match method {
        "tasks.lists" => return tasks::lists_payload(db),

        "tasks.items" => {
            let list_id = req_str(p, "list_id")?;
            let include_completed = p
                .get("include_completed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return tasks::tasks_payload(db, &list_id, include_completed);
        }

        // Which tasks point at a thread, for the "already in Tasks" affordance
        // in the mail surfaces.
        "tasks.forThread" => {
            let thread_id = req_str(p, "thread_id")?;
            return tasks::thread_tasks_payload(db, &thread_id);
        }

        "tasks.listCreate" => {
            let title = req_str(p, "title").unwrap_or_default();
            tasks::create_list(db, &title)?
        }

        "tasks.listRename" => {
            let list_id = req_str(p, "list_id")?;
            let title = req_str(p, "title")?;
            tasks::rename_list(db, &list_id, &title)?
        }

        "tasks.listDelete" => {
            let list_id = req_str(p, "list_id")?;
            tasks::delete_list(db, &list_id)?
        }

        "tasks.listReorder" => {
            let list_ids = req_str_array(p, "list_ids")?;
            tasks::reorder_lists(db, &list_ids)?
        }

        "tasks.create" => {
            let list_id = req_str(p, "list_id").unwrap_or_default();
            let title = req_str(p, "title").unwrap_or_default();
            let notes = req_str(p, "notes").unwrap_or_default();
            let due_at = p.get("due_at").and_then(Value::as_i64).unwrap_or(0);
            let account = req_str(p, "account").unwrap_or_default();
            let thread_id = req_str(p, "thread_id").unwrap_or_default();
            let message_id = req_str(p, "message_id").unwrap_or_default();
            tasks::mutate_with_due_time(db, p, |conn| {
                tasks::create_task(
                    conn,
                    &list_id,
                    &title,
                    &notes,
                    due_at,
                    &account,
                    &thread_id,
                    &message_id,
                )
            })?
        }

        // A patch: an absent field means "leave it alone", so the editor can
        // save a title without knowing the notes.
        "tasks.update" => {
            let task_id = req_str(p, "task_id")?;
            let title = p.get("title").and_then(Value::as_str);
            let notes = p.get("notes").and_then(Value::as_str);
            let due_at = p.get("due_at").and_then(Value::as_i64);
            let list_id = p.get("list_id").and_then(Value::as_str);
            tasks::mutate_with_due_time(db, p, |conn| {
                tasks::update_task(conn, &task_id, title, notes, due_at, list_id)
            })?
        }

        "tasks.setDone" => {
            let task_id = req_str(p, "task_id")?;
            let done = req_bool(p, "done")?;
            tasks::set_done(db, &task_id, done)?
        }

        "tasks.delete" => {
            let task_id = req_str(p, "task_id")?;
            tasks::delete_task(db, &task_id)?
        }

        "tasks.reorder" => {
            let list_id = req_str(p, "list_id")?;
            let task_ids = req_str_array(p, "task_ids")?;
            tasks::reorder_tasks(db, &list_id, &task_ids)?
        }

        // Undo: hands back the `restore` blob a delete returned.
        "tasks.restore" => {
            let payload = p.get("restore").cloned().unwrap_or(Value::Null);
            tasks::restore(db, &payload)?
        }

        "tasks.clearCompleted" => {
            let list_id = req_str(p, "list_id")?;
            tasks::clear_completed(db, &list_id)?
        }

        other => return Err(anyhow::anyhow!("unknown method: {other}")),
    };

    Ok(result)
}
