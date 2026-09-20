//! Tasks: the shape both frontends render.
//!
//! Desktop (`tasks.*` in the sidecar) and mobile (the same names over FFI) ask
//! the same questions of the same tables, so the JSON they get back is built
//! here rather than twice in the clients. That is the mistake kanban boards
//! made — stored as an opaque blob and parsed once per platform — and it is not
//! repeated.
//!
//! Everything is local. There is no server, no sync and no network call in this
//! module; the only route a task takes off the device is the config backup.

#[cfg(test)]
mod tests;

use anyhow::{Result, anyhow};
use base64::Engine as _;
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::store::{self, Task, TaskList};

/// Title given to the list created on first use, when the user has none. Kept
/// in English here and translated by neither client: it is a real row the user
/// can rename, not a label, and silently retitling their data when they switch
/// language would be worse than leaving what they first saw.
pub const DEFAULT_LIST_TITLE: &str = "My Tasks";

/// A fresh id. Unlike a feed — whose id is a hash of its URL, so the same feed
/// added twice collides on purpose — a task has nothing stable to hash: two
/// tasks with the same title are two tasks. So these are random.
pub fn new_id(prefix: &str) -> String {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 12];
    if ring::rand::SystemRandom::new().fill(&mut bytes).is_err() {
        // A failed RNG must not lose the user's task. Fall back to the clock
        // plus the address of a local, which differ between two calls in the
        // same second far more reliably than the clock alone.
        let now = store::now_unix().to_le_bytes();
        bytes[..8].copy_from_slice(&now);
        let addr = (&bytes as *const _ as usize).to_le_bytes();
        bytes[8..].copy_from_slice(&addr[..4]);
    }
    format!(
        "{prefix}-{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

fn list_json(list: &TaskList) -> Value {
    json!({
        "id": list.id,
        "title": list.title,
        "sort_order": list.sort_order,
        "created_at": list.created_at,
        "updated_at": list.updated_at,
    })
}

fn task_json(task: &Task) -> Value {
    json!({
        "id": task.id,
        "list_id": task.list_id,
        "title": task.title,
        "notes": task.notes,
        "due_at": task.due_at,
        "completed_at": task.completed_at,
        "done": task.completed_at > 0,
        "sort_order": task.sort_order,
        "account": task.account,
        "thread_id": task.thread_id,
        "message_id": task.message_id,
        "created_at": task.created_at,
        "updated_at": task.updated_at,
    })
}

/// Guarantee at least one list exists, returning the id of the first one.
///
/// A Tasks surface with no list has nothing to put a task in, and asking the
/// user to create a list before they can write a task is a worse first run than
/// handing them one to rename.
pub fn ensure_default_list(conn: &Connection) -> Result<String> {
    if let Some(first) = store::task_lists(conn)?.into_iter().next() {
        return Ok(first.id);
    }
    let list = store::insert_task_list(conn, &new_id("tasklist"), DEFAULT_LIST_TITLE)?;
    Ok(list.id)
}

/// Every list, plus the id the client should select when it has no opinion.
pub fn lists_payload(conn: &Connection) -> Result<Value> {
    let default_id = ensure_default_list(conn)?;
    let lists = store::task_lists(conn)?;
    Ok(json!({
        "lists": lists.iter().map(list_json).collect::<Vec<_>>(),
        "default_list_id": default_id,
    }))
}

pub fn tasks_payload(conn: &Connection, list_id: &str, include_completed: bool) -> Result<Value> {
    let tasks = store::tasks_for_list(conn, list_id, include_completed)?;
    Ok(json!({
        "list_id": list_id,
        "tasks": tasks.iter().map(task_json).collect::<Vec<_>>(),
    }))
}

pub fn thread_tasks_payload(conn: &Connection, thread_id: &str) -> Result<Value> {
    let tasks = store::tasks_for_thread(conn, thread_id)?;
    Ok(json!({
        "thread_id": thread_id,
        "tasks": tasks.iter().map(task_json).collect::<Vec<_>>(),
    }))
}

pub fn create_list(conn: &Connection, title: &str) -> Result<Value> {
    let title = fallback_title(title, DEFAULT_LIST_TITLE);
    let list = store::insert_task_list(conn, &new_id("tasklist"), &title)?;
    Ok(json!({ "list": list_json(&list) }))
}

pub fn rename_list(conn: &Connection, list_id: &str, title: &str) -> Result<Value> {
    let title = fallback_title(title, DEFAULT_LIST_TITLE);
    store::rename_task_list(conn, list_id, &title)?;
    Ok(json!({ "ok": true }))
}

/// Delete a list and hand back everything it took with it.
///
/// The `restore` payload is the whole point: deleting a list destroys work that
/// exists nowhere else — there is no server copy to re-sync — so the client
/// needs enough to put it back, and gets the rows verbatim rather than a
/// summary it would have to reconstruct from.
pub fn delete_list(conn: &mut Connection, list_id: &str) -> Result<Value> {
    let Some(list) = store::task_list(conn, list_id)? else {
        return Ok(json!({ "ok": true, "restore": Value::Null }));
    };
    let tasks = store::tasks_for_list(conn, list_id, true)?;
    store::delete_task_list(conn, list_id)?;
    Ok(json!({
        "ok": true,
        "restore": {
            "list": list_json(&list),
            "tasks": tasks.iter().map(task_json).collect::<Vec<_>>(),
        },
    }))
}

pub fn reorder_lists(conn: &mut Connection, ordered_ids: &[String]) -> Result<Value> {
    store::reorder_task_lists(conn, ordered_ids)?;
    Ok(json!({ "ok": true }))
}

/// Create a task. `list_id` may be empty, in which case it lands in the default
/// list — what "Add to Tasks" from a mail thread wants, since that surface has
/// no list picker.
#[allow(clippy::too_many_arguments)]
pub fn create_task(
    conn: &Connection,
    list_id: &str,
    title: &str,
    notes: &str,
    due_at: i64,
    account: &str,
    thread_id: &str,
    message_id: &str,
) -> Result<Value> {
    let list_id = if list_id.is_empty() {
        ensure_default_list(conn)?
    } else {
        list_id.to_string()
    };
    let title = fallback_title(title, UNTITLED_TASK);
    let task = store::insert_task(
        conn,
        &new_id("task"),
        &list_id,
        &title,
        notes,
        due_at,
        account,
        thread_id,
        message_id,
    )?;
    Ok(json!({ "task": task_json(&task) }))
}

pub fn update_task(
    conn: &Connection,
    task_id: &str,
    title: Option<&str>,
    notes: Option<&str>,
    due_at: Option<i64>,
    list_id: Option<&str>,
) -> Result<Value> {
    // An edit that blanks the title would leave an unclickable empty row, so
    // the blank is dropped rather than saved.
    let title = title.map(str::trim).filter(|value| !value.is_empty());
    store::update_task(conn, task_id, title, notes, due_at, list_id)?;
    let task = store::task(conn, task_id)?.ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
    Ok(json!({ "task": task_json(&task) }))
}

pub fn set_done(conn: &Connection, task_id: &str, done: bool) -> Result<Value> {
    store::set_task_done(conn, task_id, done)?;
    let task = store::task(conn, task_id)?.ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
    Ok(json!({ "task": task_json(&task) }))
}

pub fn delete_task(conn: &Connection, task_id: &str) -> Result<Value> {
    let Some(task) = store::task(conn, task_id)? else {
        return Ok(json!({ "ok": true, "restore": Value::Null }));
    };
    store::delete_task(conn, task_id)?;
    Ok(json!({ "ok": true, "restore": { "tasks": [task_json(&task)] } }))
}

pub fn reorder_tasks(
    conn: &mut Connection,
    list_id: &str,
    ordered_ids: &[String],
) -> Result<Value> {
    store::reorder_tasks(conn, list_id, ordered_ids)?;
    Ok(json!({ "ok": true }))
}

pub fn clear_completed(conn: &Connection, list_id: &str) -> Result<Value> {
    let cleared: Vec<Task> = store::tasks_for_list(conn, list_id, true)?
        .into_iter()
        .filter(|task| task.completed_at > 0)
        .collect();
    let removed = store::clear_completed(conn, list_id)?;
    Ok(json!({
        "ok": true,
        "removed": removed,
        "restore": { "tasks": cleared.iter().map(task_json).collect::<Vec<_>>() },
    }))
}

/// Undo a delete by putting back exactly what it returned.
///
/// Takes the `restore` payload verbatim, so the client stores one opaque blob
/// rather than learning the shape of a task. Restoring twice is a no-op, not an
/// error: two undos of one delete is a double-click, not a conflict.
pub fn restore(conn: &Connection, payload: &Value) -> Result<Value> {
    if let Some(list) = payload.get("list").filter(|value| value.is_object()) {
        let list: TaskList = serde_json::from_value(list.clone())?;
        if !list.id.is_empty() {
            store::restore_task_list(conn, &list)?;
        }
    }
    let mut restored = 0;
    if let Some(tasks) = payload.get("tasks").and_then(Value::as_array) {
        for value in tasks {
            let task: Task = serde_json::from_value(value.clone())?;
            // A task whose list went away with it has nowhere to land; the list
            // branch above put it back first, so this only skips real junk.
            if task.id.is_empty() || task.list_id.is_empty() {
                continue;
            }
            store::restore_task(conn, &task)?;
            restored += 1;
        }
    }
    Ok(json!({ "ok": true, "restored": restored }))
}

const UNTITLED_TASK: &str = "Untitled";

fn fallback_title(title: &str, fallback: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
