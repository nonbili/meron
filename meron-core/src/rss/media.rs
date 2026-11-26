use std::path::Path;

use super::fetch::http_get;
use super::{ParsedItem, stable_hash};

/// Cap on the total size of cached feed images (overridable via env). Pruned
/// oldest-first after a sync once exceeded.
const FEED_MEDIA_CAP_BYTES: u64 = 1024 * 1024 * 1024;

/// Download each item's inline images into the media dir, filling in their local
/// `key`. Best-effort: a failed fetch leaves `key = None` (the UI falls back to
/// the remote URL for image-enabled accounts). Skips images already on disk so
/// repeat syncs don't re-download. Network only — never holds the DB lock.
pub(super) fn cache_feed_images(account: &str, sub_id: &str, items: &mut [ParsedItem]) {
    let root = crate::parse::media_root();
    for item in items.iter_mut() {
        for (index, img) in item.extra.images.iter_mut().enumerate() {
            if let Some(key) = existing_image_key(&root, account, sub_id, &item.item_key, index) {
                img.key = Some(key);
                continue;
            }
            let Ok(resp) = http_get(&img.url, "", "") else {
                continue;
            };
            if resp.status != 200 || resp.body.is_empty() {
                continue;
            }
            // Only cache genuine image bytes. Signed feed-image URLs
            // can return HTTP 200 with a text error body like "URL signature expired"
            // once the signature lapses; caching that would shadow the still-valid
            // remote URL with a broken file. Skip it — key stays None and the UI falls
            // back to the live remote URL for image-enabled accounts.
            let Some(ext) = sniff_image_ext(&resp.body) else {
                continue;
            };
            if let Some(key) = write_feed_image(
                &root,
                account,
                sub_id,
                &item.item_key,
                index,
                ext,
                &resp.body,
            ) {
                img.key = Some(key);
            }
        }
    }
}

/// Download and cache a feed's declared icon into the media dir, returning its
/// media key. Best-effort and idempotent: an already-cached icon (same URL) is
/// reused without re-fetching, and any failure (empty/bad URL, non-image bytes)
/// yields `None` so the caller leaves the key empty. Network only — no DB lock.
pub(super) fn cache_feed_icon(account: &str, sub_id: &str, icon_url: &str) -> Option<String> {
    let icon_url = icon_url.trim();
    if icon_url.is_empty() {
        return None;
    }
    let root = crate::parse::media_root();
    if let Some(key) = existing_icon_key(&root, account, sub_id, icon_url) {
        return Some(key);
    }
    let resp = http_get(icon_url, "", "").ok()?;
    if resp.status != 200 || resp.body.is_empty() {
        return None;
    }
    // Same byte-sniffing guard as inline images: only cache genuine image bytes,
    // never a text error page served with HTTP 200.
    let ext = sniff_image_ext(&resp.body)?;
    write_feed_icon(&root, account, sub_id, icon_url, ext, &resp.body)
}

/// The relative dir holding a subscription's cached feed icon. Shares the
/// `<account>/<sub>` prefix with the per-item image dirs; the icon is a file
/// directly in it (`icon-<urlhash>.<ext>`), so it never collides with the
/// `<item_key>/` subdirectories below it.
fn feed_icon_dir(account: &str, sub_id: &str) -> String {
    use crate::parse::sanitize_segment as san;
    format!("{}/{}", san(account), san(sub_id))
}

/// If the feed icon for `icon_url` is already on disk, return its media key. The
/// filename embeds a hash of the URL so a changed icon misses and re-downloads
/// (the stale file is left for the size-cap pruner). Heals non-image poison.
fn existing_icon_key(root: &Path, account: &str, sub_id: &str, icon_url: &str) -> Option<String> {
    let rel_dir = feed_icon_dir(account, sub_id);
    let prefix = format!("icon-{}.", stable_hash(icon_url));
    for entry in std::fs::read_dir(root.join(&rel_dir)).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("");
        if is_image_ext(ext) {
            return Some(format!("{rel_dir}/{name}"));
        }
        let _ = std::fs::remove_file(entry.path());
    }
    None
}

/// Write feed-icon bytes under `<account>/<sub>/icon-<urlhash>.<ext>`, returning
/// the media key, or `None` if the write fails.
fn write_feed_icon(
    root: &Path,
    account: &str,
    sub_id: &str,
    icon_url: &str,
    ext: &str,
    bytes: &[u8],
) -> Option<String> {
    let key = format!(
        "{}/icon-{}.{}",
        feed_icon_dir(account, sub_id),
        stable_hash(icon_url),
        ext
    );
    let path = root.join(&key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, bytes).ok()?;
    Some(key)
}

/// The relative dir holding one item's cached images.
fn item_media_dir(account: &str, sub_id: &str, item_key: &str) -> String {
    use crate::parse::sanitize_segment as san;
    format!("{}/{}/{}", san(account), san(sub_id), san(item_key))
}

/// If image `index` for an item is already on disk, return its media key (the
/// extension can vary, so match by the `<index>.` filename prefix).
fn existing_image_key(
    root: &Path,
    account: &str,
    sub_id: &str,
    item_key: &str,
    index: usize,
) -> Option<String> {
    let rel_dir = item_media_dir(account, sub_id, item_key);
    let prefix = format!("{index}.");
    for entry in std::fs::read_dir(root.join(&rel_dir)).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("");
        if is_image_ext(ext) {
            return Some(format!("{rel_dir}/{name}"));
        }
        // Heal earlier poison (e.g. ".plain" from an expired signed URL): drop the
        // bad file so this image re-downloads on this sync.
        let _ = std::fs::remove_file(entry.path());
    }
    None
}

/// Write image bytes under `<account>/<sub>/<item>/<index>.<ext>`, returning the
/// media key, or `None` if the write fails.
fn write_feed_image(
    root: &Path,
    account: &str,
    sub_id: &str,
    item_key: &str,
    index: usize,
    ext: &str,
    bytes: &[u8],
) -> Option<String> {
    let key = format!(
        "{}/{}.{}",
        item_media_dir(account, sub_id, item_key),
        index,
        ext
    );
    let path = root.join(&key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    std::fs::write(&path, bytes).ok()?;
    Some(key)
}

/// Recognized cached-image file extensions (what `sniff_image_ext` can produce).
fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "jpg" | "png" | "gif" | "webp" | "svg")
}

/// Sniff a common image format from the leading bytes, returning its canonical
/// file extension, or `None` when the bytes aren't a recognized image (e.g. a
/// text error page served with HTTP 200). Content-type is unreliable for signed
/// image URLs, so we trust the bytes instead.
pub(super) fn sniff_image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    // SVG is text; check the leading bytes for an <svg>/xml prolog.
    let head = &bytes[..bytes.len().min(512)];
    if let Ok(text) = std::str::from_utf8(head) {
        let trimmed = text.trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && text.contains("<svg")) {
            return Some("svg");
        }
    }
    None
}

/// Whether a cached media `key` points to a real image on disk. Guards the read
/// path against poison cached before image validation existed (e.g. ".plain"
/// error bodies) so those keys are dropped in favor of the remote URL.
pub(super) fn key_is_image(root: &Path, key: &str) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(root.join(key)) else {
        return false;
    };
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf).unwrap_or(0);
    sniff_image_ext(&buf[..n]).is_some()
}

/// Keep the cached-feed-image footprint under the cap by deleting oldest files
/// first. Scoped to `rss-*` account dirs so it never touches mail attachments.
pub(super) fn prune_feed_media(root: &Path) {
    let cap = std::env::var("MERON_FEED_MEDIA_CAP")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(FEED_MEDIA_CAP_BYTES);

    let mut files: Vec<(std::path::PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let mut total: u64 = 0;
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for account_dir in entries.flatten() {
        if !account_dir
            .file_name()
            .to_string_lossy()
            .starts_with("rss-")
        {
            continue;
        }
        collect_files(&account_dir.path(), &mut files, &mut total);
    }
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

/// Recursively gather (path, size, mtime) for every file under `dir`.
fn collect_files(
    dir: &Path,
    out: &mut Vec<(std::path::PathBuf, u64, std::time::SystemTime)>,
    total: &mut u64,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            collect_files(&path, out, total);
        } else if meta.is_file() {
            *total += meta.len();
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            out.push((path, meta.len(), mtime));
        }
    }
}
