use super::{header_is_message, tcp_looks_open};

#[test]
fn a_trash_folder_is_recognised_by_its_localized_name() {
    for name in [
        "Trash",
        "INBOX.Trash",
        "INBOX/Trash",
        "[Gmail]/Bin",
        "Deleted Items",
        "Papierkorb",
        "INBOX.Papierkorb",
        "Gel&APY-scht",
        "Gel&APY-schte Elemente",
        "Corbeille",
    ] {
        assert!(super::looks_like_trash(name), "{name}");
    }
    for name in [
        "INBOX",
        "Projects/Trash",
        "Trash/2024",
        "Entw&APw-rfe",
        "Gesendet",
    ] {
        assert!(!super::looks_like_trash(name), "{name}");
    }
}

#[test]
fn a_rotated_refresh_token_is_kept_and_an_absent_one_is_not_invented() {
    let rotated: super::TokenResponse = serde_json::from_str(
        r#"{"access_token":"a2","expires_in":1800,"refresh_token":"r2","scope":"x"}"#,
    )
    .unwrap();
    let rotated = super::RefreshedToken::from(rotated);
    assert_eq!(rotated.access_token, "a2");
    assert_eq!(rotated.expires_in, 1800);
    assert_eq!(rotated.refresh_token.as_deref(), Some("r2"));

    let plain: super::TokenResponse = serde_json::from_str(r#"{"access_token":"a2"}"#).unwrap();
    let plain = super::RefreshedToken::from(plain);
    assert_eq!(plain.expires_in, 3600);
    assert_eq!(plain.refresh_token, None);

    let blank: super::TokenResponse =
        serde_json::from_str(r#"{"access_token":"a2","refresh_token":" "}"#).unwrap();
    assert_eq!(super::RefreshedToken::from(blank).refresh_token, None);
}

#[test]
fn only_a_refused_grant_counts_as_rejected() {
    let refused = super::oauth_refresh_error(
        "400 Bad Request",
        r#"{"error":"invalid_grant","error_description":"expired"}"#,
    );
    assert!(super::oauth_grant_rejected(&refused));
    assert!(
        refused
            .to_string()
            .starts_with("oauth refresh failed (400 Bad Request)")
    );
    // Wrapped the way callers add context.
    assert!(super::oauth_grant_rejected(&refused.context("sync INBOX")));

    // The text alone is not the verdict: it has to be the error code.
    let mentioned = super::oauth_refresh_error(
        "500 Internal Server Error",
        r#"{"error":"server_error","error_description":"invalid_grant lookup failed"}"#,
    );
    assert!(!super::oauth_grant_rejected(&mentioned));
    let gateway = super::oauth_refresh_error("502 Bad Gateway", "<html>invalid_grant</html>");
    assert!(!super::oauth_grant_rejected(&gateway));
    let offline = anyhow::anyhow!("oauth refresh request: connection refused");
    assert!(!super::oauth_grant_rejected(&offline));
}

#[tokio::test(flavor = "multi_thread")]
async fn tcp_looks_open_spots_a_connection_the_peer_closed() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (server, _) = listener.accept().await.unwrap();
    assert!(tcp_looks_open(&client));

    drop(server);
    // The reactor records the FIN asynchronously.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while tcp_looks_open(&client) {
        assert!(std::time::Instant::now() < deadline, "close never observed");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

use super::{
    MIN_PROTOCOL_TIMEOUT, attachment_files, civil_from_days, fetch_items, first_message_id,
    gmail_raw, header_fields, imap_quote, interleave_address_families, looks_like_drafts,
    message_id_search_criteria, normalize_message_id, prefetch_search_criteria,
    protocol_timeout_for, search_criteria, thread_key, uid_set_chunks,
};

#[test]
fn prefetch_search_includes_recent_read_mail() {
    assert_eq!(prefetch_search_criteria("01-Oct-2026"), "SINCE 01-Oct-2026");
}

#[test]
fn bodystructure_attachments_include_unopened_nested_files() {
    use async_imap::imap_proto::{
        parser::parse_response,
        types::{AttributeValue, Response},
    };
    let response = br#"* 1 FETCH (BODYSTRUCTURE (("TEXT" "PLAIN" NIL NIL NIL "7BIT" 10 1)(("IMAGE" "PNG" ("NAME" "logo.png") "<logo>" NIL "BASE64" 10 NIL ("INLINE" ("FILENAME" "logo.png")))("APPLICATION" "PDF" ("NAME" "report.pdf") NIL NIL "BASE64" 20 NIL ("ATTACHMENT" ("FILENAME" "report.pdf"))) "RELATED")("TEXT" "PLAIN" ("NAME" "notes.txt") NIL NIL "7BIT" 10 1 NIL ("ATTACHMENT" ("FILENAME" "notes.txt"))) "MIXED"))
"#;
    // IMAP responses use CRLF, including the mock response here.
    let response = String::from_utf8_lossy(response).replace('\n', "\r\n");
    let (_, Response::Fetch(_, attributes)) = parse_response(response.as_bytes()).unwrap() else {
        panic!("expected FETCH")
    };
    let body = attributes
        .iter()
        .find_map(|attr| match attr {
            AttributeValue::BodyStructure(body) => Some(body),
            _ => None,
        })
        .unwrap();
    let files = attachment_files(body);
    assert_eq!(
        files,
        vec![
            crate::store::CardFile {
                filename: "report.pdf".into(),
                mime: "application/pdf".into()
            },
            crate::store::CardFile {
                filename: "notes.txt".into(),
                mime: "text/plain".into()
            },
        ]
    );
    assert!(fetch_items(false, false).contains("BODYSTRUCTURE"));
    assert!(fetch_items(true, false).contains("BODYSTRUCTURE"));
}

#[test]
fn bodystructure_filenames_decode_words_and_extended_parameters() {
    use std::borrow::Cow;
    let check = |pairs: &[(&str, &str)], name, expected| {
        let params = Some(
            pairs
                .iter()
                .map(|(key, value)| (Cow::Borrowed(*key), Cow::Borrowed(*value)))
                .collect(),
        );
        assert_eq!(
            super::body_parameter(&params, name).as_deref(),
            Some(expected)
        );
    };
    check(
        &[("FILENAME", "=?UTF-8?B?6KuL5rGC5pu4LnBkZg==?=")],
        "filename",
        "請求書.pdf",
    );
    check(
        &[("filename*", "UTF-8'ja'%E8%AB%8B%E6%B1%82%E6%9B%B8.pdf")],
        "filename",
        "請求書.pdf",
    );
    check(
        &[
            ("name*1*", "%E6%9B%B8.pdf"),
            ("name*0*", "UTF-8''%E8%AB%8B%E6%B1%82"),
        ],
        "name",
        "請求書.pdf",
    );
    check(
        &[("filename*", "iso-8859-1''caf%E9.pdf")],
        "filename",
        "café.pdf",
    );
    check(
        &[("filename", "quote\";literal.pdf")],
        "filename",
        "quote\";literal.pdf",
    );
}

#[test]
fn provisional_files_match_mime_reader_for_named_and_unnamed_parts() {
    use async_imap::imap_proto::{
        parser::parse_response,
        types::{AttributeValue, Response},
    };
    for (structure, mime_headers, file_count) in [
        (
            r#"("APPLICATION" "PDF" ("NAME*" "utf-8''caf%C3%A9.pdf") NIL NIL "BASE64" 4)"#,
            "Content-Type: application/pdf; name*=utf-8''caf%C3%A9.pdf\r\nContent-Transfer-Encoding: base64\r\n",
            1,
        ),
        (
            r#"("TEXT" "PLAIN" ("NAME" "notes.txt") NIL NIL "7BIT" 4 1)"#,
            "Content-Type: text/plain; name=notes.txt\r\n",
            0,
        ),
        (
            r#"("TEXT" "HTML" ("NAME" "body.html") NIL NIL "7BIT" 4 1 NIL ("INLINE" ("FILENAME" "body.html")))"#,
            "Content-Type: text/html; name=body.html\r\nContent-Disposition: inline; filename=body.html\r\n",
            0,
        ),
        (
            r#"("IMAGE" "PNG" NIL NIL NIL "BASE64" 4)"#,
            "Content-Type: image/png\r\nContent-Transfer-Encoding: base64\r\n",
            1,
        ),
        (
            r#"("TEXT" "CALENDAR" ("NAME" "invite.ics") NIL NIL "7BIT" 4 1 NIL ("INLINE" ("FILENAME" "invite.ics")))"#,
            "Content-Type: text/calendar; name=invite.ics\r\nContent-Disposition: inline; filename=invite.ics\r\n",
            0,
        ),
        (
            r#"("TEXT" "CALENDAR" ("NAME" "invite.ics") NIL NIL "7BIT" 4 1 NIL ("ATTACHMENT" ("FILENAME" "invite.ics")))"#,
            "Content-Type: text/calendar; name=invite.ics\r\nContent-Disposition: attachment; filename=invite.ics\r\n",
            1,
        ),
    ] {
        let response = format!("* 1 FETCH (BODYSTRUCTURE {structure})\r\n");
        let (_, Response::Fetch(_, attributes)) = parse_response(response.as_bytes()).unwrap()
        else {
            panic!("expected FETCH")
        };
        let body = attributes
            .iter()
            .find_map(|attr| match attr {
                AttributeValue::BodyStructure(body) => Some(body),
                _ => None,
            })
            .unwrap();
        let raw = format!("{mime_headers}\r\nYQ==");
        let message = crate::parse::parse_message(raw.as_bytes(), None);
        let expected: Vec<_> = message
            .file_attachments()
            .map(|file| crate::store::CardFile {
                filename: file.filename.clone(),
                mime: file.mime.clone(),
            })
            .collect();
        assert_eq!(expected.len(), file_count, "{structure}");
        assert_eq!(attachment_files(body), expected, "{structure}");
    }
}

#[test]
fn uid_set_chunks_compacts_runs_and_splits_long_sets() {
    assert_eq!(
        uid_set_chunks(&[5, 1, 2, 3, 9, 3, 11, 12], 100),
        vec!["1:3,5,9,11:12"]
    );
    assert!(uid_set_chunks(&[], 100).is_empty());
    let scattered = (1..=20_000).map(|n| n * 2).collect::<Vec<u32>>();
    let chunks = uid_set_chunks(&scattered, 7000);
    assert!(chunks.len() > 1);
    assert!(chunks.iter().all(|chunk| chunk.len() <= 7000));
    let rejoined = chunks
        .join(",")
        .split(',')
        .map(|uid| uid.parse::<u32>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rejoined, scattered);
}

#[test]
fn protocol_stage_timeout_leaves_room_for_a_slow_but_healthy_login() {
    // The default 30s sync budget yields a 24s connect budget; an even
    // share of that (8s) is under Gmail's observed 4-7s XOAUTH2 exchange.
    assert_eq!(
        protocol_timeout_for(std::time::Duration::from_secs(24)),
        MIN_PROTOCOL_TIMEOUT
    );
    // A raised MERON_SYNC_TIMEOUT still scales the share up past the floor.
    assert_eq!(
        protocol_timeout_for(std::time::Duration::from_secs(120)),
        std::time::Duration::from_secs(40)
    );
    // A budget below the floor caps the stage at the budget itself.
    assert_eq!(
        protocol_timeout_for(std::time::Duration::from_secs(4)),
        std::time::Duration::from_secs(4)
    );
}

#[test]
fn connect_addresses_alternate_families_without_overriding_resolver_preference() {
    let v4a = "192.0.2.1:993".parse().unwrap();
    let v4b = "192.0.2.2:993".parse().unwrap();
    let v6a = "[2001:db8::1]:993".parse().unwrap();
    let v6b = "[2001:db8::2]:993".parse().unwrap();

    assert_eq!(
        interleave_address_families(vec![v6a, v6b, v4a, v4b]),
        vec![v6a, v4a, v6b, v4b]
    );
    assert_eq!(
        interleave_address_families(vec![v4a, v4b, v6a, v6b]),
        vec![v4a, v6a, v4b, v6b]
    );
}

#[test]
fn a_draft_is_recognised_by_its_message_id_header_without_the_servers_search() {
    let header = b"Message-ID: <Meron-Draft-1@example.org>\r\n\r\n";
    assert!(header_is_message(header, "meron-draft-1@example.org"));
    assert!(header_is_message(header, " <meron-draft-1@example.org> "));
    assert!(!header_is_message(header, "meron-draft-1@example"));
    assert!(!header_is_message(header, ""));
    assert!(!header_is_message(
        b"Subject: hi\r\n\r\n",
        "meron-draft-1@example.org"
    ));
}

#[test]
fn looks_like_drafts_matches_common_names_case_insensitively() {
    for name in ["Drafts", "draft", "INBOX.Drafts", "[Gmail]/Drafts"] {
        assert!(looks_like_drafts(name), "{name}");
    }
    for name in ["INBOX", "Sent", "Draft Specs", ""] {
        assert!(!looks_like_drafts(name), "{name}");
    }
}

#[test]
fn header_fields_reads_references_root_from_folded_header() {
    // A reply-to-a-reply: References spans two folded lines, root first.
    let header = b"Subject: Re: test mailo 1\r\nReferences: <CAJ7M84+root@mail.gmail.com>\r\n\t<meron-second@mailo.com>\r\nIn-Reply-To: <meron-second@mailo.com>\r\n\r\n";
    let ef = header_fields(header);
    assert_eq!(ef.references_root, "CAJ7M84+root@mail.gmail.com");
    assert_eq!(ef.in_reply_to, "meron-second@mailo.com");
}

#[test]
fn header_fields_decodes_subject_and_tolerates_absence() {
    // Quotes arrive verbatim; the envelope handed them back still escaped.
    let ef = header_fields(b"Subject: A \"quoted\" title\r\nFrom: x@y\r\n\r\n");
    assert_eq!(ef.subject, "A \"quoted\" title");
    let ef = header_fields(b"Subject: =?UTF-8?B?SGVsbMO2?=\r\n\r\n");
    assert_eq!(ef.subject, "Hell\u{f6}");
    // No / empty Subject header, and no header at all.
    assert!(header_fields(b"From: x@y\r\n\r\n").subject.is_empty());
    assert!(header_fields(b"Subject:\r\n\r\n").subject.is_empty());
    assert!(header_fields(b"").subject.is_empty());
}

#[test]
fn header_fields_decodes_encoded_word_glued_to_from_name() {
    // "GyRCJUYlOSVIGyhC" is ISO-2022-JP "テスト".
    let ef =
        header_fields(b"From: Acme=?iso-2022-jp?B?GyRCJUYlOSVIGyhC?= <news@example.com>\r\n\r\n");
    assert_eq!(ef.from_name, "Acme\u{30c6}\u{30b9}\u{30c8}");
    assert_eq!(ef.from_addr, "news@example.com");

    // Every entry of a list, with non-ASCII before a glued name.
    let ef = header_fields(
        "From: x@example.com\r\nTo: \"Zo\u{eb}\" <z@example.com>, Acme=?UTF-8?Q?Foo?= <a@example.com>, Team: Bob=?UTF-8?Q?_Jr?= <b@example.com>;\r\n\r\n".as_bytes(),
    );
    assert_eq!(
        ef.to
            .iter()
            .map(|r| (r.name.as_str(), r.addr.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Zo\u{eb}", "z@example.com"),
            ("AcmeFoo", "a@example.com"),
            ("Bob Jr", "b@example.com")
        ]
    );
}

#[test]
fn header_fields_parses_addresses_including_8bit_and_groups() {
    // The raw 8-bit display name that made the ENVELOPE unparseable.
    let header = "From: \"M\u{fc}ller\" <support@example.com>\r\nTo: a@x.com, Bob <b@x.com>\r\nCc: Team: c@x.com, d@x.com;\r\nDate: Sun, 12 Jul 2026 17:01:36 +0000\r\nMessage-ID: <abc@x.com>\r\n\r\n".as_bytes();
    let ef = header_fields(header);
    assert_eq!(ef.from_name, "M\u{fc}ller");
    assert_eq!(ef.from_addr, "support@example.com");
    assert_eq!(ef.message_id, "abc@x.com");
    assert_ne!(ef.date, 0);
    assert_eq!(
        ef.to.iter().map(|r| r.addr.as_str()).collect::<Vec<_>>(),
        ["a@x.com", "b@x.com"]
    );
    assert_eq!(ef.to[1].name, "Bob");
    // A group list contributes its members, not the group name.
    assert_eq!(
        ef.cc.iter().map(|r| r.addr.as_str()).collect::<Vec<_>>(),
        ["c@x.com", "d@x.com"]
    );
}

/// Minimal IMAP server: greets, accepts any login, reports a one-message
/// INBOX, and answers FETCH with `fetch_reply`, in which `{tag}` stands for
/// the command tag. A `fetch_reply` with no `{tag}` is never completed: the
/// server hangs up after writing it, standing in for a dropped connection.
async fn serve_canned(listener: tokio::net::TcpListener, fetch_reply: &'static str) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let Ok((sock, _)) = listener.accept().await else {
        return;
    };
    let (reader, mut writer) = sock.into_split();
    if writer.write_all(b"* OK IMAP4rev1 ready\r\n").await.is_err() {
        return;
    }
    let mut lines = tokio::io::BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut parts = line.splitn(2, ' ');
        let tag = parts.next().unwrap_or_default().to_string();
        let cmd = parts
            .next()
            .unwrap_or_default()
            .split(' ')
            .next()
            .unwrap_or_default()
            .to_uppercase();
        let reply = match cmd.as_str() {
            "CAPABILITY" => {
                format!("* CAPABILITY IMAP4rev1\r\n{tag} OK CAPABILITY completed\r\n")
            }
            "SELECT" => format!(
                "* 1 EXISTS\r\n* OK [UIDVALIDITY 42] ok\r\n* OK [UIDNEXT 1692] ok\r\n\
                 {tag} OK [READ-WRITE] SELECT completed\r\n"
            ),
            "FETCH" if !fetch_reply.contains("{tag}") => {
                let _ = writer.write_all(fetch_reply.as_bytes()).await;
                return;
            }
            "FETCH" => fetch_reply.replace("{tag}", &tag),
            _ => format!("{tag} OK {cmd} completed\r\n"),
        };
        if writer.write_all(reply.as_bytes()).await.is_err() {
            return;
        }
    }
}

async fn fetch_recent_against(fetch_reply: &'static str) -> anyhow::Error {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(serve_canned(listener, fetch_reply));

    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut client = async_imap::Client::new(super::Stream::plain(tcp));
    client.read_response().await.unwrap().unwrap();
    let mut session = client.login("u", "p").await.map_err(|(e, _)| e).unwrap();
    match super::fetch_recent(&mut session, "INBOX", 50).await {
        Ok(batch) => panic!(
            "canned reply should not produce a batch ({} messages)",
            batch.messages.len()
        ),
        Err(err) => err,
    }
}

/// The marker `is_unparseable_response` looks for is a string emitted by our
/// pinned async-imap fork, so pin it against the real thing rather than
/// against a hand-built error.
#[tokio::test]
async fn unparseable_response_marker_matches_async_imap() {
    // A raw 8-bit byte inside an IMAP quoted string: imap-proto's `quoted`
    // only accepts 0x01..=0x7F, so it rejects the whole response. This is
    // the shape from the original report; any response the parser refuses
    // would do.
    let err = fetch_recent_against(
        "* 1 FETCH (UID 1691 FLAGS (\\Seen) ENVELOPE (\"Sun, 12 Jul 2026 17:01:36 +0000\" \
         \"Re: caf\u{e9}\" NIL NIL NIL NIL NIL NIL NIL \"<c@d>\"))\r\n\
         {tag} OK FETCH completed\r\n",
    )
    .await;
    assert!(
        super::is_unparseable_response(&err),
        "should be recognised as unparseable: {err:#}"
    );
}

/// A network failure must not be mistaken for a poison message —
/// re-fetching narrower ranges would just repeat the refusal.
#[tokio::test]
async fn a_dropped_connection_is_not_treated_as_unparseable() {
    // Server hangs up part-way through a perfectly well-formed response.
    let err = fetch_recent_against("* 1 FETCH (UID 1691 FLAGS (\\See").await;
    assert!(
        !super::is_unparseable_response(&err),
        "a dropped connection should not look unparseable: {err:#}"
    );
    let err = anyhow::anyhow!(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
        .context("FETCH item");
    assert!(!super::is_unparseable_response(&err));
}

#[test]
fn normalize_message_id_extracts_bare_id_preserving_case() {
    // Case must survive: the stored id is echoed into reply In-Reply-To/
    // References headers, and receivers (Gmail) match ids case-sensitively.
    assert_eq!(
        normalize_message_id("<nonbili/NouTube/issues/253@github.com>"),
        "nonbili/NouTube/issues/253@github.com"
    );
    assert_eq!(normalize_message_id("<ID@Host>"), "ID@Host");
    assert_eq!(normalize_message_id("prefix <Id@Host> suffix"), "Id@Host");
    assert_eq!(normalize_message_id("  bare@host  "), "bare@host");
    // Unterminated bracket falls back to trimming both bracket chars.
    assert_eq!(normalize_message_id("<broken@host"), "broken@host");
    assert_eq!(normalize_message_id(""), "");
}

#[test]
fn first_message_id_takes_the_references_root() {
    assert_eq!(
        first_message_id("<Root@h> <mid@h> <leaf@h>").as_deref(),
        Some("Root@h")
    );
    assert_eq!(first_message_id("   ").as_deref(), None);
    // RFC 822 phrase before the id: brackets win over whitespace, or the
    // id would come out as "Your" and thread unrelated mail together.
    assert_eq!(
        first_message_id("Your message of Tuesday <parent@example.com>").as_deref(),
        Some("parent@example.com")
    );
    // No brackets anywhere: fall back to the first token.
    assert_eq!(
        first_message_id("bare@host trailing-junk").as_deref(),
        Some("bare@host")
    );
}

#[test]
fn header_fields_reads_a_phrase_prefixed_in_reply_to() {
    let header = b"In-Reply-To: Your message of Tuesday <parent@example.com>\r\n\r\n";
    let ef = header_fields(header);
    assert_eq!(ef.in_reply_to, "parent@example.com");
    // Same obsolete syntax in References must not poison the thread root.
    let header = b"References: Your message of Tuesday <root@example.com>\r\n\r\n";
    assert_eq!(header_fields(header).references_root, "root@example.com");
}

#[test]
fn imap_quote_escapes_quotes_backslashes_and_newlines() {
    assert_eq!(imap_quote("plain"), "\"plain\"");
    assert_eq!(imap_quote(r#"a"b\c"#), r#""a\"b\\c""#);
    assert_eq!(imap_quote("a\r\nb"), "\"a  b\"");
}

#[test]
fn search_criteria_ors_a_terms_alternatives() {
    assert_eq!(
        search_criteria(false, "from:{a b c} to:d", false, false),
        "OR FROM \"a\" OR FROM \"b\" FROM \"c\" OR TO \"d\" CC \"d\""
    );
    assert_eq!(
        search_criteria(false, "to:a OR to:b", false, false),
        "OR OR TO \"a\" CC \"a\" OR TO \"b\" CC \"b\""
    );
}

#[test]
fn search_criteria_announces_utf8_only_when_asked() {
    assert_eq!(
        search_criteria(false, "plan", false, false),
        "OR OR SUBJECT \"plan\" FROM \"plan\" TEXT \"plan\""
    );
    assert_eq!(
        search_criteria(true, "plan", false, false),
        "X-GM-RAW \"plan\""
    );
    // Non-ASCII queries need the charset; the retry drops it again.
    assert_eq!(
        search_criteria(true, "会議", true, false),
        "CHARSET UTF-8 X-GM-RAW \"会議\""
    );
    assert_eq!(
        search_criteria(false, "会議", true, false),
        "CHARSET UTF-8 OR OR SUBJECT \"会議\" FROM \"会議\" TEXT \"会議\""
    );
}

#[test]
fn gmail_groups_queries_joined_on_separate_lines() {
    assert_eq!(gmail_raw("from:a budget"), "from:a budget");
    assert_eq!(
        gmail_raw("subject:\"Q3\nAND ANDROID paid"),
        "(subject:\"Q3\") (ANDROID paid)"
    );
    assert_eq!(gmail_raw("from:a\nAND "), "from:a\nAND ");
    assert_eq!(
        gmail_raw("from:{a \"b\nAND 27\" x"),
        "(from:{a \"b\"}) (27\" x\")"
    );
}

#[test]
fn search_criteria_compiles_operators_off_gmail() {
    assert_eq!(
        search_criteria(false, "from:a@b.c to:d@e.f plan", false, false),
        "OR OR SUBJECT \"plan\" FROM \"plan\" TEXT \"plan\" FROM \"a@b.c\" \
         OR TO \"d@e.f\" CC \"d@e.f\""
    );
    assert_eq!(
        search_criteria(
            false,
            r#"subject:"Q3 plan" is:unread is:starred after:2026/01/02 before:2026-02-03"#,
            false,
            false
        ),
        "SUBJECT \"Q3 plan\" UNSEEN FLAGGED SENTSINCE 02-Jan-2026 SENTBEFORE 03-Feb-2026"
    );
    assert_eq!(
        search_criteria(false, "from:a@b.c has:attachment", false, false),
        format!("FROM \"a@b.c\" {}", super::ATTACHMENT_CANDIDATES)
    );
    // Gmail gets the query untouched, operators and all.
    assert_eq!(
        search_criteria(true, "from:a@b.c has:attachment", false, false),
        "X-GM-RAW \"from:a@b.c has:attachment\""
    );
    assert_eq!(
        search_criteria(false, "from:安娜", true, true),
        "CHARSET UTF-8 FROM {6+}\r\n安娜"
    );
}

#[test]
fn search_criteria_sends_non_ascii_as_literal_plus() {
    // "会議" is 6 UTF-8 octets; the count is bytes, not chars.
    assert_eq!(
        search_criteria(true, " 会議 ", true, true),
        "CHARSET UTF-8 X-GM-RAW {6+}\r\n会議"
    );
    assert_eq!(
        search_criteria(false, "é\r\nx", true, true),
        "CHARSET UTF-8 OR OR SUBJECT {5+}\r\né  x FROM {5+}\r\né  x TEXT {5+}\r\né  x"
    );
}

#[test]
fn message_id_search_criteria_builds_nested_or() {
    assert_eq!(message_id_search_criteria(&[]), "ALL");
    assert_eq!(
        message_id_search_criteria(&["a@h".to_string()]),
        "HEADER MESSAGE-ID \"a@h\""
    );
    assert_eq!(
        message_id_search_criteria(&["a@h".to_string(), "b@h".to_string()]),
        "OR HEADER MESSAGE-ID \"a@h\" HEADER MESSAGE-ID \"b@h\""
    );
}

#[test]
fn civil_from_days_round_trips_known_dates() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(-1), (1969, 12, 31));
    // 2000-02-29 (leap day) is 11016 days after the epoch.
    assert_eq!(civil_from_days(11016), (2000, 2, 29));
    assert_eq!(civil_from_days(20614), (2026, 6, 10));
}

// Gmail threads by subject similarity, but an encrypted message's subject
// is the protected-headers placeholder, the same for all of them: every
// encrypted mail between two people lands in one Gmail conversation, even
// a new message. Those messages thread by their own headers instead.
#[test]
fn an_encrypted_placeholder_subject_ignores_gmails_thread_id() {
    for placeholder in ["[...]", "...", "Re: [...]"] {
        assert_eq!(
            thread_key(Some(42), true, placeholder, "msgid", "", "", 7),
            "msgid",
            "{placeholder}"
        );
        assert_eq!(
            thread_key(
                Some(42),
                true,
                placeholder,
                "msgid",
                "inreply",
                "refsroot",
                7
            ),
            "refsroot"
        );
    }
    assert_eq!(
        thread_key(Some(42), true, "Lunch", "msgid", "", "", 7),
        "gmthrid:42"
    );
}

// Only an encrypted message's `...` is a placeholder: a plain Gmail message
// that happens to be titled `...` or `Re: [...]` keeps Gmail's threading.
#[test]
fn a_plain_message_titled_like_a_placeholder_keeps_gmails_thread_id() {
    for subject in ["...", "[...]", "Re: [...]"] {
        assert_eq!(
            thread_key(Some(42), false, subject, "msgid", "inreply", "refsroot", 7),
            "gmthrid:42",
            "{subject}"
        );
    }
}

#[test]
fn the_content_type_says_whether_a_message_is_encrypted() {
    let encrypted = b"Subject: ...\r\nContent-Type: multipart/encrypted; protocol=\"application/pgp-encrypted\"; boundary=b\r\n\r\n";
    let plain = b"Subject: ...\r\nContent-Type: text/plain\r\n\r\n";
    assert!(header_fields(encrypted).encrypted);
    assert!(!header_fields(plain).encrypted);
}

#[test]
fn gmail_thrid_takes_precedence() {
    // Even with full RFC headers present, a Gmail thrid wins and is namespaced.
    let key = thread_key(Some(42), false, "Plans", "msgid", "inreply", "refsroot", 7);
    assert_eq!(key, "gmthrid:42");
}

#[test]
fn falls_back_to_rfc_headers_without_thrid() {
    assert_eq!(
        thread_key(None, false, "", "msgid", "inreply", "refsroot", 7),
        "refsroot"
    );
    assert_eq!(
        thread_key(None, false, "", "msgid", "inreply", "", 7),
        "inreply"
    );
    assert_eq!(thread_key(None, false, "", "msgid", "", "", 7), "msgid");
    assert_eq!(thread_key(None, false, "", "", "", "", 7), "uid:7");
}
