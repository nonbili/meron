use super::*;

#[test]
fn request_defaults_missing_params_to_null() {
    let req: Request = serde_json::from_str(r#"{"id":7,"method":"ping"}"#).unwrap();
    assert_eq!(req.id, 7);
    assert_eq!(req.method, "ping");
    assert_eq!(req.params, Value::Null);
}

#[test]
fn ready_and_ping_expose_protocol_version() {
    assert_eq!(ready_event()["protocol"], PROTOCOL_VERSION);
    assert_eq!(ping_response()["protocol"], PROTOCOL_VERSION);
    assert_eq!(ping_response()["version"], VERSION);
}

#[test]
fn protocol_invoke_wraps_ping_response() {
    let value = invoke_protocol_json(r#"{"id":42,"method":"ping"}"#);
    assert_eq!(value["id"], 42);
    assert_eq!(value["result"]["pong"], true);
    assert_eq!(value["result"]["protocol"], PROTOCOL_VERSION);
}

#[test]
fn protocol_invoke_wraps_empty_account_list() {
    let value = invoke_protocol_json(r#"{"id":43,"method":"account.list","params":{}}"#);
    assert_eq!(value["id"], 43);
    assert_eq!(
        value["result"],
        json!({
            "accounts": []
        })
    );
}

#[test]
fn protocol_invoke_wraps_empty_mail_collections() {
    let cases = [
        (
            r#"{"id":44,"method":"mail.folderList","params":{"account_id":"acc"}}"#,
            json!({ "folders": [] }),
        ),
        (
            r#"{"id":45,"method":"mail.threadList","params":{"account_id":"acc","folder_id":"inbox"}}"#,
            json!({ "threads": [] }),
        ),
        (
            r#"{"id":46,"method":"mail.threadRead","params":{"thread_id":"thread"}}"#,
            json!({ "messages": [] }),
        ),
        (
            r#"{"id":47,"method":"mail.starredItems","params":{}}"#,
            json!({ "items": [] }),
        ),
        (
            r#"{"id":48,"method":"rss.exportOpml","params":{"account":"rss"}}"#,
            json!({ "opml": "" }),
        ),
    ];

    for (request, expected_result) in cases {
        let value = invoke_protocol_json(request);
        assert_eq!(value["result"], expected_result);
        assert!(value.get("error").is_none());
    }
}

#[test]
fn protocol_invoke_wraps_noop_mail_mutations() {
    let cases = [
        (
            r#"{"id":48,"method":"mail.send","params":{"account_id":"acc","to":"you@example.com"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":71,"method":"mail.saveDraft","params":{"account_id":"acc","draft_id":"draft@example.com"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":72,"method":"mail.discardDraft","params":{"account_id":"acc","draft_id":"draft@example.com"}}"#,
            json!({ "ok": true, "deleted": 0, "permanent": true }),
        ),
        (
            r#"{"id":54,"method":"rss.importOpml","params":{"account":"rss","opml":"<opml/>"}}"#,
            json!({ "imported": 0 }),
        ),
        (
            r#"{"id":73,"method":"account.addRSS","params":{"feed_url":"https://example.com/feed.xml","display_name":"News"}}"#,
            json!({ "account": Value::Null }),
        ),
        (
            r#"{"id":74,"method":"rss.addFeed","params":{"account_id":"rss","feed_url":"https://example.com/feed.xml"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":75,"method":"rss.removeFeed","params":{"thread_id":"rss#rss#feed"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":76,"method":"rss.moveFeed","params":{"thread_id":"rss#rss#feed","target_account_id":"rss2"}}"#,
            json!({ "ok": true, "moved": 0 }),
        ),
        (
            r#"{"id":176,"method":"rss.renameFeed","params":{"thread_id":"rss#rss#feed","title":"Feed"}}"#,
            json!({ "ok": true, "title": "" }),
        ),
        (
            r#"{"id":77,"method":"mail.readAttachment","params":{"key":"acc/INBOX/1/note.txt"}}"#,
            json!({ "data": "" }),
        ),
        (
            r#"{"id":55,"method":"account.remove","params":{"id":"acc"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":56,"method":"account.setName","params":{"id":"acc","name":"Personal"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":57,"method":"account.setSenderName","params":{"id":"acc","name":"Sender"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":58,"method":"account.setAvatar","params":{"id":"acc","avatar_url":"https://example.com/avatar.png"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":59,"method":"account.setChatWallpaper","params":{"id":"acc","wallpaper":{"kind":"preset","presetId":"grid"}}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":60,"method":"account.writeAvatarFile","params":{"id":"acc","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}}"#,
            json!({ "url": "" }),
        ),
        (
            r#"{"id":61,"method":"account.writeChatWallpaperFile","params":{"id":"acc","filename":"wallpaper.png","mime":"image/png","data":"aGVsbG8="}}"#,
            json!({ "url": "" }),
        ),
        (
            r#"{"id":62,"method":"account.setImages","params":{"id":"acc","enabled":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":63,"method":"account.setConversationHtml","params":{"id":"acc","enabled":false}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":64,"method":"account.setUnified","params":{"id":"acc","enabled":false}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":65,"method":"account.setMuted","params":{"id":"acc","enabled":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":66,"method":"account.setPaused","params":{"id":"acc","enabled":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":67,"method":"account.setRSSSyncInterval","params":{"id":"acc","minutes":30}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":68,"method":"account.setAliases","params":{"id":"acc","aliases":[]}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":69,"method":"account.reorder","params":{"accounts":["b","a"]}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":70,"method":"mail.suggestContacts","params":{"account":"acc","query":"a","limit":8}}"#,
            json!({ "contacts": [] }),
        ),
        (
            r#"{"id":49,"method":"mail.archive","params":{"thread_id":"thread"}}"#,
            json!({ "ok": true, "moved": 0 }),
        ),
        (
            r#"{"id":50,"method":"mail.delete","params":{"thread_id":"thread"}}"#,
            json!({ "ok": true, "deleted": 0 }),
        ),
        (
            r#"{"id":51,"method":"mail.markRead","params":{"thread_id":"thread","seen":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":52,"method":"mail.markAllRead","params":{"account_id":"acc","folder_id":"INBOX"}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":53,"method":"mail.markStarred","params":{"thread_id":"thread","starred":true}}"#,
            json!({ "ok": true }),
        ),
        (
            r#"{"id":54,"method":"mail.emptyFolder","params":{"account_id":"acc","folder_id":"Trash"}}"#,
            json!({ "ok": true, "deleted": 0, "expunged": 0 }),
        ),
    ];

    for (request, expected_result) in cases {
        let value = invoke_protocol_json(request);
        assert_eq!(value["result"], expected_result);
        assert!(value.get("error").is_none());
    }
}

#[test]
fn protocol_invoke_wraps_unknown_method_error() {
    let value = invoke_protocol_json(r#"{"id":9,"method":"missing"}"#);
    assert_eq!(value["id"], 9);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown method")
    );
}
