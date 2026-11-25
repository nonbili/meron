use super::*;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn a_repeated_move_without_imap_move_copies_only_what_is_missing() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = socket.into_split();
        let mut reader = tokio::io::BufReader::new(reader);
        writer.write_all(b"* OK ready\r\n").await.unwrap();
        let mut commands = Vec::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap() == 0 {
                return commands;
            }
            let (tag, command) = line.trim_end().split_once(' ').unwrap();
            let upper = command.to_uppercase();
            let untagged = if upper.starts_with("CAPABILITY") {
                "* CAPABILITY IMAP4rev1 UIDPLUS\r\n".to_string()
            } else if upper.starts_with("SELECT") {
                "* 2 EXISTS\r\n".to_string()
            } else if upper.starts_with("UID SEARCH") {
                let hit = if command.contains("copied@x") {
                    " 9"
                } else if command.contains("near@x") {
                    " 8"
                } else if command.contains("resent@x") {
                    " 7"
                } else {
                    ""
                };
                format!("* SEARCH{hit}\r\n")
            } else {
                String::new()
            };
            let untagged = if upper.starts_with("UID FETCH") {
                if upper.contains("BODY.PEEK") {
                    let (uid, id, size) = if command.contains(" 9 ") {
                        (9, "copied@x", 100)
                    } else if command.contains(" 7 ") {
                        (7, "resent@x", 999)
                    } else {
                        (8, "prefix-near@x", 100)
                    };
                    let header = format!("Message-ID: <{id}>\r\n\r\n");
                    format!(
                        "* 1 FETCH (UID {uid} RFC822.SIZE {size} BODY[HEADER] {{{}}}\r\n{header})\r\n",
                        header.len()
                    )
                } else {
                    // Every message the test names, except 6: it is gone.
                    (1..=5)
                        .map(|uid| format!("* {uid} FETCH (UID {uid} RFC822.SIZE 100)\r\n"))
                        .collect()
                }
            } else {
                untagged
            };
            let reply = format!("{untagged}{tag} OK done\r\n");
            writer.write_all(reply.as_bytes()).await.unwrap();
            commands.push(upper);
        }
    });
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut client = async_imap::Client::new(Stream::plain(tcp));
    client.read_response().await.unwrap().unwrap();
    let mut session = client
        .login("test", "test")
        .await
        .map_err(|(err, _)| err)
        .unwrap();

    // 2 is not there yet, 3 only resembles what is, and 4 was resent under
    // the same ID at another size: all three must really be copied.
    let ids = ["copied@x", "fresh@x", "near@x", "resent@x"].map(String::from);
    let all = move_to_folder_once(&mut session, "INBOX", "Work", &[1, 2, 3, 4], &ids)
        .await
        .unwrap();
    assert!(all.failed.is_none());
    assert_eq!(all.moved, [2, 3, 4, 1]);
    let unidentified = ["copied@x".to_string(), String::new()];
    let partial = move_to_folder_once(&mut session, "INBOX", "Work", &[1, 5], &unidentified)
        .await
        .unwrap();
    assert!(
        partial.failed.is_some(),
        "the one that can't be told apart stays behind, and the caller hears of it"
    );
    assert_eq!(partial.moved, [1]);
    let gone = ["gone@x".to_string()];
    let gone = move_to_folder_once(&mut session, "INBOX", "Work", &[6], &gone)
        .await
        .unwrap();
    assert_eq!(gone.moved, [6]);
    drop(session);

    let commands = server.await.unwrap();
    let sent =
        |verb: &str| -> Vec<&String> { commands.iter().filter(|c| c.starts_with(verb)).collect() };
    assert_eq!(sent("UID COPY").len(), 1);
    assert!(sent("UID COPY")[0].starts_with("UID COPY 2:4 "));
    let stored: Vec<_> = sent("UID STORE");
    assert!(stored.iter().any(|c| c.starts_with("UID STORE 1 ")));
    assert!(!stored.iter().any(|c| c.starts_with("UID STORE 5 ")));
    assert!(!stored.iter().any(|c| c.starts_with("UID STORE 6 ")));
}

#[tokio::test]
async fn copy_failure_reports_only_confirmed_appends_and_keeps_server_reason() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = socket.into_split();
        let mut reader = tokio::io::BufReader::new(reader);
        writer.write_all(b"* OK ready\r\n").await.unwrap();
        for command in 0..3 {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap();
            let response = match command {
                0 => format!("{tag} OK logged in\r\n"),
                1 => {
                    assert!(line.contains("APPEND"));
                    writer.write_all(b"+ ready\r\n").await.unwrap();
                    let mut literal = [0; 6];
                    reader.read_exact(&mut literal).await.unwrap();
                    assert_eq!(&literal, b"body\r\n");
                    format!("{tag} OK copied\r\n")
                }
                _ => {
                    assert!(line.contains("APPEND"));
                    format!(
                        "{tag} NO [LIMIT] Mail size is larger than the maximum size allowed by server configuration\r\n"
                    )
                }
            };
            writer.write_all(response.as_bytes()).await.unwrap();
        }
        let mut remainder = Vec::new();
        reader.read_to_end(&mut remainder).await.unwrap();
        assert!(remainder.is_empty(), "must stop after the rejected APPEND");
    });
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut client = async_imap::Client::new(Stream::plain(tcp));
    client.read_response().await.unwrap().unwrap();
    let mut session = client
        .login("test", "test")
        .await
        .map_err(|(err, _)| err)
        .unwrap();
    let messages = vec![
        RawMessageCopy {
            uid: 7,
            raw: b"body".to_vec(),
            seen: false,
            starred: false,
        };
        3
    ];
    let error = append_copied_messages(&mut session, "INBOX", &messages)
        .await
        .unwrap_err();
    let diagnostic = format!("{error:#}");
    assert!(
        diagnostic.contains("Copy progress: 1/3 confirmed; message size: 4 bytes"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("Mail size is larger"), "{diagnostic}");
    drop(session);
    task.await.unwrap();
}

async fn server(reply: &'static str) -> (Session, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = socket.into_split();
        let mut reader = tokio::io::BufReader::new(reader);
        writer.write_all(b"* OK ready\r\n").await.unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let tag = line.split_whitespace().next().unwrap();
        writer
            .write_all(format!("{tag} OK logged in\r\n").as_bytes())
            .await
            .unwrap();
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let tag = line.split_whitespace().next().unwrap().to_string();
        assert!(
            line.contains("APPEND \"a\\\"b\\\\c\" (\\Seen \\Flagged)"),
            "{line}"
        );
        let size: usize = line
            .split('{')
            .last()
            .unwrap()
            .trim()
            .trim_end_matches('}')
            .parse()
            .unwrap();
        writer
            .write_all(reply.replace("{tag}", &tag).as_bytes())
            .await
            .unwrap();
        if !reply.contains("+ ready") {
            // A rejected APPEND must not send the literal, and the session
            // must still be usable for the next command.
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            assert!(line.contains("NOOP"), "{line}");
            let tag = line.split_whitespace().next().unwrap();
            writer
                .write_all(format!("{tag} OK done\r\n").as_bytes())
                .await
                .unwrap();
            return Vec::new();
        }
        let mut literal = vec![0; size + 2];
        reader.read_exact(&mut literal).await.unwrap();
        writer
            .write_all(format!("* 3 EXISTS\r\n{tag} OK [APPENDUID 42 7] done\r\n").as_bytes())
            .await
            .unwrap();
        literal
    });
    let tcp = TcpStream::connect(addr).await.unwrap();
    let mut client = async_imap::Client::new(Stream::plain(tcp));
    client.read_response().await.unwrap().unwrap();
    let session = client
        .login("u", "p")
        .await
        .map_err(|(err, _)| err)
        .unwrap();
    (session, task)
}

#[tokio::test]
async fn append_waits_past_mailbox_updates_for_continuation() {
    for reply in [
        "+ ready\r\n",
        "* 2 EXISTS\r\n* 1 EXPUNGE\r\n* 1 FETCH (FLAGS (\\Seen))\r\n* OK update\r\n+ ready\r\n",
    ] {
        let (mut session, task) = server(reply).await;
        let raw = b"Subject: copy\r\n\r\nbody\x00\xff";
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            append_copied_message(
                &mut session,
                "a\"b\\c",
                &RawMessageCopy {
                    uid: 0,
                    raw: raw.to_vec(),
                    seen: true,
                    starred: true,
                },
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(task.await.unwrap(), [raw.as_slice(), b"\r\n"].concat());
    }
}

#[tokio::test]
async fn append_preserves_rejection_without_sending_literal() {
    for status in ["NO", "BAD"] {
        let reply = if status == "NO" {
            "* 2 EXISTS\r\n{tag} NO [TRYCREATE] missing mailbox\r\n"
        } else {
            "{tag} BAD invalid append\r\n"
        };
        let (mut session, task) = server(reply).await;
        let err = append_message(&mut session, "a\"b\\c", Some("(\\Seen \\Flagged)"), b"body")
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains(if status == "NO" {
                "missing mailbox"
            } else {
                "invalid append"
            }),
            "{err}"
        );
        session.noop().await.unwrap();
        assert!(task.await.unwrap().is_empty());
    }
}
