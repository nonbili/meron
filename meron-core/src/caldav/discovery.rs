//! RFC 6764 service discovery for CalDAV and CardDAV. Unlike the clients'
//! own discovery, this never sends calendar, contacts or mail credentials
//! (or the mailbox name) to the server.

use std::time::{Duration, Instant};

use hickory_resolver::{Resolver, proto::rr::RData};
use rand::Rng;
use url::{Host, Url};

const TIMEOUT: Duration = Duration::from_secs(3);

/// Which DAV service to look for.
#[derive(Clone, Copy)]
pub(crate) enum Dav {
    Calendars,
    Contacts,
}

impl Dav {
    fn name(self) -> &'static str {
        match self {
            Dav::Calendars => "caldav",
            Dav::Contacts => "carddav",
        }
    }

    /// The DAV header token a server of this kind advertises (RFC 4791 §5.1,
    /// RFC 6352 §6.1).
    fn token(self) -> &'static str {
        match self {
            Dav::Calendars => "calendar-access",
            Dav::Contacts => "addressbook",
        }
    }
}

pub(crate) fn discover_server(email: &str, dav: Dav) -> Option<Url> {
    let (mailbox, domain) = email.trim().rsplit_once('@')?;
    if mailbox.is_empty() {
        return None;
    }
    let domain = dns_domain(domain)?;
    let well_known = format!("/.well-known/{}", dav.name());
    match lookup_srv(&domain, dav) {
        Service::Advertised(url) => Some(probe(url.clone(), dav).unwrap_or(url)),
        Service::Unavailable => None,
        Service::Unlisted => probe(
            Url::parse(&format!("https://{domain}{well_known}")).ok()?,
            dav,
        ),
    }
}

fn dns_domain(input: &str) -> Option<String> {
    // Parse as a hostname, not as an authority supplied by the user or DNS.
    let Host::Domain(domain) = Host::parse(input.trim_end_matches('.')).ok()? else {
        return None;
    };
    (!domain.is_empty()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        }))
    .then_some(domain)
}

enum Service {
    Advertised(Url),
    Unavailable,
    Unlisted,
}

fn lookup_srv(domain: &str, dav: Dav) -> Service {
    let lookup = || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        runtime.block_on(async {
            let resolver = Resolver::builder_tokio().ok()?.build().ok()?;
            let name = format!("_{}s._tcp.{domain}.", dav.name());
            let response = tokio::time::timeout(TIMEOUT, resolver.srv_lookup(&name))
                .await
                .ok()?
                .ok()?;
            let records = response
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    RData::SRV(srv) => Some(srv),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if matches!(records.as_slice(), [srv] if srv.target.is_root()) {
                return Some(Service::Unavailable);
            }
            let mut servers = records
                .iter()
                .filter_map(|srv| {
                    let host = dns_domain(&srv.target.to_utf8())?;
                    if srv.port == 0 {
                        return None;
                    }
                    let url = Url::parse(&format!(
                        "https://{host}:{}/.well-known/{}",
                        srv.port,
                        dav.name()
                    ))
                    .ok()?;
                    Some((srv.priority, srv.weight, url))
                })
                .collect::<Vec<_>>();
            let mut url = choose_server(&mut servers, &mut rand::thread_rng())?;
            // A missing or slow TXT record must not discard a valid SRV answer.
            if let Ok(Ok(txt)) =
                tokio::time::timeout(Duration::from_secs(1), resolver.txt_lookup(&name)).await
            {
                for record in txt.answers() {
                    if let RData::TXT(txt) = &record.data {
                        if let Some(endpoint) = txt
                            .txt_data
                            .iter()
                            .filter_map(|data| std::str::from_utf8(data).ok())
                            .find_map(|entry| txt_endpoint(&url, entry))
                        {
                            url = endpoint;
                            break;
                        }
                    }
                }
            }
            Some(Service::Advertised(url))
        })
    };
    lookup().unwrap_or(Service::Unlisted)
}

// RFC 2782 chooses the lowest priority, then uses the weights within it.
fn choose_server(servers: &mut [(u16, u16, Url)], rng: &mut impl Rng) -> Option<Url> {
    servers.sort_unstable_by_key(|&(priority, weight, _)| (priority, weight));
    let priority = servers.first()?.0;
    let group = servers
        .split_at(servers.partition_point(|entry| entry.0 == priority))
        .0;
    let total: u32 = group.iter().map(|entry| u32::from(entry.1)).sum();
    if total == 0 {
        return Some(group[rng.gen_range(0..group.len())].2.clone());
    }
    let mut draw = rng.gen_range(0..=total);
    for (_, weight, url) in group {
        let weight = u32::from(*weight);
        if draw <= weight {
            return Some(url.clone());
        }
        draw -= weight;
    }
    None
}

fn txt_endpoint(base: &Url, entry: &str) -> Option<Url> {
    let (key, path) = entry.split_once('=')?;
    if !key.eq_ignore_ascii_case("path") || !path.starts_with('/') {
        return None;
    }
    let url = base.join(path).ok()?;
    (url.origin() == base.origin() && safe_endpoint(&url)).then_some(url)
}

fn safe_endpoint(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

fn probe(mut url: Url, dav: Dav) -> Option<Url> {
    let agent = crate::proxy::agent().ok()?;
    let deadline = Instant::now() + TIMEOUT;
    for _ in 0..=4 {
        if !safe_endpoint(&url) {
            return None;
        }
        let response = agent
            .options(url.as_str())
            .config()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_global(Some(deadline.checked_duration_since(Instant::now())?))
            .build()
            .call()
            .ok()?;
        let status = response.status().as_u16();
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            url = url
                .join(response.headers().get("location")?.to_str().ok()?)
                .ok()?;
        } else {
            // A redirect or a generic 401 alone is not evidence of the service.
            let headers = response.headers().get_all("dav");
            return (matches!(status, 200..=299 | 401 | 403)
                && advertises(
                    headers.iter().filter_map(|header| header.to_str().ok()),
                    dav,
                ))
            .then_some(url);
        }
    }
    None
}

fn advertises<'a>(mut headers: impl Iterator<Item = &'a str>, dav: Dav) -> bool {
    headers.any(|value| value.split(',').any(|token| token.trim() == dav.token()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn accepts_domains_not_url_authorities() {
        assert_eq!(dns_domain("Example.TEST.").as_deref(), Some("example.test"));
        for input in [
            "",
            "example.test:443",
            "example.test/other",
            "a@b.test",
            "127.0.0.1",
        ] {
            assert_eq!(dns_domain(input), None, "{input}");
        }
    }

    #[test]
    fn txt_path_keeps_the_advertised_https_origin() {
        let base = Url::parse("https://dav.example.test:8443/.well-known/caldav").unwrap();
        assert_eq!(
            txt_endpoint(&base, "path=/remote.php/dav/")
                .unwrap()
                .as_str(),
            "https://dav.example.test:8443/remote.php/dav/"
        );
        for entry in [
            "path=//other.example.test/",
            "path=/\\other.example.test/",
            "path=http://dav.example.test/",
        ] {
            assert!(txt_endpoint(&base, entry).is_none(), "{entry}");
        }
    }

    #[test]
    fn srv_priority_comes_before_weight() {
        let first = Url::parse("https://first.example.test/").unwrap();
        let second = Url::parse("https://second.example.test/").unwrap();
        let mut servers = [(20, 100, second), (10, 0, first.clone())];
        assert_eq!(
            choose_server(&mut servers, &mut StdRng::seed_from_u64(0)),
            Some(first)
        );
    }

    #[test]
    fn a_plain_webdav_server_is_not_a_calendar_or_contacts_server() {
        assert!(advertises(
            ["1, 2", "calendar-access, calendar-schedule"].into_iter(),
            Dav::Calendars
        ));
        assert!(!advertises(
            ["1, 2", "calendar-access-extra"].into_iter(),
            Dav::Calendars
        ));
        assert!(!advertises(
            ["1, 2", "calendar-access"].into_iter(),
            Dav::Contacts
        ));
        assert!(advertises(["1, 3, addressbook"].into_iter(), Dav::Contacts));
    }
}
