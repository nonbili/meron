//! Task lists and their items — the storage half of the optional Tasks feature.
//!
//! Local-only: nothing here talks to a server, so a row's only routes off the
//! device are the config backup and the user retyping it. Ordering is an
//! explicit `sort_order` rewritten in one transaction rather than a float
//! midpoint, because a list is small and a rewrite can never drift.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::now_unix;

/// A named list of tasks. Ordering is user-controlled, like the account rail.
///
/// Serializable because a delete hands the caller the rows it removed, and an
/// undo hands them straight back — identity and ordering included.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskList {
    pub id: String,
    pub title: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One task. `due_at` and `completed_at` use 0 rather than NULL for "unset", so
/// every read is a plain integer and ordering needs no COALESCE.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    pub id: String,
    pub list_id: String,
    pub title: String,
    pub notes: String,
    pub due_at: i64,
    pub completed_at: i64,
    pub sort_order: i64,
    /// Mail link, all three empty for a task the user typed from scratch.
    pub account: String,
    pub thread_id: String,
    pub message_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- Lists ------------------------------------------------------------------

pub fn task_lists(conn: &Connection) -> Result<Vec<TaskList>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, sort_order, created_at, updated_at
         FROM task_lists ORDER BY sort_order, created_at, id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TaskList {
                id: row.get(0)?,
                title: row.get(1)?,
                sort_order: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn task_list(conn: &Connection, id: &str) -> Result<Option<TaskList>> {
    Ok(conn
        .query_row(
            "SELECT id, title, sort_order, created_at, updated_at FROM task_lists WHERE id = ?1",
            params![id],
            |row| {
                Ok(TaskList {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    sort_order: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            },
        )
        .optional()?)
}

/// Append a list at the end of the rail. `sort_order` is one past the current
/// maximum, so a fresh list never lands above an existing one.
pub fn insert_task_list(conn: &Connection, id: &str, title: &str) -> Result<TaskList> {
    let now = now_unix();
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM task_lists",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO task_lists(id, title, sort_order, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?4)",
        params![id, title.trim(), next, now],
    )?;
    Ok(TaskList {
        id: id.to_string(),
        title: title.trim().to_string(),
        sort_order: next,
        created_at: now,
        updated_at: now,
    })
}

pub fn rename_task_list(conn: &Connection, id: &str, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE task_lists SET title = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, title.trim(), now_unix()],
    )?;
    Ok(())
}

/// Drop a list and everything in it. There are no foreign keys anywhere in this
/// schema, so the cascade is written out; both statements share one transaction
/// so a crash can't leave orphaned tasks behind.
pub fn delete_task_list(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM tasks WHERE list_id = ?1", params![id])?;
    tx.execute("DELETE FROM task_lists WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(())
}

/// Put a list back exactly as it was, id included.
///
/// Undo has to restore identity, not just content: a task carries its list id,
/// and anything else holding a reference to the list would otherwise point at
/// nothing. `INSERT OR IGNORE` because a second undo of the same delete must
/// not fail — it has simply already happened.
pub fn restore_task_list(conn: &Connection, list: &TaskList) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO task_lists(id, title, sort_order, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![
            list.id,
            list.title,
            list.sort_order,
            list.created_at,
            now_unix()
        ],
    )?;
    Ok(())
}

/// Put a task back exactly as it was — same id, same position in the list, same
/// completion time — so an undo is indistinguishable from never having deleted.
pub fn restore_task(conn: &Connection, task: &Task) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO tasks(id, list_id, title, notes, due_at, completed_at, sort_order,
                                     account, thread_id, message_id, json, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '{}', ?11, ?12)",
        params![
            task.id,
            task.list_id,
            task.title,
            task.notes,
            task.due_at,
            task.completed_at,
            task.sort_order,
            task.account,
            task.thread_id,
            task.message_id,
            task.created_at,
            now_unix(),
        ],
    )?;
    Ok(())
}

/// Rewrite list order from the ids the client hands back. Ids it doesn't
/// mention keep their relative order after the ones it does, so a reorder racing
/// a create can't silently drop the new list.
pub fn reorder_task_lists(conn: &mut Connection, ordered_ids: &[String]) -> Result<()> {
    let tx = conn.transaction()?;
    let now = now_unix();
    for (index, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE task_lists SET sort_order = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, index as i64, now],
        )?;
    }
    // Rows the client didn't mention are packed immediately after the ones it
    // did, keeping their relative order. Adding an offset to what they already
    // had would not do: their old values overlap the 0..n band just assigned,
    // and two rows sharing a sort_order order arbitrarily.
    tx.execute(
        "UPDATE task_lists SET sort_order = ?1 + (
             SELECT rank FROM (
               SELECT id, ROW_NUMBER() OVER (ORDER BY sort_order, created_at, id) - 1 AS rank
               FROM task_lists WHERE id NOT IN (SELECT value FROM json_each(?2))
             ) ranked WHERE ranked.id = task_lists.id
           )
         WHERE id NOT IN (SELECT value FROM json_each(?2))",
        params![
            ordered_ids.len() as i64,
            serde_json::to_string(ordered_ids)?
        ],
    )?;
    tx.commit()?;
    Ok(())
}

// ---- Tasks ------------------------------------------------------------------

fn read_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        list_id: row.get(1)?,
        title: row.get(2)?,
        notes: row.get(3)?,
        due_at: row.get(4)?,
        completed_at: row.get(5)?,
        sort_order: row.get(6)?,
        account: row.get(7)?,
        thread_id: row.get(8)?,
        message_id: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const TASK_COLUMNS: &str = "id, list_id, title, notes, due_at, completed_at, sort_order,
     account, thread_id, message_id, created_at, updated_at";

/// Items in a list, completed ones last and most-recently-completed first —
/// what Google Tasks shows under "Completed" and what a user scanning for what
/// they just ticked expects.
pub fn tasks_for_list(
    conn: &Connection,
    list_id: &str,
    include_completed: bool,
) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks
         WHERE list_id = ?1 {}
         ORDER BY (completed_at > 0), CASE WHEN completed_at > 0 THEN -completed_at ELSE sort_order END, created_at, id",
        if include_completed {
            ""
        } else {
            "AND completed_at = 0"
        }
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![list_id], read_task)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn task(conn: &Connection, id: &str) -> Result<Option<Task>> {
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
    Ok(conn.query_row(&sql, params![id], read_task).optional()?)
}

/// Every task linked to a thread, so a mail surface can show "already in Tasks"
/// without loading a list.
pub fn tasks_for_thread(conn: &Connection, thread_id: &str) -> Result<Vec<Task>> {
    let sql =
        format!("SELECT {TASK_COLUMNS} FROM tasks WHERE thread_id = ?1 ORDER BY created_at, id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![thread_id], read_task)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// A new task, placed at the top of its list. Adding to the top is what every
/// task app does: the thing you just thought of is the thing you're looking at.
#[allow(clippy::too_many_arguments)]
pub fn insert_task(
    conn: &Connection,
    id: &str,
    list_id: &str,
    title: &str,
    notes: &str,
    due_at: i64,
    account: &str,
    thread_id: &str,
    message_id: &str,
) -> Result<Task> {
    let now = now_unix();
    let top: i64 = conn.query_row(
        "SELECT COALESCE(MIN(sort_order), 0) - 1 FROM tasks WHERE list_id = ?1",
        params![list_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO tasks(id, list_id, title, notes, due_at, completed_at, sort_order,
                           account, thread_id, message_id, json, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?9, '{}', ?10, ?10)",
        params![
            id,
            list_id,
            title.trim(),
            notes,
            due_at.max(0),
            top,
            account,
            thread_id,
            message_id,
            now
        ],
    )?;
    Ok(Task {
        id: id.to_string(),
        list_id: list_id.to_string(),
        title: title.trim().to_string(),
        notes: notes.to_string(),
        due_at: due_at.max(0),
        completed_at: 0,
        sort_order: top,
        account: account.to_string(),
        thread_id: thread_id.to_string(),
        message_id: message_id.to_string(),
        created_at: now,
        updated_at: now,
    })
}

/// Patch a task. Every field is optional: `None` means "leave it alone", which
/// is what lets the editor save a title without knowing the notes.
pub fn update_task(
    conn: &Connection,
    id: &str,
    title: Option<&str>,
    notes: Option<&str>,
    due_at: Option<i64>,
    list_id: Option<&str>,
) -> Result<()> {
    let now = now_unix();
    if let Some(title) = title {
        conn.execute(
            "UPDATE tasks SET title = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, title.trim(), now],
        )?;
    }
    if let Some(notes) = notes {
        conn.execute(
            "UPDATE tasks SET notes = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, notes, now],
        )?;
    }
    if let Some(due_at) = due_at {
        conn.execute(
            "UPDATE tasks SET due_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, due_at.max(0), now],
        )?;
    }
    if let Some(list_id) = list_id {
        // Moving lists lands the task at the top of the destination, the same
        // place a fresh task goes, so it isn't buried on arrival.
        let top: i64 = conn.query_row(
            "SELECT COALESCE(MIN(sort_order), 0) - 1 FROM tasks WHERE list_id = ?1",
            params![list_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "UPDATE tasks SET list_id = ?2, sort_order = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, list_id, top, now],
        )?;
    }
    Ok(())
}

/// Tick or untick. The completion time doubles as the flag, so the completed
/// section can order by when it was ticked without a second column.
pub fn set_task_done(conn: &Connection, id: &str, done: bool) -> Result<()> {
    let now = now_unix();
    conn.execute(
        "UPDATE tasks SET completed_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, if done { now } else { 0 }, now],
    )?;
    Ok(())
}

pub fn delete_task(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    Ok(())
}

/// Rewrite item order within one list, same contract as [`reorder_task_lists`].
pub fn reorder_tasks(conn: &mut Connection, list_id: &str, ordered_ids: &[String]) -> Result<()> {
    let tx = conn.transaction()?;
    let now = now_unix();
    for (index, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE tasks SET sort_order = ?2, updated_at = ?3 WHERE id = ?1 AND list_id = ?4",
            params![id, index as i64, now, list_id],
        )?;
    }
    tx.execute(
        "UPDATE tasks SET sort_order = ?1 + (
             SELECT rank FROM (
               SELECT id, ROW_NUMBER() OVER (ORDER BY sort_order, created_at, id) - 1 AS rank
               FROM tasks WHERE list_id = ?2 AND id NOT IN (SELECT value FROM json_each(?3))
             ) ranked WHERE ranked.id = tasks.id
           )
         WHERE list_id = ?2 AND id NOT IN (SELECT value FROM json_each(?3))",
        params![
            ordered_ids.len() as i64,
            list_id,
            serde_json::to_string(ordered_ids)?
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// Drop every ticked task in a list, returning how many went, for the toast.
pub fn clear_completed(conn: &Connection, list_id: &str) -> Result<u32> {
    let removed = conn.execute(
        "DELETE FROM tasks WHERE list_id = ?1 AND completed_at > 0",
        params![list_id],
    )?;
    Ok(removed as u32)
}
