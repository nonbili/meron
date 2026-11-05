//! Rules: actions on matching mail after arrival, reading, or a given age.

use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::tags::{message_tag_keys, write_tag_changes};

const RULES_KEY: &str = "rules";

/// Tries a queued action gets before it is given up on, so a folder deleted
/// since the rule ran can't fail on every sync forever.
const MAX_RULE_ACTION_ATTEMPTS: i64 = 5;

/// Entries the rule log keeps.
const RULE_LOG_SIZE: i64 = 200;

/// One search and everything to do with the new mail it matches. Rules live in
/// `settings`, so backups carry them, and are kept per device: they run on the
/// accounts in `accounts`, or every account when that is empty.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: String,
    /// In the search box's syntax, read against what a new message's envelope
    /// says: its body is not here yet. Empty for all new mail.
    #[serde(default)]
    pub query: String,
    /// Arrival is the default for existing rules. Age uses the message date and
    /// includes existing Inbox mail; read starts when an unread-to-read change
    /// is observed on this device.
    #[serde(default)]
    pub trigger: RuleTrigger,
    /// Seconds after the trigger before checking the conditions and acting.
    #[serde(default)]
    pub delay_seconds: i64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub mark_read: bool,
    #[serde(default)]
    pub star: bool,
    /// `""` to leave the message where it is, or `archive`, `trash` or `folder`.
    #[serde(default)]
    pub move_to: String,
    /// The mailbox for `move_to: "folder"`.
    #[serde(default)]
    pub folder: String,
    /// The account of `folder`. Empty for the mail's own account, where a folder
    /// the account lacks skips the move.
    #[serde(default)]
    pub folder_account: String,
    /// The accounts this rule runs on; empty for all of them.
    #[serde(default)]
    pub accounts: Vec<String>,
    /// The mailbox to copy matching mail to, `""` for none, in `copy_account`.
    #[serde(default)]
    pub copy_folder: String,
    #[serde(default)]
    pub copy_account: String,
    /// Kept, but not run.
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleTrigger {
    #[default]
    Arrival,
    Read,
    Age,
}

impl Rule {
    fn has_action(&self) -> bool {
        !self.tags.is_empty()
            || self.mark_read
            || self.star
            || !self.move_to.is_empty()
            || !self.copy_folder.is_empty()
    }
}

pub fn rules(conn: &Connection) -> Result<Vec<Rule>> {
    let stored = super::settings_get(conn, &[RULES_KEY.to_string()])?;
    Ok(stored
        .get(RULES_KEY)
        .and_then(|rules| serde_json::from_value(rules.clone()).ok())
        .unwrap_or_default())
}

/// Replace the rules, refusing any that could never apply.
pub fn set_rules(conn: &Connection, incoming: Vec<Rule>) -> Result<Vec<Rule>> {
    // Also used inside tag-renaming transactions. Keep rules and their event
    // cutoffs atomic without nesting a transaction in that case.
    let tx = if conn.is_autocommit() {
        Some(Transaction::new_unchecked(
            conn,
            TransactionBehavior::Immediate,
        )?)
    } else {
        None
    };
    let previous = rules(conn)?;
    let mut clean = Vec::with_capacity(incoming.len());
    for mut rule in incoming {
        anyhow::ensure!(
            (0..=315_360_000).contains(&rule.delay_seconds),
            "a rule's delay must be between zero and ten years"
        );
        rule.query = rule.query.trim().to_string();
        anyhow::ensure!(
            rule.query.is_empty()
                || !crate::search_query::SearchQuery::parse(&rule.query).is_empty(),
            "a rule's search means nothing"
        );
        let tags = serde_json::json!(rule.tags);
        rule.tags = crate::tags::parse_tag_list(Some(&tags)).map_err(anyhow::Error::msg)?;
        rule.folder = rule.folder.trim().to_string();
        match rule.move_to.as_str() {
            "" | "archive" | "trash" => {
                rule.folder.clear();
                rule.folder_account.clear();
            }
            "folder" => anyhow::ensure!(!rule.folder.is_empty(), "a rule needs a folder"),
            other => anyhow::bail!("unknown rule move: {other}"),
        }
        rule.accounts.sort();
        rule.accounts.dedup();
        rule.copy_folder = rule.copy_folder.trim().to_string();
        anyhow::ensure!(
            rule.copy_folder.is_empty() == rule.copy_account.is_empty(),
            "a rule's copy needs an account and a folder"
        );
        anyhow::ensure!(rule.has_action(), "a rule needs an action");
        if rule.id.is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }
        clean.push(rule);
    }
    for rule in &clean {
        let timing_unchanged = previous.iter().any(|old| {
            old.id == rule.id
                && old.trigger == rule.trigger
                && old.delay_seconds == rule.delay_seconds
                && (!old.disabled || rule.disabled)
                // Expanding scope starts a fresh event window so newly added
                // accounts cannot replay their historical events.
                && (old.accounts.is_empty() || (!rule.accounts.is_empty()
                    && rule.accounts.iter().all(|id| old.accounts.contains(id))))
        });
        if !timing_unchanged {
            conn.execute(
                "INSERT OR REPLACE INTO rule_activation (rule_id, event_sequence)
                 SELECT ?1, value FROM rule_event_sequence",
                [&rule.id],
            )?;
            conn.execute(
                "DELETE FROM rule_evaluations WHERE rule_id = ?1",
                [&rule.id],
            )?;
        }
    }
    let stored = serde_json::json!(clean);
    for table in ["rule_activation", "rule_evaluations", "rule_firings"] {
        conn.execute(&format!(
            "DELETE FROM {table} WHERE rule_id NOT IN (SELECT json_extract(value, '$.id') FROM json_each(?1))"
        ), [stored.to_string()])?;
    }
    super::setting_set(conn, RULES_KEY, &stored)?;
    if let Some(tx) = tx {
        tx.commit()?;
    }
    Ok(clean)
}

/// A rule's server work on one message, waiting for the sync that matched it to
/// finish. Queued in the store so that a failed write is retried, not lost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleAction {
    pub folder: String,
    pub uid: u32,
    pub uidvalidity: u32,
    pub mark_read: bool,
    pub star: bool,
    pub trash: bool,
    /// The mailbox to move to, `""` for none.
    pub target: String,
    /// Where to copy to first. Kept until all of this is done, so each retry asks
    /// the target again before giving up the source.
    pub copies: Vec<RuleCopy>,
    /// The rules behind this, for the log.
    pub rule_ids: Vec<String>,
    /// Positive for manual runs, negative for scheduled runs, 0 for new mail.
    /// Marks whose work a finished step was: one still in flight when its run is stopped must not touch what
    /// a later run queued for the same message.
    pub run: i64,
}

/// One line of what rules did.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
pub struct RuleLogEntry {
    pub at: i64,
    pub rule_id: String,
    pub account: String,
    /// The message's subject; empty for a run by hand, which logs one line.
    pub subject: String,
    pub count: i64,
    pub failed: bool,
    pub detail: String,
}

/// A mailbox on any account.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct RuleCopy {
    pub account: String,
    pub folder: String,
}

/// What a rule's move comes to for one message.
struct RuleDestination {
    trash: bool,
    target: String,
    copy: Option<RuleCopy>,
}

impl RuleAction {
    pub fn moves(&self) -> bool {
        self.trash || !self.target.is_empty()
    }
}

/// Where `rule` sends mail in `folder`: a message already there has nowhere to
/// go, and neither does one bound for a folder this account lacks. A move to
/// another account is a copy there and then Trash here, and what fails between
/// the two is retried from the copy, which looks before it appends.
fn rule_move(
    conn: &Connection,
    account: &str,
    folder: &str,
    rule: &Rule,
) -> Result<Option<RuleDestination>> {
    let target = match rule.move_to.as_str() {
        "trash" => {
            return Ok(Some(RuleDestination {
                trash: true,
                target: String::new(),
                copy: None,
            }));
        }
        "folder" if !rule.folder_account.is_empty() && rule.folder_account != account => {
            return Ok(Some(RuleDestination {
                trash: true,
                target: String::new(),
                copy: Some(RuleCopy {
                    account: rule.folder_account.clone(),
                    folder: crate::mail_model::canon_folder(&rule.folder),
                }),
            }));
        }
        "archive" => crate::engine::cached_archive_folder_from_folders(
            super::get_folders(conn, account)?,
            folder,
        ),
        "folder" => {
            let wanted = crate::mail_model::canon_folder(&rule.folder);
            super::get_folders(conn, account)?
                .into_iter()
                .find(|item| item.name == wanted || item.display_name == rule.folder)
                .map(|item| item.name)
        }
        _ => return Ok(None),
    };
    let Some(target) = target else {
        crate::mlog!(
            crate::log::Level::Warn,
            "mail.rules",
            "account={account}: rule {} has no folder to move to here",
            rule.id
        );
        return Ok(None);
    };
    Ok((target != folder).then_some(RuleDestination {
        trash: false,
        target,
        copy: None,
    }))
}

/// Where `rule` copies mail in `folder`, unless that is where it already is.
fn rule_copy(account: &str, folder: &str, rule: &Rule) -> Option<RuleCopy> {
    if rule.copy_folder.is_empty() {
        return None;
    }
    let target = crate::mail_model::canon_folder(&rule.copy_folder);
    (rule.copy_account != account || target != folder).then(|| RuleCopy {
        account: rule.copy_account.clone(),
        folder: target,
    })
}

/// Run the rules over mail that has just arrived in `folder`: never seen on this
/// account, in any folder. Only then, so undoing what a rule did by hand sticks.
/// Mail the user wrote or threw away has not arrived in that sense.
///
/// Tags are saved here; what must reach the server first waits in the queue for
/// [`crate::engine::run_rule_actions`]. Several rules can match one message:
/// each adds what it does, and the first to move it decides where.
pub fn apply_rules(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uids: &[u32],
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    // Reading the rules and applying them are one step: see
    // [`super::replace_tag_and_rules`].
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    for uid in uids {
        tx.execute(
            "UPDATE rule_events SET arrived_at = COALESCE(arrived_at, ?4)
             WHERE message = (SELECT id FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3)",
            params![account, folder, uid, super::now_unix()],
        )?;
    }
    let rules = rules(&tx)?;
    if rules.is_empty() {
        tx.commit()?;
        return Ok(());
    }
    let role = super::folder_role(&tx, account, folder)?;
    if matches!(role.as_str(), "sent" | "drafts" | "trash" | "junk") {
        return Ok(());
    }
    let mut actions: BTreeMap<u32, RuleAction> = BTreeMap::new();
    for rule in rules {
        if rule.trigger != RuleTrigger::Arrival
            || rule.delay_seconds != 0
            || rule.disabled
            || (!rule.accounts.is_empty() && !rule.accounts.iter().any(|item| item == account))
        {
            continue;
        }
        let matched = if rule.query.is_empty() {
            uids.to_vec()
        } else {
            super::matching_uids(&tx, account, folder, &rule.query, uids)?
        };
        for uid in &matched {
            let subject = rule_message_subject(&tx, account, folder, *uid)?;
            log_rule(&tx, &rule.id, account, &subject, 1, None)?;
        }
        add_rule_effects(
            &tx,
            account,
            folder,
            uidvalidity,
            &rule,
            matched,
            &mut actions,
        )?;
    }
    queue_rule_actions(&tx, account, &actions, 0)?;
    trim_rule_log(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Check persistent event clocks during sync. Delayed work is only queued when
/// due, so edits, disabling a rule, or marking mail unread take effect first.
/// Each rule fires once per cached message; manual runs remain independent.
pub fn apply_due_rules(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    now: i64,
) -> Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let role = super::folder_role(&tx, account, folder)?;
    if matches!(role.as_str(), "sent" | "drafts" | "trash" | "junk") {
        return Ok(());
    }
    let mut actions = BTreeMap::new();
    for rule in rules(&tx)? {
        if rule.disabled
            || (rule.trigger == RuleTrigger::Arrival && rule.delay_seconds == 0)
            || (!rule.accounts.is_empty() && !rule.accounts.iter().any(|id| id == account))
            || (rule.trigger == RuleTrigger::Age && folder != EXISTING_RULE_FOLDER)
        {
            continue;
        }
        let (clock, event_sequence, eligible) = match rule.trigger {
            RuleTrigger::Arrival => (
                "e.arrived_at",
                "e.arrived_sequence",
                "e.arrived_sequence > COALESCE(a.event_sequence, 0)",
            ),
            RuleTrigger::Read => (
                "CASE WHEN m.seen <> 0 THEN e.read_at END",
                "e.read_sequence",
                "e.read_sequence > COALESCE(a.event_sequence, 0)",
            ),
            RuleTrigger::Age => ("NULLIF(m.date, 0)", "m.date", "1"),
        };
        let mut stmt = tx.prepare(&format!(
            "SELECT m.uid, {event_sequence} FROM messages m JOIN rule_events e ON e.message = m.id
             LEFT JOIN rule_activation a ON a.rule_id = ?4
             WHERE m.account = ?1 AND m.folder = ?2 AND m.uid <> 0
               AND {clock} <= ?3 AND ({eligible})
               AND NOT EXISTS (SELECT 1 FROM rule_firings f WHERE f.message = m.id AND f.rule_id = ?4)
               AND NOT EXISTS (SELECT 1 FROM rule_evaluations v
                 WHERE v.message = m.id AND v.rule_id = ?4 AND v.event_sequence = {event_sequence} AND v.query = ?5)"
        ))?;
        let candidates = stmt
            .query_map(
                params![
                    account,
                    folder,
                    now - rule.delay_seconds,
                    rule.id,
                    rule.query
                ],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, i64>(1)?)),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut matched = Vec::new();
        // Bound the search's UID list, and remember checks that did not match.
        // Input changes invalidate those checks through the store's triggers.
        for chunk in candidates.chunks(500) {
            let uids: Vec<u32> = chunk.iter().map(|(uid, _)| *uid).collect();
            if rule.query.is_empty() {
                matched.extend(uids);
            } else {
                matched.extend(super::matching_uids(
                    &tx,
                    account,
                    folder,
                    &rule.query,
                    &uids,
                )?);
            }
            for (uid, sequence) in chunk {
                tx.execute(
                    "INSERT OR REPLACE INTO rule_evaluations (message, rule_id, event_sequence, query)
                     SELECT id, ?4, ?5, ?6 FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                    params![account, folder, uid, rule.id, sequence, rule.query],
                )?;
            }
        }
        for uid in &matched {
            tx.execute(
                "INSERT INTO rule_firings (message, rule_id)
                 SELECT id, ?4 FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                params![account, folder, uid, rule.id],
            )?;
            let subject = rule_message_subject(&tx, account, folder, *uid)?;
            log_rule(&tx, &rule.id, account, &subject, 1, None)?;
        }
        add_rule_effects(
            &tx,
            account,
            folder,
            uidvalidity,
            &rule,
            matched,
            &mut actions,
        )?;
    }
    // A separate run preserves actions already queued for the same UID.
    // Negative IDs distinguish scheduled work from positive manual runs.
    let run = -(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_micros() as i64);
    queue_rule_actions(&tx, account, &actions, run)?;
    trim_rule_log(&tx)?;
    tx.commit()?;
    Ok(())
}

fn rule_message_subject(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
) -> Result<String> {
    Ok(conn
        .query_row(
            "SELECT COALESCE(subject, '') FROM messages
             WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_default())
}

fn log_rule(
    conn: &Connection,
    rule_id: &str,
    account: &str,
    subject: &str,
    count: usize,
    failure: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO rule_log (at, rule_id, account, subject, count, failed, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            super::now_unix(),
            rule_id,
            account,
            subject,
            count as i64,
            failure.is_some(),
            failure.unwrap_or_default()
        ],
    )?;
    Ok(())
}

fn trim_rule_log(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM rule_log WHERE id <= (SELECT MAX(id) FROM rule_log) - ?1",
        params![RULE_LOG_SIZE],
    )?;
    Ok(())
}

/// What rules did lately, newest first.
pub fn rule_log(conn: &Connection) -> Result<Vec<RuleLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT at, rule_id, account, subject, count, failed, detail
         FROM rule_log ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RuleLogEntry {
            at: row.get(0)?,
            rule_id: row.get(1)?,
            account: row.get(2)?,
            subject: row.get(3)?,
            count: row.get(4)?,
            failed: row.get(5)?,
            detail: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// How many messages each rule run by hand has left.
pub fn running_rules(conn: &Connection) -> Result<BTreeMap<String, i64>> {
    let mut stmt =
        conn.prepare("SELECT rule_ids, COUNT(*) FROM rule_actions WHERE run > 0 GROUP BY 1")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut running = BTreeMap::new();
    for row in rows {
        let (ids, count) = row?;
        for id in serde_json::from_str::<Vec<String>>(&ids).unwrap_or_default() {
            *running.entry(id).or_insert(0) += count;
        }
    }
    Ok(running)
}

/// Stop a rule run by hand: what is done stays done, the rest is dropped.
pub fn stop_rule(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM rule_actions WHERE run > 0 AND rule_ids = ?1",
        params![serde_json::to_string(&[id])?],
    )?;
    Ok(())
}

/// Take a removed account out of the rules. A rule that still has accounts to
/// run on carries on; one left with none, or that sent mail there, is turned off
/// rather than left to fail, or to start running on every account.
pub fn forget_rule_account(conn: &Connection, id: &str) -> Result<()> {
    let mut stored = rules(conn)?;
    if stored.is_empty() {
        return Ok(());
    }
    for rule in &mut stored {
        let scoped = rule.accounts.iter().any(|item| item == id);
        if scoped && rule.accounts.len() > 1 {
            rule.accounts.retain(|item| item != id);
        }
        if (scoped && rule.accounts.len() == 1 && rule.accounts[0] == id)
            || rule.copy_account == id
            || rule.folder_account == id
        {
            rule.disabled = true;
        }
    }
    super::setting_set(conn, RULES_KEY, &serde_json::json!(stored))?;
    Ok(())
}

/// Save `rule`'s tags on `matched` and add what is left for the server to `actions`.
fn add_rule_effects(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    rule: &Rule,
    matched: Vec<u32>,
    actions: &mut BTreeMap<u32, RuleAction>,
) -> Result<()> {
    if matched.is_empty() {
        return Ok(());
    }
    if !rule.tags.is_empty() {
        let copies = BTreeMap::from([(folder.to_string(), matched.clone())]);
        let keys = message_tag_keys(conn, account, &copies)?;
        write_tag_changes(conn, account, &keys, &rule.tags, &[], true)?;
    }
    let destination = rule_move(conn, account, folder, rule)?;
    let copy = rule_copy(account, folder, rule);
    // Mail with no Message-ID can't be told from its own copies: each would
    // count as new where it lands, and rules pointing at each other would never
    // stop. It stays put, for the actions that can't loop.
    let copying = copy.is_some() || destination.as_ref().is_some_and(|it| it.copy.is_some());
    let ids = if copying {
        super::cached_message_ids(conn, account, folder, &matched)?
    } else {
        Vec::new()
    };
    for (at, uid) in matched.into_iter().enumerate() {
        let identified = ids
            .get(at)
            .is_none_or(|id| id.as_ref().is_some_and(|id| !id.is_empty()));
        if !identified {
            let subject = rule_message_subject(conn, account, folder, uid)?;
            let reason = "this message has no Message-ID, so a rule can't safely copy it";
            log_rule(conn, &rule.id, account, &subject, 1, Some(reason))?;
        }
        let action = actions.entry(uid).or_insert_with(|| RuleAction {
            folder: folder.to_string(),
            uid,
            uidvalidity,
            ..RuleAction::default()
        });
        action.mark_read |= rule.mark_read;
        action.star |= rule.star;
        action.rule_ids.push(rule.id.clone());
        let destination = destination
            .as_ref()
            .filter(|it| identified || it.copy.is_none());
        if let Some(destination) = destination.filter(|_| !action.moves()) {
            action.trash = destination.trash;
            action.target = destination.target.clone();
            action.copies.extend(destination.copy.clone());
        }
        if identified {
            action.copies.extend(copy.clone());
        }
        action.copies.sort();
        action.copies.dedup();
    }
    Ok(())
}

fn queue_rule_actions(
    conn: &Connection,
    account: &str,
    actions: &BTreeMap<u32, RuleAction>,
    run: i64,
) -> Result<()> {
    for action in actions.values() {
        // The move delivers it there, on all the rules combined.
        let mut copies: Vec<&RuleCopy> = Vec::new();
        for copy in &action.copies {
            let delivered = copy.account == account
                && (copy.folder == action.target
                    || (action.trash
                        && super::folder_role(conn, account, &copy.folder)? == "trash"));
            if !delivered {
                copies.push(copy);
            }
        }
        if !action.mark_read && !action.star && !action.moves() && copies.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR REPLACE INTO rule_actions
               (account, folder, uid, uidvalidity, mark_read, star, trash, target, copies,
                rule_ids, run, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)",
            params![
                account,
                action.folder,
                action.uid,
                action.uidvalidity,
                action.mark_read,
                action.star,
                action.trash,
                action.target,
                serde_json::to_string(&copies)?,
                serde_json::to_string(&action.rule_ids)?,
                run
            ],
        )?;
    }
    Ok(())
}

/// Where running a rule by hand looks: the rest of a mailbox is mostly mail
/// already filed, on purpose.
pub const EXISTING_RULE_FOLDER: &str = "INBOX";

/// The accounts `rule` can run on: its own, or every account with a server.
fn rule_accounts(conn: &Connection, rule: &Rule) -> Result<Vec<String>> {
    let field = |account: &serde_json::Value, name: &str| {
        account[name].as_str().unwrap_or_default().to_string()
    };
    Ok(super::list_accounts(conn)?
        .iter()
        .filter(|account| field(account, "engine") != "rss" && field(account, "provider") != "rss")
        .map(|account| field(account, "id"))
        .filter(|id| !super::is_local_account(id))
        .filter(|id| rule.accounts.is_empty() || rule.accounts.contains(id))
        .collect())
}

fn saved_rule(conn: &Connection, id: &str) -> Result<Rule> {
    rules(conn)?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| anyhow::anyhow!("no such rule: {id}"))
}

/// The accounts a saved rule can run on.
pub fn saved_rule_accounts(conn: &Connection, id: &str) -> Result<Vec<String>> {
    rule_accounts(conn, &saved_rule(conn, id)?)
}

/// What `rule` matches among the mail cached in `account`'s Inbox.
fn existing_rule_matches(conn: &Connection, account: &str, rule: &Rule) -> Result<Vec<u32>> {
    let folder = EXISTING_RULE_FOLDER;
    let mut stmt =
        conn.prepare("SELECT uid FROM messages WHERE account = ?1 AND folder = ?2 AND uid <> 0")?;
    let uids = stmt
        .query_map(params![account, folder], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<u32>>>()?;
    if rule.query.is_empty() {
        return Ok(uids);
    }
    super::matching_uids(conn, account, folder, &rule.query, &uids)
}

/// How many messages [`queue_existing_rule`] would look at.
pub fn count_existing_rule_matches(conn: &Connection, id: &str) -> Result<usize> {
    let rule = saved_rule(conn, id)?;
    let mut count = 0;
    for account in rule_accounts(conn, &rule)? {
        count += existing_rule_matches(conn, &account, &rule)?.len();
    }
    Ok(count)
}

/// Run a saved rule over mail already in the Inbox, for
/// [`crate::engine::run_rule_on_existing`] to carry out. Answers with the
/// accounts and how many messages each queued. Mail this rule already has work
/// queued for keeps that work; other rules' work runs alongside, under its own
/// run. And [`crate::engine`] asks the target before each copy, so
/// running a rule twice doesn't double what it did.
pub fn queue_existing_rule(conn: &Connection, id: &str) -> Result<Vec<(String, usize)>> {
    let folder = EXISTING_RULE_FOLDER;
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let rule = saved_rule(&tx, id)?;
    anyhow::ensure!(!rule.disabled, "the rule is turned off");
    let run = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_micros() as i64;
    let mut queued = Vec::new();
    for account in rule_accounts(&tx, &rule)? {
        let Some((uidvalidity, _)) = super::get_folder_state(&tx, &account, folder)? else {
            continue;
        };
        let pending = pending_rule_actions(&tx, &account, folder)?;
        let matched: Vec<u32> = existing_rule_matches(&tx, &account, &rule)?
            .into_iter()
            .filter(|uid| {
                !pending
                    .iter()
                    .any(|action| action.uid == *uid && action.rule_ids.contains(&rule.id))
            })
            .collect();
        let count = matched.len();
        let mut actions = BTreeMap::new();
        add_rule_effects(
            &tx,
            &account,
            folder,
            uidvalidity,
            &rule,
            matched,
            &mut actions,
        )?;
        queue_rule_actions(&tx, &account, &actions, run)?;
        if count > 0 {
            log_rule(&tx, &rule.id, &account, "", count, None)?;
            queued.push((account, count));
        }
    }
    trim_rule_log(&tx)?;
    tx.commit()?;
    Ok(queued)
}

pub fn pending_rule_actions(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Vec<RuleAction>> {
    let mut stmt = conn.prepare(
        "SELECT folder, uid, uidvalidity, mark_read, star, trash, target, copies, rule_ids, run
         FROM rule_actions WHERE account = ?1 AND folder = ?2 ORDER BY uid",
    )?;
    let rows = stmt.query_map(params![account, folder], |row| {
        Ok(RuleAction {
            folder: row.get(0)?,
            uid: row.get(1)?,
            uidvalidity: row.get(2)?,
            mark_read: row.get(3)?,
            star: row.get(4)?,
            trash: row.get(5)?,
            target: row.get(6)?,
            copies: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
            rule_ids: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
            run: row.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Note moves that didn't happen, as another rule had already moved the message.
pub fn log_rule_lost_moves(conn: &Connection, account: &str, lost: &[RuleAction]) -> Result<()> {
    for action in lost {
        for rule_id in &action.rule_ids {
            let reason = "another rule had already moved this message";
            log_rule(conn, rule_id, account, "", 1, Some(reason))?;
        }
    }
    trim_rule_log(conn)
}

/// Clear what `run` queued for `uids`.
pub fn finish_rule_actions(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
    run: i64,
) -> Result<()> {
    for uid in uids {
        conn.execute(
            "DELETE FROM rule_actions
             WHERE account = ?1 AND folder = ?2 AND uid = ?3 AND run = ?4",
            params![account, folder, uid, run],
        )?;
    }
    Ok(())
}

/// Count a failed try, dropping what has now had its last one and logging it
/// against the rules behind it. Answers with how many were dropped.
pub fn retry_rule_actions(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
    run: i64,
    reason: &str,
) -> Result<usize> {
    let queued = pending_rule_actions(conn, account, folder)?;
    let mut dropped = 0;
    for uid in uids {
        conn.execute(
            "UPDATE rule_actions SET attempts = attempts + 1
             WHERE account = ?1 AND folder = ?2 AND uid = ?3 AND run = ?4",
            params![account, folder, uid, run],
        )?;
        let gone = conn.execute(
            "DELETE FROM rule_actions
             WHERE account = ?1 AND folder = ?2 AND uid = ?3 AND run = ?4 AND attempts >= ?5",
            params![account, folder, uid, run, MAX_RULE_ACTION_ATTEMPTS],
        )?;
        dropped += gone;
        if gone == 0 {
            continue;
        }
        let subject = rule_message_subject(conn, account, folder, *uid)?;
        let action = queued
            .iter()
            .find(|action| action.uid == *uid && action.run == run);
        for rule_id in action.iter().flat_map(|action| &action.rule_ids) {
            log_rule(conn, rule_id, account, &subject, 1, Some(reason))?;
        }
    }
    trim_rule_log(conn)?;
    Ok(dropped)
}

/// [`super::replace_tag`] for the rules: they apply the new name, or lose the
/// tag, and a rule left doing nothing is gone. Answers with the rules as they
/// now stand.
pub fn replace_rule_tag(conn: &Connection, tag: &str, to: Option<&str>) -> Result<Vec<Rule>> {
    let current = rules(conn)?;
    if to == Some(tag) {
        return Ok(current);
    }
    let mut next = Vec::with_capacity(current.len());
    for mut rule in current {
        if let Some(to) = to {
            rule.query = crate::search_query::rename_tag_terms(&rule.query, tag, to);
        }
        if rule.tags.iter().any(|item| item == tag) {
            rule.tags.retain(|item| item != tag);
            rule.tags.extend(to.map(str::to_string));
            rule.tags.sort();
            rule.tags.dedup();
        }
        if rule.has_action() {
            next.push(rule);
        }
    }
    set_rules(conn, next)
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    use crate::imap::MessageHeader;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn
    }

    fn mail(conn: &Connection, uid: u32, seen: bool, date: i64) {
        crate::store::upsert_messages(
            conn,
            "a",
            "INBOX",
            &[MessageHeader {
                uid,
                seen,
                date,
                subject: "login code".into(),
                message_id: format!("<{uid}@example.com>"),
                ..Default::default()
            }],
        )
        .unwrap();
    }

    fn rule(trigger: RuleTrigger, delay_seconds: i64) -> Rule {
        Rule {
            id: "timed".into(),
            trigger,
            delay_seconds,
            star: true,
            ..Default::default()
        }
    }

    fn due(conn: &Connection, now: i64) -> Vec<RuleAction> {
        apply_due_rules(conn, "a", "INBOX", 7, now).unwrap();
        pending_rule_actions(conn, "a", "INBOX").unwrap()
    }

    #[test]
    fn old_rules_default_to_immediate_arrival_and_delays_are_validated() {
        let old: Rule = serde_json::from_str(r#"{"id":"old","star":true}"#).unwrap();
        assert_eq!(old.trigger, RuleTrigger::Arrival);
        assert_eq!(old.delay_seconds, 0);
        let conn = db();
        assert!(set_rules(&conn, vec![rule(RuleTrigger::Read, -1)]).is_err());
        assert!(set_rules(&conn, vec![rule(RuleTrigger::Age, i64::MAX)]).is_err());
        assert!(serde_json::from_str::<Rule>(r#"{"trigger":"unknown"}"#).is_err());
    }

    #[test]
    fn arrival_delay_survives_sync_and_does_not_apply_to_initial_cache() {
        let conn = db();
        set_rules(&conn, vec![rule(RuleTrigger::Arrival, 300)]).unwrap();
        mail(&conn, 1, false, 100);
        mail(&conn, 2, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[2]).unwrap();
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );
        conn.execute(
            "UPDATE rule_events SET arrived_at = 1000 WHERE arrived_at IS NOT NULL",
            [],
        )
        .unwrap();
        mail(&conn, 2, false, 100); // Envelope refresh must preserve the clock.
        assert!(due(&conn, 1299).is_empty());
        let actions = due(&conn, 1300);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].uid, 2);
        assert_eq!(actions[0].uidvalidity, 7);
        finish_rule_actions(&conn, "a", "INBOX", &[2], actions[0].run).unwrap();
        assert!(due(&conn, 1500).is_empty());
    }

    #[test]
    fn read_delay_tracks_transitions_and_unread_cancels_the_clock() {
        let conn = db();
        set_rules(&conn, vec![rule(RuleTrigger::Read, 300)]).unwrap();
        mail(&conn, 1, false, 100);
        mail(&conn, 2, true, 100); // Already-read historical mail has no read event.
        assert!(due(&conn, 1000).is_empty());
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        conn.execute(
            "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
            [],
        )
        .unwrap();
        mail(&conn, 1, true, 100); // Repeated seen flags must not restart the delay.
        assert!(due(&conn, 1299).is_empty());
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, false).unwrap();
        assert!(due(&conn, 1300).is_empty());
        // Bulk flag changes use the same clock as per-message operations.
        conn.execute("UPDATE messages SET seen = 1 WHERE uid = 1", [])
            .unwrap();
        conn.execute(
            "UPDATE rule_events SET read_at = 2000 WHERE read_at IS NOT NULL",
            [],
        )
        .unwrap();
        assert!(due(&conn, 2299).is_empty());
        assert_eq!(due(&conn, 2300)[0].uid, 1);
    }

    #[test]
    fn age_rules_include_existing_mail_and_respect_conditions_and_disable() {
        let conn = db();
        mail(&conn, 1, false, 100);
        mail(&conn, 2, false, 901);
        mail(&conn, 3, false, 0); // Unknown dates must not look infinitely old.
        let mut rule = rule(RuleTrigger::Age, 100);
        rule.query = "subject:login".into();
        rule.accounts = vec!["other".into()];
        set_rules(&conn, vec![rule.clone()]).unwrap();
        assert!(due(&conn, 1000).is_empty());
        rule.accounts.clear();
        rule.disabled = true;
        set_rules(&conn, vec![rule.clone()]).unwrap();
        assert!(due(&conn, 1000).is_empty());
        rule.disabled = false;
        rule.query = "subject:unmatched".into();
        set_rules(&conn, vec![rule.clone()]).unwrap();
        assert!(due(&conn, 1000).is_empty());
        rule.query = "subject:login".into();
        set_rules(&conn, vec![rule]).unwrap();
        let actions = due(&conn, 1000);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].uid, 1);
        conn.execute("DELETE FROM messages WHERE uid = 1", [])
            .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM rule_firings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn newly_created_read_rules_only_consider_later_events_even_in_the_same_second() {
        let conn = db();
        mail(&conn, 1, false, 100);
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        set_rules(&conn, vec![rule(RuleTrigger::Read, 300)]).unwrap();
        conn.execute(
            "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
            [],
        )
        .unwrap();
        assert!(due(&conn, 2000).is_empty());
        // A fresh transition on this same message is eligible. Eligibility uses
        // sequences rather than wall time, so ties in event seconds are safe.
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, false).unwrap();
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        conn.execute(
            "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
            [],
        )
        .unwrap();
        assert_eq!(due(&conn, 2000)[0].uid, 1);
    }

    #[test]
    fn changing_an_immediate_rule_to_delayed_does_not_replay_old_arrivals() {
        let conn = db();
        set_rules(&conn, vec![rule(RuleTrigger::Arrival, 0)]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        finish_rule_actions(&conn, "a", "INBOX", &[1], 0).unwrap();
        set_rules(&conn, vec![rule(RuleTrigger::Arrival, 300)]).unwrap();
        mail(&conn, 2, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[2]).unwrap();
        conn.execute(
            "UPDATE rule_events SET arrived_at = 1000 WHERE arrived_at IS NOT NULL",
            [],
        )
        .unwrap();
        let actions = due(&conn, 2000);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].uid, 2);
    }

    #[test]
    fn unchanged_rules_keep_their_pending_event_clocks() {
        let conn = db();
        let original = rule(RuleTrigger::Read, 300);
        set_rules(&conn, vec![original.clone()]).unwrap();
        mail(&conn, 1, false, 100);
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        conn.execute(
            "UPDATE rule_events SET read_at = 1000 WHERE read_at IS NOT NULL",
            [],
        )
        .unwrap();
        let edited = Rule {
            move_to: "trash".into(),
            ..original
        };
        set_rules(&conn, vec![edited]).unwrap();
        assert!(due(&conn, 1299).is_empty());
        assert!(due(&conn, 1300)[0].trash);
    }

    #[test]
    fn age_checks_are_chunked_and_nonmatching_events_are_not_researched() {
        let conn = db();
        let timed = Rule {
            query: "subject:missing".into(),
            ..rule(RuleTrigger::Age, 100)
        };
        set_rules(&conn, vec![timed.clone()]).unwrap();
        for uid in 1..=1001 {
            mail(&conn, uid, false, 100);
        }
        assert!(due(&conn, 1000).is_empty());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM rule_evaluations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1001);
        let changes = conn.total_changes();
        assert!(due(&conn, 2000).is_empty());
        assert_eq!(conn.total_changes(), changes);
        // Changing the search still evaluates existing aged mail.
        set_rules(
            &conn,
            vec![Rule {
                query: "subject:login".into(),
                ..timed
            }],
        )
        .unwrap();
        assert_eq!(due(&conn, 2000).len(), 1001);
    }

    #[test]
    fn mutable_conditions_are_rechecked_after_the_deadline() {
        for trigger in [RuleTrigger::Arrival, RuleTrigger::Age] {
            for query in ["is:read", "is:starred", "tag:ready", "has:attachment"] {
                let conn = db();
                set_rules(
                    &conn,
                    vec![Rule {
                        query: query.into(),
                        ..rule(trigger, 300)
                    }],
                )
                .unwrap();
                mail(&conn, 1, false, 100);
                apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
                conn.execute(
                    "UPDATE rule_events SET arrived_at = 1000 WHERE arrived_at IS NOT NULL",
                    [],
                )
                .unwrap();
                assert!(due(&conn, 2000).is_empty(), "{trigger:?} {query}");
                let changes = conn.total_changes();
                assert!(due(&conn, 2100).is_empty());
                assert_eq!(
                    conn.total_changes(),
                    changes,
                    "unchanged {trigger:?} {query}"
                );
                match query {
                    "is:read" => {
                        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap()
                    }
                    "is:starred" => {
                        crate::store::update_message_starred(&conn, "a", "INBOX", 1, true).unwrap()
                    }
                    "tag:ready" => crate::store::change_tags(
                        &conn,
                        "a",
                        &["1@example.com".into()],
                        &["ready".into()],
                        &[],
                        false,
                    )
                    .unwrap(),
                    _ => {
                        conn.execute("UPDATE messages SET files = '[{}]' WHERE uid = 1", [])
                            .unwrap();
                    }
                }
                assert_eq!(due(&conn, 2000).len(), 1, "{trigger:?} {query}");
            }
        }
    }

    #[test]
    fn tag_changes_invalidate_all_copies_but_timestamp_refreshes_keep_the_cache() {
        let conn = db();
        set_rules(
            &conn,
            vec![Rule {
                query: "tag:ready".into(),
                ..rule(RuleTrigger::Read, 300)
            }],
        )
        .unwrap();
        mail(&conn, 1, false, 100);
        crate::store::upsert_messages(
            &conn,
            "a",
            "Saved",
            &[MessageHeader {
                uid: 9,
                message_id: "<1@example.com>".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        for (folder, uid) in [("INBOX", 1), ("Saved", 9)] {
            crate::store::update_message_seen(&conn, "a", folder, uid, true).unwrap();
        }
        conn.execute("UPDATE rule_events SET read_at = 1000", [])
            .unwrap();
        let evaluate = || {
            apply_due_rules(&conn, "a", "INBOX", 7, 2000).unwrap();
            apply_due_rules(&conn, "a", "Saved", 7, 2000).unwrap();
        };
        let cached = || {
            conn.query_row("SELECT COUNT(*) FROM rule_evaluations", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
        };
        evaluate();
        assert_eq!(cached(), 2);
        conn.execute("INSERT INTO message_tags(account, tag_key, tag, state) VALUES ('a', '1@example.com', 'ready', 2)", []).unwrap();
        assert_eq!(cached(), 0);
        evaluate();
        assert_eq!(cached(), 2);
        conn.execute("UPDATE message_tags SET updated_at = 1", [])
            .unwrap();
        let changes = conn.total_changes();
        evaluate();
        assert_eq!(conn.total_changes(), changes);
        conn.execute("UPDATE message_tags SET state = 0", [])
            .unwrap();
        assert_eq!(cached(), 0);
        evaluate();
        assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 1);
        assert_eq!(pending_rule_actions(&conn, "a", "Saved").unwrap().len(), 1);
        conn.execute("DELETE FROM message_tags", []).unwrap();
        assert_eq!(cached(), 0);
        let key = crate::store::tags::TAG_KEY_SQL;
        let plan: String = conn.query_row(&format!(
            "EXPLAIN QUERY PLAN SELECT id FROM messages WHERE uid <> 0 AND account = 'a' AND {key} = '1@example.com'"
        ), [], |r| r.get(3)).unwrap();
        assert!(plan.contains("messages_tag_key_idx"), "{plan}");
    }

    #[test]
    fn header_corrections_invalidate_negative_checks() {
        let conn = db();
        set_rules(
            &conn,
            vec![Rule {
                query: "subject:corrected".into(),
                ..rule(RuleTrigger::Age, 300)
            }],
        )
        .unwrap();
        mail(&conn, 1, false, 100);
        assert!(due(&conn, 2000).is_empty());
        conn.execute(
            "UPDATE messages SET subject = 'corrected' WHERE uid = 1",
            [],
        )
        .unwrap();
        assert_eq!(due(&conn, 2000).len(), 1);
    }

    #[test]
    fn reenabled_event_rules_ignore_events_from_while_disabled() {
        for trigger in [RuleTrigger::Read, RuleTrigger::Arrival] {
            let conn = db();
            let mut timed = rule(trigger, 300);
            set_rules(&conn, vec![timed.clone()]).unwrap();
            timed.disabled = true;
            set_rules(&conn, vec![timed.clone()]).unwrap();
            mail(&conn, 1, false, 100);
            apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
            crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
            timed.disabled = false;
            set_rules(&conn, vec![timed]).unwrap();
            conn.execute(
                "UPDATE rule_events SET arrived_at = 1000, read_at = 1000",
                [],
            )
            .unwrap();
            assert!(due(&conn, 2000).is_empty());
            mail(&conn, 2, false, 100);
            apply_rules(&conn, "a", "INBOX", 7, &[2]).unwrap();
            crate::store::update_message_seen(&conn, "a", "INBOX", 2, true).unwrap();
            conn.execute(
                "UPDATE rule_events SET arrived_at = 1000, read_at = 1000",
                [],
            )
            .unwrap();
            let actions = due(&conn, 2000);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].uid, 2);
        }
    }

    #[test]
    fn expanding_account_scope_does_not_replay_past_events() {
        let conn = db();
        let mut timed = Rule {
            accounts: vec!["other".into()],
            ..rule(RuleTrigger::Read, 300)
        };
        set_rules(&conn, vec![timed.clone()]).unwrap();
        mail(&conn, 1, false, 100);
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        timed.accounts.clear();
        set_rules(&conn, vec![timed]).unwrap();
        conn.execute("UPDATE rule_events SET read_at = 1000", [])
            .unwrap();
        assert!(due(&conn, 2000).is_empty());
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, false).unwrap();
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        conn.execute("UPDATE rule_events SET read_at = 1000", [])
            .unwrap();
        assert_eq!(due(&conn, 2000).len(), 1);
    }

    #[test]
    fn scheduled_actions_preserve_existing_automatic_work() {
        let conn = db();
        let immediate = Rule {
            id: "immediate".into(),
            mark_read: true,
            ..Default::default()
        };
        set_rules(&conn, vec![immediate, rule(RuleTrigger::Age, 100)]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        let actions = due(&conn, 1000);
        assert_eq!(actions.len(), 2);
        assert!(actions.iter().any(|a| a.mark_read && a.run == 0));
        assert!(actions.iter().any(|a| a.star && a.run < 0));
        assert!(running_rules(&conn).unwrap().is_empty());
        stop_rule(&conn, "timed").unwrap();
        assert_eq!(pending_rule_actions(&conn, "a", "INBOX").unwrap().len(), 2);
    }
}
