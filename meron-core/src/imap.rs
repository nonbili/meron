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
    /// Whether this account follows the app-wide proxy, forces a direct
    /// connection, or uses its own.
    pub proxy: crate::proxy::ProxyChoice,
    /// Hex SHA-256 of an IMAP server certificate the user inspected and
    /// accepted, for servers whose certificate webpki cannot validate (local
    /// bridges with a self-signed leaf). See [`crate::tls`].
    pub cert_pin: Option<String>,
    /// The same, for the submission server. Separate because the two can be
    /// different daemons with different certificates — a local bridge serves one
    /// certificate on both ports, but nothing guarantees that in general.
    pub smtp_cert_pin: Option<String>,
}

/// Which settings a save left out of its parameters, and so must be carried
/// over from the stored account instead of reset.
///
/// Saving an account resends what the setup form holds — servers, ports,
/// credentials. Everything else about a connection lives elsewhere in the UI (a
/// per-account proxy) or is answered by a prompt (an accepted certificate), so a
/// reconnect that only re-enters a password must not clear them.
#[derive(Clone, Copy, Debug)]
pub struct OmittedSettings {
    pub proxy: bool,
    pub cert_pin: bool,
    pub smtp_cert_pin: bool,
    /// Editing an existing account's servers must not require retyping its
    /// password, and the UI never holds one to resend: an absent `password`
    /// means "keep the stored one" rather than "set it to empty".
    pub password: bool,
}

impl OmittedSettings {
    pub fn any(&self) -> bool {
        self.proxy || self.cert_pin || self.smtp_cert_pin || self.password
    }
}

impl Creds {
    /// Copy the omitted settings across from the account as it is stored today.
    pub fn carry_over(&mut self, stored: &Creds, omitted: OmittedSettings) {
        if omitted.proxy {
            self.proxy = stored.proxy.clone();
        }
        if omitted.cert_pin {
            self.cert_pin = stored.cert_pin.clone();
        }
        if omitted.smtp_cert_pin {
            self.smtp_cert_pin = stored.smtp_cert_pin.clone();
        }
        if omitted.password {
            self.password = stored.password.clone();
        }
    }

    /// Whether this account authenticates via OAuth2 (XOAUTH2) rather than a
    /// password. Covers every provider — `gmail_oauth`, `outlook_oauth`, … — so
    /// new providers don't need to be enumerated at each auth site.
    pub fn is_oauth(&self) -> bool {
        self.auth_type.ends_with("_oauth")
    }
}

#[derive(serde::Serialize, Default)]
pub struct Folder {
    /// Canonical wire name, modified UTF-7 when the server speaks it. Everything
    /// that addresses the mailbox — SELECT, cached message rows, saved Kanban
    /// columns — uses this; only `display_name` is decoded for humans.
    pub name: String,
    /// `name` decoded from modified UTF-7. Equal to `name` for ASCII folders.
    #[serde(default)]
    pub display_name: String,
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
    /// UI role computed from `special_use` plus core name heuristics.
    #[serde(default)]
    pub role: String,
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
    /// Gmail's stable per-message id (`X-GM-MSGID`), when the server exposes it.
    #[serde(default)]
    pub gmail_msg_id: Option<u64>,
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

/// Cap on DNS plus every TCP attempt for one host. Without it a sick resolver
/// (getaddrinfo retrying across nameservers) or a run of black-holed addresses
/// can hold a sync for the better part of a minute before erroring; failing
/// fast surfaces the error banner while a retry is still worth offering. It has
/// to cover several sequential [`CONNECT_ATTEMPT_TIMEOUT`] rounds, so it is
/// deliberately larger than one attempt: capping it near a single attempt would
/// abandon a host whose later addresses are the reachable ones. This is the
/// only bound on the SMTP and certificate-probe paths, which do not run under
/// [`imap_connect_timeout`].
const TCP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Cap on the TLS handshake alone, for the same reason.
const TLS_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Total budget for DNS through authentication. Leave 20% of the configured
/// background-sync budget for connection coordination and the actual mailbox
/// command. `MERON_SYNC_TIMEOUT` is raised precisely for gateways whose
/// handshake is slow (see [`crate::engine::background_sync_timeout`]), so this
/// scales with it rather than pinning a ceiling such a gateway could never
/// finish under.
fn imap_connect_timeout() -> std::time::Duration {
    crate::engine::background_sync_timeout().mul_f32(0.8)
}

/// Floor under [`imap_protocol_timeout`]. A stage cap is a fallback for a stage
/// that never completes, not a bar a slow-but-working server has to clear:
/// Gmail's XOAUTH2 exchange routinely takes 4-7s on a congested link, so an
/// even share of the default budget (8s) fails it often enough to keep the
/// connectivity banner up on an account that is fine. Keep the floor several
/// times the slowest healthy stage seen in the wild.
const MIN_PROTOCOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// The TCP/TLS stages may complete while the server never sends its greeting or
/// never finishes authentication. Keep those protocol stages independently
/// bounded so callers receive the concrete failing stage rather than only an
/// outer operation-budget timeout. Each stage gets a share of the connect
/// budget, floored at [`MIN_PROTOCOL_TIMEOUT`] and never above the budget
/// itself; it scales with `MERON_SYNC_TIMEOUT` for the same reason the connect
/// budget does, so a DavMail-style LOGIN that needs 15s is not capped into
/// permanent failure that no setting can lift. The stages run in sequence under
/// [`imap_connect_timeout`], which stays the real ceiling for the whole connect.
fn imap_protocol_timeout() -> std::time::Duration {
    protocol_timeout_for(imap_connect_timeout())
}

fn protocol_timeout_for(connect: std::time::Duration) -> std::time::Duration {
    (connect / 3).max(MIN_PROTOCOL_TIMEOUT).min(connect)
}

/// Cap per resolved address: one black-hole address (typically an unroutable
/// IPv6 route ahead of a fine IPv4 one) must not eat the whole connect budget.
const CONNECT_ATTEMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Delay before the second of the two initially-dialed addresses. Alternating
/// families and staggering the opening attempts follows Happy Eyeballs'
/// important property: a broken preferred family cannot hold a working fallback
/// behind several full connection timeouts. Attempts that replace a failed one
/// start immediately.
const CONNECT_ATTEMPT_STAGGER: std::time::Duration = std::time::Duration::from_millis(250);

fn interleave_address_families(addrs: Vec<std::net::SocketAddr>) -> Vec<std::net::SocketAddr> {
    let prefer_ipv6 = addrs.first().is_some_and(std::net::SocketAddr::is_ipv6);
    let (ipv6, ipv4): (Vec<_>, Vec<_>) = addrs.into_iter().partition(|addr| addr.is_ipv6());
    let (preferred, fallback) = if prefer_ipv6 {
        (ipv6, ipv4)
    } else {
        (ipv4, ipv6)
    };
    let mut preferred = preferred.into_iter();
    let mut fallback = fallback.into_iter();
    let mut interleaved = Vec::new();
    loop {
        let mut added = false;
        if let Some(addr) = preferred.next() {
            interleaved.push(addr);
            added = true;
        }
        if let Some(addr) = fallback.next() {
            interleaved.push(addr);
            added = true;
        }
        if !added {
            break;
        }
    }
    interleaved
}

/// Resolve `host` and connect to each address in resolver order with a short
/// per-attempt cap. Logs slow stages so a stalling first sync can be traced to
/// DNS vs TCP from device logs alone.
pub(crate) async fn connect_tcp(host: &str, port: u16) -> Result<TcpStream> {
    tokio::time::timeout(TCP_CONNECT_TIMEOUT, connect_tcp_inner(host, port))
        .await
        .map_err(|_| {
            anyhow!(
                "TCP connect timed out after {}s",
                TCP_CONNECT_TIMEOUT.as_secs()
            )
        })?
}

async fn connect_tcp_inner(host: &str, port: u16) -> Result<TcpStream> {
    let dns_started = std::time::Instant::now();
    let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host, port))
        .await
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
    let mut pending = interleave_address_families(addrs).into_iter();
    let mut attempts = tokio::task::JoinSet::new();
    let spawn_attempt = |attempts: &mut tokio::task::JoinSet<_>, addr, delay| {
        attempts.spawn(async move {
            tokio::time::sleep(delay).await;
            let started = std::time::Instant::now();
            let result =
                tokio::time::timeout(CONNECT_ATTEMPT_TIMEOUT, TcpStream::connect(addr)).await;
            (addr, started.elapsed(), result)
        });
    };
    if let Some(addr) = pending.next() {
        spawn_attempt(&mut attempts, addr, std::time::Duration::ZERO);
    }
    if let Some(addr) = pending.next() {
        spawn_attempt(&mut attempts, addr, CONNECT_ATTEMPT_STAGGER);
    }

    // Replacement attempts start as soon as a slot frees: the stagger exists to
    // avoid firing both families at once, not to add delay after an attempt has
    // already failed. Re-delaying here would push later addresses past
    // TCP_CONNECT_TIMEOUT and leave a reachable third address never tried.
    let mut last_err = anyhow!("dns lookup: no addresses for {host}");
    while let Some(attempt) = attempts.join_next().await {
        let (addr, elapsed, result) = match attempt {
            Ok(attempt) => attempt,
            Err(error) => {
                last_err = anyhow::Error::new(error).context("tcp connect task");
                if let Some(addr) = pending.next() {
                    spawn_attempt(&mut attempts, addr, std::time::Duration::ZERO);
                }
                continue;
            }
        };
        match result {
            Ok(Ok(tcp)) => {
                if elapsed.as_millis() > 1_000 {
                    crate::mlog!(
                        crate::log::Level::Warn,
                        "net",
                        "slow TCP connect to {addr}: {}ms",
                        elapsed.as_millis()
                    );
                }
                attempts.abort_all();
                return Ok(tcp);
            }
            Ok(Err(err)) => last_err = anyhow::Error::new(err).context(format!("connect {addr}")),
            Err(_) => {
                crate::mlog!(
                    crate::log::Level::Warn,
                    "net",
                    "connect to {addr} timed out after {}ms",
                    elapsed.as_millis()
                );
                last_err = anyhow!("connect {addr}: timed out");
            }
        }
        if let Some(addr) = pending.next() {
            spawn_attempt(&mut attempts, addr, std::time::Duration::ZERO);
        }
    }
    Err(last_err.context("tcp connect"))
}

/// Open a TCP connection, optionally wrapped in implicit TLS. Shared by the
/// IMAP and SMTP paths.
///
/// `proxy` is the account's resolved proxy (see [`crate::proxy`]); `None`
/// connects directly. TLS is negotiated with the real destination host either
/// way, so the tunnel never terminates the mail server's certificate.
pub async fn connect_stream(
    host: &str,
    port: u16,
    tls: bool,
    proxy: Option<&crate::proxy::ProxyConfig>,
    cert_pin: Option<&str>,
) -> Result<Stream> {
    let tcp = open_socket(host, port, proxy).await?;
    if tls {
        Ok(Stream::Tls(Box::new(
            upgrade_to_tls(host, tcp, cert_pin).await?,
        )))
    } else {
        Ok(Stream::Plain(tcp))
    }
}

/// Connect the TCP socket (directly or through the proxy) and set the
/// keepalives every mail connection wants: NAT and firewalls drop idle IMAP
/// sessions silently otherwise, and an IDLE watcher never notices.
pub(crate) async fn open_socket(
    host: &str,
    port: u16,
    proxy: Option<&crate::proxy::ProxyConfig>,
) -> Result<TcpStream> {
    let tcp = match proxy {
        Some(proxy) => crate::proxy::connect_through(proxy, host, port).await?,
        None => connect_tcp(host, port).await?,
    };
    let sock_ref = socket2::SockRef::from(&tcp);
    let keepalive = socket2::TcpKeepalive::new()
        .with_time(std::time::Duration::from_secs(60))
        .with_interval(std::time::Duration::from_secs(10));
    let _ = sock_ref.set_tcp_keepalive(&keepalive);
    Ok(tcp)
}

/// Wrap an established plaintext TCP socket in a TLS session. Used both by the
/// implicit-TLS path above and by the STARTTLS upgrade, which hands us the raw
/// socket after the cleartext negotiation completes.
pub async fn upgrade_to_tls(
    host: &str,
    tcp: TcpStream,
    cert_pin: Option<&str>,
) -> Result<TlsStream<TcpStream>> {
    let connector = crate::tls::connector(cert_pin)?;
    let server_name =
        rustls::pki_types::ServerName::try_from(host.to_string()).context("invalid server name")?;
    let handshake_started = std::time::Instant::now();
    let result = tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, connector.connect(server_name, tcp))
        .await
        .map_err(|_| anyhow::anyhow!("timed out"))
        .context("tls handshake")?;
    let handshake_ms = handshake_started.elapsed().as_millis();
    if handshake_ms > 1_000 {
        crate::mlog!(
            crate::log::Level::Warn,
            "net",
            "slow TLS handshake with {host}: {handshake_ms}ms"
        );
    }
    // Certificate rejections come back tagged, so the account dialog can offer
    // to show the certificate and pin it rather than dead-ending on a rustls
    // error. See [`crate::tls::UntrustedCertificate`].
    result.map_err(crate::tls::UntrustedCertificate::from_io)
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

/// Drive an IMAP session up to the point where the socket is ready for the TLS
/// handshake, and hand back the raw socket. Used only by the certificate probe
/// (see [`crate::tls::probe`]); no credentials are ever sent over it.
pub(crate) async fn starttls_socket(tcp: TcpStream) -> Result<TcpStream> {
    let mut client = async_imap::Client::new(Stream::Plain(tcp));
    tokio::time::timeout(imap_protocol_timeout(), client.read_response())
        .await
        .map_err(|_| anyhow!("timed out"))
        .context("read IMAP greeting")?
        .context("read IMAP greeting")?
        .context("server closed before greeting")?;
    client
        .run_command_and_check_ok("STARTTLS", None)
        .await
        .context("STARTTLS")?;
    match client.into_inner() {
        Stream::Plain(tcp) => Ok(tcp),
        Stream::Tls(_) => Err(anyhow!("STARTTLS requested on an already-TLS stream")),
    }
}

pub async fn connect(creds: &Creds) -> Result<Session> {
    let timeout = imap_connect_timeout();
    tokio::time::timeout(timeout, connect_inner(creds))
        .await
        .map_err(|_| anyhow!("IMAP connect timed out after {}ms", timeout.as_millis()))?
}

async fn connect_inner(creds: &Creds) -> Result<Session> {
    // STARTTLS connects in cleartext and upgrades after the greeting; implicit
    // TLS wraps the socket up front. Plaintext (neither flag) is for local test
    // servers only.
    let implicit_tls = creds.tls && !creds.starttls;
    let stream = connect_stream(
        &creds.host,
        creds.port,
        implicit_tls,
        creds.proxy.resolve().as_ref(),
        creds.cert_pin.as_deref(),
    )
    .await?;
    let mut client = async_imap::Client::new(stream);
    // Consume the server greeting (e.g. "* OK ... ready"). Client::new does not
    // read it, and an unconsumed greeting shifts every response by one. That is
    // harmless for LOGIN (untagged lines are skipped), but it makes the XOAUTH2
    // handshake deadlock: the greeting is mistaken for the auth result and the
    // server's "+" continuation gets swallowed while we wait for a tagged reply.
    let greeting_started = std::time::Instant::now();
    tokio::time::timeout(imap_protocol_timeout(), client.read_response())
        .await
        .map_err(|_| anyhow!("timed out"))
        .context("read IMAP greeting")?
        .context("read IMAP greeting")?
        .context("server closed before greeting")?;
    let greeting_ms = greeting_started.elapsed().as_millis();
    if greeting_ms > 1_000 {
        crate::mlog!(
            crate::log::Level::Warn,
            "net",
            "slow IMAP greeting from {}: {greeting_ms}ms",
            creds.host
        );
    }

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
        let tls = upgrade_to_tls(&creds.host, tcp, creds.cert_pin.as_deref()).await?;
        client = async_imap::Client::new(Stream::Tls(Box::new(tls)));
    }

    let auth_started = std::time::Instant::now();
    let session_result = if creds.is_oauth() {
        let auth = XOAuth2Simple::new(&creds.user, creds.access_token.as_deref().unwrap_or(""));
        tokio::time::timeout(
            imap_protocol_timeout(),
            client.authenticate("XOAUTH2", auth),
        )
        .await
        .map_err(|_| anyhow!("oauth login timed out"))?
        .map_err(|(e, _)| anyhow!("oauth login failed: {e}"))
    } else {
        tokio::time::timeout(
            imap_protocol_timeout(),
            client.login(&creds.user, &creds.password),
        )
        .await
        .map_err(|_| anyhow!("login timed out"))?
        .map_err(|(e, _)| anyhow!("login failed: {e}"))
    };
    let session = session_result?;
    let auth_ms = auth_started.elapsed().as_millis();
    if auth_ms > 1_000 {
        crate::mlog!(
            crate::log::Level::Warn,
            "net",
            "slow IMAP authentication with {}: {auth_ms}ms",
            creds.host
        );
    }
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
    proxy: Option<crate::proxy::ProxyConfig>,
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
        let mut resp = crate::proxy::agent_for(proxy.as_ref())?
            .post(&token_url)
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
            display_name: crate::utf7::decode(name.name()),
            name: name.name().to_string(),
            delimiter: name.delimiter().map(|d| d.to_string()),
            unread: 0,
            special_use,
            role: String::new(),
        });
    }
    Ok(out)
}

pub async fn create_folder(session: &mut Session, name: &str) -> Result<()> {
    session.create(name).await.context("CREATE")?;
    Ok(())
}

/// A multi-folder delete that failed after removing part of its target list.
/// Callers must reconcile `removed` before surfacing the failure.
#[derive(Debug)]
pub struct PartialFolderDelete {
    pub removed: Vec<String>,
    failure: String,
}

impl std::fmt::Display for PartialFolderDelete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} folder(s) were deleted before the server rejected the operation: {}",
            self.removed.len(),
            self.failure
        )
    }
}

impl std::error::Error for PartialFolderDelete {}

impl PartialFolderDelete {
    pub fn into_parts(self) -> (Vec<String>, String) {
        let warning = self.to_string();
        (self.removed, warning)
    }
}

/// Move a reused session away from any mailbox that is about to be deleted.
/// This is a read-only preflight so the session pool may safely repeat it on a
/// fresh connection when a warm connection has gone stale.
pub async fn prepare_folder_delete(session: &mut Session) -> Result<()> {
    // Pooled sessions retain their selected mailbox. Some servers refuse to
    // delete that mailbox, so safely move the session to INBOX first. EXAMINE
    // implicitly deselects without expunging messages marked \Deleted.
    session
        .examine("INBOX")
        .await
        .context("EXAMINE INBOX before DELETE")?;
    Ok(())
}

/// Delete folders on the server, messages and all. The caller must first run
/// [`prepare_folder_delete`] on the same session. `names` is deleted in order,
/// so a subtree must arrive deepest first — RFC 3501 lets a server refuse DELETE
/// on a mailbox that still has inferiors. Returns the names actually removed;
/// if a later command fails, the error carries the successful prefix so callers
/// can reconcile irreversible changes. Callers must refuse special-use folders
/// first: IMAP happily deletes Sent or Archive, and nothing on the server brings
/// them back.
pub async fn delete_folders(session: &mut Session, names: &[String]) -> Result<Vec<String>> {
    let mut removed = Vec::new();
    for name in names {
        if let Err(err) = session.delete(name).await {
            let failure = format!("DELETE {name}: {err:#}");
            if removed.is_empty() {
                return Err(anyhow::anyhow!(failure));
            }
            return Err(PartialFolderDelete { removed, failure }.into());
        }
        removed.push(name.clone());
    }
    Ok(removed)
}

/// Whether `err` is a FETCH response our IMAP parser could not read, as opposed
/// to a network or server failure.
///
/// This matters because such a response is unrecoverable *on that connection*:
/// async-imap reports the parse failure without consuming the offending bytes
/// from its read buffer, so every later poll re-parses them and fails again.
/// The only way forward is to drop the session and re-fetch a range that
/// excludes the message — see `fetch_headers_isolating_unparseable` in
/// `engine.rs`.
///
/// Detection is by message text because async-imap surfaces the failure as a
/// plain `io::Error::other`, with no variant to match on. The substring is the
/// literal from its `ImapStream::decode`; if the pinned fork moves, this needs
/// rechecking (`unparseable_response_marker_matches_async_imap` covers it).
pub fn is_unparseable_response(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.to_string().contains(UNPARSEABLE_RESPONSE_MARKER))
    })
}

const UNPARSEABLE_RESPONSE_MARKER: &str = " during parsing of ";

/// SELECT `folder` and return `(UIDVALIDITY, UIDNEXT, uids)`, where `uids` holds
/// the most recent `limit` messages newest first — the same window
/// [`fetch_recent`] covers, but established without fetching any message.
///
/// Used by the poison-message recovery path, which needs the window as UIDs it
/// can subdivide. A `UID SEARCH` reply is a list of bare numbers, so it cannot
/// itself hit the parse failure that sent us down this path.
pub async fn recent_uids(
    session: &mut Session,
    folder: &str,
    limit: u32,
) -> Result<(u32, u32, Vec<u32>)> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let uidvalidity = mailbox.uid_validity.unwrap_or(0);
    let uid_next = mailbox.uid_next.unwrap_or(0);
    let set: HashSet<u32> = session.uid_search("ALL").await.context("UID SEARCH ALL")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    uids.truncate(limit.max(1) as usize);
    Ok((uidvalidity, uid_next, uids))
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
        let ef = fetch.header().map(header_fields).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            &ef.message_id,
            &ef.in_reply_to,
            &ef.references_root,
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
            gmail_msg_id: fetch.gmail_msg_id().copied(),
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

pub async fn search_uids(session: &mut Session, folder: &str, query: &str) -> Result<Vec<u32>> {
    session.select(folder).await.context("SELECT")?;
    let gmail = supports_gmail_ext(session).await;
    // SEARCH keys are US-ASCII unless the command names a charset (RFC 3501
    // §6.4.4), so anything non-ASCII — a CJK or accented query — has to be
    // announced as UTF-8 or the server is entitled to answer BAD.
    let needs_charset = !query.is_ascii();
    let mut result = session
        .uid_search(search_criteria(gmail, query, needs_charset))
        .await;
    if result.is_err() && needs_charset {
        // Servers that reject CHARSET outright (or that advertise UTF8=ACCEPT
        // and take the raw octets) get one retry with the bare criteria before
        // we give up and leave the caller with the cached hits.
        result = session
            .uid_search(search_criteria(gmail, query, false))
            .await;
    }
    let set: HashSet<u32> = result.context("UID SEARCH query")?;
    let mut uids: Vec<u32> = set.into_iter().collect();
    uids.sort_unstable_by(|a, b| b.cmp(a));
    Ok(uids)
}

/// Build the SEARCH criteria for a text `query`.
///
/// On Gmail, defer to its own search engine via X-GM-RAW: it understands the
/// full Gmail query syntax (operators like `from:`, `has:attachment`,
/// `older_than:`, relevance) instead of our crude substring OR. Elsewhere fall
/// back to plain SUBJECT/FROM/TEXT matching.
fn search_criteria(gmail: bool, query: &str, charset: bool) -> String {
    let q = imap_quote(query);
    let keys = if gmail {
        format!("X-GM-RAW {q}")
    } else {
        format!("OR OR SUBJECT {q} FROM {q} TEXT {q}")
    };
    if charset {
        format!("CHARSET UTF-8 {keys}")
    } else {
        keys
    }
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
        let ef = fetch.header().map(header_fields).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            &ef.message_id,
            &ef.in_reply_to,
            &ef.references_root,
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
                gmail_msg_id: fetch.gmail_msg_id().copied(),
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
        let ef = fetch.header().map(header_fields).unwrap_or_default();
        let thread_key = thread_key(
            fetch.gmail_thread_id().copied(),
            &ef.message_id,
            &ef.in_reply_to,
            &ef.references_root,
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
            gmail_msg_id: fetch.gmail_msg_id().copied(),
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
/// baseline or UIDVALIDITY changed, and is a no-op on servers that do not
/// advertise CONDSTORE — the capability must be checked rather than probed.
/// Proton Mail Bridge answers `SELECT "INBOX" (CONDSTORE)` with an *untagged*
/// `BAD` (" BAD [Error offset=17]: expected CR"), so the client never matches a
/// response to its tag: instead of erroring, every later command on that
/// connection reads the previous command's reply. The desynced session then
/// reports empty mailboxes and an empty folder LIST while still looking
/// healthy, so it goes back into the pool and poisons the account's syncs.
pub async fn sync_flags(
    session: &mut Session,
    folder: &str,
    since_modseq: u64,
    validity_matches: bool,
) -> Result<FlagSync> {
    if !supports_condstore(session).await {
        session.select(folder).await.context("SELECT")?;
        return Ok(FlagSync {
            highest_modseq: 0,
            changes: Vec::new(),
        });
    }
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

/// SELECT `folder` as the preflight of a flag update. Pooled sessions die
/// silently, and the SELECT is the first command that notices, so running it
/// as a separate phase lets a stale connection be replaced before any STORE
/// reaches the server (see `Engine::with_preflighted_write_session`).
pub async fn prepare_flag_update(session: &mut Session, folder: &str) -> Result<()> {
    session.select(folder).await.context("SELECT")?;
    Ok(())
}

async fn store_flag(session: &mut Session, uids: &[u32], op: &str) -> Result<()> {
    let uid_set = uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut stream = session
        .uid_store(uid_set, op)
        .await
        .context("UID STORE FLAGS.SILENT")?;
    while let Some(item) = stream.next().await {
        item.context("UID STORE item")?;
    }
    Ok(())
}

/// Add or remove the `\Seen` flag on a set of UIDs already in the selected
/// mailbox. The caller must first run [`prepare_flag_update`] on the session.
pub async fn store_seen(session: &mut Session, uids: &[u32], seen: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let op = if seen {
        "+FLAGS.SILENT (\\Seen)"
    } else {
        "-FLAGS.SILENT (\\Seen)"
    };
    store_flag(session, uids, op).await
}

/// Add or remove the `\Flagged` flag on a set of UIDs already in the selected
/// mailbox. The caller must first run [`prepare_flag_update`] on the session.
pub async fn store_starred(session: &mut Session, uids: &[u32], starred: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let op = if starred {
        "+FLAGS.SILENT (\\Flagged)"
    } else {
        "-FLAGS.SILENT (\\Flagged)"
    };
    store_flag(session, uids, op).await
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

/// Permanently delete every message in `folder`: mark the whole mailbox
/// `\Deleted` by sequence set, then EXPUNGE. Used by "empty trash"/"empty junk",
/// which must clear the server folder itself rather than the UIDs we happen to
/// have cached. Returns the message count reported by SELECT.
pub async fn empty_folder(session: &mut Session, folder: &str) -> Result<u32> {
    let mailbox = session.select(folder).await.context("SELECT")?;
    let total = mailbox.exists;
    if total == 0 {
        return Ok(0);
    }
    let mut stream = session
        .store("1:*", "+FLAGS.SILENT (\\Deleted)")
        .await
        .context("STORE Deleted")?;
    while let Some(item) = stream.next().await {
        item.context("STORE item")?;
    }
    drop(stream);

    let estream = session.expunge().await.context("EXPUNGE")?;
    futures::pin_mut!(estream);
    while let Some(item) = estream.next().await {
        item.context("EXPUNGE item")?;
    }
    Ok(total)
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

pub fn looks_like_archive(name: &str) -> bool {
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

pub fn looks_like_trash(name: &str) -> bool {
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

pub fn looks_like_junk(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "junk" | "spam" | "bulk mail" | "inbox.junk" | "inbox.spam" | "[gmail]/spam"
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

/// Message fields parsed out of the fetched RFC822 header. A named struct
/// rather than a tuple because there are enough of them (and the recipient
/// lists) that positional destructuring at the call sites would be error-prone.
#[derive(Default)]
struct HeaderFields {
    subject: String,
    from_name: String,
    from_addr: String,
    date: i64,
    message_id: String,
    in_reply_to: String,
    /// First id of the `References` header, i.e. the thread root.
    references_root: String,
    to: Vec<Recipient>,
    cc: Vec<Recipient>,
}

/// Everything the message list needs, parsed from `RFC822.HEADER`.
///
/// The IMAP `ENVELOPE` carries the same fields, but imap-proto rejects any
/// quoted string holding a byte outside `0x01..=0x7F`, so a single message whose
/// server emitted a raw 8-bit display name (rather than a literal or an RFC 2047
/// encoded-word) fails to parse. async-imap leaves that response in its buffer
/// unconsumed, wedging the connection, and the next sync refetches the same
/// range and wedges again — one message stalls the whole folder forever. The
/// header arrives as a literal, copied out by length with no parsing, so it
/// cannot hit that. It is also what we already trusted over the envelope for the
/// subject: imap-proto hands back quoted-string contents without unescaping, so
/// an envelope subject containing double quotes arrived as `\"...\"` and, being
/// part of the compound thread key, split the conversation into a duplicate
/// thread.
fn header_fields(header: &[u8]) -> HeaderFields {
    let Ok((headers, _)) = mailparse::parse_headers(header) else {
        return HeaderFields::default();
    };
    let headers = headers.as_slice();
    let mut from = header_addrs(headers, "From");
    let (from_name, from_addr) = if from.is_empty() {
        (String::new(), String::new())
    } else {
        let first = from.remove(0);
        (first.name, first.addr)
    };
    HeaderFields {
        subject: headers.get_first_value("Subject").unwrap_or_default(),
        from_name,
        from_addr,
        date: headers
            .get_first_value("Date")
            .map(|raw| parse::parse_date_to_epoch(&raw))
            .unwrap_or_default(),
        message_id: headers
            .get_first_value("Message-ID")
            .map(|raw| normalize_message_id(&raw))
            .unwrap_or_default(),
        in_reply_to: headers
            .get_first_value("In-Reply-To")
            .as_deref()
            .and_then(first_message_id)
            .unwrap_or_default(),
        references_root: headers
            .get_first_value("References")
            .as_deref()
            .and_then(first_message_id)
            .unwrap_or_default(),
        to: header_addrs(headers, "To"),
        cc: header_addrs(headers, "Cc"),
    }
}

/// Flatten one address header into recipients, expanding RFC 5322 groups and
/// dropping entries that yield no email address. A header that fails to parse
/// contributes nothing rather than failing the message.
fn header_addrs(headers: &[mailparse::MailHeader<'_>], key: &str) -> Vec<Recipient> {
    let Some(header) = headers.get_first_header(key) else {
        return Vec::new();
    };
    let Ok(list) = mailparse::addrparse_header(header) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in list.iter() {
        match entry {
            mailparse::MailAddr::Single(info) => push_recipient(&mut out, info),
            mailparse::MailAddr::Group(group) => {
                for info in &group.addrs {
                    push_recipient(&mut out, info);
                }
            }
        }
    }
    out
}

fn push_recipient(out: &mut Vec<Recipient>, info: &mailparse::SingleInfo) {
    if info.addr.is_empty() {
        return;
    }
    out.push(Recipient {
        name: info.display_name.clone().unwrap_or_default(),
        addr: info.addr.clone(),
    });
}

/// FETCH item list. Adds `X-GM-THRID` on Gmail so messages thread by Gmail's
/// server-side thread id, and `BODY.PEEK[]` when the full message is wanted.
fn fetch_items(gmail: bool, body: bool) -> &'static str {
    match (gmail, body) {
        (true, true) => "(UID FLAGS RFC822.HEADER X-GM-MSGID X-GM-THRID BODY.PEEK[])",
        (true, false) => "(UID FLAGS RFC822.HEADER X-GM-MSGID X-GM-THRID)",
        (false, true) => "(UID FLAGS RFC822.HEADER BODY.PEEK[])",
        (false, false) => "(UID FLAGS RFC822.HEADER)",
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

/// Whether the server advertises CONDSTORE (RFC 7162), i.e. accepts
/// `SELECT ... (CONDSTORE)` and `FETCH ... (CHANGEDSINCE n)`. Absent it, the
/// flag reconciliation in [`sync_flags`] must not be attempted at all: a server
/// that rejects the parameter without a properly tagged response leaves the
/// connection unusable (see the note there).
async fn supports_condstore(session: &mut Session) -> bool {
    match session.capabilities().await {
        Ok(caps) => caps.has_str("CONDSTORE"),
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

/// The first RFC message id in a header value that may hold several, such as
/// `References` (thread root first) or an `In-Reply-To` carrying more than one.
///
/// Angle brackets decide where the id starts, not whitespace: RFC 822 allowed a
/// leading phrase, and senders still emit `In-Reply-To: Your message of Tuesday
/// <parent@example.com>`. Splitting on whitespace first would take `Your` as the
/// id and, absent `References`, thread every such message together. Only a value
/// with no brackets at all falls back to its first whitespace-separated token.
fn first_message_id(value: &str) -> Option<String> {
    if value.contains('<') {
        let id = normalize_message_id(value);
        return (!id.is_empty()).then_some(id);
    }
    value
        .split_whitespace()
        .map(normalize_message_id)
        .find(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_PROTOCOL_TIMEOUT, civil_from_days, first_message_id, header_fields, imap_quote,
        interleave_address_families, looks_like_drafts, message_id_search_criteria,
        normalize_message_id, protocol_timeout_for, search_criteria, thread_key,
    };

    #[test]
    fn protocol_stage_timeout_leaves_room_for_a_slow_but_healthy_login() {
        // The default 30s sync budget yields a 24s connect budget; an even
        // share of that (8s) is under Gmail's observed 4-7s XOAUTH2 exchange.
        assert_eq!(
            protocol_timeout_for(std::time::Duration::from_secs(24)),
            MIN_PROTOCOL_TIMEOUT
        );
        // A raised MERON_SYNC_TIMEOUT still scales the share up past the floor.
        assert_eq!(
            protocol_timeout_for(std::time::Duration::from_secs(120)),
            std::time::Duration::from_secs(40)
        );
        // A budget below the floor caps the stage at the budget itself.
        assert_eq!(
            protocol_timeout_for(std::time::Duration::from_secs(4)),
            std::time::Duration::from_secs(4)
        );
    }

    #[test]
    fn connect_addresses_alternate_families_without_overriding_resolver_preference() {
        let v4a = "192.0.2.1:993".parse().unwrap();
        let v4b = "192.0.2.2:993".parse().unwrap();
        let v6a = "[2001:db8::1]:993".parse().unwrap();
        let v6b = "[2001:db8::2]:993".parse().unwrap();

        assert_eq!(
            interleave_address_families(vec![v6a, v6b, v4a, v4b]),
            vec![v6a, v4a, v6b, v4b]
        );
        assert_eq!(
            interleave_address_families(vec![v4a, v4b, v6a, v6b]),
            vec![v4a, v6a, v4b, v6b]
        );
    }

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
    fn header_fields_reads_references_root_from_folded_header() {
        // A reply-to-a-reply: References spans two folded lines, root first.
        let header = b"Subject: Re: test mailo 1\r\nReferences: <CAJ7M84+root@mail.gmail.com>\r\n\t<meron-second@mailo.com>\r\nIn-Reply-To: <meron-second@mailo.com>\r\n\r\n";
        let ef = header_fields(header);
        assert_eq!(ef.references_root, "CAJ7M84+root@mail.gmail.com");
        assert_eq!(ef.in_reply_to, "meron-second@mailo.com");
    }

    #[test]
    fn header_fields_decodes_subject_and_tolerates_absence() {
        // Quotes arrive verbatim; the envelope handed them back still escaped.
        let ef = header_fields(b"Subject: A \"quoted\" title\r\nFrom: x@y\r\n\r\n");
        assert_eq!(ef.subject, "A \"quoted\" title");
        let ef = header_fields(b"Subject: =?UTF-8?B?SGVsbMO2?=\r\n\r\n");
        assert_eq!(ef.subject, "Hell\u{f6}");
        // No / empty Subject header, and no header at all.
        assert!(header_fields(b"From: x@y\r\n\r\n").subject.is_empty());
        assert!(header_fields(b"Subject:\r\n\r\n").subject.is_empty());
        assert!(header_fields(b"").subject.is_empty());
    }

    #[test]
    fn header_fields_parses_addresses_including_8bit_and_groups() {
        // The raw 8-bit display name that made the ENVELOPE unparseable.
        let header = "From: \"M\u{fc}ller\" <support@example.com>\r\nTo: a@x.com, Bob <b@x.com>\r\nCc: Team: c@x.com, d@x.com;\r\nDate: Sun, 12 Jul 2026 17:01:36 +0000\r\nMessage-ID: <abc@x.com>\r\n\r\n".as_bytes();
        let ef = header_fields(header);
        assert_eq!(ef.from_name, "M\u{fc}ller");
        assert_eq!(ef.from_addr, "support@example.com");
        assert_eq!(ef.message_id, "abc@x.com");
        assert_ne!(ef.date, 0);
        assert_eq!(
            ef.to.iter().map(|r| r.addr.as_str()).collect::<Vec<_>>(),
            ["a@x.com", "b@x.com"]
        );
        assert_eq!(ef.to[1].name, "Bob");
        // A group list contributes its members, not the group name.
        assert_eq!(
            ef.cc.iter().map(|r| r.addr.as_str()).collect::<Vec<_>>(),
            ["c@x.com", "d@x.com"]
        );
    }

    /// Minimal IMAP server: greets, accepts any login, reports a one-message
    /// INBOX, and answers FETCH with `fetch_reply`, in which `{tag}` stands for
    /// the command tag. A `fetch_reply` with no `{tag}` is never completed: the
    /// server hangs up after writing it, standing in for a dropped connection.
    async fn serve_canned(listener: tokio::net::TcpListener, fetch_reply: &'static str) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let Ok((sock, _)) = listener.accept().await else {
            return;
        };
        let (reader, mut writer) = sock.into_split();
        if writer.write_all(b"* OK IMAP4rev1 ready\r\n").await.is_err() {
            return;
        }
        let mut lines = tokio::io::BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut parts = line.splitn(2, ' ');
            let tag = parts.next().unwrap_or_default().to_string();
            let cmd = parts
                .next()
                .unwrap_or_default()
                .split(' ')
                .next()
                .unwrap_or_default()
                .to_uppercase();
            let reply = match cmd.as_str() {
                "CAPABILITY" => {
                    format!("* CAPABILITY IMAP4rev1\r\n{tag} OK CAPABILITY completed\r\n")
                }
                "SELECT" => format!(
                    "* 1 EXISTS\r\n* OK [UIDVALIDITY 42] ok\r\n* OK [UIDNEXT 1692] ok\r\n\
                     {tag} OK [READ-WRITE] SELECT completed\r\n"
                ),
                "FETCH" if !fetch_reply.contains("{tag}") => {
                    let _ = writer.write_all(fetch_reply.as_bytes()).await;
                    return;
                }
                "FETCH" => fetch_reply.replace("{tag}", &tag),
                _ => format!("{tag} OK {cmd} completed\r\n"),
            };
            if writer.write_all(reply.as_bytes()).await.is_err() {
                return;
            }
        }
    }

    async fn fetch_recent_against(fetch_reply: &'static str) -> anyhow::Error {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve_canned(listener, fetch_reply));

        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut client = async_imap::Client::new(super::Stream::Plain(tcp));
        client.read_response().await.unwrap().unwrap();
        let mut session = client.login("u", "p").await.map_err(|(e, _)| e).unwrap();
        match super::fetch_recent(&mut session, "INBOX", 50).await {
            Ok(batch) => panic!(
                "canned reply should not produce a batch ({} messages)",
                batch.messages.len()
            ),
            Err(err) => err,
        }
    }

    /// The marker `is_unparseable_response` looks for is a string emitted by our
    /// pinned async-imap fork, so pin it against the real thing rather than
    /// against a hand-built error.
    #[tokio::test]
    async fn unparseable_response_marker_matches_async_imap() {
        // A raw 8-bit byte inside an IMAP quoted string: imap-proto's `quoted`
        // only accepts 0x01..=0x7F, so it rejects the whole response. This is
        // the shape from the original report; any response the parser refuses
        // would do.
        let err = fetch_recent_against(
            "* 1 FETCH (UID 1691 FLAGS (\\Seen) ENVELOPE (\"Sun, 12 Jul 2026 17:01:36 +0000\" \
             \"Re: caf\u{e9}\" NIL NIL NIL NIL NIL NIL NIL \"<c@d>\"))\r\n\
             {tag} OK FETCH completed\r\n",
        )
        .await;
        assert!(
            super::is_unparseable_response(&err),
            "should be recognised as unparseable: {err:#}"
        );
    }

    /// A network failure must not be mistaken for a poison message —
    /// re-fetching narrower ranges would just repeat the refusal.
    #[tokio::test]
    async fn a_dropped_connection_is_not_treated_as_unparseable() {
        // Server hangs up part-way through a perfectly well-formed response.
        let err = fetch_recent_against("* 1 FETCH (UID 1691 FLAGS (\\See").await;
        assert!(
            !super::is_unparseable_response(&err),
            "a dropped connection should not look unparseable: {err:#}"
        );
        let err = anyhow::anyhow!(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
            .context("FETCH item");
        assert!(!super::is_unparseable_response(&err));
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
        // RFC 822 phrase before the id: brackets win over whitespace, or the
        // id would come out as "Your" and thread unrelated mail together.
        assert_eq!(
            first_message_id("Your message of Tuesday <parent@example.com>").as_deref(),
            Some("parent@example.com")
        );
        // No brackets anywhere: fall back to the first token.
        assert_eq!(
            first_message_id("bare@host trailing-junk").as_deref(),
            Some("bare@host")
        );
    }

    #[test]
    fn header_fields_reads_a_phrase_prefixed_in_reply_to() {
        let header = b"In-Reply-To: Your message of Tuesday <parent@example.com>\r\n\r\n";
        let ef = header_fields(header);
        assert_eq!(ef.in_reply_to, "parent@example.com");
        // Same obsolete syntax in References must not poison the thread root.
        let header = b"References: Your message of Tuesday <root@example.com>\r\n\r\n";
        assert_eq!(header_fields(header).references_root, "root@example.com");
    }

    #[test]
    fn imap_quote_escapes_quotes_backslashes_and_newlines() {
        assert_eq!(imap_quote("plain"), "\"plain\"");
        assert_eq!(imap_quote(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(imap_quote("a\r\nb"), "\"a  b\"");
    }

    #[test]
    fn search_criteria_announces_utf8_only_when_asked() {
        assert_eq!(
            search_criteria(false, "plan", false),
            "OR OR SUBJECT \"plan\" FROM \"plan\" TEXT \"plan\""
        );
        assert_eq!(search_criteria(true, "plan", false), "X-GM-RAW \"plan\"");
        // Non-ASCII queries need the charset; the retry drops it again.
        assert_eq!(
            search_criteria(true, "会議", true),
            "CHARSET UTF-8 X-GM-RAW \"会議\""
        );
        assert_eq!(
            search_criteria(false, "会議", true),
            "CHARSET UTF-8 OR OR SUBJECT \"会議\" FROM \"会議\" TEXT \"会議\""
        );
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
