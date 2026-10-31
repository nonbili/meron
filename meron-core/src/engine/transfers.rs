//! Cross-account moves share the same safety rules on desktop and mobile.

use std::{collections::BTreeSet, sync::Arc};

use anyhow::{Context, ensure};
use serde_json::{Value, json};

use super::{Engine, delete_to_trash, refresh_written_folder};
use crate::{imap, mail_model, store};

pub fn require_complete_fetch(
    uids: &[u32],
    messages: &[imap::RawMessageCopy],
) -> anyhow::Result<()> {
    let requested: BTreeSet<_> = uids.iter().copied().collect();
    let fetched: BTreeSet<_> = messages.iter().map(|message| message.uid).collect();
    ensure!(
        requested == fetched && messages.len() == requested.len(),
        "Move failed: could not fetch every source message; source messages were kept"
    );
    Ok(())
}

/// Capture the source UIDs once, acknowledge every APPEND, then remove only
/// those UIDs. A failed or uncertain APPEND must never trigger source deletion.
pub async fn move_between_accounts(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    thread_id: &str,
    target_account: &str,
    target_folder: &str,
    uids: &[u32],
) -> anyhow::Result<Value> {
    ensure!(
        account != target_account,
        "Expected a different target account"
    );
    if uids.is_empty() {
        return Ok(json!({"ok": true, "moved": 0}));
    }
    let messages = engine
        .with_read_session(account, |session| {
            let folder = folder.to_owned();
            let uids = uids.to_vec();
            Box::pin(
                async move { imap::fetch_raw_messages_for_copy(session, &folder, &uids).await },
            )
        })
        .await?;
    require_complete_fetch(uids, &messages)?;
    let tags = {
        let copies = std::collections::BTreeMap::from([(folder.to_string(), uids.to_vec())]);
        store::carried_tags(&engine.db.lock().unwrap(), account, &copies)?
    };
    engine
        .with_write_session(target_account, |session| {
            let target = target_folder.to_owned();
            let messages = messages.clone();
            Box::pin(async move { imap::append_copied_messages(session, &target, &messages).await })
        })
        .await?;
    // Before the source goes: a failure here must not lose the tags.
    store::apply_carried_tags(&engine.db.lock().unwrap(), target_account, tags, true)?;

    let removed = delete_to_trash(engine, account, folder, uids)
        .await
        .context("Move copied messages but could not confirm removal from the source folder")?;
    {
        let db = engine.db.lock().unwrap();
        store::delete_messages_by_uid(&db, account, folder, uids)?;
    }
    // Refreshes are best effort after the confirmed writes. They must not turn
    // a successful move into a failure the user might retry.
    let mut targets = vec![(target_account, target_folder)];
    if let Some(trash) = &removed {
        targets.push((account, &trash.folder));
    }
    for (account, folder) in targets {
        if let Some(refresh) =
            refresh_written_folder(engine, account, folder, 50.max(uids.len() as u32)).await
        {
            let batch = &refresh.batch;
            let db = engine.db.lock().unwrap();
            let _ = store::ensure_folder(&db, account, folder)
                .and_then(|_| store::upsert_messages(&db, account, folder, &batch.messages))
                .and_then(|_| {
                    store::set_folder_state(&db, account, folder, batch.uidvalidity, batch.uid_next)
                });
        }
    }
    mail_model::mutation_result(
        json!({
            "ok": true, "moved": uids.len(), "source_folder": folder,
            "target_account": target_account, "target_folder": target_folder,
        }),
        &engine.db.lock().unwrap(),
        account,
        thread_id,
        folder,
        removed.as_ref().map(|trash| trash.folder.as_str()),
        None,
        None,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(uid: u32) -> imap::RawMessageCopy {
        imap::RawMessageCopy {
            uid,
            raw: vec![],
            seen: false,
            starred: false,
        }
    }

    #[test]
    fn cross_account_move_requires_every_requested_uid_exactly_once() {
        assert!(require_complete_fetch(&[7, 8], &[message(8), message(7)]).is_ok());
        for messages in [
            vec![],
            vec![message(7)],
            vec![message(7), message(9)],
            vec![message(7), message(7), message(8)],
        ] {
            assert!(require_complete_fetch(&[7, 8], &messages).is_err());
        }
    }

    // Real IMAP sessions against local servers exercise the boundary between
    // acknowledged APPENDs and source removal, without an external account.
    async fn cross_account_move_scenario(
        reject_append: bool,
        reject_removal: bool,
        incomplete_fetch: bool,
    ) {
        use std::sync::Mutex;
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
        struct Host;
        impl super::super::EngineHost for Host {
            fn open_db(&self) -> anyhow::Result<rusqlite::Connection> {
                store::open_at(":memory:")
            }
            fn apply_secret(&self, _: &rusqlite::Connection, _: &str, _: &mut imap::Creds) {}
            fn store_secret(
                &self,
                _: &rusqlite::Connection,
                _: &str,
                _: &crate::secrets::Secrets,
            ) -> anyhow::Result<()> {
                Ok(())
            }
        }
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut servers = Vec::new();
        let engine = Arc::new(Engine::new(Box::new(Host)).unwrap());
        for account in ["source", "target"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let calls = calls.clone();
            servers.push(tokio::spawn(async move {
                let mut connections = tokio::task::JoinSet::new();
                loop {
                    let (socket, _) = listener.accept().await.unwrap();
                    let calls = calls.clone();
                    connections.spawn(async move {
                        let (reader, mut writer) = socket.into_split();
                        let mut reader = tokio::io::BufReader::new(reader);
                        writer.write_all(b"* OK ready\r\n").await.unwrap();
                        loop {
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.unwrap_or(0) == 0 { break; }
                            let (tag, command) = line.trim_end().split_once(' ').unwrap();
                            calls.lock().unwrap().push(format!("{account}: {command}"));
                            let mut status = "OK done";
                            if command.starts_with("CAPABILITY") {
                                writer.write_all(b"* CAPABILITY IMAP4rev1 MOVE UIDPLUS\r\n").await.unwrap();
                            } else if command.starts_with("SELECT") {
                                writer.write_all(b"* 2 EXISTS\r\n* OK [UIDVALIDITY 1] valid\r\n* OK [UIDNEXT 10] next\r\n").await.unwrap();
                            } else if command.starts_with("UID FETCH") && command.contains("BODY.PEEK[]") {
                                let uids = if incomplete_fetch { vec![7] } else { vec![7, 8] };
                                for uid in uids {
                                    writer.write_all(format!("* 1 FETCH (UID {uid} FLAGS () BODY[] {{4}}\r\nbody)\r\n").as_bytes()).await.unwrap();
                                }
                            } else if command.starts_with("APPEND") {
                                if reject_append && calls.lock().unwrap().iter().filter(|call| call.starts_with("target: APPEND")).count() == 2 {
                                    status = "NO [OVERQUOTA] mailbox full";
                                } else {
                                    writer.write_all(b"+ ready\r\n").await.unwrap();
                                    let mut literal = [0; 6];
                                    reader.read_exact(&mut literal).await.unwrap();
                                    assert_eq!(&literal, b"body\r\n");
                                }
                            } else if command.starts_with("LIST") {
                                if reject_removal { status = "NO [NOPERM] cannot remove source"; }
                                else { writer.write_all(b"* LIST (\\Trash) \"/\" \"Trash\"\r\n").await.unwrap(); }
                            } else if command.starts_with("UID FETCH") {
                                // Cache refresh is best effort, and must not undo a move.
                                status = "NO cache refresh unavailable";
                            }
                            if writer.write_all(format!("{tag} {status}\r\n").as_bytes()).await.is_err() { break; }
                        }
                    });
                }
            }));
            let creds = {
                let db = engine.db.lock().unwrap();
                let config = json!({"host": "127.0.0.1", "port": port, "tls": false, "user": "u", "proxy": {"mode": "direct"}}).to_string();
                db.execute(
                    "INSERT INTO accounts(id, config) VALUES(?1, ?2)",
                    [account, &config],
                )
                .unwrap();
                store::ensure_folder(&db, account, "INBOX").unwrap();
                if account == "source" {
                    store::upsert_messages(
                        &db,
                        account,
                        "INBOX",
                        &[
                            imap::MessageHeader {
                                uid: 7,
                                ..Default::default()
                            },
                            imap::MessageHeader {
                                uid: 8,
                                ..Default::default()
                            },
                            imap::MessageHeader {
                                uid: 9,
                                ..Default::default()
                            },
                        ],
                    )
                    .unwrap();
                }
                store::load_account(&db, account).unwrap().unwrap()
            };
            engine.accounts.lock().await.insert(account.into(), creds);
        }
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            move_between_accounts(
                &engine,
                "source",
                "INBOX",
                "source#INBOX#7",
                "target",
                "INBOX",
                &[7, 8],
            ),
        )
        .await
        .expect("cross-account move must finish");
        let calls = calls.lock().unwrap();
        let removals: Vec<_> = calls
            .iter()
            .filter(|call| call.starts_with("source: UID MOVE"))
            .collect();
        if incomplete_fetch {
            assert!(
                format!("{:#}", result.unwrap_err())
                    .contains("could not fetch every source message")
            );
            assert!(!calls.iter().any(|call| call.starts_with("target: APPEND")));
        } else if reject_append {
            let error = format!("{:#}", result.unwrap_err());
            assert!(error.contains("Copy progress: 1/2 confirmed"), "{error}");
            assert!(removals.is_empty());
            assert!(!calls.iter().any(|call| call.starts_with("source: LIST")));
        } else if reject_removal {
            assert!(
                format!("{:#}", result.unwrap_err())
                    .contains("Move copied messages but could not confirm removal")
            );
        } else {
            assert_eq!(result.unwrap()["moved"], 2);
            assert_eq!(removals.len(), 1);
            assert!(
                removals[0].contains("7:8") || removals[0].contains("7,8"),
                "{removals:?}"
            );
            assert!(!removals[0].contains('9'));
        }
        let db = engine.db.lock().unwrap();
        let remaining: Vec<u32> = db.prepare("SELECT uid FROM messages WHERE account = 'source' AND folder = 'INBOX' ORDER BY uid").unwrap()
            .query_map([], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        if reject_append || reject_removal || incomplete_fetch {
            assert_eq!(remaining, vec![7, 8, 9]);
        } else {
            assert_eq!(remaining, vec![9]);
        }
        for server in servers {
            server.abort();
        }
    }

    #[tokio::test]
    async fn cross_account_move_keeps_sources_after_partial_append_failure() {
        cross_account_move_scenario(true, false, false).await;
    }

    #[tokio::test]
    async fn cross_account_move_reports_copied_but_not_removed() {
        cross_account_move_scenario(false, true, false).await;
    }

    #[tokio::test]
    async fn cross_account_move_keeps_sources_after_incomplete_fetch() {
        cross_account_move_scenario(false, false, true).await;
    }

    #[tokio::test]
    async fn cross_account_move_removes_only_the_captured_source_uids() {
        cross_account_move_scenario(false, false, false).await;
    }
}
