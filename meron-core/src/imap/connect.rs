use super::*;

/// Connection stream: implicit TLS (port 993 etc.) or plaintext (e.g. a local
/// test server on 3143). One enum so the async-imap `Session` type is uniform.
/// STARTTLS upgrade is not yet supported.
#[derive(Debug)]
pub enum Stream {
    Plain(TcpStream, ReadProgress),
    Tls(Box<TlsStream<TcpStream>>, ReadProgress),
}

pub(crate) type ReadProgress = std::sync::Arc<std::sync::Mutex<Option<tokio::time::Instant>>>;

impl Stream {
    pub fn plain(socket: TcpStream) -> Self {
        Self::Plain(socket, Default::default())
    }

    pub fn tls(socket: Box<TlsStream<TcpStream>>) -> Self {
        Self::Tls(socket, Default::default())
    }

    pub(crate) fn observe_reads(&mut self, progress: ReadProgress) {
        match self {
            Self::Plain(_, clock) | Self::Tls(_, clock) => *clock = progress,
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let (result, progress) = match self.get_mut() {
            Stream::Plain(s, progress) => (Pin::new(s).poll_read(cx, buf), progress),
            Stream::Tls(s, progress) => (Pin::new(s.as_mut()).poll_read(cx, buf), progress),
        };
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() > before {
            let mut clock = progress.lock().unwrap();
            if clock.is_some() {
                *clock = Some(tokio::time::Instant::now());
            }
        }
        result
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Stream::Plain(s, _) => Pin::new(s).poll_write(cx, buf),
            Stream::Tls(s, _) => Pin::new(s.as_mut()).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s, _) => Pin::new(s).poll_flush(cx),
            Stream::Tls(s, _) => Pin::new(s.as_mut()).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s, _) => Pin::new(s).poll_shutdown(cx),
            Stream::Tls(s, _) => Pin::new(s.as_mut()).poll_shutdown(cx),
        }
    }
}

pub type Session = async_imap::Session<Stream>;

/// Whether an idle session's socket still looks open. A server that drops an
/// idle connection leaves its FIN (often behind a BYE or TLS close_notify)
/// queued on the socket, and the runtime's reactor records that as read-closed
/// readiness, so a dead pooled session can be spotted without a round trip.
/// This cannot see a close that lands after the check; callers still need to
/// handle a command failing on a dropped connection.
pub fn session_looks_open(session: &Session) -> bool {
    match session.get_ref() {
        Stream::Plain(stream, _) => tcp_looks_open(stream),
        Stream::Tls(stream, _) => tcp_looks_open(stream.get_ref().0),
    }
}

pub(super) fn tcp_looks_open(tcp: &TcpStream) -> bool {
    use futures::FutureExt;
    use tokio::io::Interest;

    match tcp.ready(Interest::READABLE).now_or_never() {
        Some(Ok(ready)) => !(ready.is_read_closed() || ready.is_error()),
        Some(Err(_)) => false,
        // Nothing pending on the socket: the connection is quiet, not closed.
        None => true,
    }
}

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
    /// Fingerprint ([`grant_fingerprint`]) of a refresh token the provider
    /// refused outright (an expired or revoked OAuth grant). Persisted, so the
    /// account reads as needing a reconnect across restarts instead of failing
    /// the same refresh on every sync. It names the token rather than the
    /// account on purpose: once the account holds any other token — however
    /// and whenever a reconnect's writes land — the verdict no longer applies.
    /// See [`Creds::auth_rejected`].
    pub rejected_grant: Option<String>,
}

/// A stable, non-reversible name for a refresh token, to remember which one a
/// provider refused without storing the token outside the secret store.
pub fn grant_fingerprint(refresh_token: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(refresh_token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

/// The server answered an XOAUTH2 login with NO: it refused the access token.
/// Displays like any other failed OAuth login.
#[derive(Debug)]
pub struct OAuthLoginRejected(String);

impl std::fmt::Display for OAuthLoginRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "oauth login failed: {}", self.0)
    }
}

impl std::error::Error for OAuthLoginRejected {}

/// The error for a failed XOAUTH2 login. Only the server's own NO is a refused
/// token; a connection that dropped or stalled mid-login says nothing about it.
pub(crate) fn oauth_login_error(error: async_imap::error::Error) -> anyhow::Error {
    match error {
        async_imap::error::Error::No(_) => {
            anyhow::Error::new(OAuthLoginRejected(error.to_string()))
        }
        other => anyhow!("oauth login failed: {other}"),
    }
}

/// Whether a failed [`connect`] is the server refusing the access token, as
/// opposed to the login timing out or the connection failing.
pub fn oauth_login_rejected(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| cause.is::<OAuthLoginRejected>())
}

impl Creds {
    /// Whether the provider has refused the refresh token these credentials
    /// hold, so that nothing short of signing in again will work.
    pub fn auth_rejected(&self) -> bool {
        match (
            self.rejected_grant.as_deref(),
            self.refresh_token.as_deref(),
        ) {
            (Some(rejected), Some(token)) if !token.is_empty() => {
                rejected == grant_fingerprint(token)
            }
            _ => false,
        }
    }

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
    /// The user opted this folder in to live sync and new-mail notifications.
    /// Populated by `store::get_folders`; INBOX is always watched regardless.
    #[serde(default)]
    pub notify: bool,
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
    /// The message is end-to-end encrypted (`Content-Type: multipart/encrypted`,
    /// or decrypted since). Only then is a `...` / `[...]` subject the
    /// protected-headers placeholder rather than a real subject.
    #[serde(skip)]
    pub encrypted: bool,
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
    /// Tags among the message's keywords, when this header came from a FETCH
    /// that read its FLAGS. `None` says nothing about them.
    #[serde(skip)]
    pub server_tags: Option<Vec<String>>,
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
    /// Attachment names from BODYSTRUCTURE; None means metadata was not fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<crate::store::CardFile>>,
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
pub(super) const TCP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Cap on the TLS handshake alone, for the same reason.
pub(super) const TLS_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// Total budget for DNS through authentication. Leave 20% of the configured
/// background-sync budget for connection coordination and the actual mailbox
/// command. `MERON_SYNC_TIMEOUT` is raised precisely for gateways whose
/// handshake is slow (see [`crate::engine::background_sync_timeout`]), so this
/// scales with it rather than pinning a ceiling such a gateway could never
/// finish under.
pub(super) fn imap_connect_timeout() -> std::time::Duration {
    crate::engine::background_sync_timeout().mul_f32(0.8)
}

/// Floor under [`imap_protocol_timeout`]. A stage cap is a fallback for a stage
/// that never completes, not a bar a slow-but-working server has to clear:
/// Gmail's XOAUTH2 exchange routinely takes 4-7s on a congested link, so an
/// even share of the default budget (8s) fails it often enough to keep the
/// connectivity banner up on an account that is fine. Keep the floor several
/// times the slowest healthy stage seen in the wild.
pub(super) const MIN_PROTOCOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// The TCP/TLS stages may complete while the server never sends its greeting or
/// never finishes authentication. Keep those protocol stages independently
/// bounded so callers receive the concrete failing stage rather than only an
/// outer operation-budget timeout. Each stage gets a share of the connect
/// budget, floored at [`MIN_PROTOCOL_TIMEOUT`] and never above the budget
/// itself; it scales with `MERON_SYNC_TIMEOUT` for the same reason the connect
/// budget does, so a DavMail-style LOGIN that needs 15s is not capped into
/// permanent failure that no setting can lift. The stages run in sequence under
/// [`imap_connect_timeout`], which stays the real ceiling for the whole connect.
pub(super) fn imap_protocol_timeout() -> std::time::Duration {
    protocol_timeout_for(imap_connect_timeout())
}

pub(super) fn protocol_timeout_for(connect: std::time::Duration) -> std::time::Duration {
    (connect / 3).max(MIN_PROTOCOL_TIMEOUT).min(connect)
}

/// Cap per resolved address: one black-hole address (typically an unroutable
/// IPv6 route ahead of a fine IPv4 one) must not eat the whole connect budget.
pub(super) const CONNECT_ATTEMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Delay before the second of the two initially-dialed addresses. Alternating
/// families and staggering the opening attempts follows Happy Eyeballs'
/// important property: a broken preferred family cannot hold a working fallback
/// behind several full connection timeouts. Attempts that replace a failed one
/// start immediately.
pub(super) const CONNECT_ATTEMPT_STAGGER: std::time::Duration =
    std::time::Duration::from_millis(250);

pub(super) fn interleave_address_families(
    addrs: Vec<std::net::SocketAddr>,
) -> Vec<std::net::SocketAddr> {
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

pub(super) async fn connect_tcp_inner(host: &str, port: u16) -> Result<TcpStream> {
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
        Ok(Stream::tls(Box::new(
            upgrade_to_tls(host, tcp, cert_pin).await?,
        )))
    } else {
        Ok(Stream::plain(tcp))
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
    pub(super) auth_string: String,
    pub(super) done: bool,
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
    let mut client = async_imap::Client::new(Stream::plain(tcp));
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
        Stream::Plain(tcp, _) => Ok(tcp),
        Stream::Tls(_, _) => Err(anyhow!("STARTTLS requested on an already-TLS stream")),
    }
}

pub async fn connect(creds: &Creds) -> Result<Session> {
    let timeout = imap_connect_timeout();
    tokio::time::timeout(timeout, connect_inner(creds))
        .await
        .map_err(|_| anyhow!("IMAP connect timed out after {}ms", timeout.as_millis()))?
}

pub(super) async fn connect_inner(creds: &Creds) -> Result<Session> {
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
            Stream::Plain(tcp, _) => tcp,
            Stream::Tls(_, _) => {
                return Err(anyhow!("STARTTLS requested on an already-TLS stream"));
            }
        };
        let tls = upgrade_to_tls(&creds.host, tcp, creds.cert_pin.as_deref()).await?;
        client = async_imap::Client::new(Stream::tls(Box::new(tls)));
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
        .map_err(|(e, _)| oauth_login_error(e))
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
pub(super) struct TokenResponse {
    pub(super) access_token: String,
    pub(super) expires_in: Option<i64>,
    pub(super) refresh_token: Option<String>,
}

/// What a successful [`refresh_oauth_token`] hands back.
pub struct RefreshedToken {
    pub access_token: String,
    /// Seconds until `access_token` expires.
    pub expires_in: i64,
    /// A replacement refresh token, when the provider rotates them. Microsoft
    /// issues one on every refresh and each has its own fixed lifetime, so the
    /// caller must keep this one: holding on to the token it sent would see the
    /// account's sign-in expire that long after it was first connected, however
    /// often it refreshed. Google usually sends none.
    pub refresh_token: Option<String>,
}

impl From<TokenResponse> for RefreshedToken {
    fn from(response: TokenResponse) -> Self {
        RefreshedToken {
            access_token: response.access_token,
            expires_in: response.expires_in.unwrap_or(3600),
            refresh_token: response
                .refresh_token
                .filter(|token| !token.trim().is_empty()),
        }
    }
}

/// A token endpoint's refusal of the refresh token itself: its `error` field
/// is `invalid_grant`, which RFC 6749 §5.2 reserves for a grant that is
/// expired, revoked or otherwise no longer valid. Retrying cannot succeed.
/// Displays like any other failed refresh.
#[derive(Debug)]
pub struct OAuthGrantRejected {
    pub(super) status: String,
    pub(super) body: String,
}

impl std::fmt::Display for OAuthGrantRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "oauth refresh failed ({}): {}", self.status, self.body)
    }
}

impl std::error::Error for OAuthGrantRejected {}

/// Whether a failed [`refresh_oauth_token`] means the grant itself is dead
/// rather than the request having failed in transit or on the server's side.
pub fn oauth_grant_rejected(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| cause.is::<OAuthGrantRejected>())
}

/// The error for a non-2xx token response. Only a body whose own `error` field
/// says `invalid_grant` is a rejection; the text turning up anywhere else (a
/// description, a gateway's error page) is not.
pub(super) fn oauth_refresh_error(status: &str, body: &str) -> anyhow::Error {
    let code = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|json| json["error"].as_str().map(str::to_string));
    if code.as_deref() == Some("invalid_grant") {
        return anyhow::Error::new(OAuthGrantRejected {
            status: status.to_string(),
            body: body.to_string(),
        });
    }
    anyhow!("oauth refresh failed ({status}): {body}")
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
) -> Result<RefreshedToken> {
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
            return Err(oauth_refresh_error(&status.to_string(), &body));
        }
        serde_json::from_str(&body).context("parse oauth response JSON")
    })
    .await
    .context("oauth refresh task")??;

    Ok(parsed.into())
}
