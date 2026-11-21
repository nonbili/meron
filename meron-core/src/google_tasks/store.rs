use std::collections::{HashMap, HashSet};

use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::{Value, json};

use crate::store::{self as db, Task};

use super::client::{Item, List, payload};

pub(super) const CREATED: &str = "created";
pub(super) const CHANGED: &str = "changed";
pub(super) const CREATING: &str = "creating";
pub(super) const DELETED: &str = "deleted";
pub(super) const DELETED_WHILE_CREATING: &str = "deleted_while_creating";

const LIST_PREFIX: &str = "google-tasklist-";

pub fn is_list(id: &str) -> bool {
    id.starts_with(LIST_PREFIX)
}

pub fn require_writable(conn: &Connection, list: &str) -> Result<()> {
    if is_list(list) {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM google_task_lists WHERE id = ?1)",
            [list],
            |row| row.get(0),
        )?;
        ensure!(exists, "This Google Tasks list is disconnected");
    }
    Ok(())
}

pub fn has_account(conn: &Connection, email: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM google_task_accounts WHERE email = ?1)",
        [email],
        |row| row.get(0),
    )?)
}

pub(super) fn emails(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT email FROM google_task_accounts ORDER BY email")?;
    Ok(stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

pub(super) fn save_account(conn: &Connection, email: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO google_task_accounts (email) VALUES (?1)",
        [email],
    )?;
    Ok(())
}

pub(super) fn accounts_payload(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT email, last_error, last_sync_at FROM google_task_accounts ORDER BY email",
    )?;
    let accounts = stmt
        .query_map([], |row| {
            Ok(json!({
                "email": row.get::<_, String>(0)?,
                "last_error": row.get::<_, String>(1)?,
                "last_sync_at": row.get::<_, i64>(2)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({ "accounts": accounts }))
}

/// Each account with the minutes between its background syncs: those of the
/// Google calendar account on the same address (0 = only when asked), or
/// the calendar default when there is none.
pub fn sync_intervals(conn: &Connection) -> Result<Vec<(String, i64)>> {
    let calendars = crate::caldav::store::accounts(conn)?;
    Ok(emails(conn)?
        .into_iter()
        .map(|email| {
            let minutes = calendars
                .iter()
                .find(|account| account.google && account.username.eq_ignore_ascii_case(&email))
                .map_or(crate::caldav::store::DEFAULT_SYNC_INTERVAL, |account| {
                    account.sync_interval
                });
            (email, minutes)
        })
        .collect())
}

pub(super) fn record_sync(
    conn: &Connection,
    email: &str,
    err: Option<&anyhow::Error>,
) -> Result<()> {
    let error = err.map(|err| format!("{err:#}")).unwrap_or_default();
    conn.execute(
        "UPDATE google_task_accounts SET last_error = ?2,
         last_sync_at = CASE WHEN ?2 = '' THEN ?3 ELSE last_sync_at END WHERE email = ?1",
        params![email, error, db::now_unix()],
    )?;
    Ok(())
}

/// The accounts with edits waiting to be sent.
pub fn pending_emails(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT l.email FROM google_task_items i JOIN google_task_lists l ON l.id = i.list_id
         WHERE i.state IN ('created', 'changed', 'deleted', 'deleted_while_creating')
         ORDER BY l.email",
    )?;
    Ok(stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn pending(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM google_task_items WHERE state IN ('created', 'changed', 'deleted', 'deleted_while_creating'))",
        [], |row| row.get(0),
    )?)
}

pub(super) fn unsent(conn: &Connection, email: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM google_task_items i JOIN google_task_lists l ON l.id = i.list_id
         WHERE l.email = ?1 AND i.state <> ''",
        [email],
        |row| row.get(0),
    )?)
}

pub(super) fn remove_account(conn: &Connection, email: &str) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for list in lists(&tx, email)? {
        forget_list(&tx, &list.id, false)?;
    }
    tx.execute("DELETE FROM google_task_accounts WHERE email = ?1", [email])?;
    tx.commit()?;
    Ok(())
}

#[derive(Clone, Debug)]
pub(super) struct RemoteList {
    pub id: String,
    pub remote_id: String,
}

fn lists(conn: &Connection, email: &str) -> Result<Vec<RemoteList>> {
    let mut stmt = conn.prepare("SELECT id, remote_id FROM google_task_lists WHERE email = ?1")?;
    Ok(stmt
        .query_map([email], |row| {
            Ok(RemoteList {
                id: row.get(0)?,
                remote_id: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

pub(super) fn reconcile_lists(
    conn: &Connection,
    email: &str,
    remote: &[List],
) -> Result<(Vec<RemoteList>, bool)> {
    let tx = conn.unchecked_transaction()?;
    let mut known: HashMap<_, _> = lists(&tx, email)?
        .into_iter()
        .map(|l| (l.remote_id, l.id))
        .collect();
    let mut seen = HashSet::new();
    let mut changed = false;
    for remote in remote {
        ensure!(
            !remote.id.is_empty() && seen.insert(&remote.id),
            "Google Tasks returned duplicate or invalid list IDs"
        );
        let title = if remote.title.trim().is_empty() {
            "My Tasks"
        } else {
            &remote.title
        };
        match known.remove(&remote.id) {
            Some(id) => {
                if db::task_list(&tx, &id)?.is_some_and(|list| list.title != title) {
                    db::rename_task_list(&tx, &id, title)?;
                    changed = true;
                }
            }
            None => {
                let id = crate::tasks::new_id("google-tasklist");
                db::insert_task_list(&tx, &id, title)?;
                tx.execute(
                    "INSERT INTO google_task_lists (id, email, remote_id) VALUES (?1, ?2, ?3)",
                    params![id, email, remote.id],
                )?;
                changed = true;
            }
        }
    }
    for id in known.values() {
        forget_list(&tx, id, true)?;
        changed = true;
    }
    let lists = lists(&tx, email)?;
    tx.commit()?;
    Ok((lists, changed))
}

fn forget_list(conn: &Connection, id: &str, recover: bool) -> Result<()> {
    if recover {
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM tasks t JOIN google_task_items i USING(id)
             WHERE t.list_id = ?1 AND i.state <> ''",
            [id],
            |row| row.get(0),
        )?;
        if count > 0 {
            let local =
                db::insert_task_list(conn, &crate::tasks::new_id("tasklist"), "Recovered Tasks")?;
            conn.execute(
                "UPDATE tasks SET list_id = ?2 WHERE list_id = ?1 AND id IN
                 (SELECT id FROM google_task_items WHERE list_id = ?1 AND state <> '')",
                params![id, local.id],
            )?;
        }
    }
    conn.execute("DELETE FROM google_task_items WHERE list_id = ?1", [id])?;
    conn.execute("DELETE FROM google_task_lists WHERE id = ?1", [id])?;
    conn.execute("DELETE FROM tasks WHERE list_id = ?1", [id])?;
    conn.execute("DELETE FROM task_lists WHERE id = ?1", [id])?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Mapping {
    pub id: String,
    pub remote_id: Option<String>,
    pub etag: String,
    pub state: String,
}

fn read_mapping(row: &Row<'_>) -> rusqlite::Result<Mapping> {
    Ok(Mapping {
        id: row.get(0)?,
        remote_id: row.get(1)?,
        etag: row.get(2)?,
        state: row.get(3)?,
    })
}

pub(super) fn mappings(conn: &Connection, list: &str) -> Result<Vec<Mapping>> {
    let mut stmt = conn.prepare(
        "SELECT id, remote_id, etag, state FROM google_task_items WHERE list_id = ?1 ORDER BY id",
    )?;
    Ok(stmt
        .query_map([list], read_mapping)?
        .collect::<rusqlite::Result<_>>()?)
}

pub(super) fn mapping(conn: &Connection, id: &str) -> Result<Option<Mapping>> {
    Ok(conn
        .query_row(
            "SELECT id, remote_id, etag, state FROM google_task_items WHERE id = ?1",
            [id],
            read_mapping,
        )
        .optional()?)
}

pub fn queue(conn: &Connection, id: &str, list: &str) -> Result<()> {
    if is_list(list) {
        conn.execute(
            "INSERT INTO google_task_items (id, list_id, state) VALUES (?1, ?2, 'created')
             ON CONFLICT(id) DO UPDATE SET state = CASE
               WHEN state IN ('created', 'creating') THEN state
               WHEN state = 'deleted_while_creating' THEN 'creating'
               WHEN remote_id IS NULL THEN 'created' ELSE 'changed' END",
            params![id, list],
        )?;
    }
    Ok(())
}

pub fn queue_delete(conn: &Connection, id: &str, list: &str) -> Result<()> {
    if is_list(list) {
        conn.execute(
            "DELETE FROM google_task_items WHERE id = ?1 AND state = 'created'",
            [id],
        )?;
        conn.execute(
            "UPDATE google_task_items SET state =
               CASE WHEN state = 'creating' THEN 'deleted_while_creating' ELSE 'deleted' END
             WHERE id = ?1",
            [id],
        )?;
    }
    Ok(())
}

pub(super) fn set_state(conn: &Connection, id: &str, state: &str) -> Result<()> {
    conn.execute(
        "UPDATE google_task_items SET state = ?2 WHERE id = ?1",
        params![id, state],
    )?;
    Ok(())
}

pub(super) fn delete_mapping(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM google_task_items WHERE id = ?1", [id])?;
    Ok(())
}

fn same_task(conn: &Connection, id: &str, original: Option<&Task>) -> Result<bool> {
    let task = db::task(conn, id)?;
    Ok(task.as_ref().map(payload) == original.map(payload))
}

pub(super) fn write_complete(
    conn: &Connection,
    list: &RemoteList,
    original: &Mapping,
    task: &Task,
    item: &Item,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let current = db::task(&tx, &original.id)?;
    let state = match current.as_ref() {
        None => DELETED,
        Some(current) if payload(current) != payload(task) => CHANGED,
        Some(_) => "",
    };
    tx.execute(
        "UPDATE google_task_items SET remote_id = ?2, etag = ?3, state = ?4 WHERE id = ?1",
        params![original.id, item.id, item.etag, state],
    )?;
    if state.is_empty() {
        save_item(&tx, &list.id, &original.id, item)?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn delete_complete(conn: &Connection, original: &Mapping) -> Result<()> {
    if db::task(conn, &original.id)?.is_some() {
        conn.execute(
            "UPDATE google_task_items SET remote_id = NULL, etag = '', state = 'created' WHERE id = ?1",
            [&original.id],
        )?;
    } else {
        delete_mapping(conn, &original.id)?;
    }
    Ok(())
}

pub(super) fn drop_conflict(
    conn: &Connection,
    original: &Mapping,
    task: Option<&Task>,
) -> Result<()> {
    if same_task(conn, &original.id, task)?
        && mapping(conn, &original.id)?.as_ref() == Some(original)
    {
        set_state(conn, &original.id, "")?;
    }
    Ok(())
}

pub(super) fn apply_snapshot(
    conn: &Connection,
    list: &RemoteList,
    mut items: HashMap<String, Item>,
) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let mut changed = false;
    for mapping in mappings(&tx, &list.id)? {
        let Some(remote) = &mapping.remote_id else {
            continue;
        };
        let item = items.remove(remote);
        if !mapping.state.is_empty() {
            continue;
        }
        match item {
            Some(item) if item.etag != mapping.etag => {
                save_item(&tx, &list.id, &mapping.id, &item)?;
                changed = true;
            }
            None => {
                db::delete_task(&tx, &mapping.id)?;
                delete_mapping(&tx, &mapping.id)?;
                changed = true;
            }
            Some(_) => {}
        }
    }
    let mut new: Vec<_> = items.into_values().collect();
    new.sort_by(|a, b| a.id.cmp(&b.id));
    for item in new {
        save_item(&tx, &list.id, &crate::tasks::new_id("task"), &item)?;
        changed = true;
    }
    tx.commit()?;
    Ok(changed)
}

fn save_item(conn: &Connection, list: &str, id: &str, item: &Item) -> Result<()> {
    let title = if item.title.trim().is_empty() {
        "Untitled"
    } else {
        &item.title
    };
    let due = item.due_at();
    if db::task(conn, id)?.is_some() {
        db::update_task(conn, id, Some(title), Some(&item.notes), Some(due), None)?;
    } else {
        db::insert_task(conn, id, list, title, &item.notes, due, "", "", "")?;
    }
    db::set_task_due_time(conn, id, false)?;
    conn.execute(
        "UPDATE tasks SET completed_at = ?2 WHERE id = ?1",
        params![id, item.completed_at()],
    )?;
    conn.execute(
        "INSERT INTO google_task_items (id, list_id, remote_id, etag) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET etag = excluded.etag",
        params![id, list, item.id, item.etag],
    )?;
    Ok(())
}
