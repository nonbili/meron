use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

use meron_core::engine::Engine;
use meron_core::engine::*;
use meron_core::protocol::Request;
use meron_core::{imap, parse, proxy, secrets, smtp, store};

use crate::Writer;
use crate::sidecar::idle::*;
use crate::sidecar::params::*;

/// Handle `account.*`: listing, connecting, and per-account settings.
pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    match req.method.as_str() {
        // All accounts (mail + rss) as bridge-shaped JSON, from the one DB.
        "account.list" => {
            let mut accounts = store::list_accounts(&engine.db.lock().unwrap())?;
            let live_accounts = engine.accounts.lock().await;
            for account in &mut accounts {
                if account
                    .get("auth_type")
                    .and_then(Value::as_str)
                    .is_some_and(|auth_type| auth_type == "rss")
                {
                    continue;
                }
                let Some(id) = account
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                else {
                    continue;
                };
                if let Some(obj) = account.as_object_mut() {
                    let needs_reconnect = live_accounts
                        .get(&id)
                        .is_none_or(|creds| !creds_have_required_secret(creds));
                    obj.insert("needs_reconnect".to_string(), json!(needs_reconnect));
                }
            }
            Ok(json!({ "accounts": accounts }))
        }

        // Store (and validate) IMAP credentials for an account.
        "account.connect" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let host = req_str(p, "host")?;
            let mut creds = imap::Creds {
                host: host.clone(),
                port: req_u16(p, "port").unwrap_or(993),
                user: req_str(p, "user")?,
                password: p
                    .get("password")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                tls: p.get("tls").and_then(Value::as_bool).unwrap_or(true),
                starttls: p.get("starttls").and_then(Value::as_bool).unwrap_or(false),
                smtp_host: req_str(p, "smtp_host").unwrap_or(host),
                smtp_port: req_u16(p, "smtp_port").unwrap_or(587),
                smtp_tls: p.get("smtp_tls").and_then(Value::as_bool).unwrap_or(true),
                smtp_starttls: p
                    .get("smtp_starttls")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                auth_type: p
                    .get("auth_type")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "password".to_string()),
                access_token: p
                    .get("access_token")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                refresh_token: p
                    .get("refresh_token")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                token_expires_at: p
                    .get("token_expires_at")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
                oauth_client_id: p
                    .get("oauth_client_id")
                    .or_else(|| p.get("client_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                oauth_client_secret: p
                    .get("oauth_client_secret")
                    .or_else(|| p.get("client_secret"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                oauth_token_url: p
                    .get("oauth_token_url")
                    .or_else(|| p.get("token_url"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                oauth_scope: p
                    .get("oauth_scope")
                    .or_else(|| p.get("scope"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                proxy: proxy::ProxyChoice::from_json(p.get("proxy").unwrap_or(&Value::Null)),
                // Set once the user has inspected and accepted a certificate
                // that webpki rejects (see `account.probeCert`).
                cert_pin: cert_pin_param(p, "cert_pin"),
                smtp_cert_pin: cert_pin_param(p, "smtp_cert_pin"),
                rejected_grant: None,
            };
            // A reconnect resends the setup form, which has no field for the
            // account's proxy or the certificates it accepted. Carry those over
            // from the stored account so saving credentials again does not
            // silently reset them.
            let omitted = imap::OmittedSettings {
                proxy: p.get("proxy").is_none(),
                cert_pin: p.get("cert_pin").is_none(),
                smtp_cert_pin: p.get("smtp_cert_pin").is_none(),
                password: p.get("password").is_none(),
            };
            if omitted.any() {
                let stored = {
                    let db = engine.db.lock().unwrap();
                    store::load_account(&db, &id)?
                };
                if let Some(mut stored) = stored {
                    // The password lives in the keychain, not the account row,
                    // so the stored creds have to be hydrated before they can
                    // supply one.
                    if omitted.password {
                        let db = engine.db.lock().unwrap();
                        engine.host.apply_secret(&db, &id, &mut stored);
                    }
                    creds.carry_over(&stored, omitted);
                }
            }
            // Password accounts validate before storage. OAuth accounts may be
            // created directly after Google's token exchange; IMAP validation
            // can be slow or network-dependent, and later sync/watch calls will
            // surface any mailbox access failure.
            if p.get("validate").and_then(Value::as_bool).unwrap_or(true) {
                let mut session =
                    tokio::time::timeout(Duration::from_secs(20), imap::connect(&creds))
                        .await
                        .map_err(|_| anyhow::anyhow!("IMAP validation timed out"))??;
                let _ = session.logout().await;
                // The submission server can be a different daemon with a
                // certificate of its own; a save that only validated IMAP would
                // hand the user an account that fails at the first send.
                tokio::time::timeout(Duration::from_secs(20), smtp::check_certificate(&creds))
                    .await
                    .unwrap_or(Ok(()))?;
            }
            let meta = store::AccountMeta {
                engine: "mail".to_string(),
                provider: p
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("custom")
                    .to_string(),
                email: p
                    .get("email")
                    .and_then(Value::as_str)
                    .unwrap_or(&creds.user)
                    .to_string(),
                display_name: p
                    .get("display_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                avatar_url: p
                    .get("avatar_url")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                sender_name: p
                    .get("sender_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            };
            {
                let db = engine.db.lock().unwrap();
                store::upsert_account(&db, &id, &meta, &creds)?;
            }
            secrets::store(&id, &secrets::Secrets::from_creds(&creds))?;
            engine.accounts.lock().await.insert(id.clone(), creds);
            // This call also edits an existing account (server settings, a new
            // password, a reconnect). Replacing the cached creds is not enough:
            // warm pooled sessions and the live IDLE watcher are already
            // authenticated against the *old* server, and would keep serving
            // reads and pushes from it indefinitely. Drop the pool and wake the
            // watchers so the next connection is made with what was just saved.
            engine.clear_pool(&id);
            engine.resume_signal.notify_waiters();
            // Start watching the new account right away. Only startup resumed
            // IDLE for known accounts, so an account added mid-session stayed
            // unwatched (and its INBOX unwarmed) until the next launch: mail
            // arrived on the server and nothing pushed it into the store.
            if !engine.is_paused(&id) {
                if start_idle_watch(engine.clone(), out.clone(), id.clone(), "INBOX".to_string()) {
                    spawn_body_prefetch(engine.clone(), id.clone(), "INBOX".to_string());
                }
                start_notify_folder_watches(engine, out, &id);
            }
            Ok(json!({ "ok": true, "account": id }))
        }

        // Fetch the certificate a mail server presents, so the account dialog
        // can show it and let the user pin it. Needed for local bridges (Proton
        // Mail Bridge) whose self-signed leaf webpki refuses outright; nothing
        // is sent over the probe connection.
        "account.probeCert" => {
            let host = req_str(p, "host")?;
            let port = req_u16(p, "port").unwrap_or(993);
            let protocol = p
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or("imap")
                .to_string();
            let starttls = p.get("starttls").and_then(Value::as_bool).unwrap_or(false);
            let proxy = proxy::ProxyChoice::from_json(p.get("proxy").unwrap_or(&Value::Null));
            let info =
                meron_core::tls::probe(&host, port, &protocol, starttls, proxy.resolve().as_ref())
                    .await?;
            Ok(json!({ "certificate": info }))
        }

        // Forget an account: drop its in-memory creds, cached state, and the
        // keychain secret. The IDLE watcher notices the account is gone on its
        // next loop and exits.
        "account.remove" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            engine.accounts.lock().await.remove(&id);
            // Drop any warm sessions: their creds are gone and must not be reused.
            engine.clear_pool(&id);
            {
                let db = engine.db.lock().unwrap();
                store::delete_account(&db, &id)?;
            }
            parse::remove_account_media(&parse::media_root(), &id);
            let _ = secrets::delete(&id);
            Ok(json!({ "ok": true }))
        }

        // Set the per-account "load remote images" preference.
        "account.setImages" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_load_remote_images(&engine.db.lock().unwrap(), &id, enabled)?;
            Ok(json!({ "ok": true }))
        }

        // Toggle whether conversation bubbles render original HTML when available.
        "account.setConversationHtml" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(
                &engine.db.lock().unwrap(),
                &id,
                "conversation_html",
                enabled,
            )?;
            Ok(json!({ "ok": true }))
        }

        // Set or clear the per-account chat wallpaper preference. The bridge
        // owns image-file validation and storage; the sidecar validates the
        // persisted JSON shape.
        "account.setChatWallpaper" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let wallpaper = match p.get("wallpaper") {
                Some(Value::Null) | None => None,
                Some(value) => {
                    let obj = value
                        .as_object()
                        .ok_or_else(|| anyhow::anyhow!("wallpaper must be an object"))?;
                    let kind = obj.get("kind").and_then(Value::as_str).unwrap_or_default();
                    match kind {
                        "preset" => {
                            let preset_id = obj
                                .get("presetId")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .trim();
                            if preset_id.is_empty() {
                                anyhow::bail!("preset wallpaper requires presetId");
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
                                anyhow::bail!("custom wallpaper URL must be a Meron wallpaper");
                            }
                            Some(json!({ "kind": "custom", "url": url }))
                        }
                        _ => anyhow::bail!("unknown wallpaper kind"),
                    }
                }
            };
            store::set_account_pref_json(
                &engine.db.lock().unwrap(),
                &id,
                "chat_wallpaper",
                wallpaper,
            )?;
            Ok(json!({ "ok": true }))
        }

        // Set the account's display name.
        // Point one account at a different proxy than the app-wide setting (or
        // at none). Live sessions keep their sockets; the choice applies as
        // they reconnect.
        "account.setProxy" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let choice = proxy::ProxyChoice::from_json(p.get("proxy").unwrap_or(&Value::Null));
            store::set_account_proxy(&engine.db.lock().unwrap(), &id, &choice)?;
            if let Some(creds) = engine.accounts.lock().await.get_mut(&id) {
                creds.proxy = choice;
            }
            Ok(json!({ "ok": true }))
        }

        // Store certificate pins the user accepted for an account that already
        // exists — a server whose certificate rotated, or one whose failure only
        // showed up on a later sync or send. Omitting a key leaves that server's
        // pin alone; an explicit null clears it.
        "account.setCertPin" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let mut accounts = engine.accounts.lock().await;
            let existing = accounts.get(&id);
            let cert_pin = match p.get("cert_pin") {
                Some(_) => cert_pin_param(p, "cert_pin"),
                None => existing.and_then(|creds| creds.cert_pin.clone()),
            };
            let smtp_cert_pin = match p.get("smtp_cert_pin") {
                Some(_) => cert_pin_param(p, "smtp_cert_pin"),
                None => existing.and_then(|creds| creds.smtp_cert_pin.clone()),
            };
            store::set_account_cert_pins(
                &engine.db.lock().unwrap(),
                &id,
                cert_pin.as_deref(),
                smtp_cert_pin.as_deref(),
            )?;
            if let Some(creds) = accounts.get_mut(&id) {
                creds.cert_pin = cert_pin;
                creds.smtp_cert_pin = smtp_cert_pin;
            }
            drop(accounts);
            // Pooled sessions were built with the old trust decision.
            engine.clear_pool(&id);
            Ok(json!({ "ok": true }))
        }

        "account.setName" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let name = req_str(p, "name")?;
            {
                let db = engine.db.lock().unwrap();
                db.execute(
                    "UPDATE accounts SET display_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
                    rusqlite::params![name.trim(), id],
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Set the account's sender name.
        "account.setSenderName" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let name = req_str(p, "name")?;
            {
                let db = engine.db.lock().unwrap();
                db.execute(
                    "UPDATE accounts SET sender_name = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
                    rusqlite::params![name.trim(), id],
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Set or clear the account's UI avatar URL/path.
        "account.setAvatar" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let avatar_url = req_str(p, "avatar_url").unwrap_or_default();
            {
                let db = engine.db.lock().unwrap();
                db.execute(
                    "UPDATE accounts SET avatar_url = ?1, updated_at = strftime('%s', 'now') WHERE id = ?2",
                    rusqlite::params![avatar_url.trim(), id],
                )?;
            }
            Ok(json!({ "ok": true }))
        }

        // Replace an account's send-as aliases (the whole list). Entries are
        // {email, name?}; we trim, drop blank emails, and dedupe by email.
        "account.setAliases" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let mut aliases: Vec<store::Alias> = match p.get("aliases") {
                Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
                None => Vec::new(),
            };
            let mut seen = std::collections::HashSet::new();
            aliases.retain_mut(|a| {
                a.email = a.email.trim().to_string();
                a.name = a.name.trim().to_string();
                !a.email.is_empty() && seen.insert(a.email.to_lowercase())
            });
            {
                let db = engine.db.lock().unwrap();
                store::set_account_aliases(&db, &id, &aliases)?;
            }
            Ok(json!({ "ok": true }))
        }

        // Set or clear this account's signature override. A null `signature`
        // drops the pref, so the account follows the app-wide signature again.
        "account.setSignature" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let signature = store::AccountSignature::from_param(p.get("signature"))
                .map_err(|err| anyhow::anyhow!(err))?;
            store::set_account_pref_json(
                &engine.db.lock().unwrap(),
                &id,
                "signature",
                signature.map(|sig| json!(sig)),
            )?;
            Ok(json!({ "ok": true }))
        }

        // Toggle whether the account folds into the unified inbox. Purely a stored
        // pref the UI reads via account.list; no engine side effects.
        "account.setUnified" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(
                &engine.db.lock().unwrap(),
                &id,
                "included_in_unified",
                enabled,
            )?;
            Ok(json!({ "ok": true }))
        }

        // Toggle whether new mail/feed items raise a desktop notification. The
        // watcher still runs (mail keeps arriving); the bridge reads the `muted`
        // flag on each mail.newMessages event to decide whether to notify.
        "account.setMuted" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(&engine.db.lock().unwrap(), &id, "muted", enabled)?;
            Ok(json!({ "ok": true }))
        }

        // Pause/resume automatic checking. Pausing stops the IDLE watcher and
        // gates background syncs; resuming restarts the watcher for mail accounts.
        "account.setPaused" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let enabled = req_bool(p, "enabled")?;
            store::set_account_pref(&engine.db.lock().unwrap(), &id, "paused", enabled)?;
            if enabled {
                // Wake live watchers so the just-paused one shuts down promptly,
                // and drop warm sessions so a paused account holds no connections.
                engine.pause_signal.notify_waiters();
                engine.clear_pool(&id);
            } else if !is_rss(engine, &id)? {
                // Resume: restart the IDLE watcher (deduped) and warm the inbox.
                if start_idle_watch(engine.clone(), out.clone(), id.clone(), "INBOX".to_string()) {
                    spawn_body_prefetch(engine.clone(), id.clone(), "INBOX".to_string());
                }
                start_notify_folder_watches(engine, out, &id);
            }
            Ok(json!({ "ok": true }))
        }

        // Override Sent-copy behavior. Null removes the override so provider
        // defaults apply; true/false force or suppress IMAP APPEND after SMTP.
        "account.setSaveSentCopy" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let value = match p.get("value") {
                Some(Value::Bool(enabled)) => Some(json!(enabled)),
                Some(Value::Null) | None => None,
                _ => return Err(anyhow::anyhow!("value must be true, false, or null")),
            };
            store::set_account_pref_json(&engine.db.lock().unwrap(), &id, "save_sent_copy", value)?;
            Ok(json!({ "ok": true }))
        }

        // Set the RSS automatic sync interval. Stored in minutes so the UI and
        // scheduler can read it from account.list without sidecar state.
        "account.setRSSSyncInterval" => {
            let id = req_str(p, "account").or_else(|_| req_str(p, "id"))?;
            let minutes = req_u32(p, "minutes")?.clamp(5, 1440) as u64;
            store::set_account_pref_u64(
                &engine.db.lock().unwrap(),
                &id,
                "rss_sync_interval_minutes",
                minutes,
            )?;
            Ok(json!({ "ok": true, "minutes": minutes }))
        }

        // Reorder accounts in the database.
        "account.reorder" => {
            let ids = req_str_array(p, "accounts")?;
            store::reorder_accounts(&engine.db.lock().unwrap(), &ids)?;
            Ok(json!({ "ok": true }))
        }

        other => Err(anyhow::anyhow!("unknown method: {other}")),
    }
}
