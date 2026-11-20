//! CalDAV client (RFC 4791): find a user's calendars on a server, keep a local
//! copy of each in step with it, and write changes back.
//!
//! This is the protocol only — blocking HTTP through the app-wide proxy, and
//! the XML each request and answer is made of. What is stored, and how a
//! remote event becomes a row in `events` or `tasks`, is the sync's business.
//!
//! Discovery follows RFC 6764 and what servers actually do: the URL the user
//! typed may be the server root, `/.well-known/caldav`, the principal, or a
//! calendar home, so each is tried until a `calendar-home-set` turns up.
//! Nextcloud, Stalwart, Radicale, Fastmail and iCloud all answer one of these.

pub mod ics;
pub mod service;
pub mod store;
pub mod sync;
#[cfg(test)]
mod tests;
pub mod xml;

use std::fmt;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use ureq::RequestExt as _;
use url::Url;

use self::xml::{APPLE, CALDAV, CALSERVER, DAV, Multistatus, escape, parse_multistatus};
use crate::proxy::{self, ProxyConfig};

/// Providers whose CalDAV service hands each account a host of its own,
/// reached by redirect or by the hrefs discovery returns.
const PER_USER_HOST_PROVIDERS: &[&str] = &["icloud.com", "me.com", "google.com", "yahoo.com"];

/// Redirects followed per request. `.well-known` typically adds one hop and a
/// trailing-slash fix-up another.
const MAX_REDIRECTS: usize = 5;

/// A whole calendar fetched in one multiget can be large; cap what is read.
const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;

/// Hrefs per `calendar-multiget`. Servers cap the body they answer with, and a
/// failed batch is cheaper to retry when small.
pub const MULTIGET_BATCH: usize = 100;

/// The server refused the username or password (401). Kept as its own type so
/// callers can tell "fix your password" from "the server is down".
#[derive(Debug)]
pub struct AuthFailed;

impl fmt::Display for AuthFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CalDAV server rejected the username or password")
    }
}

impl std::error::Error for AuthFailed {}

/// The sync token handed to `sync-collection` is no longer valid (RFC 6578
/// §3.2): start over with a full listing.
#[derive(Debug)]
pub struct SyncTokenInvalid;

impl fmt::Display for SyncTokenInvalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CalDAV sync token expired")
    }
}

impl std::error::Error for SyncTokenInvalid {}

/// A write lost a race: the resource changed on the server since its etag was
/// read (412), or already exists when creating.
#[derive(Debug)]
pub struct PreconditionFailed;

impl fmt::Display for PreconditionFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CalDAV resource changed on the server")
    }
}

impl std::error::Error for PreconditionFailed {}

/// The server refused a new object because an object with its UID is
/// already in this calendar or another of the account's (RFC 4791 §5.3.2.1,
/// `no-uid-conflict`).
#[derive(Debug)]
pub struct UidConflict;

impl fmt::Display for UidConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CalDAV server already holds an event with this UID")
    }
}

impl std::error::Error for UidConflict {}

/// The server would not change a calendar's name or colour.
#[derive(Debug)]
pub struct PropertyRefused;

impl fmt::Display for PropertyRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the calendar server does not allow changing this calendar")
    }
}

impl std::error::Error for PropertyRefused {}

/// One calendar collection.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Calendar {
    pub url: String,
    pub name: String,
    /// `#RRGGBB` (Apple's `calendar-color`, alpha dropped), or empty.
    pub color: String,
    /// Upper-case component names it accepts: `VEVENT`, `VTODO`, … A server
    /// that does not say accepts everything, which is recorded as empty.
    pub components: Vec<String>,
    /// Changes whenever anything inside does (`getctag`, else the
    /// `sync-token`): a cheap "anything new?" before listing.
    pub ctag: String,
    pub sync_token: String,
    /// The user may only read it: a subscription, or a calendar shared
    /// read-only.
    pub read_only: bool,
}

impl Calendar {
    pub fn supports(&self, component: &str) -> bool {
        self.components.is_empty()
            || self
                .components
                .iter()
                .any(|c| c.eq_ignore_ascii_case(component))
    }
}

/// What the server holds for one calendar object: its URL and version.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub url: String,
    pub etag: String,
}

/// A calendar object with its iCalendar text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Object {
    pub url: String,
    pub etag: String,
    pub ics: String,
}

/// The answer to [`Client::fetch`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fetched {
    pub objects: Vec<Object>,
    /// Asked for, but no longer on the server.
    pub gone: Vec<String>,
}

/// What changed in a collection since a sync token.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    pub changed: Vec<Entry>,
    pub removed: Vec<String>,
    pub sync_token: String,
}

#[derive(Debug, Default)]
struct ChangesPage {
    changed: Vec<Entry>,
    removed: Vec<String>,
    sync_token: String,
    truncated: bool,
}

/// `sync-collection` pages followed before giving up on a server that keeps
/// saying there is more.
const MAX_SYNC_PAGES: usize = 1000;

/// Where discovery ended up.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Discovery {
    pub principal: String,
    pub home: String,
    pub calendars: Vec<Calendar>,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    /// The URL that answered, after redirects: relative hrefs resolve here.
    url: Url,
    etag: String,
    location: String,
    body: String,
}

/// The resource accepted by the server. Google can choose its URL on creation.
#[derive(Debug)]
pub struct Written {
    pub url: String,
    pub etag: String,
}

pub struct Client {
    agent: ureq::Agent,
    authorization: String,
    google: bool,
    /// The server the user named. Requests go only where it is trusted to
    /// send them (see [`Client::trusts`]), and the password only there.
    server: Url,
}

impl Client {
    pub fn new(
        server: &str,
        username: &str,
        password: &str,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Self> {
        let agent = proxy::agent_for(proxy)?;
        let token =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        Ok(Self {
            agent,
            authorization: format!("Basic {token}"),
            google: false,
            server: parse_server_url(server)?,
        })
    }

    pub fn google(email: &str, access_token: &str) -> Result<Self> {
        let mut server = Url::parse("https://apidata.googleusercontent.com/caldav/v2/")?;
        server
            .path_segments_mut()
            .map_err(|_| anyhow!("invalid Google CalDAV endpoint"))?
            .pop_if_empty()
            .push(email)
            .push("events");
        Ok(Self {
            agent: proxy::agent_for(proxy::global().as_ref())?,
            authorization: format!("Bearer {access_token}"),
            google: true,
            server,
        })
    }

    pub fn primary_calendar(&self) -> Result<Calendar> {
        let (base, ms) = self.calendar_properties(&self.server, "0")?;
        let mut calendars = calendars_in(&base, &ms)?;
        let mut calendar = calendars
            .pop()
            .ok_or_else(|| anyhow!("Google primary calendar not found"))?;
        if !calendars.is_empty() {
            bail!("Google returned multiple calendars for the primary calendar");
        }
        calendar.components = vec!["VEVENT".into()];
        Ok(calendar)
    }

    /// Whether `url` belongs to the server the user named: its own origin,
    /// or that origin upgraded to https on the default port. Providers known
    /// to move each user to a host of their own (iCloud's `p42-caldav`) are
    /// followed to other https hosts of theirs. Anything else, discovery's
    /// hrefs and redirects included, is refused rather than sent the
    /// password or an event: two hosts of one organisation need not share
    /// who may see them.
    fn trusts(&self, url: &Url) -> bool {
        if url.origin() == self.server.origin() {
            return true;
        }
        if url.scheme() != "https" {
            return false;
        }
        let (Some(host), Some(server_host)) = (url.host_str(), self.server.host_str()) else {
            return false;
        };
        let host = host.to_ascii_lowercase();
        let server_host = server_host.to_ascii_lowercase();
        if host == server_host {
            return self.server.scheme() == "http" && url.port().is_none();
        }
        self.server.scheme() == "https"
            && url.port().is_none()
            && PER_USER_HOST_PROVIDERS.iter().any(|provider| {
                let under = |h: &str| h == *provider || h.ends_with(&format!(".{provider}"));
                under(&host) && under(&server_host)
            })
    }

    /// Send one request, following redirects with the same method and body.
    /// ureq would turn a redirected PROPFIND into a GET and drop the body.
    fn send(
        &self,
        method: &str,
        url: &Url,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<HttpResponse> {
        if self.google && !matches!(method, "GET" | "PROPFIND" | "REPORT") {
            if !matches!(method, "PUT" | "DELETE") {
                bail!("Google Calendar supports changes to events here");
            }
            self.check_google_event_url(url)?;
        }
        let mut url = url.clone();
        // A sync-collection or multiget answer can be a whole calendar.
        let timeout = if method == "REPORT" { 120 } else { 30 };
        for _ in 0..=MAX_REDIRECTS {
            if !self.trusts(&url) {
                bail!(
                    "the calendar server pointed to {}, which is not {}; not following it",
                    redacted(&url),
                    self.server.host_str().unwrap_or_default()
                );
            }
            let mut builder = ureq::http::Request::builder()
                .method(method)
                .uri(url.as_str())
                .header("Authorization", &self.authorization);
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            if body.is_some() {
                let content_type = if method == "PUT" {
                    "text/calendar; charset=utf-8"
                } else {
                    "application/xml; charset=utf-8"
                };
                builder = builder.header("Content-Type", content_type);
            }
            let request = builder
                .body(body.unwrap_or_default().as_bytes().to_vec())
                .context("build CalDAV request")?;
            let mut response = request
                .with_agent(&self.agent)
                .configure()
                .allow_non_standard_methods(true)
                .http_status_as_error(false)
                .max_redirects(0)
                .timeout_global(Some(Duration::from_secs(timeout)))
                .run()
                .with_context(|| format!("{method} {}", redacted(&url)))?;
            let status = response.status().as_u16();
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_string()
            };
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                // A 303 points at a result to read, not at somewhere to send
                // the same write again; and a write is only ever sent where
                // the user's action named, never to wherever a redirect says.
                let reads = matches!(method, "GET" | "PROPFIND" | "REPORT");
                if !reads || status == 303 && method != "GET" {
                    bail!(
                        "{method} {} was redirected; not repeating it elsewhere",
                        redacted(&url)
                    );
                }
                let location = header("location");
                if location.is_empty() {
                    bail!("{method} {}: redirect without a Location", redacted(&url));
                }
                url = url
                    .join(&location)
                    .with_context(|| format!("bad redirect to {location}"))?;
                continue;
            }
            let etag = header("etag");
            let location = header("location");
            let body = response
                .body_mut()
                .with_config()
                .limit(MAX_BODY_BYTES)
                .read_to_string()
                .with_context(|| format!("read {method} {}", redacted(&url)))?;
            if status == 401 {
                if self.google {
                    bail!(
                        "Google Calendar rejected the authorization. Reconnect Google Calendar to resume syncing."
                    );
                }
                return Err(AuthFailed.into());
            }
            return Ok(HttpResponse {
                status,
                url,
                etag,
                location,
                body,
            });
        }
        bail!("{method} {}: too many redirects", redacted(&url))
    }

    fn multistatus(
        &self,
        method: &str,
        url: &Url,
        depth: &str,
        body: &str,
    ) -> Result<(Url, Multistatus)> {
        let response = self.send(method, url, &[("Depth", depth)], Some(body))?;
        if response.status != 207 {
            bail!(
                "{method} {} answered {}",
                redacted(&response.url),
                response.status
            );
        }
        let parsed = parse_multistatus(&response.body)
            .with_context(|| format!("{method} {}", redacted(&response.url)))?;
        Ok((response.url, parsed))
    }

    /// Find the calendars the account can see, starting from whatever URL
    /// the user entered.
    pub fn discover(&self) -> Result<Discovery> {
        let start = self.server.clone();
        let mut candidates = vec![start.clone()];
        let well_known = start.join("/.well-known/caldav")?;
        if well_known != start {
            candidates.push(well_known);
        }
        // Some servers answer only at the root (or redirect from it).
        let root = start.join("/")?;
        if !candidates.contains(&root) {
            candidates.push(root);
        }

        let mut last_error = None;
        for candidate in candidates {
            match self.find_home(&candidate) {
                Ok(Some((principal, home))) => {
                    let calendars = self.calendars(home.as_str())?;
                    return Ok(Discovery {
                        principal: principal.to_string(),
                        home: home.to_string(),
                        calendars,
                    });
                }
                Ok(None) => {}
                // A wrong password is the answer, not a reason to keep probing.
                Err(err) if err.is::<AuthFailed>() => return Err(err),
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow!("no CalDAV calendars found at {}", self.server)))
    }

    /// The principal and calendar home reachable from `url`, if it leads to
    /// one.
    fn find_home(&self, url: &Url) -> Result<Option<(Url, Url)>> {
        let body = propfind_body(&[
            (DAV, "current-user-principal"),
            (CALDAV, "calendar-home-set"),
        ]);
        let (base, ms) = self.multistatus("PROPFIND", url, "0", &body)?;
        let Some(response) = ms.responses.first() else {
            return Ok(None);
        };
        let principal = match response
            .prop(DAV, "current-user-principal")
            .and_then(|p| p.href())
        {
            Some(href) => base.join(href)?,
            None => base.clone(),
        };
        // The URL itself may already be the principal, which names its home.
        if let Some(home) = response
            .prop(CALDAV, "calendar-home-set")
            .and_then(|p| p.href())
        {
            return Ok(Some((principal, base.join(home)?)));
        }
        if principal == base {
            return Ok(None);
        }
        let body = propfind_body(&[(CALDAV, "calendar-home-set")]);
        let (base, ms) = self.multistatus("PROPFIND", &principal, "0", &body)?;
        let home = ms
            .responses
            .first()
            .and_then(|r| r.prop(CALDAV, "calendar-home-set"))
            .and_then(|p| p.href())
            .map(|href| base.join(href))
            .transpose()?;
        Ok(home.map(|home| (principal, home)))
    }

    /// The calendars directly inside a calendar home.
    pub fn calendars(&self, home: &str) -> Result<Vec<Calendar>> {
        let (base, ms) = self.calendar_properties(&Url::parse(home)?, "1")?;
        calendars_in(&base, &ms)
    }

    fn calendar_properties(&self, url: &Url, depth: &str) -> Result<(Url, Multistatus)> {
        let body = propfind_body(&[
            (DAV, "resourcetype"),
            (DAV, "displayname"),
            (DAV, "sync-token"),
            (DAV, "current-user-privilege-set"),
            (CALSERVER, "getctag"),
            (APPLE, "calendar-color"),
            (CALDAV, "supported-calendar-component-set"),
        ]);
        self.multistatus("PROPFIND", url, depth, &body)
    }

    /// Every object in a calendar with its etag, for servers without
    /// `sync-collection` and after a sync token expires.
    pub fn list(&self, calendar: &str) -> Result<Vec<Entry>> {
        let url = Url::parse(calendar)?;
        let body = propfind_body(&[(DAV, "getetag"), (DAV, "resourcetype")]);
        let (base, ms) = self.multistatus("PROPFIND", &url, "1", &body)?;
        let mut entries = Vec::new();
        for response in &ms.responses {
            let member = base.join(&response.href)?;
            if same_resource(&member, &base) {
                continue;
            }
            let is_collection = response
                .prop(DAV, "resourcetype")
                .is_some_and(|t| t.child(DAV, "collection").is_some());
            if is_collection {
                continue;
            }
            // A member without a version is one the server failed to describe:
            // the listing is incomplete, and must not be read as the member
            // being gone.
            let Some(etag) = response.prop_text(DAV, "getetag") else {
                bail!(
                    "{} listed {} without a version; listing again next sync",
                    redacted(&base),
                    redacted(&member)
                );
            };
            entries.push(Entry {
                url: member.to_string(),
                etag: etag.to_string(),
            });
        }
        Ok(entries)
    }

    /// What changed since `token` (RFC 6578). An empty token asks for
    /// everything. Fails with [`SyncTokenInvalid`] when the server has
    /// forgotten the token.
    pub fn changes(&self, calendar: &str, token: &str) -> Result<Changes> {
        let url = Url::parse(calendar)?;
        let mut changes = Changes {
            sync_token: token.to_string(),
            ..Default::default()
        };
        // A server may answer in pages, marking all but the last truncated
        // (507 on the collection itself) and handing out a token to go on.
        for _ in 0..MAX_SYNC_PAGES {
            let page = self.changes_page(&url, &changes.sync_token)?;
            for entry in page.changed {
                changes.removed.retain(|removed| *removed != entry.url);
                changes.changed.retain(|known| known.url != entry.url);
                changes.changed.push(entry);
            }
            for removed in page.removed {
                changes.changed.retain(|known| known.url != removed);
                if !changes.removed.contains(&removed) {
                    changes.removed.push(removed);
                }
            }
            let advanced = page.sync_token != changes.sync_token;
            changes.sync_token = page.sync_token;
            if !page.truncated {
                return Ok(changes);
            }
            if !advanced {
                bail!(
                    "sync-collection on {} stopped making progress",
                    redacted(&url)
                );
            }
        }
        bail!("sync-collection on {} has too many pages", redacted(&url))
    }

    fn changes_page(&self, url: &Url, token: &str) -> Result<ChangesPage> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<d:sync-collection xmlns:d="DAV:"><d:sync-token>{}</d:sync-token><d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>"#,
            escape(token)
        );
        // RFC 6578 §3.2: the members to report are chosen by sync-level, and
        // the request itself is Depth 0.
        let response = self.send("REPORT", url, &[("Depth", "0")], Some(&body))?;
        if response.status != 207 {
            if matches!(response.status, 403 | 409) && response.body.contains("valid-sync-token") {
                return Err(SyncTokenInvalid.into());
            }
            bail!(
                "sync-collection on {} answered {}",
                redacted(&response.url),
                response.status
            );
        }
        let ms = parse_multistatus(&response.body)?;
        let mut page = ChangesPage {
            sync_token: ms.sync_token.unwrap_or_default(),
            ..Default::default()
        };
        for item in &ms.responses {
            let member = response.url.join(&item.href)?;
            if same_resource(&member, &response.url) {
                page.truncated |= item.status == Some(507);
                continue;
            }
            if item.status == Some(404) {
                page.removed.push(member.to_string());
            } else if let Some(etag) = item.prop_text(DAV, "getetag") {
                page.changed.push(Entry {
                    url: member.to_string(),
                    etag: etag.to_string(),
                });
            }
        }
        Ok(page)
    }

    /// Fetch objects by URL, at most [`MULTIGET_BATCH`] at a time. Objects
    /// the server no longer has are listed in [`Fetched::gone`].
    pub fn fetch(&self, calendar: &str, urls: &[String]) -> Result<Fetched> {
        let calendar = Url::parse(calendar)?;
        let mut objects = Vec::new();
        for batch in urls.chunks(MULTIGET_BATCH) {
            let mut body = String::from(
                r#"<?xml version="1.0" encoding="utf-8"?>
<c:calendar-multiget xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><c:calendar-data/></d:prop>"#,
            );
            for url in batch {
                // Hrefs go to the server as paths, the way it hands them out.
                let path = Url::parse(url).map(|u| u.path().to_string())?;
                body.push_str(&format!("<d:href>{}</d:href>", escape(&path)));
            }
            body.push_str("</c:calendar-multiget>");
            let (base, ms) = self.multistatus("REPORT", &calendar, "1", &body)?;
            for response in &ms.responses {
                let (Some(etag), Some(ics)) = (
                    response.prop_text(DAV, "getetag"),
                    response.prop_text(CALDAV, "calendar-data"),
                ) else {
                    continue;
                };
                let answered = base.join(&response.href)?;
                // Keyed by the URL asked for, however the server spells it.
                let url = batch
                    .iter()
                    .find(|asked| Url::parse(asked).is_ok_and(|a| same_member(&a, &answered)))
                    .cloned()
                    .unwrap_or_else(|| answered.to_string());
                objects.push(Object {
                    url,
                    etag: etag.to_string(),
                    ics: ics.to_string(),
                });
            }
        }
        // Whatever the multiget left out is asked for alone: a sync must not
        // move past an object it never saw.
        let mut fetched = Fetched::default();
        for url in urls {
            if objects.iter().any(|o| o.url == *url) {
                continue;
            }
            match self.get(url)? {
                Some(object) => objects.push(Object {
                    url: url.clone(),
                    ..object
                }),
                None => fetched.gone.push(url.clone()),
            }
        }
        fetched.objects = objects;
        Ok(fetched)
    }

    /// Fetch one object directly.
    pub fn get(&self, url: &str) -> Result<Option<Object>> {
        let url = Url::parse(url)?;
        let response = self.send("GET", &url, &[], None)?;
        match response.status {
            200 => Ok(Some(Object {
                url: response.url.to_string(),
                etag: response.etag,
                ics: response.body,
            })),
            404 | 410 => Ok(None),
            status => bail!("GET {} answered {status}", redacted(&response.url)),
        }
    }

    /// Create (`etag` `None`) or replace (`Some(etag)`) an object.
    pub fn put(&self, url: &str, ics: &str, etag: Option<&str>) -> Result<Written> {
        let url = Url::parse(url)?;
        let mut headers = Vec::new();
        let mut create_uid = None;
        if let Some(etag) = etag {
            headers.push(("If-Match", etag));
        } else if self.google {
            self.check_google_event_url(&url)?;
            // Google implements If-Match but not If-None-Match. Check the
            // requested URL and the UID before using an unconditional PUT.
            // A PUT whose response was lost may already exist at another URL.
            let uid =
                ics::uid_of(ics).ok_or_else(|| anyhow!("Google Calendar event has no UID"))?;
            if let Some(object) = self.get(url.as_str())? {
                if ics::uid_of(&object.ics).as_deref() != Some(uid.as_str()) {
                    bail!("Google Calendar already has a different event at this URL");
                }
                let url = Url::parse(&object.url)?;
                self.check_google_event_url(&url)?;
                return Ok(Written {
                    url: url.to_string(),
                    etag: String::new(),
                });
            }
            if let Some(url) = self.google_event_by_uid(&uid)? {
                // Let the pull fetch the authoritative version at this URL.
                return Ok(Written {
                    url: url.to_string(),
                    etag: String::new(),
                });
            }
            create_uid = Some(uid);
        } else {
            headers.push(("If-None-Match", "*"));
        }
        let response = self.send("PUT", &url, &headers, Some(ics))?;
        match response.status {
            200..=299 => {
                let url = if self.google && !response.location.is_empty() {
                    let url = response.url.join(&response.location)?;
                    self.check_google_event_url(&url)?;
                    url
                } else if let Some(uid) = create_uid {
                    self.google_event_by_uid(&uid)?.unwrap_or(response.url)
                } else {
                    response.url
                };
                Ok(Written {
                    url: url.to_string(),
                    etag: response.etag,
                })
            }
            412 => Err(PreconditionFailed.into()),
            403 | 409 if response.body.contains("no-uid-conflict") => Err(UidConflict.into()),
            status => bail!(
                "PUT {} answered {status}: {}",
                redacted(&response.url),
                response.body.trim()
            ),
        }
    }

    fn check_google_event_url(&self, url: &Url) -> Result<()> {
        let parent = url.join(".")?;
        if !same_member(&parent, &as_collection(self.server.clone()))
            || !url
                .path_segments()
                .and_then(Iterator::last)
                .is_some_and(|name| !name.is_empty())
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            bail!("Google Calendar returned a URL outside the primary calendar's events");
        }
        Ok(())
    }

    fn google_event_by_uid(&self, uid: &str) -> Result<Option<Url>> {
        let body = format!(
            r#"<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
<d:prop><c:calendar-data/></d:prop><c:filter><c:comp-filter name="VCALENDAR">
<c:comp-filter name="VEVENT"><c:prop-filter name="UID"><c:text-match>{}</c:text-match>
</c:prop-filter></c:comp-filter></c:comp-filter></c:filter></c:calendar-query>"#,
            escape(uid)
        );
        let (base, ms) = self.multistatus("REPORT", &self.server, "1", &body)?;
        let mut matches = ms.responses.iter().filter(|response| {
            response
                .prop_text(CALDAV, "calendar-data")
                .and_then(ics::uid_of)
                .as_deref()
                == Some(uid)
        });
        let Some(response) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            bail!("Google Calendar returned multiple events with the same UID");
        }
        let url = base.join(&response.href)?;
        self.check_google_event_url(&url)?;
        Ok(Some(url))
    }

    /// Create an event calendar in `home`. Returns its URL.
    pub fn make_calendar(&self, home: &str, name: &str, color: &str) -> Result<String> {
        // Inside the home, whether or not its URL ends in a slash.
        let url = as_collection(Url::parse(home)?).join(&format!("{}/", uuid::Uuid::new_v4()))?;
        let color = match color {
            "" => String::new(),
            color => format!("<ic:calendar-color>{}</ic:calendar-color>", escape(color)),
        };
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<c:mkcalendar xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:ic="http://apple.com/ns/ical/"><d:set><d:prop><d:displayname>{}</d:displayname>{color}<c:supported-calendar-component-set><c:comp name="VEVENT"/></c:supported-calendar-component-set></d:prop></d:set></c:mkcalendar>"#,
            escape(name)
        );
        let response = self.send("MKCALENDAR", &url, &[], Some(&body))?;
        match response.status {
            200..=299 => Ok(response.url.to_string()),
            403 | 405 => bail!("this server does not let calendars be created here"),
            status => bail!(
                "MKCALENDAR {} answered {status}: {}",
                redacted(&response.url),
                response.body.trim()
            ),
        }
    }

    /// Rename or recolour a calendar; `None` leaves that property alone.
    /// Fails with [`PropertyRefused`] when the server keeps the calendar's
    /// properties to itself (a shared or subscribed calendar).
    pub fn update_calendar(
        &self,
        calendar: &str,
        name: Option<&str>,
        color: Option<&str>,
    ) -> Result<()> {
        let url = Url::parse(calendar)?;
        let mut props = String::new();
        if let Some(name) = name {
            props.push_str(&format!("<d:displayname>{}</d:displayname>", escape(name)));
        }
        if let Some(color) = color {
            props.push_str(&format!(
                "<ic:calendar-color>{}</ic:calendar-color>",
                escape(color)
            ));
        }
        if props.is_empty() {
            return Ok(());
        }
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<d:propertyupdate xmlns:d="DAV:" xmlns:ic="http://apple.com/ns/ical/"><d:set><d:prop>{props}</d:prop></d:set></d:propertyupdate>"#
        );
        let response = self.send("PROPPATCH", &url, &[], Some(&body))?;
        match response.status {
            // Each property has its own status inside; any failure fails all
            // (RFC 4918 §9.2: the update is atomic).
            207 => {
                let ms = parse_multistatus(&response.body)?;
                let refused = ms.responses.iter().any(|r| {
                    r.props.is_empty() || r.status.is_some_and(|s| !(200..300).contains(&s))
                });
                if refused {
                    Err(PropertyRefused.into())
                } else {
                    Ok(())
                }
            }
            200..=299 => Ok(()),
            401 => Err(AuthFailed.into()),
            403 | 405 => Err(PropertyRefused.into()),
            status => bail!("PROPPATCH {} answered {status}", redacted(&response.url)),
        }
    }

    /// Delete a whole calendar, and every event in it, on the server.
    /// Gone already counts as deleted.
    pub fn delete_calendar(&self, calendar: &str) -> Result<()> {
        let url = Url::parse(calendar)?;
        let response = self.send("DELETE", &url, &[], None)?;
        match response.status {
            // Some members could not be deleted (RFC 4918 §9.6.1): the calendar
            // is still there, with whatever was left in it.
            207 => {
                let ms = parse_multistatus(&response.body)?;
                match ms.responses.iter().find(|r| {
                    r.status
                        .is_some_and(|s| !(200..300).contains(&s) && s != 404)
                }) {
                    Some(failed) => bail!(
                        "the server could not delete everything in this calendar ({} answered {})",
                        failed.href,
                        failed.status.unwrap_or_default()
                    ),
                    None => Ok(()),
                }
            }
            200..=299 | 404 | 410 => Ok(()),
            403 | 405 => Err(PropertyRefused.into()),
            status => bail!("DELETE {} answered {status}", redacted(&response.url)),
        }
    }

    /// Delete an object. Gone already counts as deleted.
    pub fn delete(&self, url: &str, etag: Option<&str>) -> Result<()> {
        if self.google && etag.is_none_or(str::is_empty) {
            bail!("Google Calendar did not return an event version for deletion");
        }
        let url = Url::parse(url)?;
        let headers: Vec<(&str, &str)> = etag.map(|e| ("If-Match", e)).into_iter().collect();
        let response = self.send("DELETE", &url, &headers, None)?;
        match response.status {
            200..=299 | 404 | 410 => Ok(()),
            412 => Err(PreconditionFailed.into()),
            status => bail!("DELETE {} answered {status}", redacted(&response.url)),
        }
    }
}

/// The calendar collections in a Depth 1 PROPFIND of a calendar home.
fn calendars_in(base: &Url, ms: &Multistatus) -> Result<Vec<Calendar>> {
    let mut calendars = Vec::new();
    for response in &ms.responses {
        let is_calendar = response
            .prop(DAV, "resourcetype")
            .is_some_and(|t| t.child(CALDAV, "calendar").is_some());
        if !is_calendar {
            continue;
        }
        let url = base.join(&response.href)?;
        let components = response
            .prop(CALDAV, "supported-calendar-component-set")
            .map(|set| {
                set.children_named(CALDAV, "comp")
                    .filter_map(|c| c.attr("name"))
                    .map(str::to_ascii_uppercase)
                    .collect()
            })
            .unwrap_or_default();
        // Without a privilege set, assume writable and let a 403 say
        // otherwise.
        let read_only = response
            .prop(DAV, "current-user-privilege-set")
            .is_some_and(|set| !can_write(set));
        let sync_token = response
            .prop_text(DAV, "sync-token")
            .unwrap_or_default()
            .to_string();
        let name = response
            .prop_text(DAV, "displayname")
            .map(str::to_string)
            .unwrap_or_else(|| last_segment(&url));
        calendars.push(Calendar {
            ctag: response
                .prop_text(CALSERVER, "getctag")
                .map(str::to_string)
                .unwrap_or_else(|| sync_token.clone()),
            color: response
                .prop_text(APPLE, "calendar-color")
                .map(normalize_color)
                .unwrap_or_default(),
            url: url.to_string(),
            name,
            components,
            sync_token,
            read_only,
        });
    }
    Ok(calendars)
}

/// Make `event`, about to be saved, a new object in `calendar`, to be sent on
/// the next sync.
pub fn queue_new_event(
    conn: &rusqlite::Connection,
    calendar: &store::DavCalendar,
    event: &mut crate::store::CalendarEvent,
) -> Result<()> {
    let uid = uuid::Uuid::new_v4().to_string();
    event.calendar = calendar.id.clone();
    event.ics = ics::new_ics(event, &uid);
    store::save_object(
        conn,
        &store::DavObject {
            calendar: calendar.id.clone(),
            url: object_url(&calendar.url, &uid)?,
            etag: String::new(),
            local_id: event.id.clone(),
            state: store::CREATED.into(),
            moved_to: String::new(),
        },
    )
}

/// An undo of a deletion: send the event again. Still pending deletion, it
/// is simply rewritten; already deleted on the server, it is created anew.
pub fn requeue_restored_event(
    conn: &rusqlite::Connection,
    calendar: &store::DavCalendar,
    event: &mut crate::store::CalendarEvent,
) -> Result<()> {
    if store::object_for_event(conn, &event.id)?.is_some() {
        return store::mark_changed(conn, &event.id);
    }
    let uid = ics::uid_of(&event.ics).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if event.ics.is_empty() {
        event.ics = ics::new_ics(event, &uid);
    }
    let url = object_url(&calendar.url, &uid)?;
    // Back where it was, before its deletion there was sent: that object is
    // updated, at the version it had, rather than created over.
    if let Some(left) =
        store::object(conn, &calendar.id, &url)?.filter(|o| o.state == store::DELETED)
    {
        return store::save_object(
            conn,
            &store::DavObject {
                local_id: event.id.clone(),
                state: store::CHANGED.into(),
                moved_to: String::new(),
                ..left
            },
        );
    }
    store::save_object(
        conn,
        &store::DavObject {
            calendar: calendar.id.clone(),
            url,
            etag: String::new(),
            local_id: event.id.clone(),
            state: store::CREATED.into(),
            moved_to: String::new(),
        },
    )
}

/// `url` as a collection, ending in a slash, so a relative join lands
/// inside it rather than beside it.
fn as_collection(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url
}

/// Where a new object with `uid` goes: named after it, as clients do, with
/// anything unsafe in a path segment encoded.
fn object_url(calendar: &str, uid: &str) -> Result<String> {
    let url = as_collection(Url::parse(calendar)?);
    let segment: String = percent_encoding::utf8_percent_encode(
        &format!("{uid}.ics"),
        percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string()
    .replace("%2E", ".")
    .replace("%2D", "-")
    .replace("%5F", "_");
    Ok(url.join(&segment)?.to_string())
}

/// Accept `host`, `host/path` or a full URL; plain `http` only when the user
/// typed it.
pub fn parse_server_url(server: &str) -> Result<Url> {
    let server = server.trim();
    if server.is_empty() {
        bail!("CalDAV server URL is empty");
    }
    let with_scheme = if server.contains("://") {
        server.to_string()
    } else {
        format!("https://{server}")
    };
    let url = Url::parse(&with_scheme).with_context(|| format!("invalid URL {server}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("CalDAV server must be an http(s) URL: {server}");
    }
    Ok(url)
}

/// A URL for error messages: never with credentials in it.
fn redacted(url: &Url) -> String {
    let mut url = url.clone();
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.to_string()
}

/// Two URLs for the same member, whether or not they encode its path alike
/// (`alice%40x` and `alice@x`).
fn same_member(a: &Url, b: &Url) -> bool {
    let path = |u: &Url| {
        percent_encoding::percent_decode_str(u.path())
            .decode_utf8_lossy()
            .into_owned()
    };
    a.origin() == b.origin() && path(a) == path(b)
}

/// The member is the collection itself (servers list it first in Depth 1
/// answers), whether or not either spells the trailing slash.
fn same_resource(a: &Url, b: &Url) -> bool {
    a.origin() == b.origin() && a.path().trim_end_matches('/') == b.path().trim_end_matches('/')
}

fn last_segment(url: &Url) -> String {
    let segment = url
        .path()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    percent_encoding::percent_decode_str(segment)
        .decode_utf8_lossy()
        .into_owned()
}

/// `#RRGGBBAA` → `#RRGGBB`; anything else unchanged.
fn normalize_color(color: &str) -> String {
    let color = color.trim();
    if color.len() == 9 && color.starts_with('#') {
        color[..7].to_string()
    } else {
        color.to_string()
    }
}

fn can_write(privileges: &xml::Element) -> bool {
    privileges.children_named(DAV, "privilege").any(|p| {
        ["all", "write", "write-content", "bind"]
            .iter()
            .any(|name| p.child(DAV, name).is_some())
    })
}

fn propfind_body(props: &[(&str, &str)]) -> String {
    let mut prefixes = Vec::new();
    let mut body = String::new();
    for (ns, name) in props {
        let prefix = match *ns {
            DAV => "d",
            CALDAV => "c",
            CALSERVER => "cs",
            APPLE => "ic",
            _ => "x",
        };
        if prefix != "d" && !prefixes.contains(&(prefix, *ns)) {
            prefixes.push((prefix, *ns));
        }
        body.push_str(&format!("<{prefix}:{name}/>"));
    }
    let xmlns: String = prefixes
        .iter()
        .map(|(prefix, ns)| format!(r#" xmlns:{prefix}="{ns}""#))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"{xmlns}><d:prop>{body}</d:prop></d:propfind>"#
    )
}
