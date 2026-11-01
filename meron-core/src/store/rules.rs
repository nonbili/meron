//! Rules: what to do with new mail that matches a search.

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
pub fn set_rules(conn: &Connection, rules: Vec<Rule>) -> Result<Vec<Rule>> {
    let mut clean = Vec::with_capacity(rules.len());
    for mut rule in rules {
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
    super::setting_set(conn, RULES_KEY, &serde_json::json!(clean))?;
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
    /// Which run by hand queued this, 0 for new mail. Marks whose work a finished
    /// step was: one still in flight when its run is stopped must not touch what
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
    let rules = rules(&tx)?;
    if rules.is_empty() {
        return Ok(());
    }
    let role = super::folder_role(&tx, account, folder)?;
    if matches!(role.as_str(), "sent" | "drafts" | "trash" | "junk") {
        return Ok(());
    }
    let mut actions: BTreeMap<u32, RuleAction> = BTreeMap::new();
    for rule in rules {
        if rule.disabled
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
        conn.prepare("SELECT rule_ids, COUNT(*) FROM rule_actions WHERE run <> 0 GROUP BY 1")?;
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
        "DELETE FROM rule_actions WHERE run <> 0 AND rule_ids = ?1",
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
