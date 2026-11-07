//! `pgp.*`: end-to-end encryption settings, keys and the composer's key check.
//!
//! Secret keys never leave the core except as a passphrase-protected export,
//! and no response or error carries key material or a passphrase (errors are
//! printed to stderr by `main.rs::handle`).

use anyhow::Context as _;
use base64::Engine as _;
use rusqlite::OptionalExtension as _;
use serde_json::{Value, json};
use std::sync::Arc;

use meron_core::engine::Engine;
use meron_core::openpgp::{keyring, outgoing, store as pgp_store};
use meron_core::protocol::Request;
use meron_core::store;

use crate::Writer;
use crate::sidecar::params::*;

/// File contents from the frontend: text (`data`) or base64 (`data_b64`, for
/// binary `.gpg` keyrings).
fn file_data(p: &Value) -> anyhow::Result<Vec<u8>> {
    if let Some(b64) = p.get("data_b64").and_then(Value::as_str) {
        return base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .context("the file couldn't be read");
    }
    Ok(req_str(p, "data")?.into_bytes())
}

/// The account's primary address first, then its aliases.
fn account_addrs(conn: &rusqlite::Connection, account: &str) -> Vec<String> {
    let primary: Option<String> = conn
        .query_row("SELECT email FROM accounts WHERE id = ?1", [account], |r| {
            r.get(0)
        })
        .optional()
        .ok()
        .flatten()
        .flatten();
    let mut out: Vec<String> = primary
        .map(|e| e.trim().to_ascii_lowercase())
        .into_iter()
        .filter(|e| !e.is_empty())
        .collect();
    for alias in store::account_aliases(conn, account).unwrap_or_default() {
        let e = alias.email.trim().to_ascii_lowercase();
        if !e.is_empty() && !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    _out: &Writer,
) -> anyhow::Result<Value> {
    let p = &req.params;
    let engine = engine.clone();
    let method = req.method.clone();
    let p = p.clone();
    // Key generation, S2K and self-tests are CPU work: off the reactor.
    tokio::task::spawn_blocking(move || -> anyhow::Result<Value> {
        let conn = engine.db.lock().unwrap();
        match method.as_str() {
            "pgp.account.state" => {
                let account = req_str(&p, "account")?;
                Ok(json!(pgp_store::account_state(&conn, &account)?))
            }
            "pgp.account.create" => {
                let account = req_str(&p, "account")?;
                let addrs = account_addrs(&conn, &account);
                anyhow::ensure!(!addrs.is_empty(), "this account has no address");
                let key = pgp_store::create_key(&conn, &account, &addrs)?;
                Ok(json!({ "key": key }))
            }
            "pgp.account.enable" => {
                let account = req_str(&p, "account")?;
                Ok(json!({ "enabled": pgp_store::enable_existing(&conn, &account)? }))
            }
            "pgp.account.disable" => {
                let account = req_str(&p, "account")?;
                pgp_store::disable(&conn, &account)?;
                Ok(json!({ "ok": true }))
            }
            "pgp.account.importKey" => {
                let account = req_str(&p, "account")?;
                let data = zeroize::Zeroizing::new(file_data(&p)?);
                let passphrase = p.get("passphrase").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| zeroize::Zeroizing::new(s.to_string()));
                let fingerprint = p.get("fingerprint").and_then(Value::as_str);
                let addrs = account_addrs(&conn, &account);
                let result = pgp_store::import_own(&conn, &account, &addrs, &data, passphrase.as_deref().map(String::as_str), fingerprint)?;
                if matches!(result, pgp_store::ImportOwn::Imported { .. }) {
                    let refetch = pgp_store::forget_undecryptable(&conn)?;
                    return Ok(json!({ "result": result, "refetch": refetch }));
                }
                Ok(json!({ "result": result }))
            }
            "pgp.account.exportPublic" => {
                let account = req_str(&p, "account")?;
                Ok(json!({ "armored": pgp_store::export_public(&conn, &account)? }))
            }
            "pgp.account.exportSecret" => {
                let account = req_str(&p, "account")?;
                let passphrase = zeroize::Zeroizing::new(req_str(&p, "passphrase")?);
                Ok(json!({ "armored": pgp_store::export_secret(&conn, &account, &passphrase)? }))
            }
            "pgp.account.deleteKey" => {
                let account = req_str(&p, "account")?;
                let fingerprint = req_str(&p, "fingerprint")?;
                pgp_store::delete_own_key(&conn, &account, &fingerprint)?;
                Ok(json!({ "ok": true }))
            }
            "pgp.peers.list" => Ok(json!({ "peers": pgp_store::list_peers(&conn)? })),
            "pgp.peers.importPreview" => {
                let data = file_data(&p)?;
                Ok(json!(pgp_store::import_preview(&conn, &data)?))
            }
            "pgp.peers.import" => {
                let data = file_data(&p)?;
                let fingerprints: Vec<String> = p
                    .get("fingerprints")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default();
                Ok(json!({ "imported": pgp_store::import_peers(&conn, &data, &fingerprints)? }))
            }
            "pgp.peers.setVerified" => {
                let addr = req_str(&p, "addr")?;
                let fingerprint = req_str(&p, "fingerprint")?;
                let verified = p.get("verified").and_then(Value::as_bool).unwrap_or(true);
                let confirm = req_str(&p, "confirm").unwrap_or_default();
                pgp_store::set_verified(&conn, &addr, &fingerprint, &confirm, verified)?;
                Ok(json!({ "ok": true }))
            }
            "pgp.peers.remove" => {
                let addr = req_str(&p, "addr")?;
                let fingerprint = req_str(&p, "fingerprint")?;
                pgp_store::remove_peer(&conn, &addr, &fingerprint)?;
                Ok(json!({ "ok": true }))
            }
            "pgp.offer.dismiss" => {
                let account = req_str(&p, "account")?;
                let addr = req_str(&p, "addr")?;
                pgp_store::dismiss_offer(&conn, &account, &addr)?;
                Ok(json!({ "ok": true }))
            }
            // Key lookup: only ever on the user's click (it tells the servers
            // who the user writes to). Network-bound, but already off the reactor.
            "pgp.lookup" => {
                let addr = req_str(&p, "addr")?;
                let sources = meron_core::openpgp::lookup::sources(&conn);
                drop(conn);
                let result = meron_core::openpgp::lookup::lookup(&addr, &sources)?;
                // Copies of keys we already store (a revocation, a renewal) are
                // merged right away: only changes signed by the key itself or
                // its designated revoker are taken.
                let conn = engine.db.lock().unwrap();
                let mut updated = Vec::new();
                for key in &result.all_keys {
                    if pgp_store::merge_into_stored(&conn, key)? {
                        updated.push(meron_core::openpgp::keys::info(key, false, meron_core::openpgp::keys::now()));
                    }
                }
                if !updated.is_empty() {
                    pgp_store::reload(&conn)?;
                }
                let mut value = json!(result);
                value["updated"] = json!(updated);
                Ok(value)
            }
            "pgp.lookup.add" => {
                let addr = req_str(&p, "addr")?;
                let data = file_data(&p)?;
                let from_wkd = p.get("source").and_then(Value::as_str) == Some("wkd");
                Ok(json!({ "key": pgp_store::add_looked_up(&conn, &addr, &data, from_wkd)? }))
            }
            "pgp.lookup.sources" => Ok(json!({
                "sources": meron_core::openpgp::lookup::sources(&conn),
                "defaults": meron_core::openpgp::lookup::default_sources(),
            })),
            "pgp.lookup.setSources" => {
                let list: Vec<meron_core::openpgp::lookup::KeySource> =
                    serde_json::from_value(p.get("sources").cloned().unwrap_or_default()).context("sources")?;
                meron_core::openpgp::lookup::set_sources(&conn, &list)?;
                Ok(json!({ "sources": meron_core::openpgp::lookup::sources(&conn) }))
            }
            // The composer's live check: can this message be encrypted, and
            // which recipients lack a key.
            // For the MCP approval preview: a reply to encrypted mail goes out
            // encrypted whatever the tool asked.
            "pgp.reply.encrypted" => {
                let account = req_str(&p, "account")?;
                let in_reply_to = req_str(&p, "in_reply_to").unwrap_or_default();
                Ok(json!({ "encrypted": pgp_store::answers_encrypted(&conn, &account, &in_reply_to)? }))
            }
            "pgp.keys.forRecipients" => {
                let account = req_str(&p, "account")?;
                let addrs: Vec<String> = p
                    .get("addrs")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default();
                let ring = keyring::current();
                // Keys that differ from the one last used for that address:
                // the composer asks before sending.
                let changed = pgp_store::key_changes(&conn, &outgoing::chosen_peer_keys(&ring, &addrs))?;
                drop(conn);
                let own = ring.default_key(&account).filter(|_| ring.is_enabled(&account));
                let own_unusable = own.is_some_and(|k| meron_core::openpgp::keys::check(&k.public, meron_core::openpgp::keys::now()).is_some());
                let missing = outgoing::missing_keys(&ring, &addrs);
                Ok(json!({
                    "enabled": own.is_some(),
                    "own_key_unusable": own_unusable,
                    "missing": missing,
                    "changed": changed,
                }))
            }
            other => anyhow::bail!("unknown method: {other}"),
        }
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    // Inline PGP mail has no `multipart/encrypted` type to remember it by: once
    // its undecryptable status is dropped for a new attempt, a reply sent before
    // that attempt must still be encrypted.
    #[test]
    fn undecryptable_mail_forgotten_for_a_new_key_still_answers_encrypted() {
        let conn = meron_core::store::open_at(":memory:").unwrap();
        let json = serde_json::json!({
            "message_id": "inline@example.com",
            "crypto": { "encrypted": true, "problem": "no_key" },
        });
        conn.execute(
            "INSERT INTO messages (account, folder, msg_id, uid, subject, from_name, from_addr, date, body, json)
             VALUES ('a', 'INBOX', 1, 1, 's', 'n', 'x@example.com', 0, 'body', ?1)",
            [json.to_string()],
        )
        .unwrap();
        let answers = |conn: &rusqlite::Connection| {
            pgp_store::answers_encrypted(conn, "a", "<inline@example.com>").unwrap()
        };
        assert!(answers(&conn));

        assert_eq!(pgp_store::forget_undecryptable(&conn).unwrap(), 1);

        let body_dropped: bool = conn
            .query_row("SELECT body IS NULL FROM messages", [], |r| r.get(0))
            .unwrap();
        assert!(body_dropped, "fetched again with the new key");
        assert!(answers(&conn));
    }
}
