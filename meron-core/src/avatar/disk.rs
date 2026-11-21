use super::{SenderImage, fetch_cached, resolve_network, sha256_hex};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, mpsc},
    thread,
    time::{Duration, UNIX_EPOCH},
};

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FILES: usize = 2048;
// A normalized 1024 × 1024 ICO can expand to about 5.6 MiB as a PNG data URL.
const MAX_DATA_URL_BYTES: u64 = 6 * 1024 * 1024;
const REFRESH_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
static WRITES: Mutex<()> = Mutex::new(());

pub(super) fn load(root: &Path, key: &str) -> Option<(SenderImage, bool)> {
    let reference = File::open(reference_path(root, key)).ok()?;
    let fresh = is_fresh(&reference);
    let mut value = String::new();
    reference.take(80).read_to_string(&mut value).ok()?;
    let (kind, hash) = value.split_once(':')?;
    let kind = match kind {
        "gravatar" => "gravatar",
        "favicon" => "favicon",
        _ => return None,
    };
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let file = File::open(root.join(format!("{hash}.img"))).ok()?;
    let stale = !fresh || !is_fresh(&file);
    let src = read_image(file)?;
    Some((SenderImage { src, kind }, stale))
}

pub(super) fn load_fresh_url(root: &Path, url: &str) -> Option<String> {
    let file = File::open(image_path(root, url)).ok()?;
    if !is_fresh(&file) {
        return None;
    }
    read_image(file)
}

fn is_fresh(file: &File) -> bool {
    file.metadata()
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|mtime| mtime.elapsed().ok())
        .is_some_and(|age| age < REFRESH_AGE)
}

fn read_image(file: File) -> Option<String> {
    let mut src = String::new();
    file.take(MAX_DATA_URL_BYTES + 1)
        .read_to_string(&mut src)
        .ok()?;
    let (mime, body) = src.split_once(";base64,")?;
    (src.len() as u64 <= MAX_DATA_URL_BYTES
        && matches!(
            mime,
            "data:image/png" | "data:image/jpeg" | "data:image/gif" | "data:image/webp"
        )
        && !body.is_empty())
    .then_some(src)
}

fn reference_path(root: &Path, key: &str) -> PathBuf {
    root.join(format!("{}.ref", sha256_hex(key.as_bytes())))
}

fn image_path(root: &Path, url: &str) -> PathBuf {
    root.join(format!("{}.img", sha256_hex(url.as_bytes())))
}

pub(super) fn save(root: &Path, key: &str, url: &str, image: &SenderImage, downloaded: bool) {
    if image.src.is_empty() || image.src.len() as u64 > MAX_DATA_URL_BYTES {
        return;
    }
    let _guard = WRITES.lock().unwrap();
    let mut dir = DirBuilder::new();
    dir.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        dir.mode(0o700);
    }
    if dir.create(root).is_err() {
        return;
    }

    let path = image_path(root, url);
    // Reusing a disk image for another sender must leave its download time intact.
    if (downloaded || !path.is_file()) && atomic_write(root, &path, image.src.as_bytes()).is_err() {
        return;
    }
    let value = format!("{}:{}", image.kind, sha256_hex(url.as_bytes()));
    let _ = atomic_write(root, &reference_path(root, key), value.as_bytes());
    prune(root, MAX_BYTES, MAX_FILES);
}

fn atomic_write(root: &Path, path: &Path, content: &[u8]) -> io::Result<()> {
    let temp = root.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content)?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn prune(root: &Path, max_bytes: u64, max_files: usize) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            meta.is_file().then(|| {
                (
                    meta.modified().unwrap_or(UNIX_EPOCH),
                    meta.len(),
                    entry.path(),
                )
            })
        })
        .collect();
    let mut bytes: u64 = files.iter().map(|file| file.1).sum();
    let mut count = files.len();
    if bytes <= max_bytes && count <= max_files {
        return;
    }
    files.sort_unstable();
    for (_, size, path) in files {
        if fs::remove_file(path).is_ok() {
            bytes -= size;
            count -= 1;
        }
        if bytes <= max_bytes && count <= max_files {
            break;
        }
    }
}

struct Refresh {
    root: PathBuf,
    key: String,
    email: String,
    size: u64,
}

impl Refresh {
    fn run(self, fetch: impl FnMut(&str) -> Result<Option<String>, ()>) {
        let (_, domain) = self.email.rsplit_once('@').unwrap();
        if let Ok((image, Some(url))) = resolve_network(&self.email, domain, self.size, true, fetch)
        {
            save(&self.root, &self.key, &url, &image, true);
        }
    }
}

pub(super) fn queue_refresh(root: &Path, key: &str, email: &str, size: u64) -> Result<(), ()> {
    static QUEUE: OnceLock<Option<mpsc::SyncSender<Refresh>>> = OnceLock::new();
    // One worker and a bounded queue keep a large inbox from starting a burst
    // of network calls on startup. The caller retries scheduling if the queue is full.
    let queue = QUEUE.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<Refresh>(32);
        thread::Builder::new()
            .name("sender-avatar-refresh".into())
            .spawn(move || {
                for job in rx {
                    job.run(fetch_cached);
                }
            })
            .ok()
            .map(|_| tx)
    });
    queue
        .as_ref()
        .ok_or(())?
        .try_send(Refresh {
            root: root.into(),
            key: key.into(),
            email: email.into(),
            size,
        })
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMAIL: &str = "sender@example.com";
    const KEY: &str = "sender:sender@example.com:96";
    const FAVICON_URL: &str = "https://icons.duckduckgo.com/ip3/example.com.ico";
    const IMAGE: &str = "data:image/png;base64,iVBORw0KGgo=";
    const UPDATED: &str = "data:image/jpeg;base64,/9j/";

    struct CacheDir(PathBuf);

    impl CacheDir {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("meron-avatars-{}", uuid::Uuid::new_v4())))
        }
    }

    impl Drop for CacheDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn desktop_resolver_reads_saved_avatar_without_an_in_memory_entry() {
        let CacheDir(ref root) = CacheDir::new();
        let image = SenderImage {
            src: IMAGE.into(),
            kind: "favicon",
        };
        save(root, KEY, FAVICON_URL, &image, true);
        let params = serde_json::json!({ "email": EMAIL, "size": 96 });
        let resolved = super::super::resolve_with_disk(&params, Some(root));
        assert_eq!((resolved.src.as_str(), resolved.kind), (IMAGE, "favicon"));
    }

    #[test]
    fn shared_favicon_has_one_image_file_and_expires_from_its_original_download() {
        let CacheDir(ref root) = CacheDir::new();
        let other_key = "sender:other@example.com:48";
        let image = SenderImage {
            src: IMAGE.into(),
            kind: "favicon",
        };
        save(root, KEY, FAVICON_URL, &image, true);
        assert_eq!(load_fresh_url(root, FAVICON_URL).as_deref(), Some(IMAGE));
        File::open(image_path(root, FAVICON_URL))
            .unwrap()
            .set_modified(UNIX_EPOCH)
            .unwrap();
        save(root, other_key, FAVICON_URL, &image, false);
        assert_eq!(fs::read_dir(root).unwrap().count(), 3);
        assert!(load_fresh_url(root, FAVICON_URL).is_none());
        for key in [KEY, other_key] {
            let (saved, stale) = load(root, key).unwrap();
            assert!(stale);
            assert_eq!(saved.src, IMAGE);
        }
    }

    #[test]
    fn failed_refresh_keeps_stale_image_and_success_replaces_it() {
        let CacheDir(ref root) = CacheDir::new();
        let image = SenderImage {
            src: IMAGE.into(),
            kind: "favicon",
        };
        let refresh = || Refresh {
            root: root.clone(),
            key: KEY.into(),
            email: EMAIL.into(),
            size: 96,
        };
        save(root, KEY, FAVICON_URL, &image, true);
        File::open(reference_path(root, KEY))
            .unwrap()
            .set_modified(UNIX_EPOCH)
            .unwrap();

        refresh().run(|_| Err(()));
        refresh().run(|url| {
            if url == FAVICON_URL {
                Ok(Some(UPDATED.into()))
            } else {
                Err(())
            }
        });
        let (saved, stale) = load(root, KEY).unwrap();
        assert!(stale);
        assert_eq!(saved.src, IMAGE);

        refresh().run(|_| Ok(Some(UPDATED.into())));
        let (saved, stale) = load(root, KEY).unwrap();
        assert!(!stale);
        assert_eq!((saved.src.as_str(), saved.kind), (UPDATED, "gravatar"));
    }

    #[test]
    fn eviction_trims_to_both_limits() {
        let CacheDir(ref root) = CacheDir::new();
        fs::create_dir(root).unwrap();
        let oldest = root.join("oldest");
        fs::write(&oldest, "a").unwrap();
        File::open(&oldest)
            .unwrap()
            .set_modified(UNIX_EPOCH)
            .unwrap();
        fs::write(root.join("newer"), "bc").unwrap();
        prune(root, 3, 1);
        assert!(!oldest.exists());
        assert!(root.join("newer").exists());
        prune(root, 1, 1);
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }
}
