use super::*;

#[test]
fn mobile_protocol_routes_rss_accounts_through_mail_shapes() {
    let data_dir = unique_data_dir("rss-mail-shapes");
    seed_rss_account(&data_dir, "rss-account", "News");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    seed_rss_feed(&conn, "rss-account", "feed-a", "Feed A");
    seed_rss_item(
        &conn,
        "rss-account",
        "feed-a",
        "item-old",
        "Old item",
        100,
        true,
    );
    seed_rss_item(
        &conn,
        "rss-account",
        "feed-a",
        "item-new",
        "New item",
        200,
        true,
    );
    drop(conn);

    let folders = invoke_mobile_protocol_json(
        r#"{"id":86,"method":"mail.folderList","params":{"account_id":"rss-account"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(folders["id"], 86);
    assert_eq!(folders["result"]["folders"][0]["id"], "inbox");
    assert_eq!(folders["result"]["folders"][0]["role"], "inbox");
    assert_eq!(folders["result"]["folders"][0]["unread"], 2);

    let threads = invoke_mobile_protocol_json(
        r#"{"id":87,"method":"mail.threadList","params":{"account_id":"rss-account","folder_id":"inbox","filter":"all"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(threads["id"], 87);
    assert_eq!(threads["result"]["folder_unread"], 2);
    let first = &threads["result"]["threads"][0];
    assert_eq!(first["thread_id"], "rss-account#rss#feed-a");
    assert_eq!(first["account_id"], "rss-account");
    assert_eq!(first["folder_id"], "inbox");
    assert_eq!(first["from_name"], "Feed A");
    assert_eq!(first["subject"], "Feed A");
    assert_eq!(first["unread"], true);
    assert_eq!(first["unread_count"], 2);

    let read = invoke_mobile_protocol_json(
        r#"{"id":88,"method":"mail.threadRead","params":{"thread_id":"rss-account#rss#feed-a","limit":1}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(read["id"], 88);
    assert_eq!(
        read["result"]["messages"][0]["id"],
        "rss-account#rss#feed-a#item-new"
    );
    assert_eq!(read["result"]["messages"][0]["subject"], "New item");
    assert_eq!(read["result"]["messages"][0]["body"], "Body for New item");
    assert_eq!(
        read["result"]["messages"][0]["link"],
        "https://example.com/item-new"
    );
    assert_eq!(read["result"]["messages"][0]["unread"], true);
    assert_eq!(read["result"]["next_cursor"], "ts:200:item-new");

    let mark_read = invoke_mobile_protocol_json(
        r#"{"id":89,"method":"mail.markRead","params":{"thread_id":"rss-account#rss#feed-a","seen":true,"message_ids":["rss-account#rss#feed-a#item-new"]}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(mark_read["result"]["ok"], true);

    let mark_starred = invoke_mobile_protocol_json(
        r#"{"id":90,"method":"mail.markStarred","params":{"thread_id":"rss-account#rss#feed-a","starred":true,"item_keys":["item-new"]}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(mark_starred["result"]["ok"], true);

    let reread = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"rss.thread","params":{"thread_id":"rss-account#rss#feed-a"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    let messages = reread["result"]["messages"].as_array().unwrap();
    let new_item = messages
        .iter()
        .find(|message| message["id"] == "rss-account#rss#feed-a#item-new")
        .unwrap();
    assert_eq!(new_item["unread"], false);
    assert_eq!(new_item["starred"], true);

    let starred = invoke_mobile_protocol_json(
        r#"{"id":92,"method":"mail.starredItems","params":{"limit":5}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(
        starred["result"]["items"][0]["id"],
        "rss-account#rss#feed-a#item-new"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_exports_and_imports_opml() {
    let data_dir = unique_data_dir("rss-opml-mobile");
    seed_rss_account(&data_dir, "rss-source", "Source feeds");
    seed_rss_account(&data_dir, "rss-target", "Target feeds");
    let conn = store::open_at(data_dir.join("meron.db")).unwrap();
    seed_rss_feed(&conn, "rss-source", "feed-a", "Feed A");
    drop(conn);

    let exported = invoke_mobile_protocol_json(
        r#"{"id":91,"method":"rss.exportOpml","params":{"account":"rss-source"}}"#,
        Some(data_dir.to_str().unwrap()),
    );
    assert_eq!(exported["id"], 91);
    let opml = exported["result"]["opml"].as_str().unwrap();
    assert!(opml.contains("<opml version=\"2.0\">"));
    assert!(opml.contains("Feed A"));

    let request = serde_json::json!({
            "id": 92,
            "method": "rss.importOpml",
            "params": {
                "account": "rss-target",
                "opml": r#"<?xml version="1.0"?><opml version="2.0"><body><outline text="Imported"><outline type="rss" text="Imported Feed" xmlUrl="https://import.example/feed.xml"/></outline></body></opml>"#,
            },
        })
        .to_string();
    let imported = invoke_mobile_protocol_json(&request, Some(data_dir.to_str().unwrap()));
    assert_eq!(imported["id"], 92);
    assert_eq!(imported["result"]["imported"], 1);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn mobile_protocol_syncs_empty_rss_account_without_mail_credentials() {
    let data_dir = unique_data_dir("rss-sync-empty");
    seed_rss_account(&data_dir, "rss-empty", "RSS");

    let value = invoke_mobile_protocol_json(
        r#"{"id":93,"method":"mail.sync","params":{"account_id":"rss-empty"}}"#,
        Some(data_dir.to_str().unwrap()),
    );

    assert_eq!(value["id"], 93);
    assert_eq!(value["result"]["ok"], true);
    assert_eq!(value["result"]["account"], "rss-empty");
    assert_eq!(value["result"]["synced"], 0);
    assert_eq!(value["result"]["rss"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}
