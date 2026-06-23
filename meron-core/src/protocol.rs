use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Component, Path};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::imap::{self, Creds, MessageHeader};
use crate::parse::Message;
use crate::rss;
use crate::secrets::Secrets;
use crate::smtp::{self, AttachmentInput};
use crate::store::{self, AccountMeta};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the Meron core request/response/event protocol.
///
/// Bump this on any breaking change to request, response, or event shapes. The
/// desktop Go bridge checks it on the `ready` event, and mobile hosts should
/// assert it during core startup before issuing commands.
pub const PROTOCOL_VERSION: u32 = 1;
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const OUTLOOK_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const OUTLOOK_SCOPES: &str = "offline_access openid email https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send";

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

fn list_mobile_accounts(data_dir: &str) -> Result<Value, String> {
    with_mobile_db(data_dir, |conn| {
        let mut accounts = store::list_accounts(&conn).map_err(|err| err.to_string())?;
        let creds_by_id = store::load_accounts(&conn)
            .map_err(|err| err.to_string())?
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        for account in &mut accounts {
            if account
                .get("auth_type")
                .and_then(Value::as_str)
                .is_some_and(|auth_type| auth_type == "rss")
            {
                continue;
            }
            let needs_reconnect = account
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| {
                    let mut creds = creds_by_id.get(id)?.clone();
                    load_mobile_secret(&conn, id).apply_to(&mut creds);
                    Some(account_needs_reconnect(&creds))
                })
                .unwrap_or(true);
            if let Some(obj) = account.as_object_mut() {
                obj.insert("needs_reconnect".to_string(), json!(needs_reconnect));
            }
        }
        Ok(json!({ "accounts": accounts }))
    })
}

fn req_account_pref_id(params: &Value) -> Result<String, String> {
    req_str(params, "account")
        .or_else(|_| req_str(params, "account_id"))
        .or_else(|_| req_str(params, "id"))
}

fn remove_mobile_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    with_mobile_db(data_dir, |conn| {
        store::delete_account(&conn, &id).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn autodiscover_mobile_account(params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?.trim().to_string();
    let Some((_, domain)) = email.rsplit_once('@') else {
        return Err("invalid email".to_string());
    };
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return Err("invalid email".to_string());
    }
    let (provider, imap_host, imap_port, smtp_host, smtp_port, source) =
        mobile_provider_mail_settings(&domain).unwrap_or_else(|| {
            (
                String::new(),
                format!("imap.{domain}"),
                993,
                format!("smtp.{domain}"),
                465,
                "guess".to_string(),
            )
        });
    let mut result = json!({
        "imap_host": imap_host,
        "imap_port": imap_port,
        "smtp_host": smtp_host,
        "smtp_port": smtp_port,
        "username": email,
        "source": source,
    });
    if !provider.is_empty() {
        result["provider_name"] = json!(provider);
    }
    if let Some((provider, url)) =
        mobile_app_password_hint(&domain, result["imap_host"].as_str().unwrap_or_default())
    {
        result["app_password_hint"] = json!({ "provider": provider, "url": url });
    }
    Ok(result)
}

fn mobile_provider_mail_settings(
    domain: &str,
) -> Option<(String, String, u16, String, u16, String)> {
    let settings = match domain {
        "gmail.com" | "googlemail.com" => ("Gmail", "imap.gmail.com", 993, "smtp.gmail.com", 465),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => (
            "Outlook",
            "outlook.office365.com",
            993,
            "smtp.office365.com",
            587,
        ),
        "icloud.com" | "me.com" | "mac.com" => {
            ("iCloud", "imap.mail.me.com", 993, "smtp.mail.me.com", 587)
        }
        "yahoo.com" | "ymail.com" | "rocketmail.com" => (
            "Yahoo",
            "imap.mail.yahoo.com",
            993,
            "smtp.mail.yahoo.com",
            465,
        ),
        "aol.com" => ("AOL", "imap.aol.com", 993, "smtp.aol.com", 465),
        "fastmail.com" | "fastmail.fm" => (
            "Fastmail",
            "imap.fastmail.com",
            993,
            "smtp.fastmail.com",
            465,
        ),
        "proton.me" | "protonmail.com" => {
            ("Proton Mail Bridge", "127.0.0.1", 1143, "127.0.0.1", 1025)
        }
        _ => return None,
    };
    Some((
        settings.0.to_string(),
        settings.1.to_string(),
        settings.2,
        settings.3.to_string(),
        settings.4,
        "known".to_string(),
    ))
}

fn mobile_app_password_hint(domain: &str, imap_host: &str) -> Option<(&'static str, &'static str)> {
    let host = imap_host.to_ascii_lowercase();
    let contains = |needle: &str| host.contains(needle) || domain.contains(needle);
    if contains("mail.me.com")
        || contains("icloud.com")
        || contains("me.com")
        || contains("mac.com")
    {
        Some(("iCloud", "https://account.apple.com/account/manage"))
    } else if contains("fastmail") {
        Some((
            "Fastmail",
            "https://app.fastmail.com/settings/security/apppassword",
        ))
    } else if contains("yahoo") || contains("ymail") || contains("rocketmail") {
        Some((
            "Yahoo",
            "https://login.yahoo.com/account/security/app-passwords",
        ))
    } else if contains("aol.com") {
        Some((
            "AOL",
            "https://login.aol.com/account/security/app-passwords",
        ))
    } else {
        None
    }
}

fn reorder_mobile_accounts(data_dir: &str, params: &Value) -> Result<Value, String> {
    let ids = req_str_array(params, "accounts")?;
    with_mobile_db(data_dir, |conn| {
        store::reorder_accounts(&conn, &ids).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn suggest_mobile_contacts(data_dir: &str, params: &Value) -> Result<Value, String> {
    let mut account = opt_str(params, "account");
    if account.is_empty() {
        account = opt_str(params, "account_id");
    }
    if account.is_empty() {
        account = opt_str(params, "id");
    }
    let query = opt_str(params, "query");
    let limit = opt_u32(params, "limit").unwrap_or(8);
    with_mobile_db(data_dir, |conn| {
        let contacts = store::suggest_contacts(&conn, &account, &query, limit)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "contacts": contacts }))
    })
}

fn set_mobile_account_name(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let name = opt_str(params, "name");
    with_mobile_db(data_dir, |conn| {
        conn.execute(
            "UPDATE accounts SET display_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
            rusqlite::params![name, id],
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn set_mobile_account_sender_name(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let name = opt_str(params, "name");
    with_mobile_db(data_dir, |conn| {
        conn.execute(
            "UPDATE accounts SET sender_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
            rusqlite::params![name, id],
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn set_mobile_account_avatar(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let avatar_url = opt_str(params, "avatar_url");
    with_mobile_db(data_dir, |conn| {
        conn.execute(
            "UPDATE accounts SET avatar_url = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
            rusqlite::params![avatar_url, id],
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn write_mobile_account_media_file(
    data_dir: &str,
    params: &Value,
    media_kind: &str,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let filename = req_str(params, "filename")?;
    let mime = opt_str(params, "mime").to_lowercase();
    let data = req_str(params, "data")?;
    let bytes = STANDARD
        .decode(data.as_bytes())
        .map_err(|err| format!("invalid media data: {err}"))?;
    if bytes.is_empty() {
        return Err("media file is empty".to_string());
    }
    let ext = media_extension(&filename, &mime)?;
    let account_dir = safe_media_segment(&id);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let stored_name = format!("{stamp}.{ext}");
    let relative_url = format!("/media/{media_kind}/{account_dir}/{stored_name}");
    let dir = Path::new(data_dir)
        .join("media")
        .join(media_kind)
        .join(&account_dir);
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    std::fs::write(dir.join(stored_name), bytes).map_err(|err| err.to_string())?;
    Ok(json!({ "url": relative_url }))
}

fn read_mobile_attachment_file(data_dir: &str, params: &Value) -> Result<Value, String> {
    let key = req_str(params, "key")?;
    let relative = Path::new(&key);
    if key.trim().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err("invalid attachment key".to_string());
    }
    let root = Path::new(data_dir).join("attachments");
    let path = root.join(relative);
    let root = root.canonicalize().map_err(|err| err.to_string())?;
    let path = path.canonicalize().map_err(|err| err.to_string())?;
    if !path.starts_with(&root) {
        return Err("invalid attachment key".to_string());
    }
    let data = std::fs::read(path).map_err(|err| err.to_string())?;
    Ok(json!({ "data": STANDARD.encode(data) }))
}

fn mobile_storage_usage(data_dir: &str) -> Result<Value, String> {
    let root = Path::new(data_dir);
    if data_dir.trim().is_empty() {
        return Err("mobile core is not initialized".to_string());
    }
    Ok(json!({
        "cacheBytes": path_size_bytes(&root.join("attachments")).map_err(|err| err.to_string())?,
        "dbBytes": path_size_bytes(&root.join("meron.db")).map_err(|err| err.to_string())?,
    }))
}

fn clear_mobile_storage_cache(data_dir: &str) -> Result<Value, String> {
    let cache_dir = Path::new(data_dir).join("attachments");
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir).map_err(|err| err.to_string())?;
    }
    std::fs::create_dir_all(&cache_dir).map_err(|err| err.to_string())?;
    mobile_storage_usage(data_dir)
}

fn path_size_bytes(path: &Path) -> std::io::Result<u64> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }
    let mut total = 0;
    for entry in std::fs::read_dir(path)? {
        total += path_size_bytes(&entry?.path())?;
    }
    Ok(total)
}

fn media_extension(filename: &str, mime: &str) -> Result<&'static str, String> {
    let name_ext = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim().to_lowercase())
        .unwrap_or_default();
    match name_ext.as_str() {
        "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpg"),
        "webp" => Ok("webp"),
        "gif" => Ok("gif"),
        _ => match mime {
            "image/png" => Ok("png"),
            "image/jpeg" | "image/jpg" => Ok("jpg"),
            "image/webp" => Ok("webp"),
            "image/gif" => Ok("gif"),
            _ => Err("media file must be png, jpeg, webp, or gif".to_string()),
        },
    }
}

fn safe_media_segment(value: &str) -> String {
    let segment: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = segment.trim_matches('_').to_string();
    if cleaned.is_empty() {
        "account".to_string()
    } else {
        cleaned
    }
}

fn set_mobile_account_chat_wallpaper(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let wallpaper = match params.get("wallpaper") {
        Some(Value::Null) | None => None,
        Some(value) => {
            let obj = value
                .as_object()
                .ok_or_else(|| "wallpaper must be an object".to_string())?;
            match obj.get("kind").and_then(Value::as_str).unwrap_or_default() {
                "preset" => {
                    let preset_id = obj
                        .get("presetId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if preset_id.is_empty() {
                        return Err("preset wallpaper requires presetId".to_string());
                    }
                    Some(json!({ "kind": "preset", "presetId": preset_id }))
                }
                "custom" => {
                    let url = obj
                        .get("url")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if !url.starts_with("/media/wallpapers/") {
                        return Err("custom wallpaper URL must be a Meron wallpaper".to_string());
                    }
                    Some(json!({ "kind": "custom", "url": url }))
                }
                _ => return Err("unknown wallpaper kind".to_string()),
            }
        }
    };
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref_json(&conn, &id, "chat_wallpaper", wallpaper)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn set_mobile_account_images(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let enabled = req_bool(params, "enabled")?;
    with_mobile_db(data_dir, |conn| {
        store::set_load_remote_images(&conn, &id, enabled).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn set_mobile_account_bool_pref(
    data_dir: &str,
    params: &Value,
    key: &str,
) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let enabled = req_bool(params, "enabled")?;
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref(&conn, &id, key, enabled).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true }))
    })
}

fn set_mobile_account_rss_sync_interval(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let minutes = params
        .get("minutes")
        .and_then(Value::as_u64)
        .unwrap_or(60)
        .clamp(5, 1440);
    with_mobile_db(data_dir, |conn| {
        store::set_account_pref_u64(&conn, &id, "rss_sync_interval_minutes", minutes)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "minutes": minutes }))
    })
}

fn set_mobile_account_aliases(data_dir: &str, params: &Value) -> Result<Value, String> {
    let id = req_account_pref_id(params)?;
    let mut aliases: Vec<store::Alias> = params
        .get("aliases")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| "aliases must be an array".to_string())?
        .unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    aliases.retain_mut(|alias| {
        alias.email = alias.email.trim().to_string();
        alias.name = alias.name.trim().to_string();
        !alias.email.is_empty() && seen.insert(alias.email.to_lowercase())
    });
    with_mobile_db(data_dir, |conn| {
        store::set_account_aliases(&conn, &id, &aliases).map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "aliases": aliases.len() }))
    })
}

fn add_mobile_password_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?;
    if !email.contains('@') {
        return Err("invalid email".to_string());
    }
    let imap_host = req_str(params, "imap_host")?;
    let smtp_host = req_str(params, "smtp_host")?;
    if imap_host.trim().is_empty() || smtp_host.trim().is_empty() {
        return Err("server required".to_string());
    }
    let username = req_str(params, "username")?;
    if username.trim().is_empty() {
        return Err("username required".to_string());
    }

    let imap_port = req_u16(params, "imap_port").unwrap_or(993);
    let smtp_port = req_u16(params, "smtp_port").unwrap_or(465);
    let tls = params.get("tls").and_then(Value::as_bool).unwrap_or(true);
    let (imap_tls, imap_starttls) = tls_mode(tls, imap_port);
    let (smtp_tls, smtp_starttls) = tls_mode(tls, smtp_port);
    let id = account_id(&email);
    let password = opt_str(params, "password");
    let creds = Creds {
        host: imap_host,
        port: imap_port,
        user: username,
        password,
        tls: imap_tls,
        starttls: imap_starttls,
        smtp_host,
        smtp_port,
        smtp_tls,
        smtp_starttls,
        auth_type: "password".to_string(),
        access_token: None,
        refresh_token: None,
        token_expires_at: 0,
    };
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider: "custom".to_string(),
        email: email.clone(),
        display_name: opt_str(params, "display_name"),
        avatar_url: String::new(),
        sender_name: opt_str(params, "sender_name"),
    };

    with_mobile_db(data_dir, |conn| {
        store::upsert_account(&conn, &id, &meta, &creds).map_err(|err| err.to_string())?;
        store_mobile_secret(&conn, &id, &creds)?;
        let mut account = store::list_accounts(&conn)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .ok_or_else(|| "account not found after save".to_string())?;
        if let Some(obj) = account.as_object_mut() {
            obj.insert(
                "needs_reconnect".to_string(),
                json!(account_needs_reconnect(&creds)),
            );
        }
        Ok(json!({ "account": account }))
    })
}

fn add_mobile_oauth_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?;
    if !email.contains('@') {
        return Err("invalid email".to_string());
    }
    let provider = req_str(params, "provider")?.to_lowercase();
    let (auth_type, default_imap_host, default_smtp_host, default_smtp_port) =
        match provider.as_str() {
            "gmail" => ("gmail_oauth", "imap.gmail.com", "smtp.gmail.com", 587),
            "outlook" => (
                "outlook_oauth",
                "outlook.office365.com",
                "smtp.office365.com",
                587,
            ),
            _ => return Err("unsupported oauth provider".to_string()),
        };
    let username = opt_str(params, "username");
    let access_token = opt_str(params, "access_token");
    let refresh_token = req_str(params, "refresh_token")?;
    let imap_host = params
        .get("imap_host")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_imap_host)
        .to_string();
    let smtp_host = params
        .get("smtp_host")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_smtp_host)
        .to_string();
    let imap_port = req_u16(params, "imap_port").unwrap_or(993);
    let smtp_port = req_u16(params, "smtp_port").unwrap_or(default_smtp_port);
    let token_expires_at = params
        .get("token_expires_at")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let creds = Creds {
        host: imap_host,
        port: imap_port,
        user: if username.is_empty() {
            email.clone()
        } else {
            username
        },
        password: String::new(),
        tls: true,
        starttls: false,
        smtp_host,
        smtp_port,
        smtp_tls: false,
        smtp_starttls: true,
        auth_type: auth_type.to_string(),
        access_token: if access_token.is_empty() {
            None
        } else {
            Some(access_token)
        },
        refresh_token: Some(refresh_token),
        token_expires_at,
    };
    let id = account_id(&email);
    let meta = AccountMeta {
        engine: "mail".to_string(),
        provider,
        email: email.clone(),
        display_name: opt_str(params, "display_name"),
        avatar_url: opt_str(params, "avatar_url"),
        sender_name: opt_str(params, "sender_name"),
    };

    with_mobile_db(data_dir, |conn| {
        store::upsert_account(&conn, &id, &meta, &creds).map_err(|err| err.to_string())?;
        store_mobile_secret(&conn, &id, &creds)?;
        let mut account = store::list_accounts(&conn)
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|account| account.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .ok_or_else(|| "account not found after save".to_string())?;
        if let Some(obj) = account.as_object_mut() {
            obj.insert(
                "needs_reconnect".to_string(),
                json!(account_needs_reconnect(&creds)),
            );
        }
        Ok(json!({ "account": account }))
    })
}

#[derive(Debug, Deserialize)]
struct OAuthCodeTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

fn exchange_mobile_oauth_code(data_dir: &str, params: &Value) -> Result<Value, String> {
    let email = req_str(params, "email")?;
    let provider = req_str(params, "provider")?.to_lowercase();
    let code = req_str(params, "code")?;
    let client_id = req_str(params, "client_id")?;
    let redirect_uri = req_str(params, "redirect_uri")?;
    let code_verifier = req_str(params, "code_verifier")?;
    let client_secret = opt_str(params, "client_secret");
    let token_url = params
        .get("token_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| match provider.as_str() {
            "gmail" => GOOGLE_TOKEN_URL.to_string(),
            "outlook" => OUTLOOK_TOKEN_URL.to_string(),
            _ => String::new(),
        });
    if token_url.is_empty() {
        return Err("unsupported oauth provider".to_string());
    }

    let token = exchange_oauth_code_blocking(
        &token_url,
        &provider,
        &code,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code_verifier,
    )?;
    let refresh_token = token
        .refresh_token
        .or_else(|| {
            params
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| "oauth response missing refresh_token".to_string())?;
    let token_expires_at = now_epoch_seconds() + token.expires_in.unwrap_or(3600);

    let mut add_params = params.clone();
    let obj = add_params
        .as_object_mut()
        .ok_or_else(|| "params must be an object".to_string())?;
    obj.insert("email".to_string(), Value::String(email));
    obj.insert("provider".to_string(), Value::String(provider));
    obj.insert(
        "access_token".to_string(),
        Value::String(token.access_token),
    );
    obj.insert("refresh_token".to_string(), Value::String(refresh_token));
    obj.insert("token_expires_at".to_string(), json!(token_expires_at));
    add_mobile_oauth_account(data_dir, &add_params)
}

fn exchange_oauth_code_blocking(
    token_url: &str,
    provider: &str,
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<OAuthCodeTokenResponse, String> {
    let mut form: Vec<(&str, &str)> = vec![
        ("client_id", client_id),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
        ("code_verifier", code_verifier),
    ];
    if !client_secret.is_empty() {
        form.push(("client_secret", client_secret));
    }
    if provider == "outlook" {
        form.push(("scope", OUTLOOK_SCOPES));
    }

    let mut resp = ureq::post(token_url)
        .config()
        .http_status_as_error(false)
        .build()
        .send_form(form)
        .map_err(|err| format!("oauth code exchange request: {err:#}"))?;
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("read oauth code exchange response: {err:#}"))?;
    if !status.is_success() {
        return Err(format!("oauth code exchange failed ({status}): {body}"));
    }
    serde_json::from_str(&body).map_err(|err| format!("parse oauth code exchange response: {err}"))
}

fn add_mobile_rss_account(data_dir: &str, params: &Value) -> Result<Value, String> {
    let feed_url = req_str(params, "feed_url")?;
    let display_name = opt_str(params, "display_name");
    with_mobile_db_mutex(data_dir, |db| {
        let account = rss::add(&db, &feed_url, &display_name).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "account": account }))
    })
}

fn add_mobile_rss_feed(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account = req_account_id(params)?;
    let feed_url = req_str(params, "feed_url")?;
    with_mobile_db_mutex(data_dir, |db| {
        rss::add_feed(&db, &account, &feed_url).map_err(|err| format!("{err:#}"))
    })
}

fn remove_mobile_rss_feed(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    with_mobile_db(data_dir, |conn| {
        rss::remove_feed(&conn, &thread_id).map_err(|err| format!("{err:#}"))
    })
}

fn move_mobile_rss_feed(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_account =
        req_str(params, "target_account").or_else(|_| req_str(params, "target_account_id"))?;
    with_mobile_db(data_dir, |conn| {
        rss::move_feed(&conn, &thread_id, &target_account).map_err(|err| format!("{err:#}"))
    })
}

fn export_mobile_opml(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account = req_account_id(params)?;
    with_mobile_db(data_dir, |conn| {
        let opml = rss::export_opml(&conn, &account).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "opml": opml }))
    })
}

fn import_mobile_opml(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account = req_account_id(params)?;
    let opml = req_str(params, "opml")?;
    with_mobile_db_mutex(data_dir, |db| {
        let imported = rss::import_opml(&db, &opml, &account).map_err(|err| format!("{err:#}"))?;
        Ok(json!({ "imported": imported }))
    })
}

fn send_mobile_message(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let to = req_str(params, "to")?;
    let cc = opt_str(params, "cc");
    let bcc = opt_str(params, "bcc");
    let subject = opt_str(params, "subject");
    let body = opt_str(params, "body");
    let html = opt_str(params, "html");
    let in_reply_to = opt_str(params, "in_reply_to");
    let references = opt_str(params, "references");
    let reply_to = opt_str(params, "reply_to");
    let message_id = opt_str(params, "message_id");
    let requested_from = opt_str(params, "from");
    let attachments = parse_mobile_attachments(params)?;

    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let (from_addr, sender_name) =
            resolve_mobile_send_from(&conn, &account_id, &creds, &requested_from);
        let raw = run_mobile_async(smtp::send(
            &creds,
            &from_addr,
            &sender_name,
            &to,
            &cc,
            &bcc,
            &subject,
            &body,
            &html,
            &attachments,
            &in_reply_to,
            &references,
            &reply_to,
            &message_id,
        ))?;
        let sent_bytes = raw.len();
        if let Err(err) = run_mobile_async(async move {
            let mut session = imap::connect(&creds).await?;
            let sent = imap::find_sent_folder(&mut session)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no Sent folder found"))?;
            imap::append_to_sent(&mut session, &sent, &raw).await?;
            let _ = session.logout().await;
            anyhow::Ok(())
        }) {
            eprintln!("meron-core: mobile APPEND to Sent failed for {account_id}: {err}");
        }
        Ok(json!({ "ok": true, "sent_bytes": sent_bytes }))
    })
}

fn save_mobile_draft(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let to = opt_str(params, "to");
    let cc = opt_str(params, "cc");
    let bcc = opt_str(params, "bcc");
    let subject = opt_str(params, "subject");
    let body = opt_str(params, "body");
    let html = opt_str(params, "html");
    let in_reply_to = opt_str(params, "in_reply_to");
    let references = opt_str(params, "references");
    let reply_to = opt_str(params, "reply_to");
    let draft_id = req_str(params, "draft_id")?;
    let requested_from = opt_str(params, "from");
    let attachments = parse_mobile_attachments(params)?;

    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let (from_addr, sender_name) =
            resolve_mobile_send_from(&conn, &account_id, &creds, &requested_from);
        let raw = smtp::build_message(
            &sender_name,
            &from_addr,
            &to,
            &cc,
            &bcc,
            true,
            &subject,
            &body,
            &html,
            &attachments,
            &in_reply_to,
            &references,
            &reply_to,
            &draft_id,
        )
        .map_err(|err| format!("{err:#}"))?;
        let saved_id = draft_id.clone();
        let saved_bytes = raw.len();
        run_mobile_async(async move {
            let mut session = imap::connect(&creds).await?;
            let drafts = imap::find_drafts_folder(&mut session)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
            imap::replace_draft(&mut session, &drafts, &raw, &saved_id).await?;
            let _ = session.logout().await;
            anyhow::Ok(())
        })?;
        Ok(json!({ "ok": true, "draft_id": draft_id, "saved_bytes": saved_bytes }))
    })
}

fn discard_mobile_draft(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let draft_id = opt_str(params, "draft_id");
    if draft_id.trim().is_empty() {
        return Ok(json!({ "ok": true, "deleted": 0, "permanent": true }));
    }
    with_mobile_db(data_dir, |conn| {
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let deleted = run_mobile_async(async move {
            let mut session = imap::connect(&creds).await?;
            let drafts = imap::find_drafts_folder(&mut session)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no Drafts folder found"))?;
            let deleted = imap::discard_draft(&mut session, &drafts, &draft_id).await?;
            let _ = session.logout().await;
            anyhow::Ok(deleted)
        })?;
        Ok(json!({ "ok": true, "deleted": deleted, "permanent": true }))
    })
}

fn sync_mobile_mail(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .unwrap_or_else(|| "INBOX".to_string());
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(50)
        .clamp(1, 500);
    let sync_folders = params
        .get("folders")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return sync_mobile_rss_with_conn(conn, account_id);
        }
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }

        let synced = run_mobile_async(async {
            let mut session = imap::connect(&creds).await?;
            let folders = if sync_folders {
                imap::list_folders(&mut session).await?
            } else {
                Vec::new()
            };
            let batch = imap::fetch_recent(&mut session, &folder, limit).await?;
            let flag_sync = imap::sync_flags(&mut session, &folder, 0, false).await.ok();
            let server_uids = imap::list_all_uids(&mut session, &folder).await.ok();
            let _ = session.logout().await;
            anyhow::Ok((folders, batch, flag_sync, server_uids))
        })?;

        let (folders, batch, flag_sync, server_uids) = synced;
        if sync_folders {
            store::upsert_folders(&conn, &account_id, &folders).map_err(|err| err.to_string())?;
        }
        let prior_validity = store::get_folder_state(&conn, &account_id, &folder)
            .map_err(|err| err.to_string())?
            .map(|(validity, _)| validity)
            .unwrap_or(0);
        if prior_validity != 0 && prior_validity != batch.uidvalidity {
            store::clear_folder_messages(&conn, &account_id, &folder)
                .map_err(|err| err.to_string())?;
        }
        let count = batch.messages.len();
        store::upsert_messages(&conn, &account_id, &folder, &batch.messages)
            .map_err(|err| err.to_string())?;
        store::ensure_folder(&conn, &account_id, &folder).map_err(|err| err.to_string())?;
        if let Some(uids) = server_uids.as_ref() {
            let validity_ok = prior_validity == 0 || prior_validity == batch.uidvalidity;
            if validity_ok {
                store::prune_missing_messages(&conn, &account_id, &folder, uids)
                    .map_err(|err| err.to_string())?;
            }
        }
        if let Some(flags) = flag_sync {
            for (uid, seen, starred) in flags.changes {
                store::update_message_seen(&conn, &account_id, &folder, uid, seen)
                    .map_err(|err| err.to_string())?;
                store::update_message_starred(&conn, &account_id, &folder, uid, starred)
                    .map_err(|err| err.to_string())?;
            }
            if flags.highest_modseq > 0 {
                store::set_folder_modseq(&conn, &account_id, &folder, flags.highest_modseq)
                    .map_err(|err| err.to_string())?;
            }
        }
        store::set_folder_state(
            &conn,
            &account_id,
            &folder,
            batch.uidvalidity,
            batch.uid_next,
        )
        .map_err(|err| err.to_string())?;

        Ok(json!({
            "ok": true,
            "account": account_id,
            "folder": folder,
            "synced": count,
            "folders": folders.len(),
        }))
    })
}

fn sync_mobile_rss(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    with_mobile_db(data_dir, |conn| sync_mobile_rss_with_conn(conn, account_id))
}

fn sync_mobile_rss_with_conn(
    conn: rusqlite::Connection,
    account_id: String,
) -> Result<Value, String> {
    if !is_rss_account(&conn, &account_id)? {
        return Err(format!("account is not RSS: {account_id}"));
    }
    let db = Mutex::new(conn);
    let synced = rss::sync_account(&db, &account_id).map_err(|err| format!("{err:#}"))?;
    Ok(json!({
        "ok": true,
        "account": account_id,
        "synced": synced,
        "rss": true,
    }))
}

fn list_mobile_folders(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            let folders = rss::folders(&conn, &account_id).map_err(|err| format!("{err:#}"))?;
            return Ok(json!({ "folders": folders }));
        }
        let folders = store::get_folders(&conn, &account_id).map_err(|err| err.to_string())?;
        let folders = folders
            .into_iter()
            .map(|folder| {
                let role = if folder.name.eq_ignore_ascii_case("INBOX") {
                    "inbox"
                } else if looks_like_archive_folder(&folder.name) {
                    "archive"
                } else {
                    "folder"
                };
                json!({
                    "id": folder.name,
                    "account_id": account_id,
                    "name": folder.name,
                    "role": role,
                    "delimiter": folder.delimiter.unwrap_or_default(),
                    "unread": folder.unread,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "folders": folders }))
    })
}

fn create_mobile_folder(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    let name = req_str(params, "name")?.trim().to_string();
    if name.is_empty() {
        return Err("Folder name is required".to_string());
    }
    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Err("RSS accounts do not support folders".to_string());
        }
        let creds = load_mobile_account_creds(&conn, &account_id)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {account_id}"));
        }
        let folders = run_mobile_async(async {
            let mut session = imap::connect(&creds).await?;
            imap::create_folder(&mut session, &name).await?;
            let folders = imap::list_folders(&mut session).await?;
            let _ = session.logout().await;
            anyhow::Ok(folders)
        })?;
        store::upsert_folders(&conn, &account_id, &folders).map_err(|err| err.to_string())?;
        list_mobile_folders(data_dir, params)
    })
}

fn list_mobile_threads(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_str(params, "account_id")?;
    let folder_id = params
        .get("folder_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(canon_folder)
        .unwrap_or_else(|| "INBOX".to_string());
    let query = params
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let filter = params
        .get("filter")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let before_cursor = params
        .get("before_cursor")
        .and_then(Value::as_str)
        .and_then(parse_mail_cursor);

    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            let threads =
                rss::recent(&conn, &account_id, &query, 50).map_err(|err| format!("{err:#}"))?;
            return Ok(json!({ "threads": threads }));
        }
        let (mut messages, next_cursor) = if query.is_empty() {
            store::get_recent_page(
                &conn,
                &account_id,
                &folder_id,
                50,
                before_cursor,
                filter == "unread",
            )
            .map_err(|err| err.to_string())?
        } else {
            (
                store::search_messages(&conn, &account_id, &folder_id, &query, 50)
                    .map_err(|err| err.to_string())?,
                None,
            )
        };
        store::apply_card_identity(&conn, &account_id, &mut messages);
        let threads = thread_cards_json(&account_id, &folder_id, messages);
        let mut out = json!({ "threads": threads });
        if let Some(cursor) = next_cursor {
            out.as_object_mut()
                .unwrap()
                .insert("next_cursor".to_string(), Value::String(cursor));
        }
        Ok(out)
    })
}

fn read_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    if is_rss_thread_id(&thread_id) {
        return read_mobile_rss_thread(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    with_mobile_db(data_dir, |conn| {
        let headers = if let Some(uid) = parsed.uid {
            let header = store::get_thread_headers(
                &conn,
                &parsed.account,
                &parsed.folder,
                &format!("uid:{uid}"),
            )
            .map_err(|err| err.to_string())?
            .into_iter()
            .find(|header| header.uid == uid);
            match header {
                Some(header) => vec![header],
                None => {
                    let cached =
                        store::get_cached_message(&conn, &parsed.account, &parsed.folder, uid)
                            .map_err(|err| err.to_string())?;
                    if cached.is_some() {
                        vec![MessageHeader {
                            uid,
                            folder: parsed.folder.clone(),
                            thread_key: format!("uid:{uid}"),
                            ..Default::default()
                        }]
                    } else {
                        Vec::new()
                    }
                }
            }
        } else {
            store::get_thread_headers_all_folders(&conn, &parsed.account, &parsed.thread_key)
                .map_err(|err| err.to_string())?
        };

        // Mobile sync stores headers only, so the first time a thread is opened
        // its message bodies aren't cached yet. Fetch them on demand from IMAP
        // (grouped by folder), cache them, then render. Best-effort: a fetch
        // failure still renders whatever is cached.
        let mut missing: std::collections::BTreeMap<String, Vec<u32>> =
            std::collections::BTreeMap::new();
        for header in &headers {
            if header.uid == 0 {
                continue;
            }
            let folder = if header.folder.is_empty() {
                parsed.folder.clone()
            } else {
                header.folder.clone()
            };
            let cached = store::has_cached_body(&conn, &parsed.account, &folder, header.uid)
                .unwrap_or(false);
            if !cached {
                missing.entry(folder).or_default().push(header.uid);
            }
        }
        if !missing.is_empty() {
            if let Ok(creds) = load_mobile_account_creds(&conn, &parsed.account) {
                if !account_needs_reconnect(&creds) {
                    let media_root = std::path::PathBuf::from(data_dir).join("attachments");
                    let fetched = run_mobile_async(async {
                        let mut session = imap::connect(&creds).await?;
                        let mut all = Vec::new();
                        for (folder, uids) in &missing {
                            let bodies = imap::fetch_bodies(
                                &mut session,
                                folder,
                                uids,
                                media_root.clone(),
                                &parsed.account,
                            )
                            .await?;
                            for (uid, message) in bodies {
                                all.push((folder.clone(), uid, message));
                            }
                        }
                        let _ = session.logout().await;
                        anyhow::Ok(all)
                    });
                    if let Ok(fetched) = fetched {
                        for (folder, uid, message) in fetched {
                            let _ = store::save_cached_message(
                                &conn,
                                &parsed.account,
                                &folder,
                                uid,
                                &message,
                            );
                        }
                    }
                }
            }
        }

        let messages = headers
            .into_iter()
            .filter_map(|header| {
                let folder = if header.folder.is_empty() {
                    parsed.folder.as_str()
                } else {
                    header.folder.as_str()
                };
                let cached =
                    store::get_cached_message(&conn, &parsed.account, folder, header.uid).ok()?;
                Some(message_json(
                    &parsed.account,
                    &thread_id,
                    folder,
                    &header,
                    cached.as_ref(),
                ))
            })
            .collect::<Vec<_>>();
        Ok(json!({ "messages": messages }))
    })
}

fn read_mobile_rss_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let limit = opt_u32(params, "limit");
    let before_cursor = params
        .get("before_cursor")
        .and_then(Value::as_str)
        .and_then(parse_rss_cursor);
    with_mobile_db(data_dir, |conn| {
        let (messages, next_cursor) =
            rss::read_thread_page(&conn, &thread_id, before_cursor, limit)
                .map_err(|err| format!("{err:#}"))?;
        let mut out = json!({ "messages": messages });
        if let Some(cursor) = next_cursor {
            out.as_object_mut()
                .unwrap()
                .insert("next_cursor".to_string(), Value::String(cursor));
        }
        Ok(out)
    })
}

fn list_mobile_starred_items(data_dir: &str, params: &Value) -> Result<Value, String> {
    let limit = opt_u32(params, "limit").unwrap_or(200);
    with_mobile_db(data_dir, |conn| {
        let mut items = store::get_starred_all_accounts(&conn, limit)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|(account, header)| starred_item_json(&account, &header))
            .collect::<Vec<_>>();
        items
            .extend(rss::starred_items(&conn, i64::from(limit)).map_err(|err| format!("{err:#}"))?);
        items.sort_by(|a, b| {
            b.get("date")
                .and_then(Value::as_i64)
                .cmp(&a.get("date").and_then(Value::as_i64))
        });
        items.truncate(limit as usize);
        Ok(json!({ "items": items }))
    })
}

fn delete_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let parsed = parse_thread_id_with_request_folder(&thread_id, params)
        .ok_or_else(|| "invalid thread_id".to_string())?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        if uids.is_empty() {
            return Ok(json!({ "ok": true, "deleted": 0 }));
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let delete_result = run_mobile_async(async {
            let mut session = imap::connect(&creds).await?;
            let drafts = imap::find_drafts_folder(&mut session).await?;
            let trash = if drafts.as_deref() == Some(parsed.folder.as_str()) {
                imap::expunge_uids(&mut session, &parsed.folder, &uids).await?;
                None
            } else {
                let trash = imap::find_trash_folder(&mut session)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Trash folder not found for this account"))?;
                if trash == parsed.folder {
                    imap::expunge_uids(&mut session, &parsed.folder, &uids).await?;
                    None
                } else {
                    imap::move_to_folder(&mut session, &parsed.folder, &trash, &uids).await?;
                    Some(trash)
                }
            };
            let _ = session.logout().await;
            anyhow::Ok(trash)
        })?;
        let deleted = store::delete_messages_by_uid(&conn, &parsed.account, &parsed.folder, &uids)
            .map_err(|err| err.to_string())?;
        match delete_result {
            None => Ok(json!({ "ok": true, "deleted": deleted, "permanent": true })),
            Some(trash) => Ok(json!({ "ok": true, "deleted": deleted, "trash": trash })),
        }
    })
}

fn archive_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    with_mobile_db(data_dir, |conn| {
        let target_folder = find_cached_archive_folder(&conn, &parsed.account)?;
        if parsed.folder == target_folder {
            return Ok(json!({
                "ok": true,
                "moved": 0,
                "folder": target_folder,
                "thread_id": thread_id,
            }));
        }
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            return Ok(json!({
                "ok": true,
                "moved": 0,
                "folder": target_folder,
                "thread_id": thread_id,
            }));
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let target_batch = run_mobile_async(async {
            let mut session = imap::connect(&creds).await?;
            imap::move_to_folder(&mut session, &parsed.folder, &target_folder, &uids).await?;
            let batch =
                imap::fetch_recent(&mut session, &target_folder, 50.max(uids.len() as u32)).await?;
            let _ = session.logout().await;
            anyhow::Ok(batch)
        })?;
        store::ensure_folder(&conn, &parsed.account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::upsert_messages(
            &conn,
            &parsed.account,
            &target_folder,
            &target_batch.messages,
        )
        .map_err(|err| err.to_string())?;
        store::set_folder_state(
            &conn,
            &parsed.account,
            &target_folder,
            target_batch.uidvalidity,
            target_batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        let moved = store::move_messages_by_uid(
            &conn,
            &parsed.account,
            &parsed.folder,
            &target_folder,
            &uids,
        )
        .map_err(|err| err.to_string())?;
        let moved_thread_id = format_thread_id(&parsed.account, &target_folder, &parsed.thread_key);
        Ok(json!({
            "ok": true,
            "moved": moved,
            "folder": target_folder,
            "thread_id": moved_thread_id,
        }))
    })
}

fn move_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_folder = canon_folder(&req_str(params, "target_folder_id")?);
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    if parsed.folder == target_folder {
        return Ok(
            json!({ "ok": true, "moved": 0, "folder": target_folder, "thread_id": thread_id }),
        );
    }
    with_mobile_db(data_dir, |conn| {
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            return Ok(
                json!({ "ok": true, "moved": 0, "folder": target_folder, "thread_id": thread_id }),
            );
        }
        let creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let target_batch = run_mobile_async(async {
            let mut session = imap::connect(&creds).await?;
            imap::move_to_folder(&mut session, &parsed.folder, &target_folder, &uids).await?;
            let batch =
                imap::fetch_recent(&mut session, &target_folder, 50.max(uids.len() as u32)).await?;
            let _ = session.logout().await;
            anyhow::Ok(batch)
        })?;
        store::ensure_folder(&conn, &parsed.account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::upsert_messages(
            &conn,
            &parsed.account,
            &target_folder,
            &target_batch.messages,
        )
        .map_err(|err| err.to_string())?;
        store::set_folder_state(
            &conn,
            &parsed.account,
            &target_folder,
            target_batch.uidvalidity,
            target_batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        let moved = store::move_messages_by_uid(
            &conn,
            &parsed.account,
            &parsed.folder,
            &target_folder,
            &uids,
        )
        .map_err(|err| err.to_string())?;
        let moved_thread_id = format_thread_id(&parsed.account, &target_folder, &parsed.thread_key);
        Ok(json!({
            "ok": true,
            "moved": moved,
            "folder": target_folder,
            "thread_id": moved_thread_id,
        }))
    })
}

fn copy_mobile_thread(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let target_account = req_str(params, "target_account_id")?;
    let target_folder = canon_folder(&req_str(params, "target_folder_id")?);
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    with_mobile_db(data_dir, |conn| {
        let uids = cached_thread_uids(&conn, &parsed)?;
        if uids.is_empty() {
            return Ok(json!({
                "ok": true,
                "copied": 0,
                "source_folder": parsed.folder,
                "target_account": target_account,
                "target_folder": target_folder,
            }));
        }
        let source_creds = load_mobile_account_creds(&conn, &parsed.account)?;
        if account_needs_reconnect(&source_creds) {
            return Err(format!("account needs reconnect: {}", parsed.account));
        }
        let target_creds = load_mobile_account_creds(&conn, &target_account)?;
        if account_needs_reconnect(&target_creds) {
            return Err(format!("account needs reconnect: {target_account}"));
        }
        let target_batch = run_mobile_async(async {
            let raw_messages = {
                let mut session = imap::connect(&source_creds).await?;
                let messages =
                    imap::fetch_raw_messages_for_copy(&mut session, &parsed.folder, &uids).await?;
                let _ = session.logout().await;
                messages
            };
            let copied = raw_messages.len();
            let mut session = imap::connect(&target_creds).await?;
            for message in &raw_messages {
                imap::append_copied_message(&mut session, &target_folder, message).await?;
            }
            let batch =
                imap::fetch_recent(&mut session, &target_folder, 50.max(copied as u32)).await?;
            let _ = session.logout().await;
            anyhow::Ok((copied, batch))
        })?;
        let (copied, batch) = target_batch;
        store::ensure_folder(&conn, &target_account, &target_folder)
            .map_err(|err| err.to_string())?;
        store::upsert_messages(&conn, &target_account, &target_folder, &batch.messages)
            .map_err(|err| err.to_string())?;
        store::set_folder_state(
            &conn,
            &target_account,
            &target_folder,
            batch.uidvalidity,
            batch.uid_next,
        )
        .map_err(|err| err.to_string())?;
        Ok(json!({
            "ok": true,
            "copied": copied,
            "source_folder": parsed.folder,
            "target_account": target_account,
            "target_folder": target_folder,
        }))
    })
}

fn mark_mobile_thread_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let seen = req_bool(params, "seen")?;
    if is_rss_thread_id(&thread_id) {
        return mark_mobile_rss_thread_read(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        if !uids.is_empty() {
            let creds = load_mobile_account_creds(&conn, &parsed.account)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {}", parsed.account));
            }
            run_mobile_async(async {
                let mut session = imap::connect(&creds).await?;
                imap::set_seen(&mut session, &parsed.folder, &uids, seen).await?;
                let _ = session.logout().await;
                anyhow::Ok(())
            })?;
        }
        if has_requested_mobile_message_ids(params) {
            for uid in uids {
                store::update_message_seen(&conn, &parsed.account, &parsed.folder, uid, seen)
                    .map_err(|err| err.to_string())?;
            }
        } else if let Some(uid) = parsed.uid {
            store::update_message_seen(&conn, &parsed.account, &parsed.folder, uid, seen)
                .map_err(|err| err.to_string())?;
        } else {
            store::update_thread_seen(
                &conn,
                &parsed.account,
                &parsed.folder,
                &parsed.thread_key,
                seen,
            )
            .map_err(|err| err.to_string())?;
        }
        Ok(json!({ "ok": true }))
    })
}

fn mark_mobile_folder_all_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let account_id = req_account_id(params)?;
    let folder = params
        .get("folder_id")
        .or_else(|| params.get("folder"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(canon_folder)
        .unwrap_or_else(|| "INBOX".to_string());

    with_mobile_db(data_dir, |conn| {
        if is_rss_account(&conn, &account_id)? {
            return Ok(json!({ "ok": true, "updated": 0, "rss": true }));
        }
        let uids =
            store::get_unseen_uids(&conn, &account_id, &folder).map_err(|err| err.to_string())?;
        if !uids.is_empty() {
            let creds = load_mobile_account_creds(&conn, &account_id)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {account_id}"));
            }
            run_mobile_async(async {
                let mut session = imap::connect(&creds).await?;
                imap::set_seen(&mut session, &folder, &uids, true).await?;
                let _ = session.logout().await;
                anyhow::Ok(())
            })?;
        }
        store::mark_folder_seen(&conn, &account_id, &folder, true)
            .map_err(|err| err.to_string())?;
        Ok(json!({ "ok": true, "updated": uids.len(), "folder": folder }))
    })
}

fn mark_mobile_thread_starred(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let starred = req_bool(params, "starred")?;
    if is_rss_thread_id(&thread_id) {
        return mark_mobile_rss_thread_starred(data_dir, params);
    }
    let parsed = parse_thread_id(&thread_id).ok_or_else(|| "invalid thread_id".to_string())?;
    with_mobile_db(data_dir, |conn| {
        let uids = requested_mobile_uids(&conn, &parsed, params)?;
        if !uids.is_empty() {
            let creds = load_mobile_account_creds(&conn, &parsed.account)?;
            if account_needs_reconnect(&creds) {
                return Err(format!("account needs reconnect: {}", parsed.account));
            }
            run_mobile_async(async {
                let mut session = imap::connect(&creds).await?;
                imap::set_starred(&mut session, &parsed.folder, &uids, starred).await?;
                let _ = session.logout().await;
                anyhow::Ok(())
            })?;
        }
        if has_requested_mobile_message_ids(params) {
            for uid in uids {
                store::update_message_starred(&conn, &parsed.account, &parsed.folder, uid, starred)
                    .map_err(|err| err.to_string())?;
            }
        } else if let Some(uid) = parsed.uid {
            store::update_message_starred(&conn, &parsed.account, &parsed.folder, uid, starred)
                .map_err(|err| err.to_string())?;
        } else {
            store::update_thread_starred(
                &conn,
                &parsed.account,
                &parsed.folder,
                &parsed.thread_key,
                starred,
            )
            .map_err(|err| err.to_string())?;
        }
        Ok(json!({ "ok": true }))
    })
}

fn mark_mobile_rss_thread_read(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let seen = params.get("seen").and_then(Value::as_bool).unwrap_or(true);
    let item_keys = rss_item_keys(params, &thread_id);
    with_mobile_db(data_dir, |conn| {
        if item_keys.is_empty() {
            rss::mark_thread_read(&conn, &thread_id, seen).map_err(|err| format!("{err:#}"))?;
        } else {
            rss::mark_items_read(&conn, &thread_id, &item_keys, seen)
                .map_err(|err| format!("{err:#}"))?;
        }
        Ok(json!({ "ok": true }))
    })
}

fn mark_mobile_rss_thread_starred(data_dir: &str, params: &Value) -> Result<Value, String> {
    let thread_id = req_str(params, "thread_id")?;
    let starred = params
        .get("starred")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let item_keys = rss_item_keys(params, &thread_id);
    with_mobile_db(data_dir, |conn| {
        if item_keys.is_empty() {
            rss::mark_thread_starred(&conn, &thread_id, starred)
                .map_err(|err| format!("{err:#}"))?;
        } else {
            rss::mark_items_starred(&conn, &thread_id, &item_keys, starred)
                .map_err(|err| format!("{err:#}"))?;
        }
        Ok(json!({ "ok": true }))
    })
}

fn cached_thread_uids(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
) -> Result<Vec<u32>, String> {
    if let Some(uid) = parsed.uid {
        return Ok(vec![uid]);
    }
    Ok(
        store::get_thread_headers(conn, &parsed.account, &parsed.folder, &parsed.thread_key)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|header| header.uid)
            .collect(),
    )
}

fn has_requested_mobile_message_ids(params: &Value) -> bool {
    params
        .get("message_ids")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str().is_some_and(|value| !value.trim().is_empty()))
        })
}

fn requested_mobile_uids(
    conn: &rusqlite::Connection,
    parsed: &ParsedThreadId,
    params: &Value,
) -> Result<Vec<u32>, String> {
    let uids = params
        .get("message_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(mobile_message_id_uid)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if has_requested_mobile_message_ids(params) {
        return Ok(uids);
    }
    cached_thread_uids(conn, parsed)
}

fn mobile_message_id_uid(value: &str) -> Option<u32> {
    value
        .trim()
        .rsplit('#')
        .next()
        .and_then(|uid| uid.parse::<u32>().ok())
}

fn find_cached_archive_folder(
    conn: &rusqlite::Connection,
    account: &str,
) -> Result<String, String> {
    store::get_folders(conn, account)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|folder| looks_like_archive_folder(&folder.name))
        .map(|folder| folder.name)
        .ok_or_else(|| "Archive folder not found for this account".to_string())
}

fn req_account_id(params: &Value) -> Result<String, String> {
    req_str(params, "account_id").or_else(|_| req_str(params, "account"))
}

fn is_rss_account(conn: &rusqlite::Connection, account_id: &str) -> Result<bool, String> {
    Ok(store::account_engine(conn, account_id)
        .map_err(|err| err.to_string())?
        .as_deref()
        == Some("rss"))
}

fn account_needs_reconnect(creds: &Creds) -> bool {
    if creds.is_oauth() {
        !creds
            .refresh_token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty())
    } else {
        creds.password.trim().is_empty()
    }
}

/// Mobile secret persistence. Unlike desktop (OS keychain via `secrets`),
/// mobile has no keychain, so the per-account secret blob lives in the
/// app-private SQLite DB. Keep these the only secret read/write on the mobile
/// path so password/token persistence stays consistent.
fn store_mobile_secret(
    conn: &rusqlite::Connection,
    account_id: &str,
    creds: &Creds,
) -> Result<(), String> {
    let blob = serde_json::to_string(&Secrets::from_creds(creds)).map_err(|err| err.to_string())?;
    store::upsert_secret(conn, account_id, &blob).map_err(|err| err.to_string())
}

fn load_mobile_secret(conn: &rusqlite::Connection, account_id: &str) -> Secrets {
    store::load_secret(conn, account_id)
        .ok()
        .flatten()
        .and_then(|blob| serde_json::from_str::<Secrets>(&blob).ok())
        .unwrap_or_default()
}

fn load_mobile_account_creds(
    conn: &rusqlite::Connection,
    account_id: &str,
) -> Result<Creds, String> {
    let mut creds = store::load_accounts(conn)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|(id, _)| id == account_id)
        .map(|(_, creds)| creds)
        .ok_or_else(|| format!("account not found: {account_id}"))?;
    load_mobile_secret(conn, account_id).apply_to(&mut creds);
    Ok(creds)
}

fn parse_mobile_attachments(params: &Value) -> Result<Vec<AttachmentInput>, String> {
    match params.get("attachments") {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                serde_json::from_value::<AttachmentInput>(value.clone())
                    .map_err(|err| format!("invalid attachment: {err}"))
            })
            .collect(),
        Some(_) => Err("attachments must be an array".to_string()),
        None => Ok(Vec::new()),
    }
}

fn resolve_mobile_send_from(
    conn: &rusqlite::Connection,
    account_id: &str,
    creds: &Creds,
    requested_from: &str,
) -> (String, String) {
    let sender_name = conn
        .query_row(
            "SELECT sender_name FROM accounts WHERE id = ?1",
            rusqlite::params![account_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default();
    let aliases = store::account_aliases(conn, account_id).unwrap_or_default();
    let requested = requested_from.trim().to_lowercase();
    if requested.is_empty() || requested == creds.user.trim().to_lowercase() {
        return (creds.user.clone(), sender_name);
    }
    if let Some(alias) = aliases
        .iter()
        .find(|alias| alias.email.trim().to_lowercase() == requested)
    {
        let name = if alias.name.trim().is_empty() {
            sender_name
        } else {
            alias.name.clone()
        };
        return (alias.email.trim().to_string(), name);
    }
    (creds.user.clone(), sender_name)
}

fn run_mobile_async<F, T>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("tokio runtime: {err}"))?
        .block_on(future)
        .map_err(|err| format!("{err:#}"))
}

fn with_mobile_db<F>(data_dir: &str, f: F) -> Result<Value, String>
where
    F: FnOnce(rusqlite::Connection) -> Result<Value, String>,
{
    let data_dir = data_dir.trim();
    if data_dir.is_empty() {
        return Err("mobile core is not initialized".to_string());
    }
    let db_path = Path::new(data_dir).join("meron.db");
    let conn = match mobile_db_key() {
        Some(key) => store::open_at_keyed(&db_path, &key),
        // No key set (tests/unkeyed init): plaintext, matching prior behavior.
        None => store::open_at(&db_path),
    }
    .map_err(|err| err.to_string())?;
    f(conn)
}

/// SQLCipher key for the mobile store. The platform host (Android Keystore /
/// iOS Keychain) owns the key and passes it through the keyed `init` FFI; we
/// keep it process-global so every `with_mobile_db` opens the encrypted store
/// without threading the key through every command handler.
static MOBILE_DB_KEY: Mutex<Option<String>> = Mutex::new(None);

/// Set (or clear, with an empty/`None` key) the mobile store's SQLCipher key.
pub fn set_mobile_db_key(key: Option<String>) {
    *MOBILE_DB_KEY.lock().unwrap() = key.filter(|k| !k.is_empty());
}

fn mobile_db_key() -> Option<String> {
    MOBILE_DB_KEY.lock().unwrap().clone()
}

fn with_mobile_db_mutex<F>(data_dir: &str, f: F) -> Result<Value, String>
where
    F: FnOnce(Mutex<rusqlite::Connection>) -> Result<Value, String>,
{
    with_mobile_db(data_dir, |conn| f(Mutex::new(conn)))
}

fn account_id(email: &str) -> String {
    email.trim().to_lowercase()
}

fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn tls_mode(tls: bool, port: u16) -> (bool, bool) {
    if !tls {
        return (false, false);
    }
    match port {
        3143 | 3587 => (false, false),
        143 | 25 | 587 => (false, true),
        _ => (true, false),
    }
}

fn req_str(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{key} required"))
}

fn req_str_array(params: &Value, key: &str) -> Result<Vec<String>, String> {
    let values = params
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} required"))?;
    Ok(values
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect())
}

fn opt_str(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn req_u16(params: &Value, key: &str) -> Option<u16> {
    params.get(key)?.as_u64()?.try_into().ok()
}

fn opt_u32(params: &Value, key: &str) -> Option<u32> {
    params.get(key)?.as_u64()?.try_into().ok()
}

fn req_bool(params: &Value, key: &str) -> Result<bool, String> {
    params
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{key} required"))
}

fn canon_folder(folder: &str) -> String {
    if folder.eq_ignore_ascii_case("inbox") {
        "INBOX".to_string()
    } else {
        folder.to_string()
    }
}

fn looks_like_archive_folder(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "archive"
            | "archives"
            | "all mail"
            | "inbox.archive"
            | "inbox.archives"
            | "[gmail]/all mail"
            | "[google mail]/all mail"
    )
}

fn parse_mail_cursor(cursor: &str) -> Option<(i64, u32)> {
    let parts = cursor.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "date" {
        return None;
    }
    Some((parts[1].parse().ok()?, parts[2].parse().ok()?))
}

fn parse_rss_cursor(cursor: &str) -> Option<(i64, String)> {
    let rest = cursor.strip_prefix("ts:")?;
    let (ts, key) = rest.split_once(':')?;
    Some((ts.parse().ok()?, key.to_string()))
}

fn is_rss_thread_id(thread_id: &str) -> bool {
    thread_id.contains("#rss#")
}

fn rss_item_keys(params: &Value, thread_id: &str) -> Vec<String> {
    params
        .get("item_keys")
        .or_else(|| params.get("message_ids"))
        .and_then(Value::as_array)
        .map(|items| {
            let item_prefix = format!("{thread_id}#");
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|raw| {
                    let value = raw.trim();
                    if value.is_empty() {
                        None
                    } else {
                        Some(
                            value
                                .strip_prefix(&item_prefix)
                                .unwrap_or(value)
                                .to_string(),
                        )
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn thread_cards_json(
    account_id: &str,
    folder_id: &str,
    messages: Vec<MessageHeader>,
) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen_keys = std::collections::HashSet::new();
    for message in messages {
        let thread_key = if message.thread_key.is_empty() {
            format!("uid:{}", message.uid)
        } else {
            message.thread_key.clone()
        };
        if !seen_keys.insert(thread_key.clone()) {
            continue;
        }
        let message_folder = if message.folder.is_empty() {
            folder_id.to_string()
        } else {
            message.folder.clone()
        };
        let thread_id = format_thread_id(account_id, &message_folder, &thread_key);
        out.push(json!({
            "id": thread_id,
            "account_id": account_id,
            "folder_id": message_folder,
            "thread_id": thread_id,
            "from_name": message.from_name,
            "from_addr": message.from_addr,
            "to": "",
            "subject": message.subject,
            "preview": "",
            "body": "",
            "date": message.date,
            "unread": !message.seen,
            "unread_count": if message.seen { 0 } else { 1 },
            "starred": message.starred,
            "has_attachments": false,
            "recipient_overflow": message.recipient_overflow,
        }));
    }
    out
}

fn format_thread_id(account_id: &str, folder: &str, thread_key: &str) -> String {
    if let Some(uid) = thread_key.strip_prefix("uid:") {
        return format!("{account_id}#{}#{uid}", canon_folder(folder));
    }
    let encoded = URL_SAFE_NO_PAD.encode(thread_key.as_bytes());
    format!("{account_id}#{}#t.{encoded}", canon_folder(folder))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedThreadId {
    account: String,
    folder: String,
    thread_key: String,
    uid: Option<u32>,
}

fn parse_thread_id(thread_id: &str) -> Option<ParsedThreadId> {
    let first = thread_id.find('#')?;
    let last = thread_id.rfind('#')?;
    if first == 0 || last <= first {
        return None;
    }
    let account = thread_id[..first].to_string();
    let folder = thread_id[first + 1..last].to_string();
    let key = &thread_id[last + 1..];
    if let Some(encoded) = key.strip_prefix("t.") {
        let decoded = URL_SAFE_NO_PAD.decode(encoded.as_bytes()).ok()?;
        let thread_key = String::from_utf8(decoded).ok()?;
        return Some(ParsedThreadId {
            account,
            folder,
            thread_key,
            uid: None,
        });
    }
    let uid = key.parse::<u32>().ok()?;
    Some(ParsedThreadId {
        account,
        folder,
        thread_key: format!("uid:{uid}"),
        uid: Some(uid),
    })
}

fn parse_thread_id_with_request_folder(thread_id: &str, params: &Value) -> Option<ParsedThreadId> {
    let mut parsed = parse_thread_id(thread_id)?;
    if let Some(folder) = params
        .get("folder")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parsed.folder = folder.to_string();
    }
    Some(parsed)
}

fn message_json(
    account_id: &str,
    thread_id: &str,
    folder: &str,
    header: &MessageHeader,
    cached: Option<&Message>,
) -> Value {
    let id = format!("{thread_id}#{}", header.uid);
    json!({
        "id": id,
        "account_id": account_id,
        "folder_id": folder,
        "thread_id": thread_id,
        "from_name": cached.map(|message| message.from_name.as_str()).unwrap_or(header.from_name.as_str()),
        "from_addr": cached.map(|message| message.from_addr.as_str()).unwrap_or(header.from_addr.as_str()),
        "to": cached.map(|message| message.to.as_str()).unwrap_or(""),
        "reply_to": cached.map(|message| message.reply_to.as_str()).unwrap_or(""),
        "cc": cached.map(|message| message.cc.as_str()).unwrap_or(""),
        "bcc": cached.map(|message| message.bcc.as_str()).unwrap_or(""),
        "message_id": cached.map(|message| message.message_id.as_str()).unwrap_or(""),
        "references": cached.map(|message| message.references.as_str()).unwrap_or(""),
        "subject": cached.map(|message| message.subject.as_str()).unwrap_or(header.subject.as_str()),
        "preview": cached.map(|message| message.preview.as_str()).unwrap_or(""),
        "body": cached.map(|message| message.body.as_str()).unwrap_or(""),
        "body_html": cached.and_then(|message| message.body_html.as_deref()).unwrap_or(""),
        "date": cached.map(|message| message.date).unwrap_or(header.date),
        "unread": !header.seen,
        "starred": header.starred,
        "has_attachments": cached.map(|message| !message.attachments.is_empty()).unwrap_or(false),
        "attachments": cached
            .map(|message| serde_json::to_value(&message.attachments).unwrap_or_else(|_| json!([])))
            .unwrap_or_else(|| json!([])),
    })
}

fn starred_item_json(account_id: &str, header: &MessageHeader) -> Value {
    let folder = if header.folder.is_empty() {
        "INBOX"
    } else {
        header.folder.as_str()
    };
    let thread_key = if header.thread_key.is_empty() {
        format!("uid:{}", header.uid)
    } else {
        header.thread_key.clone()
    };
    let thread_id = format_thread_id(account_id, folder, &thread_key);
    json!({
        "id": format!("{thread_id}#{}", header.uid),
        "account_id": account_id,
        "folder_id": folder,
        "thread_id": thread_id,
        "from_name": header.from_name.as_str(),
        "from_addr": header.from_addr.as_str(),
        "to": "",
        "reply_to": "",
        "cc": "",
        "bcc": "",
        "message_id": "",
        "references": "",
        "subject": header.subject.as_str(),
        "preview": "",
        "body": "",
        "body_html": "",
        "date": header.date,
        "unread": !header.seen,
        "starred": true,
        "has_attachments": false,
        "attachments": [],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::{Folder, MessageHeader, Recipient};
    use crate::store::{RssItemExtra, RssMedia};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn request_defaults_missing_params_to_null() {
        let req: Request = serde_json::from_str(r#"{"id":7,"method":"ping"}"#).unwrap();
        assert_eq!(req.id, 7);
        assert_eq!(req.method, "ping");
        assert_eq!(req.params, Value::Null);
    }

    #[test]
    fn ready_and_ping_expose_protocol_version() {
        assert_eq!(ready_event()["protocol"], PROTOCOL_VERSION);
        assert_eq!(ping_response()["protocol"], PROTOCOL_VERSION);
        assert_eq!(ping_response()["version"], VERSION);
    }

    #[test]
    fn protocol_invoke_wraps_ping_response() {
        let value = invoke_protocol_json(r#"{"id":42,"method":"ping"}"#);
        assert_eq!(value["id"], 42);
        assert_eq!(value["result"]["pong"], true);
        assert_eq!(value["result"]["protocol"], PROTOCOL_VERSION);
    }

    #[test]
    fn protocol_invoke_wraps_empty_account_list() {
        let value = invoke_protocol_json(r#"{"id":43,"method":"account.list","params":{}}"#);
        assert_eq!(value["id"], 43);
        assert_eq!(
            value["result"],
            json!({
                "accounts": []
            })
        );
    }

    #[test]
    fn mobile_protocol_persists_password_account_metadata() {
        let data_dir = unique_data_dir("account");
        let unique = unique_test_suffix();
        let email = format!("me+{unique}@example.com");
        let request = format!(
            r#"{{"id":60,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Me","sender_name":"Sender","imap_host":"imap.example.com","imap_port":993,"smtp_host":"smtp.example.com","smtp_port":587,"username":"{email}","password":"secret","tls":true}}}}"#
        );
        let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
        assert_eq!(added["id"], 60);
        assert_eq!(added["result"]["account"]["id"], email.as_str());
        assert_eq!(added["result"]["account"]["email"], email.as_str());
        assert_eq!(added["result"]["account"]["display_name"], "Me");
        assert_eq!(added["result"]["account"]["sender_name"], "Sender");
        assert_eq!(added["result"]["account"]["provider"], "custom");
        assert_eq!(added["result"]["account"]["auth_type"], "password");
        assert_eq!(added["result"]["account"]["imap_host"], "imap.example.com");
        assert_eq!(added["result"]["account"]["imap_port"], 993);
        assert_eq!(added["result"]["account"]["smtp_host"], "smtp.example.com");
        assert_eq!(added["result"]["account"]["smtp_port"], 587);
        assert_eq!(added["result"]["account"]["tls"], true);
        assert_eq!(added["result"]["account"]["needs_reconnect"], false);

        let listed = invoke_mobile_protocol_json(
            r#"{"id":61,"method":"account.list","params":{}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(listed["id"], 61);
        assert_eq!(listed["result"]["accounts"][0]["id"], email.as_str());
        assert_eq!(listed["result"]["accounts"][0]["needs_reconnect"], false);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_updates_account_settings_and_aliases() {
        let data_dir = unique_data_dir("account-settings");
        let unique = unique_test_suffix();
        let email = format!("settings+{unique}@example.com");
        let data_dir_str = data_dir.to_str().unwrap();
        let add_request = format!(
            r#"{{"id":70,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Old","sender_name":"Old Sender","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
        );
        assert!(
            invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
                .get("error")
                .is_none()
        );

        let commands = [
            format!(
                r#"{{"id":71,"method":"account.setName","params":{{"id":"{email}","name":"Personal"}}}}"#
            ),
            format!(
                r#"{{"id":72,"method":"account.setSenderName","params":{{"id":"{email}","name":"Sender"}}}}"#
            ),
            format!(
                r#"{{"id":73,"method":"account.setAvatar","params":{{"id":"{email}","avatar_url":"https://example.com/avatar.png"}}}}"#
            ),
            format!(
                r#"{{"id":74,"method":"account.setChatWallpaper","params":{{"id":"{email}","wallpaper":{{"kind":"preset","presetId":"grid"}}}}}}"#
            ),
            format!(
                r#"{{"id":75,"method":"account.setImages","params":{{"id":"{email}","enabled":true}}}}"#
            ),
            format!(
                r#"{{"id":76,"method":"account.setConversationHtml","params":{{"id":"{email}","enabled":false}}}}"#
            ),
            format!(
                r#"{{"id":77,"method":"account.setUnified","params":{{"id":"{email}","enabled":false}}}}"#
            ),
            format!(
                r#"{{"id":78,"method":"account.setMuted","params":{{"id":"{email}","enabled":true}}}}"#
            ),
            format!(
                r#"{{"id":79,"method":"account.setPaused","params":{{"id":"{email}","enabled":true}}}}"#
            ),
            format!(
                r#"{{"id":80,"method":"account.setRSSSyncInterval","params":{{"id":"{email}","minutes":30}}}}"#
            ),
            format!(
                r#"{{"id":81,"method":"account.setAliases","params":{{"id":"{email}","aliases":[{{"email":"alias@example.com","name":"Alias"}},{{"email":"alias@example.com","name":"Duplicate"}},{{"email":"   ","name":"Blank"}}]}}}}"#
            ),
        ];
        for command in commands {
            let value = invoke_mobile_protocol_json(&command, Some(data_dir_str));
            assert!(value.get("error").is_none(), "{value}");
        }

        let listed = invoke_mobile_protocol_json(
            r#"{"id":82,"method":"account.list","params":{}}"#,
            Some(data_dir_str),
        );
        let account = &listed["result"]["accounts"][0];
        assert_eq!(account["display_name"], "Personal", "{listed}");
        assert_eq!(account["sender_name"], "Sender", "{listed}");
        assert_eq!(
            account["avatar_url"], "https://example.com/avatar.png",
            "{listed}"
        );
        assert_eq!(account["chat_wallpaper"]["kind"], "preset", "{listed}");
        assert_eq!(account["chat_wallpaper"]["presetId"], "grid", "{listed}");
        assert_eq!(account["load_remote_images"], true, "{listed}");
        assert_eq!(account["conversation_html"], false, "{listed}");
        assert_eq!(account["included_in_unified"], false, "{listed}");
        assert_eq!(account["muted"], true, "{listed}");
        assert_eq!(account["paused"], true, "{listed}");
        assert_eq!(account["aliases"].as_array().unwrap().len(), 1, "{listed}");
        assert_eq!(
            account["aliases"][0]["email"], "alias@example.com",
            "{listed}"
        );
        assert_eq!(account["aliases"][0]["name"], "Alias", "{listed}");

        let remove =
            format!(r#"{{"id":83,"method":"account.remove","params":{{"id":"{email}"}}}}"#);
        assert!(
            invoke_mobile_protocol_json(&remove, Some(data_dir_str))
                .get("error")
                .is_none()
        );
        let empty = invoke_mobile_protocol_json(
            r#"{"id":84,"method":"account.list","params":{}}"#,
            Some(data_dir_str),
        );
        assert_eq!(empty["result"]["accounts"].as_array().unwrap().len(), 0);

        seed_rss_account(&data_dir, "rss-settings", "Feeds");
        let interval = invoke_mobile_protocol_json(
            r#"{"id":85,"method":"account.setRSSSyncInterval","params":{"id":"rss-settings","minutes":30}}"#,
            Some(data_dir_str),
        );
        assert!(interval.get("error").is_none(), "{interval}");
        let rss_listed = invoke_mobile_protocol_json(
            r#"{"id":84,"method":"account.list","params":{}}"#,
            Some(data_dir_str),
        );
        assert_eq!(
            rss_listed["result"]["accounts"][0]["rss_sync_interval_minutes"], 30,
            "{rss_listed}"
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_writes_account_media_files() {
        let data_dir = unique_data_dir("account-media");
        let unique = unique_test_suffix();
        let email = format!("media+{unique}@example.com");
        let data_dir_str = data_dir.to_str().unwrap();
        let add_request = format!(
            r#"{{"id":85,"method":"account.addPassword","params":{{"email":"{email}","display_name":"Media","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
        );
        assert!(
            invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
                .get("error")
                .is_none()
        );

        let avatar_request = format!(
            r#"{{"id":86,"method":"account.writeAvatarFile","params":{{"id":"{email}","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}}}}"#
        );
        let avatar = invoke_mobile_protocol_json(&avatar_request, Some(data_dir_str));
        let avatar_url = avatar["result"]["url"].as_str().unwrap();
        assert!(avatar_url.starts_with("/media/avatars/"), "{avatar}");
        assert!(
            data_dir.join(avatar_url.trim_start_matches('/')).exists(),
            "{avatar_url}"
        );

        let wallpaper_request = format!(
            r#"{{"id":87,"method":"account.writeChatWallpaperFile","params":{{"id":"{email}","filename":"wallpaper.webp","mime":"image/webp","data":"d2FsbA=="}}}}"#
        );
        let wallpaper = invoke_mobile_protocol_json(&wallpaper_request, Some(data_dir_str));
        let wallpaper_url = wallpaper["result"]["url"].as_str().unwrap();
        assert!(
            wallpaper_url.starts_with("/media/wallpapers/"),
            "{wallpaper}"
        );
        assert!(
            data_dir
                .join(wallpaper_url.trim_start_matches('/'))
                .exists(),
            "{wallpaper_url}"
        );

        let set_wallpaper = format!(
            r#"{{"id":88,"method":"account.setChatWallpaper","params":{{"id":"{email}","wallpaper":{{"kind":"custom","url":"{wallpaper_url}"}}}}}}"#
        );
        assert!(
            invoke_mobile_protocol_json(&set_wallpaper, Some(data_dir_str))
                .get("error")
                .is_none()
        );

        let listed = invoke_mobile_protocol_json(
            r#"{"id":89,"method":"account.list","params":{}}"#,
            Some(data_dir_str),
        );
        assert_eq!(
            listed["result"]["accounts"][0]["chat_wallpaper"]["url"],
            wallpaper_url
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_reads_cached_attachment_file() {
        let data_dir = unique_data_dir("attachment-read");
        let data_dir_str = data_dir.to_str().unwrap();
        let attachment_dir = data_dir
            .join("attachments")
            .join("acc")
            .join("INBOX")
            .join("1");
        std::fs::create_dir_all(&attachment_dir).unwrap();
        std::fs::write(attachment_dir.join("note.txt"), b"Hi").unwrap();

        let read = invoke_mobile_protocol_json(
            r#"{"id":90,"method":"mail.attachmentRead","params":{"key":"acc/INBOX/1/note.txt"}}"#,
            Some(data_dir_str),
        );
        assert_eq!(read["id"], 90);
        assert_eq!(read["result"]["data"], STANDARD.encode(b"Hi"));

        let desktop_alias_read = invoke_mobile_protocol_json(
            r#"{"id":92,"method":"mail.readAttachment","params":{"key":"acc/INBOX/1/note.txt"}}"#,
            Some(data_dir_str),
        );
        assert_eq!(desktop_alias_read["id"], 92);
        assert_eq!(desktop_alias_read["result"]["data"], STANDARD.encode(b"Hi"));

        let rejected = invoke_mobile_protocol_json(
            r#"{"id":91,"method":"mail.attachmentRead","params":{"key":"../meron.db"}}"#,
            Some(data_dir_str),
        );
        assert!(
            rejected["error"]["message"]
                .as_str()
                .unwrap()
                .contains("invalid attachment key")
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_reports_and_clears_storage_cache() {
        let data_dir = unique_data_dir("storage-cache");
        let data_dir_str = data_dir.to_str().unwrap();
        seed_mobile_account(&data_dir, "me@example.com");
        let attachment_dir = data_dir
            .join("attachments")
            .join("acc")
            .join("INBOX")
            .join("1");
        std::fs::create_dir_all(&attachment_dir).unwrap();
        std::fs::write(attachment_dir.join("one.txt"), b"Hi").unwrap();
        std::fs::write(attachment_dir.join("two.txt"), b"There").unwrap();

        let usage = invoke_mobile_protocol_json(
            r#"{"id":92,"method":"storage.usage"}"#,
            Some(data_dir_str),
        );
        assert_eq!(usage["id"], 92);
        assert_eq!(usage["result"]["cacheBytes"], 7);
        assert!(usage["result"]["dbBytes"].as_u64().unwrap() > 0, "{usage}");

        let cleared = invoke_mobile_protocol_json(
            r#"{"id":93,"method":"storage.clearCache"}"#,
            Some(data_dir_str),
        );
        assert_eq!(cleared["id"], 93);
        assert_eq!(cleared["result"]["cacheBytes"], 0);
        assert!(
            cleared["result"]["dbBytes"].as_u64().unwrap() > 0,
            "{cleared}"
        );
        assert!(data_dir.join("attachments").exists());
        assert!(data_dir.join("meron.db").exists());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_reorders_accounts() {
        let data_dir = unique_data_dir("account-reorder");
        let data_dir_str = data_dir.to_str().unwrap();
        for email in [
            "first@example.com",
            "second@example.com",
            "third@example.com",
        ] {
            let add_request = format!(
                r#"{{"id":90,"method":"account.addPassword","params":{{"email":"{email}","display_name":"{email}","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"{email}","password":"secret"}}}}"#
            );
            assert!(
                invoke_mobile_protocol_json(&add_request, Some(data_dir_str))
                    .get("error")
                    .is_none()
            );
        }

        let reorder = invoke_mobile_protocol_json(
            r#"{"id":91,"method":"account.reorder","params":{"accounts":["third@example.com","first@example.com","second@example.com"]}}"#,
            Some(data_dir_str),
        );
        assert_eq!(reorder["result"]["ok"], true, "{reorder}");

        let listed = invoke_mobile_protocol_json(
            r#"{"id":92,"method":"account.list","params":{}}"#,
            Some(data_dir_str),
        );
        let accounts = listed["result"]["accounts"].as_array().unwrap();
        let ids: Vec<_> = accounts
            .iter()
            .map(|account| account["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            vec![
                "third@example.com",
                "first@example.com",
                "second@example.com"
            ]
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_suggests_contacts_from_cached_messages() {
        let data_dir = unique_data_dir("contact-suggest");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 1,
                    subject: "Hello".to_string(),
                    from_name: "Aki".to_string(),
                    from_addr: "aki@example.com".to_string(),
                    date: 100,
                    thread_key: "one".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 2,
                    subject: "Again".to_string(),
                    from_name: "Aki".to_string(),
                    from_addr: "aki@example.com".to_string(),
                    date: 200,
                    thread_key: "two".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 3,
                    subject: "Plan".to_string(),
                    from_name: "Bea".to_string(),
                    from_addr: "bea@example.com".to_string(),
                    date: 300,
                    thread_key: "three".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        drop(conn);

        let suggestions = invoke_mobile_protocol_json(
            r#"{"id":93,"method":"mail.suggestContacts","params":{"account":"me@example.com","query":"","limit":2}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(
            suggestions["result"]["contacts"][0]["addr"], "aki@example.com",
            "{suggestions}"
        );
        assert_eq!(
            suggestions["result"]["contacts"][0]["name"], "Aki",
            "{suggestions}"
        );
        assert_eq!(
            suggestions["result"]["contacts"][1]["addr"], "bea@example.com",
            "{suggestions}"
        );

        let filtered = invoke_mobile_protocol_json(
            r#"{"id":94,"method":"contacts.suggest","params":{"account_id":"me@example.com","query":"bea","limit":8}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(filtered["result"]["contacts"].as_array().unwrap().len(), 1);
        assert_eq!(
            filtered["result"]["contacts"][0]["addr"], "bea@example.com",
            "{filtered}"
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_persists_oauth_account_metadata() {
        let data_dir = unique_data_dir("oauth-account");
        let unique = unique_test_suffix();
        let email = format!("me+{unique}@gmail.com");
        let request = format!(
            r#"{{"id":63,"method":"account.addOAuth","params":{{"email":"{email}","provider":"gmail","display_name":"Gmail","sender_name":"Sender","access_token":"access","refresh_token":"refresh","token_expires_at":1700000000}}}}"#
        );
        let added = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
        assert_eq!(added["id"], 63);
        assert_eq!(added["result"]["account"]["id"], email.as_str());
        assert_eq!(added["result"]["account"]["email"], email.as_str());
        assert_eq!(added["result"]["account"]["display_name"], "Gmail");
        assert_eq!(added["result"]["account"]["sender_name"], "Sender");
        assert_eq!(added["result"]["account"]["provider"], "gmail");
        assert_eq!(added["result"]["account"]["auth_type"], "gmail_oauth");
        assert_eq!(added["result"]["account"]["imap_host"], "imap.gmail.com");
        assert_eq!(added["result"]["account"]["imap_port"], 993);
        assert_eq!(added["result"]["account"]["smtp_host"], "smtp.gmail.com");
        assert_eq!(added["result"]["account"]["smtp_port"], 587);
        assert_eq!(added["result"]["account"]["needs_reconnect"], false);

        let listed = invoke_mobile_protocol_json(
            r#"{"id":64,"method":"account.list","params":{}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(listed["id"], 64);
        assert_eq!(listed["result"]["accounts"][0]["id"], email.as_str());
        assert_eq!(listed["result"]["accounts"][0]["needs_reconnect"], false);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_exchanges_oauth_code_and_persists_account() {
        let data_dir = unique_data_dir("oauth-code");
        let token_url = one_shot_oauth_token_server();
        let unique = unique_test_suffix();
        let email = format!("me+{unique}@gmail.com");
        let request = format!(
            r#"{{"id":66,"method":"account.exchangeOAuthCode","params":{{"email":"{email}","provider":"gmail","display_name":"Gmail","sender_name":"Sender","code":"auth-code","client_id":"client","client_secret":"secret","redirect_uri":"jp.nonbili.meron.oauth://oauth","code_verifier":"verifier","token_url":"{token_url}"}}}}"#
        );

        let exchanged = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
        assert_eq!(exchanged["id"], 66, "{exchanged}");
        assert_eq!(
            exchanged["result"]["account"]["id"],
            email.as_str(),
            "{exchanged}"
        );
        assert_eq!(
            exchanged["result"]["account"]["provider"], "gmail",
            "{exchanged}"
        );
        assert_eq!(
            exchanged["result"]["account"]["auth_type"], "gmail_oauth",
            "{exchanged}"
        );
        assert_eq!(
            exchanged["result"]["account"]["needs_reconnect"], false,
            "{exchanged}"
        );

        let listed = invoke_mobile_protocol_json(
            r#"{"id":67,"method":"account.list","params":{}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(
            listed["result"]["accounts"][0]["id"],
            email.as_str(),
            "{listed}"
        );
        assert_eq!(
            listed["result"]["accounts"][0]["needs_reconnect"], false,
            "{listed}"
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_rejects_unsupported_oauth_provider() {
        let data_dir = unique_data_dir("oauth-provider");
        let value = invoke_mobile_protocol_json(
            r#"{"id":65,"method":"account.addOAuth","params":{"email":"me@example.com","provider":"other","refresh_token":"refresh"}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 65);
        assert_eq!(value["error"]["message"], "unsupported oauth provider");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn one_shot_oauth_token_server() -> String {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buf = [0_u8; 1024];
                loop {
                    let read = stream.read(&mut buf).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..read]);
                    let Some(header_end) =
                        request.windows(4).position(|window| window == b"\r\n\r\n")
                    else {
                        continue;
                    };
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
                if request.is_empty() {
                    continue;
                }
                let body = r#"{"access_token":"access-from-code","refresh_token":"refresh-from-code","expires_in":3600}"#;
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
                stream.flush().unwrap();
                return;
            }
        });
        format!("http://{addr}/token")
    }

    #[test]
    fn mobile_protocol_send_requires_usable_account_secret() {
        let data_dir = unique_data_dir("send-needs-secret");
        seed_mobile_account(&data_dir, "me@example.com");

        let value = invoke_mobile_protocol_json(
            r#"{"id":83,"method":"mail.send","params":{"account_id":"me@example.com","to":"you@example.com","subject":"Hi","body":"Hello"}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 83);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_sync_requires_usable_account_secret() {
        let data_dir = unique_data_dir("sync-needs-secret");
        seed_mobile_account(&data_dir, "me@example.com");

        let value = invoke_mobile_protocol_json(
            r#"{"id":84,"method":"mail.sync","params":{"account_id":"me@example.com","folder_id":"inbox","limit":10}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 84);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let desktop_name = invoke_mobile_protocol_json(
            r#"{"id":85,"method":"messages.sync","params":{"account":"me@example.com","folder":"INBOX","limit":10}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(desktop_name["id"], 85);
        assert!(
            desktop_name["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_rejects_invalid_password_account() {
        let data_dir = unique_data_dir("invalid-account");
        let value = invoke_mobile_protocol_json(
            r#"{"id":62,"method":"account.addPassword","params":{"email":"missing-at","imap_host":"imap.example.com","smtp_host":"smtp.example.com","username":"me"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 62);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("invalid email")
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_lists_folders_from_store() {
        let data_dir = unique_data_dir("folders");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::upsert_folders(
            &conn,
            "me@example.com",
            &[
                Folder {
                    name: "INBOX".to_string(),
                    delimiter: Some("/".to_string()),
                    unread: 0,
                },
                Folder {
                    name: "Archive".to_string(),
                    delimiter: Some("/".to_string()),
                    unread: 0,
                },
            ],
        )
        .unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[MessageHeader {
                uid: 1,
                subject: "Unread".to_string(),
                seen: false,
                ..Default::default()
            }],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":63,"method":"mail.folderList","params":{"account_id":"me@example.com"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 63);
        assert_eq!(value["result"]["folders"][0]["id"], "Archive");
        assert_eq!(value["result"]["folders"][0]["role"], "archive");
        assert_eq!(value["result"]["folders"][1]["id"], "INBOX");
        assert_eq!(value["result"]["folders"][1]["role"], "inbox");
        assert_eq!(value["result"]["folders"][1]["unread"], 1);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_folder_create_requires_usable_account_secret() {
        let data_dir = unique_data_dir("folder-create-needs-secret");
        seed_mobile_account(&data_dir, "me@example.com");

        let value = invoke_mobile_protocol_json(
            r#"{"id":66,"method":"mail.folderCreate","params":{"account_id":"me@example.com","name":"Work"}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 66);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_move_requires_usable_account_secret() {
        let data_dir = unique_data_dir("move-needs-secret");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[MessageHeader {
                uid: 7,
                subject: "Move me".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":67,"method":"mail.move","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_folder_id":"Work"}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 67);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_copy_requires_usable_account_secret() {
        let data_dir = unique_data_dir("copy-needs-secret");
        seed_mobile_account(&data_dir, "me@example.com");
        seed_mobile_account(&data_dir, "target@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::ensure_folder(&conn, "target@example.com", "Archive").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[MessageHeader {
                uid: 7,
                subject: "Copy me".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":68,"method":"mail.copy","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","target_account_id":"target@example.com","target_folder_id":"Archive"}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 68);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_lists_threads_from_store() {
        let data_dir = unique_data_dir("threads");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 7,
                    subject: "Newest".to_string(),
                    from_name: "Ada".to_string(),
                    from_addr: "ada@example.com".to_string(),
                    date: 200,
                    seen: false,
                    starred: true,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 3,
                    subject: "Older".to_string(),
                    from_name: "Bea".to_string(),
                    from_addr: "bea@example.com".to_string(),
                    date: 100,
                    seen: true,
                    starred: false,
                    thread_key: "older-topic".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":64,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"all"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 64);
        let first = &value["result"]["threads"][0];
        assert_eq!(first["account_id"], "me@example.com");
        assert_eq!(first["folder_id"], "INBOX");
        assert_eq!(first["subject"], "Newest");
        assert_eq!(first["from_name"], "Ada");
        assert_eq!(first["unread"], true);
        assert_eq!(first["unread_count"], 1);
        assert_eq!(first["starred"], true);
        assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");

        let unread = invoke_mobile_protocol_json(
            r#"{"id":65,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"inbox","filter":"unread"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(unread["result"]["threads"].as_array().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_reads_cached_thread_messages_from_store() {
        let data_dir = unique_data_dir("thread-read");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[MessageHeader {
                uid: 9,
                subject: "Cached subject".to_string(),
                from_name: "Ada".to_string(),
                from_addr: "ada@example.com".to_string(),
                date: 300,
                seen: false,
                starred: true,
                thread_key: "topic".to_string(),
                to: vec![Recipient {
                    name: "Me".to_string(),
                    addr: "me@example.com".to_string(),
                }],
                ..Default::default()
            }],
        )
        .unwrap();
        store::save_cached_message(
            &conn,
            "me@example.com",
            "INBOX",
            9,
            &Message {
                subject: "Cached subject".to_string(),
                from_name: "Ada".to_string(),
                from_addr: "ada@example.com".to_string(),
                to: "Me <me@example.com>".to_string(),
                message_id: "topic".to_string(),
                references: "root".to_string(),
                date: 300,
                body: "Hello from cache".to_string(),
                body_html: Some("<p>Hello from cache</p>".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":66,"method":"mail.threadRead","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 66);
        let first = &value["result"]["messages"][0];
        assert_eq!(first["id"], "me@example.com#INBOX#t.dG9waWM#9");
        assert_eq!(first["account_id"], "me@example.com");
        assert_eq!(first["folder_id"], "INBOX");
        assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");
        assert_eq!(first["from_name"], "Ada");
        assert_eq!(first["to"], "Me <me@example.com>");
        assert_eq!(first["message_id"], "topic");
        assert_eq!(first["references"], "root");
        assert_eq!(first["subject"], "Cached subject");
        assert_eq!(first["body"], "Hello from cache");
        assert_eq!(first["body_html"], "<p>Hello from cache</p>");
        assert_eq!(first["unread"], true);
        assert_eq!(first["starred"], true);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_reads_uid_thread_message_from_store() {
        let data_dir = unique_data_dir("uid-thread-read");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::save_cached_message(
            &conn,
            "me@example.com",
            "Drafts",
            4,
            &Message {
                subject: "Draft".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 400,
                body: "Draft body".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":67,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 67);
        assert_eq!(value["result"]["messages"][0]["subject"], "Draft");
        assert_eq!(value["result"]["messages"][0]["body"], "Draft body");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_mark_read_and_starred_require_server_credentials() {
        let data_dir = unique_data_dir("thread-actions");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 10,
                    subject: "First".to_string(),
                    seen: false,
                    starred: false,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 11,
                    subject: "Second".to_string(),
                    seen: false,
                    starred: false,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        drop(conn);

        let read = invoke_mobile_protocol_json(
            r#"{"id":68,"method":"mail.markRead","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","seen":true}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(read["id"], 68);
        assert!(
            read["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let starred = invoke_mobile_protocol_json(
            r#"{"id":69,"method":"mail.markStarred","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM","starred":true}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(starred["id"], 69);
        assert!(
            starred["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let mark_all = invoke_mobile_protocol_json(
            r#"{"id":72,"method":"mail.markAllRead","params":{"account_id":"me@example.com","folder_id":"INBOX"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(mark_all["id"], 72);
        assert!(
            mark_all["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let threads = invoke_mobile_protocol_json(
            r#"{"id":70,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        let first = &threads["result"]["threads"][0];
        assert_eq!(first["thread_id"], "me@example.com#INBOX#t.dG9waWM");
        assert_eq!(first["unread"], true);
        assert_eq!(first["unread_count"], 1);
        assert_eq!(first["starred"], false);

        let unread = invoke_mobile_protocol_json(
            r#"{"id":71,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"unread"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(unread["result"]["threads"].as_array().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_marks_uid_thread_read_requires_server_credentials() {
        let data_dir = unique_data_dir("uid-thread-actions");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::save_cached_message(
            &conn,
            "me@example.com",
            "Drafts",
            4,
            &Message {
                subject: "Draft".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 400,
                body: "Draft body".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":72,"method":"mail.markRead","params":{"thread_id":"me@example.com#Drafts#4","seen":true}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let read = invoke_mobile_protocol_json(
            r#"{"id":73,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(read["result"]["messages"][0]["unread"], true);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_lists_starred_items_from_store() {
        let data_dir = unique_data_dir("starred-items");
        seed_mobile_account(&data_dir, "a1@example.com");
        seed_mobile_account(&data_dir, "a2@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "a1@example.com", "INBOX").unwrap();
        store::ensure_folder(&conn, "a2@example.com", "Sent").unwrap();
        store::upsert_messages(
            &conn,
            "a1@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 1,
                    subject: "Older starred".to_string(),
                    from_name: "Ada".to_string(),
                    from_addr: "ada@example.com".to_string(),
                    date: 100,
                    seen: false,
                    starred: true,
                    thread_key: "older".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 2,
                    subject: "Not starred".to_string(),
                    date: 300,
                    seen: false,
                    starred: false,
                    thread_key: "skip".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        store::upsert_messages(
            &conn,
            "a2@example.com",
            "Sent",
            &[MessageHeader {
                uid: 7,
                subject: "Newest starred".to_string(),
                from_name: "Bea".to_string(),
                from_addr: "bea@example.com".to_string(),
                date: 200,
                seen: true,
                starred: true,
                thread_key: "newer".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":74,"method":"mail.starredItems","params":{"limit":2}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 74);
        let items = value["result"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["account_id"], "a2@example.com");
        assert_eq!(items[0]["folder_id"], "Sent");
        assert_eq!(items[0]["subject"], "Newest starred");
        assert_eq!(items[0]["thread_id"], "a2@example.com#Sent#t.bmV3ZXI");
        assert_eq!(items[0]["id"], "a2@example.com#Sent#t.bmV3ZXI#7");
        assert_eq!(items[0]["unread"], false);
        assert_eq!(items[0]["starred"], true);
        assert_eq!(items[1]["account_id"], "a1@example.com");
        assert_eq!(items[1]["subject"], "Older starred");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_routes_rss_accounts_through_mail_shapes() {
        let data_dir = unique_data_dir("rss-mail-shapes");
        seed_rss_account(&data_dir, "rss-account", "News");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        seed_rss_feed(&conn, "rss-account", "feed-a", "Feed A");
        seed_rss_item(
            &conn,
            "rss-account",
            "feed-a",
            "item-old",
            "Old item",
            100,
            true,
        );
        seed_rss_item(
            &conn,
            "rss-account",
            "feed-a",
            "item-new",
            "New item",
            200,
            true,
        );
        drop(conn);

        let folders = invoke_mobile_protocol_json(
            r#"{"id":86,"method":"mail.folderList","params":{"account_id":"rss-account"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(folders["id"], 86);
        assert_eq!(folders["result"]["folders"][0]["id"], "inbox");
        assert_eq!(folders["result"]["folders"][0]["role"], "inbox");
        assert_eq!(folders["result"]["folders"][0]["unread"], 2);

        let threads = invoke_mobile_protocol_json(
            r#"{"id":87,"method":"mail.threadList","params":{"account_id":"rss-account","folder_id":"inbox","filter":"all"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(threads["id"], 87);
        let first = &threads["result"]["threads"][0];
        assert_eq!(first["thread_id"], "rss-account#rss#feed-a");
        assert_eq!(first["account_id"], "rss-account");
        assert_eq!(first["folder_id"], "inbox");
        assert_eq!(first["from_name"], "Feed A");
        assert_eq!(first["subject"], "Feed A");
        assert_eq!(first["unread"], true);
        assert_eq!(first["unread_count"], 2);

        let read = invoke_mobile_protocol_json(
            r#"{"id":88,"method":"mail.threadRead","params":{"thread_id":"rss-account#rss#feed-a","limit":1}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(read["id"], 88);
        assert_eq!(
            read["result"]["messages"][0]["id"],
            "rss-account#rss#feed-a#item-new"
        );
        assert_eq!(read["result"]["messages"][0]["subject"], "New item");
        assert_eq!(
            read["result"]["messages"][0]["body"],
            "Body for New item\n\nSource: https://example.com/item-new"
        );
        assert_eq!(read["result"]["messages"][0]["unread"], true);
        assert_eq!(read["result"]["next_cursor"], "ts:200:item-new");

        let mark_read = invoke_mobile_protocol_json(
            r#"{"id":89,"method":"mail.markRead","params":{"thread_id":"rss-account#rss#feed-a","seen":true,"message_ids":["rss-account#rss#feed-a#item-new"]}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(mark_read["result"]["ok"], true);

        let mark_starred = invoke_mobile_protocol_json(
            r#"{"id":90,"method":"mail.markStarred","params":{"thread_id":"rss-account#rss#feed-a","starred":true,"item_keys":["item-new"]}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(mark_starred["result"]["ok"], true);

        let reread = invoke_mobile_protocol_json(
            r#"{"id":91,"method":"rss.thread","params":{"thread_id":"rss-account#rss#feed-a"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        let messages = reread["result"]["messages"].as_array().unwrap();
        let new_item = messages
            .iter()
            .find(|message| message["id"] == "rss-account#rss#feed-a#item-new")
            .unwrap();
        assert_eq!(new_item["unread"], false);
        assert_eq!(new_item["starred"], true);

        let starred = invoke_mobile_protocol_json(
            r#"{"id":92,"method":"mail.starredItems","params":{"limit":5}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(
            starred["result"]["items"][0]["id"],
            "rss-account#rss#feed-a#item-new"
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_exports_and_imports_opml() {
        let data_dir = unique_data_dir("rss-opml-mobile");
        seed_rss_account(&data_dir, "rss-source", "Source feeds");
        seed_rss_account(&data_dir, "rss-target", "Target feeds");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        seed_rss_feed(&conn, "rss-source", "feed-a", "Feed A");
        drop(conn);

        let exported = invoke_mobile_protocol_json(
            r#"{"id":91,"method":"rss.exportOpml","params":{"account":"rss-source"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(exported["id"], 91);
        let opml = exported["result"]["opml"].as_str().unwrap();
        assert!(opml.contains("<opml version=\"2.0\">"));
        assert!(opml.contains("Feed A"));

        let request = serde_json::json!({
            "id": 92,
            "method": "rss.importOpml",
            "params": {
                "account": "rss-target",
                "opml": r#"<?xml version="1.0"?><opml version="2.0"><body><outline text="Imported"><outline type="rss" text="Imported Feed" xmlUrl="https://import.example/feed.xml"/></outline></body></opml>"#,
            },
        })
        .to_string();
        let imported = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
        assert_eq!(imported["id"], 92);
        assert_eq!(imported["result"]["imported"], 1);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_syncs_empty_rss_account_without_mail_credentials() {
        let data_dir = unique_data_dir("rss-sync-empty");
        seed_rss_account(&data_dir, "rss-empty", "RSS");

        let value = invoke_mobile_protocol_json(
            r#"{"id":93,"method":"mail.sync","params":{"account_id":"rss-empty"}}"#,
            Some(data_dir.to_str().unwrap()),
        );

        assert_eq!(value["id"], 93);
        assert_eq!(value["result"]["ok"], true);
        assert_eq!(value["result"]["account"], "rss-empty");
        assert_eq!(value["result"]["synced"], 0);
        assert_eq!(value["result"]["rss"], true);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_delete_requires_server_credentials() {
        let data_dir = unique_data_dir("thread-delete");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 20,
                    subject: "Delete me".to_string(),
                    date: 300,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 21,
                    subject: "Delete me too".to_string(),
                    date: 200,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 22,
                    subject: "Keep me".to_string(),
                    date: 100,
                    thread_key: "other".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":75,"method":"mail.delete","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 75);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let threads = invoke_mobile_protocol_json(
            r#"{"id":76,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        let rows = threads["result"]["threads"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["subject"], "Delete me");
        assert_eq!(rows[1]["subject"], "Keep me");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_delete_uid_thread_requires_server_credentials() {
        let data_dir = unique_data_dir("uid-thread-delete");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::save_cached_message(
            &conn,
            "me@example.com",
            "Drafts",
            4,
            &Message {
                subject: "Draft".to_string(),
                from_name: "Me".to_string(),
                from_addr: "me@example.com".to_string(),
                date: 400,
                body: "Draft body".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":77,"method":"mail.delete","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let read = invoke_mobile_protocol_json(
            r#"{"id":78,"method":"mail.threadRead","params":{"thread_id":"me@example.com#Drafts#4"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(read["result"]["messages"].as_array().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_archive_requires_server_credentials() {
        let data_dir = unique_data_dir("thread-archive");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::upsert_folders(
            &conn,
            "me@example.com",
            &[
                Folder {
                    name: "INBOX".to_string(),
                    delimiter: Some("/".to_string()),
                    unread: 0,
                },
                Folder {
                    name: "Archive".to_string(),
                    delimiter: Some("/".to_string()),
                    unread: 0,
                },
            ],
        )
        .unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 31,
                    subject: "Archive me".to_string(),
                    date: 300,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 32,
                    subject: "Archive me too".to_string(),
                    date: 200,
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 33,
                    subject: "Keep me".to_string(),
                    date: 100,
                    thread_key: "other".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":79,"method":"mail.archive","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 79);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("account needs reconnect")
        );

        let inbox = invoke_mobile_protocol_json(
            r#"{"id":80,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"INBOX","filter":"all"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        let inbox_rows = inbox["result"]["threads"].as_array().unwrap();
        assert_eq!(inbox_rows.len(), 2);
        assert_eq!(inbox_rows[0]["subject"], "Archive me");
        assert_eq!(inbox_rows[1]["subject"], "Keep me");

        let archive = invoke_mobile_protocol_json(
            r#"{"id":81,"method":"mail.threadList","params":{"account_id":"me@example.com","folder_id":"Archive","filter":"all"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        let archive_rows = archive["result"]["threads"].as_array().unwrap();
        assert_eq!(archive_rows.len(), 0);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mobile_protocol_archive_requires_cached_archive_folder() {
        let data_dir = unique_data_dir("thread-archive-missing-folder");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[MessageHeader {
                uid: 41,
                subject: "Archive me".to_string(),
                thread_key: "topic".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();
        drop(conn);

        let value = invoke_mobile_protocol_json(
            r#"{"id":82,"method":"mail.archive","params":{"thread_id":"me@example.com#INBOX#t.dG9waWM"}}"#,
            Some(data_dir.to_str().unwrap()),
        );
        assert_eq!(value["id"], 82);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Archive folder not found")
        );

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn protocol_invoke_wraps_empty_mail_collections() {
        let cases = [
            (
                r#"{"id":44,"method":"mail.folderList","params":{"account_id":"acc"}}"#,
                json!({ "folders": [] }),
            ),
            (
                r#"{"id":45,"method":"mail.threadList","params":{"account_id":"acc","folder_id":"inbox"}}"#,
                json!({ "threads": [] }),
            ),
            (
                r#"{"id":46,"method":"mail.threadRead","params":{"thread_id":"thread"}}"#,
                json!({ "messages": [] }),
            ),
            (
                r#"{"id":47,"method":"mail.starredItems","params":{}}"#,
                json!({ "items": [] }),
            ),
            (
                r#"{"id":48,"method":"rss.exportOpml","params":{"account":"rss"}}"#,
                json!({ "opml": "" }),
            ),
        ];

        for (request, expected_result) in cases {
            let value = invoke_protocol_json(request);
            assert_eq!(value["result"], expected_result);
            assert!(value.get("error").is_none());
        }
    }

    #[test]
    fn protocol_invoke_wraps_noop_mail_mutations() {
        let cases = [
            (
                r#"{"id":48,"method":"mail.send","params":{"account_id":"acc","to":"you@example.com"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":71,"method":"mail.saveDraft","params":{"account_id":"acc","draft_id":"draft@example.com"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":72,"method":"mail.discardDraft","params":{"account_id":"acc","draft_id":"draft@example.com"}}"#,
                json!({ "ok": true, "deleted": 0, "permanent": true }),
            ),
            (
                r#"{"id":54,"method":"rss.importOpml","params":{"account":"rss","opml":"<opml/>"}}"#,
                json!({ "imported": 0 }),
            ),
            (
                r#"{"id":73,"method":"account.addRSS","params":{"feed_url":"https://example.com/feed.xml","display_name":"News"}}"#,
                json!({ "account": Value::Null }),
            ),
            (
                r#"{"id":74,"method":"rss.addFeed","params":{"account_id":"rss","feed_url":"https://example.com/feed.xml"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":75,"method":"rss.removeFeed","params":{"thread_id":"rss#rss#feed"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":76,"method":"rss.moveFeed","params":{"thread_id":"rss#rss#feed","target_account_id":"rss2"}}"#,
                json!({ "ok": true, "moved": 0 }),
            ),
            (
                r#"{"id":77,"method":"mail.readAttachment","params":{"key":"acc/INBOX/1/note.txt"}}"#,
                json!({ "data": "" }),
            ),
            (
                r#"{"id":55,"method":"account.remove","params":{"id":"acc"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":56,"method":"account.setName","params":{"id":"acc","name":"Personal"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":57,"method":"account.setSenderName","params":{"id":"acc","name":"Sender"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":58,"method":"account.setAvatar","params":{"id":"acc","avatar_url":"https://example.com/avatar.png"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":59,"method":"account.setChatWallpaper","params":{"id":"acc","wallpaper":{"kind":"preset","presetId":"grid"}}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":60,"method":"account.writeAvatarFile","params":{"id":"acc","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}}"#,
                json!({ "url": "" }),
            ),
            (
                r#"{"id":61,"method":"account.writeChatWallpaperFile","params":{"id":"acc","filename":"wallpaper.png","mime":"image/png","data":"aGVsbG8="}}"#,
                json!({ "url": "" }),
            ),
            (
                r#"{"id":62,"method":"account.setImages","params":{"id":"acc","enabled":true}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":63,"method":"account.setConversationHtml","params":{"id":"acc","enabled":false}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":64,"method":"account.setUnified","params":{"id":"acc","enabled":false}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":65,"method":"account.setMuted","params":{"id":"acc","enabled":true}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":66,"method":"account.setPaused","params":{"id":"acc","enabled":true}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":67,"method":"account.setRSSSyncInterval","params":{"id":"acc","minutes":30}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":68,"method":"account.setAliases","params":{"id":"acc","aliases":[]}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":69,"method":"account.reorder","params":{"accounts":["b","a"]}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":70,"method":"mail.suggestContacts","params":{"account":"acc","query":"a","limit":8}}"#,
                json!({ "contacts": [] }),
            ),
            (
                r#"{"id":49,"method":"mail.archive","params":{"thread_id":"thread"}}"#,
                json!({ "ok": true, "moved": 0 }),
            ),
            (
                r#"{"id":50,"method":"mail.delete","params":{"thread_id":"thread"}}"#,
                json!({ "ok": true, "deleted": 0 }),
            ),
            (
                r#"{"id":51,"method":"mail.markRead","params":{"thread_id":"thread","seen":true}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":52,"method":"mail.markAllRead","params":{"account_id":"acc","folder_id":"INBOX"}}"#,
                json!({ "ok": true }),
            ),
            (
                r#"{"id":53,"method":"mail.markStarred","params":{"thread_id":"thread","starred":true}}"#,
                json!({ "ok": true }),
            ),
        ];

        for (request, expected_result) in cases {
            let value = invoke_protocol_json(request);
            assert_eq!(value["result"], expected_result);
            assert!(value.get("error").is_none());
        }
    }

    #[test]
    fn protocol_invoke_wraps_unknown_method_error() {
        let value = invoke_protocol_json(r#"{"id":9,"method":"missing"}"#);
        assert_eq!(value["id"], 9);
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown method")
        );
    }

    fn unique_data_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("meron-mobile-{label}-{}", unique_test_suffix()))
    }

    fn unique_test_suffix() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        let count = TEST_UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{pid}-{nanos}-{count}")
    }

    fn seed_mobile_account(data_dir: &std::path::Path, email: &str) {
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        let creds = Creds {
            host: "imap.example.com".to_string(),
            port: 993,
            user: email.to_string(),
            password: String::new(),
            tls: true,
            starttls: false,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
            smtp_tls: true,
            smtp_starttls: false,
            auth_type: "password".to_string(),
            access_token: None,
            refresh_token: None,
            token_expires_at: 0,
        };
        let meta = AccountMeta {
            engine: "mail".to_string(),
            provider: "custom".to_string(),
            email: email.to_string(),
            display_name: String::new(),
            avatar_url: String::new(),
            sender_name: String::new(),
        };
        store::upsert_account(&conn, email, &meta, &creds).unwrap();
    }

    fn seed_rss_account(data_dir: &std::path::Path, account: &str, display_name: &str) {
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, email, display_name, config)
             VALUES(?1, 'rss', 'rss', ?2, ?3, '{}')",
            rusqlite::params![account, format!("{account}.local"), display_name],
        )
        .unwrap();
    }

    fn seed_rss_feed(conn: &rusqlite::Connection, account: &str, sub_id: &str, title: &str) {
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, feed_title, enabled)
             VALUES(?1, ?2, ?3, ?4, ?4, 1)",
            rusqlite::params![
                sub_id,
                account,
                format!("https://example.com/{sub_id}"),
                title
            ],
        )
        .unwrap();
    }

    fn seed_rss_item(
        conn: &rusqlite::Connection,
        account: &str,
        sub_id: &str,
        item_key: &str,
        title: &str,
        ts: i64,
        unread: bool,
    ) {
        store::upsert_rss_item(
            conn,
            account,
            sub_id,
            item_key,
            title,
            unread,
            Some(&format!("<p>Body for {title}</p>")),
            &RssItemExtra {
                author: "Author".to_string(),
                link: format!("https://example.com/{item_key}"),
                summary: format!("Summary for {title}"),
                content: String::new(),
                images: Vec::<RssMedia>::new(),
                videos: Vec::<RssMedia>::new(),
                published_at: ts,
                updated_at: 0,
                fetched_at: ts,
            },
        )
        .unwrap();
    }

    #[test]
    fn requested_mobile_uids_uses_message_ids_without_thread_fallback() {
        let data_dir = unique_data_dir("targeted-uids");
        seed_mobile_account(&data_dir, "me@example.com");
        let conn = store::open_at(data_dir.join("meron.db")).unwrap();
        store::ensure_folder(&conn, "me@example.com", "INBOX").unwrap();
        store::upsert_messages(
            &conn,
            "me@example.com",
            "INBOX",
            &[
                MessageHeader {
                    uid: 7,
                    subject: "One".to_string(),
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
                MessageHeader {
                    uid: 8,
                    subject: "Two".to_string(),
                    thread_key: "topic".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();
        let parsed = ParsedThreadId {
            account: "me@example.com".to_string(),
            folder: "INBOX".to_string(),
            thread_key: "topic".to_string(),
            uid: None,
        };

        let targeted = requested_mobile_uids(
            &conn,
            &parsed,
            &json!({ "message_ids": ["me@example.com#INBOX#t.dG9waWM#8"] }),
        )
        .unwrap();
        assert_eq!(targeted, vec![8]);

        let malformed =
            requested_mobile_uids(&conn, &parsed, &json!({ "message_ids": ["bad"] })).unwrap();
        assert!(malformed.is_empty());

        let fallback = requested_mobile_uids(&conn, &parsed, &json!({})).unwrap();
        assert_eq!(fallback, vec![7, 8]);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn parse_thread_id_with_request_folder_overrides_encoded_folder() {
        let parsed = parse_thread_id_with_request_folder(
            "me@example.com#INBOX#t.dG9waWM",
            &json!({ "folder": "Trash" }),
        )
        .unwrap();

        assert_eq!(parsed.account, "me@example.com");
        assert_eq!(parsed.folder, "Trash");
        assert_eq!(parsed.thread_key, "topic");
    }

    #[test]
    fn mobile_autodiscover_returns_known_provider_or_guess() {
        let gmail = invoke_mobile_protocol_json(
            r#"{"id":96,"method":"account.autodiscover","params":{"email":"me@gmail.com"}}"#,
            None,
        );
        assert_eq!(gmail["id"], 96);
        assert_eq!(gmail["result"]["imap_host"], "imap.gmail.com");
        assert_eq!(gmail["result"]["smtp_host"], "smtp.gmail.com");
        assert_eq!(gmail["result"]["username"], "me@gmail.com");
        assert_eq!(gmail["result"]["source"], "known");

        let guessed = invoke_mobile_protocol_json(
            r#"{"id":97,"method":"account.autodiscover","params":{"email":"me@example.org"}}"#,
            None,
        );
        assert_eq!(guessed["result"]["imap_host"], "imap.example.org");
        assert_eq!(guessed["result"]["smtp_host"], "smtp.example.org");
        assert_eq!(guessed["result"]["source"], "guess");
    }
}
