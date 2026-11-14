//! Mailbox change detection for IMAP IDLE watchers, shared by the desktop
//! sidecar and the mobile FFI watcher.
//!
//! async-imap returns only what arrives during the IDLE wait as `NewData`.
//! Updates the server sends before IDLE's continuation, or alongside the DONE
//! that ends each cycle, go to the session's `unsolicited_responses` queue.
//! Mail landing in that window is announced only there, so a watcher must read
//! the queue each cycle or the arrival waits for some later push.

/// Empty an IDLE session's queue of unsolicited responses, returning whether
/// any reported a mailbox change: a new or removed message, or changed flags.
/// Status replies and keepalives (`* OK Still here`) do not count.
pub fn drain_mailbox_changes(
    unsolicited: &async_channel::Receiver<async_imap::types::UnsolicitedResponse>,
) -> bool {
    use async_imap::imap_proto::Response;
    use async_imap::types::UnsolicitedResponse;
    let mut changed = false;
    while let Ok(response) = unsolicited.try_recv() {
        changed |= match response {
            UnsolicitedResponse::Exists(_)
            | UnsolicitedResponse::Recent(_)
            | UnsolicitedResponse::Expunge(_) => true,
            UnsolicitedResponse::Other(data) => {
                matches!(
                    data.parsed(),
                    Response::Fetch(..) | Response::Vanished { .. }
                )
            }
            UnsolicitedResponse::Status { .. } => false,
        };
    }
    changed
}

/// Longest one IDLE cycle runs before it is ended and restarted, checking
/// for unannounced mail in between.
pub const IDLE_CYCLE: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Wait in IDLE for a server push, returning whether one came (true) or
/// `cycle` ran out (false). async-imap's own timeout restarts on every
/// response, keepalives included, so a server sending `* OK Still here` every
/// few minutes would keep a cycle open forever and the end-of-cycle check
/// would never run. This deadline is absolute.
pub async fn wait_for_push<T>(
    handle: &mut async_imap::extensions::idle::Handle<T>,
    cycle: std::time::Duration,
) -> anyhow::Result<bool>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    use anyhow::Context as _;
    let (idle_fut, _stop) = handle.wait_with_timeout(cycle);
    match tokio::time::timeout(cycle, idle_fut).await {
        Err(_) => Ok(false),
        Ok(response) => Ok(matches!(
            response.context("IDLE")?,
            async_imap::extensions::idle::IdleResponse::NewData(_)
        )),
    }
}

/// After a quiet IDLE cycle, ask the server directly whether `folder` gained
/// mail it never announced. Some servers (Gmail among them) occasionally skip
/// an IDLE push; without this the arrival waits for the next full sync, up to
/// an hour later. Two round trips on the already-open IDLE connection: NOOP
/// makes the server flush pending updates, and STATUS compares its UIDNEXT
/// with the last one stored (`known`: UIDVALIDITY and UIDNEXT).
pub async fn missed_arrivals<T>(
    session: &mut async_imap::Session<T>,
    folder: &str,
    known: Option<(u32, u32)>,
) -> anyhow::Result<bool>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    use anyhow::Context as _;
    session.noop().await.context("NOOP")?;
    if drain_mailbox_changes(&session.unsolicited_responses) {
        return Ok(true);
    }
    // Nothing stored yet to compare with: the catch-up sync that precedes the
    // first IDLE cycle sets it, so this only happens if that sync wrote none.
    let Some((validity, uid_next)) = known else {
        return Ok(false);
    };
    let status = session
        .status(folder, "(UIDNEXT UIDVALIDITY)")
        .await
        .context("STATUS")?;
    let changed = drain_mailbox_changes(&session.unsolicited_responses)
        || status.uid_validity.is_some_and(|server| server != validity)
        || status.uid_next.is_some_and(|server| server > uid_next);
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    type FakeSession = async_imap::Session<tokio::io::DuplexStream>;

    /// A logged-in session against a scripted server. `reply` answers each
    /// command line given its tag, the command, and the tag of the IDLE in
    /// progress (for DONE); LOGIN is answered for it.
    async fn scripted_session(
        reply: impl Fn(&str, &str, &str) -> String + Send + 'static,
    ) -> (FakeSession, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (client_io, server_io) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(server_io);
            let mut lines = BufReader::new(read).lines();
            let mut idle_tag = String::new();
            while let Some(line) = lines.next_line().await.unwrap() {
                let (tag, command) = line.split_once(' ').unwrap_or((line.as_str(), ""));
                let response = if command.starts_with("LOGIN") {
                    format!("{tag} OK logged in\r\n")
                } else {
                    if command == "IDLE" {
                        idle_tag = tag.to_string();
                    }
                    reply(tag, command, &idle_tag)
                };
                write.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let session = async_imap::Client::new(client_io)
            .login("user", "secret")
            .await
            .map_err(|(err, _)| err)
            .unwrap();
        (session, server)
    }

    /// One IDLE cycle against a scripted server: `on_idle` answers the IDLE
    /// command (ending with the continuation), `on_done` answers DONE (ending
    /// with the tagged OK). The wait times out with no push in between, the
    /// way Gmail's 15-minute cycle ends. Returns whether the watcher's drain
    /// sees a mailbox change after init and after DONE.
    async fn idle_cycle(on_idle: &'static str, on_done: &'static str) -> (bool, bool) {
        let (session, server) = scripted_session(move |_, command, idle_tag| {
            if command.is_empty() {
                on_done.replace("{tag}", idle_tag)
            } else {
                on_idle.to_string()
            }
        })
        .await;
        let unsolicited = session.unsolicited_responses.clone();
        let mut handle = session.idle();
        handle.init().await.unwrap();
        let after_init = drain_mailbox_changes(&unsolicited);
        let (idle_fut, _stop) = handle.wait_with_timeout(Duration::from_millis(100));
        assert!(matches!(
            idle_fut.await.unwrap(),
            async_imap::extensions::idle::IdleResponse::Timeout
        ));
        let session = handle.done().await.unwrap();
        let after_done = drain_mailbox_changes(&unsolicited);
        drop(session);
        server.await.unwrap();
        (after_init, after_done)
    }

    #[tokio::test]
    async fn mail_announced_with_done_is_not_lost() {
        let (_, after_done) = idle_cycle(
            "+ idling\r\n",
            "* 7 EXISTS\r\n* 1 RECENT\r\n{tag} OK IDLE terminated\r\n",
        )
        .await;
        assert!(after_done, "an EXISTS sent with DONE must trigger a sync");
    }

    #[tokio::test]
    async fn mail_announced_before_idle_continuation_is_not_lost() {
        let (after_init, _) =
            idle_cycle("* 8 EXISTS\r\n+ idling\r\n", "{tag} OK IDLE terminated\r\n").await;
        assert!(
            after_init,
            "an EXISTS sent before + idling must end the wait"
        );
    }

    #[tokio::test]
    async fn flag_change_announced_with_done_is_not_lost() {
        let (_, after_done) = idle_cycle(
            "+ idling\r\n",
            "* 3 FETCH (UID 42 FLAGS (\\Seen))\r\n{tag} OK IDLE terminated\r\n",
        )
        .await;
        assert!(after_done);
    }

    #[tokio::test]
    async fn keepalives_alone_do_not_trigger_a_sync() {
        let (after_init, after_done) = idle_cycle(
            "* OK Still here\r\n+ idling\r\n",
            "* OK Still here\r\n{tag} OK IDLE terminated\r\n",
        )
        .await;
        assert!(!after_init && !after_done);
    }

    /// The safety-net check against a server answering NOOP with `on_noop`
    /// (untagged lines only) and STATUS with `status` (one untagged line).
    async fn check_missed(
        on_noop: &'static str,
        status: &'static str,
        known: Option<(u32, u32)>,
    ) -> bool {
        let (mut session, server) = scripted_session(move |tag, command, _| {
            if command == "NOOP" {
                format!("{on_noop}{tag} OK NOOP completed\r\n")
            } else {
                assert!(command.starts_with("STATUS"), "unexpected {command}");
                format!("{status}{tag} OK STATUS completed\r\n")
            }
        })
        .await;
        let missed = missed_arrivals(&mut session, "INBOX", known).await.unwrap();
        drop(session);
        server.await.unwrap();
        missed
    }

    #[tokio::test]
    async fn keepalives_cannot_extend_an_idle_cycle() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        // A server that answers IDLE, then sends a keepalive every 20ms —
        // more often than the cycle — until DONE.
        let (client_io, server_io) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            let (read, mut write) = tokio::io::split(server_io);
            let mut lines = BufReader::new(read).lines();
            let login = lines.next_line().await.unwrap().unwrap();
            let tag = login.split(' ').next().unwrap().to_string();
            write
                .write_all(format!("{tag} OK logged in\r\n").as_bytes())
                .await
                .unwrap();
            let idle = lines.next_line().await.unwrap().unwrap();
            let idle_tag = idle.split(' ').next().unwrap().to_string();
            write.write_all(b"+ idling\r\n").await.unwrap();
            loop {
                tokio::select! {
                    line = lines.next_line() => {
                        assert_eq!(line.unwrap().as_deref(), Some("DONE"));
                        let done = format!("{idle_tag} OK IDLE terminated\r\n");
                        write.write_all(done.as_bytes()).await.unwrap();
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(20)) => {
                        write.write_all(b"* OK Still here\r\n").await.unwrap();
                    }
                }
            }
        });
        let session = async_imap::Client::new(client_io)
            .login("user", "secret")
            .await
            .map_err(|(err, _)| err)
            .unwrap();
        let mut handle = session.idle();
        handle.init().await.unwrap();
        let wait = wait_for_push(&mut handle, Duration::from_millis(150));
        let pushed = tokio::time::timeout(Duration::from_secs(2), wait)
            .await
            .expect("keepalives kept the IDLE cycle open past its deadline")
            .unwrap();
        assert!(!pushed, "a keepalive is not a push");
        handle.done().await.unwrap();
        server.await.unwrap();
    }

    const KNOWN: Option<(u32, u32)> = Some((1, 4950));

    #[tokio::test]
    async fn unannounced_arrival_is_found_by_uidnext() {
        let status = "* STATUS INBOX (UIDNEXT 4951 UIDVALIDITY 1)\r\n";
        assert!(check_missed("", status, KNOWN).await);
    }

    #[tokio::test]
    async fn arrival_flushed_by_noop_is_found() {
        let status = "* STATUS INBOX (UIDNEXT 4950 UIDVALIDITY 1)\r\n";
        assert!(check_missed("* 12 EXISTS\r\n", status, KNOWN).await);
    }

    #[tokio::test]
    async fn changed_uidvalidity_is_a_change() {
        let status = "* STATUS INBOX (UIDNEXT 3 UIDVALIDITY 2)\r\n";
        assert!(check_missed("", status, KNOWN).await);
    }

    #[tokio::test]
    async fn quiet_mailbox_needs_no_sync() {
        let status = "* STATUS INBOX (UIDNEXT 4950 UIDVALIDITY 1)\r\n";
        assert!(!check_missed("* OK Still here\r\n", status, KNOWN).await);
    }
}
