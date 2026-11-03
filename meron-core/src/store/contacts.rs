//! Recipient autocomplete from correspondents seen in the store.

use anyhow::Result;
use rusqlite::{Connection, params};

/// Cached mail involving an exact address, across folders and accounts. Never
/// match body text (or a substring of another person's address).
pub fn contact_messages(
    conn: &Connection,
    addr: &str,
) -> Result<Vec<(String, crate::imap::MessageHeader)>> {
    let addr = addr.trim();
    // FTS narrows candidates before exact JSON participant checks. Short
    // addresses fall back to the scan (the trigram index cannot answer them).
    let candidate = if addr.chars().count() >= 3 {
        "AND m.id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?2
          UNION SELECT rowid FROM messages_recipients_fts WHERE messages_recipients_fts MATCH ?3)"
    } else {
        "AND (?2 IS NOT NULL AND ?3 IS NOT NULL)"
    };
    let phrase = format!("\"{}\"", addr.replace('"', "\"\""));
    let mut stmt = conn.prepare(&format!(
        "SELECT uid, subject, from_name, from_addr, date, seen, starred, thread_key,
                json_extract(json, '$.to'), COALESCE(json_extract(json, '$.message_id'), ''),
                account, folder, json_extract(json, '$.cc')
         FROM messages m WHERE uid <> 0 {candidate} AND ?1 IS NOT NULL
         ORDER BY date DESC, uid DESC, account, folder",
    ))?;
    let rows = stmt.query_map(
        params![addr, format!("from_addr : {phrase}"), phrase],
        |row| {
            let mut header = super::search_header_from_row(row)?;
            header.folder = row.get(11)?;
            let cc = super::parse_recipients_json(row.get(12)?);
            let key = addr.to_lowercase();
            let matches = header.from_addr.to_lowercase() == key
                || header
                    .to
                    .iter()
                    .chain(cc.iter())
                    .any(|r| r.addr.to_lowercase() == key);
            let account: String = row.get(10)?;
            Ok(matches.then_some((account, header)))
        },
    )?;
    let candidates = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut excluded = std::collections::HashMap::new();
    let mut messages = Vec::new();
    for (account, header) in candidates.into_iter().flatten() {
        let key = (account.clone(), header.folder.clone());
        let hidden = match excluded.get(&key) {
            Some(hidden) => *hidden,
            None => {
                let role = super::folder_role(conn, &account, &header.folder)?;
                let hidden = matches!(role.as_str(), "junk" | "trash");
                excluded.insert(key, hidden);
                hidden
            }
        };
        if !hidden {
            messages.push((account, header));
        }
    }
    Ok(messages)
}

/// Build details only for the requested page. Grouping first preserves thread
/// identities across folders, but expensive counts/tags/roles stay page-sized.
pub fn contact_thread_page(
    conn: &Connection,
    addr: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<serde_json::Value> {
    use std::collections::{BTreeMap, HashSet};
    let mut accounts = BTreeMap::<String, Vec<_>>::new();
    for (account, header) in contact_messages(conn, addr)? {
        accounts.entry(account).or_default().push(header);
    }
    let identity = |account: &str, folder: &str, key: &str| {
        if key.starts_with("uid:") {
            format!("{account}\n{folder}\n{key}")
        } else {
            format!("{account}\n{key}")
        }
    };
    let mut candidates = Vec::new();
    for (account, headers) in &accounts {
        for card in super::group_thread_cards(headers.clone(), "INBOX") {
            candidates.push((
                card.header.date,
                identity(account, &card.header.folder, &card.thread_key),
            ));
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let after: Option<(i64, String)> = cursor
        .filter(|v| !v.is_empty())
        .map(serde_json::from_str)
        .transpose()?;
    if let Some((date, key)) = after {
        candidates.retain(|(d, k)| *d < date || (*d == date && *k > key));
    }
    let more = candidates.len() > limit;
    candidates.truncate(limit);
    let next = if more {
        candidates.last().map(serde_json::to_string).transpose()?
    } else {
        None
    };
    let selected: HashSet<_> = candidates.iter().map(|(_, key)| key.clone()).collect();
    let mut threads = Vec::new();
    for (account, headers) in accounts {
        let headers: Vec<_> = headers
            .into_iter()
            .filter(|h| {
                selected.contains(&identity(&account, &h.folder, &super::card_thread_key(h)))
            })
            .collect();
        if headers.is_empty() {
            continue;
        }
        let drafts = super::draft_thread_keys(conn, &account)?;
        threads.extend(crate::mail_model::thread_cards_json(
            conn, &account, "INBOX", headers, &drafts,
        )?);
    }
    threads.sort_by(|a, b| {
        b["date"]
            .as_i64()
            .cmp(&a["date"].as_i64())
            .then_with(|| a["thread_id"].as_str().cmp(&b["thread_id"].as_str()))
    });
    Ok(serde_json::json!({ "threads": threads, "next_cursor": next }))
}

/// A correspondent surfaced for recipient autocomplete.
#[derive(serde::Serialize)]
pub struct Contact {
    pub name: String,
    pub addr: String,
}

/// Suggest contacts for recipient autocomplete, drawn from both the senders and
/// the To/Cc recipients of cached messages. Results are distinct by address and
/// ranked by how many messages the address appears on. An empty `query` returns
/// the top correspondents; otherwise we match the substring against address and
/// display name. When `account` is empty we search across all accounts (unified
/// compose).
///
/// Reads the `contacts` directory the `messages` triggers keep (see
/// `db::contacts_ddl`), matching in Rust so a name's non-ASCII case folds,
/// which SQLite's `lower()` doesn't.
pub fn suggest_contacts(
    conn: &Connection,
    account: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<Contact>> {
    list_contacts(conn, account, query, limit, ContactSort::Frequent, false)
}

/// How [`list_contacts`] orders its results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactSort {
    /// Best match first while searching, then most messages.
    Frequent,
    /// By display name (the address when there is none), A to Z.
    Name,
    /// Newest message first.
    Recent,
}

impl ContactSort {
    /// Unknown values fall back to [`ContactSort::Frequent`], the autocomplete
    /// order, so older clients keep their behaviour.
    pub fn parse(value: &str) -> Self {
        match value {
            "name" => Self::Name,
            "recent" => Self::Recent,
            _ => Self::Frequent,
        }
    }
}

/// [`suggest_contacts`] in a chosen order. Only [`ContactSort::Frequent`]
/// puts the best match first: the Contacts panel's other orders hold while
/// its search narrows them.
///
/// A `directory` listing (the Contacts panel) leaves out the user's own
/// addresses, which every sent message counts, and, until a search asks for
/// them, automated senders nobody writes back to. Autocomplete keeps both:
/// mailing yourself is a real use.
pub fn list_contacts(
    conn: &Connection,
    account: &str,
    query: &str,
    limit: u32,
    sort: ContactSort,
    directory: bool,
) -> Result<Vec<Contact>> {
    use std::collections::{HashMap, HashSet};

    let q = query.trim().to_lowercase();
    let own: HashSet<String> = if directory {
        own_addresses(conn)?
    } else {
        HashSet::new()
    };
    let hide_automated = directory && q.is_empty();
    // Resolve metadata once, not once per contact. Folder roles can change
    // after messages were cached, so exclusion belongs on the read side, not
    // in the participant triggers. Missing metadata still uses name heuristics.
    let mut folder_roles = HashMap::new();
    let mut folders = conn.prepare("SELECT account, name, special_use FROM folders")?;
    for row in folders.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })? {
        let (account, name, special_use) = row?;
        let role = super::classify_folder_role(&name, special_use.as_deref());
        folder_roles.insert((account, name), role);
    }
    let account_filter = if account.is_empty() {
        "?1 = ''"
    } else {
        "account = ?1"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT addr_key, addr, name, count, last_date, account, folder
         FROM contacts WHERE {account_filter}
         ORDER BY addr_key, name"
    ))?;
    let rows = stmt.query_map(params![account], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, u32>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;

    // One entry per address, across accounts and the names it came with.
    struct Tally {
        names: Vec<(String, u32)>,
        addr: String,
        count: u32,
        last_date: i64,
    }
    let mut tallies: HashMap<String, Tally> = HashMap::new();
    for row in rows {
        let (key, addr, name, count, last_date, account, folder) = row?;
        let role = folder_roles
            .entry((account, folder.clone()))
            .or_insert_with(|| super::classify_folder_role(&folder, None));
        if matches!(*role, "junk" | "trash") {
            continue;
        }
        // Folded again: SQLite's `lower()` leaves non-ASCII capitals alone.
        let entry = tallies.entry(key.to_lowercase()).or_insert_with(|| Tally {
            names: Vec::new(),
            addr: String::new(),
            count: 0,
            last_date: 0,
        });
        entry.count += count;
        entry.last_date = entry.last_date.max(last_date);
        // Casing differs between rows; take one regardless of their order.
        if addr > entry.addr {
            entry.addr = addr;
        }
        if !name.is_empty() {
            match entry.names.iter_mut().find(|(seen, _)| *seen == name) {
                Some((_, seen)) => *seen += count,
                None => entry.names.push((name, count)),
            }
        }
    }

    // How well `text` holds the query: 0 at its start or a word's, 1 inside
    // a word.
    let rank = |text: &str| -> Option<u8> {
        let text = text.to_lowercase();
        let at = text.find(&q)?;
        let word_start = !text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric);
        Some(if word_start { 0 } else { 1 })
    };
    // The most frequent of `names`; ties go to the alphabetically first, for a
    // stable pick.
    let top = |names: &[&(String, u32)]| {
        names
            .iter()
            .rev()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name.clone())
            .unwrap_or_default()
    };
    struct Hit {
        rank: u8,
        count: u32,
        last_date: i64,
        name: String,
        addr: String,
    }
    let mut hits: Vec<Hit> = tallies
        .into_values()
        .filter_map(|t| {
            let key = t.addr.to_lowercase();
            if own.contains(&key) || (hide_automated && is_automated_address(&key)) {
                return None;
            }
            let all: Vec<&(String, u32)> = t.names.iter().collect();
            if q.is_empty() {
                return Some(Hit {
                    rank: 0,
                    count: t.count,
                    last_date: t.last_date,
                    name: top(&all),
                    addr: t.addr,
                });
            }
            let named: Vec<(&(String, u32), u8)> = t
                .names
                .iter()
                .filter_map(|entry| Some((entry, rank(&entry.0)?)))
                .collect();
            let name_rank = named.iter().map(|(_, rank)| *rank).min();
            if let Some(addr_rank) = rank(&t.addr) {
                return Some(Hit {
                    rank: name_rank.map_or(addr_rank, |name_rank| name_rank.min(addr_rank)),
                    count: t.count,
                    last_date: t.last_date,
                    name: top(&all),
                    addr: t.addr,
                });
            }
            // Matched by name alone: rank it, and show it, by the names that
            // match. A shared sender (a notifications address carrying each
            // actor's name) mustn't outrank a person on its other names' mail.
            let matching: Vec<&(String, u32)> = named.iter().map(|(entry, _)| *entry).collect();
            Some(Hit {
                rank: name_rank?,
                count: matching.iter().map(|(_, count)| count).sum(),
                last_date: t.last_date,
                name: top(&matching),
                addr: t.addr,
            })
        })
        .collect();
    let label = |hit: &Hit| {
        if hit.name.is_empty() {
            hit.addr.to_lowercase()
        } else {
            hit.name.to_lowercase()
        }
    };
    let by_addr = |a: &Hit, b: &Hit| a.addr.to_lowercase().cmp(&b.addr.to_lowercase());
    match sort {
        ContactSort::Frequent => hits.sort_by(|a, b| {
            a.rank
                .cmp(&b.rank)
                .then_with(|| b.count.cmp(&a.count))
                .then_with(|| by_addr(a, b))
        }),
        ContactSort::Name => {
            hits.sort_by_cached_key(|hit| (label(hit), hit.addr.to_lowercase()));
        }
        ContactSort::Recent => hits.sort_by(|a, b| {
            b.last_date
                .cmp(&a.last_date)
                .then_with(|| b.count.cmp(&a.count))
                .then_with(|| by_addr(a, b))
        }),
    }
    hits.truncate(limit as usize);
    Ok(hits
        .into_iter()
        .map(|hit| Contact {
            name: hit.name,
            addr: hit.addr,
        })
        .collect())
}

/// Every account's address and send-as aliases, lowercased.
fn own_addresses(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let accounts: Vec<(String, String)> = conn
        .prepare("SELECT id, email FROM accounts")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut own = std::collections::HashSet::new();
    for (id, email) in accounts {
        own.insert(email.trim().to_lowercase());
        for alias in super::account_aliases(conn, &id)? {
            own.insert(alias.email.trim().to_lowercase());
        }
    }
    own.remove("");
    Ok(own)
}

/// An address that sends but doesn't read replies: `no-reply@`,
/// `account-security-noreply@`, `comment@noreply.github.com`, bounce handlers
/// and the like. Only the first domain label is checked for no-reply patterns.
fn is_automated_address(addr: &str) -> bool {
    let (local, domain) = addr.split_once('@').unwrap_or((addr, ""));
    let local = local.to_lowercase();
    let is_no_reply = |part: &str| {
        let squashed: String = part
            .to_lowercase()
            .chars()
            .filter(|c| !matches!(c, '-' | '_' | '.'))
            .collect();
        squashed.contains("noreply") || squashed.contains("donotreply")
    };
    is_no_reply(&local)
        || is_no_reply(domain.split('.').next().unwrap_or(""))
        || matches!(
            local.as_str(),
            "mailer-daemon"
                | "postmaster"
                | "bounce"
                | "bounces"
                | "notification"
                | "notifications"
        )
}
