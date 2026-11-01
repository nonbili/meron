//! Rules: what to do with new mail that matches a search.

use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::tags::{message_tag_keys, write_tag_changes};

const RULES_KEY: &str = "rules";

/// Tries a queued action gets before it is given up on, so a folder deleted
/// since the rule ran can't fail on every sync forever.
const MAX_RULE_ACTION_ATTEMPTS: i64 = 5;

/// One search and everything to do with the new mail it matches. Rules live in
/// `settings`, so backups carry them, and are kept per device and shared by every
/// account: on an account without the rule's folder, the move is skipped.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: String,
    /// In the search box's syntax, read against what a new message's envelope
    /// says: its body is not here yet.
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
}

impl Rule {
    fn has_action(&self) -> bool {
        !self.tags.is_empty() || self.mark_read || self.star || !self.move_to.is_empty()
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
            !crate::search_query::SearchQuery::parse(&rule.query).is_empty(),
            "a rule needs a search"
        );
        let tags = serde_json::json!(rule.tags);
        rule.tags = crate::tags::parse_tag_list(Some(&tags)).map_err(anyhow::Error::msg)?;
        rule.folder = rule.folder.trim().to_string();
        match rule.move_to.as_str() {
            "" | "archive" | "trash" => rule.folder.clear(),
            "folder" => anyhow::ensure!(!rule.folder.is_empty(), "a rule needs a folder"),
            other => anyhow::bail!("unknown rule move: {other}"),
        }
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
}

impl RuleAction {
    fn moves(&self) -> bool {
        self.trash || !self.target.is_empty()
    }
}

/// Where `rule` sends mail in `folder`, as `(trash, target)`: a message already
/// there has nowhere to go, and neither does one bound for a folder this account
/// lacks.
fn rule_move(
    conn: &Connection,
    account: &str,
    folder: &str,
    rule: &Rule,
) -> Result<Option<(bool, String)>> {
    let target = match rule.move_to.as_str() {
        "trash" => return Ok(Some((true, String::new()))),
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
    Ok((target != folder).then_some((false, target)))
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
        let matched = super::matching_uids(&tx, account, folder, &rule.query, uids)?;
        if matched.is_empty() {
            continue;
        }
        if !rule.tags.is_empty() {
            let copies = BTreeMap::from([(folder.to_string(), matched.clone())]);
            let keys = message_tag_keys(&tx, account, &copies)?;
            write_tag_changes(&tx, account, &keys, &rule.tags, &[], true)?;
        }
        let destination = rule_move(&tx, account, folder, &rule)?;
        for uid in matched {
            let action = actions.entry(uid).or_insert_with(|| RuleAction {
                folder: folder.to_string(),
                uid,
                uidvalidity,
                ..RuleAction::default()
            });
            action.mark_read |= rule.mark_read;
            action.star |= rule.star;
            if let Some((trash, target)) = destination.as_ref().filter(|_| !action.moves()) {
                action.trash = *trash;
                action.target = target.clone();
            }
        }
    }
    for action in actions.values() {
        if !action.mark_read && !action.star && !action.moves() {
            continue;
        }
        tx.execute(
            "INSERT OR REPLACE INTO rule_actions
               (account, folder, uid, uidvalidity, mark_read, star, trash, target, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![
                account,
                action.folder,
                action.uid,
                action.uidvalidity,
                action.mark_read,
                action.star,
                action.trash,
                action.target
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn pending_rule_actions(
    conn: &Connection,
    account: &str,
    folder: &str,
) -> Result<Vec<RuleAction>> {
    let mut stmt = conn.prepare(
        "SELECT folder, uid, uidvalidity, mark_read, star, trash, target
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
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn finish_rule_actions(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<()> {
    for uid in uids {
        conn.execute(
            "DELETE FROM rule_actions WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
        )?;
    }
    Ok(())
}

/// Count a failed try, dropping what has now had its last one. Answers with how
/// many were dropped.
pub fn retry_rule_actions(
    conn: &Connection,
    account: &str,
    folder: &str,
    uids: &[u32],
) -> Result<usize> {
    let mut dropped = 0;
    for uid in uids {
        conn.execute(
            "UPDATE rule_actions SET attempts = attempts + 1
             WHERE account = ?1 AND folder = ?2 AND uid = ?3",
            params![account, folder, uid],
        )?;
        dropped += conn.execute(
            "DELETE FROM rule_actions
             WHERE account = ?1 AND folder = ?2 AND uid = ?3 AND attempts >= ?4",
            params![account, folder, uid, MAX_RULE_ACTION_ATTEMPTS],
        )?;
    }
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
