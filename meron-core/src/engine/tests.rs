use super::{
    Pooled, SENT_COPY_MATCH_WINDOW_SECS, SearchContinuation, TransferTimedOut, batch_placement,
    cached_archive_folder_from_folders, cached_search_mail_page, companion_folders,
    continue_search_page, find_role_folder, limit_prefetch_uids, parse_background_sync_timeout,
    pool_return, pool_take, prefetch::prefetch_failure_is_the_messages,
    record_search_folder_result, sent_copy_landed, should_append_sent_copy, store_folder_tail,
    thread_gap_search_folders,
};
use crate::{imap, parse};
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const MAX_IDLE: Duration = Duration::from_secs(120);
const MAX_POOLED: usize = 3;

#[test]
pub fn take_returns_none_for_unknown_account() {
    let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
    assert_eq!(pool_take(&mut map, "a", Instant::now(), MAX_IDLE), None);
}

#[test]
pub fn return_then_take_round_trips_lifo() {
    let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
    let now = Instant::now();
    pool_return(&mut map, "a", 1, now, MAX_POOLED);
    pool_return(&mut map, "a", 2, now, MAX_POOLED);
    // LIFO: the hottest (last returned) comes back first.
    assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), Some(2));
    assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), Some(1));
    assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), None);
}

#[test]
pub fn return_drops_session_when_at_capacity() {
    let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
    let now = Instant::now();
    for s in 0..(MAX_POOLED as u32 + 2) {
        pool_return(&mut map, "a", s, now, MAX_POOLED);
    }
    assert_eq!(map["a"].len(), MAX_POOLED);
}

#[test]
pub fn take_evicts_sessions_idle_past_max_idle() {
    let mut map: HashMap<String, Vec<Pooled<u32>>> = HashMap::new();
    let now = Instant::now();
    let stale = now.checked_sub(MAX_IDLE + Duration::from_secs(1)).unwrap();
    // One stale entry, then a fresh one on top.
    pool_return(&mut map, "a", 1, stale, MAX_POOLED);
    pool_return(&mut map, "a", 2, now, MAX_POOLED);
    // Fresh one is taken; the stale one underneath is evicted on the next take.
    assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), Some(2));
    assert_eq!(pool_take(&mut map, "a", now, MAX_IDLE), None);
}

fn role_folder(name: &str, special_use: Option<&str>) -> crate::imap::Folder {
    crate::imap::Folder {
        name: name.to_string(),
        special_use: special_use.map(str::to_string),
        ..Default::default()
    }
}

#[test]
pub fn find_role_folder_prefers_server_attribute_over_name() {
    // A localized drafts folder carries the \Drafts attribute; a folder
    // that merely *looks* like drafts by name must lose to it.
    let folders = vec![
        role_folder("Drafts", None),
        role_folder("Mail/Entwürfe", Some("drafts")),
    ];
    assert_eq!(
        find_role_folder(folders, "drafts", crate::imap::looks_like_drafts, "INBOX"),
        Some("Mail/Entwürfe".to_string())
    );
}

#[test]
pub fn find_role_folder_falls_back_to_name_heuristic() {
    // No special-use attributes recorded (server without RFC 6154, or rows
    // synced before the column existed): the name heuristic still works.
    let folders = vec![role_folder("INBOX", None), role_folder("Drafts", None)];
    assert_eq!(
        find_role_folder(folders, "drafts", crate::imap::looks_like_drafts, "INBOX"),
        Some("Drafts".to_string())
    );
}

#[test]
pub fn find_role_folder_never_returns_the_excluded_folder() {
    let folders = vec![role_folder("Drafts", Some("drafts"))];
    assert_eq!(
        find_role_folder(folders, "drafts", crate::imap::looks_like_drafts, "drafts"),
        None
    );
}

#[test]
pub fn cached_archive_folder_prefers_server_attribute_over_name() {
    let folders = vec![
        role_folder("Archive", None),
        role_folder("Mail/Archiv", Some("archive")),
    ];
    assert_eq!(
        cached_archive_folder_from_folders(folders, "INBOX"),
        Some("Mail/Archiv".to_string())
    );
}

#[test]
pub fn cached_archive_folder_resolves_localized_attribute_only_name() {
    let folders = vec![
        role_folder("INBOX", None),
        role_folder("Mail/Archiv", Some("archive")),
    ];
    assert_eq!(
        cached_archive_folder_from_folders(folders, "INBOX"),
        Some("Mail/Archiv".to_string())
    );
}

#[test]
pub fn cached_archive_folder_accepts_gmail_all_attribute() {
    // Gmail advertises All Mail as \All (recorded as "all"), and localized
    // accounts defeat the name heuristic — the attribute alone must win.
    let folders = vec![
        role_folder("INBOX", None),
        role_folder("[Gmail]/Alle Nachrichten", Some("all")),
    ];
    assert_eq!(
        cached_archive_folder_from_folders(folders, "INBOX"),
        Some("[Gmail]/Alle Nachrichten".to_string())
    );
}

#[test]
pub fn cached_archive_folder_falls_back_to_name_heuristic() {
    let folders = vec![role_folder("INBOX", None), role_folder("All Mail", None)];
    assert_eq!(
        cached_archive_folder_from_folders(folders, "INBOX"),
        Some("All Mail".to_string())
    );
}

#[test]
pub fn companion_folders_lists_sent_before_drafts() {
    assert_eq!(
        companion_folders(
            Some("[Gmail]/Sent Mail".to_string()),
            Some("[Gmail]/Drafts".to_string())
        ),
        vec![
            ("Sent", "[Gmail]/Sent Mail".to_string()),
            ("Drafts", "[Gmail]/Drafts".to_string()),
        ]
    );
}

#[test]
pub fn companion_folders_skips_drafts_matching_sent_case_insensitively() {
    // A misconfigured server can advertise both roles on one mailbox;
    // syncing it twice in the same tail would be wasted I/O.
    assert_eq!(
        companion_folders(Some("Sent".to_string()), Some("sent".to_string())),
        vec![("Sent", "Sent".to_string())]
    );
}

#[test]
pub fn companion_folders_empty_when_neither_resolves() {
    assert_eq!(companion_folders(None, None), Vec::new());
}

#[test]
pub fn background_sync_timeout_rejects_unsupported_values() {
    assert_eq!(
        parse_background_sync_timeout(Some("300")),
        Duration::from_secs(300)
    );
    for value in [None, Some("0"), Some("invalid"), Some("86401")] {
        assert_eq!(
            parse_background_sync_timeout(value),
            Duration::from_secs(30)
        );
    }
    assert_eq!(
        parse_background_sync_timeout(Some("18446744073709551615")),
        Duration::from_secs(30)
    );
}

#[test]
pub fn thread_gap_search_folders_include_drafts_once_before_archive() {
    let folders = thread_gap_search_folders(
        Some("Sent".to_string()),
        Some("Drafts".to_string()),
        Some("[Gmail]/All Mail".to_string()),
    );
    assert_eq!(folders, vec!["INBOX", "Sent", "Drafts", "[Gmail]/All Mail"]);
}

#[test]
pub fn thread_gap_search_folders_dedup_case_insensitively() {
    let folders = thread_gap_search_folders(
        Some("inbox".to_string()),
        Some("Drafts".to_string()),
        Some("drafts".to_string()),
    );
    assert_eq!(folders, vec!["INBOX", "Drafts"]);
}

#[test]
pub fn prefetch_blames_a_message_only_for_failures_of_its_own() {
    // Went silent or outlasted its allowance, or refused by the server:
    // potentially the message's.
    assert!(prefetch_failure_is_the_messages(
        &TransferTimedOut(Duration::from_secs(120)).into()
    ));
    assert!(prefetch_failure_is_the_messages(&anyhow::anyhow!(
        "UID FETCH: no such message"
    )));
    // The connection went away: says nothing about the message.
    assert!(!prefetch_failure_is_the_messages(&anyhow::Error::new(
        async_imap::error::Error::ConnectionLost
    )));
}

#[test]
pub fn prefetch_limit_keeps_all_uids_when_uncapped() {
    // Uncapped runs still warm newest mail first; nothing is dropped.
    assert_eq!(
        limit_prefetch_uids(vec![1, 2, 3, 4], None),
        vec![4, 3, 2, 1]
    );
}

#[test]
pub fn prefetch_limit_spends_mobile_budget_on_newest_uids() {
    assert_eq!(limit_prefetch_uids(vec![1, 2, 3, 4], Some(2)), vec![4, 3]);
}

#[test]
pub fn prefetch_limit_zero_fetches_nothing() {
    assert_eq!(
        limit_prefetch_uids(vec![1, 2, 3, 4], Some(0)),
        Vec::<u32>::new()
    );
}

fn sent_envelope(message_id: &str) -> parse::SentEnvelope {
    parse::SentEnvelope {
        message_id: message_id.to_string(),
        subject: "Re: lunch".to_string(),
        from_addr: "me@example.com".to_string(),
        recipients: vec!["a@example.com".to_string(), "b@example.com".to_string()],
        date: 1_700_000_000,
    }
}

fn sent_candidate(message_id: &str) -> imap::MessageHeader {
    imap::MessageHeader {
        uid: 7,
        subject: "Re: lunch".to_string(),
        from_addr: "me@example.com".to_string(),
        date: 1_700_000_000,
        message_id: message_id.to_string(),
        to: vec![imap::Recipient {
            name: "B".to_string(),
            addr: "B@Example.com".to_string(),
        }],
        cc: vec![imap::Recipient {
            name: String::new(),
            addr: "a@example.com".to_string(),
        }],
        ..Default::default()
    }
}

#[test]
fn a_sent_copy_lands_by_id_or_by_its_envelope() {
    // The id we sent is the direct answer when it comes back.
    assert!(sent_copy_landed(
        &sent_envelope("reply-1@meron"),
        &sent_candidate("reply-1@meron")
    ));
    // Proton Bridge replaces it, so the envelope has to answer instead —
    // recipients compare as a set, case-insensitively, across To and Cc.
    assert!(sent_copy_landed(
        &sent_envelope("reply-1@meron"),
        &sent_candidate("abc@protonmail.internalid")
    ));

    // Another message the folder gained in the same seconds is not this
    // send's copy: a different subject, recipients or sender rules it out.
    let mut other = sent_candidate("other@meron");
    other.subject = "Dinner?".to_string();
    assert!(!sent_copy_landed(&sent_envelope("reply-1@meron"), &other));
    let mut other = sent_candidate("other@meron");
    other.cc.clear();
    assert!(!sent_copy_landed(&sent_envelope("reply-1@meron"), &other));
    let mut other = sent_candidate("other@meron");
    other.from_addr = "colleague@example.com".to_string();
    assert!(!sent_copy_landed(&sent_envelope("reply-1@meron"), &other));

    // An identical envelope from far enough back is an earlier send of the
    // same message, not this one; modest skew still matches.
    let mut earlier = sent_candidate("earlier@meron");
    earlier.date -= SENT_COPY_MATCH_WINDOW_SECS + 1;
    assert!(!sent_copy_landed(&sent_envelope("reply-1@meron"), &earlier));
    let mut skewed = sent_candidate("skewed@meron");
    skewed.date += SENT_COPY_MATCH_WINDOW_SECS - 1;
    assert!(sent_copy_landed(&sent_envelope("reply-1@meron"), &skewed));

    // Unreadable dates must not make everything match everything.
    let mut undated = sent_candidate("undated@meron");
    undated.date = 0;
    assert!(!sent_copy_landed(&sent_envelope("reply-1@meron"), &undated));
    let mut undated_send = sent_envelope("reply-1@meron");
    undated_send.date = 0;
    assert!(!sent_copy_landed(
        &undated_send,
        &sent_candidate("srv@meron")
    ));

    // A blank id on our side never matches a blank id on the server's.
    let mut blank = sent_envelope("");
    blank.subject = "Other".to_string();
    assert!(!sent_copy_landed(&blank, &sent_candidate("")));
}

#[test]
pub fn sent_copy_policy_uses_provider_defaults_and_overrides() {
    assert!(!should_append_sent_copy("gmail_oauth", "", None));
    assert!(!should_append_sent_copy("outlook_oauth", "", None));
    assert!(!should_append_sent_copy("password", "smtp.gmail.com", None));
    assert!(!should_append_sent_copy(
        "custom",
        "smtp.office365.com",
        None
    ));
    assert!(should_append_sent_copy(
        "password",
        "smtp.example.com",
        None
    ));
    assert!(should_append_sent_copy("custom", "", None));

    assert!(should_append_sent_copy("gmail_oauth", "", Some(true)));
    assert!(should_append_sent_copy(
        "password",
        "smtp.gmail.com",
        Some(true)
    ));
    assert!(!should_append_sent_copy(
        "password",
        "smtp.example.com",
        Some(false)
    ));
}

#[test]
fn a_folder_tail_read_resets_the_cache_when_uidvalidity_changes() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO messages(account, folder, msg_id, uid, subject, date)
         VALUES('acct', 'Sent', 'old-1', 4, 'old generation', 100)",
        [],
    )
    .unwrap();
    crate::store::set_folder_state(&conn, "acct", "Sent", 111, 5).unwrap();
    crate::store::set_folder_modseq(&conn, "acct", "Sent", 42).unwrap();

    // Same generation: the tail read adds to what is cached.
    let batch = imap::RecentBatch {
        uidvalidity: 111,
        uid_next: 6,
        messages: vec![imap::MessageHeader {
            uid: 5,
            message_id: "new-1".to_string(),
            ..Default::default()
        }],
    };
    store_folder_tail(&conn, "acct", "Sent", &batch).unwrap();
    assert_eq!(cached_uids(&conn), vec![4, 5]);
    assert_eq!(
        crate::store::get_folder_modseq(&conn, "acct", "Sent").unwrap(),
        42
    );

    // A new generation: the old UIDs and the modseq that indexed them go,
    // or the next ordinary sync would see a validity matching its own and
    // skip the reset for good.
    let batch = imap::RecentBatch {
        uidvalidity: 222,
        uid_next: 3,
        messages: vec![imap::MessageHeader {
            uid: 2,
            message_id: "fresh-1".to_string(),
            ..Default::default()
        }],
    };
    store_folder_tail(&conn, "acct", "Sent", &batch).unwrap();
    assert_eq!(cached_uids(&conn), vec![2]);
    assert_eq!(
        crate::store::get_folder_state(&conn, "acct", "Sent").unwrap(),
        Some((222, 3))
    );
    assert_eq!(
        crate::store::get_folder_modseq(&conn, "acct", "Sent").unwrap(),
        0
    );
}

fn cached_uids(conn: &Connection) -> Vec<u32> {
    let mut stmt = conn
        .prepare("SELECT uid FROM messages WHERE account = 'acct' AND folder = 'Sent' ORDER BY uid")
        .unwrap();
    stmt.query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(|uid| uid.unwrap() as u32)
        .collect()
}

#[test]
fn cached_search_page_advances_the_engine_cursor() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    for (uid, date) in [(1u32, 100i64), (2, 200)] {
        conn.execute(
            "INSERT INTO messages(
               account, folder, msg_id, uid, subject, from_name, from_addr, date
             ) VALUES('acct', 'INBOX', ?1, ?2, 'deploy notes', 'Ops',
                      'ops@example.com', ?3)",
            params![uid.to_string(), uid, date],
        )
        .unwrap();
    }
    let folders = ["INBOX".to_string()];
    let first = cached_search_mail_page(&conn, "acct", &folders, "deploy", 1, None).unwrap();
    assert_eq!(first.messages[0].uid, 2);
    let cursor = crate::thread_list::parse_search_cursor(
        first.next_cursor.as_deref().expect("full page has cursor"),
    )
    .unwrap();

    let second =
        cached_search_mail_page(&conn, "acct", &folders, "deploy", 1, Some(&cursor)).unwrap();
    assert_eq!(second.messages[0].uid, 1);
}

fn insert_deploy_hits(conn: &Connection, uids: std::ops::RangeInclusive<u32>) {
    for uid in uids {
        conn.execute(
            "INSERT INTO messages(account, folder, msg_id, uid, subject, from_name, from_addr, date)
             VALUES('acct', 'INBOX', ?1, ?2, 'deploy notes', 'Ops', 'ops@example.com', ?3)",
            params![uid.to_string(), uid, uid as i64],
        )
        .unwrap();
    }
}

fn snapshot_cursor(token: &str, offset: u32) -> crate::thread_list::SearchCursor {
    crate::thread_list::SearchCursor {
        date: 0,
        uid: 0,
        folder: String::new(),
        scanned: 0,
        snapshot: Some(token.to_string()),
        offset,
    }
}

fn expect_page(continuation: SearchContinuation) -> super::SearchMailPage {
    match continuation {
        SearchContinuation::Page(page) => page,
        SearchContinuation::NeedsServer(_) => panic!("expected a local page"),
    }
}

#[test]
fn capped_search_snapshot_resumes_the_cache_where_its_scan_stopped() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    // Cached hits 2..=5, plus uid 1: an old server hit, also in the cache
    // since fetched hits are cached. The batch took cached hits 5 and 4, hit
    // its cap, and appended the server hit after them.
    insert_deploy_hits(&conn, 1..=5);
    let folders = ["INBOX".to_string()];
    let snapshot_rows =
        crate::store::search_messages_in_folders(&conn, "acct", &folders, "deploy", 10, None)
            .unwrap()
            .into_iter()
            .filter(|hit| [5, 4, 1].contains(&hit.uid))
            .collect::<Vec<_>>();
    let resume = crate::thread_list::format_search_cursor(&crate::thread_list::SearchCursor {
        date: 4,
        uid: 4,
        folder: "INBOX".to_string(),
        scanned: 0,
        snapshot: None,
        offset: 0,
    });
    let token =
        crate::store::create_search_snapshot(&conn, "acct", "deploy", &folders, &[]).unwrap();
    crate::store::finish_search_batch(&conn, &token, "acct", &[], &snapshot_rows, Some(&resume))
        .unwrap();

    let first = expect_page(
        continue_search_page(
            &conn,
            "acct",
            &folders,
            "deploy",
            10,
            &snapshot_cursor(&token, 0),
        )
        .unwrap(),
    );
    assert_eq!(
        first.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![5, 4, 1]
    );
    let tail =
        crate::thread_list::parse_search_cursor(first.next_cursor.as_deref().unwrap()).unwrap();

    // Paging past the snapshot resumes after cached hit 4, not after the
    // server hit, so 3 and 2 are not skipped; uid 1 is not listed twice.
    let rest =
        expect_page(continue_search_page(&conn, "acct", &folders, "deploy", 10, &tail).unwrap());
    assert_eq!(
        rest.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![3, 2]
    );
    assert!(rest.next_cursor.is_none());
}

#[test]
fn cache_tail_pages_keep_going_when_the_filter_thins_a_page() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    insert_deploy_hits(&conn, 1..=5);
    let folders = ["INBOX".to_string()];
    let listed =
        crate::store::search_messages_in_folders(&conn, "acct", &folders, "deploy", 10, None)
            .unwrap()
            .into_iter()
            .filter(|hit| [5, 3, 2].contains(&hit.uid))
            .collect::<Vec<_>>();
    let resume = crate::thread_list::format_search_cursor(&crate::thread_list::SearchCursor {
        date: 5,
        uid: 5,
        folder: "INBOX".to_string(),
        scanned: 0,
        snapshot: None,
        offset: 0,
    });
    let token =
        crate::store::create_search_snapshot(&conn, "acct", "deploy", &folders, &[]).unwrap();
    crate::store::finish_search_batch(&conn, &token, "acct", &[], &listed, Some(&resume)).unwrap();
    let first = expect_page(
        continue_search_page(
            &conn,
            "acct",
            &folders,
            "deploy",
            3,
            &snapshot_cursor(&token, 0),
        )
        .unwrap(),
    );
    let tail =
        crate::thread_list::parse_search_cursor(first.next_cursor.as_deref().unwrap()).unwrap();

    // The cache page after 5 is [4, 3, 2]; 3 and 2 are already listed, but the
    // page was full, so paging continues to 1.
    let thinned =
        expect_page(continue_search_page(&conn, "acct", &folders, "deploy", 3, &tail).unwrap());
    assert_eq!(
        thinned.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![4]
    );
    let next =
        crate::thread_list::parse_search_cursor(thinned.next_cursor.as_deref().unwrap()).unwrap();
    let last =
        expect_page(continue_search_page(&conn, "acct", &folders, "deploy", 3, &next).unwrap());
    assert_eq!(
        last.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![1]
    );
    assert!(last.next_cursor.is_none());
}

#[test]
fn search_page_reaching_unfetched_server_hits_needs_the_server() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    // The server matched UIDs 1..=10, all already cached; only the newest two
    // have been fetched into the snapshot.
    insert_deploy_hits(&conn, 1..=10);
    let folders = ["INBOX".to_string()];
    let pending = (1..=10)
        .map(|uid| crate::store::PendingSearchHit {
            folder: "INBOX".to_string(),
            uid,
            date: uid as i64,
        })
        .collect::<Vec<_>>();
    let token =
        crate::store::create_search_snapshot(&conn, "acct", "deploy", &folders, &pending).unwrap();
    let fetched =
        crate::store::search_messages_in_folders(&conn, "acct", &folders, "deploy", 2, None)
            .unwrap();
    crate::store::finish_search_batch(
        &conn,
        &token,
        "acct",
        &[("INBOX".to_string(), 10), ("INBOX".to_string(), 9)],
        &fetched,
        None,
    )
    .unwrap();

    // A full page inside the fetched part is served locally, with a cursor on.
    let first = expect_page(
        continue_search_page(
            &conn,
            "acct",
            &folders,
            "deploy",
            1,
            &snapshot_cursor(&token, 0),
        )
        .unwrap(),
    );
    assert_eq!(first.messages[0].uid, 10);
    assert!(first.next_cursor.is_some());

    // A page that runs past it asks for the next server batch, carrying the
    // local part as the offline answer, flagged incomplete. That answer goes
    // on through the cached matches below the snapshot instead of ending it.
    let partial = match continue_search_page(
        &conn,
        "acct",
        &folders,
        "deploy",
        5,
        &snapshot_cursor(&token, 0),
    )
    .unwrap()
    {
        SearchContinuation::NeedsServer(partial) => partial,
        SearchContinuation::Page(_) => panic!("unfetched server hits must not be skipped"),
    };
    assert_eq!(
        partial.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![10, 9]
    );
    assert!(partial.incomplete);
    let mut cursor =
        crate::thread_list::parse_search_cursor(partial.next_cursor.as_deref().unwrap()).unwrap();
    let mut rest = Vec::new();
    loop {
        let page = expect_page(
            continue_search_page(&conn, "acct", &folders, "deploy", 5, &cursor).unwrap(),
        );
        assert!(page.incomplete, "the tail passes over pending server hits");
        rest.extend(page.messages.iter().map(|m| m.uid));
        let Some(next) = page.next_cursor else {
            break;
        };
        cursor = crate::thread_list::parse_search_cursor(&next).unwrap();
    }
    assert_eq!(rest, (1..=8).rev().collect::<Vec<_>>());

    let (batch, next) = crate::store::take_search_pending(&conn, &token, 3).unwrap();
    assert_eq!(
        batch.iter().map(|hit| hit.uid).collect::<Vec<_>>(),
        vec![8, 7, 6]
    );
    assert_eq!(next.map(|hit| hit.uid), Some(5));
}

#[test]
fn search_page_reaching_the_snapshot_end_offline_starts_on_the_cache() {
    let conn = Connection::open_in_memory().unwrap();
    crate::store::run_migrations(&conn).unwrap();
    insert_deploy_hits(&conn, 1..=3);
    let folders = ["INBOX".to_string()];
    let pending = (1..=3)
        .map(|uid| crate::store::PendingSearchHit {
            folder: "INBOX".to_string(),
            uid,
            date: uid as i64,
        })
        .collect::<Vec<_>>();
    let token =
        crate::store::create_search_snapshot(&conn, "acct", "deploy", &folders, &pending).unwrap();

    // Nothing placed yet: the offline answer is the cache itself.
    match continue_search_page(
        &conn,
        "acct",
        &folders,
        "deploy",
        10,
        &snapshot_cursor(&token, 0),
    )
    .unwrap()
    {
        SearchContinuation::NeedsServer(partial) => {
            assert_eq!(
                partial.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
                vec![3, 2, 1]
            );
            assert!(partial.incomplete);
            assert!(partial.next_cursor.is_none());
        }
        SearchContinuation::Page(_) => panic!("unfetched server hits must not be skipped"),
    }
}

#[test]
fn search_batches_place_hits_by_date_not_uid() {
    let hit = |folder: &str, uid: u32, date: i64| crate::store::PendingSearchHit {
        folder: folder.to_string(),
        uid,
        date,
    };
    // UIDs 501..=2 are dated 501..=2, but UID 1 was imported late and is the
    // newest match. Batches are taken by date, so it leads the first one.
    let pending = std::iter::once(hit("INBOX", 1, 1000))
        .chain((2..=501).rev().map(|uid| hit("INBOX", uid, uid as i64)))
        .collect::<Vec<_>>();
    let (batch, next) = pending.split_at(3);
    let fetched = std::collections::HashSet::from(["INBOX"]);
    let (frontier, done) = batch_placement(batch, next.first(), &fetched);
    assert_eq!(frontier, Some((499, 499, "INBOX")));
    assert_eq!(
        done.iter().map(|(_, uid)| *uid).collect::<Vec<_>>(),
        vec![1, 501, 500]
    );

    // A folder whose fetch failed keeps its hits pending, and nothing older
    // than its newest is placed: Sent's 800 is, its 700 waits behind 750.
    let batch = [
        hit("Sent", 9, 800),
        hit("Archive", 4, 750),
        hit("Sent", 8, 700),
    ];
    let next = hit("Sent", 7, 600);
    let fetched = std::collections::HashSet::from(["Sent"]);
    let (frontier, done) = batch_placement(&batch, Some(&next), &fetched);
    assert_eq!(frontier, Some((750, 4, "Archive")));
    assert_eq!(done, vec![("Sent".to_string(), 9)]);

    // When the failed folder holds the newest hit, nothing fetched can be
    // placed, and the batch must report no progress rather than loop.
    let archive_only = std::collections::HashSet::from(["Archive"]);
    let (frontier, done) = batch_placement(&batch, Some(&next), &archive_only);
    assert_eq!(frontier, Some((800, 9, "Sent")));
    assert!(done.is_empty());

    // Nothing left pending: every fetched hit is placed.
    let (frontier, done) = batch_placement(&batch[..1], None, &fetched);
    assert_eq!(frontier, None);
    assert_eq!(done.len(), 1);
}

#[test]
fn folder_search_failure_keeps_other_folder_successes() {
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    record_search_folder_result(
        "INBOX",
        Ok(vec![crate::imap::MessageHeader {
            uid: 7,
            ..Default::default()
        }]),
        &mut successes,
        &mut failures,
    );
    record_search_folder_result(
        "Sent",
        Err(anyhow::anyhow!("SELECT failed")),
        &mut successes,
        &mut failures,
    );

    assert_eq!(successes.len(), 1);
    assert_eq!(successes[0].0, "INBOX");
    assert_eq!(successes[0].1[0].uid, 7);
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].0, "Sent");
}

#[test]
fn moved_copy_uids_picks_only_the_copies_a_move_created() {
    use super::sync::moved_copy_uids;
    let header = |uid: u32, message_id: &str| imap::MessageHeader {
        uid,
        message_id: message_id.to_string(),
        ..Default::default()
    };
    let id = |value: &str| value.to_string();

    // Arrivals past the pre-move UIDNEXT: the two moved copies, plus mail
    // delivered meanwhile — with another Message-ID, and with none at all.
    let arrivals = [
        header(9, "<Moved@Example.com>"),
        header(10, "<other@example.com>"),
        header(11, ""),
        header(12, "<second@example.com>"),
    ];
    let moved = [id("<moved@example.com>"), id("<second@example.com>")];
    assert_eq!(moved_copy_uids(&arrivals, &moved), vec![9, 12]);

    // A moved copy that isn't among the arrivals: incomplete, so nothing.
    let moved = [id("<moved@example.com>"), id("<missing@example.com>")];
    assert!(moved_copy_uids(&arrivals, &moved).is_empty());

    // An id-less move is picked out only when the id-less arrivals match it
    // one for one; a second one arriving meanwhile makes it ambiguous.
    assert_eq!(moved_copy_uids(&arrivals, &[id("")]), vec![11]);
    let crowded = [header(11, ""), header(13, "")];
    assert!(moved_copy_uids(&crowded, &[id("")]).is_empty());

    // Duplicate Message-IDs count per copy.
    let twins = [
        header(20, "<twin@example.com>"),
        header(21, "<twin@example.com>"),
    ];
    let moved = [id("<twin@example.com>"), id("<twin@example.com>")];
    assert_eq!(moved_copy_uids(&twins, &moved), vec![20, 21]);
    assert!(moved_copy_uids(&twins, &moved[..1]).is_empty());
}

#[tokio::test]
async fn transfer_deadline_bounds_credential_wait_without_cancelling_refresh() {
    use std::io::{Read, Write};
    use std::sync::Arc;

    struct RefreshHost(Arc<tokio::sync::Notify>);
    impl super::EngineHost for RefreshHost {
        fn open_db(&self) -> anyhow::Result<Connection> {
            crate::store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &Connection, _: &str, _: &mut imap::Creds) {}
        fn store_secret(
            &self,
            _: &Connection,
            _: &str,
            secret: &crate::secrets::Secrets,
        ) -> anyhow::Result<()> {
            assert_eq!(secret.access_token.as_deref(), Some("refreshed"));
            self.0.notify_one();
            Ok(())
        }
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
        }
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let body = r#"{"access_token":"refreshed","expires_in":3600}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let persisted = Arc::new(tokio::sync::Notify::new());
    let engine = Arc::new(super::Engine::new(Box::new(RefreshHost(persisted.clone()))).unwrap());
    let mut creds = {
        let db = engine.db.lock().unwrap();
        db.execute(
            "INSERT INTO accounts(id, config) VALUES('acc', ?1)",
            [r#"{"host":"localhost","user":"u","proxy":{"mode":"direct"}}"#],
        )
        .unwrap();
        crate::store::load_account(&db, "acc").unwrap().unwrap()
    };
    creds.auth_type = "gmail_oauth".into();
    creds.refresh_token = Some("refresh".into());
    creds.oauth_client_id = "client".into();
    creds.oauth_token_url = format!("http://{address}/token");
    engine.accounts.lock().await.insert("acc".into(), creds);
    let waiter = tokio::spawn({
        let engine = engine.clone();
        async move {
            engine
                .with_transfer_session("acc", Duration::from_millis(200), |_| {
                    Box::pin(async { Ok(()) })
                })
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(5), started_rx)
        .await
        .unwrap()
        .unwrap();
    let err = tokio::time::timeout(Duration::from_secs(1), waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(err.to_string().contains("session preparation timed out"));
    assert!(!err.is::<TransferTimedOut>());
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), persisted.notified())
        .await
        .unwrap();
    assert_eq!(
        engine.accounts.lock().await["acc"].access_token.as_deref(),
        Some("refreshed")
    );
    assert!(
        crate::store::load_accounts(&engine.db.lock().unwrap()).unwrap()[0]
            .1
            .token_expires_at
            > 0
    );
    server.join().unwrap();
}

#[tokio::test]
async fn pooled_transfer_server_no_is_retried_once_fresh() {
    pooled_transfer_recovery(false, false, false).await;
}

#[tokio::test]
async fn stale_pooled_transfer_reuses_a_session_returned_during_coordination() {
    pooled_transfer_recovery(true, false, false).await;
}

#[tokio::test]
async fn pooled_transfer_command_is_not_run_a_third_time() {
    pooled_transfer_recovery(true, true, false).await;
}

#[tokio::test]
async fn read_with_a_second_stale_pooled_session_reconnects_before_retrying_the_command() {
    pooled_transfer_recovery(true, false, true).await;
}

async fn pooled_transfer_recovery(
    reuse_returned: bool,
    second_fails: bool,
    read_with_stale_second: bool,
) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    struct Host;
    impl super::EngineHost for Host {
        fn open_db(&self) -> anyhow::Result<Connection> {
            crate::store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &Connection, _: &str, _: &mut imap::Creds) {}
        fn store_secret(
            &self,
            _: &Connection,
            _: &str,
            _: &crate::secrets::Secrets,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let failed = Arc::new(tokio::sync::Notify::new());
    let server_failed = failed.clone();
    let server = tokio::spawn(async move {
        for connection in 0..if read_with_stale_second { 3 } else { 2 } {
            let (socket, _) = listener.accept().await.unwrap();
            let (reader, mut writer) = socket.into_split();
            writer.write_all(b"* OK ready\r\n").await.unwrap();
            let mut lines = tokio::io::BufReader::new(reader).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                let (tag, command) = line.split_once(' ').unwrap();
                let operation = command.starts_with("SELECT");
                let fail = ((connection == 0 || second_fails) && operation)
                    || (read_with_stale_second && connection == 1 && command.starts_with("NOOP"));
                if connection >= 1 && operation {
                    writer.write_all(b"* 0 EXISTS\r\n").await.unwrap();
                }
                let status = if fail {
                    "NO genuine server refusal"
                } else {
                    "OK done"
                };
                writer
                    .write_all(format!("{tag} {status}\r\n").as_bytes())
                    .await
                    .unwrap();
                if fail {
                    server_failed.notify_one();
                }
                if fail || (connection >= 1 && operation) {
                    break;
                }
            }
        }
    });
    let engine = Arc::new(super::Engine::new(Box::new(Host)).unwrap());
    let creds = {
        let db = engine.db.lock().unwrap();
        let config = serde_json::json!({"host":"127.0.0.1", "port":port, "tls":false, "user":"u", "proxy":{"mode":"direct"}}).to_string();
        db.execute(
            "INSERT INTO accounts(id, config) VALUES('acc', ?1)",
            [&config],
        )
        .unwrap();
        crate::store::load_account(&db, "acc").unwrap().unwrap()
    };
    engine
        .accounts
        .lock()
        .await
        .insert("acc".into(), creds.clone());
    engine.return_pooled("acc", imap::connect(&creds).await.unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let coordination = engine.connect_lock("acc");
    let guard = if reuse_returned {
        Some(coordination.lock().await)
    } else {
        None
    };
    let worker = tokio::spawn({
        let engine = engine.clone();
        let calls = calls.clone();
        async move {
            if read_with_stale_second {
                engine
                    .with_read_session("acc", |session| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Box::pin(async move {
                            session.select("INBOX").await?;
                            Ok(())
                        })
                    })
                    .await
            } else {
                engine
                    .with_transfer_session("acc", Duration::from_secs(3), |session| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Box::pin(async move {
                            session.select("INBOX").await?;
                            Ok(())
                        })
                    })
                    .await
            }
        }
    });
    if reuse_returned {
        tokio::time::timeout(Duration::from_secs(1), failed.notified())
            .await
            .unwrap();
        engine.return_pooled("acc", imap::connect(&creds).await.unwrap());
    }
    drop(guard);
    let result = worker.await.unwrap();
    if second_fails {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("genuine server refusal")
        );
    } else {
        result.unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn transfer_deadline_bounds_connection_coordination() {
    struct Host;
    impl super::EngineHost for Host {
        fn open_db(&self) -> anyhow::Result<Connection> {
            crate::store::open_at(":memory:")
        }
        fn apply_secret(&self, _: &Connection, _: &str, _: &mut imap::Creds) {}
        fn store_secret(
            &self,
            _: &Connection,
            _: &str,
            _: &crate::secrets::Secrets,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }
    let engine = super::Engine::new(Box::new(Host)).unwrap();
    let coordination = engine.connect_lock("acc");
    let _connect_guard = coordination.lock().await;
    let _accounts_guard = engine.accounts.lock().await;
    let err = tokio::time::timeout(
        Duration::from_millis(500),
        engine.with_transfer_session::<(), _>("acc", Duration::from_millis(50), |_| {
            Box::pin(async { panic!("No command should run before preparation succeeds") })
        }),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(err.to_string().contains("session preparation timed out"));
    assert!(!err.is::<TransferTimedOut>());
}

#[test]
fn only_a_reconnect_failure_becomes_a_reconnect_event() {
    let refused = anyhow::anyhow!("account needs reconnect: a@example.com").context("sync INBOX");
    let (name, detail) = super::needs_reconnect_event("a@example.com", &refused).unwrap();
    assert_eq!(name, "account.needsReconnect");
    assert_eq!(detail["account"], "a@example.com");

    let offline = anyhow::anyhow!("connection refused");
    assert!(super::needs_reconnect_event("a@example.com", &offline).is_none());
}
