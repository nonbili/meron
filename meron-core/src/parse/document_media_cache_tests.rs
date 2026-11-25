use super::*;

#[test]
fn pruning_preserves_open_document_media() {
    let root = std::env::temp_dir().join(format!("meron-reader-prune-{}", uuid::Uuid::new_v4()));
    let document = root.join("_eml/reader/1/attachment.png");
    let cached = root.join("account/INBOX/1/attachment.png");
    for path in [&document, &cached] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    std::fs::write(&document, b"reader attachment").unwrap();
    // A sparse file exceeds the cache cap without allocating a GiB of data.
    std::fs::File::create(&cached)
        .unwrap()
        .set_len(media_cache_cap() + 1)
        .unwrap();
    prune_media_cache(&root);
    assert_eq!(std::fs::read(&document).unwrap(), b"reader attachment");
    assert!(!cached.exists());
    std::fs::remove_dir_all(root).unwrap();
}
