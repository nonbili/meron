use super::*;

fn read_thread(conn: &Connection, thread_id: &str) -> Result<Vec<Value>> {
    let (items, _) = read_thread_page(conn, thread_id, None, None)?;
    Ok(items)
}

fn entry(id: &str, link: &str, title: &str, published: i64) -> feed_rs::model::Entry {
    let mut e = feed_rs::model::Entry::default();
    e.id = id.to_string();
    if !link.is_empty() {
        e.links = vec![feed_rs::model::Link {
            href: link.to_string(),
            rel: None,
            media_type: None,
            href_lang: None,
            title: None,
            length: None,
        }];
    }
    if !title.is_empty() {
        e.title = Some(feed_rs::model::Text {
            content_type: "text/plain".parse().unwrap(),
            src: None,
            content: title.to_string(),
        });
    }
    if published != 0 {
        e.published = chrono::DateTime::from_timestamp(published, 0);
    }
    e
}

#[test]
fn item_identity_prefers_guid_then_link_then_fallback() {
    let with_guid = parse_item(
        &entry("guid-1", "https://example.com/a", "A", 1700000000),
        1,
    );
    let changed_link = parse_item(
        &entry("guid-1", "https://example.com/changed", "A", 1700000000),
        1,
    );
    assert_eq!(
        with_guid.as_ref().unwrap().item_key,
        changed_link.as_ref().unwrap().item_key,
        "GUID should be stable across link changes"
    );

    let link_only = parse_item(&entry("", "https://example.com/a", "A", 0), 1);
    let same_link = parse_item(&entry("", "https://example.com/a", "Different", 0), 1);
    assert_eq!(
        link_only.as_ref().unwrap().item_key,
        same_link.as_ref().unwrap().item_key,
        "link should be stable when GUID is absent"
    );

    let fallback = parse_item(&entry("", "", "A", 1700000000), 1);
    assert!(!fallback.unwrap().item_key.is_empty());
}

#[test]
fn import_opml_accepts_inoreader_export() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-acct', 'rss', 'rss', 'RSS', '{}')",
        [],
    )
    .unwrap();
    let db = Mutex::new(conn);
    let opml = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="1.0">
  <head>
    <title>Feeds exported from Reader</title>
  </head>
  <body>
    <outline text="Tech" title="Tech">
      <outline text="Feed One" title="Feed One" type="rss" xmlUrl="https://feeds.example.test/one.xml" htmlUrl="https://example.test/one"/>
      <outline text="Feed Two" title="Feed Two" type="rss" xmlUrl="https://feeds.example.test/two.xml" htmlUrl="https://example.test/two"/>
    </outline>
    <outline text="News" title="News">
      <outline text="Feed Three" title="Feed Three" type="rss" xmlUrl="https://feeds.example.test/three.xml" htmlUrl="https://example.test/three"/>
    </outline>
  </body>
</opml>"#;

    assert_eq!(import_opml(&db, opml, "rss-acct").unwrap(), 3);
    let count: i64 = db
        .lock()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM subscriptions WHERE account = 'rss-acct'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 3);
}

#[test]
fn move_feed_reassigns_subscription_and_preserves_items() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-one', 'rss', 'rss', 'One', '{}'),
                   ('rss-two', 'rss', 'rss', 'Two', '{}')",
        [],
    )
    .unwrap();
    conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, feed_title, etag, last_modified, enabled)
             VALUES('feed-1', 'rss-one', 'https://example.com/feed', 'Example', 'Example Feed', 'etag-1', 'mod-1', 1)",
            [],
        )
        .unwrap();
    store::upsert_rss_item(
        &conn,
        "rss-one",
        "feed-1",
        "item-1",
        "Post",
        false,
        None,
        &RssItemExtra {
            author: "Author".to_string(),
            link: "https://example.com/post".to_string(),
            summary: "Summary".to_string(),
            content: String::new(),
            images: vec![],
            videos: vec![],
            published_at: 10,
            updated_at: 0,
            fetched_at: 11,
        },
    )
    .unwrap();
    conn.execute(
            "UPDATE messages SET starred = 1 WHERE account = 'rss-one' AND folder = 'feed-1' AND msg_id = 'item-1'",
            [],
        )
        .unwrap();

    let res = move_feed(&conn, "rss-one#rss#feed-1", "rss-two").unwrap();
    assert_eq!(res["thread_id"], "rss-two#rss#feed-1");

    let sub_account: String = conn
        .query_row(
            "SELECT account FROM subscriptions WHERE id = 'feed-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(sub_account, "rss-two");

    let (item_account, seen, starred): (String, i64, i64) = conn
            .query_row(
                "SELECT account, seen, starred FROM messages WHERE folder = 'feed-1' AND msg_id = 'item-1'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap();
    assert_eq!(item_account, "rss-two");
    assert_eq!(seen, 1);
    assert_eq!(starred, 1);
}

#[test]
fn move_feed_rejects_non_rss_target() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-one', 'rss', 'rss', 'One', '{}'),
                   ('mail-one', 'mail', 'custom', 'Mail', '{}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-one', 'https://example.com/feed', 'Example', 1)",
        [],
    )
    .unwrap();

    let err = move_feed(&conn, "rss-one#rss#feed-1", "mail-one").unwrap_err();
    assert!(err.to_string().contains("is not RSS"));
}

#[test]
fn normalize_adds_scheme_and_requires_host() {
    assert_eq!(
        normalize_feed_url("example.com/feed").unwrap(),
        "https://example.com/feed"
    );
    assert!(normalize_feed_url("   ").is_err());
    assert!(normalize_feed_url("ftp://example.com").is_err());
}

#[test]
fn discovers_feed_link_from_html_head() {
    let html = r#"<html><head><link rel="alternate" type="application/rss+xml" href="/feed.xml"></head><body></body></html>"#;
    assert_eq!(
        discover_feed_url("https://example.com/", html).as_deref(),
        Some("https://example.com/feed.xml")
    );
}

#[test]
fn new_account_ids_are_unique_and_prefixed() {
    let a = new_account_id();
    let b = new_account_id();
    assert!(a.starts_with("rss-"));
    assert!(b.starts_with("rss-"));
    assert_ne!(a, b, "each account gets a distinct id (no name grouping)");
}

#[test]
fn sniff_image_ext_recognizes_images_and_rejects_error_bodies() {
    assert_eq!(
        sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]),
        Some("jpg")
    );
    assert_eq!(sniff_image_ext(b"\x89PNG\r\n\x1a\n....."), Some("png"));
    assert_eq!(sniff_image_ext(b"GIF89a....."), Some("gif"));
    assert_eq!(
        sniff_image_ext(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
        Some("webp")
    );
    assert_eq!(sniff_image_ext(b"<svg xmlns=\"...\">"), Some("svg"));
    // The HTTP-200 text bodies that previously poisoned the cache.
    assert_eq!(sniff_image_ext(b"URL signature expired"), None);
    assert_eq!(sniff_image_ext(b""), None);
}

#[test]
fn clean_feed_text_strips_html() {
    assert_eq!(
        clean_feed_text("<p>Hello <strong>world</strong>.</p>"),
        "Hello world."
    );
    assert_eq!(
        clean_feed_text("<p>Line 1</p><p>Line 2</p>"),
        "Line 1\nLine 2"
    );
    assert_eq!(clean_feed_text("Hello<br>world"), "Hello\nworld");
}

#[test]
fn extract_image_urls_collects_dedupes_and_skips_data_uris() {
    let html = r#"<p>caption</p>
            <img src="https://cdn.example.com/a/1.jpg">
            <img src="https://cdn.example.com/a/2.png">
            <img src="https://cdn.example.com/a/1.jpg">
            <img src="data:image/png;base64,AAAA">
            <img src="https://cdn.example.com/open/rss" width="1" height="1">
            <img src="https://cdn.example.com/open/rss2" width="0" height="0">
            <img>"#;
    assert_eq!(
        extract_image_urls(html),
        vec![
            "https://cdn.example.com/a/1.jpg".to_string(),
            "https://cdn.example.com/a/2.png".to_string(),
        ]
    );
    assert!(extract_image_urls("  ").is_empty());
}

#[test]
fn read_thread_exposes_inline_images_as_url_attachments() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-acct', 'https://example.com/feed', 'Example Feed', 1)",
        [],
    )
    .unwrap();
    store::upsert_rss_item(
        &conn,
        "rss-acct",
        "feed-1",
        "item-1",
        "Post",
        true,
        None,
        &RssItemExtra {
            author: String::new(),
            link: "https://example.com/post".to_string(),
            summary: "Caption".to_string(),
            content: String::new(),
            images: vec![RssMedia {
                url: "https://cdn.example.com/p/1.jpg".to_string(),
                key: None,
            }],
            videos: Vec::new(),
            published_at: 1_700_000_000,
            updated_at: 0,
            fetched_at: 1_700_000_100,
        },
    )
    .unwrap();

    let items = read_thread(&conn, "rss-acct#rss#feed-1").unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["has_attachments"], true);
    let attachments = items[0]["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0]["url"], "https://cdn.example.com/p/1.jpg");
    assert_eq!(attachments[0]["filename"], "1.jpg");
    assert!(attachments[0]["key"].is_null());
}

#[test]
fn extract_video_urls_handles_src_and_source_children() {
    let html = r#"<p>caption</p>
            <video controls><source src="https://cdn.example.com/v/1.mp4"></video>
            <video src="https://cdn.example.com/v/2.webm" controls></video>
            <video><source src="https://cdn.example.com/v/1.mp4"></video>
            <video><source src="data:video/mp4;base64,AAAA"></video>"#;
    assert_eq!(
        extract_video_urls(html),
        vec![
            "https://cdn.example.com/v/1.mp4".to_string(),
            "https://cdn.example.com/v/2.webm".to_string(),
        ]
    );
    assert!(extract_video_urls("  ").is_empty());
}

#[test]
fn read_thread_exposes_inline_videos_as_remote_attachments() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-acct', 'https://example.com/feed', 'Example Feed', 1)",
        [],
    )
    .unwrap();
    store::upsert_rss_item(
        &conn,
        "rss-acct",
        "feed-1",
        "item-1",
        "Post",
        true,
        None,
        &RssItemExtra {
            author: String::new(),
            link: "https://example.com/post".to_string(),
            summary: "Caption".to_string(),
            content: String::new(),
            images: Vec::new(),
            videos: vec![RssMedia {
                url: "https://media.example.test/link/abc/media/1".to_string(),
                key: None,
            }],
            published_at: 1_700_000_000,
            updated_at: 0,
            fetched_at: 1_700_000_100,
        },
    )
    .unwrap();

    let items = read_thread(&conn, "rss-acct#rss#feed-1").unwrap();
    assert_eq!(items.len(), 1);
    let attachments = items[0]["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(
        attachments[0]["url"],
        "https://media.example.test/link/abc/media/1"
    );
    assert_eq!(attachments[0]["mime"], "video/mp4");
    assert!(attachments[0]["key"].is_null());
}

#[test]
fn starred_items_returns_only_starred_rows_across_subscriptions() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    for (sub, account, url, title) in [
        ("feed-1", "rss-acct", "https://example.com/feed", "Feed One"),
        (
            "feed-2",
            "rss-other",
            "https://other.example/feed",
            "Feed Two",
        ),
    ] {
        conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, enabled)
                 VALUES(?1, ?2, ?3, ?4, 1)",
            params![sub, account, url, title],
        )
        .unwrap();
    }
    let extra = |published_at: i64| RssItemExtra {
        author: String::new(),
        link: "https://example.com/post".to_string(),
        summary: "Caption".to_string(),
        content: String::new(),
        images: Vec::new(),
        videos: Vec::new(),
        published_at,
        updated_at: 0,
        fetched_at: published_at + 100,
    };
    store::upsert_rss_item(
        &conn,
        "rss-acct",
        "feed-1",
        "item-1",
        "Old starred",
        true,
        None,
        &extra(1_700_000_000),
    )
    .unwrap();
    store::upsert_rss_item(
        &conn,
        "rss-acct",
        "feed-1",
        "item-2",
        "Unstarred",
        true,
        None,
        &extra(1_700_000_500),
    )
    .unwrap();
    store::upsert_rss_item(
        &conn,
        "rss-other",
        "feed-2",
        "item-3",
        "New starred",
        true,
        None,
        &extra(1_700_001_000),
    )
    .unwrap();
    store::update_rss_item_starred(&conn, "rss-acct", "feed-1", "item-1", true).unwrap();
    store::update_rss_item_starred(&conn, "rss-other", "feed-2", "item-3", true).unwrap();

    let items = starred_items(&conn, 10).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["rss-other#rss#feed-2#item-3", "rss-acct#rss#feed-1#item-1",]
    );
    assert!(items.iter().all(|m| m["starred"] == true));
    assert_eq!(items[0]["from_name"], "Feed Two");
}

#[test]
fn recent_and_read_thread_build_bridge_messages() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, enabled)
             VALUES('feed-1', 'rss-acct', 'https://example.com/feed', 'Example Feed', 1)",
        [],
    )
    .unwrap();
    store::upsert_rss_item(
        &conn,
        "rss-acct",
        "feed-1",
        "item-1",
        "Hello RSS",
        true,
        None,
        &RssItemExtra {
            author: String::new(),
            link: "https://example.com/post".to_string(),
            summary: "Hello world.".to_string(),
            content: String::new(),
            images: Vec::new(),
            videos: Vec::new(),
            published_at: 1_700_000_000,
            updated_at: 0,
            fetched_at: 1_700_000_100,
        },
    )
    .unwrap();

    let threads = recent(&conn, "rss-acct", "", "", 10).unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["subject"], "Example Feed");
    assert_eq!(threads[0]["from_name"], "Example Feed");
    assert_eq!(threads[0]["preview"], "Hello RSS - Hello world.");
    assert_eq!(threads[0]["unread"], true);

    let thread_id = threads[0]["thread_id"].as_str().unwrap().to_string();
    assert!(parse_thread_id(&thread_id).is_some());

    let items = read_thread(&conn, &thread_id).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["subject"], "Hello RSS");
    assert_eq!(
        items[0]["body"],
        "Hello world.\n\nSource: https://example.com/post"
    );
    assert_eq!(items[0]["unread"], true);
}

// ---- test helpers for the additions below -------------------------------

fn rss_conn_with_feed(account: &str, sub: &str) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES(?1, 'rss', 'rss', 'RSS', '{}')",
        params![account],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, feed_title, enabled)
             VALUES(?1, ?2, 'https://example.com/feed', 'Example Feed', 'Example Feed', 1)",
        params![sub, account],
    )
    .unwrap();
    conn
}

fn insert_item(conn: &Connection, account: &str, sub: &str, key: &str, published_at: i64) {
    store::upsert_rss_item(
        conn,
        account,
        sub,
        key,
        key,
        true,
        None,
        &RssItemExtra {
            author: String::new(),
            link: "https://example.com/post".to_string(),
            summary: "Caption".to_string(),
            content: String::new(),
            images: Vec::new(),
            videos: Vec::new(),
            published_at,
            updated_at: 0,
            fetched_at: published_at + 100,
        },
    )
    .unwrap();
}

// ---- thread list filters -------------------------------------------------

#[test]
fn recent_filters_feeds_by_unread_and_starred() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, feed_title, enabled)
             VALUES('feed-2', 'rss-acct', 'https://example.com/two', 'Second Feed', 'Second Feed', 1)",
        [],
    )
    .unwrap();
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-2", "item-2", 200);
    // feed-2 is fully read but holds the only starred item.
    store::update_rss_item_seen(&conn, "rss-acct", "feed-2", "item-2", true).unwrap();
    store::update_rss_item_starred(&conn, "rss-acct", "feed-2", "item-2", true).unwrap();

    let titles = |filter: &str| {
        recent(&conn, "rss-acct", "", filter, 10)
            .unwrap()
            .iter()
            .map(|thread| thread["subject"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles("").len(), 2, "no filter keeps every feed");
    assert_eq!(titles("unread"), vec!["Example Feed".to_string()]);
    assert_eq!(titles("starred"), vec!["Second Feed".to_string()]);
    let starred = recent(&conn, "rss-acct", "", "starred", 10).unwrap();
    assert_eq!(starred[0]["starred"], Value::Null);
    assert_eq!(starred[0]["has_starred_items"], true);
}

// ---- read_thread_page pagination ----------------------------------------

#[test]
fn read_thread_page_walks_backwards_with_cursor() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);
    insert_item(&conn, "rss-acct", "feed-1", "item-3", 300);

    // First page (limit 2): the two newest items, ascending within the page,
    // plus a cursor pointing at the oldest item on the page.
    let (page1, cursor1) = read_thread_page(&conn, "rss-acct#rss#feed-1", None, Some(2)).unwrap();
    assert_eq!(
        page1
            .iter()
            .map(|m| m["subject"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["item-2", "item-3"],
    );
    let cursor1 = cursor1.expect("more items remain, cursor must be present");
    assert_eq!(cursor1, "ts:200:item-2");

    // Second page from that cursor: the remaining older item, no further cursor.
    let (page2, cursor2) = read_thread_page(
        &conn,
        "rss-acct#rss#feed-1",
        Some((200, "item-2".into())),
        Some(2),
    )
    .unwrap();
    assert_eq!(
        page2
            .iter()
            .map(|m| m["subject"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["item-1"],
    );
    assert!(cursor2.is_none(), "last page must not yield a cursor");
}

#[test]
fn read_thread_page_unpaginated_returns_all_ascending() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 300);
    insert_item(&conn, "rss-acct", "feed-1", "item-3", 200);

    let (items, cursor) = read_thread_page(&conn, "rss-acct#rss#feed-1", None, None).unwrap();
    assert!(cursor.is_none());
    assert_eq!(
        items
            .iter()
            .map(|m| m["subject"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["item-1", "item-3", "item-2"], // ts 100, 200, 300 ascending
    );
}

// ---- mark read / starred lifecycle --------------------------------------

fn seen_starred(conn: &Connection, sub: &str, key: &str) -> (i64, i64) {
    conn.query_row(
        "SELECT seen, starred FROM messages WHERE folder = ?1 AND msg_id = ?2",
        params![sub, key],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )
    .unwrap()
}

#[test]
fn mark_items_then_thread_read() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

    // Marking one item read leaves the other untouched.
    mark_items_read(&conn, "rss-acct#rss#feed-1", &["item-1".into()], true).unwrap();
    assert_eq!(seen_starred(&conn, "feed-1", "item-1").0, 1);
    assert_eq!(seen_starred(&conn, "feed-1", "item-2").0, 0);

    // Marking the thread read flips the whole feed.
    mark_thread_read(&conn, "rss-acct#rss#feed-1", true).unwrap();
    assert_eq!(seen_starred(&conn, "feed-1", "item-1").0, 1);
    assert_eq!(seen_starred(&conn, "feed-1", "item-2").0, 1);
}

#[test]
fn marking_a_feed_unread_flags_only_its_newest_item() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);
    mark_thread_read(&conn, "rss-acct#rss#feed-1", true).unwrap();

    // "Mark unread" means "bring this feed back", not "I read none of these":
    // the card shows one unread item and opening it lands on that item.
    mark_thread_read(&conn, "rss-acct#rss#feed-1", false).unwrap();

    assert_eq!(seen_starred(&conn, "feed-1", "item-1").0, 1);
    assert_eq!(seen_starred(&conn, "feed-1", "item-2").0, 0);
}

#[test]
fn mark_account_read_clears_every_feed_without_touching_other_accounts() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-other', 'rss', 'rss', 'Other', '{}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO subscriptions(id, account, url, title, feed_title, etag, last_modified, enabled)
             VALUES('feed-other', 'rss-other', 'https://example.com/other', 'Other', 'Other', '', '', 1)",
        [],
    )
    .unwrap();
    insert_item(&conn, "rss-other", "feed-other", "item-other", 300);

    assert_eq!(mark_account_read(&conn, "rss-acct").unwrap(), 2);
    assert_eq!(unread_count(&conn, "rss-acct").unwrap(), 0);
    assert_eq!(unread_count(&conn, "rss-other").unwrap(), 1);
}

#[test]
fn mark_account_read_rejects_non_rss_account_ids() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);

    assert!(mark_account_read(&conn, "mail-acct").is_err());
    assert_eq!(unread_count(&conn, "rss-acct").unwrap(), 1);
}

#[test]
fn mark_items_then_thread_starred() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

    mark_items_starred(&conn, "rss-acct#rss#feed-1", &["item-2".into()], true).unwrap();
    assert_eq!(seen_starred(&conn, "feed-1", "item-1").1, 0);
    assert_eq!(seen_starred(&conn, "feed-1", "item-2").1, 1);

    mark_thread_starred(&conn, "rss-acct#rss#feed-1", true).unwrap();
    assert_eq!(seen_starred(&conn, "feed-1", "item-1").1, 1);
    assert_eq!(seen_starred(&conn, "feed-1", "item-2").1, 1);
}

#[test]
fn mark_rejects_invalid_thread_id() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    assert!(mark_thread_read(&conn, "not-an-rss-id", true).is_err());
    assert!(mark_items_read(&conn, "bad", &["x".into()], true).is_err());
}

// ---- remove_feed --------------------------------------------------------

#[test]
fn remove_feed_deletes_subscription_and_items() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    insert_item(&conn, "rss-acct", "feed-1", "item-1", 100);
    insert_item(&conn, "rss-acct", "feed-1", "item-2", 200);

    remove_feed(&conn, "rss-acct#rss#feed-1").unwrap();

    let subs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM subscriptions WHERE id = 'feed-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let msgs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE folder = 'feed-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(subs, 0, "subscription row must be deleted");
    assert_eq!(msgs, 0, "the feed's cached items must be deleted");
}

#[test]
fn remove_feed_rejects_invalid_thread_id() {
    let conn = rss_conn_with_feed("rss-acct", "feed-1");
    assert!(remove_feed(&conn, "garbage").is_err());
}

// ---- OPML export round-trips with import --------------------------------

#[test]
fn export_opml_round_trips_through_import() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-src', 'rss', 'rss', 'My Feeds', '{}')",
        [],
    )
    .unwrap();
    // Two feeds with an XML-special character in a title to exercise escaping.
    conn.execute(
            "INSERT INTO subscriptions(id, account, url, title, feed_title, site_url, enabled)
             VALUES('feed-1', 'rss-src', 'https://a.example/feed.xml', 'News & Co', 'News', 'https://a.example', 1),
                   ('feed-2', 'rss-src', 'https://b.example/feed.xml', 'Tech', 'Tech', '', 1)",
            [],
        )
        .unwrap();

    let opml = export_opml(&conn, "rss-src").unwrap();
    assert!(
        opml.contains("News &amp; Co"),
        "title must be XML-escaped: {opml}"
    );
    assert!(opml.contains("xmlUrl=\"https://a.example/feed.xml\""));

    // Import into a *separate* database — the `url` column is globally UNIQUE,
    // so re-importing the same feeds into the same DB is (correctly) a no-op.
    // The real round-trip is export-here, import-on-another-machine.
    let dst = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&dst).unwrap();
    dst.execute(
        "INSERT INTO accounts(id, engine, provider, display_name, config)
             VALUES('rss-dst', 'rss', 'rss', 'Dest', '{}')",
        [],
    )
    .unwrap();
    let db = Mutex::new(dst);
    let imported = import_opml(&db, &opml, "rss-dst").unwrap();
    assert_eq!(
        imported, 2,
        "both feeds must round-trip into the new account"
    );

    let urls: Vec<String> = {
        let conn = db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT url FROM subscriptions WHERE account = 'rss-dst' ORDER BY url")
            .unwrap();

        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert_eq!(
        urls,
        vec![
            "https://a.example/feed.xml".to_string(),
            "https://b.example/feed.xml".to_string(),
        ]
    );
}

#[test]
fn export_opml_rejects_unknown_account() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    assert!(export_opml(&conn, "nope").is_err());
}

// ---- small pure helpers -------------------------------------------------

#[test]
fn first_line_collapses_whitespace_and_truncates() {
    assert_eq!(first_line("  hello   world \n again "), "hello world again");
    let long = "x".repeat(300);
    let preview = first_line(&long);
    assert_eq!(preview.chars().count(), 223); // 220 chars + "..."
    assert!(preview.ends_with("..."));
}

#[test]
fn mime_guesses_from_extension() {
    assert_eq!(image_mime("https://x/a.PNG?v=2"), "image/png");
    assert_eq!(image_mime("https://x/a.webp"), "image/webp");
    assert_eq!(image_mime("https://x/a.bin"), "image/jpeg"); // default
    assert_eq!(video_mime("https://x/a.webm"), "video/webm");
    assert_eq!(video_mime("https://x/a"), "video/mp4"); // default
}

// ---- end-to-end feed fetch over a real HTTP server ----------------------

/// Spawn a throwaway HTTP/1.1 server that replies to every request with the
/// same body. Returns the feed URL. The thread is detached (dies with the
/// test process). Exercises the real `ureq` fetch path without any network.
fn serve_feed(body: &'static str) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf); // drain the request line/headers
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}/feed.xml")
}

const SAMPLE_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Sample Feed</title>
    <link>https://example.com</link>
    <item>
      <title>First post</title>
      <link>https://example.com/1</link>
      <guid>guid-1</guid>
      <description>Hello from the first post.</description>
    </item>
    <item>
      <title>Second post</title>
      <link>https://example.com/2</link>
      <guid>guid-2</guid>
      <description>And the second.</description>
    </item>
  </channel>
</rss>"#;

#[test]
fn add_fetches_parses_and_syncs_over_http() {
    // Keep prune_feed_media (called by sync_account) confined to a temp dir
    // instead of the real media cache.
    let media = std::env::temp_dir().join(format!("meron-rss-test-{}", std::process::id()));
    unsafe { std::env::set_var("MERON_MEDIA_DIR", &media) };

    let url = serve_feed(SAMPLE_RSS);
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    let db = Mutex::new(conn);

    // add() fetches the feed, creates the account, and stores both items.
    let account = add(&db, &url, "Sample Feed").unwrap();
    let account_id = account["id"].as_str().unwrap().to_string();
    assert!(account_id.starts_with("rss-"));

    let (subs, msgs): (i64, i64) = {
        let conn = db.lock().unwrap();
        let subs = conn
            .query_row(
                "SELECT COUNT(*) FROM subscriptions WHERE account = ?1",
                params![account_id],
                |r| r.get(0),
            )
            .unwrap();
        let msgs = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE account = ?1",
                params![account_id],
                |r| r.get(0),
            )
            .unwrap();
        (subs, msgs)
    };
    assert_eq!(subs, 1, "one subscription created");
    assert_eq!(msgs, 2, "both feed items stored");

    // The parsed items surface as a thread with the feed's titles.
    let threads = {
        let conn = db.lock().unwrap();
        recent(&conn, &account_id, "", "", 10).unwrap()
    };
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["from_name"], "Sample Feed");

    // A re-sync of the unchanged feed finds no new items (guids dedupe).
    let new_items = sync_account(&db, &account_id).unwrap();
    assert!(
        new_items.is_empty(),
        "unchanged feed yields no new items on re-sync"
    );
}

/// Spawn a throwaway HTTP/1.1 server that serves `bodies` in order, one per
/// request, repeating the last body once the list runs out. Lets a test fetch a
/// feed, then re-fetch it after a new entry appeared.
fn serve_feed_sequence(bodies: Vec<&'static str>) -> String {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut served = 0usize;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf); // drain the request line/headers
            let body = bodies[served.min(bodies.len() - 1)];
            served += 1;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}/feed.xml")
}

/// A feed whose only entry is dated *later* than the arrival the other feed
/// publishes below, so it wins any "newest stored item" ordering.
const LATE_DATED_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Late Dated Feed</title>
    <link>https://late.example.com</link>
    <item>
      <title>Published yesterday</title>
      <link>https://late.example.com/1</link>
      <guid>late-1</guid>
      <pubDate>Tue, 25 Aug 2026 12:00:00 GMT</pubDate>
      <description>Already seen, already read.</description>
    </item>
  </channel>
</rss>"#;

const SLOW_FEED_BEFORE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Slow Feed</title>
    <link>https://slow.example.com</link>
    <item>
      <title>An older post</title>
      <link>https://slow.example.com/1</link>
      <guid>slow-1</guid>
      <pubDate>Thu, 20 Aug 2026 12:00:00 GMT</pubDate>
      <description>Nothing new here.</description>
    </item>
  </channel>
</rss>"#;

/// The same feed after publishing an entry dated *before* the other feed's
/// entry — the arrival a notification must name.
const SLOW_FEED_AFTER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Slow Feed</title>
    <link>https://slow.example.com</link>
    <item>
      <title>The actual arrival</title>
      <link>https://slow.example.com/2</link>
      <guid>slow-2</guid>
      <pubDate>Mon, 24 Aug 2026 12:00:00 GMT</pubDate>
      <description>This is what just landed.</description>
    </item>
    <item>
      <title>An older post</title>
      <link>https://slow.example.com/1</link>
      <guid>slow-1</guid>
      <pubDate>Thu, 20 Aug 2026 12:00:00 GMT</pubDate>
      <description>Nothing new here.</description>
    </item>
  </channel>
</rss>"#;

/// A sync reports the entries that actually arrived, not the account's
/// newest-dated stored row. The two differ whenever a feed publishes with a
/// timestamp older than something another feed already stored, which used to
/// make the notification name the wrong feed and open the wrong thread.
#[test]
fn sync_reports_arrivals_not_the_newest_stored_item() {
    let media = std::env::temp_dir().join(format!("meron-rss-arrivals-{}", std::process::id()));
    unsafe { std::env::set_var("MERON_MEDIA_DIR", &media) };

    let late_url = serve_feed(LATE_DATED_RSS);
    let slow_url = serve_feed_sequence(vec![SLOW_FEED_BEFORE, SLOW_FEED_AFTER]);
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    let db = Mutex::new(conn);

    let account = add(&db, &late_url, "Feeds").unwrap();
    let account_id = account["id"].as_str().unwrap().to_string();
    add_feed(&db, &account_id, &slow_url).unwrap();

    // Both feeds are re-fetched; only the slow feed gained an entry, and its
    // date sorts below the late feed's already-stored (and read) entry.
    let new_items = sync_account(&db, &account_id).unwrap();
    assert_eq!(new_items.len(), 1, "one genuine arrival");
    assert_eq!(new_items[0].title, "The actual arrival");
    assert_eq!(new_items[0].feed_title, "Slow Feed");
    assert_eq!(new_items[0].preview, "This is what just landed.");
    assert_eq!(
        new_items[0].subscription_id,
        rss_subscription_id(&normalize_feed_url(&slow_url).unwrap()),
        "tapping the notification opens the feed the entry landed in"
    );

    // The trap: the account's newest-dated row belongs to the *other* feed, so
    // anything that picks the notification's subject by date lands on it.
    let newest_stored: String = {
        let conn = db.lock().unwrap();
        conn.query_row(
            "SELECT subject FROM messages WHERE account = ?1 ORDER BY date DESC, id DESC LIMIT 1",
            params![account_id],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert_eq!(newest_stored, "Published yesterday");

    let stored_key: String = {
        let conn = db.lock().unwrap();
        conn.query_row(
            "SELECT msg_id FROM messages WHERE account = ?1 AND subject = ?2",
            params![account_id, "The actual arrival"],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert_eq!(
        new_items[0].item_key, stored_key,
        "the arrival carries the key its stored row is identified by"
    );

    let detail = new_items_detail(&account_id, "Feeds", false, &new_items).unwrap();
    assert_eq!(detail["from"], "Slow Feed");
    assert_eq!(detail["subject"], "The actual arrival");
    assert_eq!(detail["count"], 1);
    assert_eq!(detail["threadKey"], new_items[0].subscription_id);
}

fn new_item(feed: &str, title: &str, date: i64) -> NewItem {
    NewItem {
        subscription_id: format!("sub-{feed}"),
        item_key: format!("key-{feed}-{title}"),
        feed_title: feed.to_string(),
        title: title.to_string(),
        preview: format!("{title} preview"),
        date,
    }
}

/// The detail describes the newest arrival at the top level (what the desktop
/// bridge reads) and every arrival in `messages` (what per-message clients read).
#[test]
fn new_items_detail_describes_every_arrival() {
    let items = vec![
        new_item("Feed A", "Newest", 300),
        new_item("Feed B", "Middle", 200),
        new_item("Feed A", "Oldest", 100),
    ];
    let detail = new_items_detail("rss-1", "My Feeds", false, &items).unwrap();

    assert_eq!(detail["account"], "rss-1");
    assert_eq!(detail["accountName"], "My Feeds");
    assert_eq!(detail["folder"], "inbox");
    assert_eq!(detail["count"], 3);
    assert_eq!(detail["muted"], false);
    assert_eq!(detail["from"], "Feed A");
    assert_eq!(detail["subject"], "Newest");
    assert_eq!(detail["preview"], "Newest preview");
    assert_eq!(detail["threadKey"], "sub-Feed A");

    let messages = detail["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1]["from"], "Feed B");
    assert_eq!(messages[1]["subject"], "Middle");
    assert_eq!(messages[1]["preview"], "Middle preview");
    assert_eq!(messages[1]["threadKey"], "sub-Feed B");
    assert_eq!(messages[1]["date"], 200);
    assert_eq!(messages[1]["itemKey"], "key-Feed B-Middle");
}

/// Two entries of one feed that happen to share a title are still told apart.
/// A feed is a single thread, so the thread key cannot separate them, and a
/// client keying a notification on thread + subject would have the second
/// arrival silently replace the first.
#[test]
fn new_items_detail_identifies_same_titled_arrivals_separately() {
    let mut first = new_item("Feed A", "Daily digest", 200);
    first.item_key = "key-monday".to_string();
    let mut second = new_item("Feed A", "Daily digest", 100);
    second.item_key = "key-tuesday".to_string();
    let detail = new_items_detail("rss-1", "My Feeds", false, &[first, second]).unwrap();

    let messages = detail["messages"].as_array().unwrap();
    assert_eq!(messages[0]["subject"], messages[1]["subject"]);
    assert_eq!(messages[0]["threadKey"], messages[1]["threadKey"]);
    assert_eq!(messages[0]["itemKey"], "key-monday");
    assert_eq!(messages[1]["itemKey"], "key-tuesday");
}

/// A batch bigger than the per-message cap still reports its true size, and the
/// muted flag rides along so the bridge can suppress the OS notification.
#[test]
fn new_items_detail_caps_listed_messages_and_carries_muted() {
    let items: Vec<NewItem> = (0..crate::mail_model::NEW_MESSAGES_DETAIL_MAX + 3)
        .map(|i| new_item("Feed", &format!("Item {i}"), 1000 - i as i64))
        .collect();
    let detail = new_items_detail("rss-1", "My Feeds", true, &items).unwrap();

    assert_eq!(detail["count"], items.len());
    assert_eq!(
        detail["messages"].as_array().unwrap().len(),
        crate::mail_model::NEW_MESSAGES_DETAIL_MAX
    );
    assert_eq!(detail["muted"], true);
}

/// No arrivals means no notification at all — the caller falls back to a silent
/// `mail.synced` refresh.
#[test]
fn new_items_detail_is_none_without_arrivals() {
    assert!(new_items_detail("rss-1", "My Feeds", false, &[]).is_none());
}

/// The mobile `rss.sync` response carries the same `new_messages` detail
/// `mail.sync` does. Android's periodic worker notifies from the response alone
/// — it has no live event listener, and periodic refresh is the only thing that
/// syncs feeds in the background (an RSS account has nothing to IDLE on), so
/// without this feeds refreshed silently and never raised a notification.
#[test]
fn mobile_rss_sync_response_carries_new_arrivals() {
    let media = std::env::temp_dir().join(format!("meron-rss-mobile-{}", std::process::id()));
    unsafe { std::env::set_var("MERON_MEDIA_DIR", &media) };

    let url = serve_feed_sequence(vec![SLOW_FEED_BEFORE, SLOW_FEED_AFTER]);
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    let db = Mutex::new(conn);
    let account = add(&db, &url, "My Feeds").unwrap();
    let account_id = account["id"].as_str().unwrap().to_string();

    let conn = db.into_inner().unwrap();
    let response = crate::protocol::sync_mobile_rss_with_conn(conn, account_id.clone()).unwrap();

    assert_eq!(response["synced"], 1);
    let detail = &response["new_messages"];
    assert_eq!(detail["account"], account_id.as_str());
    // An RSS account has no email address, so the label is its display name.
    assert_eq!(detail["accountName"], "My Feeds");
    assert_eq!(detail["from"], "Slow Feed");
    assert_eq!(detail["subject"], "The actual arrival");
    assert_eq!(detail["count"], 1);
    assert_eq!(detail["muted"], false);
    assert_eq!(detail["messages"].as_array().unwrap().len(), 1);
}

/// A muted RSS account still syncs — the feed list refreshes — it just carries
/// the flag the clients read to skip the OS notification.
#[test]
fn mobile_rss_sync_marks_a_muted_account() {
    let media = std::env::temp_dir().join(format!("meron-rss-muted-{}", std::process::id()));
    unsafe { std::env::set_var("MERON_MEDIA_DIR", &media) };

    let url = serve_feed_sequence(vec![SLOW_FEED_BEFORE, SLOW_FEED_AFTER]);
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    let db = Mutex::new(conn);
    let account = add(&db, &url, "My Feeds").unwrap();
    let account_id = account["id"].as_str().unwrap().to_string();

    let conn = db.into_inner().unwrap();
    crate::store::set_account_pref(&conn, &account_id, "muted", true).unwrap();
    let response = crate::protocol::sync_mobile_rss_with_conn(conn, account_id).unwrap();

    assert_eq!(response["synced"], 1);
    assert_eq!(response["new_messages"]["muted"], true);
}

/// Nothing new means no `new_messages` at all, so a background refresh over
/// unchanged feeds stays silent.
#[test]
fn mobile_rss_sync_omits_new_messages_without_arrivals() {
    let media = std::env::temp_dir().join(format!("meron-rss-quiet-{}", std::process::id()));
    unsafe { std::env::set_var("MERON_MEDIA_DIR", &media) };

    let url = serve_feed(SLOW_FEED_BEFORE);
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    let db = Mutex::new(conn);
    let account = add(&db, &url, "My Feeds").unwrap();
    let account_id = account["id"].as_str().unwrap().to_string();

    let conn = db.into_inner().unwrap();
    let response = crate::protocol::sync_mobile_rss_with_conn(conn, account_id).unwrap();

    assert_eq!(response["synced"], 0);
    assert!(
        response.get("new_messages").is_none(),
        "an unchanged feed raises no notification"
    );
}
