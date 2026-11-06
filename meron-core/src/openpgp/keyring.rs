//! The keys the parser and the sender use, held in memory.
//!
//! Message parsing runs deep inside fetch paths that have no database handle,
//! so the store loads every key once into a shared snapshot here and swaps a
//! new snapshot in after each change. Tests build their own `Keyring` and pass
//! it explicitly instead of touching the global one.

use std::sync::{Arc, RwLock};

use pgp::composed::{SignedPublicKey, SignedSecretKey};
use serde::Serialize;

use super::keys::{self, fingerprint_hex};

/// One of the user's own keys.
#[derive(Debug, Clone)]
pub struct OwnKey {
    pub account_id: String,
    pub fingerprint: String,
    pub secret: SignedSecretKey,
    pub public: SignedPublicKey,
    /// Used for signing and Autocrypt; other own keys only decrypt.
    pub is_default: bool,
}

/// Where we learned a correspondent's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Autocrypt,
    Gossip,
    Attachment,
    Imported,
    /// Looked up on the recipient's domain (WKD).
    Wkd,
    /// Looked up on a keyserver.
    Keyserver,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Autocrypt => "autocrypt",
            Source::Gossip => "gossip",
            Source::Attachment => "attachment",
            Source::Imported => "imported",
            Source::Wkd => "wkd",
            Source::Keyserver => "keyserver",
        }
    }

    pub fn parse(s: &str) -> Source {
        match s {
            "gossip" => Source::Gossip,
            "attachment" => Source::Attachment,
            "imported" => Source::Imported,
            "wkd" => Source::Wkd,
            "keyserver" => Source::Keyserver,
            _ => Source::Autocrypt,
        }
    }
}

/// A correspondent's key, for one address (a key with several user IDs has
/// one entry per address).
#[derive(Debug, Clone)]
pub struct PeerKey {
    pub addr: String,
    pub fingerprint: String,
    pub key: SignedPublicKey,
    pub source: Source,
    pub verified: bool,
    /// When this key was last announced or imported (unix seconds): the most
    /// recent valid key is the one encrypted to.
    pub last_seen: i64,
}

/// How far a signer's key is trusted, for the message indicator colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    /// Verified by the user, or one of the user's own keys.
    Verified,
    /// Known through Autocrypt, an attachment or an import (trust on first use).
    Known,
    /// Not signed, signed by a key we don't have, or the signature failed.
    Unconfirmed,
}

#[derive(Debug, Clone, Default)]
pub struct Keyring {
    pub own: Vec<OwnKey>,
    pub peers: Vec<PeerKey>,
    /// Accounts with end-to-end encryption switched on.
    pub enabled_accounts: Vec<String>,
    /// Dismissed "turn it on" offers: (account, sender or `*`).
    pub dismissed: Vec<(String, String)>,
}

impl Keyring {
    pub fn secret_keys(&self) -> Vec<&SignedSecretKey> {
        self.own.iter().map(|k| &k.secret).collect()
    }

    pub fn default_key(&self, account_id: &str) -> Option<&OwnKey> {
        self.own.iter().find(|k| k.account_id == account_id && k.is_default)
    }

    pub fn is_enabled(&self, account_id: &str) -> bool {
        self.enabled_accounts.iter().any(|a| a == account_id)
    }

    pub fn offer_dismissed(&self, account_id: &str, addr: &str) -> bool {
        let addr = addr.trim().to_ascii_lowercase();
        self.dismissed.iter().any(|(a, d)| a == account_id && (d == "*" || *d == addr))
    }

    pub fn is_own(&self, fingerprint: &str) -> bool {
        self.own.iter().any(|k| k.fingerprint == fingerprint)
    }

    /// Every peer key stored for `addr`, newest first.
    pub fn peers_for(&self, addr: &str) -> Vec<&PeerKey> {
        let addr = addr.trim().to_ascii_lowercase();
        let mut out: Vec<&PeerKey> = self.peers.iter().filter(|p| p.addr == addr).collect();
        out.sort_by_key(|p| std::cmp::Reverse(p.last_seen));
        out
    }

    /// Keys to verify a message from `addr` with: theirs first, then ours (our
    /// sent mail, our other devices), then every other known key, so a
    /// signature by someone else's key is recognised as such ("signed by
    /// someone other than the sender") instead of looking merely unknown.
    /// Signature checks filter by issuer first, so the long tail is cheap.
    pub fn verifiers_for(&self, addr: &str) -> Vec<&SignedPublicKey> {
        let mut out: Vec<&SignedPublicKey> = self.peers_for(addr).into_iter().map(|p| &p.key).collect();
        out.extend(self.own.iter().map(|k| &k.public));
        let addr = addr.trim().to_ascii_lowercase();
        let mut seen: Vec<&str> = Vec::new();
        for p in self.peers.iter().filter(|p| p.addr != addr) {
            if !seen.contains(&p.fingerprint.as_str()) {
                seen.push(&p.fingerprint);
                out.push(&p.key);
            }
        }
        out
    }

    /// A stored or own public key by primary fingerprint (designated revokers).
    pub fn key_by_fingerprint(&self, fingerprint: &str) -> Option<SignedPublicKey> {
        self.peers
            .iter()
            .find(|p| p.fingerprint == fingerprint)
            .map(|p| p.key.clone())
            .or_else(|| self.own.iter().find(|k| k.fingerprint == fingerprint).map(|k| k.public.clone()))
    }

    /// Whether `addr` is an address of one of our own keys. Only then does a
    /// signature by our own key prove the sender (our sent mail, our other
    /// accounts); a stranger replaying our signed ciphertext from their own
    /// address must not be trusted as that sender.
    pub fn is_own_address(&self, addr: &str) -> bool {
        let addr = addr.trim().to_ascii_lowercase();
        self.own.iter().any(|k| keys::info(&k.public, true, keys::now()).emails.contains(&addr))
    }

    /// The trust a signature by `fingerprint` earns for a message from `addr`.
    pub fn trust_of(&self, fingerprint: &str, addr: &str) -> Trust {
        if self.is_own(fingerprint) {
            // Our own key vouches for the sender only on our own mail.
            return if self.is_own_address(addr) { Trust::Verified } else { Trust::Unconfirmed };
        }
        match self.peers_for(addr).into_iter().find(|p| p.fingerprint == fingerprint) {
            Some(p) if p.verified => Trust::Verified,
            Some(_) => Trust::Known,
            None => Trust::Unconfirmed,
        }
    }

    /// The key to encrypt to for `addr` now, or why there is none.
    pub fn encryption_key_for(&self, addr: &str) -> Result<&PeerKey, NoKeyReason> {
        let peers = self.peers_for(addr);
        if peers.is_empty() {
            // Mail to ourselves (another of our accounts) uses our own key.
            return Err(NoKeyReason::None);
        }
        // A verified key wins; a newer unverified key never replaces it.
        if let Some(verified) = peers.iter().find(|p| p.verified) {
            if keys::check(&verified.key, keys::now()).is_none() {
                return Ok(verified);
            }
        }
        if peers.iter().any(|p| p.verified) {
            return Err(NoKeyReason::NewKeyUnverified);
        }
        peers
            .into_iter()
            .find(|p| keys::check(&p.key, keys::now()).is_none())
            .ok_or(NoKeyReason::Unusable)
    }

    /// Own public key for `addr` when it belongs to one of our accounts.
    pub fn own_key_for_addr(&self, addr: &str) -> Option<&OwnKey> {
        let addr = addr.trim().to_ascii_lowercase();
        self.own
            .iter()
            .filter(|k| k.is_default)
            .find(|k| keys::info(&k.public, true, keys::now()).emails.contains(&addr))
    }
}

/// Why a recipient can't be encrypted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoKeyReason {
    None,
    /// Every stored key is expired, revoked or lacks an encryption subkey.
    Unusable,
    /// The verified key is unusable and only a newer, unverified one exists.
    NewKeyUnverified,
}

static CURRENT: RwLock<Option<Arc<Keyring>>> = RwLock::new(None);

/// The keys in effect now (empty until the store has loaded them).
pub fn current() -> Arc<Keyring> {
    CURRENT.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Swaps in a new snapshot.
pub fn replace(ring: Keyring) {
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(Arc::new(ring));
    }
}

/// Builds an `OwnKey` from a stored secret key.
pub fn own_key(account_id: &str, secret: SignedSecretKey, is_default: bool) -> OwnKey {
    let public = secret.to_public_key();
    OwnKey {
        account_id: account_id.to_string(),
        fingerprint: fingerprint_hex(&secret.primary_key),
        secret,
        public,
        is_default,
    }
}
