//! CalDAV-backed task lists and their VTODO objects.

use std::collections::HashMap;

use anyhow::{Result, ensure};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::store::{self, CHANGED, CREATED, DavCalendar, DavObject};
use super::{ics, object_url};
use crate::store as db;

const LIST_PREFIX: &str = "caldav:";

pub fn calendar_id(list_id: &str) -> Option<&str> {
    list_id.strip_prefix(LIST_PREFIX)
}

pub(super) fn list_id(calendar: &str) -> String {
    format!("{LIST_PREFIX}{calendar}")
}

pub(super) fn is_task(id: &str) -> bool {
    id.starts_with("task-")
}

pub fn writable(conn: &Connection, list: &str) -> Result<bool> {
    let Some(id) = calendar_id(list) else {
        return Ok(true);
    };
    Ok(store::calendar(conn, id)?.is_some_and(|c| c.syncs_tasks() && !c.read_only))
}

pub fn require_writable(conn: &Connection, list: &str) -> Result<()> {
    ensure!(
        writable(conn, list)?,
        "This CalDAV task list is read-only or disconnected"
    );
    Ok(())
}

pub(super) fn ensure_list(conn: &Connection, calendar: &DavCalendar) -> Result<bool> {
    let id = list_id(&calendar.id);
    match db::task_list(conn, &id)? {
        None => {
            db::insert_task_list(conn, &id, &calendar.name)?;
            conn.execute(
                "UPDATE task_lists SET sort_order = COALESCE(
                    (SELECT json_extract(json, '$.task_list_order') FROM dav_calendars WHERE id = ?2),
                    sort_order) WHERE id = ?1",
                params![id, calendar.id],
            )?;
            Ok(true)
        }
        Some(list) if list.title != calendar.name => {
            db::rename_task_list(conn, &id, &calendar.name)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[derive(Deserialize, Serialize)]
struct LocalMetadata {
    uid: String,
    id: String,
    sort_order: i64,
    created_at: i64,
    account: String,
    thread_id: String,
    message_id: String,
}

/// Keep device-only fields across an unsubscribe. The next full listing
/// reattaches them only to the same VTODO.
pub(super) fn stash_local_metadata(conn: &Connection, calendar: &str) -> Result<()> {
    let mut metadata = local_metadata(conn, calendar)?;
    let id = list_id(calendar);
    for object in store::objects(conn, calendar)?
        .into_iter()
        .filter(|object| is_task(&object.local_id))
    {
        let Some(task) = db::task(conn, &object.local_id)? else {
            continue;
        };
        let uid = ics::uid_of(&original(conn, &task.id)?).unwrap_or_else(|| task.id.clone());
        metadata.insert(
            object.url,
            LocalMetadata {
                uid,
                id: task.id,
                sort_order: task.sort_order,
                created_at: task.created_at,
                account: task.account,
                thread_id: task.thread_id,
                message_id: task.message_id,
            },
        );
    }
    let order = db::task_list(conn, &id)?.map(|list| list.sort_order);
    conn.execute(
        "UPDATE dav_calendars SET json = json_set(json, '$.task_metadata', json(?2),
             '$.task_list_order', ?3) WHERE id = ?1",
        params![calendar, serde_json::to_string(&metadata)?, order],
    )?;
    Ok(())
}

fn local_metadata(conn: &Connection, calendar: &str) -> Result<HashMap<String, LocalMetadata>> {
    use rusqlite::OptionalExtension as _;
    let json: Option<String> = conn
        .query_row(
            "SELECT json_extract(json, '$.task_metadata') FROM dav_calendars WHERE id = ?1",
            [calendar],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(json
        .map(|json| serde_json::from_str(&json))
        .transpose()?
        .unwrap_or_default())
}

pub(super) fn clear_local_metadata(conn: &Connection, calendar: &str) -> Result<()> {
    conn.execute(
        "UPDATE dav_calendars SET json = json_remove(json, '$.task_metadata') WHERE id = ?1",
        [calendar],
    )?;
    Ok(())
}

pub(super) fn forget(conn: &Connection, calendar: &str, keep_unsent: bool) -> Result<()> {
    let id = list_id(calendar);
    if keep_unsent {
        let saved: i64 = conn.query_row(
            "SELECT count(*) FROM tasks WHERE list_id = ?1 AND id IN
               (SELECT local_id FROM dav_objects WHERE calendar = ?2 AND state IN ('created', 'changed'))",
            params![id, calendar],
            |row| row.get(0),
        )?;
        if saved > 0 {
            let local =
                db::insert_task_list(conn, &crate::tasks::new_id("tasklist"), "Recovered Tasks")?;
            conn.execute(
                "UPDATE tasks SET list_id = ?3, json = json_remove(json, '$.caldav_ics')
                 WHERE list_id = ?1 AND id IN
                   (SELECT local_id FROM dav_objects WHERE calendar = ?2 AND state IN ('created', 'changed'))",
                params![id, calendar, local.id],
            )?;
        }
    }
    conn.execute("DELETE FROM tasks WHERE list_id = ?1", [&id])?;
    conn.execute("DELETE FROM task_lists WHERE id = ?1", [&id])?;
    conn.execute(
        "DELETE FROM dav_objects WHERE calendar = ?1 AND local_id GLOB 'task-*'",
        [calendar],
    )?;
    Ok(())
}

pub(super) fn count(conn: &Connection, account: &str) -> Result<usize> {
    Ok(conn.query_row(
        "SELECT count(*) FROM task_lists l JOIN dav_calendars c ON l.id = 'caldav:' || c.id
         WHERE c.account = ?1",
        [account],
        |row| row.get::<_, i64>(0),
    )? as usize)
}

pub fn pending(conn: &Connection, calendar: &str) -> Result<usize> {
    Ok(conn.query_row(
        "SELECT count(*) FROM dav_objects WHERE calendar = ?1 AND state <> ''
         AND local_id GLOB 'task-*'",
        [calendar],
        |row| row.get::<_, i64>(0),
    )? as usize)
}

/// Cache the original calendar object to preserve properties Meron doesn't edit.
pub fn original(conn: &Connection, id: &str) -> Result<String> {
    Ok(conn.query_row(
        "SELECT COALESCE(json_extract(json, '$.caldav_ics'), '') FROM tasks WHERE id = ?1",
        [id],
        |row| row.get(0),
    )?)
}

fn save_original(conn: &Connection, id: &str, source: &str) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET json = json_set(json, '$.caldav_ics', ?2) WHERE id = ?1",
        params![id, source],
    )?;
    Ok(())
}

pub(super) fn outgoing(conn: &Connection, id: &str) -> Result<Option<String>> {
    let Some(task) = db::task(conn, id)? else {
        return Ok(None);
    };
    Ok(Some(ics::task_ics(&original(conn, id)?, &task)))
}

pub fn queue(conn: &Connection, id: &str, undo_ics: Option<&str>) -> Result<()> {
    let Some(task) = db::task(conn, id)? else {
        return Ok(());
    };
    let Some(calendar) = calendar_id(&task.list_id) else {
        return Ok(());
    };
    require_writable(conn, &task.list_id)?;
    if let Some(source) = undo_ics.filter(|source| ics::task_from_ics(source).is_some()) {
        save_original(conn, id, source)?;
    }
    if store::object_for_event(conn, id)?.is_some() {
        return store::mark_changed(conn, id);
    }
    let calendar = store::calendar(conn, calendar)?
        .ok_or_else(|| anyhow::anyhow!("CalDAV calendar disconnected"))?;
    let uid = undo_ics.and_then(ics::uid_of).unwrap_or(task.id);
    let url = object_url(&calendar.url, &uid)?;
    let prior = store::object(conn, &calendar.id, &url)?;
    ensure!(
        prior.as_ref().is_none_or(|row| row.local_id == id),
        "This task already exists on the server"
    );
    let state = if prior.is_some() { CHANGED } else { CREATED };
    store::save_object(
        conn,
        &DavObject {
            calendar: calendar.id,
            url,
            local_id: id.to_string(),
            etag: prior.map_or_else(String::new, |row| row.etag),
            state: state.into(),
            ..Default::default()
        },
    )
}

pub(super) fn apply(
    conn: &Connection,
    calendar: &DavCalendar,
    object: &super::Object,
    mut task: db::Task,
    local_id: &str,
) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;
    let now = db::now_unix();
    let mut returning = None;
    let id = if is_task(local_id) {
        local_id.to_string()
    } else {
        use rusqlite::OptionalExtension as _;
        let serialized: Option<String> = tx.query_row(
            "SELECT entry.value FROM dav_calendars c, json_each(c.json, '$.task_metadata') entry
             WHERE c.id = ?1 AND entry.key = ?2",
            params![calendar.id, object.url],
            |row| row.get(0),
        ).optional()?;
        if let Some(serialized) = serialized {
            let saved: LocalMetadata = serde_json::from_str(&serialized)?;
            if ics::uid_of(&object.ics).as_deref() == Some(saved.uid.as_str())
                && db::task(&tx, &saved.id)?.is_none()
            {
                returning = Some(saved);
            }
        }
        returning
            .as_ref()
            .map(|saved| saved.id.clone())
            .unwrap_or_else(|| crate::tasks::new_id("task"))
    };
    let old = db::task(&tx, &id)?;
    let changed = old.as_ref().is_none_or(|old| {
        old.title != task.title
            || old.notes != task.notes
            || old.due_at != task.due_at
            || old.due_has_time != task.due_has_time
            || old.completed_at != task.completed_at
    });
    if changed {
        if old.is_some() {
            db::update_task(
                &tx,
                &id,
                Some(&task.title),
                Some(&task.notes),
                Some(task.due_at),
                None,
            )?;
            db::set_task_due_time(&tx, &id, task.due_has_time)?;
            tx.execute(
                "UPDATE tasks SET completed_at = ?2 WHERE id = ?1",
                params![id, task.completed_at],
            )?;
        } else {
            task.id.clone_from(&id);
            task.list_id = list_id(&calendar.id);
            if let Some(saved) = returning {
                task.created_at = saved.created_at;
                task.sort_order = saved.sort_order;
                task.account = saved.account;
                task.thread_id = saved.thread_id;
                task.message_id = saved.message_id;
            } else {
                task.created_at = now;
                task.sort_order = tx.query_row(
                    "SELECT COALESCE(MIN(sort_order), 0) - 1 FROM tasks WHERE list_id = ?1",
                    [&task.list_id],
                    |row| row.get(0),
                )?;
            }
            db::restore_task(&tx, &task)?;
        }
    }
    if old.is_none() || original(&tx, &id)? != object.ics {
        save_original(&tx, &id, &object.ics)?;
    }
    store::save_object(
        &tx,
        &DavObject {
            calendar: calendar.id.clone(),
            url: object.url.clone(),
            etag: object.etag.clone(),
            local_id: id.clone(),
            ..Default::default()
        },
    )?;
    tx.commit()?;
    Ok(changed)
}

pub(super) fn remove(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.execute("DELETE FROM tasks WHERE id = ?1", [id])? > 0)
}

/// Keep the version and local identity until the deletion is acknowledged.
pub fn queue_delete(conn: &Connection, task: &db::Task) -> Result<()> {
    if calendar_id(&task.list_id).is_some() {
        conn.execute(
            "UPDATE dav_objects SET state = 'deleted' WHERE local_id = ?1",
            [&task.id],
        )?;
    }
    Ok(())
}

pub fn calendars_with_pending(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT calendar FROM dav_objects WHERE state <> '' AND local_id GLOB 'task-*'",
    )?;
    Ok(stmt
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_server_ics_can_replace_a_conflicting_local_edit() {
        let conn = Connection::open_in_memory().unwrap();
        db::run_migrations(&conn).unwrap();
        let calendar = DavCalendar {
            id: "cal-fixture".into(),
            ..Default::default()
        };
        ensure_list(&conn, &calendar).unwrap();
        let mut object = super::super::Object {
            url: "https://dav.example.test/tasks/sample.ics".into(),
            etag: "original".into(),
            ics: "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VTODO\r\nUID:sample\r\nSUMMARY:Server copy\r\nEND:VTODO\r\nEND:VCALENDAR\r\n".into(),
        };
        let task = ics::task_from_ics(&object.ics).unwrap();
        assert!(apply(&conn, &calendar, &object, task.clone(), "").unwrap());
        let id = store::object(&conn, &calendar.id, &object.url)
            .unwrap()
            .unwrap()
            .local_id;
        db::update_task(&conn, &id, Some("Conflicting local edit"), None, None, None).unwrap();
        object.etag = "rewritten".into();
        assert!(apply(&conn, &calendar, &object, task, &id).unwrap());
        assert_eq!(db::task(&conn, &id).unwrap().unwrap().title, "Server copy");
    }
}
