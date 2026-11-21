//! Google Tasks REST sync. Remote IDs are scoped to a provider list. The task
//! row retains its Meron ID, ordering and mail link across refreshes.

mod client;
mod store;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Result, anyhow, bail, ensure};
use rusqlite::Connection;
use serde_json::{Value, json};

use self::client::{Client, Item};
use self::store::{Mapping, RemoteList};
use crate::engine::Engine;
use crate::google_grant::{self, Connected};
use crate::oauth::GOOGLE_TASKS_SCOPE;
use crate::store as db;

pub use store::{
    has_account, is_list, pending, pending_emails, queue, queue_delete, require_writable,
    sync_intervals,
};

fn lock(db: &Mutex<Connection>) -> Result<std::sync::MutexGuard<'_, Connection>> {
    db.lock().map_err(|_| anyhow!("store lock poisoned"))
}

pub fn dispatch(engine: &Engine, method: &str, params: &Value) -> Result<Value> {
    let email = params["email"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match method {
        "googleTasks.accounts" => store::accounts_payload(&*lock(&engine.db)?),
        "googleTasks.add" => {
            ensure!(
                email.contains('@'),
                "Google sign-in did not return an email address"
            );
            connect(engine, &email, params)
        }
        "googleTasks.sync" => {
            sync(engine, (!email.is_empty()).then_some(email.as_str()), false)?;
            store::accounts_payload(&*lock(&engine.db)?)
        }
        "googleTasks.remove" => {
            let _operation = engine
                .google_tasks_operation
                .lock()
                .map_err(|_| anyhow!("Google Tasks lock poisoned"))?;
            let conn = lock(&engine.db)?;
            let discard = params["discard_unsent"].as_bool().unwrap_or(false);
            let unsent = store::unsent(&conn, &email)?;
            if unsent > 0 && !discard {
                return Ok(json!({ "unsent": unsent }));
            }
            store::remove_account(&conn, &email)?;
            google_grant::release(engine, &conn, &email)?;
            store::accounts_payload(&conn)
        }
        _ => bail!("unknown Google Tasks command: {method}"),
    }
}

fn connect(engine: &Engine, email: &str, params: &Value) -> Result<Value> {
    let pending = google_grant::pending(engine, email, GOOGLE_TASKS_SCOPE);
    let result = (|| {
        let access = match google_grant::connect(engine, email, GOOGLE_TASKS_SCOPE, params)? {
            Connected::SignIn(scopes) => {
                return Ok(json!({ "needs_sign_in": true, "scopes": scopes }));
            }
            Connected::Token(access) => access,
        };
        let _operation = engine
            .google_tasks_operation
            .lock()
            .map_err(|_| anyhow!("Google Tasks lock poisoned"))?;
        let client = Client::new(&access)?;
        let lists = match client.lists() {
            Ok(lists) => lists,
            Err(err) => {
                if let Some(scopes) =
                    google_grant::sign_in_after(engine, email, GOOGLE_TASKS_SCOPE, params, &err)?
                {
                    return Ok(json!({ "needs_sign_in": true, "scopes": scopes }));
                }
                return Err(err);
            }
        };
        {
            let conn = lock(&engine.db)?;
            store::save_account(&conn, email)?;
            // Switched off while the sign-in was open: keep queued edits for
            // when Tasks is back, as a paused sync would.
            if !db::tasks_enabled(&conn)? {
                return store::accounts_payload(&conn);
            }
        }
        let outcome = sync_lists(&engine.db, &client, email, lists);
        let conn = lock(&engine.db)?;
        store::record_sync(&conn, email, outcome.as_ref().err())?;
        store::accounts_payload(&conn)
    })();
    google_grant::abandon(pending)?;
    result
}

/// Run queued writes and refresh tasks. A background trigger skips a sync in
/// progress. The next poll picks up any edits made while that sync was running.
pub fn sync(engine: &Engine, email: Option<&str>, background: bool) -> Result<bool> {
    let _operation = if background {
        match engine.google_tasks_operation.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(false),
            Err(err) => bail!("{err}"),
        }
    } else {
        engine
            .google_tasks_operation
            .lock()
            .map_err(|_| anyhow!("Google Tasks lock poisoned"))?
    };
    let emails = store::emails(&*lock(&engine.db)?)?;
    let mut changed = false;
    for email in emails
        .iter()
        .filter(|known| email.is_none_or(|email| email == *known))
    {
        let result = (|| {
            let access = google_grant::access_token(engine, email, GOOGLE_TASKS_SCOPE)?
                .ok_or_else(|| anyhow!("Reconnect Google Tasks to resume syncing"))?;
            let client = Client::new(&access)?;
            let lists = client.lists()?;
            sync_lists(&engine.db, &client, email, lists)
        })();
        let conn = lock(&engine.db)?;
        store::record_sync(&conn, email, result.as_ref().err())?;
        // A run may have applied changes before another list failed.
        changed |= result.unwrap_or(true);
    }
    Ok(changed)
}

fn sync_lists(
    db: &Mutex<Connection>,
    client: &Client,
    email: &str,
    lists: Vec<client::List>,
) -> Result<bool> {
    let (lists, mut changed) = store::reconcile_lists(&*lock(db)?, email, &lists)?;
    let mut first_error = None;
    for list in lists {
        match sync_list(db, client, &list) {
            Ok(list_changed) => changed |= list_changed,
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }
    first_error.map_or(Ok(changed), Err)
}

fn sync_list(db: &Mutex<Connection>, client: &Client, list: &RemoteList) -> Result<bool> {
    let mut remote: HashMap<String, Item> = HashMap::new();
    for item in client
        .items(&list.remote_id)?
        .into_iter()
        .filter(|item| !item.deleted)
    {
        ensure!(
            !item.id.is_empty() && !item.etag.is_empty(),
            "Google Tasks returned a task without an ID or version"
        );
        ensure!(
            remote.insert(item.id.clone(), item).is_none(),
            "Google Tasks listed a task twice"
        );
    }
    let pending = store::mappings(&*lock(db)?, &list.id)?;
    let mut first_error = None;
    let mut pushed = false;
    for mapping in pending.iter().filter(|mapping| !mapping.state.is_empty()) {
        match push(db, client, list, mapping, &mut remote) {
            Ok(sent) => pushed |= sent,
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }
    let changed = store::apply_snapshot(&*lock(db)?, list, remote)?;
    first_error.map_or(Ok(changed || pushed), Err)
}

fn push(
    db: &Mutex<Connection>,
    client: &Client,
    list: &RemoteList,
    seen: &Mapping,
    remote: &mut HashMap<String, Item>,
) -> Result<bool> {
    let (task, mapping) = {
        let conn = lock(db)?;
        let Some(current) = store::mapping(&conn, &seen.id)?.filter(|mapping| mapping == seen)
        else {
            return Ok(false);
        };
        let task = db::task(&conn, &seen.id)?;
        if current.state == store::CREATED && task.is_some() {
            // An insert has no API idempotency key. Persist the attempt before
            // sending so an unknown outcome is never automatically duplicated.
            store::set_state(&conn, &seen.id, store::CREATING)?;
        }
        (task, current)
    };
    let task_id = &mapping.id;
    let Some(remote_id) = &mapping.remote_id else {
        return match (mapping.state.as_str(), task) {
            (store::CREATED, Some(task)) => match client.create(&list.remote_id, &task) {
                Ok(item) => {
                    validate_item(&item, None)?;
                    store::write_complete(&*lock(db)?, list, &mapping, &task, &item)?;
                    remote.insert(item.id.clone(), item);
                    Ok(true)
                }
                Err(err) => {
                    if err
                        .downcast_ref::<client::HttpStatus>()
                        .is_some_and(|s| (400..500).contains(&s.0))
                        || err.is::<crate::google_grant::Rejected>()
                        || err
                            .downcast_ref::<ureq::Error>()
                            .is_some_and(crate::caldav::unreachable)
                    {
                        let conn = lock(db)?;
                        let current = store::mapping(&conn, task_id)?;
                        match current.as_ref().map(|m| m.state.as_str()) {
                            Some(store::CREATING) => {
                                store::set_state(&conn, task_id, store::CREATED)?
                            }
                            Some(store::DELETED_WHILE_CREATING) => {
                                store::delete_mapping(&conn, task_id)?
                            }
                            _ => {}
                        }
                    }
                    Err(err)
                }
            },
            (store::CREATING, Some(task)) => bail!(
                "Google Tasks creation was interrupted for \"{}\". Check Google Tasks for this item, then delete the local copy. If Google does not have it, add it again in Meron.",
                task.title
            ),
            (_, None) => {
                let conn = lock(db)?;
                if db::task(&conn, task_id)?.is_none() {
                    store::delete_mapping(&conn, task_id)?;
                }
                Ok(false)
            }
            _ => Ok(false),
        };
    };

    // The server's copy wins if the saved version changed or was removed.
    if remote
        .get(remote_id)
        .is_none_or(|item| item.etag != mapping.etag)
    {
        store::drop_conflict(&*lock(db)?, &mapping, task.as_ref())?;
        return Ok(false);
    }
    let write = match (mapping.state.as_str(), task.as_ref()) {
        (store::CHANGED, Some(task)) => client
            .update(&list.remote_id, remote_id, &mapping.etag, task)
            .map(Some),
        (store::DELETED, None) => client
            .delete(&list.remote_id, remote_id, &mapping.etag)
            .map(|()| None),
        _ => return Ok(false),
    };
    match write {
        Ok(Some(item)) => {
            validate_item(&item, Some(remote_id))?;
            let task = task
                .as_ref()
                .ok_or_else(|| anyhow!("missing task for Google Tasks update"))?;
            store::write_complete(&*lock(db)?, list, &mapping, task, &item)?;
            remote.insert(item.id.clone(), item);
        }
        Ok(None) => {
            store::delete_complete(&*lock(db)?, &mapping)?;
            remote.remove(remote_id);
        }
        Err(err) if client::status(&err, 404) || client::status(&err, 410) => {
            store::drop_conflict(&*lock(db)?, &mapping, task.as_ref())?;
            remote.remove(remote_id);
        }
        // Keep the edit until a full fresh listing can reconcile it.
        Err(err) if client::status(&err, 412) => {}
        Err(err) => return Err(err),
    }
    Ok(true)
}

fn validate_item(item: &Item, id: Option<&str>) -> Result<()> {
    ensure!(
        !item.id.is_empty()
            && !item.etag.is_empty()
            && !item.deleted
            && id.is_none_or(|id| item.id == id),
        "Unexpected Google Tasks write response"
    );
    Ok(())
}

/// Google interprets its due field as a calendar day even when the server
/// representation carries a UTC timestamp.
pub fn use_date_only(conn: &Connection, task_id: &str) -> Result<bool> {
    let Some(task) = db::task(conn, task_id)?.filter(|task| is_list(&task.list_id)) else {
        return Ok(false);
    };
    if task.due_has_time {
        let due = chrono::DateTime::from_timestamp(task.due_at, 0)
            .and_then(|at| {
                client::date_at_local_midnight(at.with_timezone(&chrono::Local).date_naive())
            })
            .unwrap_or_default();
        db::update_task(conn, task_id, None, None, Some(due), None)?;
        db::set_task_due_time(conn, task_id, false)?;
    }
    Ok(true)
}
