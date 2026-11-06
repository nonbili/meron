//! Autocrypt headers (Autocrypt Level 1): how keys travel in ordinary mail.
//!
//! `Autocrypt: addr=a@example.org; prefer-encrypt=mutual; keydata=<base64>`
//! Unknown attributes make the header invalid unless they start with `_`
//! (spec §2.1). Keys are only taken when `addr` equals the message's From
//! address, like Delta Chat does.

use base64::Engine as _;
use pgp::composed::SignedPublicKey;

use super::keys;

/// A key announced in a message's headers.
#[derive(Debug, Clone)]
pub struct Announced {
    pub addr: String,
    pub key: SignedPublicKey,
    pub prefer_mutual: bool,
    /// `Autocrypt-Gossip` (a group member's key, sent by someone else).
    pub gossip: bool,
}

/// Parses one header value. `None` when it is malformed or carries an
/// unknown critical attribute.
pub fn parse(value: &str) -> Option<(String, Vec<u8>, bool)> {
    let mut addr = None;
    let mut keydata = None;
    let mut mutual = false;
    for attr in value.split(';') {
        let attr = attr.trim();
        if attr.is_empty() {
            continue;
        }
        let (name, val) = attr.split_once('=')?;
        match name.trim() {
            "addr" => addr = Some(val.trim().to_ascii_lowercase()),
            "keydata" => {
                let compact: String = val.chars().filter(|c| !c.is_whitespace()).collect();
                keydata = base64::engine::general_purpose::STANDARD.decode(compact).ok();
            }
            "prefer-encrypt" => mutual = val.trim() == "mutual",
            other if other.starts_with('_') => {}
            _ => return None,
        }
    }
    Some((addr?, keydata?, mutual))
}

/// Keys announced in `headers` (outer or protected inner ones) for `from`.
/// For `Autocrypt:` the last valid header for the From address wins
/// (Delta Chat's rule); gossip headers are kept for their own addresses.
pub fn announced(headers: &[mailparse::MailHeader<'_>], from: &str) -> Vec<Announced> {
    let from = from.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    let mut own: Option<Announced> = None;
    for header in headers {
        let name = header.get_key();
        let gossip = name.eq_ignore_ascii_case("Autocrypt-Gossip");
        if !gossip && !name.eq_ignore_ascii_case("Autocrypt") {
            continue;
        }
        let Some((addr, data, mutual)) = parse(&header.get_value()) else { continue };
        let Ok(key) = keys::parse_public(&data) else { continue };
        let found = Announced { addr: addr.clone(), key, prefer_mutual: mutual, gossip };
        if gossip {
            out.push(found);
        } else if addr == from {
            own = Some(found);
        }
    }
    out.extend(own);
    out
}

/// The header value we send for `addr` with `key`, folded for SMTP.
pub fn header_value(addr: &str, key: &SignedPublicKey) -> anyhow::Result<String> {
    let data = base64::engine::general_purpose::STANDARD.encode(keys::binary_public(key)?);
    let mut value = format!("addr={addr}; prefer-encrypt=mutual; keydata=");
    for (i, chunk) in data.as_bytes().chunks(76).enumerate() {
        if i > 0 || !value.is_empty() {
            value.push_str("\r\n ");
        }
        value.push_str(std::str::from_utf8(chunk).unwrap_or_default());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/openpgp/testdata").join(name))
            .expect("fixture")
    }

    #[test]
    fn plain_mail_announces_the_senders_key() {
        let raw = fixture("autocrypt-plain.eml");
        let mail = mailparse::parse_mail(&raw).unwrap();
        let found = announced(&mail.headers, "carol@example.com");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].addr, "carol@example.com");
        assert!(found[0].prefer_mutual);
        assert!(!found[0].gossip);
    }

    #[test]
    fn header_for_another_address_is_ignored() {
        let raw = fixture("autocrypt-mismatch.eml");
        let mail = mailparse::parse_mail(&raw).unwrap();
        assert!(announced(&mail.headers, "mallory@example.com").is_empty());
    }

    #[test]
    fn unknown_critical_attribute_invalidates_but_underscore_does_not() {
        assert!(parse("addr=a@b.org; keydata=AAAA; color=red").is_none());
        assert!(parse("addr=a@b.org; _color=red; keydata=AAAA").is_some());
        assert!(parse("keydata=AAAA").is_none());
    }

    #[test]
    fn emitted_header_parses_back_to_the_same_key() {
        let key = keys::generate(&["me@example.org".into()]).unwrap().to_public_key();
        let value = header_value("me@example.org", &key).unwrap();
        assert!(value.lines().all(|l| l.len() <= 78));
        let (addr, data, mutual) = parse(&value.replace("\r\n", "")).unwrap();
        assert_eq!(addr, "me@example.org");
        assert!(mutual);
        assert_eq!(keys::fingerprint_hex(&keys::parse_public(&data).unwrap().primary_key), keys::fingerprint_hex(&key.primary_key));
    }
}
