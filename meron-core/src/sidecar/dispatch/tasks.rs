use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::protocol::Request;
use meron_core::tasks;

use crate::sidecar::params::*;
use crate::{Writer, emit};

/// Handle the optional Tasks surface: `tasks.*`.
///
/// Every call is local SQLite, so unlike the mail and feed handlers nothing
/// here goes to the blocking pool — there is no socket to wait on.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    let result = match req.method.as_str() {
        "tasks.lists" => return tasks::lists_payload(&engine.db.lock().unwrap()),

        "tasks.items" => {
            let list_id = req_str(p, "list_id")?;
            let include_completed = p
                .get("include_completed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return tasks::tasks_payload(&engine.db.lock().unwrap(), &list_id, include_completed);
        }

        // Which tasks point at a thread, for the "already in Tasks" affordance
        // in the mail surfaces.
        "tasks.forThread" => {
            let thread_id = req_str(p, "thread_id")?;
            return tasks::thread_tasks_payload(&engine.db.lock().unwrap(), &thread_id);
        }

        "tasks.listCreate" => {
            let title = req_str(p, "title").unwrap_or_default();
            tasks::create_list(&engine.db.lock().unwrap(), &title)?
        }

        "tasks.listRename" => {
            let list_id = req_str(p, "list_id")?;
            let title = req_str(p, "title")?;
            tasks::rename_list(&engine.db.lock().unwrap(), &list_id, &title)?
        }

        "tasks.listDelete" => {
            let list_id = req_str(p, "list_id")?;
            tasks::delete_list(&mut engine.db.lock().unwrap(), &list_id)?
        }

        "tasks.listReorder" => {
            let list_ids = req_str_array(p, "list_ids")?;
            tasks::reorder_lists(&mut engine.db.lock().unwrap(), &list_ids)?
        }

        "tasks.create" => {
            let list_id = req_str(p, "list_id").unwrap_or_default();
            let title = req_str(p, "title").unwrap_or_default();
            let notes = req_str(p, "notes").unwrap_or_default();
            let due_at = p.get("due_at").and_then(Value::as_i64).unwrap_or(0);
            let account = req_str(p, "account").unwrap_or_default();
            let thread_id = req_str(p, "thread_id").unwrap_or_default();
            let message_id = req_str(p, "message_id").unwrap_or_default();
            tasks::create_task(
                &engine.db.lock().unwrap(),
                &list_id,
                &title,
                &notes,
                due_at,
                &account,
                &thread_id,
                &message_id,
            )?
        }

        // A patch: an absent field means "leave it alone", so the editor can
        // save a title without knowing the notes.
        "tasks.update" => {
            let task_id = req_str(p, "task_id")?;
            let title = p.get("title").and_then(Value::as_str);
            let notes = p.get("notes").and_then(Value::as_str);
            let due_at = p.get("due_at").and_then(Value::as_i64);
            let list_id = p.get("list_id").and_then(Value::as_str);
            tasks::update_task(
                &engine.db.lock().unwrap(),
                &task_id,
                title,
                notes,
                due_at,
                list_id,
            )?
        }

        "tasks.setDone" => {
            let task_id = req_str(p, "task_id")?;
            let done = req_bool(p, "done")?;
            tasks::set_done(&engine.db.lock().unwrap(), &task_id, done)?
        }

        "tasks.delete" => {
            let task_id = req_str(p, "task_id")?;
            tasks::delete_task(&engine.db.lock().unwrap(), &task_id)?
        }

        "tasks.reorder" => {
            let list_id = req_str(p, "list_id")?;
            let task_ids = req_str_array(p, "task_ids")?;
            tasks::reorder_tasks(&mut engine.db.lock().unwrap(), &list_id, &task_ids)?
        }

        // Undo: hands back the `restore` blob a delete returned.
        "tasks.restore" => {
            let payload = p.get("restore").cloned().unwrap_or(Value::Null);
            tasks::restore(&engine.db.lock().unwrap(), &payload)?
        }

        "tasks.clearCompleted" => {
            let list_id = req_str(p, "list_id")?;
            tasks::clear_completed(&engine.db.lock().unwrap(), &list_id)?
        }

        other => return Err(anyhow::anyhow!("unknown method: {other}")),
    };

    // Only mutations reach here. One event for all of them: a second surface
    // showing tasks (the thread pane's badge, another window) re-reads rather
    // than trying to replay the specific change.
    emit(out, "tasks.changed", json!({ "method": req.method })).await;
    Ok(result)
}
