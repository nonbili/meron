//! IMAP read path: TLS connect + login (async-imap over tokio-rustls), folder
//! listing, recent-message envelopes, and full message reads.
//!
//! `connect` performs a fresh TLS + auth handshake. The request path does not
//! call it directly per operation — `Engine::with_session` (in `main.rs`) pools
//! warm authenticated sessions and reuses them, falling back to `connect` only
//! when no live session is available. IMAP IDLE watchers hold their own
//! dedicated long-lived connections, separate from that pool.

use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use mailparse::MailHeaderMap;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::parse;

/// Connection stream: implicit TLS (port 993 etc.) or plaintext (e.g. a local
/// test server on 3143). One enum so the async-imap `Session` type is uniform.
/// STARTTLS upgrade is not yet supported.
#[derive(Debug)]
pub enum Stream {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_flush(cx),
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            Stream::Tls(s) => Pin::new(s.as_mut()).poll_shutdown(cx),
        }
    }
}

pub type Session = async_imap::Session<Stream>;

#[derive(Clone)]
pub struct Creds {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    /// Implicit TLS on connect. False = plaintext (no encryption).
    pub tls: bool,
    /// Upgrade a plaintext IMAP connection to TLS via STARTTLS after the
    /// greeting. Takes precedence over `tls`: when true we connect plaintext
    /// and issue STARTTLS rather than wrapping the socket in implicit TLS.
    pub starttls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: bool,
    /// STARTTLS for the SMTP submission connection (typically port 587).
    pub smtp_starttls: bool,
    pub auth_type: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: i64,
    pub oauth_client_id: String,
    pub oauth_client_secret: String,
    pub oauth_token_url: String,
    pub oauth_scope: String,
}

impl Creds {
    /// Whether this account authenticates via OAuth2 (XOAUTH2) rather than a
    /// password. Covers every provider — `gmail_oauth`, `outlook_oauth`, … — so
    /// new providers don't need to be enumerated at each auth site.
    pub fn is_oauth(&self) -> bool {
        self.auth_type.ends_with("_oauth")
    }
}

#[derive(serde::Serialize, Default)]
pub struct Folder {
    pub name: String,
    pub delimiter: Option<String>,
    /// Count of unseen messages cached for this folder. Populated by
    /// `store::get_folders`; the IMAP LIST path leaves it at 0.
    #[serde(default)]
    pub unread: u32,
    /// RFC 6154 special-use role ("drafts", "sent", "trash", "junk",
    /// "archive", "all") as advertised by LIST, when the server supports the
    /// extension. Cached in the store so role lookups (e.g. which folder holds
    /// drafts) don't depend on name heuristics alone.
    #[serde(default)]
    pub special_use: Option<String>,
}

/// One addressee parsed from an envelope `To`/`Cc` list.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct Recipient {
    pub name: String,
    pub addr: String,
}

#[derive(serde::Serialize, Default, Clone)]
pub struct MessageHeader {
    pub uid: u32,
    /// IMAP folder this UID lives in. Populated only by cross-folder queries
    /// (thread views that merge Inbox + Sent); single-folder paths leave it
    /// empty and the caller supplies the folder out-of-band.
    #[serde(default)]
    pub folder: String,
    pub subject: String,
    pub from_name: String,
    pub from_addr: String,
    /// Send time as Unix epoch seconds (0 when unknown). Formatted for display
    /// in local time by the frontend.
    pub date: i64,
    pub seen: bool,
    pub starred: bool,
    pub thread_key: String,
    /// Normalized RFC Message-ID from the envelope, when available.
    #[serde(default)]
    pub message_id: String,
    /// Normalized RFC In-Reply-To from the envelope, when available.
    #[serde(default)]
    pub in_reply_to: String,
    /// Envelope `To`/`Cc` addressees. Populated by the envelope-fetch paths and
    /// persisted for recipient autocomplete; the cached-row SELECT paths that
    /// don't need them leave these empty.
    #[serde(default)]
    pub to: Vec<Recipient>,
    #[serde(default)]
    pub cc: Vec<Recipient>,
    /// For the thread-card projection only: when an outbound message's identity is
    /// rewritten to the recipient (see `store::apply_card_identity`), this holds the
    /// number of *additional* recipients beyond the one shown, for a "+N" hint.
    #[serde(default)]
    pub recipient_overflow: u32,
}

/// A batch of recent messages plus the folder's UID sync markers, so the caller
/// can detect UIDVALIDITY resets and persist `uid_next`.
pub struct RecentBatch {
    pub uidvalidity: u32,
    pub uid_next: u32,
    pub messages: Vec<MessageHeader>,
}

fn tls_connector() -> Result<tokio_rustls::TlsConnector> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .context("tls protocol versions")?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

/// Cap on the DNS lookup and on the TLS handshake, each. Without it a sick
/// resolver (getaddrinfo retrying across nameservers) can hold a sync for the
/// better part of a minute before erroring; failing fast surfaces the error
/// banner while a retry is still worth offering.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Cap per resolved address: one black-hole address (typically an unroutable
/// IPv6 route ahead of a fine IPv4 one) must not eat the whole connect budget.
const CONNECT_ATTEMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Resolve `host` and connect to each address in resolver order with a short
/// per-attempt cap. Logs slow stages so a stalling first sync can be traced to
/// DNS vs TCP from device logs alone.
async fn connect_tcp(host: &str, port: u16) -> Result<TcpStream> {
    let dns_started = std::time::Instant::now();
    let addrs: Vec<std::net::SocketAddr> =
        tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::lookup_host((host, port)))
            .await
            .map_err(|_| anyhow!("timed out"))
            .context("dns lookup")?
            .context("dns lookup")?
            .collect();
    let dns_ms = dns_started.elapsed().as_millis();
    if dns_ms > 1_000 {
        crate::mlog!(
            crate::log::Level::Warn,
            "net",
            "slow DNS for {host}: {dns_ms}ms"
        );
    }
    let mut last_err = anyhow!("dns lookup: no addresses for {host}");
    for addr in addrs {
        let attempt_started = std::time::Instant::now();
        match tokio::time::timeout(CONNECT_ATTEMPT_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(tcp)) => return Ok(tcp),
            Ok(Err(err)) => last_err = anyhow::Error::new(err).context(format!("connect {addr}")),
            Err(_) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "net",
                    "connect to {addr} timed out after {}ms",
                    attempt_started.elapsed().as_millis()
                );
                last_err = anyhow!("connect {addr}: timed out");
            }
        }
    }
    Err(last_err.context("tcp connect"))
}

/// Open a TCP connection, optionally wrapped in implicit TLS. Shared by the
/// IMAP and SMTP paths.
pub async fn connect_stream(host: &str, port: u16, tls: bool) -> Result<Stream> {
    let tcp = connect_tcp(host, port).await?;

    // Enable TCP keepalives to prevent silent drops by NAT/firewalls and detect network loss quickly.
    let sock_ref = socket2::SockRef::from(&tcp);
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(std::time::Duration::from_secs(60))
        .with_interval(std::time::Duration::from_secs(10));
    let _ = sock_ref.set_tcp_keepalive(&keepalive);
    if tls {
        Ok(Stream::Tls(Box::new(upgrade_to_tls(host, tcp).await?)))
    } else {
        Ok(Stream::Plain(tcp))
    }
}

/// Wrap an established plaintext TCP socket in a TLS session. Used both by the
/// implicit-TLS path above and by the STARTTLS upgrade, which hands us the raw
/// socket after the cleartext negotiation completes.
pub async fn upgrade_to_tls(host: &str, tcp: TcpStream) -> Result<TlsStream<TcpStream>> {
    let connector = tls_connector()?;
    let server_name =
        rustls::pki_types::ServerName::try_from(host.to_string()).context("invalid server name")?;
    tokio::time::timeout(CONNECT_TIMEOUT, connector.connect(server_name, tcp))
        .await
        .map_err(|_| anyhow::anyhow!("timed out"))
        .context("tls handshake")?
        .context("tls handshake")
}

pub struct XOAuth2Simple {
    auth_string: String,
    done: bool,
}

impl XOAuth2Simple {
    pub fn new(user: &str, access_token: &str) -> Self {
        Self {
            auth_string: format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token),
            done: false,
        }
    }
}

impl async_imap::Authenticator for XOAuth2Simple {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        if self.done {
            Vec::new()
        } else {
            self.done = true;
            self.auth_string.clone().into_bytes()
        }
    }
}

pub async fn connect(creds: &Creds) -> Result<Session> {
    // STARTTLS connects in cleartext and upgrades after the greeting; implicit
    // TLS wraps the socket up front. Plaintext (neither flag) is for local test
    // servers only.
    let implicit_tls = creds.tls && !creds.starttls;
    let stream = connect_stream(&creds.host, creds.port, implicit_tls).await?;
    let mut client = async_imap::Client::new(stream);
    // Consume the server greeting (e.g. "* OK ... ready"). Client::new does not
    // read it, and an unconsumed greeting shifts every response by one. That is
    // harmless for LOGIN (untagged lines are skipped), but it makes the XOAUTH2
    // handshake deadlock: the greeting is mistaken for the auth result and the
    // server's "+" continuation gets swallowed while we wait for a tagged reply.
    client
        .read_response()
        .await
        .context("read IMAP greeting")?
        .context("server closed before greeting")?;

    // STARTTLS: ask the server to begin TLS, then upgrade the underlying socket
    // in place. There is no second greeting after STARTTLS, so we go straight to
    // auth on the new TLS client.
    if creds.starttls {
        client
            .run_command_and_check_ok("STARTTLS", None)
            .await
            .context("STARTTLS")?;
        let tcp = match client.into_inner() {
            Stream::Plain(tcp) => tcp,
            Stream::Tls(_) => return Err(anyhow!("STARTTLS requested on an already-TLS stream")),
        };
        let tls = upgrade_to_tls(&creds.host, tcp).await?;
        client = async_imap::Client::new(Stream::Tls(Box::new(tls)));
    }

    let session = if creds.is_oauth() {
        let auth = XOAuth2Simple::new(&creds.user, creds.access_token.as_deref().unwrap_or(""));
        client
            .authenticate("XOAUTH2", auth)
            .await
            .map_err(|(e, _)| anyhow!("oauth login failed: {e}"))?
    } else {
        client
            .login(&creds.user, &creds.password)
            .await
            .map_err(|(e, _)| anyhow!("login failed: {e}"))?
    };
    Ok(session)
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<i64>,
}

/// Exchange a refresh token for a fresh access token via an OAuth token
/// endpoint. Uses `ureq` (a minimal blocking HTTP client over the same rustls
/// stack as IMAP/SMTP) on a blocking task, so we get correct HTTP framing —
/// status codes, Content-Length/chunked, gzip — instead of hand-parsing bytes.
///
/// `token_url` is the provider's token endpoint. `client_secret` may be empty
/// (Microsoft public clients use PKCE with no secret), in which case it's
/// omitted. `scope`, when present, requests an access token for that resource —
/// Microsoft binds tokens to a resource, so the IMAP/SMTP scopes are passed on
/// refresh; Google ignores it.
pub async fn refresh_oauth_token(
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    scope: Option<&str>,
) -> Result<(String, i64)> {
    let token_url = token_url.to_string();
    let client_id = client_id.to_string();
    let client_secret = client_secret.to_string();
    let refresh_token = refresh_token.to_string();
    let scope = scope.map(|s| s.to_string());

    let parsed = tokio::task::spawn_blocking(move || -> Result<TokenResponse> {
        let mut form: Vec<(&str, &str)> = vec![
            ("client_id", client_id.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];
        if !client_secret.is_empty() {
            form.push(("client_secret", client_secret.as_str()));
        }
        if let Some(scope) = scope.as_deref() {
            form.push(("scope", scope));
        }

        // Keep non-2xx as a response (not a transport error) so we can surface
        // the JSON error body the provider returns for an invalid/revoked token.
        let mut resp = ureq::post(&token_url)
            .config()
            .http_status_as_error(false)
            .build()
            .send_form(form)
            .context("oauth refresh request")?;

        let status = resp.status();
        let body = resp
            .body_mut()
            .read_to_string()
            .context("read oauth response")?;
        if !status.is_success() {
            return Err(anyhow!("oauth refresh failed ({status}): {body}"));
        }
        serde_json::from_str(&body).context("parse oauth response JSON")
    })
    .await
    .context("oauth refresh task")??;

    Ok((parsed.access_token, parsed.expires_in.unwrap_or(3600)))
}

pub async fn list_folders(session: &mut Session) -> Result<Vec<Folder>> {
    let mut out = Vec::new();
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    while let Some(item) = stream.next().await {
        let name = item.context("LIST item")?;
        let special_use = name.attributes().iter().find_map(|attr| {
            use async_imap::imap_proto::NameAttribute;
            match attr {
                NameAttribute::Drafts => Some("drafts"),
                NameAttribute::Sent => Some("sent"),
                NameAttribute::Trash => Some("trash"),
                NameAttribute::Junk => Some("junk"),
                NameAttribute::Archive => Some("archive"),
                NameAttribute::All => Some("all"),
                _ => None,
            }
            .map(str::to_string)
        });
        out.push(Folder {
            name: name.name().to_string(),
            delimiter: name.delimiter().map(|d| d.to_string()),
            unread: 0,
            special_use,
        });
    }
    Ok(out)
}

pub async fn create_folder(session: &mut Session, name: &str) -> Result<()> {
    session.create(name).await.context("CREATE")?;
    Ok(())
}

/// Fetch the most recent `limit` messages in `folder` as envelope summaries,
/// newest first. This is the capability Delta Chat core refuses to give us:
/// reading mail that already existed before setup.
pub async fn fetch_recent(session: &mut Session, folder: &str, limit: u32) -> Result<RecentBatch> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let uidvalidity = mailbox.uid_validity.unwrap_or(0);
    let uid_next = mailbox.uid_next.unwrap_or(0);
    let total = mailbox.exists;
    if total == 0 {
        return Ok(RecentBatch {
            uidvalidity,
            uid_next,
            messages: Vec::new(),
        });
    }
    let limit = limit.max(1);
    let start = total.saturating_sub(limit).saturating_add(1).max(1);
    let set = format!("{start}:{total}");

    let gmail = supports_gmail_ext(session).await;
    let mut out = Vec::new();
    let mut stream = session
        .fetch(set, fetch_items(gmail, false))
        .await
        .context("FETCH")?;
    while let Some(item) = stream.next().await {
        let fetch = item.context("FETCH item")?;
        let uid = match fetch.uid {
            Some(uid) => uid,
            None => continue,
        };
        let seen = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
        let starred = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
        let mut ef = match fetch.envelope() {
            Some(envelope) => envelope_fields(envelope),
            None => EnvelopeFields::default(),
        };
        if let Some(subject) = fetch.header().and_then(header_subject) {
            ef.subject = subject;
        }
        let references_root = fetch.header().and_then(references_root).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            &ef.message_id,
            &ef.in_reply_to,
            &references_root,
            uid,
        );
        out.push(MessageHeader {
            uid,
            subject: ef.subject,
            from_name: ef.from_name,
            from_addr: ef.from_addr,
            date: ef.date,
            seen,
            starred,
            thread_key,
            message_id: ef.message_id,
            in_reply_to: ef.in_reply_to,
            folder: String::new(),
            to: ef.to,
            cc: ef.cc,
            recipient_overflow: 0,
        });
    }
    drop(stream);
    out.reverse();
    Ok(RecentBatch {
        uidvalidity,
        uid_next,
        messages: out,
    })
}

pub async fn search_uids(
    session: &mut Session,
    folder: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<u32>> {
    session.select(folder).await.context("SELECT")?;
    let q = imap_quote(query);
    // On Gmail, defer to its own search engine via X-GM-RAW: it understands the
    // full Gmail query syntax (operators like `from:`, `has:attachment`,
    // `older_than:`, relevance) instead of our crude substring OR. Elsewhere fall
    // back to plain SUBJECT/FROM/TEXT matching.
    let criteria = if supports_gmail_ext(session).await {
        format!("X-GM-RAW {q}")
    } else {
        format!("OR OR SUBJECT {q} FROM {q} TEXT {q}")
    };
    let set: HashSet<u32> = session
        .uid_search(criteria)
        .await
        .context("UID SEARCH query")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    uids.truncate(limit as usize);
    Ok(uids)
}

/// Return every UID currently in the folder. Used to prune locally cached
/// messages that have been moved or deleted by another client.
pub async fn list_all_uids(session: &mut Session, folder: &str) -> Result<HashSet<u32>> {
    session.select(folder).await.context("SELECT")?;
    let set: HashSet<u32> = session.uid_search("ALL").await.context("UID SEARCH ALL")?;
    Ok(set)
}

pub async fn search_starred_uids(
    session: &mut Session,
    folder: &str,
    limit: u32,
) -> Result<Vec<u32>> {
    session.select(folder).await.context("SELECT")?;
    let set: HashSet<u32> = session
        .uid_search("FLAGGED")
        .await
        .context("UID SEARCH FLAGGED")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    uids.truncate(limit as usize);
    Ok(uids)
}

/// A message located by Message-ID and fully fetched: its header row (with a
/// computed `thread_key` and source `folder` populated) plus the parsed body.
/// Used by the on-demand ancestor fetch that fills thread gaps when a reply or
/// draft references messages outside the locally-synced window.
pub struct FetchedMessage {
    pub header: MessageHeader,
    pub message: parse::Message,
}

/// Locate messages in `folder` whose `Message-ID` matches any id in `ids` (bare,
/// no angle brackets) and fetch flags + envelope + full body for each match in a
/// single round-trip. Returns one `FetchedMessage` per located UID; ids with no
/// match here are simply absent from the result so the caller can try the next
/// folder. The `thread_key` is computed exactly as the recent-sync path does, so
/// a fetched ancestor groups into the same thread as the reply that referenced it.
pub async fn fetch_by_message_ids(
    session: &mut Session,
    folder: &str,
    ids: &[String],
    media_root: &std::path::Path,
    account: &str,
) -> Result<Vec<FetchedMessage>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let set: HashSet<u32> = session
        .uid_search(message_id_search_criteria(ids))
        .await
        .context("UID SEARCH Message-ID")?;
    if set.is_empty() {
        return Ok(Vec::new());
    }
    let uid_set = set.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let gmail = supports_gmail_ext(session).await;
    let mut out = Vec::new();
    let mut stream = session
        .uid_fetch(uid_set, fetch_items(gmail, true))
        .await
        .context("UID FETCH Message-ID matches")?;
    while let Some(item) = stream.next().await {
        let fetch = item.context("UID FETCH item")?;
        let uid = match fetch.uid {
            Some(uid) => uid,
            None => continue,
        };
        let raw = match fetch.body() {
            Some(body) => body.to_vec(),
            None => continue,
        };
        let seen = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
        let starred = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
        let mut ef = match fetch.envelope() {
            Some(envelope) => envelope_fields(envelope),
            None => EnvelopeFields::default(),
        };
        if let Some(subject) = fetch.header().and_then(header_subject) {
            ef.subject = subject;
        }
        let references_root = fetch.header().and_then(references_root).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            &ef.message_id,
            &ef.in_reply_to,
            &references_root,
            uid,
        );
        let media = parse::MediaCtx {
            root: media_root.to_path_buf(),
            account: account.to_string(),
            folder: folder.to_string(),
            uid,
        };
        let message = parse::parse_message(&raw, Some(&media));
        out.push(FetchedMessage {
            header: MessageHeader {
                uid,
                folder: folder.to_string(),
                subject: ef.subject,
                from_name: ef.from_name,
                from_addr: ef.from_addr,
                date: ef.date,
                seen,
                starred,
                thread_key,
                message_id: ef.message_id,
                in_reply_to: ef.in_reply_to,
                to: ef.to,
                cc: ef.cc,
                recipient_overflow: 0,
            },
            message,
        });
    }
    drop(stream);
    Ok(out)
}

/// Build a SEARCH criteria matching any `HEADER MESSAGE-ID` in `ids`. IMAP SEARCH
/// has no native set membership, so terms are chained with the binary prefix
/// `OR` operator (`OR a b`, then `OR (OR a b) c`, …).
fn message_id_search_criteria(ids: &[String]) -> String {
    let mut terms = ids
        .iter()
        .map(|id| format!("HEADER MESSAGE-ID {}", imap_quote(id)));
    let Some(first) = terms.next() else {
        return "ALL".to_string();
    };
    terms.fold(first, |expr, term| format!("OR {expr} {term}"))
}

pub async fn fetch_headers_by_uid(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<MessageHeader>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let gmail = supports_gmail_ext(session).await;
    let mut out = Vec::new();
    let mut stream = session
        .uid_fetch(uid_set, fetch_items(gmail, false))
        .await
        .context("UID FETCH search headers")?;
    while let Some(item) = stream.next().await {
        let fetch = item.context("UID FETCH search header item")?;
        let uid = match fetch.uid {
            Some(uid) => uid,
            None => continue,
        };
        let seen = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
        let starred = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
        let mut ef = match fetch.envelope() {
            Some(envelope) => envelope_fields(envelope),
            None => EnvelopeFields::default(),
        };
        if let Some(subject) = fetch.header().and_then(header_subject) {
            ef.subject = subject;
        }
        let references_root = fetch.header().and_then(references_root).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            &ef.message_id,
            &ef.in_reply_to,
            &references_root,
            uid,
        );
        out.push(MessageHeader {
            uid,
            subject: ef.subject,
            from_name: ef.from_name,
            from_addr: ef.from_addr,
            date: ef.date,
            seen,
            starred,
            thread_key,
            message_id: ef.message_id,
            in_reply_to: ef.in_reply_to,
            folder: String::new(),
            to: ef.to,
            cc: ef.cc,
            recipient_overflow: 0,
        });
    }
    drop(stream);
    out.sort_unstable_by(|a, b| b.uid.cmp(&a.uid));
    Ok(out)
}

/// Read-state changes reconciled via CONDSTORE: which messages flipped \Seen or \Flagged (starred),
/// plus the folder's current HIGHESTMODSEQ to persist for the next incremental
/// fetch.
pub struct FlagSync {
    pub highest_modseq: u64,
    pub changes: Vec<(u32, bool, bool)>, // (uid, seen, starred)
}

/// Reconcile \Seen and \Flagged across an entire folder using CONDSTORE so updates made
/// on other devices are reflected even for messages older than the recent
/// window. Selects with `(CONDSTORE)` to learn HIGHESTMODSEQ, then fetches only
/// the messages whose flags changed since `since_modseq` (CHANGEDSINCE).
///
/// Returns no changes (but still reports HIGHESTMODSEQ) when there's no prior
/// baseline or UIDVALIDITY changed. On servers without CONDSTORE the SELECT or
/// FETCH errors; callers treat that as a no-op and fall back to the recent sync.
pub async fn sync_flags(
    session: &mut Session,
    folder: &str,
    since_modseq: u64,
    validity_matches: bool,
) -> Result<FlagSync> {
    let mailbox = session
        .select_condstore(folder)
        .await
        .context("SELECT CONDSTORE")?;
    let highest_modseq = mailbox.highest_modseq.unwrap_or(0);

    if since_modseq == 0
        || !validity_matches
        || highest_modseq == 0
        || highest_modseq <= since_modseq
    {
        return Ok(FlagSync {
            highest_modseq,
            changes: Vec::new(),
        });
    }

    let query = format!("(FLAGS) (CHANGEDSINCE {since_modseq})");
    let mut stream = session
        .uid_fetch("1:*", query)
        .await
        .context("UID FETCH CHANGEDSINCE")?;
    let mut changes = Vec::new();
    while let Some(item) = stream.next().await {
        let fetch = item.context("flag fetch item")?;
        let uid = match fetch.uid {
            Some(uid) => uid,
            None => continue,
        };
        let seen = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
        let starred = fetch
            .flags()
            .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
        changes.push((uid, seen, starred));
    }
    drop(stream);
    Ok(FlagSync {
        highest_modseq,
        changes,
    })
}

pub async fn read_message(
    session: &mut Session,
    folder: &str,
    uid: u32,
    media: &parse::MediaCtx,
) -> Result<parse::Message> {
    session.select(folder).await.context("SELECT")?;
    // Opening a thread should not mark it read until the reader reaches the
    // bottom. Use PEEK here and let the explicit mark-read path set \Seen.
    fetch_full_message(session, uid, media, true)
        .await?
        .ok_or_else(|| anyhow!("message uid {uid} not found in {folder}"))
}

/// Add or remove the `\Seen` flag on a set of UIDs. `seen = true` marks read
/// (`+FLAGS`), `false` marks unread (`-FLAGS`).
pub async fn set_seen(session: &mut Session, folder: &str, uids: &[u32], seen: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await.context("SELECT")?;
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let op = if seen {
        "+FLAGS.SILENT (\\Seen)"
    } else {
        "-FLAGS.SILENT (\\Seen)"
    };
    let mut stream = session
        .uid_store(uid_set, op)
        .await
        .context("UID STORE FLAGS.SILENT")?;
    while let Some(item) = stream.next().await {
        item.context("UID STORE item")?;
    }
    Ok(())
}

/// Add or remove the `\Flagged` flag on a set of UIDs. `starred = true` marks starred
/// (`+FLAGS`), `false` marks unstarred (`-FLAGS`).
pub async fn set_starred(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
    starred: bool,
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await.context("SELECT")?;
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let op = if starred {
        "+FLAGS.SILENT (\\Flagged)"
    } else {
        "-FLAGS.SILENT (\\Flagged)"
    };
    let mut stream = session
        .uid_store(uid_set, op)
        .await
        .context("UID STORE FLAGS.SILENT")?;
    while let Some(item) = stream.next().await {
        item.context("UID STORE item")?;
    }
    Ok(())
}

pub async fn move_to_folder(
    session: &mut Session,
    source_folder: &str,
    dest_folder: &str,
    uids: &[u32],
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(source_folder).await.context("SELECT")?;
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    // UID MOVE is the RFC 6851 MOVE extension; servers that don't advertise it
    // (e.g. mailo) reject it with "Unknown command". Fall back to the classic
    // COPY + \Deleted + EXPUNGE sequence, which every IMAP server supports.
    let supports_move = match session.capabilities().await {
        Ok(caps) => caps.has_str("MOVE"),
        Err(_) => false,
    };
    if supports_move {
        session
            .uid_mv(&uid_set, dest_folder)
            .await
            .context("UID MOVE")?;
    } else {
        session
            .uid_copy(&uid_set, dest_folder)
            .await
            .context("UID COPY")?;
        expunge_uids(session, source_folder, uids).await?;
    }
    Ok(())
}

#[derive(Clone)]
pub struct RawMessageCopy {
    pub raw: Vec<u8>,
    pub seen: bool,
    pub starred: bool,
}

/// Fetch raw RFC822 bytes plus the user-visible flags worth preserving when a
/// message is copied into another mailbox. Uses BODY.PEEK[] so copying never
/// marks unread source mail as read.
pub async fn fetch_raw_messages_for_copy(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<RawMessageCopy>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    session.select(folder).await.context("SELECT")?;
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stream = session
        .uid_fetch(uid_set, "(FLAGS BODY.PEEK[])")
        .await
        .context("UID FETCH raw copy")?;
    let mut out = Vec::new();
    while let Some(item) = stream.next().await {
        let fetch = item.context("UID FETCH raw copy item")?;
        if let Some(body) = fetch.body() {
            let seen = fetch
                .flags()
                .any(|flag| matches!(flag, async_imap::types::Flag::Seen));
            let starred = fetch
                .flags()
                .any(|flag| matches!(flag, async_imap::types::Flag::Flagged));
            out.push(RawMessageCopy {
                raw: body.to_vec(),
                seen,
                starred,
            });
        }
    }
    Ok(out)
}

pub async fn append_copied_message(
    session: &mut Session,
    folder: &str,
    message: &RawMessageCopy,
) -> Result<()> {
    let flags = match (message.seen, message.starred) {
        (true, true) => Some("(\\Seen \\Flagged)"),
        (true, false) => Some("(\\Seen)"),
        (false, true) => Some("(\\Flagged)"),
        (false, false) => None,
    };
    session
        .append(folder, flags, None, &message.raw)
        .await
        .context("IMAP APPEND copied message")?;
    Ok(())
}

/// Permanently delete `uids` from `folder`: mark them `\Deleted`, then UID
/// EXPUNGE so they skip Trash entirely. Used for discarding drafts, which are
/// unsent and ephemeral — moving them to Trash leaves confusing `\Draft` copies.
pub async fn expunge_uids(session: &mut Session, folder: &str, uids: &[u32]) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(folder).await.context("SELECT")?;
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stream = session
        .uid_store(&uid_set, "+FLAGS.SILENT (\\Deleted)")
        .await
        .context("UID STORE Deleted")?;
    while let Some(item) = stream.next().await {
        item.context("UID STORE item")?;
    }
    drop(stream);

    let supports_uidplus = match session.capabilities().await {
        Ok(caps) => caps.has_str("UIDPLUS"),
        Err(_) => false,
    };

    if supports_uidplus {
        let estream = session.uid_expunge(&uid_set).await.context("UID EXPUNGE")?;
        futures::pin_mut!(estream);
        while let Some(item) = estream.next().await {
            item.context("UID EXPUNGE item")?;
        }
    } else {
        let estream = session.expunge().await.context("EXPUNGE")?;
        futures::pin_mut!(estream);
        while let Some(item) = estream.next().await {
            item.context("EXPUNGE item")?;
        }
    }
    Ok(())
}

/// Append a raw RFC822 message to `folder` with the `\Seen` flag, so the user's
/// own sent reply appears in Sent without being marked unread. Used after SMTP
/// send so the message threads back into the conversation on next sync.
pub async fn append_to_sent(session: &mut Session, folder: &str, raw: &[u8]) -> Result<()> {
    session
        .append(folder, Some("(\\Seen)"), None, raw)
        .await
        .context("IMAP APPEND")?;
    Ok(())
}

/// Append a draft to `folder`, then prune older copies that share the same
/// `message_id` so repeated autosaves replace the draft in place instead of
/// piling up duplicates. `message_id` is the bare id (no angle brackets) that
/// `smtp::build_message` embedded as the Message-ID header. Pruning is
/// best-effort — the APPEND is what matters, so search/expunge failures are
/// logged and swallowed.
pub async fn replace_draft(
    session: &mut Session,
    folder: &str,
    raw: &[u8],
    message_id: &str,
) -> Result<()> {
    session
        .append(folder, Some("(\\Draft \\Seen)"), None, raw)
        .await
        .context("IMAP APPEND to Drafts")?;
    if message_id.trim().is_empty() {
        return Ok(());
    }
    if let Err(err) = prune_old_drafts(session, folder, message_id).await {
        eprintln!("meron-core: prune old drafts failed message_id={message_id}: {err:#}");
    }
    Ok(())
}

/// Mark every draft in `folder` carrying `message_id` as `\Deleted` except the
/// newest (highest UID) and UID EXPUNGE them. Assumes `folder` holds at most one
/// live copy plus stale duplicates from earlier autosaves.
async fn prune_old_drafts(session: &mut Session, folder: &str, message_id: &str) -> Result<()> {
    session.select(folder).await.context("SELECT Drafts")?;
    let criteria = format!("HEADER MESSAGE-ID {}", imap_quote(message_id));
    let set: HashSet<u32> = session
        .uid_search(criteria)
        .await
        .context("UID SEARCH draft id")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    if uids.len() <= 1 {
        return Ok(());
    }
    // Keep the newest copy (highest UID); the rest are stale autosaves.
    uids.sort_unstable();
    uids.pop();
    expunge_uids(session, folder, &uids).await
}

/// Permanently remove every draft in `folder` carrying `message_id`. Used after
/// the message is sent, because the saved Drafts copy is no longer useful.
pub async fn discard_draft(session: &mut Session, folder: &str, message_id: &str) -> Result<usize> {
    if message_id.trim().is_empty() {
        return Ok(0);
    }
    session.select(folder).await.context("SELECT Drafts")?;
    let criteria = format!("HEADER MESSAGE-ID {}", imap_quote(message_id));
    let set: HashSet<u32> = session
        .uid_search(criteria)
        .await
        .context("UID SEARCH draft id")?;
    let uids: Vec<u32> = set.into_iter().collect();
    if uids.is_empty() {
        return Ok(0);
    }
    let deleted = uids.len();
    expunge_uids(session, folder, &uids).await?;
    Ok(deleted)
}

/// Best-effort resolver for an account's Sent folder. Prefers IMAP SPECIAL-USE
/// (`\Sent` attribute) when the server advertises it; falls back to common name
/// patterns (Gmail's `[Gmail]/Sent Mail`, Dovecot's `Sent`, Courier's `INBOX.Sent`).
/// Returns `None` when the server has no LIST entries matching either path.
pub async fn find_sent_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(attr, async_imap::imap_proto::NameAttribute::Sent) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_sent(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

pub async fn find_trash_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(attr, async_imap::imap_proto::NameAttribute::Trash) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_trash(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

pub async fn find_archive_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(
                attr,
                async_imap::imap_proto::NameAttribute::Archive
                    | async_imap::imap_proto::NameAttribute::All
            ) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_archive(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

pub async fn find_drafts_folder(session: &mut Session) -> Result<Option<String>> {
    let mut stream = session.list(Some(""), Some("*")).await.context("LIST")?;
    let mut by_attr: Option<String> = None;
    let mut by_name: Option<String> = None;
    while let Some(item) = stream.next().await {
        let entry = item.context("LIST item")?;
        let name = entry.name().to_string();
        for attr in entry.attributes() {
            if matches!(attr, async_imap::imap_proto::NameAttribute::Drafts) {
                by_attr = Some(name.clone());
            }
        }
        if by_name.is_none() && looks_like_drafts(&name) {
            by_name = Some(name);
        }
    }
    drop(stream);
    Ok(by_attr.or(by_name))
}

fn looks_like_archive(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "archive"
            | "archives"
            | "all mail"
            | "inbox.archive"
            | "inbox.archives"
            | "[gmail]/all mail"
            | "[google mail]/all mail"
    )
}

pub fn looks_like_sent(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Gmail's localized account uses "[Gmail]/Sent Mail"; Dovecot ships "Sent";
    // Courier/Cyrus often namespace under INBOX (`INBOX.Sent`).
    matches!(
        n.as_str(),
        "sent" | "sent mail" | "sent messages" | "sent items" | "inbox.sent" | "[gmail]/sent mail"
    )
}

fn looks_like_trash(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "trash"
            | "deleted"
            | "deleted items"
            | "deleted messages"
            | "bin"
            | "inbox.trash"
            | "inbox.deleted"
            | "inbox.deleted items"
            | "[gmail]/trash"
            | "[gmail]/bin"
    )
}

pub fn looks_like_drafts(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "drafts" | "draft" | "inbox.drafts" | "inbox.draft" | "[gmail]/drafts" | "[gmail]/draft"
    )
}

/// Fetch and parse one full message by UID, assuming `folder` is already
/// selected on `session`. Shared by the on-demand reader (which selects then
/// calls this once) and the body prefetcher (which selects once and pulls many
/// UIDs over a single connection).
///
/// `peek` selects `BODY.PEEK[]` over `RFC822`: both return the full message, but
/// `RFC822` implicitly sets `\Seen` server-side while `BODY.PEEK[]` does not. The
/// prefetcher must peek so background warming never marks unread mail as read.
pub async fn fetch_full_message(
    session: &mut Session,
    uid: u32,
    media: &parse::MediaCtx,
    peek: bool,
) -> Result<Option<parse::Message>> {
    let item = if peek { "(BODY.PEEK[])" } else { "(RFC822)" };
    let mut stream = session
        .uid_fetch(uid.to_string(), item)
        .await
        .context("UID FETCH")?;
    let mut raw: Option<Vec<u8>> = None;
    while let Some(item) = stream.next().await {
        let fetch = item.context("UID FETCH item")?;
        if let Some(body) = fetch.body() {
            raw = Some(body.to_vec());
            break;
        }
    }
    drop(stream);
    Ok(raw.map(|bytes| parse::parse_message(&bytes, Some(media))))
}

/// Select `folder` and fetch full bodies for `uids`, parsing each into a
/// `Message`. Used by the mobile thread reader, which (unlike desktop) does not
/// warm bodies during sync, so opening a thread fetches its bodies on demand.
/// `peek` so reading doesn't flip server-side `\Seen`.
pub async fn fetch_bodies(
    session: &mut Session,
    folder: &str,
    uids: &[u32],
    media_root: std::path::PathBuf,
    account: &str,
) -> Result<Vec<(u32, parse::Message)>> {
    session.select(folder).await.context("SELECT")?;
    let mut out = Vec::new();
    for &uid in uids {
        let media = parse::MediaCtx {
            root: media_root.clone(),
            account: account.to_string(),
            folder: folder.to_string(),
            uid,
        };
        if let Some(message) = fetch_full_message(session, uid, &media, true).await? {
            out.push((uid, message));
        }
    }
    Ok(out)
}

/// UIDs worth prefetching full bodies for in `folder`: messages that are both
/// unread *and* received within the last `days` days. The two criteria are ANDed
/// (IMAP SEARCH semantics). Uses server-side SEARCH so the set isn't limited to
/// the recent-envelope window and doesn't depend on locally-parsed date strings.
/// Selects the folder.
pub async fn search_prefetch_uids(
    session: &mut Session,
    folder: &str,
    days: u32,
) -> Result<Vec<u32>> {
    session.select(folder).await.context("SELECT")?;
    let since = imap_date_days_ago(days);
    let set: HashSet<u32> = session
        .uid_search(format!("UNSEEN SINCE {since}"))
        .await
        .context("UID SEARCH UNSEEN SINCE")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    uids.sort_unstable();
    Ok(uids)
}

/// Format the IMAP `dd-Mon-yyyy` date for `days` days before now (UTC), for use
/// in `SEARCH SINCE` (which compares by internal date, ignoring time of day).
fn imap_date_days_ago(days: u32) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let secs = now - (days as i64) * 86_400;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{:02}-{}-{:04}", d, MONTHS[(m - 1) as usize], y)
}

fn imap_quote(value: &str) -> String {
    let escaped = value
        .trim()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ");
    format!("\"{escaped}\"")
}

/// Convert a count of days since the Unix epoch to a civil (year, month, day),
/// month in 1..=12 and day in 1..=31. Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

/// Parsed envelope fields. A named struct rather than a tuple because there are
/// enough of them (and the recipient lists) that positional destructuring at the
/// call sites would be error-prone.
#[derive(Default)]
struct EnvelopeFields {
    subject: String,
    from_name: String,
    from_addr: String,
    date: i64,
    message_id: String,
    in_reply_to: String,
    to: Vec<Recipient>,
    cc: Vec<Recipient>,
}

fn envelope_fields(envelope: &async_imap::imap_proto::Envelope) -> EnvelopeFields {
    let subject = envelope
        .subject
        .as_ref()
        .map(|raw| parse::decode_words(&String::from_utf8_lossy(raw)))
        .unwrap_or_default();
    let date = envelope
        .date
        .as_ref()
        .map(|raw| parse::parse_date_to_epoch(&String::from_utf8_lossy(raw)))
        .unwrap_or_default();
    let (from_name, from_addr) = envelope
        .from
        .as_ref()
        .and_then(|list| list.first())
        .map(address_fields)
        .unwrap_or_default();
    let message_id = envelope
        .message_id
        .as_ref()
        .map(|raw| normalize_message_id(&String::from_utf8_lossy(raw)))
        .unwrap_or_default();
    let in_reply_to = envelope
        .in_reply_to
        .as_ref()
        .map(|raw| normalize_message_id(&String::from_utf8_lossy(raw)))
        .unwrap_or_default();
    EnvelopeFields {
        subject,
        from_name,
        from_addr,
        date,
        message_id,
        in_reply_to,
        to: address_list(envelope.to.as_deref()),
        cc: address_list(envelope.cc.as_deref()),
    }
}

/// Convert an envelope address list (`To`/`Cc`) into recipients, dropping
/// entries that yield no email address.
fn address_list(list: Option<&[async_imap::imap_proto::Address]>) -> Vec<Recipient> {
    list.into_iter()
        .flatten()
        .map(|addr| {
            let (name, addr) = address_fields(addr);
            Recipient { name, addr }
        })
        .filter(|r| !r.addr.is_empty())
        .collect()
}

/// FETCH item list. Adds `X-GM-THRID` on Gmail so messages thread by Gmail's
/// server-side thread id, and `BODY.PEEK[]` when the full message is wanted.
fn fetch_items(gmail: bool, body: bool) -> &'static str {
    match (gmail, body) {
        (true, true) => "(UID FLAGS ENVELOPE RFC822.HEADER X-GM-THRID BODY.PEEK[])",
        (true, false) => "(UID FLAGS ENVELOPE RFC822.HEADER X-GM-THRID)",
        (false, true) => "(UID FLAGS ENVELOPE RFC822.HEADER BODY.PEEK[])",
        (false, false) => "(UID FLAGS ENVELOPE RFC822.HEADER)",
    }
}

/// Whether the server advertises Gmail's `X-GM-EXT-1` extension, i.e. supports
/// `X-GM-THRID`. Covers Gmail/Workspace via both OAuth and app passwords. Issues
/// one CAPABILITY round-trip; negligible next to the connect-time TLS + auth.
async fn supports_gmail_ext(session: &mut Session) -> bool {
    match session.capabilities().await {
        Ok(caps) => caps.has_str("X-GM-EXT-1"),
        Err(_) => false,
    }
}

fn thread_key(
    gmail_thrid: Option<u64>,
    message_id: &str,
    in_reply_to: &str,
    references_root: &str,
    uid: u32,
) -> String {
    // Gmail computes threads server-side (References + In-Reply-To + subject
    // similarity), matching what the user sees in the Gmail UI. Prefer it when
    // the server exposes X-GM-THRID; namespace it so it never collides with a
    // Message-ID-based key.
    if let Some(thrid) = gmail_thrid {
        return format!("gmthrid:{thrid}");
    }
    if !references_root.is_empty() {
        return references_root.to_string();
    }
    if !in_reply_to.is_empty() {
        return in_reply_to.to_string();
    }
    if !message_id.is_empty() {
        return message_id.to_string();
    }
    format!("uid:{uid}")
}

/// Strip angle brackets and whitespace from an RFC Message-ID, preserving the
/// original casing. Message-IDs are case-sensitive opaque tokens (RFC 5322) and
/// this value is echoed back on the wire in reply `In-Reply-To`/`References`
/// headers, where a case change breaks the recipient's threading (e.g. Gmail
/// matches ids byte-for-byte). Store lookups that compare ids wrap both sides
/// in `lower()` instead.
fn normalize_message_id(value: &str) -> String {
    if let Some(start) = value.find('<')
        && let Some(end) = value[start..].find('>')
    {
        let id = &value[start + 1..start + end];
        return id.trim().to_string();
    }
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string()
}

fn references_root(header: &[u8]) -> Option<String> {
    let (headers, _) = mailparse::parse_headers(header).ok()?;
    headers
        .get_first_value("References")
        .as_deref()
        .and_then(first_message_id)
}

/// Subject parsed from the fetched RFC822 header, preferred over the ENVELOPE
/// subject: imap-proto hands back IMAP quoted-string contents without
/// unescaping, so an envelope subject containing double quotes arrives as
/// `\"...\"`. Since the subject is part of the compound thread key, that
/// artifact splits the conversation into a duplicate thread.
fn header_subject(header: &[u8]) -> Option<String> {
    let (headers, _) = mailparse::parse_headers(header).ok()?;
    headers.get_first_value("Subject").filter(|s| !s.is_empty())
}

fn first_message_id(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .map(normalize_message_id)
        .find(|id| !id.is_empty())
}

fn address_fields(addr: &async_imap::imap_proto::Address) -> (String, String) {
    let name = addr
        .name
        .as_ref()
        .map(|raw| parse::decode_words(&String::from_utf8_lossy(raw)))
        .unwrap_or_default();
    let mailbox = addr
        .mailbox
        .as_ref()
        .map(|raw| String::from_utf8_lossy(raw).into_owned())
        .unwrap_or_default();
    let host = addr
        .host
        .as_ref()
        .map(|raw| String::from_utf8_lossy(raw).into_owned())
        .unwrap_or_default();
    let email = if !mailbox.is_empty() && !host.is_empty() {
        format!("{mailbox}@{host}")
    } else {
        mailbox
    };
    (name, email)
}

#[cfg(test)]
mod tests {
    use super::{
        civil_from_days, first_message_id, header_subject, imap_quote, looks_like_drafts,
        message_id_search_criteria, normalize_message_id, references_root, thread_key,
    };

    #[test]
    fn looks_like_drafts_matches_common_names_case_insensitively() {
        for name in ["Drafts", "draft", "INBOX.Drafts", "[Gmail]/Drafts"] {
            assert!(looks_like_drafts(name), "{name}");
        }
        for name in ["INBOX", "Sent", "Draft Specs", ""] {
            assert!(!looks_like_drafts(name), "{name}");
        }
    }

    #[test]
    fn references_root_uses_first_id_of_folded_header() {
        // A reply-to-a-reply: References spans two folded lines, root first.
        let header = b"Subject: Re: test mailo 1\r\nReferences: <CAJ7M84+root@mail.gmail.com>\r\n\t<meron-second@mailo.com>\r\nIn-Reply-To: <meron-second@mailo.com>\r\n\r\n";
        assert_eq!(
            references_root(header).as_deref(),
            Some("CAJ7M84+root@mail.gmail.com")
        );
    }

    #[test]
    fn header_subject_decodes_and_falls_back() {
        let header = b"Subject: A \"quoted\" title\r\nFrom: x@y\r\n\r\n";
        assert_eq!(
            header_subject(header).as_deref(),
            Some("A \"quoted\" title")
        );
        let encoded = b"Subject: =?UTF-8?B?SGVsbMO2?=\r\n\r\n";
        assert_eq!(header_subject(encoded).as_deref(), Some("Hellö"));
        // No / empty Subject header: caller keeps the envelope subject.
        assert_eq!(header_subject(b"From: x@y\r\n\r\n"), None);
        assert_eq!(header_subject(b"Subject:\r\n\r\n"), None);
    }

    #[test]
    fn normalize_message_id_extracts_bare_id_preserving_case() {
        // Case must survive: the stored id is echoed into reply In-Reply-To/
        // References headers, and receivers (Gmail) match ids case-sensitively.
        assert_eq!(
            normalize_message_id("<nonbili/NouTube/issues/253@github.com>"),
            "nonbili/NouTube/issues/253@github.com"
        );
        assert_eq!(normalize_message_id("<ID@Host>"), "ID@Host");
        assert_eq!(normalize_message_id("prefix <Id@Host> suffix"), "Id@Host");
        assert_eq!(normalize_message_id("  bare@host  "), "bare@host");
        // Unterminated bracket falls back to trimming both bracket chars.
        assert_eq!(normalize_message_id("<broken@host"), "broken@host");
        assert_eq!(normalize_message_id(""), "");
    }

    #[test]
    fn first_message_id_takes_the_references_root() {
        assert_eq!(
            first_message_id("<Root@h> <mid@h> <leaf@h>").as_deref(),
            Some("Root@h")
        );
        assert_eq!(first_message_id("   ").as_deref(), None);
    }

    #[test]
    fn imap_quote_escapes_quotes_backslashes_and_newlines() {
        assert_eq!(imap_quote("plain"), "\"plain\"");
        assert_eq!(imap_quote(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(imap_quote("a\r\nb"), "\"a  b\"");
    }

    #[test]
    fn message_id_search_criteria_builds_nested_or() {
        assert_eq!(message_id_search_criteria(&[]), "ALL");
        assert_eq!(
            message_id_search_criteria(&["a@h".to_string()]),
            "HEADER MESSAGE-ID \"a@h\""
        );
        assert_eq!(
            message_id_search_criteria(&["a@h".to_string(), "b@h".to_string()]),
            "OR HEADER MESSAGE-ID \"a@h\" HEADER MESSAGE-ID \"b@h\""
        );
    }

    #[test]
    fn civil_from_days_round_trips_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        // 2000-02-29 (leap day) is 11016 days after the epoch.
        assert_eq!(civil_from_days(11016), (2000, 2, 29));
        assert_eq!(civil_from_days(20614), (2026, 6, 10));
    }

    #[test]
    fn gmail_thrid_takes_precedence() {
        // Even with full RFC headers present, a Gmail thrid wins and is namespaced.
        let key = thread_key(Some(42), "msgid", "inreply", "refsroot", 7);
        assert_eq!(key, "gmthrid:42");
    }

    #[test]
    fn falls_back_to_rfc_headers_without_thrid() {
        assert_eq!(
            thread_key(None, "msgid", "inreply", "refsroot", 7),
            "refsroot"
        );
        assert_eq!(thread_key(None, "msgid", "inreply", "", 7), "inreply");
        assert_eq!(thread_key(None, "msgid", "", "", 7), "msgid");
        assert_eq!(thread_key(None, "", "", "", 7), "uid:7");
    }
}
