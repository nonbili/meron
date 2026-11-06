//! PGP/MIME on the receiving side: find the OpenPGP layer of a message,
//! open it, and say which MIME tree may be shown.
//!
//! EFAIL rule (security notes §4): only a message whose *top-level* body is
//! the encrypted (or signed) part is opened. An encrypted part nested among
//! other parts is never decrypted, so its plaintext can never be rendered next
//! to attacker-controlled HTML; the message is shown as it is, marked
//! "partly encrypted".

use mailparse::{MailHeaderMap as _, ParsedMail};
use pgp::composed::SignedPublicKey;
use serde::{Deserialize, Serialize};

use super::autocrypt::{self, Announced};
use super::crypt::{self, DecryptError, SigCheck};
use super::keyring::Keyring;
use super::keys;

/// What went wrong, for the problem strip. `None` on a good message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Problem {
    /// Encrypted to a key Meron doesn't have.
    NoKey,
    /// Modified after it was sent, or corrupt: nothing is shown.
    Integrity,
    TooLarge,
    Unreadable,
    /// A signature by a key we have didn't verify.
    InvalidSignature,
    /// Validly signed, but by a key that isn't the From address's.
    SignerMismatch,
    /// An encrypted part sits among unencrypted ones; it was not opened.
    PartlyEncrypted,
    /// Inline encryption inside an HTML body, which Meron doesn't open.
    InlineInHtml,
    /// Signed by a key that was revoked when the signature was made.
    KeyRevoked,
    /// Signed by a key that had expired when the signature was made.
    KeyExpired,
    /// The encrypted message appears redirected: the signer named intended
    /// recipients that don't include us, or its protected inner `From`
    /// differs from the envelope `From`. Either way it reads as a copy of
    /// a message meant for, or from, someone else.
    Redirected,
}

/// The message's OpenPGP facts, persisted with the cached body (every field
/// is written, nulls included: the store merges with `json_patch`, where a
/// missing field would keep a stale value such as an old `no_key` problem).
/// Trust (the
/// indicator colour) is *not* stored: it depends on later choices (verifying
/// a key) and is computed when the message is read ([`CryptoStatus::trust`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoStatus {
    #[serde(default)]
    pub encrypted: bool,
    /// A signature covers the shown content (valid or not).
    #[serde(default)]
    pub signed: bool,
    /// Primary fingerprint of the key whose signature verified.
    #[serde(default)]
    pub signer: Option<String>,
    /// Issuer named by the signature when we couldn't verify it.
    #[serde(default)]
    pub issuer: Option<String>,
    /// Only part of the message is covered by the signature.
    #[serde(default)]
    pub partial: bool,
    /// Made with SHA-1 or MD5.
    #[serde(default)]
    pub weak_hash: bool,
    #[serde(default)]
    pub problem: Option<Problem>,
    /// Signed with a key we had never seen for this address before.
    #[serde(default)]
    pub key_changed: bool,
    /// Sent through a mailing list (List-Id): broken signatures are expected there.
    #[serde(default)]
    pub on_list: bool,
    /// The sender announced a key (Autocrypt): drives the "turn it on" offer.
    #[serde(default)]
    pub autocrypt: bool,
    /// Our key that opened it.
    #[serde(default)]
    pub decrypted_with: Option<String>,
}

impl CryptoStatus {
    /// Whether there is anything to show at all.
    pub fn is_empty(&self) -> bool {
        !self.encrypted && !self.signed && self.problem.is_none() && !self.autocrypt
    }

    /// The indicator colour now, given the current keys.
    pub fn trust(&self, ring: &Keyring, from_addr: &str) -> super::keyring::Trust {
        use super::keyring::Trust;
        if self.problem.is_some() || self.partial || self.weak_hash {
            return Trust::Unconfirmed;
        }
        match &self.signer {
            Some(fpr) => ring.trust_of(fpr, from_addr),
            None => Trust::Unconfirmed,
        }
    }
}

/// The status as the frontend gets it: stored facts plus what depends on
/// the current keys (trust colour, a verified key being bypassed, the offer).
pub fn view_json(
    status: &CryptoStatus,
    ring: &Keyring,
    account_id: &str,
    from_addr: &str,
    outgoing: bool,
) -> serde_json::Value {
    let trust = status.trust(ring, from_addr);
    let from = from_addr.trim().to_ascii_lowercase();
    // A new key while a verified one exists: the rose key-change variant.
    let verified_conflict = status.key_changed && ring.peers_for(&from).iter().any(|p| p.verified);
    let offer = status.autocrypt
        && !outgoing
        && !status.on_list
        && !ring.is_enabled(account_id)
        && !ring.offer_dismissed(account_id, &from);
    let signer_key = status.signer.as_ref().and_then(|fpr| {
        ring.peers_for(&from)
            .into_iter()
            .find(|p| &p.fingerprint == fpr)
            .map(|p| p.fingerprint.clone())
    });
    serde_json::json!({
        "encrypted": status.encrypted,
        "signed": status.signed,
        "trust": trust,
        "signer": status.signer,
        "issuer": status.issuer,
        "partial": status.partial,
        "weak_hash": status.weak_hash,
        "problem": status.problem,
        "key_changed": status.key_changed,
        "verified_conflict": verified_conflict,
        "on_list": status.on_list,
        "own": status.signer.as_deref().is_some_and(|f| ring.is_own(f)) && ring.is_own_address(&from),
        // Whether the signer's key is stored for this address (Verify… needs it).
        "signer_stored": signer_key.is_some(),
        "offer": offer,
    })
}

/// Which MIME tree to render after opening.
#[derive(Debug)]
pub enum Content {
    /// The message as received (not OpenPGP, or partly so).
    Outer,
    /// The first subpart of the top-level `multipart/signed`.
    SignedPart,
    /// Decrypted inner MIME bytes; when `signed_inside`, render its first
    /// subpart (it is a `multipart/signed` itself).
    Inner { bytes: Vec<u8>, signed_inside: bool },
    /// Nothing: decryption failed, fail closed.
    Nothing,
}

#[derive(Debug)]
pub struct Opened {
    pub content: Content,
    pub status: Option<CryptoStatus>,
    /// Keys to record (Autocrypt, gossip).
    pub announced: Vec<Announced>,
    /// Keys attached to the message for the sender's address.
    pub attached: Vec<(String, SignedPublicKey)>,
    /// Subject from the protected (inner) headers, RFC 9788.
    pub inner_subject: Option<String>,
}

fn mime(part: &ParsedMail<'_>) -> String {
    part.ctype.mimetype.to_ascii_lowercase()
}

/// RFC 3156 `multipart/encrypted`, plus the two broken shapes Exchange and
/// Google Workspace produce (Delta Chat repairs the same ones). Returns the
/// part holding the OpenPGP data.
fn encrypted_data<'a, 'b>(mail: &'a ParsedMail<'b>) -> Option<&'a ParsedMail<'b>> {
    let rfc3156 = |m: &'a ParsedMail<'b>| -> Option<&'a ParsedMail<'b>> {
        if mime(m) != "multipart/encrypted" {
            return None;
        }
        match &m.subparts[..] {
            [version, data]
                if mime(version) == "application/pgp-encrypted"
                    && mime(data) == "application/octet-stream" =>
            {
                Some(data)
            }
            _ => None,
        }
    };
    if let Some(data) = rfc3156(mail) {
        return Some(data);
    }
    if mime(mail) != "multipart/mixed" {
        return None;
    }
    match &mail.subparts[..] {
        // Exchange: multipart/mixed [empty text, pgp-encrypted, octet-stream].
        [text, version, data]
            if mime(text) == "text/plain"
                && text.get_body().is_ok_and(|b| b.trim().is_empty())
                && mime(version) == "application/pgp-encrypted"
                && mime(data) == "application/octet-stream" =>
        {
            Some(data)
        }
        // Google Workspace: multipart/mixed [text, multipart/encrypted].
        [text, enc]
            if mime(text) == "text/plain" && text.get_body().is_ok_and(|b| b.trim().is_empty()) =>
        {
            rfc3156(enc)
        }
        _ => None,
    }
}

fn contains_encrypted(part: &ParsedMail<'_>) -> bool {
    mime(part) == "multipart/encrypted" || part.subparts.iter().any(contains_encrypted)
}

fn is_armored_message(text: &str) -> bool {
    text.trim_start().starts_with("-----BEGIN PGP MESSAGE-----")
}

/// Keys attached as `application/pgp-keys` whose user IDs include `from`
/// (a mail may only bring its sender's key, never someone else's).
fn attached_keys(part: &ParsedMail<'_>, from: &str, out: &mut Vec<(String, SignedPublicKey)>) {
    if !part.subparts.is_empty() {
        for sub in &part.subparts {
            attached_keys(sub, from, out);
        }
        return;
    }
    if mime(part) != "application/pgp-keys" {
        return;
    }
    let Ok(body) = part.get_body_raw() else {
        return;
    };
    let Ok(parsed) = keys::parse_keys(&body) else {
        return;
    };
    for key in parsed.public {
        if keys::info(&key, false, keys::now())
            .emails
            .iter()
            .any(|e| e == from)
        {
            out.push((from.to_string(), key));
        }
    }
}

/// Filenames of attached key parts (`application/pgp-keys`) that hold only
/// the sender's own keys. Those are recorded automatically (Thunderbird
/// attaches the sender's key to every signed message), so they are hidden
/// from the attachment list. A key file with anyone else's key stays: there
/// the key may be what the mail is about.
pub fn sender_key_files(part: &ParsedMail<'_>, from_addr: &str) -> Vec<String> {
    let from = from_addr.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    collect_sender_key_files(part, &from, &mut out);
    out
}

fn collect_sender_key_files(part: &ParsedMail<'_>, from: &str, out: &mut Vec<String>) {
    if !part.subparts.is_empty() {
        for sub in &part.subparts {
            collect_sender_key_files(sub, from, out);
        }
        return;
    }
    if mime(part) != "application/pgp-keys" {
        return;
    }
    let Ok(body) = part.get_body_raw() else {
        return;
    };
    let Ok(parsed) = keys::parse_keys(&body) else {
        return;
    };
    let all_senders = !parsed.public.is_empty()
        && parsed.secret.is_empty()
        && parsed.public.iter().all(|k| {
            keys::info(k, false, keys::now())
                .emails
                .iter()
                .any(|e| e == from)
        });
    if !all_senders {
        return;
    }
    let disposition = part.get_content_disposition();
    if let Some(name) = disposition
        .params
        .get("filename")
        .or_else(|| part.ctype.params.get("name"))
    {
        out.push(name.clone());
    }
}

/// Opens the OpenPGP layer of `mail` (if any) with the keys in `ring`.
pub fn open(mail: &ParsedMail<'_>, from_addr: &str, ring: &Keyring) -> Opened {
    let from = from_addr.trim().to_ascii_lowercase();
    let on_list = mail.headers.get_first_value("List-Id").is_some();
    let mut announced = autocrypt::announced(&mail.headers, &from);
    // Gossip is only legitimate inside the encrypted, signed part of a message
    // (Autocrypt Level 1). A cleartext Autocrypt-Gossip header on an ordinary
    // mail must never plant a key for the gossiped address.
    announced.retain(|a| !a.gossip);
    let mut attached = Vec::new();
    let mut status = CryptoStatus {
        on_list,
        ..Default::default()
    };

    // Encrypted (PGP/MIME, or inline as the only text part).
    let inline_text = (mime(mail) == "text/plain" && mail.subparts.is_empty())
        .then(|| mail.get_body().ok())
        .flatten()
        .filter(|t| is_armored_message(t));
    let data = match (encrypted_data(mail), &inline_text) {
        (Some(part), _) => part.get_body_raw().ok(),
        (None, Some(text)) => Some(text.as_bytes().to_vec()),
        (None, None) => None,
    };
    if let Some(data) = data {
        status.encrypted = true;
        let opened = open_encrypted(
            &data,
            inline_text.is_some(),
            &from,
            ring,
            &mut announced,
            &mut attached,
            &mut status,
        );
        status.autocrypt = announced.iter().any(|a| !a.gossip && a.addr == from);
        return Opened {
            status: Some(status),
            announced,
            attached,
            ..opened
        };
    }

    // An encrypted part somewhere deeper: never decrypted (EFAIL).
    if contains_encrypted(mail) {
        status.problem = Some(Problem::PartlyEncrypted);
        status.autocrypt = announced.iter().any(|a| !a.gossip && a.addr == from);
        return Opened {
            content: Content::Outer,
            status: Some(status),
            announced,
            attached,
            inner_subject: None,
        };
    }

    attached_keys(mail, &from, &mut attached);
    let extra: Vec<SignedPublicKey> = announced
        .iter()
        .filter(|a| !a.gossip)
        .map(|a| a.key.clone())
        .chain(attached.iter().map(|(_, k)| k.clone()))
        .collect();

    // Signed: the whole body, or (mailing lists) the first part of a mixed one.
    let mut content = Content::Outer;
    if let Some(checks) = verify_signed(mail, &from, ring, &extra) {
        status.signed = true;
        content = Content::SignedPart;
        apply_signature(&mut status, &checks, &from, ring, &extra);
    } else if mime(mail) == "multipart/mixed" {
        if let Some(first) = mail.subparts.first() {
            if let Some(checks) = verify_signed(first, &from, ring, &extra) {
                status.signed = true;
                status.partial = true;
                apply_signature(&mut status, &checks, &from, ring, &extra);
            }
        }
    } else if inline_in_html(mail) {
        status.problem = Some(Problem::InlineInHtml);
    }
    status.autocrypt = announced.iter().any(|a| !a.gossip && a.addr == from);
    let status = (!status.is_empty()).then_some(status);
    Opened {
        content,
        status,
        announced,
        attached,
        inner_subject: None,
    }
}

fn inline_in_html(mail: &ParsedMail<'_>) -> bool {
    mail.subparts.is_empty()
        && mime(mail) == "text/html"
        && mail
            .get_body()
            .is_ok_and(|b| b.contains("-----BEGIN PGP MESSAGE-----"))
}

fn verify_signed(
    part: &ParsedMail<'_>,
    from: &str,
    ring: &Keyring,
    extra: &[SignedPublicKey],
) -> Option<Vec<SigCheck>> {
    if mime(part) != "multipart/signed" {
        return None;
    }
    let [content, signature] = &part.subparts[..] else {
        return None;
    };
    if mime(signature) != "application/pgp-signature" {
        return None;
    }
    let sig = signature.get_body_raw().ok()?;
    let mut verifiers: Vec<&SignedPublicKey> = ring.verifiers_for(from);
    verifiers.extend(extra.iter());
    Some(crypt::verify_detached_with(
        content.raw_bytes,
        &sig,
        &verifiers,
        &|f| ring.key_by_fingerprint(f),
    ))
}

#[allow(clippy::too_many_arguments)]
fn open_encrypted(
    data: &[u8],
    inline: bool,
    from: &str,
    ring: &Keyring,
    announced: &mut Vec<Announced>,
    attached: &mut Vec<(String, SignedPublicKey)>,
    status: &mut CryptoStatus,
) -> Opened {
    let secrets = ring.secret_keys();
    // Verify against stored keys plus the key announced in this message's
    // outer headers (Delta Chat signs with the key it announces).
    let outer_keys: Vec<SignedPublicKey> = announced
        .iter()
        .filter(|a| !a.gossip)
        .map(|a| a.key.clone())
        .collect();
    let mut verifiers: Vec<&SignedPublicKey> = ring.verifiers_for(from);
    verifiers.extend(outer_keys.iter());
    let decrypted =
        match crypt::decrypt_with(data, &secrets, &verifiers, &|f| ring.key_by_fingerprint(f)) {
            Ok(d) => d,
            Err(e) => {
                status.problem = Some(match e {
                    DecryptError::NoKey => Problem::NoKey,
                    DecryptError::Integrity => Problem::Integrity,
                    DecryptError::TooLarge => Problem::TooLarge,
                    DecryptError::Unreadable => Problem::Unreadable,
                });
                return Opened {
                    content: Content::Nothing,
                    status: None,
                    announced: Vec::new(),
                    attached: Vec::new(),
                    inner_subject: None,
                };
            }
        };
    status.decrypted_with = decrypted.decrypted_with.clone();
    let mut checks = decrypted.signatures.clone();

    let bytes = if inline {
        // Inline PGP decrypts to text, not MIME.
        let mut b = b"Content-Type: text/plain; charset=utf-8\r\n\r\n".to_vec();
        b.extend_from_slice(&decrypted.plaintext);
        b
    } else {
        decrypted.plaintext
    };
    let mut inner_subject = None;
    let mut signed_inside = false;
    let mut inner_keys: Vec<SignedPublicKey> = Vec::new();
    // Gossip from the decrypted headers, held back until the signature is
    // checked: it is recorded only for a message validly signed by its From
    //, decided after signature verification below.
    let mut inner_gossip: Vec<Announced> = Vec::new();
    // The protected (inner) From, for G4: it must match the envelope From.
    let mut inner_from: Option<String> = None;
    if let Ok(inner) = mailparse::parse_mail(&bytes) {
        // Protected headers (RFC 9788): Subject and Autocrypt inside.
        if !inline {
            inner_subject = inner
                .headers
                .get_first_value("Subject")
                .filter(|s| !s.trim().is_empty());
            inner_from = inner
                .headers
                .get_first_value("From")
                .filter(|s| !s.trim().is_empty());
            let inner_announced = autocrypt::announced(&inner.headers, from);
            inner_keys.extend(
                inner_announced
                    .iter()
                    .filter(|a| !a.gossip)
                    .map(|a| a.key.clone()),
            );
            // Protected headers win over the outer ones (Delta Chat's rule).
            if inner_announced.iter().any(|a| !a.gossip) {
                announced.retain(|a| a.gossip);
            }
            let (gossip, direct): (Vec<_>, Vec<_>) =
                inner_announced.into_iter().partition(|a| a.gossip);
            inner_gossip = gossip;
            announced.extend(direct);
        }
        attached_keys(&inner, from, attached);
        inner_keys.extend(attached.iter().map(|(_, k)| k.clone()));
        // Signed with an inner key we only just learned: check again with it.
        if !inner_keys.is_empty() && !checks.iter().any(|c| c.valid) && !checks.is_empty() {
            let mut all = verifiers.clone();
            all.extend(inner_keys.iter());
            if let Ok(again) =
                crypt::decrypt_with(data, &secrets, &all, &|f| ring.key_by_fingerprint(f))
            {
                checks = again.signatures;
            }
        }
        // Sign-then-encrypt as two MIME layers (older Enigmail, K-9).
        if checks.is_empty() {
            let mut extra = outer_keys.clone();
            extra.extend(inner_keys.iter().cloned());
            if let Some(inner_checks) = verify_signed(&inner, from, ring, &extra) {
                checks = inner_checks;
                signed_inside = true;
            }
        }
        // There the protected headers sit on the signed part, one level down.
        if signed_inside && inner_subject.is_none() && !inline {
            inner_subject = inner
                .subparts
                .first()
                .and_then(|part| part.headers.get_first_value("Subject"))
                .filter(|s| !s.trim().is_empty());
        }
    }
    let mut extra = outer_keys;
    extra.extend(inner_keys);
    if !checks.is_empty() {
        status.signed = true;
        apply_signature(status, &checks, from, ring, &extra);
    }
    // G3: the signer named intended recipients, but none is one of our keys —
    // the message was encrypted to someone else and redirected to us.
    if status.problem.is_none()
        && let Some(valid) = checks.iter().find(|c| c.valid)
        && !valid.intended_recipients.is_empty()
        && !valid
            .intended_recipients
            .iter()
            .any(|fp| status.decrypted_with.as_deref() == Some(fp.as_str()) || ring.is_own(fp))
    {
        status.problem = Some(Problem::Redirected);
    }
    // G4: the protected inner From differs from the envelope From — the
    // encrypted payload was lifted from a message by a different sender. Lists
    // legitimately rewrite From, so they are left out.
    if status.problem.is_none()
        && !status.on_list
        && let Some(inner) = inner_from.as_deref()
    {
        let inner_addr = super::outgoing::bare_addr(inner);
        if !inner_addr.is_empty() && inner_addr != from {
            status.problem = Some(Problem::Redirected);
        }
    }
    // Record inner gossip only from a message validly signed by its From
    //: an unsigned or mis-signed encrypted message cannot vouch for
    // third parties' keys.
    if status.signed && status.problem.is_none() && status.signer.is_some() {
        announced.extend(inner_gossip);
    }
    Opened {
        content: Content::Inner {
            bytes,
            signed_inside,
        },
        status: None,
        announced: Vec::new(),
        attached: Vec::new(),
        inner_subject,
    }
}

/// Folds signature checks into the status: which key signed, whether it
/// matches the From address, whether it is new for that address.
fn apply_signature(
    status: &mut CryptoStatus,
    checks: &[SigCheck],
    from: &str,
    ring: &Keyring,
    extra: &[SignedPublicKey],
) {
    status.weak_hash = checks.iter().any(|c| c.weak_hash);
    // L5: verification broke the verifier (a caught rPGP panic). We could not
    // validate the signature at all, so flag it invalid rather than leave it
    // looking merely unconfirmed.
    if checks.iter().any(|c| c.errored) && !checks.iter().any(|c| c.valid) {
        status.problem = Some(Problem::InvalidSignature);
        return;
    }
    // G1: the signature verified, but its key was revoked or expired then.
    if !checks.iter().any(|c| c.valid) {
        if let Some(c) = checks.iter().find(|c| c.key_problem.is_some()) {
            status.issuer = c.signer.clone().or_else(|| c.issuer.clone());
            status.problem = Some(match c.key_problem {
                Some(keys::SigningProblem::Revoked) => Problem::KeyRevoked,
                _ => Problem::KeyExpired,
            });
            return;
        }
    }
    let Some(valid) = checks.iter().find(|c| c.valid) else {
        status.issuer = checks.iter().find_map(|c| c.issuer.clone());
        // We hold the key the signature names, yet it didn't verify: altered.
        let have_issuer_key = checks.iter().any(|c| {
            c.signer.is_some()
                || c.issuer
                    .as_deref()
                    .is_some_and(|i| key_known(ring, extra, from, i))
        });
        if have_issuer_key && !status.weak_hash {
            // Lists change messages: expected, so not alarming there.
            if !status.on_list {
                status.problem = Some(Problem::InvalidSignature);
            } else {
                status.partial = true;
            }
        }
        return;
    };
    let fpr = valid.signer.clone().unwrap_or_default();
    status.signer = Some(fpr.clone());
    // Does the signing key belong to the From address?
    let key = ring
        .verifiers_for(from)
        .into_iter()
        .chain(extra.iter())
        .find(|k| keys::fingerprint_hex(&k.primary_key) == fpr);
    // The signing key must belong to the From address: a peer key whose user
    // ID carries it, or our own key only when From is one of our own addresses.
    // Our key vouching for a stranger's From is the replay oracle,
    // so there is no blanket own-key exemption.
    let matches_from = key.is_some_and(|k| {
        keys::info(k, false, keys::now())
            .emails
            .iter()
            .any(|e| e == from)
    }) || (ring.is_own(&fpr) && ring.is_own_address(from));
    if !matches_from {
        status.problem = Some(Problem::SignerMismatch);
    }
    let known: Vec<_> = ring.peers_for(from);
    status.key_changed =
        !ring.is_own(&fpr) && !known.is_empty() && !known.iter().any(|p| p.fingerprint == fpr);
}

fn key_known(ring: &Keyring, extra: &[SignedPublicKey], from: &str, issuer: &str) -> bool {
    ring.verifiers_for(from)
        .into_iter()
        .chain(extra.iter())
        .any(|k| {
            let fpr = keys::fingerprint_hex(&k.primary_key);
            fpr == issuer
                || fpr.ends_with(issuer)
                || k.public_subkeys.iter().any(|s| {
                    let f = keys::fingerprint_hex(&s.key);
                    f == issuer || f.ends_with(issuer)
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpgp::keyring::{self, Keyring, PeerKey, Source, Trust};
    use crate::openpgp::keys::{Unlock, parse_keys, unlock};
    use crate::parse::{Message, parse_message_with};

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/openpgp/testdata")
                .join(name),
        )
        .expect("fixture")
    }

    fn public(name: &str) -> SignedPublicKey {
        parse_keys(&fixture(name)).unwrap().public.remove(0)
    }

    fn alice_ring() -> Keyring {
        let key = parse_keys(&fixture("alice-secret.asc"))
            .unwrap()
            .secret
            .remove(0);
        let Unlock::Ok(secret) = unlock(key, None).unwrap() else {
            panic!()
        };
        Keyring {
            own: vec![keyring::own_key("alice", secret, true)],
            peers: Vec::new(),
            enabled_accounts: vec!["alice".into()],
            dismissed: Vec::new(),
        }
    }

    fn with_peer(mut ring: Keyring, addr: &str, file: &str, verified: bool) -> Keyring {
        let key = public(file);
        ring.peers.push(PeerKey {
            addr: addr.into(),
            fingerprint: keys::fingerprint_hex(&key.primary_key),
            key,
            source: Source::Autocrypt,
            verified,
            last_seen: 1,
        });
        ring
    }

    fn parse(name: &str, ring: &Keyring) -> Message {
        parse_message_with(&fixture(name), None, ring)
    }

    fn crypto(m: &Message) -> &CryptoStatus {
        m.crypto.as_ref().expect("crypto status")
    }

    const SECRET: &str = "The meeting moved to Thursday";

    // L5: a crafted detached signature panics rPGP's armor reader. The panic is
    // caught (the sidecar survives), but the message must not then be shown as a
    // bland "signed" with no problem: a signature we could not verify at all is
    // surfaced as invalid, never left looking merely unconfirmed.
    #[test]
    fn a_signature_that_panics_the_verifier_is_flagged_invalid_not_signed_ok() {
        let ring = alice_ring();
        let m = parse("l5-panic-sig.bin", &ring);
        let c = crypto(&m);
        assert_eq!(c.problem, Some(Problem::InvalidSignature));
    }

    // H3: Mallory resends a message Alice sent (signed by Alice, encrypted to
    // Alice) as a new mail From her own address. Alice's own key must not vouch
    // for Mallory's From: no "verified", and a reply must not quote it.
    #[test]
    fn own_signed_ciphertext_replayed_by_a_stranger_is_not_trusted() {
        let ring = with_peer(
            alice_ring(),
            "mallory@example.com",
            "mallory-public.asc",
            false,
        );
        let own = &ring.own[0];
        let bob = public("bob-public.asc");
        let inner = b"Content-Type: text/plain; charset=utf-8\r\n\r\nAlice's private note\r\n";
        let armored = crypt::encrypt(inner, &[&own.public, &bob], &own.secret).unwrap();
        let eml = format!(
            "From: Mallory <mallory@example.com>\r\nTo: alice@example.org\r\nSubject: hi\r\nMessage-ID: <x@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n--b\r\nContent-Type: application/octet-stream\r\n\r\n{armored}\r\n--b--\r\n"
        );
        let m = parse_message_with(eml.as_bytes(), None, &ring);
        let c = crypto(&m);
        assert_eq!(
            c.problem,
            Some(Problem::SignerMismatch),
            "own key must not vouch for a stranger's From"
        );
        assert_ne!(c.trust(&ring, &m.from_addr), Trust::Verified);
        // trust_of directly (the path card_lock takes on stored status): our
        // key verifies our own mail, never a stranger's From.
        let own_fpr = ring.own[0].fingerprint.clone();
        assert_eq!(
            ring.trust_of(&own_fpr, "alice@example.org"),
            Trust::Verified
        );
        assert_ne!(
            ring.trust_of(&own_fpr, "mallory@example.com"),
            Trust::Verified
        );
    }

    // H4: a cleartext Autocrypt-Gossip header on an ordinary (unencrypted) mail
    // must not plant a key for the gossiped address.
    #[test]
    fn outer_gossip_in_plain_mail_is_ignored() {
        use base64::Engine as _;
        let ring = alice_ring();
        let mallory = public("mallory-public.asc");
        let kd = base64::engine::general_purpose::STANDARD
            .encode(keys::binary_public(&mallory).unwrap());
        let eml = format!(
            "From: Mallory <mallory@example.com>\r\nTo: alice@example.org\r\nSubject: hi\r\nAutocrypt-Gossip: addr=boss@example.com; keydata={kd}\r\nContent-Type: text/plain\r\n\r\nhello\r\n"
        );
        let m = parse_message_with(eml.as_bytes(), None, &ring);
        assert!(
            m.found_keys.announced.iter().all(|a| !a.gossip),
            "outer gossip must be dropped"
        );
    }

    // A secret key for `email` from a fixture holding several (unprotected).
    fn secret_of(file: &str, email: &str) -> pgp::composed::SignedSecretKey {
        let parsed = parse_keys(&fixture(file)).unwrap();
        let key = parsed
            .secret
            .into_iter()
            .find(|k| {
                keys::secret_info(k, keys::now())
                    .emails
                    .iter()
                    .any(|e| e == email)
            })
            .expect("secret key for email");
        match unlock(key, None).unwrap() {
            Unlock::Ok(k) => k,
            _ => panic!("locked"),
        }
    }

    // Wrap an armored OpenPGP message as a PGP/MIME mail from `from`.
    fn wrap_encrypted(from: &str, armored: &str) -> String {
        format!(
            "From: {from}\r\nTo: alice@example.org\r\nSubject: hi\r\nMessage-ID: <x@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n--b\r\nContent-Type: application/octet-stream\r\n\r\n{armored}\r\n--b--\r\n"
        )
    }

    // G3: a message encrypted to us but whose signature names only someone else
    // as the intended recipient was redirected to us, and is flagged.
    #[test]
    fn a_message_whose_intended_recipient_is_not_us_is_flagged_g3() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let carol = secret_of("two-secret-keys.asc", "carol@example.com");
        let alice = ring.own[0].public.clone();
        let bob = public("bob-public.asc");
        let inner = b"Content-Type: text/plain; charset=utf-8\r\n\r\nredirected body\r\n";
        // Encrypted to Alice, but Carol's signature names only Bob as intended.
        let armored = crypt::encrypt_to(inner, &[&alice, &bob], &[&bob], &carol).unwrap();
        let m = parse_message_with(
            wrap_encrypted("Carol <carol@example.com>", &armored).as_bytes(),
            None,
            &ring,
        );
        assert_eq!(
            crypto(&m).problem,
            Some(Problem::Redirected),
            "G3: intended recipients exclude us"
        );
        assert_ne!(crypto(&m).trust(&ring, &m.from_addr), Trust::Verified);
    }

    // A normal message to us (intended includes us) is not flagged (no G3 false
    // positive).
    #[test]
    fn a_message_intended_for_us_is_not_flagged_g3() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let carol = secret_of("two-secret-keys.asc", "carol@example.com");
        let alice = ring.own[0].public.clone();
        let inner = b"Content-Type: text/plain; charset=utf-8\r\n\r\nhello\r\n";
        let armored = crypt::encrypt(inner, &[&alice], &carol).unwrap();
        let m = parse_message_with(
            wrap_encrypted("Carol <carol@example.com>", &armored).as_bytes(),
            None,
            &ring,
        );
        assert_eq!(
            crypto(&m).problem,
            None,
            "a message intended for us is fine"
        );
    }

    // G4: the decrypted inner From differs from the envelope From — the payload
    // was lifted from a different sender's message.
    #[test]
    fn a_message_whose_inner_from_differs_is_flagged_g4() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let carol = secret_of("two-secret-keys.asc", "carol@example.com");
        let alice = ring.own[0].public.clone();
        let inner = b"From: Eve <eve@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nbody\r\n";
        let armored = crypt::encrypt(inner, &[&alice], &carol).unwrap();
        let m = parse_message_with(
            wrap_encrypted("Carol <carol@example.com>", &armored).as_bytes(),
            None,
            &ring,
        );
        assert_eq!(
            crypto(&m).problem,
            Some(Problem::Redirected),
            "G4: inner From != outer From"
        );
    }

    #[test]
    fn encrypted_and_signed_by_a_known_key_shows_body_and_inner_subject() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let m = parse("enc-signed.eml", &ring);
        assert!(m.body.contains(SECRET), "{}", m.body);
        assert_eq!(m.subject, "Secret plans");
        let c = crypto(&m);
        assert!(c.encrypted && c.signed);
        assert_eq!(c.problem, None);
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Known);
        assert!(m.attachments.iter().all(|a| a.filename != "encrypted.asc"));
    }

    #[test]
    fn sign_then_encrypt_in_two_layers_keeps_the_inner_subject() {
        // Older Enigmail and K-9: a multipart/signed encrypted as a whole, so the
        // protected Subject sits on the signed part, not on the decrypted top.
        let ring = with_peer(
            alice_ring(),
            "mallory@example.com",
            "mallory-public.asc",
            false,
        );
        let m = parse("enc-two-layer.eml", &ring);
        assert!(m.body.contains(SECRET), "{}", m.body);
        assert_eq!(m.subject, "Layered secret");
        let c = crypto(&m);
        assert!(c.encrypted && c.signed, "the inner signature is found");
    }

    #[test]
    fn a_verified_key_raises_trust_and_an_unknown_one_lowers_it() {
        let verified = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", true);
        let m = parse("enc-signed.eml", &verified);
        assert_eq!(crypto(&m).trust(&verified, &m.from_addr), Trust::Verified);
        let unknown = alice_ring();
        let m = parse("enc-signed.eml", &unknown);
        let c = crypto(&m);
        assert!(m.body.contains(SECRET));
        assert_eq!(c.signer, None);
        assert_eq!(c.problem, None, "an unknown signer is amber, not an error");
        assert_eq!(c.trust(&unknown, &m.from_addr), Trust::Unconfirmed);
    }

    #[test]
    fn unsigned_encrypted_mail_is_encrypted_but_unconfirmed() {
        let ring = alice_ring();
        let m = parse("enc-unsigned.eml", &ring);
        let c = crypto(&m);
        assert!(c.encrypted && !c.signed);
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Unconfirmed);
        assert_eq!(m.subject, "Unsigned secret");
    }

    #[test]
    fn tampered_and_foreign_mail_show_nothing() {
        let ring = alice_ring();
        for (file, problem) in [
            ("enc-tampered.eml", Problem::Integrity),
            ("enc-wrong-recipient.eml", Problem::NoKey),
            ("bomb.eml", Problem::TooLarge),
        ] {
            let m = parse(file, &ring);
            assert_eq!(crypto(&m).problem, Some(problem), "{file}");
            assert!(m.body.is_empty(), "{file}: no plaintext on failure");
            assert!(m.attachments.is_empty(), "{file}");
            assert!(m.body_html.is_none(), "{file}");
        }
    }

    #[test]
    fn efail_wrapper_is_not_decrypted() {
        let m = parse("efail.eml", &alice_ring());
        assert_eq!(crypto(&m).problem, Some(Problem::PartlyEncrypted));
        assert!(!m.body.contains(SECRET));
        assert!(!m.body_html.as_deref().unwrap_or_default().contains(SECRET));
    }

    #[test]
    fn signed_by_someone_other_than_from_is_flagged() {
        let ring = with_peer(
            with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false),
            "mallory@example.com",
            "mallory-public.asc",
            false,
        );
        let m = parse("enc-signer-mismatch.eml", &ring);
        assert_eq!(crypto(&m).problem, Some(Problem::SignerMismatch));
        assert_eq!(crypto(&m).trust(&ring, &m.from_addr), Trust::Unconfirmed);
        let m = parse("signed-by-mallory.eml", &ring);
        assert_eq!(crypto(&m).problem, Some(Problem::SignerMismatch));
    }

    #[test]
    fn signed_only_mail_verifies_and_hides_its_signature_file() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let m = parse("signed.eml", &ring);
        let c = crypto(&m);
        assert!(c.signed && !c.encrypted);
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Known);
        assert!(m.body.contains("signed, not encrypted"));
        assert!(
            m.attachments
                .iter()
                .all(|a| a.mime != "application/pgp-signature")
        );
    }

    #[test]
    fn altered_signed_mail_is_an_invalid_signature() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let m = parse("signed-altered.eml", &ring);
        assert_eq!(crypto(&m).problem, Some(Problem::InvalidSignature));
    }

    #[test]
    fn sha1_signature_is_weak_and_unconfirmed() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let m = parse("signed-sha1.eml", &ring);
        let c = crypto(&m);
        assert!(c.weak_hash);
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Unconfirmed);
    }

    #[test]
    fn mailing_list_footer_makes_it_partial_not_an_error() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let m = parse("list-footer.eml", &ring);
        let c = crypto(&m);
        assert!(c.partial && c.on_list);
        assert_eq!(c.problem, None);
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Unconfirmed);
        // Meron shows the first text part of a multipart/mixed (unchanged
        // behaviour for all mail): here that is the signed text.
        assert!(m.body.contains("signed, not encrypted"), "{}", m.body);
    }

    #[test]
    fn exchange_mixed_up_and_inline_mail_decrypt() {
        let ring = alice_ring();
        let m = parse("mixed-up.eml", &ring);
        assert!(m.body.contains(SECRET), "{}", m.body);
        let m = parse("inline.eml", &ring);
        assert!(crypto(&m).encrypted);
        assert!(
            m.body.contains("Inline secret: the code is 4711"),
            "{}",
            m.body
        );
    }

    #[test]
    fn decrypted_attachments_are_listed_from_the_inner_message() {
        let m = parse("enc-attachment.eml", &alice_ring());
        let names: Vec<&str> = m.attachments.iter().map(|a| a.filename.as_str()).collect();
        assert_eq!(names, vec!["report.txt"]);
        assert!(m.body.contains("The report is attached."));
    }

    #[test]
    fn autocrypt_headers_offer_keys_only_for_the_sender() {
        let ring = alice_ring();
        let m = parse("autocrypt-plain.eml", &ring);
        assert!(crypto(&m).autocrypt);
        assert_eq!(m.found_keys.announced.len(), 1);
        let m = parse("autocrypt-mismatch.eml", &ring);
        assert!(m.found_keys.announced.is_empty());
        assert!(
            m.crypto.is_none(),
            "plain mail without a valid header stays plain"
        );
        let m = parse("autocrypt-list.eml", &ring);
        assert!(crypto(&m).on_list);
    }

    #[test]
    fn a_new_key_for_a_known_address_is_a_key_change() {
        let ring = with_peer(alice_ring(), "carol@example.com", "carol-public.asc", false);
        let m = parse("key-change.eml", &ring);
        let c = crypto(&m);
        assert!(m.body.contains("new laptop"));
        assert!(
            c.signer.is_some(),
            "verified with the key from the Autocrypt header"
        );
        assert!(c.key_changed);
        assert_eq!(c.problem, None);
    }

    #[test]
    fn a_signature_by_a_revoked_key_is_not_valid_even_if_made_before() {
        let ring = with_peer(alice_ring(), "rex@example.com", "revoked-public.asc", false);
        let m = parse("signed-by-revoked.eml", &ring);
        let c = crypto(&m);
        assert_eq!(c.problem, Some(Problem::KeyRevoked));
        assert_eq!(c.signer, None);
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Unconfirmed);
    }

    #[test]
    fn a_key_revoked_by_its_designated_revoker_no_longer_signs() {
        let dan = match unlock(
            parse_keys(&fixture("dan-secret.asc"))
                .unwrap()
                .secret
                .remove(0),
            None,
        )
        .unwrap()
        {
            Unlock::Ok(k) => k,
            other => panic!("{other:?}"),
        };
        let tess = public("tess-public.asc");
        let mut ring = with_peer(alice_ring(), "tess@example.com", "tess-public.asc", false);
        let m = parse("signed-by-tess.eml", &ring);
        assert_eq!(crypto(&m).problem, None, "valid before the revocation");
        ring.peers.retain(|p| p.addr != "tess@example.com");
        let revoked = crate::openpgp::keys::tests::revoke_by(&dan, &tess);
        ring.peers.push(PeerKey {
            addr: "tess@example.com".into(),
            fingerprint: keys::fingerprint_hex(&revoked.primary_key),
            key: revoked,
            source: Source::Autocrypt,
            verified: false,
            last_seen: 2,
        });
        let dan_pub = dan.to_public_key();
        ring.peers.push(PeerKey {
            addr: "dan@example.com".into(),
            fingerprint: keys::fingerprint_hex(&dan_pub.primary_key),
            key: dan_pub,
            source: Source::Autocrypt,
            verified: false,
            last_seen: 2,
        });
        let m = parse("signed-by-tess.eml", &ring);
        assert_eq!(crypto(&m).problem, Some(Problem::KeyRevoked));
    }

    #[test]
    fn a_signature_made_while_the_key_was_valid_stays_valid_after_expiry() {
        let ring = with_peer(
            alice_ring(),
            "erin@example.com",
            "expired-public.asc",
            false,
        );
        let m = parse("signed-by-expired-while-valid.eml", &ring);
        let c = crypto(&m);
        assert_eq!(c.problem, None);
        assert!(c.signer.is_some());
        assert_eq!(c.trust(&ring, &m.from_addr), Trust::Known);
    }

    /// A message shaped like Thunderbird's: text plus the sender's key attached.
    fn with_key_attachment(key_file: &str) -> Vec<u8> {
        let key = String::from_utf8(fixture(key_file))
            .unwrap()
            .replace('\n', "\r\n");
        format!(
            "From: Carol Example <carol@example.com>\r\nTo: alice@example.org\r\nSubject: Testing this\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello\r\n\
             --b\r\nContent-Type: application/pgp-keys; name=\"OpenPGP_0x1234.asc\"\r\n\
             Content-Disposition: attachment; filename=\"OpenPGP_0x1234.asc\"\r\n\r\n{key}\r\n--b--\r\n"
        )
        .into_bytes()
    }

    #[test]
    fn the_senders_attached_key_is_recorded_and_not_shown_as_a_file() {
        let m = parse_message_with(
            &with_key_attachment("carol-public.asc"),
            None,
            &alice_ring(),
        );
        assert!(
            m.attachments.is_empty(),
            "{:?}",
            m.attachments
                .iter()
                .map(|a| &a.filename)
                .collect::<Vec<_>>()
        );
        assert_eq!(m.found_keys.attached.len(), 1);
        assert!(m.body.contains("Hello"));
    }

    #[test]
    fn someone_elses_key_stays_a_visible_attachment() {
        let m = parse_message_with(
            &with_key_attachment("mallory-public.asc"),
            None,
            &alice_ring(),
        );
        assert_eq!(m.attachments.len(), 1);
        assert!(
            m.found_keys.attached.is_empty(),
            "never recorded for an address other than the sender's"
        );
    }

    #[test]
    fn plain_mail_has_no_crypto_status() {
        let raw = b"From: a@example.com\r\nTo: b@example.com\r\nSubject: hi\r\n\r\nhello\r\n";
        assert!(
            parse_message_with(raw, None, &alice_ring())
                .crypto
                .is_none()
        );
    }

    #[test]
    fn our_own_sent_mail_counts_as_verified() {
        let ring = alice_ring();
        let me = &ring.own[0];
        let carol = public("carol-public.asc");
        let inner = "Content-Type: text/plain; charset=utf-8; protected-headers=\"v1\"\r\nSubject: From me\r\n\r\nhello carol\r\n";
        let armored = crypt::encrypt(inner.as_bytes(), &[&carol, &me.public], &me.secret).unwrap();
        let raw = format!(
            "From: Alice <alice@example.org>\r\nTo: carol@example.com\r\nSubject: [...]\r\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: application/pgp-encrypted\r\n\r\nVersion: 1\r\n\r\n--b\r\nContent-Type: application/octet-stream\r\n\r\n{}\r\n--b--\r\n",
            armored.replace('\n', "\r\n")
        );
        let m = parse_message_with(raw.as_bytes(), None, &ring);
        assert_eq!(m.subject, "From me");
        assert!(m.body.contains("hello carol"));
        assert_eq!(crypto(&m).trust(&ring, &m.from_addr), Trust::Verified);
        assert_eq!(crypto(&m).problem, None);
    }
}
