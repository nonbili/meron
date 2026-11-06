//! PGP/MIME on the sending side (RFC 3156 with RFC 9788 protected headers).
//!
//! The message is built as usual (`smtp::build_message`); that whole message,
//! headers included, becomes the encrypted payload, marked
//! `protected-headers="v1"` and carrying our Autocrypt header (Delta Chat
//! checks the signature against the key announced *inside*). The outer
//! message keeps only what transport needs, with the Subject replaced by
//! `[...]`, so the real subject travels encrypted (as Thunderbird does).

use anyhow::{Context as _, Result, bail};
use pgp::composed::{SignedPublicKey, SignedSecretKey};

use super::autocrypt;
use super::crypt;
use super::keyring::{Keyring, NoKeyReason};

/// What an encrypted send needs.
pub struct SendKeys {
    pub signer: SignedSecretKey,
    pub own: SignedPublicKey,
    /// To and Cc recipients' keys (one message for all of them).
    pub to_cc: Vec<SignedPublicKey>,
    /// Each Bcc recipient (entry as typed, key): a message of its own, so no
    /// recipient learns of a Bcc from the key IDs in the ciphertext.
    pub bcc: Vec<(String, SignedPublicKey)>,
    /// The Autocrypt header value for the From address.
    pub autocrypt: String,
}

/// A recipient that can't be encrypted to, for the composer and the error.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct Missing {
    pub addr: String,
    pub reason: NoKeyReason,
}

/// Bare lower-case address of a `Name <addr>` entry.
pub fn bare_addr(entry: &str) -> String {
    let e = entry.trim();
    match (e.rfind('<'), e.rfind('>')) {
        (Some(s), Some(t)) if t > s => e[s + 1..t].trim().to_ascii_lowercase(),
        _ => e.trim_matches('"').to_ascii_lowercase(),
    }
}

/// Splits a comma-separated recipient field like `smtp::parse_addrs`
/// (commas inside quotes or angle brackets don't split).
pub fn split_recipients(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut quoted, mut bracket) = (false, false);
    for ch in raw.chars() {
        match ch {
            '"' => quoted = !quoted,
            '<' if !quoted => bracket = true,
            '>' if !quoted => bracket = false,
            ',' if !quoted && !bracket => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// The key to encrypt to for one address: a correspondent's, or our own
/// when writing to one of our accounts.
fn key_for<'a>(ring: &'a Keyring, addr: &str) -> std::result::Result<&'a SignedPublicKey, NoKeyReason> {
    match ring.encryption_key_for(addr) {
        Ok(peer) => Ok(&peer.key),
        Err(reason) => ring.own_key_for_addr(addr).map(|k| &k.public).ok_or(reason),
    }
}

/// How a send or draft goes out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    Encrypt,
    Plain,
}

/// The core's rule for every send and draft: encrypted when the
/// caller asks for it, and also when the message answers encrypted mail and
/// the caller didn't explicitly ask for plain. A caller that says nothing
/// (an MCP tool, a composer path that lost the flag) can't downgrade a thread.
pub fn protection(encrypt: bool, plain: bool, answers_encrypted: bool) -> Protection {
    if encrypt || (answers_encrypted && !plain) { Protection::Encrypt } else { Protection::Plain }
}

/// The correspondent key each recipient would get (bare address, primary
/// fingerprint). Our own addresses and recipients without a key are left out:
/// only a correspondent's key can change behind the user's back.
pub fn chosen_peer_keys(ring: &Keyring, addrs: &[String]) -> Vec<(String, String)> {
    addrs
        .iter()
        .map(|a| bare_addr(a))
        .filter(|a| !a.is_empty())
        .filter_map(|addr| ring.encryption_key_for(&addr).ok().map(|peer| (addr, peer.fingerprint.clone())))
        .collect()
}

/// Which recipients have no usable key (empty: all can be encrypted to).
pub fn missing_keys(ring: &Keyring, addrs: &[String]) -> Vec<Missing> {
    addrs
        .iter()
        .map(|a| bare_addr(a))
        .filter(|a| !a.is_empty())
        .filter_map(|addr| key_for(ring, &addr).err().map(|reason| Missing { addr, reason }))
        .collect()
}

/// Gathers the keys for an encrypted send, or fails naming every recipient
/// without one. Never falls back to sending unencrypted: the composer must
/// have resolved that before (security notes R9).
pub fn resolve(ring: &Keyring, account_id: &str, from_addr: &str, to: &str, cc: &str, bcc: &str) -> Result<SendKeys> {
    let own = ring
        .default_key(account_id)
        .filter(|_| ring.is_enabled(account_id))
        .context("end-to-end encryption is not set up for this account")?;
    if super::keys::check(&own.public, super::keys::now()).is_some() {
        bail!("your encryption key has expired or can't be used: replace it in the account settings");
    }
    let to_cc: Vec<String> = split_recipients(to).into_iter().chain(split_recipients(cc)).collect();
    let bcc_list = split_recipients(bcc);
    let all: Vec<String> = to_cc.iter().chain(bcc_list.iter()).cloned().collect();
    let missing = missing_keys(ring, &all);
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|m| m.addr.as_str()).collect();
        bail!("can't encrypt to {}: no usable key", names.join(", "));
    }
    let key = |entry: &str| key_for(ring, &bare_addr(entry)).map(Clone::clone).map_err(|_| anyhow::anyhow!("no key"));
    Ok(SendKeys {
        signer: own.secret.clone(),
        own: own.public.clone(),
        to_cc: to_cc.iter().map(|e| key(e)).collect::<Result<_>>()?,
        bcc: bcc_list.iter().map(|e| Ok((e.clone(), key(e)?))).collect::<Result<_>>()?,
        autocrypt: autocrypt::header_value(&bare_addr(from_addr), &own.public)?,
    })
}

/// The Autocrypt header value for plain sends from an account with
/// encryption on (keys spread without the user doing anything).
pub fn autocrypt_for(ring: &Keyring, account_id: &str, from_addr: &str) -> Option<String> {
    if !ring.is_enabled(account_id) {
        return None;
    }
    let own = ring.default_key(account_id)?;
    autocrypt::header_value(&bare_addr(from_addr), &own.public).ok()
}

/// Splits a built message at the blank line between headers and body.
fn split(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    let pos = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some((&raw[..pos + 2], &raw[pos + 4..]))
}

/// Header fields of a header block, each with its continuation lines.
fn fields(head: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(head);
    let mut out: Vec<String> = Vec::new();
    for line in text.split("\r\n").filter(|l| !l.is_empty()) {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = out.last_mut() {
                last.push_str("\r\n");
                last.push_str(line);
            }
        } else {
            out.push(line.to_string());
        }
    }
    out
}

fn name_of(field: &str) -> String {
    field.split(':').next().unwrap_or_default().trim().to_ascii_lowercase()
}

/// Adds a header field at the top of a built message.
pub fn add_header(raw: &[u8], name: &str, value: &str) -> Vec<u8> {
    let mut out = format!("{name}: {value}\r\n").into_bytes();
    out.extend_from_slice(raw);
    out
}

/// The encrypted payload: the built message itself, its Content-Type marked
/// `protected-headers="v1"`, plus our Autocrypt header.
pub fn inner_payload(built: &[u8], autocrypt: &str) -> Result<Vec<u8>> {
    let (head, body) = split(built).context("built message has no header block")?;
    let mut out = String::new();
    out.push_str(&format!("Autocrypt: {autocrypt}\r\n"));
    for field in fields(head) {
        if name_of(&field) == "content-type" {
            out.push_str(&field);
            out.push_str("; protected-headers=\"v1\"\r\n");
        } else {
            out.push_str(&field);
            out.push_str("\r\n");
        }
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    Ok(bytes)
}

/// Headers kept outside: what delivery, threading and the mailbox list need.
const OUTER: &[&str] = &["from", "to", "cc", "bcc", "reply-to", "date", "message-id", "in-reply-to", "references"];

/// Encrypts `built` for `recipients` (callers include our own key) and wraps
/// it as RFC 3156 `multipart/encrypted` with a `[...]` subject.
pub fn wrap(built: &[u8], recipients: &[&SignedPublicKey], signer: &SignedSecretKey, autocrypt: &str) -> Result<Vec<u8>> {
    let payload = inner_payload(built, autocrypt)?;
    let armored = crypt::encrypt(&payload, recipients, signer)?;
    let (head, _) = split(built).context("built message has no header block")?;
    let boundary = format!("meron-pgp-{}", uuid::Uuid::new_v4().simple());
    let mut out = String::new();
    for field in fields(head) {
        if OUTER.contains(&name_of(&field).as_str()) {
            out.push_str(&field);
            out.push_str("\r\n");
        }
    }
    out.push_str("Subject: [...]\r\n");
    out.push_str("MIME-Version: 1.0\r\n");
    out.push_str(&format!("Autocrypt: {autocrypt}\r\n"));
    out.push_str(&format!(
        "Content-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\";\r\n boundary=\"{boundary}\"\r\n\r\n"
    ));
    out.push_str("This is an OpenPGP/MIME encrypted message (RFC 4880 and 3156)\r\n");
    out.push_str(&format!("--{boundary}\r\n"));
    out.push_str("Content-Type: application/pgp-encrypted\r\nContent-Description: PGP/MIME version identification\r\n\r\nVersion: 1\r\n\r\n");
    out.push_str(&format!("--{boundary}\r\n"));
    out.push_str("Content-Type: application/octet-stream; name=\"encrypted.asc\"\r\n");
    out.push_str("Content-Description: OpenPGP encrypted message\r\n");
    out.push_str("Content-Disposition: inline; filename=\"encrypted.asc\"\r\n\r\n");
    out.push_str(&armored.replace("\r\n", "\n").replace('\n', "\r\n"));
    if !out.ends_with("\r\n") {
        out.push_str("\r\n");
    }
    out.push_str(&format!("\r\n--{boundary}--\r\n"));
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpgp::keyring::{self, PeerKey, Source, Trust};
    use crate::openpgp::keys;

    fn ring_with(me: &SignedSecretKey, peers: &[(&str, &SignedSecretKey)]) -> Keyring {
        Keyring {
            own: vec![keyring::own_key("acc", me.clone(), true)],
            peers: peers
                .iter()
                .map(|(addr, k)| {
                    let key = k.to_public_key();
                    PeerKey { addr: addr.to_string(), fingerprint: keys::fingerprint_hex(&key.primary_key), key, source: Source::Autocrypt, verified: false, last_seen: 1 }
                })
                .collect(),
            enabled_accounts: vec!["acc".into()],
            dismissed: Vec::new(),
        }
    }

    fn built(bcc: bool) -> Vec<u8> {
        crate::smtp::build_message(
            "Me",
            "me@example.org",
            "Peer <peer@example.org>",
            "",
            if bcc { "hidden@example.org" } else { "" },
            bcc,
            "Real subject",
            "Secret body text",
            "",
            &[],
            "",
            "",
            "",
            "abc@example.org",
        )
        .unwrap()
    }

    #[test]
    fn wrapped_message_hides_subject_and_body_and_round_trips_through_our_parser() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let peer = keys::generate(&["peer@example.org".into()]).unwrap();
        let ring = ring_with(&me, &[("peer@example.org", &peer)]);
        let keys = resolve(&ring, "acc", "me@example.org", "Peer <peer@example.org>", "", "").unwrap();
        let raw = wrap(&built(false), &[&keys.to_cc[0], &keys.own], &keys.signer, &keys.autocrypt).unwrap();
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("Subject: [...]"));
        assert!(!text.contains("Real subject"));
        assert!(!text.contains("Secret body text"));
        assert!(text.contains("Message-ID: <abc@example.org>"));
        assert!(text.contains("Autocrypt: addr=me@example.org"));
        // The peer reads it: real subject, body, our key verified.
        let peer_ring = ring_with(&peer, &[("me@example.org", &me)]);
        let m = crate::parse::parse_message_with(&raw, None, &peer_ring);
        assert_eq!(m.subject, "Real subject");
        assert!(m.body.contains("Secret body text"));
        let c = m.crypto.unwrap();
        assert!(c.encrypted && c.signed);
        assert_eq!(c.trust(&peer_ring, &m.from_addr), Trust::Known);
        // And so do we (Sent copy), as verified.
        let m = crate::parse::parse_message_with(&raw, None, &ring);
        assert_eq!(m.crypto.unwrap().trust(&ring, &m.from_addr), Trust::Verified);
    }

    #[test]
    fn a_reply_to_encrypted_mail_is_encrypted_unless_plain_is_asked_for() {
        // Audit M1: callers that say nothing (MCP, a path that forgot the flag)
        // get encryption when the reply answers encrypted mail.
        use Protection::{Encrypt, Plain};
        assert_eq!(protection(true, false, false), Encrypt);
        assert_eq!(protection(false, false, false), Plain);
        assert_eq!(protection(false, false, true), Encrypt, "nothing said, answers encrypted mail");
        assert_eq!(protection(false, true, true), Plain, "the user turned encryption off");
        assert_eq!(protection(true, true, true), Encrypt, "encrypt wins over a stray plain");
    }

    #[test]
    fn chosen_keys_name_each_correspondent_but_not_our_own_addresses() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let peer = keys::generate(&["peer@example.org".into()]).unwrap();
        let ring = ring_with(&me, &[("peer@example.org", &peer)]);
        let chosen = chosen_peer_keys(&ring, &["Peer <Peer@example.org>".into(), "me@example.org".into(), "x@example.org".into()]);
        assert_eq!(chosen, vec![("peer@example.org".to_string(), keys::fingerprint_hex(&peer.primary_key))]);
    }

    #[test]
    fn a_recipient_without_a_key_is_refused_not_downgraded() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let ring = ring_with(&me, &[]);
        let err = resolve(&ring, "acc", "me@example.org", "x@example.org, Y <y@example.org>", "", "").err().unwrap();
        assert!(err.to_string().contains("x@example.org, y@example.org"), "{err}");
        let missing = missing_keys(&ring, &["x@example.org".into(), "me@example.org".into()]);
        assert_eq!(missing, vec![Missing { addr: "x@example.org".into(), reason: NoKeyReason::None }]);
    }

    #[test]
    fn an_account_with_encryption_off_cannot_send_encrypted() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let mut ring = ring_with(&me, &[]);
        ring.enabled_accounts.clear();
        assert!(resolve(&ring, "acc", "me@example.org", "me@example.org", "", "").is_err());
        assert!(autocrypt_for(&ring, "acc", "me@example.org").is_none());
    }

    #[test]
    fn bcc_recipients_get_their_own_message_without_the_others_keys() {
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let peer = keys::generate(&["peer@example.org".into()]).unwrap();
        let hidden = keys::generate(&["hidden@example.org".into()]).unwrap();
        let ring = ring_with(&me, &[("peer@example.org", &peer), ("hidden@example.org", &hidden)]);
        let k = resolve(&ring, "acc", "me@example.org", "peer@example.org", "", "hidden@example.org").unwrap();
        assert_eq!(k.bcc.len(), 1);
        // The To copy: the hidden recipient can't open it.
        let main = wrap(&built(false), &[&k.to_cc[0], &k.own], &k.signer, &k.autocrypt).unwrap();
        let hidden_ring = ring_with(&hidden, &[("me@example.org", &me)]);
        let m = crate::parse::parse_message_with(&main, None, &hidden_ring);
        assert_eq!(m.crypto.unwrap().problem, Some(crate::openpgp::mime::Problem::NoKey));
        // Its own copy it can.
        let copy = wrap(&built(false), &[&k.bcc[0].1, &k.own], &k.signer, &k.autocrypt).unwrap();
        assert!(crate::parse::parse_message_with(&copy, None, &hidden_ring).body.contains("Secret body text"));
        assert!(!String::from_utf8_lossy(&main).contains("hidden@example.org"));
    }

    #[test]
    fn gnupg_reads_our_pgp_mime_payload() {
        let Some(home) = crate::openpgp::crypt::tests::gpg_home() else {
            eprintln!("gpg not installed: interop test skipped");
            return;
        };
        let me = keys::generate(&["me@example.org".into()]).unwrap();
        let carol_secret = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/openpgp/testdata/alice-secret.asc")).unwrap();
        let alice = keys::parse_keys(&carol_secret).unwrap().secret.remove(0);
        let gpg = crate::openpgp::crypt::tests::gpg;
        assert!(gpg(home.path(), &["--import"], &carol_secret).unwrap().0);
        assert!(gpg(home.path(), &["--import"], keys::armored_public(&me.to_public_key()).unwrap().as_bytes()).unwrap().0);
        let raw = wrap(&built(false), &[&alice.to_public_key(), &me.to_public_key()], &me, "addr=me@example.org; keydata=AA").unwrap();
        let text = String::from_utf8_lossy(&raw);
        let start = text.find("-----BEGIN PGP MESSAGE-----").unwrap();
        let end = text.find("-----END PGP MESSAGE-----").unwrap() + 25;
        let (ok, plain, err) = gpg(home.path(), &["--status-fd", "2", "--decrypt"], text[start..end].as_bytes()).unwrap();
        assert!(ok, "{err}");
        let plain = String::from_utf8_lossy(&plain);
        assert!(plain.contains("protected-headers=\"v1\""));
        assert!(plain.contains("Subject: Real subject"));
        assert!(err.contains("GOODSIG"), "{err}");
    }
}
