//! SMTP send path: build a MIME message with `mail-builder` and submit it over
//! `async-smtp` (the chatmail/Delta Chat SMTP stack), reusing the IMAP `Stream`
//! enum for plaintext or implicit-TLS transport.

use anyhow::{Context, Result};
use async_smtp::authentication::{Credentials, Mechanism};
use async_smtp::error::Error as SmtpError;
use async_smtp::{EmailAddress, Envelope, SendableEmail, SmtpClient, SmtpTransport};
use base64::Engine as _;
use mail_builder::MessageBuilder;
use std::time::Duration;
use tokio::io::BufReader;

use crate::imap::{Creds, connect_stream};

/// Cap on each pre-DATA exchange (greeting, STARTTLS, AUTH). async-smtp has no
/// I/O timeouts of its own, so without these a connection that dies mid-command
/// hangs the send forever.
const SMTP_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// Cap on the DATA phase, sized for large attachments on slow uplinks.
const SMTP_DATA_TIMEOUT: Duration = Duration::from_secs(300);

async fn with_timeout<T>(
    limit: Duration,
    what: &str,
    fut: impl std::future::Future<Output = T>,
) -> Result<T> {
    tokio::time::timeout(limit, fut)
        .await
        .map_err(|_| anyhow::anyhow!("{what} timed out after {}s", limit.as_secs()))
}

#[derive(serde::Deserialize, Debug)]
pub struct AttachmentInput {
    pub filename: String,
    pub mime: String,
    pub data: String, // base64 encoded
    #[serde(default)]
    pub inline_id: String,
}

/// Split a comma-separated recipient string into trimmed, non-empty entries.
/// Commas inside a double-quoted display name (`"Doe, Jane" <j@x>`) or inside
/// the `<...>` address brackets don't split, so quoted contact names survive.
fn parse_addrs(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut in_brackets = false;
    for ch in raw.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            '<' if !in_quotes => {
                in_brackets = true;
                current.push(ch);
            }
            '>' if !in_quotes => {
                in_brackets = false;
                current.push(ch);
            }
            ',' if !in_quotes && !in_brackets => {
                let entry = current.trim();
                if !entry.is_empty() {
                    out.push(entry.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let entry = current.trim();
    if !entry.is_empty() {
        out.push(entry.to_string());
    }
    out
}

/// Split a `Name <addr>` (or bare `addr`) entry into its display name and
/// address. mail-builder's `From<&str> for Address` treats the entire input as
/// the email field — so passing `"Name <addr>"` produces a header like
/// `<Name <addr>>`, which receiving clients render as a malformed recipient
/// (display name "<Name >" with literal angle brackets). Splitting here keeps
/// the display name in `name` where it belongs.
fn split_name_addr(entry: &str) -> (String, String) {
    let trimmed = entry.trim();
    if let Some(start) = trimmed.find('<')
        && let Some(end_rel) = trimmed[start + 1..].find('>')
    {
        let name = trimmed[..start].trim();
        // Unwrap a quoted display name (`"Doe, Jane"`) and undo its escapes;
        // mail-builder re-encodes the raw name itself.
        let name = if name.len() >= 2 && name.starts_with('"') && name.ends_with('"') {
            name[1..name.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
                .trim()
                .to_string()
        } else {
            name.to_string()
        };
        let addr = trimmed[start + 1..start + 1 + end_rel].trim();
        return (name, addr.to_string());
    }
    (String::new(), trimmed.to_string())
}

pub fn build_message(
    sender_name: &str,
    from: &str,
    to: &str,
    cc: &str,
    bcc: &str,
    // Whether to write a `Bcc:` header into the MIME. True for copies we keep
    // (Sent, Drafts) so the sender can see who they blind-copied; false for the
    // copy transmitted to recipients, which must not leak the Bcc list (delivery
    // still reaches them via the SMTP envelope's RCPT TO).
    include_bcc: bool,
    subject: &str,
    body: &str,
    html: &str,
    attachments: &[AttachmentInput],
    in_reply_to: &str,
    references: &str,
    reply_to: &str,
    message_id: &str,
) -> Result<Vec<u8>> {
    let to_list = parse_addrs(to);
    let cc_list = parse_addrs(cc);
    let bcc_list = parse_addrs(bcc);
    let reply_to_list = parse_addrs(reply_to);
    let to_pairs: Vec<(String, String)> = to_list.iter().map(|s| split_name_addr(s)).collect();

    let mut builder = MessageBuilder::new()
        .from((sender_name, from))
        .to(to_pairs)
        .subject(subject);

    // Drafts pass a stable id so each autosave overwrites the same Message-ID,
    // letting the IMAP layer find and prune the prior copy. Sends pass "" and
    // let mail-builder mint a fresh one.
    if !message_id.trim().is_empty() {
        // Accept either bare `id@host` or `<id@host>`: mail-builder adds the
        // angle brackets itself, so a pre-bracketed id would emit `<<...>>`.
        builder = builder.message_id(bare_id(message_id));
    }

    if !reply_to_list.is_empty() {
        let reply_to_pairs: Vec<(String, String)> =
            reply_to_list.iter().map(|s| split_name_addr(s)).collect();
        builder = builder.reply_to(reply_to_pairs);
    }

    builder = if html.is_empty() {
        builder.text_body(body)
    } else {
        builder.html_body(html).text_body(body)
    };

    if !cc_list.is_empty() {
        let cc_pairs: Vec<(String, String)> = cc_list.iter().map(|s| split_name_addr(s)).collect();
        builder = builder.cc(cc_pairs);
    }

    if include_bcc && !bcc_list.is_empty() {
        let bcc_pairs: Vec<(String, String)> =
            bcc_list.iter().map(|s| split_name_addr(s)).collect();
        builder = builder.bcc(bcc_pairs);
    }

    let in_reply_to_bare = bare_id(in_reply_to);
    if !in_reply_to_bare.is_empty() {
        builder = builder.in_reply_to(in_reply_to_bare.as_str());
    }
    let refs_bare: Vec<String> = references
        .split_whitespace()
        .map(bare_id)
        .filter(|tok| !tok.is_empty())
        .collect();
    if !refs_bare.is_empty() {
        let refs_refs: Vec<&str> = refs_bare.iter().map(String::as_str).collect();
        builder = builder.references(refs_refs.as_slice());
    }

    for att in attachments {
        // A decode failure must fail the whole build: silently dropping the
        // attachment would send the message without its file and report success.
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&att.data)
            .with_context(|| format!("decode attachment {:?}", att.filename))?;
        if !att.inline_id.trim().is_empty() {
            builder = builder.inline(&att.mime, att.inline_id.trim(), bytes);
        } else {
            builder = builder.attachment(&att.mime, &att.filename, bytes);
        }
    }

    builder.write_to_vec().context("build MIME message")
}

pub async fn send(
    creds: &Creds,
    from_addr: &str,
    sender_name: &str,
    to: &str,
    cc: &str,
    bcc: &str,
    subject: &str,
    body: &str,
    html: &str,
    attachments: &[AttachmentInput],
    in_reply_to: &str,
    references: &str,
    reply_to: &str,
    message_id: &str,
) -> Result<Vec<u8>> {
    // Caller passes the chosen send-as address (primary or a verified alias),
    // already validated against the account; fall back to the IMAP login.
    let from = if from_addr.trim().is_empty() {
        creds.user.as_str()
    } else {
        from_addr.trim()
    };

    let to_list = parse_addrs(to);
    let cc_list = parse_addrs(cc);
    let bcc_list = parse_addrs(bcc);

    // Message-ID resolution. A caller-supplied id wins: the client generates it
    // up front so the optimistic bubble carries the real Message-ID and a quick
    // follow-up reply can thread against it before the Sent copy syncs back.
    // Otherwise, when there's a Bcc we build the message twice — the transmitted
    // copy omits the `Bcc:` header while the Sent copy keeps it — so both must
    // share an explicit Message-ID for replies to thread; without a Bcc we leave
    // it empty and let the SMTP library mint one.
    let message_id = {
        let provided = message_id.trim();
        if !provided.is_empty() {
            provided.to_string()
        } else if bcc_list.is_empty() {
            String::new()
        } else {
            let domain = from.rsplit('@').next().unwrap_or("localhost");
            format!("{}@{}", uuid::Uuid::new_v4(), domain)
        }
    };

    let raw = build_message(
        sender_name,
        from,
        to,
        cc,
        bcc,
        false,
        subject,
        body,
        html,
        attachments,
        in_reply_to,
        references,
        reply_to,
        &message_id,
    )?;

    // Fall back to the IMAP host if SMTP settings were not provided.
    let host = if creds.smtp_host.is_empty() {
        creds.host.as_str()
    } else {
        creds.smtp_host.as_str()
    };
    let port = if creds.smtp_port == 0 {
        587
    } else {
        creds.smtp_port
    };

    // STARTTLS connects in cleartext, then upgrades after EHLO; implicit TLS
    // wraps the socket up front. smtp_starttls takes precedence over smtp_tls.
    let implicit_tls = creds.smtp_tls && !creds.smtp_starttls;
    let stream = connect_stream(host, port, implicit_tls).await?;
    let mut transport = with_timeout(
        SMTP_COMMAND_TIMEOUT,
        "smtp greeting",
        SmtpTransport::new(SmtpClient::new(), BufReader::new(stream)),
    )
    .await?
    .context("smtp connect")?;

    if creds.smtp_starttls {
        // `starttls()` issues the command and returns the raw stream to upgrade;
        // we then re-run EHLO over TLS via a transport built without expecting a
        // greeting (the server sends none after STARTTLS).
        let inner = with_timeout(SMTP_COMMAND_TIMEOUT, "smtp starttls", transport.starttls())
            .await?
            .context("SMTP STARTTLS")?;
        let tcp = match inner.into_inner() {
            crate::imap::Stream::Plain(tcp) => tcp,
            crate::imap::Stream::Tls(_) => {
                return Err(anyhow::anyhow!(
                    "STARTTLS requested on an already-TLS stream"
                ));
            }
        };
        let tls = crate::imap::upgrade_to_tls(host, tcp).await?;
        let upgraded = crate::imap::Stream::Tls(Box::new(tls));
        transport = with_timeout(
            SMTP_COMMAND_TIMEOUT,
            "smtp ehlo",
            SmtpTransport::new(
                SmtpClient::new().without_greeting(),
                BufReader::new(upgraded),
            ),
        )
        .await?
        .context("smtp connect (post-STARTTLS)")?;
    }

    let secret = if creds.is_oauth() {
        creds.access_token.clone().unwrap_or_default()
    } else {
        creds.password.clone()
    };
    let credentials = Credentials::new(creds.user.clone(), secret.clone());
    let mechanisms = if creds.is_oauth() {
        vec![Mechanism::Xoauth2]
    } else {
        vec![Mechanism::Plain, Mechanism::Login]
    };
    // AUTH explicitly instead of try_login: try_login silently skips auth when
    // the EHLO capabilities include none of our mechanisms, which would surface
    // later as a confusing RCPT/DATA rejection instead of an auth error. Try
    // each mechanism in order; only a command-level rejection (502/503/504 —
    // the server doesn't do AUTH at all) falls through to unauthenticated
    // submission, preserving relays that accept mail without login. An empty
    // secret means a deliberately unauthenticated relay; skip AUTH entirely.
    if !secret.is_empty() {
        let mut authed = false;
        let mut auth_unsupported = true;
        let mut last_err: Option<SmtpError> = None;
        for mechanism in &mechanisms {
            match with_timeout(
                SMTP_COMMAND_TIMEOUT,
                "smtp auth",
                transport.auth(*mechanism, &credentials),
            )
            .await?
            {
                Ok(_) => {
                    authed = true;
                    break;
                }
                Err(err) => {
                    let command_rejected = matches!(
                        &err,
                        SmtpError::Permanent(resp)
                            if resp.has_code(502) || resp.has_code(503) || resp.has_code(504)
                    );
                    if !command_rejected {
                        auth_unsupported = false;
                    }
                    last_err = Some(err);
                }
            }
        }
        if !authed {
            if !auth_unsupported {
                let err = last_err.expect("auth failed without an error");
                return Err(anyhow::Error::new(err).context("smtp auth"));
            }
            crate::mlog!(
                crate::log::Level::Warn,
                "mail.send",
                "SMTP server rejected AUTH as unsupported; submitting unauthenticated"
            );
        }
    }

    // SMTP envelope addresses must be bare ("addr@host"); MIME header entries
    // may carry a display name. The header form survives in `to_list`/`cc_list`
    // for the builder above; here we strip down to the address for RCPT TO.
    let mut recipients = Vec::new();
    for addr in envelope_recipients(&to_list, &cc_list, &bcc_list) {
        recipients.push(EmailAddress::new(addr).context("recipient address")?);
    }
    let envelope = Envelope::new(
        Some(EmailAddress::new(from.to_string()).context("from address")?),
        recipients,
    )
    .context("envelope")?;
    with_timeout(
        SMTP_DATA_TIMEOUT,
        "smtp send",
        transport.send(SendableEmail::new(envelope, raw.clone())),
    )
    .await?
    .context("smtp send")?;
    let _ = with_timeout(Duration::from_secs(10), "smtp quit", transport.quit()).await;

    // Return the copy to file in Sent: identical to the transmitted message
    // unless there's a Bcc, in which case we rebuild with the `Bcc:` header (and
    // the same Message-ID) so the user's Sent folder records who they bcc'd.
    if bcc_list.is_empty() {
        Ok(raw)
    } else {
        build_message(
            sender_name,
            from,
            to,
            cc,
            bcc,
            true,
            subject,
            body,
            html,
            attachments,
            in_reply_to,
            references,
            reply_to,
            &message_id,
        )
    }
}

/// Flatten to/cc/bcc header entries into the bare envelope address list,
/// dropping case-insensitive duplicates so no recipient gets two RCPT TOs.
fn envelope_recipients(to: &[String], cc: &[String], bcc: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for entry in to.iter().chain(cc.iter()).chain(bcc.iter()) {
        let bare = bare_addr(entry);
        if seen.insert(bare.to_lowercase()) {
            out.push(bare);
        }
    }
    out
}

/// Extract the bare email address from a "Name <addr>" header entry, or return
/// the input when it's already a bare address. SMTP `RCPT TO` rejects the
/// display-name form, so all envelope addresses pass through this first.
fn bare_addr(entry: &str) -> String {
    let trimmed = entry.trim();
    if let Some(start) = trimmed.find('<')
        && let Some(end) = trimmed[start + 1..].find('>')
    {
        return trimmed[start + 1..start + 1 + end].trim().to_string();
    }
    trimmed.to_string()
}

/// Strip angle brackets and surrounding whitespace from a Message-ID token,
/// leaving a bare `id@host`. The MessageBuilder rewraps with `<...>`.
fn bare_id(token: &str) -> String {
    token
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{bare_addr, bare_id, build_message, parse_addrs, split_name_addr};

    #[test]
    fn parse_addrs_splits_and_trims() {
        assert_eq!(
            parse_addrs("a@x.com, Bob <b@y.com> ,, c@z.com"),
            vec!["a@x.com", "Bob <b@y.com>", "c@z.com"]
        );
        assert!(parse_addrs("").is_empty());
    }

    #[test]
    fn parse_addrs_keeps_quoted_and_bracketed_commas() {
        assert_eq!(
            parse_addrs("\"Doe, Jane\" <j@x.com>, a@y.com"),
            vec!["\"Doe, Jane\" <j@x.com>", "a@y.com"]
        );
        // Unterminated quote: the rest of the string stays one entry rather
        // than producing bogus half-recipients.
        assert_eq!(
            parse_addrs("\"Doe, Jane <j@x.com>"),
            vec!["\"Doe, Jane <j@x.com>"]
        );
    }

    #[test]
    fn envelope_recipients_dedupes_case_insensitively() {
        let to = vec!["Alice <a@x.com>".to_string(), "b@y.com".to_string()];
        let cc = vec!["A@X.COM".to_string()];
        let bcc = vec!["b@y.com".to_string(), "c@z.com".to_string()];
        assert_eq!(
            super::envelope_recipients(&to, &cc, &bcc),
            vec!["a@x.com", "b@y.com", "c@z.com"]
        );
    }

    #[test]
    fn build_message_rejects_undecodable_attachment() {
        let atts = vec![super::AttachmentInput {
            filename: "broken.bin".into(),
            mime: "application/octet-stream".into(),
            data: "not!!valid@@base64".into(),
            inline_id: String::new(),
        }];
        let err = build_message(
            "Alice",
            "alice@x.com",
            "bob@y.com",
            "",
            "",
            false,
            "Subject",
            "body",
            "",
            &atts,
            "",
            "",
            "",
            "",
        )
        .expect_err("must fail instead of sending without the attachment");
        assert!(err.to_string().contains("broken.bin"), "{err:#}");
    }

    #[test]
    fn build_message_addresses_quoted_comma_recipient() {
        let raw = build_message(
            "Alice",
            "alice@x.com",
            "\"Doe, Jane\" <j@x.com>",
            "",
            "",
            false,
            "Subject",
            "body",
            "",
            &[],
            "",
            "",
            "",
            "",
        )
        .expect("build_message");
        let raw = String::from_utf8(raw).expect("utf8");
        assert!(raw.contains("<j@x.com>"), "{raw}");
        // The display name must stay attached to the one recipient, not split
        // into a second bogus address.
        assert!(!raw.contains("<Doe>"), "{raw}");
    }

    #[test]
    fn split_name_addr_handles_named_quoted_and_bare() {
        assert_eq!(
            split_name_addr("Alice Example <a@x.com>"),
            ("Alice Example".to_string(), "a@x.com".to_string())
        );
        assert_eq!(
            split_name_addr("\"Quoted Name\" <a@x.com>"),
            ("Quoted Name".to_string(), "a@x.com".to_string())
        );
        assert_eq!(
            split_name_addr(" a@x.com "),
            (String::new(), "a@x.com".to_string())
        );
        assert_eq!(
            split_name_addr("\"Doe, Jane\" <j@x.com>"),
            ("Doe, Jane".to_string(), "j@x.com".to_string())
        );
        assert_eq!(
            split_name_addr("\"Ada \\\"Lovelace\\\"\" <a@x.com>"),
            ("Ada \"Lovelace\"".to_string(), "a@x.com".to_string())
        );
    }

    #[test]
    fn bare_addr_strips_display_name() {
        assert_eq!(bare_addr("Alice <a@x.com>"), "a@x.com");
        assert_eq!(bare_addr("a@x.com"), "a@x.com");
    }

    #[test]
    fn bare_id_strips_angle_brackets() {
        assert_eq!(bare_id("<id@host>"), "id@host");
        assert_eq!(bare_id(" id@host "), "id@host");
    }

    fn build(message_id: &str, bcc: &str, include_bcc: bool) -> String {
        let raw = build_message(
            "Alice",
            "alice@x.com",
            "bob@y.com",
            "",
            bcc,
            include_bcc,
            "Subject",
            "body",
            "",
            &[],
            "parent@x.com",
            "root@x.com parent@x.com",
            "",
            message_id,
        )
        .expect("build_message");
        String::from_utf8(raw).expect("utf8")
    }

    #[test]
    fn build_message_emits_single_bracketed_message_id() {
        // Bare and pre-bracketed ids must both come out as a single <id@host>.
        for input in ["itest@x.com", "<itest@x.com>"] {
            let raw = build(input, "", false);
            assert!(raw.contains("Message-ID: <itest@x.com>"), "raw: {raw}");
            assert!(!raw.contains("<<"), "double-bracketed Message-ID in: {raw}");
        }
    }

    #[test]
    fn build_message_threads_via_reply_headers() {
        let raw = build("", "", false);
        assert!(raw.contains("In-Reply-To: <parent@x.com>"), "raw: {raw}");
        assert!(
            raw.contains("References: <root@x.com> <parent@x.com>"),
            "raw: {raw}"
        );
    }

    #[test]
    fn build_message_keeps_bcc_only_for_kept_copies() {
        let kept = build("", "secret@z.com", true);
        assert!(
            kept.contains("secret@z.com"),
            "kept copy must carry Bcc: {kept}"
        );
        let wire = build("", "secret@z.com", false);
        assert!(
            !wire.contains("secret@z.com"),
            "wire copy must not leak Bcc: {wire}"
        );
    }

    #[test]
    fn inline_image_send_parses_back_with_media_refs() {
        // Full send-side/receive-side roundtrip for composer inline images:
        // the built MIME's `cid:` refs must come back from parse_message with
        // every `cid:` rewritten to the served `/media/<key>` path, so a
        // message sent from Meron renders its inline images in Meron too.
        use base64::Engine as _;
        let atts = vec![
            super::AttachmentInput {
                filename: "pasted-image-1.png".into(),
                mime: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3]),
                inline_id: "meron-image-1784002518563-a1b2c3d@meron".into(),
            },
            super::AttachmentInput {
                filename: "pasted-image-2.png".into(),
                mime: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.encode([9u8, 8, 7]),
                inline_id: "meron-image-1784002609378-x9y8z7w@meron".into(),
            },
        ];
        let html = r#"<p>one</p><img src="cid:meron-image-1784002518563-a1b2c3d@meron" alt="pasted-image-1.png"><p>two</p><img src="cid:meron-image-1784002609378-x9y8z7w@meron" alt="pasted-image-2.png"><p>bye</p>"#;
        let raw = build_message(
            "Alice",
            "alice@x.com",
            "bob@y.com",
            "",
            "",
            false,
            "Subject",
            "plain fallback",
            html,
            &atts,
            "",
            "",
            "",
            "",
        )
        .expect("build_message");

        let root =
            std::env::temp_dir().join(format!("meron-inline-roundtrip-{}", std::process::id()));
        let ctx = crate::parse::MediaCtx {
            root: root.clone(),
            account: "acct".into(),
            folder: "Sent".into(),
            uid: 1,
        };
        let msg = crate::parse::parse_message(&raw, Some(&ctx));
        let _ = std::fs::remove_dir_all(&root);

        let html_out = msg.body_html.expect("html kept");
        assert!(
            !html_out.contains("cid:"),
            "unrewritten cid ref: {html_out}"
        );
        assert!(html_out.contains("/media/acct/Sent/1/0.png"), "{html_out}");
        assert!(html_out.contains("/media/acct/Sent/1/1.png"), "{html_out}");
        assert_eq!(msg.attachments.len(), 2);
    }
}
