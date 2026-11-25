use super::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

async fn server(
    caps: &'static str,
) -> (
    Session,
    Arc<Mutex<Vec<String>>>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let recorded = commands.clone();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = socket.into_split();
        writer.write_all(b"* OK IMAP4rev1 ready\r\n").await.unwrap();
        let mut lines = tokio::io::BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let (tag, command) = line.split_once(' ').unwrap();
            recorded.lock().unwrap().push(command.to_string());
            let prefix = if command == "CAPABILITY" {
                format!("* CAPABILITY IMAP4rev1 {caps}\r\n")
            } else if command.starts_with("SELECT") {
                "* 2 EXISTS\r\n* OK [UIDVALIDITY 42] valid\r\n".into()
            } else {
                String::new()
            };
            if writer
                .write_all(format!("{prefix}{tag} OK done\r\n").as_bytes())
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut client = async_imap::Client::new(Stream::plain(tcp));
    client.read_response().await.unwrap().unwrap();
    let session = client
        .login("test", "test")
        .await
        .map_err(|(err, _)| err)
        .unwrap();
    (session, commands, task)
}
#[tokio::test]
async fn checked_delete_never_expunges_unselected_messages() {
    let (mut session, commands, task) = server("UIDPLUS").await;
    expunge_uids_checked(&mut session, "Trash", &[7, 9], 42)
        .await
        .unwrap();
    let calls = commands.lock().unwrap().clone();
    assert!(calls.iter().any(|c| c == "UID EXPUNGE 7,9"), "{calls:?}");
    assert!(
        calls.iter().any(|c| c.starts_with("UID STORE 7,9 ")),
        "{calls:?}"
    );
    assert!(!calls.iter().any(|c| c == "EXPUNGE"), "{calls:?}");
    task.abort();
}
#[tokio::test]
async fn changed_mailbox_or_missing_uidplus_fails_before_mutation() {
    for (caps, validity) in [("UIDPLUS", 43), ("", 42)] {
        let (mut session, commands, task) = server(caps).await;
        assert!(
            expunge_uids_checked(&mut session, "Trash", &[7], validity)
                .await
                .is_err()
        );
        let calls = commands.lock().unwrap().clone();
        assert!(
            !calls
                .iter()
                .any(|c| c.contains("STORE") || c.contains("EXPUNGE")),
            "{calls:?}"
        );
        task.abort();
    }
}
#[tokio::test]
async fn checked_move_uses_move_or_scoped_copy_expunge_only() {
    for caps in ["MOVE", "UIDPLUS", ""] {
        let (mut session, commands, task) = server(caps).await;
        let result = move_to_folder_checked(&mut session, "Drafts", "Trash", &[7]).await;
        let calls = commands.lock().unwrap().clone();
        assert_eq!(result.is_ok(), !caps.is_empty(), "{calls:?}");
        assert!(!calls.iter().any(|c| c == "EXPUNGE"), "{calls:?}");
        if caps == "MOVE" {
            assert!(
                calls.iter().any(|c| c.starts_with("UID MOVE 7 ")),
                "{calls:?}"
            );
            assert!(!calls.iter().any(|c| c.contains("STORE")), "{calls:?}");
        } else if caps == "UIDPLUS" {
            assert!(
                calls.iter().any(|c| c.starts_with("UID COPY 7 ")),
                "{calls:?}"
            );
            assert!(calls.iter().any(|c| c == "UID EXPUNGE 7"), "{calls:?}");
        } else {
            assert!(!calls.iter().any(|c| c.starts_with("UID ")), "{calls:?}");
        }
        task.abort();
    }
}
#[tokio::test]
async fn large_uid_sets_are_sent_in_bounded_commands() {
    for caps in ["MOVE", "UIDPLUS"] {
        let (mut session, commands, task) = server(caps).await;
        let uids = (1..=20_000).map(|n| n * 2).collect::<Vec<u32>>();
        move_to_folder(&mut session, "INBOX", "Archive", &uids)
            .await
            .unwrap();
        let calls = commands.lock().unwrap().clone();
        let uid_calls = calls
            .iter()
            .filter(|c| c.starts_with("UID "))
            .collect::<Vec<_>>();
        assert!(uid_calls.len() > 3, "{}", uid_calls.len());
        assert!(uid_calls.iter().all(|c| c.len() < 8192));
        let verb = if caps == "MOVE" {
            "UID MOVE "
        } else {
            "UID COPY "
        };
        let moved = uid_calls
            .iter()
            .filter_map(|c| c.strip_prefix(verb))
            .flat_map(|c| c.split_once(' ').unwrap().0.split(','))
            .count();
        assert_eq!(moved, uids.len());
        if caps == "UIDPLUS" {
            assert!(uid_calls.iter().any(|c| c.starts_with("UID EXPUNGE ")));
        }
        task.abort();
    }
}
