//! Tasks: the shape both frontends render.
//!
//! Desktop (`tasks.*` in the sidecar) and mobile (the same names over FFI) ask
//! the same questions of the same tables, so the JSON they get back is built
//! here rather than twice in the clients. That is the mistake kanban boards
//! made — stored as an opaque blob and parsed once per platform — and it is not
//! repeated.
//!
//! Task mutations and their sync queue entries share a database transaction.

#[cfg(test)]
mod tests;

use anyhow::{Result, anyhow};
use base64::Engine as _;
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::store::{self, Task, TaskList};
use crate::{caldav::todo, google_tasks};

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
        "due_has_time": task.due_has_time,
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
    if let Some(first) = store::task_lists(conn)?
        .into_iter()
        .find(|list| is_local_list(&list.id))
    {
        return Ok(first.id);
    }
    let list = store::insert_task_list(conn, &new_id("tasklist"), DEFAULT_LIST_TITLE)?;
    Ok(list.id)
}

/// Every list, plus the id the client should select when it has no opinion.
pub fn lists_payload(conn: &Connection) -> Result<Value> {
    let default_id = ensure_default_list(conn)?;
    let lists = store::task_lists(conn)?;
    let lists: Result<Vec<Value>> = lists
        .iter()
        .map(|list| {
            let mut value = list_json(list);
            if todo::calendar_id(&list.id).is_some() {
                value["provider"] = json!("caldav");
                value["read_only"] = json!(!todo::writable(conn, &list.id)?);
            } else if google_tasks::is_list(&list.id) {
                value["provider"] = json!("google");
            }
            Ok(value)
        })
        .collect();
    Ok(json!({
        "lists": lists?,
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
    require_local_list(list_id)?;
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
    require_local_list(list_id)?;
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
    require_writable(conn, &list_id)?;
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
    let old = store::task(conn, task_id)?.ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
    require_writable(conn, &old.list_id)?;
    let list_id = list_id.filter(|id| *id != old.list_id);
    if let Some(id) = list_id {
        require_local_list(&old.list_id)?;
        require_local_list(id)?;
    }
    // An edit that blanks the title would leave an unclickable empty row, so
    // the blank is dropped rather than saved.
    let title = title.map(str::trim).filter(|value| !value.is_empty());
    store::update_task(conn, task_id, title, notes, due_at, list_id)?;
    let task = store::task(conn, task_id)?.ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
    Ok(json!({ "task": task_json(&task) }))
}

pub fn set_done(conn: &Connection, task_id: &str, done: bool) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    let task = store::task(&tx, task_id)?.ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
    require_writable(&tx, &task.list_id)?;
    if (task.completed_at > 0) == done {
        return Ok(json!({ "task": task_json(&task) }));
    }
    store::set_task_done(&tx, task_id, done)?;
    queue(&tx, task_id, &task.list_id, None)?;
    let updated = store::task(&tx, task_id)?.ok_or_else(|| anyhow!("unknown task: {task_id}"))?;
    tx.commit()?;
    Ok(json!({ "task": task_json(&updated) }))
}

fn task_for_undo(conn: &Connection, task: &Task) -> Result<Value> {
    let mut value = task_json(task);
    if todo::calendar_id(&task.list_id).is_some() {
        value["caldav_ics"] = json!(todo::original(conn, &task.id)?);
    }
    Ok(value)
}

pub fn delete_task(conn: &Connection, task_id: &str) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    let Some(task) = store::task(&tx, task_id)? else {
        return Ok(json!({ "ok": true, "restore": Value::Null }));
    };
    require_writable(&tx, &task.list_id)?;
    let snapshot = task_for_undo(&tx, &task)?;
    queue_delete(&tx, &task)?;
    store::delete_task(&tx, task_id)?;
    tx.commit()?;
    Ok(json!({ "ok": true, "restore": { "tasks": [snapshot] } }))
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
    let tx = conn.unchecked_transaction()?;
    require_writable(&tx, list_id)?;
    let mut snapshots = Vec::new();
    for task in store::tasks_for_list(&tx, list_id, true)?
        .into_iter()
        .filter(|task| task.completed_at > 0)
    {
        snapshots.push(task_for_undo(&tx, &task)?);
        queue_delete(&tx, &task)?;
    }
    let removed = store::clear_completed(&tx, list_id)?;
    tx.commit()?;
    Ok(json!({ "ok": true, "removed": removed, "restore": { "tasks": snapshots } }))
}

/// Undo a delete by putting back exactly what it returned.
///
/// Takes the `restore` payload verbatim, so the client stores one opaque blob
/// rather than learning the shape of a task. Restoring twice is a no-op, not an
/// error: two undos of one delete is a double-click, not a conflict.
pub fn restore(conn: &Connection, payload: &Value) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    if let Some(list) = payload.get("list").filter(|value| value.is_object()) {
        let list: TaskList = serde_json::from_value(list.clone())?;
        if !list.id.is_empty() {
            require_local_list(&list.id)?;
            store::restore_task_list(&tx, &list)?;
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
            if store::task(&tx, &task.id)?.is_none() {
                require_writable(&tx, &task.list_id)?;
                store::restore_task(&tx, &task)?;
                queue(
                    &tx,
                    &task.id,
                    &task.list_id,
                    value.get("caldav_ics").and_then(Value::as_str),
                )?;
            }
            restored += 1;
        }
    }
    tx.commit()?;
    Ok(json!({ "ok": true, "restored": restored }))
}

pub(crate) fn is_local_list(id: &str) -> bool {
    todo::calendar_id(id).is_none() && !google_tasks::is_list(id)
}

fn require_writable(conn: &Connection, id: &str) -> Result<()> {
    todo::require_writable(conn, id)?;
    google_tasks::require_writable(conn, id)
}

fn queue(conn: &Connection, id: &str, list: &str, caldav_ics: Option<&str>) -> Result<()> {
    if google_tasks::is_list(list) {
        google_tasks::queue(conn, id, list)
    } else {
        todo::queue(conn, id, caldav_ics)
    }
}

fn queue_delete(conn: &Connection, task: &Task) -> Result<()> {
    todo::queue_delete(conn, task)?;
    google_tasks::queue_delete(conn, &task.id, &task.list_id)
}

fn require_local_list(id: &str) -> Result<()> {
    anyhow::ensure!(
        is_local_list(id),
        "This action is available for local task lists"
    );
    Ok(())
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

/// Apply the optional time flag to a task mutation's response as well as storage.
pub fn apply_due_time(conn: &Connection, params: &Value, mut result: Value) -> Result<Value> {
    if let (Some(has_time), Some(id)) = (
        params.get("due_has_time").and_then(Value::as_bool),
        result["task"]["id"].as_str(),
    ) {
        let has_time = has_time && result["task"]["due_at"].as_i64().unwrap_or(0) > 0;
        store::set_task_due_time(conn, id, has_time)?;
        result["task"]["due_has_time"] = json!(has_time);
    }
    Ok(result)
}

/// Save the date, optional time flag, and other edits as one visible state.
/// Reminder readers on other connections see either the old task or the new
/// task, and a failure saving the time flag rolls back the entire mutation.
pub fn mutate_with_due_time(
    conn: &Connection,
    params: &Value,
    mutation: impl FnOnce(&Connection) -> Result<Value>,
) -> Result<Value> {
    let tx = conn.unchecked_transaction()?;
    let mut result = apply_due_time(&tx, params, mutation(&tx)?)?;
    if let (Some(id), Some(list)) = (
        result["task"]["id"].as_str(),
        result["task"]["list_id"].as_str(),
    ) {
        let date_only = google_tasks::use_date_only(&tx, id)?;
        queue(&tx, id, list, None)?;
        if date_only && let Some(task) = store::task(&tx, id)? {
            result["task"] = task_json(&task);
        }
    }
    tx.commit()?;
    Ok(result)
}
