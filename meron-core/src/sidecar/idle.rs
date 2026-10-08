use anyhow::Context as _;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::{imap, mail_model, parse, store};

use crate::{Writer, emit};

pub(crate) const IDLE_LIMIT: u32 = 50;

/// Longest a notification waits on the body fetch its snippets need. Past this
/// the event goes out with whatever bodies are cached: a late notification is
/// worse than one showing subjects alone, and the general prefetch fills the
/// rest in anyway.
pub(crate) const NOTIFY_PREVIEW_TIMEOUT: Duration = Duration::from_secs(8);

/// `mail.newMessages` detail for a batch of arrivals, with the arrivals' bodies
/// fetched first so the notification can show the mail itself.
pub(crate) async fn new_messages_detail(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    headers: &[imap::MessageHeader],
) -> Option<Value> {
    let uids: Vec<u32> = headers
        .iter()
        .take(mail_model::NEW_MESSAGES_DETAIL_MAX)
        .map(|header| header.uid)
        .collect();
    let fetch = fetch_bodies_for_uids(engine, account, folder, &uids, parse::media_root());
    match tokio::time::timeout(NOTIFY_PREVIEW_TIMEOUT, fetch).await {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => eprintln!("meron-core: notification bodies for {account}: {err:#}"),
        Err(_) => eprintln!("meron-core: notification bodies for {account}: timed out"),
    }
    let account_name = account_label(engine, account);
    let muted = engine.is_muted(account);
    let db = engine.db.lock().unwrap();
    mail_model::new_messages_detail(&db, account, &account_name, folder, muted, headers)
}

/// Friendly display name or email address of an account for user-facing notifications.
pub(crate) fn account_label(engine: &Arc<Engine>, account: &str) -> String {
    let db = engine.db.lock().unwrap();
    store::account_label(&db, account)
}

pub(crate) fn watch_key(account: &str, folder: &str) -> String {
    format!("{account}\n{folder}")
}

pub(crate) fn start_idle_watch(
    engine: Arc<Engine>,
    out: Writer,
    account: String,
    folder: String,
) -> bool {
    let key = watch_key(&account, &folder);
    {
        let mut watched = engine.watched.lock().unwrap();
        if watched.contains(&key) {
            return false;
        }
        watched.insert(key);
    }
    tokio::spawn(idle_watch(engine, out, account, folder));
    true
}

/// Watches a view asked for (`watch.start`: a kanban column showing the
/// folder), as opposed to ones kept for notifications. A watch runs while
/// either wants it, so each side checks the other before stopping one.
static VIEW_WATCHES: std::sync::Mutex<std::collections::BTreeSet<String>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Record whether a view wants `folder` watched.
pub(crate) fn set_view_watch(account: &str, folder: &str, wanted: bool) {
    let key = watch_key(account, folder);
    let mut views = VIEW_WATCHES.lock().unwrap();
    if wanted {
        views.insert(key);
    } else {
        views.remove(&key);
    }
}

pub(crate) fn view_watches(account: &str, folder: &str) -> bool {
    VIEW_WATCHES
        .lock()
        .unwrap()
        .contains(&watch_key(account, folder))
}

/// Stop a running watch and wake it so it exits now. Returns whether one ran.
pub(crate) fn stop_idle_watch(engine: &Engine, account: &str, folder: &str) -> bool {
    let removed = engine
        .watched
        .lock()
        .unwrap()
        .remove(&watch_key(account, folder));
    if removed {
        engine.pause_signal.notify_waiters();
    }
    removed
}

/// Watch the folders the user opted in to notifications for (INBOX is started
/// by the caller). IMAP IDLE covers one mailbox per connection, so only the
/// plan's live folders get a watcher; [`poll_notify_folders`] covers the rest.
/// Safe to call again after the opt-ins change: running watches are kept, and
/// a folder promoted into the live set gets one.
pub(crate) fn start_notify_folder_watches(engine: &Arc<Engine>, out: &Writer, account: &str) {
    let plan = store::notify_folder_plan(&engine.db.lock().unwrap(), account).unwrap_or_default();
    for folder in plan.live {
        start_idle_watch(engine.clone(), out.clone(), account.to_string(), folder);
    }
}

/// How often opted-in folders past the live set are checked for new mail.
pub(crate) const NOTIFY_POLL_INTERVAL: Duration = Duration::from_secs(120);

/// Check the opted-in folders that have no IDLE connection of their own, over
/// pooled sessions, for as long as the sidecar runs. One folder at a time: a
/// poll is cheap and nothing is waiting on it.
pub(crate) async fn poll_notify_folders(engine: Arc<Engine>, out: Writer) {
    loop {
        tokio::time::sleep(NOTIFY_POLL_INTERVAL).await;
        let accounts: Vec<String> = engine.accounts.lock().await.keys().cloned().collect();
        for account in accounts {
            if engine.is_paused(&account) {
                continue;
            }
            let polled = store::notify_folder_plan(&engine.db.lock().unwrap(), &account)
                .map(|plan| plan.polled)
                .unwrap_or_default();
            for folder in polled {
                // A view (a kanban column) may already be watching it live.
                if engine
                    .watched
                    .lock()
                    .unwrap()
                    .contains(&watch_key(&account, &folder))
                {
                    continue;
                }
                if let Err(err) = sync_and_notify(&engine, &out, &account, &folder).await {
                    eprintln!("meron-core: poll {account}/{folder}: {err:#}");
                }
            }
        }
    }
}

/// Long-lived per-account/folder IDLE watcher. Reconnects with backoff on error
/// so a dropped connection or server timeout resumes pushing updates.
pub(crate) async fn idle_watch(engine: Arc<Engine>, out: Writer, account: String, folder: String) {
    let key = watch_key(&account, &folder);
    loop {
        // Stop cleanly once the account has been removed (account.remove).
        if !engine.accounts.lock().await.contains_key(&account) {
            engine.watched.lock().unwrap().remove(&key);
            break;
        }
        // Stop checking while paused; account.setPaused respawns us on resume.
        if engine.is_paused(&account) {
            engine.watched.lock().unwrap().remove(&key);
            break;
        }
        if !engine.watched.lock().unwrap().contains(&key) {
            break;
        }
        if let Err(e) = idle_once(&engine, &out, &account, &folder).await {
            if let Some((name, detail)) = meron_core::engine::needs_reconnect_event(&account, &e) {
                emit(&out, name, detail).await;
            }
            emit(
                &out,
                "error",
                json!({ "message": format!("idle {account}/{folder}: {e:#}") }),
            )
            .await;
            // Back off before reconnecting on error, but wake immediately on a
            // pause toggle so a just-paused account stops promptly (next
            // iteration sees is_paused). A clean return (pause or OS resume)
            // skips the backoff: pause exits at the top, resume reconnects now.
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(15)) => {}
                _ = wait_for_watch_stop(&engine, &account, &key) => {}
            }
        }
    }
    emit(
        &out,
        "watch.stopped",
        json!({ "account": account, "folder": folder }),
    )
    .await;
}

async fn wait_for_watch_stop(engine: &Engine, account: &str, key: &str) {
    loop {
        let notified = engine.pause_signal.notified();
        tokio::pin!(notified);
        // Register before checking persistent state so notify_waiters cannot
        // fall between the check and the wait. Stops before registration are
        // caught by the state check instead.
        notified.as_mut().enable();
        if !engine.watched.lock().unwrap().contains(key) || engine.is_paused(account) {
            return;
        }
        notified.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meron_core::secrets;
    use rusqlite::Connection;

    struct TestHost;

    impl EngineHost for TestHost {
        fn open_db(&self) -> anyhow::Result<Connection> {
            store::open_at(":memory:")
        }

        fn apply_secret(&self, _: &Connection, _: &str, _: &mut imap::Creds) {}

        fn store_secret(
            &self,
            _: &Connection,
            _: &str,
            _: &secrets::Secrets,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn stop_before_wait_registration_is_not_lost() {
        let engine = Engine::new(Box::new(TestHost)).unwrap();
        let key = watch_key("bob", "Archive");
        engine.watched.lock().unwrap().insert(key.clone());
        let wait = wait_for_watch_stop(&engine, "bob", &key);
        // Simulate watch.stop during sync, before the cancellation future polls.
        engine.watched.lock().unwrap().remove(&key);
        engine.pause_signal.notify_waiters();
        tokio::time::timeout(Duration::from_secs(1), wait)
            .await
            .expect("a stop before registration must still cancel the watch");
    }

    #[tokio::test]
    async fn stopping_a_watch_wakes_it_and_view_interest_is_tracked_per_folder() {
        let engine = Engine::new(Box::new(TestHost)).unwrap();
        let key = watch_key("carol", "Work");
        engine.watched.lock().unwrap().insert(key.clone());
        set_view_watch("carol", "Work", true);
        assert!(view_watches("carol", "Work"));
        assert!(!view_watches("carol", "Other"));
        set_view_watch("carol", "Work", false);
        assert!(!view_watches("carol", "Work"));

        let wait = wait_for_watch_stop(&engine, "carol", &key);
        tokio::pin!(wait);
        assert!(futures::poll!(&mut wait).is_pending());
        assert!(stop_idle_watch(&engine, "carol", "Work"));
        tokio::time::timeout(Duration::from_secs(1), wait)
            .await
            .expect("a stopped watch must wake");
        assert!(!stop_idle_watch(&engine, "carol", "Work"));
    }

    #[tokio::test]
    async fn unrelated_stop_keeps_watching_until_own_stop() {
        let engine = Engine::new(Box::new(TestHost)).unwrap();
        let key = watch_key("bob", "Archive");
        engine.watched.lock().unwrap().insert(key.clone());
        let wait = wait_for_watch_stop(&engine, "bob", &key);
        tokio::pin!(wait);
        assert!(futures::poll!(&mut wait).is_pending());
        engine.pause_signal.notify_waiters();
        assert!(futures::poll!(&mut wait).is_pending());
        engine.watched.lock().unwrap().remove(&key);
        engine.pause_signal.notify_waiters();
        tokio::time::timeout(Duration::from_secs(1), wait)
            .await
            .expect("a registered watcher must wake when stopped");
    }
}

/// Sync `folder` and surface the result to the UI: a "new mail" toast when
/// genuine arrivals landed in INBOX or a folder opted in to notifications,
/// otherwise a silent refresh.
/// Shared by the IDLE wake path and the post-connect catch-up so both behave
/// identically.
pub(crate) async fn sync_and_notify(
    engine: &Arc<Engine>,
    out: &Writer,
    account: &str,
    folder: &str,
) -> anyhow::Result<()> {
    // An IDLE wake can mean new mail *or* just a flag change (e.g. a message
    // read on another device). UIDNEXT only advances for new arrivals, so
    // compare it across the refresh to tell them apart.
    let is_inbox = folder.eq_ignore_ascii_case("INBOX");
    // Refresh on a separate connection (the IDLE one stays dedicated to IDLE).
    let synced = sync_messages(engine, account, folder, IDLE_LIMIT).await?;

    let arrivals = (!synced.arrivals.is_empty()).then_some(synced.arrivals);

    if let Some(headers) = arrivals {
        // Building the detail fetches the arrivals' own bodies (the notification
        // shows a snippet of each); warm the rest of the backlog behind it so the
        // first open of anything else is instant too.
        let detail = new_messages_detail(engine, account, folder, &headers).await;
        spawn_body_prefetch(engine.clone(), account.to_string(), folder.to_string());
        if let Some(detail) = detail {
            emit(out, "mail.newMessages", detail).await;
        }
    } else {
        if !is_inbox {
            spawn_body_prefetch(engine.clone(), account.to_string(), folder.to_string());
        }
        // Flag-only change: refresh the UI silently, no "new mail" toast.
        emit(
            out,
            "mail.synced",
            json!({ "account": account, "folder": folder, "synced": synced.count }),
        )
        .await;
    }
    Ok(())
}

/// One IDLE connection lifecycle: hold a dedicated session on one mailbox, and
/// on each server notification refresh that folder in the store.
pub(crate) async fn idle_once(
    engine: &Arc<Engine>,
    out: &Writer,
    account: &str,
    folder: &str,
) -> anyhow::Result<()> {
    let key = watch_key(account, folder);
    let mut session = tokio::select! {
        biased;
        _ = wait_for_watch_stop(engine, account, &key) => return Ok(()),
        result = async {
            let creds = engine.ensure_valid_creds(account).await?;
            let mut session = imap::connect(&creds).await?;
            session.select(folder).await.with_context(|| format!("SELECT {folder}"))?;
            Ok::<_, anyhow::Error>(session)
        } => result?,
    };

    // Catch up before parking in IDLE: the server only pushes notifications for
    // mail that arrives *after* IDLE begins, so anything delivered while we were
    // disconnected (startup, error reconnect, or resume from suspend) would
    // otherwise stay invisible until the next push. Cheap because idle_once is
    // only (re)entered on a fresh connection, not on each 15-min IDLE timeout.
    // Finish persistence and notification together even if stopped mid-sync:
    // dropping the future could lose arrivals after UIDNEXT is saved, or leave
    // an incomplete JSON line on stdout while emit is writing it.
    sync_and_notify(engine, out, account, folder).await?;

    loop {
        // Only cancel socket work. Checking persistent stop state here also
        // catches stops received during the preceding sync or event write.
        // Unrelated pause signals leave this connection in IDLE.
        let (next_session, response) = tokio::select! {
            biased;
            _ = wait_for_watch_stop(engine, account, &key) => return Ok(()),
            // Drop a socket held across suspend without waiting for DONE.
            _ = engine.resume_signal.notified() => return Ok(()),
            result = async {
                let mut handle = session.idle();
                handle.init().await.context("IDLE init")?;
                let response = {
                    let (idle_fut, _stop) = handle.wait_with_timeout(Duration::from_secs(15 * 60));
                    idle_fut.await.context("IDLE")?
                };
                let session = handle.done().await.context("IDLE done")?;
                Ok::<_, anyhow::Error>((session, response))
            } => result?,
        };
        session = next_session;
        if let async_imap::extensions::idle::IdleResponse::NewData(_) = response {
            sync_and_notify(engine, out, account, folder).await?;
        }
    }
}
