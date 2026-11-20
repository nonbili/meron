//! Request dispatch: routes each method to its domain handler.

mod accounts;
mod actions;
mod backup;
pub(crate) mod caldav;
mod calendar;
mod compose;
mod feeds;
mod folders;
pub(crate) mod messages;
mod oauth;
mod openpgp;
mod tasks;
mod watch;

use serde_json::Value;
use std::sync::Arc;

use meron_core::changelog;
use meron_core::engine::Engine;
use meron_core::protocol::{Request, ping_response};

use crate::Writer;
use crate::sidecar::prefs;

pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    if crate::sidecar::local::handles(req) {
        return crate::sidecar::local::dispatch(engine, req, out).await;
    }
    match req.method.as_str() {
        "mcp.prepareDelete" => meron_core::mcp_mail::prepare_delete(engine, p.clone()).await,
        "mcp.delete" => meron_core::mcp_mail::delete(engine, p.clone()).await,
        "mcp.organize" => meron_core::mcp_mail::organize(engine, p.clone()).await,
        "ping" => Ok(ping_response()),
        "eml.read" => {
            let path = crate::sidecar::params::req_str(p, "path")?;
            let session = crate::sidecar::params::req_str(p, "session")?;
            let message = tokio::task::spawn_blocking(move || {
                meron_core::eml::read(
                    std::path::Path::new(&path),
                    &session,
                    &meron_core::parse::media_root(),
                )
            })
            .await??;
            Ok(serde_json::to_value(message)?)
        }
        "reminders.list" | "reminders.claim" | "reminders.skipPast" | "reminders.acknowledge" => {
            let method = req.method.clone();
            let params = p.clone();
            crate::sidecar::db::run(&engine.db, "reminders.dispatch", move |db| {
                meron_core::reminders::dispatch(db, &method, &params)
            })
            .await
        }
        "search.parse" | "search.build" => meron_core::search_query::handle_request(&req.method, p)
            .ok_or_else(|| anyhow::anyhow!("bad {} request", req.method)),
        "avatar.resolve" => {
            let params = p.clone();
            let image =
                tokio::task::spawn_blocking(move || meron_core::avatar::resolve(&params)).await?;
            Ok(serde_json::to_value(image)?)
        }

        // Fetch the in-app changelog from the GitHub releases atom feed. The
        // network call runs on the blocking pool.
        "changelog.fetch" => {
            let variant = changelog::Variant::parse(
                p.get("variant")
                    .and_then(Value::as_str)
                    .unwrap_or("desktop"),
            );
            let releases = tokio::task::spawn_blocking(move || changelog::fetch(variant)).await??;
            Ok(releases)
        }

        "app.prefsGet" | "app.prefsSet" => prefs::dispatch(engine, req, out).await,

        "oauth.authorizeUrl" | "oauth.exchangeCode" | "oauth.fetchPhoto" => {
            oauth::dispatch(req).await
        }

        "backup.export" | "backup.import" => backup::dispatch(engine, req, out).await,

        m if m.starts_with("pgp.") => openpgp::dispatch(engine, req, out).await,

        m if m.starts_with("caldav.") => caldav::dispatch(engine, req, out).await,
        m if m.starts_with("carddav.") => {
            let engine = engine.clone();
            let method = m.to_string();
            let params = p.clone();
            let result = tokio::task::spawn_blocking(move || {
                meron_core::carddav::dispatch(&engine, &method, &params)
            })
            .await??;
            if !matches!(m, "carddav.accounts" | "carddav.contacts") {
                crate::emit(out, "carddav.changed", serde_json::json!({})).await;
            }
            Ok(result)
        }

        "account.list"
        | "account.connect"
        | "account.probeCert"
        | "account.remove"
        | "account.setImages"
        | "account.setConversationHtml"
        | "account.setChatWallpaper"
        | "account.setColor"
        | "account.setProxy"
        | "account.setCertPin"
        | "account.setName"
        | "account.setSenderName"
        | "account.setAvatar"
        | "account.setAliases"
        | "account.setSignature"
        | "account.setUnified"
        | "account.setMuted"
        | "account.setPaused"
        | "account.setSaveSentCopy"
        | "account.setRSSSyncInterval"
        | "account.reorder" => accounts::dispatch(engine, req, out).await,

        "account.addRss" | "feed.add" | "feed.remove" | "feed.move" | "feed.rename"
        | "rss.exportOpml" | "rss.importOpml" | "rss.thread" | "rss.markRead"
        | "rss.markAllRead" | "rss.markStarred" => feeds::dispatch(engine, req, out).await,

        "tasks.lists"
        | "tasks.listCreate"
        | "tasks.listRename"
        | "tasks.listDelete"
        | "tasks.listReorder"
        | "tasks.items"
        | "tasks.create"
        | "tasks.update"
        | "tasks.setDone"
        | "tasks.delete"
        | "tasks.reorder"
        | "tasks.clearCompleted"
        | "tasks.restore"
        | "tasks.forThread" => tasks::dispatch(engine, req, out).await,

        "calendar.events"
        | "calendar.get"
        | "calendar.create"
        | "calendar.update"
        | "calendar.delete"
        | "calendar.restore"
        | "invite.removeFromCalendar"
        | "invite.addToCalendar" => calendar::dispatch(engine, req, out).await,

        "folders.list"
        | "folders.create"
        | "folders.delete"
        | "folders.rename"
        | "folders.setNotify"
        | "messages.emptyFolder" => folders::dispatch(engine, req, out).await,

        "messages.unifiedRecent"
        | "messages.recent"
        | "starred.items"
        | "messages.stillUnread"
        | "identity.allocate"
        | "contacts.suggest"
        | "contacts.threads"
        | "messages.sync"
        | "messages.read"
        | "messages.thread"
        | "messages.threadHeaders" => messages::dispatch(engine, req, out).await,

        "send"
        | "invite.respond"
        | "save_draft"
        | "discard_draft"
        | "messages.saveRaw"
        | "messages.exportUids"
        | "messages.saveRawBatch" => compose::dispatch(engine, req, out).await,

        "messages.markRead"
        | "messages.markStarred"
        | "messages.setTags"
        | "tags.replace"
        | "messages.delete"
        | "messages.move"
        | "messages.archive"
        | "messages.copy"
        | "messages.markAllRead"
        | "messages.markAllReadUnified" => actions::dispatch(engine, req, out).await,

        "tags.list" | "rules.list" | "rules.set" | "rules.matches" | "rules.run"
        | "rules.status" | "rules.stop" => messages::dispatch(engine, req, out).await,

        "watch.start" | "watch.stop" | "system.resumed" => watch::dispatch(engine, req, out).await,

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}

#[cfg(test)]
mod contention_tests {
    use super::*;
    use meron_core::{engine::EngineHost, imap, secrets, store};
    use std::time::{Duration, Instant};

    struct Host;
    impl EngineHost for Host {
        fn open_db(&self) -> anyhow::Result<rusqlite::Connection> {
            store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &rusqlite::Connection, _: &str, _: &mut imap::Creds) {}
        fn store_secret(
            &self,
            _: &rusqlite::Connection,
            _: &str,
            _: &secrets::Secrets,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    // One runtime worker makes any synchronous DB wait observable. The holder
    // has a fallback deadline so a regression fails rather than deadlocking.
    #[tokio::test(flavor = "current_thread")]
    async fn cached_startup_reads_do_not_block_ping_or_responses() {
        let engine = Arc::new(Engine::new(Box::new(Host)).unwrap());
        let db = engine.db.clone();
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _guard = db.lock().unwrap();
            locked_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(Duration::from_secs(2));
        });
        locked_rx.recv().unwrap();
        let out = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
        let mut reads = Vec::new();
        let started = Instant::now();
        // Refresh scheduling must also return immediately while the DB is held.
        crate::sidecar::spawn::spawn_message_sync(
            engine.clone(),
            out.clone(),
            "test".into(),
            "INBOX".into(),
            50,
        );
        crate::sidecar::spawn::spawn_rss_sync(engine.clone(), out.clone(), "test".into());
        crate::sidecar::spawn::spawn_folder_sync(engine.clone(), out.clone(), "test".into());
        for (method, params) in [
            ("tasks.lists", serde_json::json!({})),
            ("tasks.items", serde_json::json!({"list_id":"test"})),
            (
                "folders.list",
                serde_json::json!({"account":"test", "refresh":true}),
            ),
            (
                "messages.recent",
                serde_json::json!({"account":"test", "refresh":true}),
            ),
            ("account.list", serde_json::json!({})),
            ("reminders.list", serde_json::json!({})),
        ] {
            let engine = engine.clone();
            let out = out.clone();
            reads.push(tokio::spawn(async move {
                dispatch(
                    &engine,
                    &Request {
                        id: 1,
                        method: method.into(),
                        params,
                    },
                    &out,
                )
                .await
            }));
        }
        tokio::task::yield_now().await;
        let ping = dispatch(
            &engine,
            &Request {
                id: 2,
                method: "ping".into(),
                params: serde_json::json!({}),
            },
            &out,
        )
        .await
        .unwrap();
        crate::respond(&out, 2, ping).await;
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "DB waits stalled the runtime"
        );
        assert!(reads.iter().all(|read| !read.is_finished()));
        release_tx.send(()).unwrap();
        for read in reads {
            read.await.unwrap().unwrap();
        }
        holder.join().unwrap();
    }
}
