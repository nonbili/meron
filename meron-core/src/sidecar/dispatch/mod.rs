//! Request dispatch: routes each method to its domain handler.

mod accounts;
mod actions;
mod backup;
mod compose;
mod feeds;
mod folders;
mod messages;
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
    match req.method.as_str() {
        "mcp.prepareDelete" => meron_core::mcp_mail::prepare_delete(engine, p.clone()).await,
        "mcp.delete" => meron_core::mcp_mail::delete(engine, p.clone()).await,
        "mcp.organize" => meron_core::mcp_mail::organize(engine, p.clone()).await,
        "ping" => Ok(ping_response()),
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

        "backup.export" | "backup.import" => backup::dispatch(engine, req, out).await,

        "account.list"
        | "account.connect"
        | "account.probeCert"
        | "account.remove"
        | "account.setImages"
        | "account.setConversationHtml"
        | "account.setChatWallpaper"
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

        "account.addRss" | "feed.add" | "feed.remove" | "feed.move" | "rss.exportOpml"
        | "rss.importOpml" | "rss.thread" | "rss.markRead" | "rss.markAllRead"
        | "rss.markStarred" => feeds::dispatch(engine, req, out).await,

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

        "folders.list"
        | "folders.create"
        | "folders.delete"
        | "folders.archive"
        | "messages.emptyFolder" => folders::dispatch(engine, req, out).await,

        "messages.unifiedRecent"
        | "messages.recent"
        | "starred.items"
        | "identity.allocate"
        | "contacts.suggest"
        | "messages.sync"
        | "messages.read"
        | "messages.thread"
        | "messages.threadHeaders" => messages::dispatch(engine, req, out).await,

        "send" | "save_draft" | "discard_draft" | "messages.saveRaw" => {
            compose::dispatch(engine, req, out).await
        }

        "messages.markRead"
        | "messages.markStarred"
        | "messages.delete"
        | "messages.move"
        | "messages.copy"
        | "messages.markAllRead"
        | "messages.markAllReadUnified" => actions::dispatch(engine, req, out).await,

        "watch.start" | "watch.stop" | "system.resumed" => watch::dispatch(engine, req, out).await,

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
