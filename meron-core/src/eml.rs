//! Read an external RFC822 document without adding it to a mailbox.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::parse::{self, MediaCtx, Message};

pub const MAX_EML_BYTES: u64 = 100 * 1024 * 1024;
// Leave room for the protocol envelope below the Go scanner's 16 MiB limit.
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

struct ResponseLimit(usize);
impl Write for ResponseLimit {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.0 {
            return Err(std::io::Error::other(
                "EML message content exceeds the 8 MiB display limit",
            ));
        }
        self.0 -= bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// If the host times out and removes its snapshot, a still-running parser may
// finish writing attachments afterwards. Reclaim that abandoned session only
// after all parser writes have stopped, including when parsing returns an error.
struct AbandonedSession<'a> {
    source: &'a Path,
    dir: PathBuf,
}
impl Drop for AbandonedSession<'_> {
    fn drop(&mut self) {
        if !self.source.exists() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

pub fn read(path: &Path, session: &str, media_root: &Path) -> anyhow::Result<Message> {
    if session.is_empty()
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        bail!("invalid EML session");
    }
    let _cleanup = AbandonedSession {
        source: path,
        dir: media_root.join("_eml").join(session),
    };
    let file = std::fs::File::open(path).context("open EML file")?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        bail!("EML document must be a regular file");
    }
    if metadata.len() > MAX_EML_BYTES {
        bail!("EML file exceeds 100 MiB");
    }
    let mut raw = Vec::new();
    file.take(MAX_EML_BYTES + 1).read_to_end(&mut raw)?;
    if raw.len() as u64 > MAX_EML_BYTES {
        bail!("EML file exceeds 100 MiB");
    }
    let parsed = mailparse::parse_mail(&raw).context("invalid EML file")?;
    let first_line = raw.split(|byte| *byte == b'\n').next().unwrap_or_default();
    let valid_header = first_line
        .iter()
        .position(|byte| *byte == b':')
        .is_some_and(|colon| {
            colon > 0
                && first_line[..colon]
                    .iter()
                    .all(|byte| (33..=126).contains(byte))
        });
    if parsed.headers.is_empty() || !valid_header {
        bail!("EML file has no message headers");
    }
    let ctx = MediaCtx {
        root: media_root.to_path_buf(),
        account: "_eml".into(),
        folder: session.into(),
        uid: 1,
    };
    // Temporary documents are cleaned up by their owning reader window. Avoid
    // pruning the shared cache while opening a document.
    let mut message = parse::parse_message_unpruned(&raw, Some(&ctx));
    if let Some(html) = message.body_html.take() {
        message.body_html = Some(parse::prepare_message_html(&html, false, &message.subject));
    }
    // Count encoded bytes, including JSON escaping, without allocating a second
    // potentially huge body. Oversized documents return a small protocol error.
    serde_json::to_writer(ResponseLimit(MAX_RESPONSE_BYTES), &message)?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_limit_counts_json_escaping() {
        let message = Message {
            body: "\0".repeat(2 * 1024 * 1024),
            ..Message::default()
        };
        assert!(serde_json::to_writer(ResponseLimit(MAX_RESPONSE_BYTES), &message).is_err());
    }

    #[test]
    fn abandoned_snapshot_reclaims_late_attachments() {
        let root = std::env::temp_dir().join(format!("meron-eml-{}", uuid::Uuid::new_v4()));
        let dir = root.join("_eml/abandoned/1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("late.png"), b"late attachment").unwrap();
        assert!(read(&root.join("removed-snapshot.eml"), "abandoned", &root).is_err());
        assert!(!root.join("_eml/abandoned").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_external_mime_with_isolated_media_and_blocked_remote_content() {
        let root = std::env::temp_dir().join(format!("meron-eml-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("test.eml");
        let raw = concat!(
            "From: Alice <alice@example.com>\r\n",
            "To: Bob <bob@example.com>\r\n",
            "Subject: =?UTF-8?B?SGVsbG8=?=\r\n",
            "MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=x\r\n\r\n",
            "--x\r\nContent-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>Hello</p><img src=\"cid:logo\"><img src=\"https://example.com/tracker.png\">\r\n",
            "--x\r\nContent-Type: image/png\r\nContent-ID: <logo>\r\n",
            "Content-Disposition: inline; filename=logo.png\r\n",
            "Content-Transfer-Encoding: base64\r\n\r\naGVsbG8=\r\n--x--\r\n"
        );
        std::fs::write(&path, raw).unwrap();
        let message = read(&path, "test-session", &root).unwrap();
        assert_eq!(message.subject, "Hello");
        assert_eq!(message.from_addr, "alice@example.com");
        assert!(message.to.contains("bob@example.com"));
        let html = message.body_html.unwrap();
        let key = message.attachments[0].key.as_ref().unwrap();
        assert!(key.starts_with("_eml/test-session/1/"));
        assert!(html.contains(&format!("/media/{key}")));
        assert!(html.contains("Content-Security-Policy"));
        assert!(!html.contains("img-src https:"));
        assert_eq!(std::fs::read(root.join(key)).unwrap(), b"hello");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
        assert!(read(&path, "../escape", &root).is_err());
        std::fs::write(&path, b"not an email").unwrap();
        assert!(read(&path, "test-session", &root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
