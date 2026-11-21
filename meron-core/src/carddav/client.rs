use super::{Book, Card};
use crate::caldav::xml::{CARDDAV, DAV, Multistatus, escape, parse_multistatus};
use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use std::{collections::BTreeMap, time::Duration};
use ureq::RequestExt as _;
use url::Url;

pub(super) struct Client {
    pub server: Url,
    agent: ureq::Agent,
    authorization: String,
    google: bool,
}

pub(super) const GOOGLE_DISCOVERY: &str = "https://www.googleapis.com/.well-known/carddav";

#[derive(Debug)]
struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CardDAV authorization was rejected")
    }
}

impl std::error::Error for Unauthorized {}

/// The server could not be reached at all (refused, timed out, bad TLS).
/// Every URL discovery tries is on the same origin, so none of the others
/// would answer either.
#[derive(Debug)]
struct Unreachable(String);

impl std::fmt::Display for Unreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not reach the CardDAV server: {}", self.0)
    }
}

impl std::error::Error for Unreachable {}

/// How long to wait for a server to accept a connection; one that is down
/// or not listening on that port should fail fast, not after the whole
/// request timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Accept `host`, `host/path` or a full URL; plain `http` only when the user
/// typed it, as for CalDAV.
pub(super) fn server_url(input: &str) -> Result<Url> {
    let input = input.trim();
    ensure!(!input.is_empty(), "enter a CardDAV server URL");
    let url = if input.contains("://") {
        Url::parse(input)
    } else {
        Url::parse(&format!("https://{input}"))
    }
    .context("invalid CardDAV server URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some() && clean(&url),
        "use an HTTP(S) server URL without credentials, query or fragment"
    );
    Ok(url)
}

fn clean(url: &Url) -> bool {
    url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

impl Client {
    pub fn basic(server: Url, username: &str, password: &str) -> Result<Self> {
        let token =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        Self::new(server, format!("Basic {token}"), false)
    }

    pub fn google(access_token: &str) -> Result<Self> {
        Self::new(
            Url::parse(GOOGLE_DISCOVERY)?,
            format!("Bearer {access_token}"),
            true,
        )
    }

    fn new(server: Url, authorization: String, google: bool) -> Result<Self> {
        Ok(Self {
            server,
            authorization,
            google,
            agent: crate::proxy::agent_for(crate::proxy::global().as_ref())?,
        })
    }

    fn check(&self, url: &Url) -> Result<()> {
        let upgrade = self.server.scheme() == "http"
            && self.server.port().is_none()
            && url.scheme() == "https"
            && url.port().is_none()
            && url.host_str() == self.server.host_str();
        ensure!(
            clean(url) && (url.origin() == self.server.origin() || upgrade),
            "CardDAV server pointed outside its own origin; not sending credentials"
        );
        Ok(())
    }

    fn request(
        &self,
        method: &str,
        url: Url,
        depth: &str,
        body: &str,
    ) -> Result<(Url, u16, String)> {
        self.send(method, url, &[("Depth", depth)], body)
    }

    fn send(
        &self,
        method: &str,
        mut url: Url,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<(Url, u16, String)> {
        let mut https = url.scheme() == "https";
        for _ in 0..=5 {
            self.check(&url)?;
            ensure!(
                !https || url.scheme() == "https",
                "CardDAV redirected to insecure HTTP"
            );
            https |= url.scheme() == "https";
            let mut request = ureq::http::Request::builder()
                .method(method)
                .uri(url.as_str())
                .header("Authorization", &self.authorization);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            if !body.is_empty() {
                request = request.header(
                    "Content-Type",
                    if matches!(method, "PUT" | "POST") {
                        "text/vcard; charset=utf-8"
                    } else {
                        "application/xml; charset=utf-8"
                    },
                );
            }
            let mut response = request
                .body(body.as_bytes())?
                .with_agent(&self.agent)
                .configure()
                .allow_non_standard_methods(true)
                .http_status_as_error(false)
                .max_redirects(0)
                .timeout_connect(Some(CONNECT_TIMEOUT))
                .timeout_global(Some(Duration::from_secs(30)))
                .run()
                .map_err(|err| {
                    if method == "POST" {
                        anyhow::anyhow!("Google Contacts did not confirm creation. Sync and check whether the contact exists before retrying.")
                    } else if crate::caldav::unreachable(&err) {
                        Unreachable(err.to_string()).into()
                    } else {
                        anyhow::anyhow!("CardDAV {method} request failed: {err}")
                    }
                })?;
            let status = response.status().as_u16();
            if matches!(status, 301 | 302 | 307 | 308) {
                ensure!(
                    matches!(method, "PROPFIND" | "REPORT"),
                    "CardDAV redirected the contact request; sync and try again"
                );
                let location = response
                    .headers()
                    .get("location")
                    .context("CardDAV redirect missing Location")?
                    .to_str()?;
                url = url.join(location).context("invalid CardDAV redirect")?;
                continue;
            }
            if status == 401 {
                if self.google {
                    return Err(crate::google_grant::Rejected(
                        "Google Contacts rejected authorization. Reconnect Google Contacts to resume syncing.",
                    )
                    .into());
                }
                return Err(Unauthorized.into());
            }
            let text = if matches!(method, "GET" | "PROPFIND" | "REPORT") {
                response
                    .body_mut()
                    .with_config()
                    .limit(32 * 1024 * 1024)
                    .read_to_string()?
            } else {
                String::new()
            };
            return Ok((url, status, text));
        }
        bail!("too many CardDAV redirects")
    }

    pub(super) fn create(&self, book: &str, uid: &str, vcard: &str) -> Result<()> {
        let mut url = Url::parse(book)?;
        if self.google {
            let (_, status, _) = self.send("POST", url, &[], vcard)?;
            return written(status);
        }
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("invalid address book URL"))?
            .pop_if_empty()
            .push(&format!("{uid}.vcf"));
        let (_, status, _) = self.send("PUT", url, &[("If-None-Match", "*")], vcard)?;
        if status == 412 {
            bail!("This contact was already created. Sync the address book to see it.");
        }
        written(status)
    }

    pub(super) fn change(
        &self,
        book: &str,
        url: &str,
        etag: &str,
        edit: impl FnOnce(&str) -> Result<String>,
        delete: bool,
    ) -> Result<()> {
        let url = Url::parse(url)?;
        ensure!(
            member_of(&Url::parse(book)?, &url),
            "CardDAV member outside its address book"
        );
        // If-Match must be a strong entity tag and never the wildcard.
        let etag = write_etag(etag).context("This server did not provide a strong contact version; Meron cannot safely edit or delete this contact")?;
        let body = if delete {
            String::new()
        } else {
            let (_, status, body) = self.send("GET", url.clone(), &[], "")?;
            if matches!(status, 404 | 410) {
                bail!("This contact changed on the server. Sync the address book and try again.");
            }
            ensure!(
                status == 200,
                "CardDAV could not read the contact (HTTP {status})"
            );
            edit(&body)?
        };
        let (_, status, _) = self.send(
            if delete { "DELETE" } else { "PUT" },
            url,
            &[("If-Match", &etag)],
            &body,
        )?;
        written(status)
    }

    fn propfind(&self, url: &Url, depth: &str, props: &str) -> Result<(Url, Multistatus)> {
        let body = format!(
            r#"<d:propfind xmlns:d="DAV:" xmlns:c="{CARDDAV}"><d:prop>{props}</d:prop></d:propfind>"#
        );
        let (base, status, text) = self.request("PROPFIND", url.clone(), depth, &body)?;
        ensure!(
            status == 207,
            "CardDAV PROPFIND returned HTTP {status}{}",
            google_reason(&text)
        );
        Ok((base, parse_multistatus(&text)?))
    }

    pub fn discover(&self) -> Result<Vec<Book>> {
        if self.google {
            let books = self.discover_from(&self.server)?;
            ensure!(!books.is_empty(), "no Google Contacts address book found");
            return Ok(books);
        }
        let known = self.server.join("/.well-known/carddav")?;
        let root = self.server.join("/")?;
        let mut tried = Vec::new();
        let mut last_error = None;
        for url in [&self.server, &known, &root] {
            if tried.contains(&url) {
                continue;
            }
            tried.push(url);
            match self.discover_from(url) {
                Ok(books) if !books.is_empty() => return Ok(books),
                Ok(_) => {}
                Err(err) if err.is::<Unauthorized>() || err.is::<Unreachable>() => return Err(err),
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no CardDAV address books found")))
    }

    fn discover_from(&self, url: &Url) -> Result<Vec<Book>> {
        let (base, props) = self.propfind(url, "0", "<d:current-user-principal/><d:principal-URL/><c:addressbook-home-set/><d:resourcetype/><d:displayname/>")?;
        let first = props
            .responses
            .first()
            .context("missing CardDAV discovery response")?;
        if let Some(book) = self.book(&base, first)? {
            return Ok(vec![book]);
        }
        let mut homes = home_urls(&base, first)?;
        if homes.is_empty() {
            let principal = first
                .prop(DAV, "current-user-principal")
                .and_then(|p| p.href())
                .or_else(|| first.prop(DAV, "principal-URL").and_then(|p| p.href()));
            if let Some(principal) = principal {
                let (base, ms) =
                    self.propfind(&base.join(principal)?, "0", "<c:addressbook-home-set/>")?;
                if let Some(row) = ms.responses.first() {
                    homes = home_urls(&base, row)?;
                }
            }
        }
        if homes.is_empty() {
            homes.push(base);
        }
        let mut books = BTreeMap::new();
        for home in homes {
            let (base, ms) = self.propfind(&home, "1", "<d:resourcetype/><d:displayname/>")?;
            for row in &ms.responses {
                if let Some(book) = self.book(&base, row)? {
                    books.insert(book.url.clone(), book);
                }
            }
        }
        Ok(books.into_values().collect())
    }

    fn book(&self, base: &Url, row: &crate::caldav::xml::Response) -> Result<Option<Book>> {
        if !row
            .prop(DAV, "resourcetype")
            .is_some_and(|p| p.child(CARDDAV, "addressbook").is_some())
        {
            return Ok(None);
        }
        let url = base.join(&row.href)?;
        self.check(&url)?;
        Ok(Some(Book {
            name: row
                .prop_text(DAV, "displayname")
                .unwrap_or("Address book")
                .to_string(),
            url: url.into(),
        }))
    }

    pub fn snapshot(
        &self,
        book: &Book,
        known: &BTreeMap<String, String>,
    ) -> Result<(BTreeMap<String, String>, Vec<Card>)> {
        let (base, ms) = self.propfind(
            &Url::parse(&book.url)?,
            "1",
            "<d:resourcetype/><d:getetag/>",
        )?;
        let mut versions = BTreeMap::new();
        let mut saw_book = false;
        for row in &ms.responses {
            let url = base.join(&row.href)?;
            self.check(&url)?;
            ensure!(
                row.status.is_none_or(|s| (200..300).contains(&s)),
                "incomplete CardDAV address book listing"
            );
            if same_path(&base, &url) {
                saw_book = row
                    .prop(DAV, "resourcetype")
                    .is_some_and(|p| p.child(CARDDAV, "addressbook").is_some());
                continue;
            }
            ensure!(
                member_of(&base, &url),
                "CardDAV member outside its address book"
            );
            if row
                .prop(DAV, "resourcetype")
                .is_some_and(|p| p.child(DAV, "collection").is_some())
            {
                continue;
            }
            let etag = row
                .prop_text(DAV, "getetag")
                .context("CardDAV member is missing its ETag")?;
            versions.insert(
                if self.google {
                    canonical_member(&url)
                } else {
                    url.to_string()
                },
                etag.to_string(),
            );
        }
        ensure!(
            saw_book,
            "CardDAV did not return the requested address book"
        );
        let changed: Vec<_> = versions
            .iter()
            .filter(|(url, etag)| known.get(*url) != Some(*etag))
            .map(|(url, _)| url.clone())
            .collect();
        let cards = self.fetch(&base, &changed, &mut versions)?;
        Ok((versions, cards))
    }

    fn fetch(
        &self,
        book: &Url,
        changed: &[String],
        versions: &mut BTreeMap<String, String>,
    ) -> Result<Vec<Card>> {
        let mut cards = Vec::new();
        for batch in changed.chunks(10) {
            let mut wanted = BTreeMap::new();
            let mut body = format!(
                r#"<c:addressbook-multiget xmlns:d="DAV:" xmlns:c="{CARDDAV}"><d:prop><d:getetag/><c:address-data/></d:prop>"#
            );
            for href in batch {
                let url = Url::parse(href)?;
                self.check(&url)?;
                ensure!(
                    member_of(book, &url),
                    "CardDAV member outside its address book"
                );
                body.push_str(&format!("<d:href>{}</d:href>", escape(url.path())));
                wanted.insert(path_key(&url), href);
            }
            body.push_str("</c:addressbook-multiget>");
            let (base, status, text) = self.request("REPORT", book.clone(), "1", &body)?;
            ensure!(
                status == 207 && same_path(book, &base),
                "CardDAV multiget failed (HTTP {status})"
            );
            for row in parse_multistatus(&text)?.responses {
                let url = base.join(&row.href)?;
                self.check(&url)?;
                ensure!(
                    member_of(book, &url),
                    "CardDAV member outside its address book"
                );
                let Some(href) = wanted.remove(&path_key(&url)) else {
                    continue;
                };
                if matches!(row.status, Some(404 | 410)) {
                    versions.remove(href.as_str());
                    continue;
                }
                ensure!(
                    row.status.is_none_or(|s| (200..300).contains(&s)),
                    "CardDAV returned an unsuccessful card response; retry sync"
                );
                let etag = row
                    .prop_text(DAV, "getetag")
                    .context("CardDAV card is missing its ETag; retry sync")?;
                let data = row
                    .prop_text(CARDDAV, "address-data")
                    .context("CardDAV card is missing its data; retry sync")?;
                cards.push(
                    Card::parse(href, etag, data).unwrap_or_else(|_| Card::invalid(href, etag)),
                );
            }
            ensure!(
                wanted.is_empty(),
                "CardDAV returned an incomplete card batch; retry sync"
            );
        }
        Ok(cards)
    }
}

fn write_etag(etag: &str) -> Option<String> {
    let quoted = etag.strip_prefix('"').and_then(|tag| tag.strip_suffix('"'));
    let tag = quoted.unwrap_or(etag);
    if quoted.is_none() && (tag.is_empty() || tag == "*" || tag.starts_with("W/")) {
        return None;
    }
    tag.bytes()
        .all(|b| matches!(b, 0x21 | 0x23..=0x7e))
        .then(|| format!("\"{tag}\""))
}

fn written(status: u16) -> Result<()> {
    match status {
        200 | 201 | 204 => Ok(()),
        404 | 410 | 412 => {
            bail!("This contact changed on the server. Sync the address book and try again.")
        }
        403 => bail!("The server does not allow this change to the address book."),
        _ => bail!(
            "The server did not confirm the contact change (HTTP {status}). Sync before retrying."
        ),
    }
}

fn home_urls(base: &Url, row: &crate::caldav::xml::Response) -> Result<Vec<Url>> {
    row.prop(CARDDAV, "addressbook-home-set")
        .into_iter()
        .flat_map(|p| p.children_named(DAV, "href"))
        .map(|href| Ok(base.join(href.text())?))
        .collect()
}

fn path_key(url: &Url) -> Vec<Vec<u8>> {
    url.path()
        .trim_end_matches('/')
        .split('/')
        .map(|s| percent_encoding::percent_decode_str(s).collect())
        .collect()
}

fn same_path(a: &Url, b: &Url) -> bool {
    a.origin() == b.origin() && path_key(a) == path_key(b)
}

fn member_of(book: &Url, member: &Url) -> bool {
    let parent = path_key(book);
    let child = path_key(member);
    book.origin() == member.origin()
        && child.len() == parent.len() + 1
        && child.starts_with(&parent)
}

// Google can spell the same href with a literal @ or %40. Normalize only
// escaped unreserved characters and the unambiguous path-segment @.
fn canonical_member(url: &Url) -> String {
    let mut url = url.clone();
    let path = path_key(&url)
        .iter()
        .map(|part| {
            use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};
            const SET: &AsciiSet = &NON_ALPHANUMERIC
                .remove(b'-')
                .remove(b'.')
                .remove(b'_')
                .remove(b'~')
                .remove(b'@');
            percent_encode(part, SET).to_string()
        })
        .collect::<Vec<_>>()
        .join("/");
    url.set_path(&path);
    url.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreachable_server_fails_once_with_the_reason() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let client = Client::basic(
            server_url(&format!("http://127.0.0.1:{port}/dav/")).unwrap(),
            "user1@mail.localhost",
            "fictional-password",
        )
        .unwrap();
        let Err(err) = client.discover() else {
            panic!("discovery reached a closed port");
        };
        assert!(err.is::<Unreachable>(), "{err:#}");
        assert!(
            err.to_string()
                .starts_with("Could not reach the CardDAV server"),
            "{err:#}"
        );
    }

    #[test]
    fn server_url_keeps_typed_http_and_defaults_to_https() {
        assert_eq!(
            server_url(" dav.example.test/books ").unwrap().as_str(),
            "https://dav.example.test/books"
        );
        assert_eq!(
            server_url("http://192.0.2.10:4080").unwrap().as_str(),
            "http://192.0.2.10:4080/"
        );
        for input in [
            "",
            "ftp://dav.example.test",
            "https://user:pw@dav.example.test",
            "https://dav.example.test/?q=1",
        ] {
            assert!(server_url(input).is_err(), "{input}");
        }
    }
    use std::{io::Write, net::TcpListener};

    fn fixture(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (Client, Book, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                tx.send(crate::test_support::read_http_request(&mut stream))
                    .unwrap();
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let book = Book {
            url: format!("{origin}/book/cora%40example.test/"),
            name: "Contacts".into(),
        };
        (
            Client::new(
                Url::parse(&origin).unwrap(),
                "Bearer test-token".into(),
                true,
            )
            .unwrap(),
            book,
            rx,
        )
    }

    #[test]
    fn google_creates_with_post_and_deletes_conditionally() {
        let (client, book, requests) =
            fixture(vec![("201 Created", ""), ("412 Precondition Failed", "")]);
        let vcard = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Cora Fixture\r\nEND:VCARD\r\n";
        client.create(&book.url, "test-contact", vcard).unwrap();
        let req = requests.recv().unwrap();
        assert!(req.starts_with("POST /book/cora%40example.test/ HTTP/"));
        assert!(
            req.to_ascii_lowercase()
                .contains("content-type: text/vcard")
        );
        assert!(req.ends_with(vcard));
        let member = format!("{}server-id.vcf", book.url);
        let err = client
            .change(&book.url, &member, "\"v1\"", |_| unreachable!(), true)
            .unwrap_err();
        assert!(err.to_string().contains("changed on the server"));
        let req = requests.recv().unwrap();
        assert!(req.starts_with("DELETE /book/cora%40example.test/server-id.vcf HTTP/"));
        assert!(req.to_ascii_lowercase().contains("if-match: \"v1\""));
    }

    #[test]
    fn snapshot_skips_collections_invalid_cards_and_confirmed_deletions() {
        let listing = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
          <d:response><d:href>/book/cora%40example.test/</d:href><d:propstat><d:prop><d:resourcetype><c:addressbook/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat><d:propstat><d:prop><d:getetag/></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response>
          <d:response><d:href>/book/cora%40example.test/sub/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
          <d:response><d:href>/book/cora%40example.test/bad.vcf</d:href><d:propstat><d:prop><d:getetag>bad</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
          <d:response><d:href>/book/cora%40example.test/gone.vcf</d:href><d:propstat><d:prop><d:getetag>gone</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
        </d:multistatus>"#;
        let fetched = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
          <d:response><d:href>/book/cora%40example.test/bad.vcf</d:href><d:propstat><d:prop><d:getetag>bad</d:getetag><c:address-data>not a vCard</c:address-data></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
          <d:response><d:href>/book/cora%40example.test/gone.vcf</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>
        </d:multistatus>"#;
        let (client, book, _requests) = fixture(vec![
            ("207 Multi-Status", listing),
            ("207 Multi-Status", fetched),
        ]);
        let (versions, cards) = client.snapshot(&book, &BTreeMap::new()).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(cards.len(), 1);
        assert!(cards[0].invalid);
        assert_eq!(versions.get(&cards[0].url), Some(&cards[0].etag));
    }

    #[test]
    fn update_uses_conditional_put_even_without_get_etag() {
        let (client, book, requests) =
            fixture(vec![("200 OK", "original"), ("204 No Content", "")]);
        let member = format!("{}contact.vcf", book.url);
        client
            .change(
                &book.url,
                &member,
                "v1",
                |original| {
                    assert_eq!(original, "original");
                    Ok("updated".into())
                },
                false,
            )
            .unwrap();
        assert!(requests.recv().unwrap().starts_with("GET "));
        let put = requests.recv().unwrap();
        assert!(put.starts_with("PUT "));
        assert!(put.to_ascii_lowercase().contains("if-match: \"v1\""));
        assert!(write_etag("W/\"v1\"").is_none());
        assert!(write_etag("*").is_none());
    }

    #[test]
    fn credentials_never_leave_the_server_origin() {
        let (client, _, _) = fixture(vec![]);
        assert!(
            client
                .check(&Url::parse("https://foreign.example.test/").unwrap())
                .is_err()
        );
    }
}

/// Google's own explanation of a refused request (a JSON `error.message`,
/// e.g. that the CardDAV API is not enabled for the app's Cloud project), as
/// `: …` to follow the status. Empty for any other body.
fn google_reason(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|body| {
            body["error"]["message"]
                .as_str()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|message| !message.is_empty())
        .map(|message| format!(": {message}"))
        .unwrap_or_default()
}

#[cfg(test)]
mod google_reason_tests {
    #[test]
    fn google_errors_carry_their_message() {
        let body = r#"{"error":{"code":403,"message":"Google Contacts CardDAV API has not been used in project 1 before or it is disabled.","status":"PERMISSION_DENIED"}}"#;
        assert_eq!(
            super::google_reason(body),
            ": Google Contacts CardDAV API has not been used in project 1 before or it is disabled."
        );
        assert_eq!(super::google_reason("<html>Forbidden</html>"), "");
    }
}
