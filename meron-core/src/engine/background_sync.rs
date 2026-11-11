use std::future::Future;
use std::time::Duration;

use super::background_sync_timeout;

pub(crate) const BACKGROUND_SYNC_RETRY_DELAY: Duration = if cfg!(test) {
    Duration::ZERO
} else {
    Duration::from_secs(2)
};

pub(crate) fn is_transient_io_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::UnexpectedEof
    )
}

pub(crate) fn is_transient_sync_error(error: &anyhow::Error) -> bool {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<std::io::Error>()
            && is_transient_io_error(io_error)
        {
            return true;
        }
        if let Some(imap_error) = cause.downcast_ref::<async_imap::error::Error>() {
            match imap_error {
                async_imap::error::Error::ConnectionLost => return true,
                async_imap::error::Error::Io(io_error) if is_transient_io_error(io_error) => {
                    return true;
                }
                _ => {}
            }
        }
    }

    // Tokio timeouts and SOCKS reply codes are repository-generated anyhow
    // messages rather than typed sources. Keep this fallback narrow; protocol,
    // authentication, and certificate errors must not retry.
    let message = format!("{error:#}").to_ascii_lowercase();
    [
        "tcp connect",
        "dial tcp",
        "dns lookup",
        "timed out",
        "timeout",
        "network is unreachable",
        "network unreachable",
        "host unreachable",
        "connection refused",
        "connection reset",
        "connection abort",
        "connection closed",
        "connection lost",
        "broken pipe",
        "failed to lookup address",
        "no address associated with hostname",
        "server closed before greeting",
        "unexpected eof",
        "bytes remaining in stream",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

#[derive(Debug)]
pub struct BackgroundSyncCancelled;

impl std::fmt::Display for BackgroundSyncCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("background sync cancelled")
    }
}

impl std::error::Error for BackgroundSyncCancelled {}

#[derive(Debug)]
pub struct BackgroundSyncTimedOut(u64);

impl std::fmt::Display for BackgroundSyncTimedOut {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "timed out after {}s", self.0)
    }
}

impl std::error::Error for BackgroundSyncTimedOut {}

/// Retry a background read once when it fails for a recognizable transport
/// reason. Both attempts share the configured per-folder ceiling so a stuck
/// sync does not hold its dedup key indefinitely.
pub async fn retry_background_sync<T, C, F, Fut>(
    label: &str,
    can_attempt: C,
    operation: F,
) -> anyhow::Result<T>
where
    C: FnMut() -> bool,
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    retry_with_budget(label, can_attempt, operation, background_sync_timeout()).await
}

async fn retry_with_budget<T, C, F, Fut>(
    label: &str,
    mut can_attempt: C,
    mut operation: F,
    sync_timeout: Duration,
) -> anyhow::Result<T>
where
    C: FnMut() -> bool,
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    if !can_attempt() {
        return Err(anyhow::Error::new(BackgroundSyncCancelled));
    }
    let deadline = tokio::time::Instant::now() + sync_timeout;
    let first = tokio::time::timeout_at(deadline, operation()).await;
    let first_error = match first {
        Ok(Ok(value)) => return Ok(value),
        Ok(Err(error)) if is_transient_sync_error(&error) => error,
        Ok(Err(error)) => return Err(error),
        Err(_) => {
            return Err(anyhow::Error::new(BackgroundSyncTimedOut(
                sync_timeout.as_secs(),
            )));
        }
    };

    eprintln!("meron-core: {label} failed, retrying: {first_error:#}");
    if tokio::time::timeout_at(deadline, tokio::time::sleep(BACKGROUND_SYNC_RETRY_DELAY))
        .await
        .is_err()
    {
        // Keep the first attempt's cause as context: the budget expiring says
        // only that time ran out, and without this the caller (and the log) get
        // a bare "timed out" with no diagnosis. `Error::is` still finds the
        // typed error through the context chain, so the outer_timeout
        // classification is unaffected.
        return Err(
            anyhow::Error::new(BackgroundSyncTimedOut(sync_timeout.as_secs()))
                .context(format!("first attempt failed: {first_error:#}")),
        );
    }
    if !can_attempt() {
        return Err(anyhow::Error::new(BackgroundSyncCancelled));
    }

    match tokio::time::timeout_at(deadline, operation()).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(retry_error)) => Err(anyhow::anyhow!(
            "retry failed: {retry_error:#}; first attempt: {first_error:#}"
        )),
        Err(_) => Err(
            anyhow::Error::new(BackgroundSyncTimedOut(sync_timeout.as_secs()))
                .context(format!("first attempt failed: {first_error:#}")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{BackgroundSyncCancelled, is_transient_sync_error, retry_background_sync};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[tokio::test]
    async fn network_budget_expires_without_starting_persistence() {
        let result = super::retry_with_budget(
            "stalled network",
            || true,
            || std::future::pending::<anyhow::Result<()>>(),
            std::time::Duration::from_millis(5),
        )
        .await;
        assert!(result.unwrap_err().is::<super::BackgroundSyncTimedOut>());
    }

    #[tokio::test]
    async fn completed_network_budget_does_not_cancel_later_commit_or_notification() {
        let budget = std::time::Duration::from_millis(5);
        let messages = super::retry_with_budget(
            "network",
            || true,
            || async {
                anyhow::Ok(vec![crate::imap::MessageHeader {
                    uid: 20,
                    message_id: "arrival@example.com".into(),
                    ..Default::default()
                }])
            },
            budget,
        )
        .await
        .unwrap();
        let notification = tokio::task::spawn_blocking(move || {
            std::thread::sleep(budget * 4);
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            crate::store::run_migrations(&conn).unwrap();
            let arrivals =
                crate::store::classify_inbox_arrivals(&conn, "acct", 20, 21, &messages).unwrap();
            crate::store::upsert_messages(&conn, "acct", "INBOX", &messages).unwrap();
            crate::store::set_folder_state(&conn, "acct", "INBOX", 1, 21).unwrap();
            arrivals
        })
        .await
        .unwrap();
        assert_eq!(notification[0].uid, 20);
    }

    #[test]
    fn imap_disconnect_errors_are_transient() {
        let lost = anyhow::Error::new(async_imap::error::Error::ConnectionLost);
        assert!(is_transient_sync_error(&lost));

        let buffered_eof = anyhow::Error::new(async_imap::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "bytes remaining in stream",
        )));
        assert!(is_transient_sync_error(&buffered_eof));

        let rejected = anyhow::Error::new(async_imap::error::Error::No(
            "[AUTHENTICATIONFAILED] invalid credentials".to_string(),
        ));
        assert!(!is_transient_sync_error(&rejected));
    }

    #[test]
    fn only_a_refused_oauth_login_asks_for_an_early_token_renewal() {
        use async_imap::error::Error;
        let refused = crate::imap::oauth_login_error(Error::No(
            "[AUTHENTICATIONFAILED] Invalid credentials".to_string(),
        ))
        .context("sync INBOX");
        assert!(crate::imap::oauth_login_rejected(&refused));
        assert!(format!("{refused:#}").contains("oauth login failed: "));
        // A connection lost mid-login is not the server's verdict on the token.
        let dropped = crate::imap::oauth_login_error(Error::ConnectionLost);
        assert!(!crate::imap::oauth_login_rejected(&dropped));
        assert!(dropped.to_string().starts_with("oauth login failed: "));
    }

    #[test]
    fn socks_host_unreachable_is_transient() {
        let error = anyhow::anyhow!("SOCKS5 connect failed: host unreachable");
        assert!(is_transient_sync_error(&error));
    }

    #[tokio::test]
    async fn background_sync_recovers_from_one_transient_failure() {
        let attempts = AtomicUsize::new(0);

        let result = retry_background_sync(
            "test sync",
            || true,
            || {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        anyhow::bail!("network is unreachable")
                    }
                    Ok("synced")
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "synced");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn background_sync_reports_failure_after_retry() {
        let attempts = AtomicUsize::new(0);

        let error = retry_background_sync::<(), _, _, _>(
            "test sync",
            || true,
            || {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        anyhow::bail!("connection reset by peer")
                    }
                    anyhow::bail!("network is unreachable")
                }
            },
        )
        .await
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("network is unreachable"));
        assert!(message.contains("connection reset by peer"));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn background_sync_does_not_retry_permanent_failure() {
        let attempts = AtomicUsize::new(0);

        let error = retry_background_sync(
            "test sync",
            || true,
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                async {
                    anyhow::Result::<()>::Err(anyhow::anyhow!(
                        "login failed: authentication failed"
                    ))
                }
            },
        )
        .await
        .unwrap_err();

        assert_eq!(error.to_string(), "login failed: authentication failed");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn background_sync_does_not_retry_after_pause() {
        let paused = AtomicBool::new(false);
        let attempts = AtomicUsize::new(0);

        let error = retry_background_sync::<(), _, _, _>(
            "test sync",
            || !paused.load(Ordering::SeqCst),
            || {
                attempts.fetch_add(1, Ordering::SeqCst);
                paused.store(true, Ordering::SeqCst);
                async { anyhow::Result::<()>::Err(anyhow::anyhow!("connection lost")) }
            },
        )
        .await
        .unwrap_err();

        assert!(error.is::<BackgroundSyncCancelled>());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
