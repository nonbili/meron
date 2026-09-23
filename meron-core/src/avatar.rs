//! Sender image policy shared by desktop and mobile. Call only when sender images
//! are enabled. Network work is blocking and must run off the UI/async executor.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::Read,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

const MAX_IMAGE_BYTES: usize = 1024 * 1024;
const PLACEHOLDER_HASH: &str = "e5db88ea2322863ca17817b99d60006c625a31cff0dad49cf05d3c6d16a75c17";

#[derive(Clone, Default, Serialize)]
pub struct SenderImage {
    pub src: String,
    pub kind: &'static str,
}

// Err means the lookup was inconclusive (offline, timeout, rate limit, server
// failure, or incomplete body). Only conclusive results belong in the cache.
type Lookup = Result<SenderImage, ()>;

struct CachedImage {
    key: String,
    result: Arc<OnceLock<Lookup>>,
    at: Instant,
}

#[derive(Default)]
struct ImageCache(Mutex<VecDeque<CachedImage>>);
static CACHE: OnceLock<ImageCache> = OnceLock::new();

impl ImageCache {
    fn get_or_load(&self, key: String, load: impl FnOnce() -> Lookup) -> Lookup {
        let result = {
            let mut entries = self.0.lock().unwrap();
            // Keep pending work discoverable until it finishes, even if the cache
            // is full. The network timeout bounds its lifetime.
            entries.retain(|entry| {
                entry.result.get().is_none() || entry.at.elapsed() < Duration::from_secs(3600)
            });
            if let Some(entry) = entries.iter().find(|entry| entry.key == key) {
                Arc::clone(&entry.result)
            } else {
                let result = Arc::new(OnceLock::new());
                entries.push_back(CachedImage {
                    key: key.clone(),
                    result: Arc::clone(&result),
                    at: Instant::now(),
                });
                result
            }
        };
        // Only one caller runs the loader for this key. Other callers wait on
        // this cell without holding the cache lock or blocking unrelated keys.
        let image = result.get_or_init(load).clone();
        let mut entries = self.0.lock().unwrap();
        if image.is_err() {
            entries.retain(|entry| !Arc::ptr_eq(&entry.result, &result));
        }
        while entries.len() > 512
            || entries
                .iter()
                .filter_map(|e| e.result.get())
                .filter_map(|r| r.as_ref().ok())
                .map(|image| image.src.len())
                .sum::<usize>()
                > 16 * 1024 * 1024
        {
            let Some(index) = entries
                .iter()
                .position(|entry| entry.result.get().is_some())
            else {
                break;
            };
            entries.remove(index);
        }
        image
    }
}

fn cached(key: String, load: impl FnOnce() -> Lookup) -> Lookup {
    CACHE.get_or_init(Default::default).get_or_load(key, load)
}

pub fn resolve(params: &serde_json::Value) -> SenderImage {
    let email = params["email"].as_str().unwrap_or("").trim().to_lowercase();
    let size = params["size"].as_u64().unwrap_or(96).clamp(16, 512);
    let Some((local, domain)) = email.rsplit_once('@') else {
        return SenderImage::default();
    };
    if local.is_empty() || local.contains('@') || favicon_domains(domain).is_empty() {
        return SenderImage::default();
    }
    cached(format!("sender:{email}:{size}"), || {
        resolve_sender_with(&email, domain, size, |url| {
            let image = cached(format!("url:{url}"), || {
                Ok(SenderImage {
                    src: fetch_image(url)?.unwrap_or_default(),
                    kind: "",
                })
            })?;
            Ok((!image.src.is_empty()).then_some(image.src))
        })
    })
    .unwrap_or_default()
}

fn resolve_sender_with(
    email: &str,
    domain: &str,
    size: u64,
    mut fetch: impl FnMut(&str) -> Result<Option<String>, ()>,
) -> Lookup {
    let hash = sha256_hex(email.as_bytes());
    let gravatar = fetch(&format!(
        "https://www.gravatar.com/avatar/{hash}?s={size}&d=404"
    ));
    if let Ok(Some(src)) = gravatar {
        return Ok(SenderImage {
            src,
            kind: "gravatar",
        });
    }
    let fallback = resolve_favicon_with(domain, fetch)?;
    if fallback.src.is_empty() && gravatar.is_err() {
        Err(())
    } else {
        Ok(fallback)
    }
}

fn favicon_domains(domain: &str) -> Vec<&str> {
    if domain.len() > 253
        || domain.parse::<std::net::IpAddr>().is_ok()
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return vec![];
    }
    let Some(parent) = psl::domain_str(domain) else {
        return vec![];
    };
    if domain == parent {
        vec![domain]
    } else {
        vec![domain, parent]
    }
}

fn resolve_favicon_with(
    domain: &str,
    mut fetch: impl FnMut(&str) -> Result<Option<String>, ()>,
) -> Lookup {
    let mut transient_failure = false;
    for candidate in favicon_domains(domain) {
        match fetch(&format!("https://icons.duckduckgo.com/ip3/{candidate}.ico")) {
            Ok(Some(src)) => {
                return Ok(SenderImage {
                    src,
                    kind: "favicon",
                });
            }
            Ok(None) => {}
            Err(()) => transient_failure = true,
        }
    }
    if transient_failure {
        Err(())
    } else {
        Ok(SenderImage::default())
    }
}

fn fetch_image(url: &str) -> Result<Option<String>, ()> {
    fetch_image_with_agent(url, &crate::proxy::agent().map_err(|_| ())?)
}

fn fetch_image_with_agent(url: &str, agent: &ureq::Agent) -> Result<Option<String>, ()> {
    let mut response = agent
        .get(url)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .call()
        .map_err(|_| ())?;
    let status = response.status().as_u16();
    if !usable_status(status)? {
        return Ok(None);
    }
    let mut data = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take((MAX_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut data)
        .map_err(|_| ())?;
    Ok(image_data_url(status, &data))
}

fn usable_status(status: u16) -> Result<bool, ()> {
    if status == 408 || status == 429 || status >= 500 {
        Err(())
    } else {
        Ok(status == 200)
    }
}

fn image_data_url(status: u16, data: &[u8]) -> Option<String> {
    if status != 200 || data.len() > MAX_IMAGE_BYTES || sha256_hex(data) == PLACEHOLDER_HASH {
        return None;
    }
    let mime = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif"
    } else if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if data.starts_with(b"\x00\x00\x01\x00") {
        "image/x-icon"
    } else {
        return None;
    };
    // Android's BitmapFactory does not decode ICO. Normalize it here so all
    // clients can display the same favicon; leave other formats untouched.
    if mime == "image/x-icon" {
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(data), image::ImageFormat::Ico);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(1024);
        limits.max_image_height = Some(1024);
        limits.max_alloc = Some(16 * 1024 * 1024);
        reader.limits(limits);
        let bitmap = reader.decode().ok()?;
        let mut png = std::io::Cursor::new(Vec::new());
        bitmap.write_to(&mut png, image::ImageFormat::Png).ok()?;
        return Some(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(png.into_inner())
        ));
    }
    Some(format!("data:{mime};base64,{}", STANDARD.encode(data)))
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registrable_domain_fallback() {
        assert_eq!(
            favicon_domains("em1.cloudflare.com"),
            ["em1.cloudflare.com", "cloudflare.com"]
        );
        assert_eq!(
            favicon_domains("mail.example.co.uk"),
            ["mail.example.co.uk", "example.co.uk"]
        );
        assert_eq!(
            favicon_domains("mail.customer.github.io"),
            ["mail.customer.github.io", "customer.github.io"]
        );
        assert_eq!(favicon_domains("example.com"), ["example.com"]);
        for invalid in [
            "co.uk",
            "localhost",
            "evil.com/path",
            "a..com",
            "127.0.0.1",
            "-bad.com",
        ] {
            assert!(favicon_domains(invalid).is_empty(), "{invalid}");
        }
    }
    #[test]
    fn rejects_placeholder_even_with_success_status() {
        let placeholder = include_bytes!("avatar/duckduckgo-placeholder.png");
        assert!(image_data_url(200, placeholder).is_none());
        assert!(image_data_url(404, placeholder).is_none());
        assert!(image_data_url(200, b"<html>error</html>").is_none());
        assert!(image_data_url(200, &vec![0; MAX_IMAGE_BYTES + 1]).is_none());
        assert!(
            image_data_url(200, b"GIF89a")
                .unwrap()
                .starts_with("data:image/gif;base64,")
        );
    }
    #[test]
    fn fallback_order_and_exhaustion() {
        for success_at in [0, 1, 2] {
            let mut urls = vec![];
            let image = resolve_favicon_with("em1.cloudflare.com", |url| {
                urls.push(url.to_owned());
                Ok((urls.len() == success_at).then(|| "image".to_owned()))
            });
            let image = image.unwrap();
            assert_eq!(urls.len(), if success_at == 1 { 1 } else { 2 });
            assert_eq!(
                urls[0],
                "https://icons.duckduckgo.com/ip3/em1.cloudflare.com.ico"
            );
            if urls.len() == 2 {
                assert_eq!(
                    urls[1],
                    "https://icons.duckduckgo.com/ip3/cloudflare.com.ico"
                );
            }
            assert_eq!(image.src.is_empty(), success_at == 0);
        }
    }
    #[test]
    fn sender_uses_gravatar_then_domain_then_parent() {
        for success_at in 0..=3 {
            let mut urls = vec![];
            let image =
                resolve_sender_with("em@em1.cloudflare.com", "em1.cloudflare.com", 96, |url| {
                    urls.push(url.to_owned());
                    Ok((urls.len() == success_at).then(|| "image".to_owned()))
                });
            let image = image.unwrap();
            assert_eq!(urls.len(), if success_at == 0 { 3 } else { success_at });
            assert!(urls[0].starts_with("https://www.gravatar.com/avatar/"));
            assert!(urls[0].ends_with("?s=96&d=404"));
            if urls.len() > 1 {
                assert_eq!(
                    urls[1],
                    "https://icons.duckduckgo.com/ip3/em1.cloudflare.com.ico"
                );
            }
            if urls.len() > 2 {
                assert_eq!(
                    urls[2],
                    "https://icons.duckduckgo.com/ip3/cloudflare.com.ico"
                );
            }
            assert_eq!(
                image.kind,
                if success_at == 0 {
                    ""
                } else if success_at == 1 {
                    "gravatar"
                } else {
                    "favicon"
                }
            );
        }
    }

    #[test]
    fn converts_ico_for_mobile() {
        let bitmap = image::DynamicImage::new_rgba8(16, 16);
        let mut ico = std::io::Cursor::new(Vec::new());
        bitmap.write_to(&mut ico, image::ImageFormat::Ico).unwrap();
        let src = image_data_url(200, &ico.into_inner()).unwrap();
        assert!(src.starts_with("data:image/png;base64,"));
        let png = STANDARD.decode(src.split_once(',').unwrap().1).unwrap();
        assert!(image::load_from_memory(&png).is_ok());
    }

    #[test]
    fn transient_failures_do_not_poison_url_or_sender_cache() {
        let cache = ImageCache::default();
        let mut calls = 0;
        for attempt in 0..3 {
            let result = cache.get_or_load("sender:test".into(), || {
                resolve_sender_with("test@example.com", "example.com", 96, |url| {
                    let image = cache.get_or_load(url.into(), || {
                        calls += 1;
                        if attempt == 0 {
                            Err(())
                        } else {
                            Ok(SenderImage {
                                src: "image".into(),
                                kind: "gravatar",
                            })
                        }
                    })?;
                    Ok(Some(image.src))
                })
            });
            assert_eq!(result.is_err(), attempt == 0);
        }
        // Both providers fail, then Gravatar recovers; the final lookup is cached.
        assert_eq!(calls, 3);
    }

    #[test]
    fn transient_failure_at_each_fallback_is_retryable() {
        for failed_at in 1..=3 {
            let mut calls = 0;
            let result = resolve_sender_with("test@sub.example.com", "sub.example.com", 96, |_| {
                calls += 1;
                if calls == failed_at {
                    Err(())
                } else {
                    Ok(None)
                }
            });
            assert!(result.is_err());
            assert_eq!(calls, 3);
        }
    }

    #[test]
    fn provider_failure_still_allows_a_working_fallback() {
        let mut calls = 0;
        let image = resolve_sender_with("test@sub.example.com", "sub.example.com", 96, |_| {
            calls += 1;
            if calls < 3 {
                Err(())
            } else {
                Ok(Some("parent icon".into()))
            }
        })
        .unwrap();
        assert_eq!(image.src, "parent icon");
        assert_eq!(image.kind, "favicon");
    }

    #[test]
    fn shares_pending_lookups_even_when_they_fail() {
        for fail in [false, true] {
            let cache = Arc::new(ImageCache::default());
            let (started_tx, started_rx) = std::sync::mpsc::channel();
            let (finish_tx, finish_rx) = std::sync::mpsc::channel();
            let first_cache = Arc::clone(&cache);
            let first = std::thread::spawn(move || {
                first_cache.get_or_load("same".into(), || {
                    started_tx.send(()).unwrap();
                    finish_rx.recv().unwrap();
                    if fail {
                        Err(())
                    } else {
                        Ok(SenderImage::default())
                    }
                })
            });
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let second_cache = Arc::clone(&cache);
            let second = std::thread::spawn(move || {
                second_cache.get_or_load("same".into(), || panic!("duplicate network lookup"))
            });
            // Wait until both callers have joined the cell before completing it.
            let deadline = Instant::now() + Duration::from_secs(5);
            while Arc::strong_count(&cache.0.lock().unwrap()[0].result) < 3 {
                assert!(Instant::now() < deadline, "second lookup did not join");
                std::thread::yield_now();
            }
            finish_tx.send(()).unwrap();
            assert_eq!(first.join().unwrap().is_err(), fail);
            assert_eq!(second.join().unwrap().is_err(), fail);
            if fail {
                assert!(
                    cache
                        .get_or_load("same".into(), || Ok(SenderImage::default()))
                        .is_ok()
                );
            }
        }
    }

    #[test]
    fn http_failures_are_classified_before_caching() {
        for status in [408, 429, 500, 502, 503, 504] {
            assert!(usable_status(status).is_err());
        }
        for status in [403, 404, 410] {
            assert_eq!(usable_status(status), Ok(false));
        }
        assert_eq!(usable_status(200), Ok(true));
    }

    #[test]
    fn connection_and_incomplete_body_failures_are_retryable() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            stream.read(&mut request).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial",
                )
                .unwrap();
        });
        assert!(fetch_image_with_agent(&url, &ureq::Agent::new_with_defaults()).is_err());
        server.join().unwrap();
        // The listener has closed: this lookup must fail, not become a 404.
        assert!(fetch_image_with_agent(&url, &ureq::Agent::new_with_defaults()).is_err());
    }

    #[test]
    fn caches_misses_and_successes() {
        for key in ["test:miss", "test:hit"] {
            cached(key.into(), || {
                Ok(SenderImage {
                    src: if key.ends_with("hit") {
                        "image".into()
                    } else {
                        String::new()
                    },
                    kind: "favicon",
                })
            })
            .unwrap();
            cached(key.into(), || panic!("cached result must be reused")).unwrap();
        }
    }
}
