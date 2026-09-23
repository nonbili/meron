use super::{
    Pooled, SENT_COPY_MATCH_WINDOW_SECS, cached_archive_folder_from_folders,
    cached_search_mail_page, companion_folders, find_role_folder, limit_prefetch_uids,
    parse_background_sync_timeout, pool_return, pool_take, record_search_folder_result,
    sent_copy_landed, should_append_sent_copy, store_folder_tail, thread_gap_search_folders,
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
pub fn prefetch_limit_keeps_all_uids_when_uncapped() {
    assert_eq!(
        limit_prefetch_uids(vec![1, 2, 3, 4], None),
        vec![1, 2, 3, 4]
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
    assert_eq!(successes[0].1[0].folder, "INBOX");
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
