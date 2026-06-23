pub(crate) use crate::imap::{self, Creds, MessageHeader};
pub(crate) use crate::parse::Message;
pub(crate) use crate::rss;
pub(crate) use crate::secrets::Secrets;
pub(crate) use crate::smtp::{self, AttachmentInput};
pub(crate) use crate::store::{self, AccountMeta};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub(crate) use std::path::{Component, Path};
pub(crate) use std::sync::Mutex;
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

mod accounts;
mod feeds;
mod helpers;
mod mail;
mod media;
mod oauth;

pub(crate) use accounts::*;
pub(crate) use feeds::*;
pub(crate) use helpers::*;
pub(crate) use mail::*;
pub(crate) use media::*;
pub(crate) use oauth::*;

#[cfg(test)]
mod tests;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the Meron core request/response/event protocol.
///
/// Bump this on any breaking change to request, response, or event shapes. The
/// desktop Go bridge checks it on the `ready` event, and mobile hosts should
/// assert it during core startup before issuing commands.
pub const PROTOCOL_VERSION: u32 = 1;
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub fn ready_event() -> Value {
    json!({ "version": VERSION, "protocol": PROTOCOL_VERSION })
}

pub fn ping_response() -> Value {
    json!({ "pong": true, "version": VERSION, "protocol": PROTOCOL_VERSION })
}

pub fn dispatch_protocol_request(req: &Request) -> Result<Value, String> {
    match req.method.as_str() {
        "ping" => Ok(ping_response()),
        "account.list" => Ok(json!({ "accounts": [] })),
        "account.autodiscover" => autodiscover_mobile_account(&req.params),
        "account.addOAuth" => Ok(json!({ "account": Value::Null })),
        "account.exchangeOAuthCode" => Ok(json!({ "account": Value::Null })),
        "account.addRss" | "account.addRSS" => Ok(json!({ "account": Value::Null })),
        "account.remove" => Ok(json!({ "ok": true })),
        "account.setName" => Ok(json!({ "ok": true })),
        "account.setSenderName" => Ok(json!({ "ok": true })),
        "account.setAvatar" => Ok(json!({ "ok": true })),
        "account.writeAvatarFile" => Ok(json!({ "url": "" })),
        "account.setChatWallpaper" => Ok(json!({ "ok": true })),
        "account.writeChatWallpaperFile" => Ok(json!({ "url": "" })),
        "account.setImages" => Ok(json!({ "ok": true })),
        "account.setConversationHtml" => Ok(json!({ "ok": true })),
        "account.setUnified" => Ok(json!({ "ok": true })),
        "account.setMuted" => Ok(json!({ "ok": true })),
        "account.setPaused" => Ok(json!({ "ok": true })),
        "account.setRSSSyncInterval" => Ok(json!({ "ok": true })),
        "account.setAliases" => Ok(json!({ "ok": true })),
        "account.reorder" => Ok(json!({ "ok": true })),
        "contacts.suggest" | "mail.suggestContacts" => Ok(json!({ "contacts": [] })),
        "feed.add" | "rss.addFeed" => Ok(json!({ "ok": true })),
        "feed.remove" | "rss.removeFeed" => Ok(json!({ "ok": true })),
        "feed.move" | "rss.moveFeed" => Ok(json!({ "ok": true, "moved": 0 })),
        "rss.exportOpml" => Ok(json!({ "opml": "" })),
        "rss.importOpml" => Ok(json!({ "imported": 0 })),
        "mail.folderList" => Ok(json!({ "folders": [] })),
        "mail.folderCreate" => Ok(json!({ "folders": [] })),
        "mail.threadList" => Ok(json!({ "threads": [] })),
        "mail.threadRead" => Ok(json!({ "messages": [] })),
        "mail.attachmentRead" | "mail.readAttachment" => Ok(json!({ "data": "" })),
        "storage.usage" => Ok(json!({ "cacheBytes": 0, "dbBytes": 0 })),
        "storage.clearCache" => Ok(json!({ "cacheBytes": 0, "dbBytes": 0 })),
        "mail.starredItems" => Ok(json!({ "items": [] })),
        "mail.send" => Ok(json!({ "ok": true })),
        "mail.saveDraft" => Ok(json!({ "ok": true })),
        "mail.discardDraft" => Ok(json!({ "ok": true, "deleted": 0, "permanent": true })),
        "mail.archive" => Ok(json!({ "ok": true, "moved": 0 })),
        "mail.delete" => Ok(json!({ "ok": true, "deleted": 0 })),
        "mail.move" => Ok(json!({ "ok": true, "moved": 0 })),
        "mail.copy" => Ok(json!({ "ok": true, "copied": 0 })),
        "mail.markRead" => Ok(json!({ "ok": true })),
        "mail.markAllRead" => Ok(json!({ "ok": true })),
        "mail.markStarred" => Ok(json!({ "ok": true })),
        "rss.thread" => Ok(json!({ "messages": [] })),
        "rss.markRead" => Ok(json!({ "ok": true })),
        "rss.markStarred" => Ok(json!({ "ok": true })),
        method => Err(format!("unknown method: {method}")),
    }
}

pub fn dispatch_mobile_protocol_request(req: &Request, data_dir: &str) -> Result<Value, String> {
    match req.method.as_str() {
        "account.list" => list_mobile_accounts(data_dir),
        "account.addPassword" => add_mobile_password_account(data_dir, &req.params),
        "account.autodiscover" => autodiscover_mobile_account(&req.params),
        "account.addOAuth" => add_mobile_oauth_account(data_dir, &req.params),
        "account.exchangeOAuthCode" => exchange_mobile_oauth_code(data_dir, &req.params),
        "account.addRss" | "account.addRSS" => add_mobile_rss_account(data_dir, &req.params),
        "account.remove" => remove_mobile_account(data_dir, &req.params),
        "account.setName" => set_mobile_account_name(data_dir, &req.params),
        "account.setSenderName" => set_mobile_account_sender_name(data_dir, &req.params),
        "account.setAvatar" => set_mobile_account_avatar(data_dir, &req.params),
        "account.writeAvatarFile" => {
            write_mobile_account_media_file(data_dir, &req.params, "avatars")
        }
        "account.setChatWallpaper" => set_mobile_account_chat_wallpaper(data_dir, &req.params),
        "account.writeChatWallpaperFile" => {
            write_mobile_account_media_file(data_dir, &req.params, "wallpapers")
        }
        "account.setImages" => set_mobile_account_images(data_dir, &req.params),
        "account.setConversationHtml" => {
            set_mobile_account_bool_pref(data_dir, &req.params, "conversation_html")
        }
        "account.setUnified" => {
            set_mobile_account_bool_pref(data_dir, &req.params, "included_in_unified")
        }
        "account.setMuted" => set_mobile_account_bool_pref(data_dir, &req.params, "muted"),
        "account.setPaused" => set_mobile_account_bool_pref(data_dir, &req.params, "paused"),
        "account.setRSSSyncInterval" => set_mobile_account_rss_sync_interval(data_dir, &req.params),
        "account.setAliases" => set_mobile_account_aliases(data_dir, &req.params),
        "account.reorder" => reorder_mobile_accounts(data_dir, &req.params),
        "contacts.suggest" | "mail.suggestContacts" => {
            suggest_mobile_contacts(data_dir, &req.params)
        }
        "feed.add" | "rss.addFeed" => add_mobile_rss_feed(data_dir, &req.params),
        "feed.remove" | "rss.removeFeed" => remove_mobile_rss_feed(data_dir, &req.params),
        "feed.move" | "rss.moveFeed" => move_mobile_rss_feed(data_dir, &req.params),
        "rss.exportOpml" => export_mobile_opml(data_dir, &req.params),
        "rss.importOpml" => import_mobile_opml(data_dir, &req.params),
        "mail.folderList" => list_mobile_folders(data_dir, &req.params),
        "mail.folderCreate" => create_mobile_folder(data_dir, &req.params),
        "mail.threadList" => list_mobile_threads(data_dir, &req.params),
        "mail.threadRead" => read_mobile_thread(data_dir, &req.params),
        "mail.attachmentRead" | "mail.readAttachment" => {
            read_mobile_attachment_file(data_dir, &req.params)
        }
        "storage.usage" => mobile_storage_usage(data_dir),
        "storage.clearCache" => clear_mobile_storage_cache(data_dir),
        "mail.starredItems" => list_mobile_starred_items(data_dir, &req.params),
        "mail.sync" | "messages.sync" => sync_mobile_mail(data_dir, &req.params),
        "mail.send" => send_mobile_message(data_dir, &req.params),
        "mail.saveDraft" => save_mobile_draft(data_dir, &req.params),
        "mail.discardDraft" => discard_mobile_draft(data_dir, &req.params),
        "mail.archive" => archive_mobile_thread(data_dir, &req.params),
        "mail.delete" => delete_mobile_thread(data_dir, &req.params),
        "mail.move" => move_mobile_thread(data_dir, &req.params),
        "mail.copy" => copy_mobile_thread(data_dir, &req.params),
        "mail.markRead" => mark_mobile_thread_read(data_dir, &req.params),
        "mail.markAllRead" => mark_mobile_folder_all_read(data_dir, &req.params),
        "mail.markStarred" => mark_mobile_thread_starred(data_dir, &req.params),
        "rss.sync" => sync_mobile_rss(data_dir, &req.params),
        "rss.thread" => read_mobile_rss_thread(data_dir, &req.params),
        "rss.markRead" => mark_mobile_rss_thread_read(data_dir, &req.params),
        "rss.markStarred" => mark_mobile_rss_thread_starred(data_dir, &req.params),
        _ => dispatch_protocol_request(req),
    }
}

pub fn invoke_protocol_json(request_json: &str) -> Value {
    match serde_json::from_str::<Request>(request_json) {
        Ok(req) => match dispatch_protocol_request(&req) {
            Ok(result) => json!({ "id": req.id, "result": result }),
            Err(message) => json!({ "id": req.id, "error": { "message": message } }),
        },
        Err(err) => json!({ "error": { "message": format!("bad request: {err}") } }),
    }
}

pub fn invoke_mobile_protocol_json(request_json: &str, data_dir: Option<&str>) -> Value {
    match serde_json::from_str::<Request>(request_json) {
        Ok(req) => {
            let result = match data_dir {
                Some(data_dir) => dispatch_mobile_protocol_request(&req, data_dir),
                None => dispatch_protocol_request(&req),
            };
            match result {
                Ok(result) => json!({ "id": req.id, "result": result }),
                Err(message) => json!({ "id": req.id, "error": { "message": message } }),
            }
        }
        Err(err) => json!({ "error": { "message": format!("bad request: {err}") } }),
    }
}
