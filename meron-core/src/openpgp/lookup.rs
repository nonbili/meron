//! Looking up a recipient's key on demand: WKD (the recipient's own mail
//! domain), keys.openpgp.org (VKS) and HKP keyservers.
//!
//! Only on the user's click: a lookup tells the server who
//! the user is writing to. Found keys are shown before they are added, and
//! only keys whose user IDs carry the looked-up address are offered.

use std::io::Read as _;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use base64::Engine as _;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::keys::{self, KeyInfo};

/// Largest response read from a key source (a key with many signatures is a
/// few hundred KiB; anything larger is not worth parsing).
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
const SETTING_KEY: &str = "openpgp_key_sources";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Web Key Directory: the recipient's own mail domain publishes the key.
    Wkd,
    /// Verifying Keyserver API (keys.openpgp.org): addresses confirmed by email.
    Vks,
    /// Classic HKP keyserver: anyone can upload a key for any address.
    Hkp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySource {
    pub kind: SourceKind,
    /// Base URL (`https://keys.openpgp.org`); empty for WKD.
    #[serde(default)]
    pub url: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

impl KeySource {
    /// Whether the source checks that a key belongs to its address.
    pub fn verified(&self) -> bool {
        matches!(self.kind, SourceKind::Wkd | SourceKind::Vks)
    }

    pub fn label(&self) -> String {
        match self.kind {
            SourceKind::Wkd => "WKD".to_string(),
            _ => self
                .url
                .trim_start_matches("https://")
                .trim_end_matches('/')
                .to_string(),
        }
    }
}

/// WKD and keys.openpgp.org: the sources that check addresses.
pub fn default_sources() -> Vec<KeySource> {
    vec![
        KeySource {
            kind: SourceKind::Wkd,
            url: String::new(),
            enabled: true,
        },
        KeySource {
            kind: SourceKind::Vks,
            url: "https://keys.openpgp.org".into(),
            enabled: true,
        },
    ]
}

pub fn sources(conn: &Connection) -> Vec<KeySource> {
    let value = crate::store::settings_get(conn, &[SETTING_KEY.to_string()]).ok();
    value
        .and_then(|v| v.get(SETTING_KEY).cloned())
        .and_then(|v| serde_json::from_value::<Vec<KeySource>>(v).ok())
        .unwrap_or_else(default_sources)
}

/// Saves the user's source list. Custom sources must be HTTPS keyservers.
pub fn set_sources(conn: &Connection, list: &[KeySource]) -> Result<()> {
    let mut clean = Vec::new();
    for s in list {
        let mut s = s.clone();
        if s.kind == SourceKind::Wkd {
            s.url.clear();
        } else {
            let parsed = url::Url::parse(s.url.trim()).context("not a valid keyserver address")?;
            if parsed.scheme() != "https" || parsed.host_str().is_none() {
                bail!("keyservers must use https://");
            }
            s.url = format!(
                "https://{}{}",
                parsed.host_str().unwrap_or_default(),
                parsed.port().map(|p| format!(":{p}")).unwrap_or_default()
            );
        }
        if !clean
            .iter()
            .any(|c: &KeySource| c.kind == s.kind && c.url == s.url)
        {
            clean.push(s);
        }
    }
    crate::store::setting_set(conn, SETTING_KEY, &serde_json::to_value(&clean)?)
}

/// One key found for the address.
#[derive(Debug, Serialize)]
pub struct Found {
    pub source: KeySource,
    pub verified: bool,
    pub info: KeyInfo,
    /// The public key, for `pgp.lookup.add` (public: not sensitive).
    pub data_b64: String,
}

#[derive(Debug, Serialize)]
pub struct Failed {
    pub source: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct LookupResult {
    pub addr: String,
    pub found: Vec<Found>,
    /// Sources asked (labels), so "nothing found" can say where.
    pub searched: Vec<String>,
    pub failed: Vec<Failed>,
    /// Every key found for the address, usable or not (revoked, expired): the
    /// caller merges them into stored copies, so a revocation found by a
    /// lookup takes effect. Not sent to the UI.
    #[serde(skip)]
    pub all_keys: Vec<pgp::composed::SignedPublicKey>,
}

/// z-base-32 (RFC 6189 alphabet), as WKD names its files.
fn zbase32(data: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &b in data {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// The two WKD URLs for an address (advanced method first, then direct).
pub fn wkd_urls(addr: &str) -> Option<[String; 2]> {
    let (local, domain) = addr.trim().rsplit_once('@')?;
    if local.is_empty() || domain.is_empty() || domain.contains('/') {
        return None;
    }
    let domain = domain.to_ascii_lowercase();
    // WKD needs a real DNS name. Reject ports, IPv6 brackets, IP literals and
    // single-label names, so a click-triggered lookup can't reach a local or
    // private service like x@127.0.0.1:631.
    if domain.contains(':')
        || domain.contains('[')
        || domain.contains(']')
        || !domain.contains('.')
        || domain.parse::<std::net::IpAddr>().is_ok()
    {
        return None;
    }
    let digest = ring::digest::digest(
        &ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        local.to_lowercase().as_bytes(),
    );
    let hash = zbase32(digest.as_ref());
    let l = url::form_urlencoded::byte_serialize(local.as_bytes()).collect::<String>();
    Some([
        format!("https://openpgpkey.{domain}/.well-known/openpgpkey/{domain}/hu/{hash}?l={l}"),
        format!("https://{domain}/.well-known/openpgpkey/hu/{hash}?l={l}"),
    ])
}

fn source_urls(source: &KeySource, addr: &str) -> Vec<String> {
    let enc = url::form_urlencoded::byte_serialize(addr.as_bytes()).collect::<String>();
    match source.kind {
        SourceKind::Wkd => wkd_urls(addr).map(Vec::from).unwrap_or_default(),
        SourceKind::Vks => vec![format!("{}/vks/v1/by-email/{enc}", source.url)],
        SourceKind::Hkp => vec![format!(
            "{}/pks/lookup?op=get&options=mr&search={enc}",
            source.url
        )],
    }
}

/// Fetches one URL: `Ok(None)` for "no key here" (404 and friends).
pub type Fetch<'a> = dyn Fn(&str) -> Result<Option<Vec<u8>>> + 'a;

fn http_fetch(agent: &ureq::Agent, url: &str) -> Result<Option<Vec<u8>>> {
    let mut response = agent
        .get(url)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(TIMEOUT))
        // No redirects: the URL was checked (https, no IP literal, no
        // loopback or local name), and a redirect could point anywhere, an
        // internal address included. WKD and keyservers serve keys directly.
        .max_redirects(0)
        .build()
        .call()
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let status = response.status().as_u16();
    if status == 404 || status == 410 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        bail!("HTTP {status}");
    }
    let mut data = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("response too large");
    }
    Ok(Some(data))
}

/// Looks `addr` up in every enabled source, over the app-wide proxy.
pub fn lookup(addr: &str, sources: &[KeySource]) -> Result<LookupResult> {
    let agent = crate::proxy::agent()?;
    Ok(lookup_with(addr, sources, &|url| http_fetch(&agent, url)))
}

pub fn lookup_with(addr: &str, sources: &[KeySource], fetch: &Fetch<'_>) -> LookupResult {
    let addr = addr.trim().to_ascii_lowercase();
    let mut out = LookupResult {
        addr: addr.clone(),
        found: Vec::new(),
        searched: Vec::new(),
        failed: Vec::new(),
        all_keys: Vec::new(),
    };
    for source in sources.iter().filter(|s| s.enabled) {
        out.searched.push(source.label());
        let mut last_error = None;
        let mut got = None;
        for url in source_urls(source, &addr) {
            match fetch(&url) {
                Ok(Some(body)) => {
                    got = Some(body);
                    break;
                }
                Ok(None) => {}
                // WKD's advanced host often doesn't exist: try the direct one.
                Err(e) => last_error = Some(e.to_string()),
            }
        }
        let Some(body) = got else {
            if let Some(error) = last_error.filter(|_| source.kind != SourceKind::Wkd) {
                out.failed.push(Failed {
                    source: source.label(),
                    error,
                });
            }
            continue;
        };
        let Ok(parsed) = keys::parse_keys(&body) else {
            continue;
        };
        for key in parsed.public {
            let info = keys::info(&key, false, keys::now());
            // Only keys for this address; unusable ones are kept for merging
            // (a revocation) but never offered.
            if !info.emails.contains(&addr) {
                continue;
            }
            out.all_keys.push(key.clone());
            if info.unusable.is_some() {
                continue;
            }
            if out
                .found
                .iter()
                .any(|f| f.info.fingerprint == info.fingerprint)
            {
                continue;
            }
            let Ok(bin) = keys::binary_public(&key) else {
                continue;
            };
            out.found.push(Found {
                verified: source.verified(),
                source: source.clone(),
                info,
                data_b64: base64::engine::general_purpose::STANDARD.encode(bin),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // A source that answers with a redirect isn't followed: it could point at
    // an internal address the URL checks would have refused.
    #[test]
    fn a_redirect_is_not_followed() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = served.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let Ok(mut stream) = stream else { return };
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let request = String::from_utf8_lossy(&buf);
                let reply = if request.starts_with("GET /key ") {
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/internal\r\nContent-Length: 0\r\n\r\n"
                    )
                } else {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    "HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nsecret".to_string()
                };
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        let agent = ureq::Agent::new_with_defaults();
        let result = http_fetch(&agent, &format!("http://127.0.0.1:{port}/key"));
        assert!(result.is_err(), "{result:?}");
        assert_eq!(
            served.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the target was never asked"
        );
    }

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/openpgp/testdata")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn wkd_urls_follow_the_spec_example() {
        // draft-koch-openpgp-webkey-service: Joe.Doe@Example.ORG
        let [advanced, direct] = wkd_urls("Joe.Doe@Example.ORG").unwrap();
        assert_eq!(
            advanced,
            "https://openpgpkey.example.org/.well-known/openpgpkey/example.org/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q?l=Joe.Doe"
        );
        assert_eq!(
            direct,
            "https://example.org/.well-known/openpgpkey/hu/iy9q119eutrkn8s1mk4r39qejnbu3n5q?l=Joe.Doe"
        );
        assert!(wkd_urls("nope").is_none());
    }

    // A recipient whose domain is an IP literal or carries a port must
    // not produce a WKD URL, so a click-triggered lookup can't reach a local or
    // private service (e.g. x@127.0.0.1:631). WKD requires a real DNS name.
    #[test]
    fn wkd_refuses_ip_literals_and_ports() {
        assert!(wkd_urls("x@127.0.0.1").is_none(), "IPv4 literal");
        assert!(wkd_urls("x@127.0.0.1:631").is_none(), "host with port");
        assert!(wkd_urls("x@[::1]").is_none(), "IPv6 literal");
        assert!(wkd_urls("x@192.168.0.5").is_none(), "private IPv4");
        assert!(wkd_urls("x@localhost").is_none(), "no dot, not a FQDN");
        // A normal domain still works.
        assert!(wkd_urls("x@example.org").is_some());
    }

    #[test]
    fn finds_only_usable_keys_for_the_address_and_says_where_it_looked() {
        let keyring = fixture("contacts-keyring.asc");
        let fetch = |url: &str| -> Result<Option<Vec<u8>>> {
            if url.contains("keys.openpgp.org/vks/v1/by-email/carol%40example.com") {
                return Ok(Some(keyring.clone()));
            }
            if url.contains("openpgpkey.example.com") {
                bail!("dns lookup failed");
            }
            Ok(None)
        };
        let res = lookup_with("Carol@Example.com", &default_sources(), &fetch);
        assert_eq!(res.searched, vec!["WKD", "keys.openpgp.org"]);
        assert_eq!(
            res.found.len(),
            1,
            "the keyring holds five keys, one is Carol's"
        );
        assert!(res.found[0].verified);
        assert_eq!(res.found[0].info.emails, vec!["carol@example.com"]);
        assert!(
            res.failed.is_empty(),
            "a missing WKD host is normal, not a failure"
        );
    }

    #[test]
    fn hkp_results_are_marked_unverified_and_revoked_keys_dropped() {
        let keyring = fixture("contacts-keyring.asc");
        let hkp = KeySource {
            kind: SourceKind::Hkp,
            url: "https://keyserver.ubuntu.com".into(),
            enabled: true,
        };
        let fetch = |url: &str| -> Result<Option<Vec<u8>>> {
            assert!(
                url.starts_with(
                    "https://keyserver.ubuntu.com/pks/lookup?op=get&options=mr&search="
                ),
                "{url}"
            );
            Ok(Some(keyring.clone()))
        };
        let res = lookup_with("carol@example.com", &[hkp.clone()], &fetch);
        assert!(!res.found[0].verified);
        let rex = lookup_with("rex@example.com", &[hkp], &fetch);
        assert!(rex.found.is_empty(), "a revoked key is never offered");
        assert_eq!(
            rex.all_keys.len(),
            1,
            "but it is kept, so the caller can merge the revocation"
        );
    }

    #[test]
    fn a_failing_keyserver_is_reported() {
        let res = lookup_with("a@b.org", &default_sources()[1..], &|_: &str| -> Result<
            Option<Vec<u8>>,
        > {
            bail!("timeout")
        });
        assert_eq!(res.failed.len(), 1);
        assert_eq!(res.failed[0].source, "keys.openpgp.org");
    }

    #[test]
    fn sources_are_stored_https_only_and_deduplicated() {
        let conn = crate::store::open_at(":memory:").unwrap();
        assert_eq!(sources(&conn), default_sources());
        let mut list = default_sources();
        list.push(KeySource {
            kind: SourceKind::Hkp,
            url: "https://keyserver.ubuntu.com/".into(),
            enabled: true,
        });
        list.push(KeySource {
            kind: SourceKind::Hkp,
            url: "https://keyserver.ubuntu.com".into(),
            enabled: true,
        });
        set_sources(&conn, &list).unwrap();
        let stored = sources(&conn);
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[2].url, "https://keyserver.ubuntu.com");
        let bad = vec![KeySource {
            kind: SourceKind::Hkp,
            url: "http://pgp.example.com".into(),
            enabled: true,
        }];
        assert!(set_sources(&conn, &bad).is_err());
    }
}
