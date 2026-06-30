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

    let threads = recent(&conn, "rss-acct", "", 10).unwrap();
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
        recent(&conn, &account_id, "", 10).unwrap()
    };
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["from_name"], "Sample Feed");

    // A re-sync of the unchanged feed finds no new items (guids dedupe).
    let new_items = sync_account(&db, &account_id).unwrap();
    assert_eq!(
        new_items, 0,
        "unchanged feed yields no new items on re-sync"
    );
}
