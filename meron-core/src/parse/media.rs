use super::*;

/// Default cap for files served from the media cache. The desktop bridge stores
/// this under the user's cache directory, so it must not grow without bound.
pub(super) const MEDIA_CAP_BYTES: u64 = 1024 * 1024 * 1024;

/// Where inline images are written to disk and the message identity used to build
/// their on-disk path / served key (`account/folder/uid/index.ext`). The desktop
/// bridge serves these files at `/media/<key>`, so image bytes never travel back
/// through the JSON sidecar protocol. Built per `parse_message` call.
pub struct MediaCtx {
    pub root: PathBuf,
    pub account: String,
    pub folder: String,
    pub uid: u32,
}

/// Root directory for on-disk attachment files. The bridge sets `MERON_MEDIA_DIR`
/// so both sides agree; the fallback only matters when running the sidecar alone.
pub fn media_root() -> PathBuf {
    if let Ok(path) = std::env::var("MERON_MEDIA_DIR") {
        return PathBuf::from(path);
    }
    let base = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.cache")
    });
    PathBuf::from(base).join("meron/attachments")
}

/// Key prefix for attachments of decrypted mail. They are written under
/// [`secure_media_root`] (tmpfs on Linux) instead of the plain cache, the
/// only place decrypted bytes would otherwise sit on disk outside SQLCipher.
/// After a reboot they are gone and the message is fetched and decrypted
/// again, like any pruned attachment.
pub const SECURE_MEDIA_PREFIX: &str = "e2ee/";

pub(super) static MOBILE_SECURE_MEDIA: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Mobile uses its private sandbox, never the desktop's cache fallback. Wipe
/// crash leftovers once per process, not on foreground/background reconfigure.
pub(crate) fn configure_mobile_secure_media(data_dir: &Path) {
    MOBILE_SECURE_MEDIA.get_or_init(|| {
        let root = data_dir.join("secure-media");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700));
        }
        root
    });
}

/// Where decrypted attachments go. The bridge sets `MERON_SECURE_MEDIA_DIR`
/// (on Linux `$XDG_RUNTIME_DIR/meron…`); the fallback keeps the sidecar
/// usable when run alone.
pub fn secure_media_root() -> PathBuf {
    if let Some(root) = MOBILE_SECURE_MEDIA.get() {
        return root.clone();
    }
    if let Ok(path) = std::env::var("MERON_SECURE_MEDIA_DIR") {
        return PathBuf::from(path);
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("meron/secure-media");
    }
    media_root().with_file_name("secure-media")
}

/// The file behind a media key, wherever its root is.
pub fn media_file(root: &Path, key: &str) -> PathBuf {
    match key.strip_prefix(SECURE_MEDIA_PREFIX) {
        Some(rest) => secure_media_root().join(rest),
        None => root.join(key),
    }
}

/// Write one attachment's bytes to `<root>/<account>/<folder>/<uid>/<index>.<ext>`
/// and return the relative key, or `None` if the write fails (the part is then
/// just dropped from the rendered view rather than failing the whole message).
/// The extension comes from the original filename when present (so downloads keep
/// a sensible type), falling back to the MIME subtype.
pub(super) fn write_media(
    ctx: &MediaCtx,
    index: usize,
    filename: &str,
    mime: &str,
    bytes: &[u8],
) -> Option<String> {
    write_media_with_cap(ctx, index, filename, mime, bytes, media_cache_cap())
}

pub(super) fn write_media_with_cap(
    ctx: &MediaCtx,
    index: usize,
    filename: &str,
    mime: &str,
    bytes: &[u8],
    cap: u64,
) -> Option<String> {
    // An individual file larger than the entire cache cannot survive pruning.
    if bytes.len() as u64 > cap {
        return None;
    }
    let ext = sanitize_segment(
        Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.is_empty())
            // The MIME subtype without its structured-syntax suffix (RFC 6838):
            // `image/svg+xml` is an `.svg`, which the desktop's file server
            // serves as an image; `.svg_xml` would go out as text/plain.
            .or_else(|| {
                mime.rsplit('/')
                    .next()
                    .and_then(|sub| sub.split('+').next())
                    .filter(|e| !e.is_empty())
            })
            .unwrap_or("bin"),
    );
    let key = format!(
        "{}/{}/{}/{}.{}",
        sanitize_segment(&ctx.account),
        sanitize_segment(&ctx.folder),
        ctx.uid,
        index,
        ext
    );
    let path = ctx.root.join(&key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, bytes).ok()?;
    Some(key)
}

/// True when every attachment's bytes are still on disk. Used before serving
/// cached mail rows; when false the caller refetches and re-parses the message.
/// Two cases force a refetch: a keyed file that pruning removed independently of
/// SQLite, and a key-less attachment whose bytes were never written — a row
/// cached before attachments were persisted to disk, or a write that failed.
/// An `oversized` attachment is key-less by design and is not one of them.
pub fn cached_media_available(root: &Path, message: &Message) -> bool {
    missing_media_count(root, message) == 0
}

/// How many of a cached message's attachments a download could put back on
/// disk: pruned, or never written. Not the ones left off for their size.
pub fn missing_media_count(root: &Path, message: &Message) -> usize {
    message
        .attachments
        .iter()
        .filter(|att| match att.key.as_deref() {
            Some(key) => !media_file(root, key).is_file(),
            None => !att.oversized,
        })
        .count()
}

/// Keep the shared media cache under its configured cap by removing oldest files
/// first. The cap is bytes and can be overridden with `MERON_MEDIA_CAP`.
pub(super) fn media_cache_cap() -> u64 {
    std::env::var("MERON_MEDIA_CAP")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(MEDIA_CAP_BYTES)
}

/// A media root for tests that fetch bodies: the fetch prunes it, which walks all
/// of it and deletes from it, so it must never be the shared temp dir itself.
#[cfg(test)]
pub(crate) fn test_media_root() -> PathBuf {
    std::env::temp_dir().join("meron-test-media")
}

pub fn prune_media_cache(root: &Path) {
    let cap = media_cache_cap();
    let mut files = Vec::new();
    let mut total = 0u64;
    collect_media_files(root, &mut files, &mut total, true);
    if total <= cap {
        return;
    }

    files.sort_by_key(|(_, _, mtime)| *mtime);
    let mut over = total - cap;
    for (path, size, _) in files {
        if over == 0 {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            over = over.saturating_sub(size);
        }
    }
}

pub(super) fn collect_media_files(
    dir: &Path,
    out: &mut Vec<(PathBuf, u64, std::time::SystemTime)>,
    total: &mut u64,
    skip_document_sessions: bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        // Reader sessions have no refetchable mailbox source and are reclaimed
        // on close/startup. Ordinary cache eviction must leave them intact.
        if skip_document_sessions && entry.file_name() == "_eml" {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_media_files(&path, out, total, false);
        } else if meta.is_file() {
            *total += meta.len();
            out.push((
                path,
                meta.len(),
                meta.modified().unwrap_or(std::time::UNIX_EPOCH),
            ));
        }
    }
}

/// Reduce a path segment to filesystem- and URL-safe characters so account names
/// and IMAP folder paths (which may contain `/`, brackets, spaces) can't escape
/// the media root or break the served URL.
pub(crate) fn sanitize_segment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Best-effort removal of cached attachment/feed media for one account.
pub fn remove_account_media(root: &Path, account: &str) {
    let segment = sanitize_segment(account);
    if segment.is_empty() {
        return;
    }
    let _ = std::fs::remove_dir_all(root.join(segment));
}

/// Best-effort removal of one folder's cached attachment files.
pub fn remove_folder_media(root: &Path, account: &str, folder: &str) {
    let (account, folder) = (sanitize_segment(account), sanitize_segment(folder));
    if account.is_empty() || folder.is_empty() {
        return;
    }
    let _ = std::fs::remove_dir_all(root.join(account).join(folder));
}

/// Best-effort removal of one message's cached attachment files.
pub fn remove_message_media(root: &Path, account: &str, folder: &str, uid: u32) {
    let (account, folder) = (sanitize_segment(account), sanitize_segment(folder));
    if account.is_empty() || folder.is_empty() {
        return;
    }
    let _ = std::fs::remove_dir_all(root.join(account).join(folder).join(uid.to_string()));
}
