//! OpenPGP keys in the SQLCipher store (tables from migrations 0029 and 0034).
//!
//! Secret keys are stored unprotected inside the encrypted database, like
//! Delta Chat's `keypairs` (decision 2026-11-01): the database key already
//! lives in the OS keyring, so a second keyring-held layer would add no
//! protection. Every load self-tests the key (sign + verify), which also
//! catches a key whose public half was tampered with.

use anyhow::{Context as _, Result, bail};
use rusqlite::{Connection, OptionalExtension as _, params};
use serde::Serialize;
use std::collections::HashMap;

use super::autocrypt::Announced;
use super::keyring::{self, Keyring, PeerKey, Source};
use super::keys::{self, KeyInfo, Unusable, fingerprint_hex};

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Path of the app's main store. Keys live there only: the Local Folders
/// archive saves messages through the same code, and must neither record
/// keys into its own (unused) tables nor swap in an empty keyring.
static MAIN_DB: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Called once by the main engine: remembers its store and loads the keys.
pub fn init_main(conn: &Connection) -> Result<()> {
    if let Some(path) = conn.path().filter(|p| !p.is_empty()) {
        let _ = MAIN_DB.set(path.to_string());
    }
    reparse_key_attachments_once(conn)?;
    reparse_signer_mismatches_once(conn)?;
    refetch_encrypted_cached_without_openpgp_once(conn)?;
    // No sweep over the messages here: placeholder reply subjects are repaired
    // per thread when a decrypted message is saved (`save_cached_message`), so
    // opening the store reads no rows, encryption or not.
    reload(conn)
}

const REPARSE_MARK: &str = "openpgp_reparse_key_attachments";

/// One-time: messages cached before attached sender keys were hidden (and
/// recorded) still list the key file. Their cached bodies are dropped so the
/// next read fetches and parses them again. Runs once per store.
fn reparse_key_attachments_once(conn: &Connection) -> Result<usize> {
    let done = crate::store::settings_get(conn, &[REPARSE_MARK.to_string()])?
        .get(REPARSE_MARK)
        .is_some_and(|v| !v.is_null());
    if done {
        return Ok(0);
    }
    let n = conn.execute(
        &format!(
            "UPDATE messages SET body = NULL, json = {DROP_CACHED_BODY_JSON}
         WHERE files LIKE '%application/pgp-keys%'"
        ),
        [],
    )?;
    crate::store::setting_set(conn, REPARSE_MARK, &serde_json::json!(true))?;
    Ok(n)
}

/// Older builds also called weak identity bindings a sender mismatch.
/// Fetch those messages again once so the corrected reason reaches the UI.
fn reparse_signer_mismatches_once(conn: &Connection) -> Result<usize> {
    const MARK: &str = "openpgp_reparse_signer_mismatches";
    if crate::store::settings_get(conn, &[MARK.to_string()])?
        .get(MARK)
        .is_some_and(|v| !v.is_null())
    {
        return Ok(0);
    }
    let n = conn.execute(
        &format!(
            "UPDATE messages SET body = NULL, json = {DROP_CACHED_BODY_JSON}
             WHERE body IS NOT NULL AND json_extract(json, '$.crypto.problem') = 'signer_mismatch'"
        ),
        [],
    )?;
    crate::store::setting_set(conn, MARK, &serde_json::json!(true))?;
    Ok(n)
}

/// A cached row's `json` with its parsed body dropped (so the next read fetches
/// it again): the body HTML, attachments and OpenPGP status go, but an
/// encrypted message stays known as encrypted (`$.encrypted`), so a reply sent
/// before it is fetched again is still encrypted (`answers_encrypted`).
pub const DROP_CACHED_BODY_JSON: &str = "CASE WHEN json_extract(json, '$.crypto.encrypted') = 1
       THEN json_set(json_remove(json, '$.body_html', '$.attachments', '$.crypto'), '$.encrypted', json('true'))
       ELSE json_remove(json, '$.body_html', '$.attachments', '$.crypto') END";

const REFETCH_MARK: &str = "openpgp_refetch_encrypted_cached_without_openpgp";

/// One-time: encrypted messages whose bodies a build without OpenPGP cached.
/// Such a row has no `crypto` entry, an `encrypted.asc` file (or the PGP/MIME
/// version part) instead of a body, or an inline PGP block, and keeps the
/// protected-headers placeholder as its subject. A cached body is never
/// fetched again, so without this the message would stay undecrypted however
/// often it is opened. Dropping the body makes the next read (or the prefetch)
/// fetch and decrypt it, which also writes back the real subject. Runs once
/// per store; rows the OpenPGP build cached carry `crypto` and are left alone.
fn refetch_encrypted_cached_without_openpgp_once(conn: &Connection) -> Result<usize> {
    let done = crate::store::settings_get(conn, &[REFETCH_MARK.to_string()])?
        .get(REFETCH_MARK)
        .is_some_and(|v| !v.is_null());
    if done {
        return Ok(0);
    }
    let n = conn.execute(
        &format!(
            "UPDATE messages SET body = NULL, json = {DROP_CACHED_BODY_JSON}
         WHERE body IS NOT NULL
           AND json_extract(json, '$.crypto') IS NULL
           AND (files LIKE '%\"encrypted.asc\"%'
                OR files LIKE '%application/pgp-encrypted%'
                OR body LIKE '%-----BEGIN PGP MESSAGE-----%'
                OR trim(subject) IN ('...', '[...]'))"
        ),
        [],
    )?;
    crate::store::setting_set(conn, REFETCH_MARK, &serde_json::json!(true))?;
    Ok(n)
}

/// Messages in `folder` still titled with the protected-headers placeholder
/// and without a cached body, whatever their age: encrypted mail never
/// decrypted. The prefetcher adds them to its 14-day window so their real
/// subject arrives without the user opening each one.
pub fn undecrypted_placeholder_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Vec<u32>> {
    // Replies count too ("Re: [...]"): the placeholder test strips prefixes.
    let mut stmt = conn.prepare(
        "SELECT uid, subject FROM messages
         WHERE account = ?1 AND folder = ?2 AND uid <> 0 AND body IS NULL
           AND subject LIKE '%...%'
         ORDER BY uid",
    )?;
    let rows = stmt
        .query_map(params![account, folder], |r| {
            Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, subject)| crate::store::is_encrypted_placeholder_subject(subject))
        .map(|(uid, _)| uid)
        .collect())
}

/// Whether `conn` is the store keys belong to. Before the main engine
/// registers a database, any connection can act as the main store.
fn is_main(conn: &Connection) -> bool {
    match (MAIN_DB.get(), conn.path()) {
        (Some(main), Some(path)) => main == path,
        _ => true,
    }
}

/// Loads every key into the shared [`keyring`] snapshot. A stored secret key
/// that fails its self-test is skipped (and stays in the table for the user
/// to see), never used.
pub fn reload(conn: &Connection) -> Result<()> {
    if !is_main(conn) {
        return Ok(());
    }
    keyring::replace(load(conn)?);
    Ok(())
}

pub fn load(conn: &Connection) -> Result<Keyring> {
    let mut ring = Keyring::default();
    let mut stmt =
        conn.prepare("SELECT account_id, secret_key, is_default FROM openpgp_own_keys")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, bool>(2)?,
        ))
    })?;
    for row in rows {
        let (account, blob, is_default) = row?;
        let blob = zeroize::Zeroizing::new(blob);
        let Ok(secret) = keys::parse_secret(&blob) else {
            continue;
        };
        if keys::self_test(&secret).is_err() {
            continue;
        }
        ring.own
            .push(keyring::own_key(&account, secret, is_default));
    }
    let mut stmt = conn.prepare(
        "SELECT addr, fingerprint, public_key, source, verified, last_seen FROM openpgp_peer_keys",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Vec<u8>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })?;
    for row in rows {
        let (addr, fingerprint, blob, source, verified, last_seen) = row?;
        let Ok(key) = keys::parse_public(&blob) else {
            continue;
        };
        ring.peers.push(PeerKey {
            addr,
            fingerprint,
            key,
            source: Source::parse(&source),
            verified,
            last_seen,
        });
    }
    let mut stmt = conn.prepare("SELECT account_id FROM openpgp_accounts WHERE enabled = 1")?;
    ring.enabled_accounts = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut stmt = conn.prepare("SELECT account_id FROM openpgp_sign_plain")?;
    ring.sign_plain_accounts = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut stmt = conn.prepare("SELECT account_id, addr FROM openpgp_offer_dismissed")?;
    ring.dismissed = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ring)
}

/// The account's encryption state, for its settings card.
#[derive(Debug, Serialize)]
pub struct AccountState {
    pub enabled: bool,
    /// Mail that isn't encrypted goes out signed (`set_sign_plain`).
    pub sign_plain: bool,
    /// The key used for signing and Autocrypt.
    pub key: Option<KeyInfo>,
    /// Older own keys, kept so earlier mail still opens.
    pub previous: Vec<KeyInfo>,
}

pub fn account_state(conn: &Connection, account_id: &str) -> Result<AccountState> {
    let enabled = conn
        .query_row(
            "SELECT enabled FROM openpgp_accounts WHERE account_id = ?1",
            [account_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false);
    let mut key = None;
    let mut previous = Vec::new();
    let mut stmt = conn.prepare("SELECT secret_key, is_default FROM openpgp_own_keys WHERE account_id = ?1 ORDER BY added_at DESC")?;
    let rows = stmt.query_map([account_id], |r| {
        Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, bool>(1)?))
    })?;
    for row in rows {
        let (blob, is_default) = row?;
        let blob = zeroize::Zeroizing::new(blob);
        let Ok(secret) = keys::parse_secret(&blob) else {
            continue;
        };
        let info = keys::secret_info(&secret, keys::now());
        if is_default && key.is_none() {
            key = Some(info);
        } else {
            previous.push(info);
        }
    }
    Ok(AccountState {
        enabled,
        sign_plain: sign_plain(conn, account_id)?,
        key,
        previous,
    })
}

fn set_enabled(conn: &Connection, account_id: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO openpgp_accounts (account_id, enabled) VALUES (?1, ?2)
         ON CONFLICT(account_id) DO UPDATE SET enabled = excluded.enabled",
        params![account_id, enabled],
    )?;
    Ok(())
}

fn sign_plain(conn: &Connection, account_id: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM openpgp_sign_plain WHERE account_id = ?1)",
        [account_id],
        |r| r.get(0),
    )?)
}

/// "Sign unencrypted mail": whether the account's mail that isn't encrypted
/// goes out signed (PGP/MIME `multipart/signed`). Encrypted mail is always
/// signed; this only speaks to the rest.
pub fn set_sign_plain(conn: &Connection, account_id: &str, sign: bool) -> Result<()> {
    if sign {
        conn.execute(
            "INSERT OR IGNORE INTO openpgp_sign_plain (account_id) VALUES (?1)",
            [account_id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM openpgp_sign_plain WHERE account_id = ?1",
            [account_id],
        )?;
    }
    reload(conn)
}

/// Stores `secret` as the account's default key (earlier keys stay for
/// decrypting) and switches encryption on.
fn store_default_key(
    conn: &Connection,
    account_id: &str,
    secret: &pgp::composed::SignedSecretKey,
) -> Result<()> {
    keys::self_test(secret).context("the key failed its self-test")?;
    let fingerprint = fingerprint_hex(&secret.primary_key);
    let blob = keys::binary_secret(secret)?;
    let tx = conn.unchecked_transaction()?;
    // The same key imported again: a renewed or extended export replaces the
    // stored copy, an older one doesn't (see `keys::secret_supersedes`).
    let stored: Option<Vec<u8>> = tx
        .query_row(
            "SELECT secret_key FROM openpgp_own_keys WHERE fingerprint = ?1",
            [&fingerprint],
            |r| r.get(0),
        )
        .optional()?;
    let replace = stored
        .map(zeroize::Zeroizing::new)
        .and_then(|stored| keys::parse_secret(&stored).ok())
        .is_none_or(|stored| keys::secret_supersedes(secret, &stored));
    tx.execute(
        "UPDATE openpgp_own_keys SET is_default = 0 WHERE account_id = ?1",
        [account_id],
    )?;
    tx.execute(
        "INSERT INTO openpgp_own_keys (fingerprint, account_id, secret_key, is_default, added_at)
         VALUES (?1, ?2, ?3, 1, ?4)
         ON CONFLICT(fingerprint) DO UPDATE SET account_id = excluded.account_id, is_default = 1,
             secret_key = CASE WHEN ?5 THEN excluded.secret_key ELSE secret_key END",
        params![
            fingerprint,
            account_id,
            blob.as_slice(),
            now_secs(),
            replace
        ],
    )?;
    tx.execute(
        "INSERT INTO openpgp_accounts (account_id, enabled) VALUES (?1, 1)
         ON CONFLICT(account_id) DO UPDATE SET enabled = 1",
        [account_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// "Create a new key": generates one for the account's addresses.
pub fn create_key(conn: &Connection, account_id: &str, addrs: &[String]) -> Result<KeyInfo> {
    let secret = keys::generate(addrs)?;
    store_default_key(conn, account_id, &secret)?;
    reload(conn)?;
    Ok(keys::secret_info(&secret, keys::now()))
}

/// Turns encryption back on with the account's existing default key.
pub fn enable_existing(conn: &Connection, account_id: &str) -> Result<bool> {
    let has_key: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM openpgp_own_keys WHERE account_id = ?1 AND is_default = 1)",
        [account_id],
        |r| r.get(0),
    )?;
    if has_key {
        set_enabled(conn, account_id, true)?;
        reload(conn)?;
    }
    Ok(has_key)
}

pub fn disable(conn: &Connection, account_id: &str) -> Result<()> {
    set_enabled(conn, account_id, false)?;
    reload(conn)
}

/// Outcome of "Use my existing key".
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ImportOwn {
    Imported {
        key: KeyInfo,
        address_matches: bool,
    },
    /// The file holds several secret keys: the UI asks which.
    Choose {
        keys: Vec<KeyInfo>,
    },
    NeedsPassphrase {
        key: KeyInfo,
    },
    /// The file is a backup encrypted with a code (OpenKeychain's "Backup
    /// keys", an Autocrypt Setup Message): the UI asks for it. `wrong` when
    /// the code given didn't open it.
    NeedsBackupCode {
        wrong: bool,
    },
    OnCard,
    NoSecretKey,
}

/// "Use my existing key": parses `data` (a key file, or a backup opened with
/// `backup_code`), unlocks the chosen key with `passphrase`, self-tests and
/// stores it as the account's default.
pub fn import_own(
    conn: &Connection,
    account_id: &str,
    account_addrs: &[String],
    data: &[u8],
    passphrase: Option<&str>,
    backup_code: Option<&str>,
    fingerprint: Option<&str>,
) -> Result<ImportOwn> {
    let mut secrets = keys::parse_keys(data)?.secret;
    if secrets.is_empty() {
        // Not a key file: perhaps a backup encrypted with a code.
        match super::crypt::open_backup(data, backup_code) {
            super::crypt::Backup::NotOne => {}
            super::crypt::Backup::NeedsCode => {
                return Ok(ImportOwn::NeedsBackupCode {
                    wrong: backup_code.is_some(),
                });
            }
            super::crypt::Backup::Opened(plain) => secrets = keys::parse_keys(&plain)?.secret,
        }
    }
    if secrets.is_empty() {
        return Ok(ImportOwn::NoSecretKey);
    }
    let chosen = match fingerprint {
        Some(f) => secrets
            .into_iter()
            .find(|k| fingerprint_hex(&k.primary_key) == f)
            .context("that key isn't in the file")?,
        None if secrets.len() > 1 => {
            return Ok(ImportOwn::Choose {
                keys: secrets
                    .iter()
                    .map(|k| keys::secret_info(k, keys::now()))
                    .collect(),
            });
        }
        None => secrets.remove(0),
    };
    let info = keys::secret_info(&chosen, keys::now());
    match keys::unlock(chosen, passphrase)? {
        keys::Unlock::NeedsPassphrase => Ok(ImportOwn::NeedsPassphrase { key: info }),
        keys::Unlock::OnCard => Ok(ImportOwn::OnCard),
        keys::Unlock::Ok(secret) => {
            store_default_key(conn, account_id, &secret)?;
            // Our own mail signed with it (sent from another device, say) was
            // checked without it: read it again.
            recheck_signed_by(conn, &secret.to_public_key());
            reload(conn)?;
            let lower: Vec<String> = account_addrs
                .iter()
                .map(|a| a.to_ascii_lowercase())
                .collect();
            let address_matches = info.emails.iter().any(|e| lower.contains(e));
            Ok(ImportOwn::Imported {
                key: info,
                address_matches,
            })
        }
    }
}

fn default_secret(conn: &Connection, account_id: &str) -> Result<pgp::composed::SignedSecretKey> {
    let blob: Vec<u8> = conn
        .query_row(
            "SELECT secret_key FROM openpgp_own_keys WHERE account_id = ?1 AND is_default = 1",
            [account_id],
            |r| r.get(0),
        )
        .optional()?
        .context("this account has no encryption key")?;
    let blob = zeroize::Zeroizing::new(blob);
    keys::parse_secret(&blob)
}

pub fn export_public(conn: &Connection, account_id: &str) -> Result<String> {
    keys::armored_public(&default_secret(conn, account_id)?.to_public_key())
}

/// The secret key, protected with `passphrase` (never returned unprotected).
pub fn export_secret(conn: &Connection, account_id: &str, passphrase: &str) -> Result<String> {
    keys::export_protected(&default_secret(conn, account_id)?, passphrase)
}

/// Deletes one of the account's own keys. Mail encrypted to it can no longer
/// be opened, which the UI says before calling this.
pub fn delete_own_key(conn: &Connection, account_id: &str, fingerprint: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM openpgp_own_keys WHERE account_id = ?1 AND fingerprint = ?2",
        params![account_id, fingerprint],
    )?;
    let remaining: i64 = conn.query_row(
        "SELECT COUNT(*) FROM openpgp_own_keys WHERE account_id = ?1",
        [account_id],
        |r| r.get(0),
    )?;
    if remaining == 0 {
        set_enabled(conn, account_id, false)?;
    }
    reload(conn)
}

// Peers ---------------------------------------------------------------------

/// A public key from this database by primary fingerprint (designated
/// revokers are resolved from the store the merge writes to, not the global
/// snapshot).
fn db_key(conn: &Connection, fingerprint: &str) -> Option<pgp::composed::SignedPublicKey> {
    let peer: Option<Vec<u8>> = conn
        .query_row(
            "SELECT public_key FROM openpgp_peer_keys WHERE fingerprint = ?1 LIMIT 1",
            [fingerprint],
            |r| r.get(0),
        )
        .ok();
    if let Some(blob) = peer {
        return keys::parse_public(&blob).ok();
    }
    let own: Option<Vec<u8>> = conn
        .query_row(
            "SELECT secret_key FROM openpgp_own_keys WHERE fingerprint = ?1",
            [fingerprint],
            |r| r.get(0),
        )
        .ok();
    own.map(zeroize::Zeroizing::new)
        .and_then(|b| keys::parse_secret(&b).ok())
        .map(|k| k.to_public_key())
}

/// Folds `key` into the stored copy with the same fingerprint (every
/// address row), so revocations and renewals published later take effect.
/// Stored key bytes are only ever extended through [`keys::merge`] (which keeps
/// only signatures made by the key itself), never replaced: an older copy
/// arriving late can't undo a revocation. Returns whether anything changed.
pub fn merge_into_stored(conn: &Connection, key: &pgp::composed::SignedPublicKey) -> Result<bool> {
    let fingerprint = fingerprint_hex(&key.primary_key);
    let stored: Option<Vec<u8>> = conn
        .query_row(
            "SELECT public_key FROM openpgp_peer_keys WHERE fingerprint = ?1 LIMIT 1",
            [&fingerprint],
            |r| r.get(0),
        )
        .optional()?;
    let Some(blob) = stored else { return Ok(false) };
    let Ok(stored) = keys::parse_public(&blob) else {
        return Ok(false);
    };
    let Some(merged) = keys::merge_with(&stored, key, &|f| db_key(conn, f)) else {
        return Ok(false);
    };
    conn.execute(
        "UPDATE openpgp_peer_keys SET public_key = ?1 WHERE fingerprint = ?2",
        params![keys::binary_public(&merged)?, fingerprint],
    )?;
    // A renewal (or a new signing subkey) can make a signature that failed
    // against the old copy (expired then, say) verify now.
    recheck_signed_by(conn, &merged);
    Ok(true)
}

/// Records keys announced in a message (Autocrypt, gossip) or attached to it.
/// Returns whether anything changed, so callers reload the snapshot only then.
///
/// TOFU rules: a key never seen for the address is added; a known one gets its
/// `last_seen` moved forward (never back: old mail fetched late must not win);
/// gossip only fills in addresses with no key at all, so a group member can't
/// swap someone else's key.
pub fn ingest(
    conn: &Connection,
    announced: &[Announced],
    attached: &[(String, pgp::composed::SignedPublicKey)],
    message_date: i64,
) -> Result<bool> {
    // In-memory tests exercise ingestion independently of the file-backed
    // main store that another test may register in this process.
    let in_memory_test = cfg!(test) && conn.path() == Some("");
    if !in_memory_test && !is_main(conn) {
        return Ok(false);
    }
    let mut changed = false;
    // The Date header is the sender's claim: never later than now (Autocrypt
    // Level 1's "effective date"), or one forged mail dated years ahead would
    // stay the newest and choose this address's key for good.
    let now = now_secs();
    let seen = if message_date > 0 {
        message_date.min(now)
    } else {
        now
    };
    let mut put = |addr: &str,
                   key: &pgp::composed::SignedPublicKey,
                   source: Source,
                   only_if_new_addr: bool|
     -> Result<()> {
        // A copy of a key we already have (revoked, renewed) updates it first.
        changed |= merge_into_stored(conn, key)?;
        if keys::check_with(key, keys::now(), &|f| db_key(conn, f))
            .is_some_and(|u| u != Unusable::Expired)
        {
            return Ok(());
        }
        let addr = addr.to_ascii_lowercase();
        if only_if_new_addr {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM openpgp_peer_keys WHERE addr = ?1)",
                [&addr],
                |r| r.get(0),
            )?;
            if exists {
                return Ok(());
            }
        }
        let fingerprint = fingerprint_hex(&key.primary_key);
        let blob = keys::binary_public(&keys::minimize(key))?;
        let known: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM openpgp_peer_keys WHERE fingerprint = ?1)",
            [&fingerprint],
            |r| r.get(0),
        )?;
        let n = conn.execute(
            "INSERT INTO openpgp_peer_keys (addr, fingerprint, public_key, source, verified, first_seen, last_seen)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)
             ON CONFLICT(addr, fingerprint) DO UPDATE SET
               last_seen = MAX(openpgp_peer_keys.last_seen, excluded.last_seen)
             WHERE excluded.last_seen > openpgp_peer_keys.last_seen",
            params![addr, fingerprint, blob, source.as_str(), seen],
        )?;
        changed |= n > 0;
        if n > 0 && !known {
            recheck_signed_by(conn, key);
        }
        Ok(())
    };
    for a in announced {
        // A gossiped key must carry the gossiped address in its own user IDs,
        // so a group member can't gossip a key that isn't for that address.
        if a.gossip {
            let addr = a.addr.to_ascii_lowercase();
            if !keys::info(&a.key, false, keys::now())
                .emails
                .iter()
                .any(|e| *e == addr)
            {
                continue;
            }
        }
        put(
            &a.addr,
            &a.key,
            if a.gossip {
                Source::Gossip
            } else {
                Source::Autocrypt
            },
            a.gossip,
        )?;
    }
    for (addr, key) in attached {
        put(addr, key, Source::Attachment, false)?;
    }
    Ok(changed)
}

/// Whether `in_reply_to` names a cached message of `account` that arrived
/// encrypted. The core decides a reply's protection from this, not from what
/// the caller sends.
pub fn answers_encrypted(conn: &Connection, account: &str, in_reply_to: &str) -> Result<bool> {
    let first = in_reply_to.split_whitespace().next().unwrap_or("");
    let id = first.trim_start_matches('<').trim_end_matches('>').trim();
    if id.is_empty() {
        return Ok(false);
    }
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM messages
           WHERE account = ?1
             AND lower(COALESCE(json_extract(json, '$.message_id'), '')) = lower(?2)
             AND (json_extract(json, '$.crypto.encrypted') = 1
                  OR json_extract(json, '$.encrypted') = 1))",
        params![account, id],
        |r| r.get(0),
    )?)
}

/// Whether a send or draft is encrypted, by `outgoing::protection` with the
/// answered message looked up here; the second value is true when it is
/// encrypted only because it answers encrypted mail (nothing was asked).
pub fn reply_protection(
    conn: &Connection,
    account: &str,
    in_reply_to: &str,
    encrypt: bool,
    plain: bool,
) -> Result<(bool, bool)> {
    use super::outgoing::{Protection, protection};
    let answers = answers_encrypted(conn, account, in_reply_to)?;
    let encrypted = protection(encrypt, plain, answers) == Protection::Encrypt;
    Ok((encrypted, encrypted && !encrypt))
}

/// A recipient whose key would differ from the one Meron last encrypted to
/// for that address (the composer asks before sending).
#[derive(Debug, Clone, Serialize)]
pub struct KeyChange {
    pub addr: String,
    /// Primary fingerprint last used for this address.
    pub previous: String,
    /// Primary fingerprint Meron would use now.
    pub current: String,
}

/// Remembers which key each recipient's mail was just encrypted to.
pub fn record_used(conn: &Connection, used: &[(String, String)]) -> Result<()> {
    let now = now_secs();
    for (addr, fingerprint) in used {
        conn.execute(
            "INSERT INTO openpgp_key_used (addr, fingerprint, used_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(addr) DO UPDATE SET fingerprint = excluded.fingerprint, used_at = excluded.used_at",
            params![addr.to_ascii_lowercase(), fingerprint, now],
        )?;
    }
    Ok(())
}

/// Of the keys a send would use (`chosen`: address, fingerprint), those that
/// differ from the key last used for that address. An address never written
/// to encrypted has nothing to compare and is not listed.
pub fn key_changes(conn: &Connection, chosen: &[(String, String)]) -> Result<Vec<KeyChange>> {
    let mut out = Vec::new();
    for (addr, current) in chosen {
        let addr = addr.to_ascii_lowercase();
        let previous: Option<String> = conn
            .query_row(
                "SELECT fingerprint FROM openpgp_key_used WHERE addr = ?1",
                [&addr],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous.filter(|p| p != current) {
            out.push(KeyChange {
                addr,
                previous,
                current: current.clone(),
            });
        }
    }
    Ok(out)
}

/// A peer key row for the Settings → Encryption page.
#[derive(Debug, Serialize)]
pub struct PeerRow {
    pub addr: String,
    pub source: Source,
    pub verified: bool,
    pub first_seen: i64,
    pub last_seen: i64,
    pub info: KeyInfo,
    /// The address has a verified key and this is a different one.
    pub conflicts_with_verified: bool,
}

pub fn list_peers(conn: &Connection) -> Result<Vec<PeerRow>> {
    let mut stmt = conn.prepare(
        "SELECT addr, public_key, source, verified, first_seen, last_seen FROM openpgp_peer_keys ORDER BY addr, last_seen DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, bool>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })?;
    let mut out: Vec<PeerRow> = Vec::new();
    for row in rows {
        let (addr, blob, source, verified, first_seen, last_seen) = row?;
        let Ok(key) = keys::parse_public(&blob) else {
            continue;
        };
        out.push(PeerRow {
            addr,
            source: Source::parse(&source),
            verified,
            first_seen,
            last_seen,
            info: keys::info(&key, false, keys::now()),
            conflicts_with_verified: false,
        });
    }
    let verified: Vec<(String, String)> = out
        .iter()
        .filter(|r| r.verified)
        .map(|r| (r.addr.clone(), r.info.fingerprint.clone()))
        .collect();
    for row in &mut out {
        row.conflicts_with_verified = !row.verified
            && verified
                .iter()
                .any(|(a, f)| *a == row.addr && *f != row.info.fingerprint);
    }
    Ok(out)
}

/// Summary of a public-key import, shown before anything is stored.
#[derive(Debug, Serialize, Default)]
pub struct ImportPreview {
    pub ready: Vec<KeyInfo>,
    pub skipped: Vec<SkippedKey>,
    pub already_present: Vec<KeyInfo>,
    /// Would replace (take precedence over) an unverified key for that address.
    pub replaces_unverified: Vec<String>,
    /// Differs from a verified key for that address; imported but not preferred.
    pub conflicts_with_verified: Vec<String>,
    /// Keys already stored that this file updates (a revocation, a renewal).
    /// Applied on import whether or not they are among the picked keys.
    pub updates: Vec<KeyInfo>,
    pub unreadable: usize,
}

#[derive(Debug, Serialize)]
pub struct SkippedKey {
    pub info: KeyInfo,
    pub reason: SkipReason,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    Expired,
    Revoked,
    InvalidBindings,
    NoEncryptionKey,
    NoEmail,
}

fn peer_keys_in(data: &[u8]) -> Result<(Vec<pgp::composed::SignedPublicKey>, usize)> {
    let parsed = keys::parse_keys(data)?;
    let mut all = parsed.public;
    // A secret-key file shared by mistake: only its public half is used.
    all.extend(parsed.secret.into_iter().map(|s| s.to_public_key()));
    Ok((all, parsed.unreadable))
}

pub fn import_preview(conn: &Connection, data: &[u8]) -> Result<ImportPreview> {
    let (all, unreadable) = peer_keys_in(data)?;
    let ring = load(conn)?;
    let mut out = ImportPreview {
        unreadable,
        ..Default::default()
    };
    for key in all {
        let info = keys::info(&key, false, keys::now());
        let fpr = info.fingerprint.clone();
        if let Some(stored) = ring.peers.iter().find(|p| p.fingerprint == fpr) {
            if keys::merge(&stored.key, &key).is_some() {
                out.updates.push(info.clone());
            }
        }
        let reason = match info.unusable {
            Some(Unusable::Expired) => Some(SkipReason::Expired),
            Some(Unusable::Revoked) => Some(SkipReason::Revoked),
            Some(Unusable::InvalidBindings) => Some(SkipReason::InvalidBindings),
            Some(Unusable::NoEncryptionKey) => Some(SkipReason::NoEncryptionKey),
            Some(Unusable::SecretOnCard) | None => None,
        }
        .or_else(|| info.emails.is_empty().then_some(SkipReason::NoEmail));
        if let Some(reason) = reason {
            out.skipped.push(SkippedKey { info, reason });
            continue;
        }
        let mut present = true;
        for addr in &info.emails {
            let existing = ring.peers_for(addr);
            if !existing.iter().any(|p| p.fingerprint == info.fingerprint) {
                present = false;
                if existing.iter().any(|p| p.verified) {
                    out.conflicts_with_verified.push(addr.clone());
                } else if !existing.is_empty() {
                    out.replaces_unverified.push(addr.clone());
                }
            }
        }
        if present {
            out.already_present.push(info);
        } else {
            out.ready.push(info);
        }
    }
    Ok(out)
}

/// Imports the keys in `data` whose fingerprints are in `fingerprints` (from
/// the preview). An explicit import counts as the newest key for its
/// addresses, so it is preferred over an unverified Autocrypt key; a verified
/// key stays preferred regardless ([`Keyring::encryption_key_for`]).
pub fn import_peers(conn: &Connection, data: &[u8], fingerprints: &[String]) -> Result<usize> {
    let (all, _) = peer_keys_in(data)?;
    let now = now_secs();
    let mut count = 0;
    for key in all {
        // Updates to stored keys (revocations) apply whether or not picked.
        merge_into_stored(conn, &key)?;
        let info = keys::info(&key, false, keys::now());
        if !fingerprints.contains(&info.fingerprint) || info.unusable.is_some() {
            continue;
        }
        let blob = keys::binary_public(&keys::minimize(&key))?;
        for addr in &info.emails {
            conn.execute(
                "INSERT INTO openpgp_peer_keys (addr, fingerprint, public_key, source, verified, first_seen, last_seen)
                 VALUES (?1, ?2, ?3, 'imported', 0, ?4, ?4)
                 ON CONFLICT(addr, fingerprint) DO UPDATE SET last_seen = excluded.last_seen",
                params![addr, info.fingerprint, blob, now],
            )?;
        }
        recheck_signed_by(conn, &key);
        count += 1;
    }
    reload(conn)?;
    Ok(count)
}

/// Adds a key the user picked from a lookup ("Use this key"). Checked again
/// here: it must carry `addr` and be usable. It counts as the newest key for
/// the address (Known, not Verified).
pub fn add_looked_up(
    conn: &Connection,
    addr: &str,
    data: &[u8],
    from_wkd: bool,
) -> Result<KeyInfo> {
    let addr = addr.trim().to_ascii_lowercase();
    let key = keys::parse_public(data)?;
    let info = keys::info(&key, false, keys::now());
    if !info.emails.contains(&addr) {
        bail!("this key isn't for {addr}");
    }
    if info.unusable.is_some() {
        bail!("this key can't be used");
    }
    merge_into_stored(conn, &key)?;
    let source = if from_wkd {
        Source::Wkd
    } else {
        Source::Keyserver
    };
    let now = now_secs();
    conn.execute(
        "INSERT INTO openpgp_peer_keys (addr, fingerprint, public_key, source, verified, first_seen, last_seen)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)
         ON CONFLICT(addr, fingerprint) DO UPDATE SET last_seen = excluded.last_seen",
        params![addr, info.fingerprint, keys::binary_public(&keys::minimize(&key))?, source.as_str(), now],
    )?;
    recheck_signed_by(conn, &key);
    reload(conn)?;
    Ok(info)
}

/// Forgets cached bodies that couldn't be decrypted, so the next read
/// fetches and decrypts them with the keys just added. They stay known as
/// encrypted meanwhile (inline PGP has no MIME type that says so), so a reply
/// sent before that is still encrypted. Returns how many.
pub fn forget_undecryptable(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        &format!(
            "UPDATE messages SET body = NULL, json = {DROP_CACHED_BODY_JSON}
             WHERE json_extract(json, '$.crypto.problem') = 'no_key'"
        ),
        [],
    )?)
}

/// Cached messages signed by `key` while it wasn't stored: their signature was
/// checked without it (`signer` unknown, `issuer` one of its parts) and stays
/// unverified however often they are opened. The signed bytes aren't kept, so
/// the cached body is dropped, as for a reparse: the next read (or the
/// prefetch) fetches the message again and verifies it with the key. Best
/// effort: the key is stored either way, so a failure here is only logged.
fn recheck_signed_by(conn: &Connection, key: &pgp::composed::SignedPublicKey) {
    if let Err(err) = drop_unverified_signed_by(conn, key) {
        crate::mlog!(
            crate::log::Level::Warn,
            "openpgp",
            "couldn't mark messages for a new signature check: {err:#}"
        );
    }
}

/// [`recheck_signed_by`]: returns how many cached bodies were dropped.
fn drop_unverified_signed_by(
    conn: &Connection,
    key: &pgp::composed::SignedPublicKey,
) -> Result<usize> {
    let parts: Vec<String> = std::iter::once(fingerprint_hex(&key.primary_key))
        .chain(
            key.public_subkeys
                .iter()
                .map(|sub| fingerprint_hex(&sub.key)),
        )
        .collect();
    let mut stmt = conn.prepare(
        "SELECT id, json_extract(json, '$.crypto.issuer') FROM messages
         WHERE body IS NOT NULL
           AND json_extract(json, '$.crypto.signed') = 1
           AND json_extract(json, '$.crypto.signer') IS NULL
           AND json_extract(json, '$.crypto.issuer') IS NOT NULL",
    )?;
    // The issuer is a fingerprint, or only a 16-hex key ID (its tail).
    let ids: Vec<i64> = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|row| row.ok())
        .filter(|(_, issuer)| {
            let issuer = issuer.to_ascii_uppercase();
            !issuer.is_empty() && parts.iter().any(|part| part.ends_with(&issuer))
        })
        .map(|(id, _)| id)
        .collect();
    for id in &ids {
        conn.execute(
            &format!(
                "UPDATE messages SET body = NULL, json = {DROP_CACHED_BODY_JSON} WHERE id = ?1"
            ),
            [id],
        )?;
    }
    Ok(ids.len())
}

/// Marks a key verified (or not). `confirm_fingerprint` must equal the key's
/// fingerprint: the UI passes what the user compared, so a stale dialog can't
/// verify a different key.
pub fn set_verified(
    conn: &Connection,
    addr: &str,
    fingerprint: &str,
    confirm_fingerprint: &str,
    verified: bool,
) -> Result<()> {
    if verified && normalize_fingerprint(confirm_fingerprint) != fingerprint {
        bail!("the fingerprint doesn't match this key");
    }
    let n = conn.execute(
        "UPDATE openpgp_peer_keys SET verified = ?3 WHERE addr = ?1 AND fingerprint = ?2",
        params![addr.to_ascii_lowercase(), fingerprint, verified],
    )?;
    if n == 0 {
        bail!("no such key");
    }
    reload(conn)
}

pub fn remove_peer(conn: &Connection, addr: &str, fingerprint: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM openpgp_peer_keys WHERE addr = ?1 AND fingerprint = ?2",
        params![addr.to_ascii_lowercase(), fingerprint],
    )?;
    reload(conn)
}

/// Upper-case hex without spaces, for comparing a pasted fingerprint.
pub fn normalize_fingerprint(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_hexdigit)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// The lock for each encrypted message among `uids` in one folder, with the
/// trust its signature earns under `ring`: one query for a whole list page.
/// Plain mail, and uids the cache lacks, have no entry. Trust is judged
/// against the stored sender, not the card's: a Sent card names the recipient.
pub fn card_locks(
    conn: &Connection,
    ring: &Keyring,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<HashMap<u32, keyring::Trust>> {
    let mut locks = HashMap::new();
    if uids.is_empty() {
        return Ok(locks);
    }
    let mut stmt = conn.prepare_cached(
        "SELECT uid, from_addr, json_extract(json, '$.crypto') FROM messages
         WHERE account = ?1 AND folder = ?2 AND uid IN (SELECT value FROM json_each(?3))
           AND json_extract(json, '$.crypto.encrypted') = 1",
    )?;
    let rows = stmt.query_map(
        params![account, folder, serde_json::to_string(uids)?],
        |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )?;
    for row in rows {
        let (uid, from, json) = row?;
        if let Ok(status) = serde_json::from_str::<super::mime::CryptoStatus>(&json) {
            locks.insert(uid, status.trust(ring, &from));
        }
    }
    Ok(locks)
}

// Offer ---------------------------------------------------------------------

/// "Not now" (one sender) or "Don't ask again" (`addr` = `*`).
pub fn dismiss_offer(conn: &Connection, account_id: &str, addr: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO openpgp_offer_dismissed (account_id, addr) VALUES (?1, ?2)",
        params![account_id, addr.to_ascii_lowercase()],
    )?;
    reload(conn)
}

pub fn offer_dismissed(conn: &Connection, account_id: &str, addr: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM openpgp_offer_dismissed WHERE account_id = ?1 AND (addr = ?2 OR addr = '*'))",
        params![account_id, addr.to_ascii_lowercase()],
        |r| r.get(0),
    )
    .unwrap_or(false)
}

// Backup ----------------------------------------------------------------------

/// One account's keys in a backup.
#[derive(Debug, Serialize, serde::Deserialize, Clone)]
pub struct BackupKeys {
    pub account_id: String,
    pub enabled: bool,
    /// "Sign unencrypted mail" (absent in backups made before it existed).
    #[serde(default)]
    pub sign_plain: bool,
    /// Armored public keys of the account's own keys (always included).
    pub public: Vec<String>,
    /// Base64 of the stored secret keys; only in backups that include secrets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret: Vec<String>,
}

pub fn backup_collect(conn: &Connection, include_secrets: bool) -> Result<Vec<BackupKeys>> {
    use base64::Engine as _;
    let mut out: Vec<BackupKeys> = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT k.account_id, k.secret_key, COALESCE(a.enabled, 0) FROM openpgp_own_keys k
         LEFT JOIN openpgp_accounts a ON a.account_id = k.account_id ORDER BY k.account_id, k.is_default, k.added_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, bool>(2)?,
        ))
    })?;
    for row in rows {
        let (account_id, blob, enabled) = row?;
        let blob = zeroize::Zeroizing::new(blob);
        let Ok(secret) = keys::parse_secret(&blob) else {
            continue;
        };
        let entry = match out.iter_mut().find(|e| e.account_id == account_id) {
            Some(e) => e,
            None => {
                out.push(BackupKeys {
                    sign_plain: sign_plain(conn, &account_id)?,
                    account_id: account_id.clone(),
                    enabled,
                    public: Vec::new(),
                    secret: Vec::new(),
                });
                out.last_mut().expect("just pushed")
            }
        };
        entry
            .public
            .push(keys::armored_public(&secret.to_public_key())?);
        if include_secrets {
            entry
                .secret
                .push(base64::engine::general_purpose::STANDARD.encode(blob.as_slice()));
        }
    }
    Ok(out)
}

/// Restores keys from a backup. Keys already present are kept; a restored
/// key becomes the default only when the account has none (edge case: restore
/// onto a profile that already made its own key). Returns accounts whose
/// secret keys were not in the backup.
pub fn backup_apply(conn: &Connection, entries: &[BackupKeys]) -> Result<Vec<String>> {
    backup_apply_with(conn, entries, true)
}

fn backup_apply_with(
    conn: &Connection,
    entries: &[BackupKeys],
    restore_secrets: bool,
) -> Result<Vec<String>> {
    use base64::Engine as _;
    let mut missing = Vec::new();
    let mut restored: Vec<pgp::composed::SignedPublicKey> = Vec::new();
    for entry in entries {
        // No secret in the backup, or this build doesn't restore secrets:
        // the account has no usable key and is reported as missing.
        if entry.secret.is_empty() || !restore_secrets {
            missing.push(entry.account_id.clone());
            continue;
        }
        let has_default: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM openpgp_own_keys WHERE account_id = ?1 AND is_default = 1)",
            [&entry.account_id],
            |r| r.get(0),
        )?;
        // Backup order is oldest first with the default last.
        for (i, b64) in entry.secret.iter().enumerate() {
            let blob = zeroize::Zeroizing::new(
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .context("backup key")?,
            );
            let secret = keys::parse_secret(&blob)?;
            keys::self_test(&secret)?;
            restored.push(secret.to_public_key());
            let make_default = !has_default && i + 1 == entry.secret.len();
            if make_default {
                conn.execute(
                    "UPDATE openpgp_own_keys SET is_default = 0 WHERE account_id = ?1",
                    [&entry.account_id],
                )?;
            }
            conn.execute(
                "INSERT INTO openpgp_own_keys (fingerprint, account_id, secret_key, is_default, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(fingerprint) DO NOTHING",
                params![fingerprint_hex(&secret.primary_key), entry.account_id, blob.as_slice(), make_default, now_secs()],
            )?;
        }
        if entry.enabled && !has_default {
            set_enabled(conn, &entry.account_id, true)?;
        }
        if entry.sign_plain && !has_default {
            conn.execute(
                "INSERT OR IGNORE INTO openpgp_sign_plain (account_id) VALUES (?1)",
                [&entry.account_id],
            )?;
        }
    }
    // Mail already cached that these keys open or signed is read again.
    if restored.is_empty() {
        reload(conn)?;
        return Ok(missing);
    }
    forget_undecryptable(conn)?;
    for key in &restored {
        recheck_signed_by(conn, key);
    }
    reload(conn)?;
    Ok(missing)
}

#[cfg(test)]
mod tests;
