//! Rules: actions on matching mail after arrival, reading, or a given age.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::tags::{message_tag_keys, write_tag_changes};
use crate::search_query::{SearchFields, SearchQuery};

const RULES_KEY: &str = "rules";

/// Tries a queued action gets before it is given up on, so a folder deleted
/// since the rule ran can't fail on every sync forever.
const MAX_RULE_ACTION_ATTEMPTS: i64 = 5;

/// Entries the rule log keeps.
const RULE_LOG_SIZE: i64 = 200;

/// Some conditions and everything to do with the new mail they match. Rules
/// live in `settings`, so backups carry them, and are kept per device: they
/// run on the accounts in `accounts`, or every account when that is empty.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub id: String,
    /// Read against what a new message's envelope says: its body is not here
    /// yet. None for all new mail.
    #[serde(default)]
    pub conditions: Vec<RuleCondition>,
    /// Whether any one of `conditions` is enough, rather than all of them.
    #[serde(default)]
    pub match_any: bool,
    /// What rules kept before they had `conditions`: a search, in the search
    /// box's syntax. Read into `conditions` whenever a rule is, so it is only
    /// ever set on the way in. The store writes [`Rule::compat_query`] here
    /// for older releases reading it, or a backup of it.
    #[serde(default, skip_serializing)]
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
    /// Addresses to forward matching mail to, comma-separated as a composer's
    /// field; `""` for none. The account the mail is in sends the forward.
    #[serde(default)]
    pub forward_to: String,
    /// Kept, but not run.
    #[serde(default)]
    pub disabled: bool,
    /// Runs on mail the user sent, as its copy shows up in the Sent folder,
    /// instead of on mail that arrives. Such a rule runs right away and leaves
    /// the copy where it is: it can tag, star and copy it, but not forward it.
    #[serde(default)]
    pub sent: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleTrigger {
    #[default]
    Arrival,
    Read,
    Age,
}

/// One thing a message has to say, or not say, for a rule to take it.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RuleCondition {
    /// `from`, `to` and `subject` take comma-separated alternatives, any of
    /// which matches; `body` and `subject_or_body` one piece of text, which
    /// waits for the body to be downloaded; `words` is search text, what a
    /// rule from before conditions couldn't say as rows; `tag` one tag;
    /// `attachment`, `unread` and `starred` take no value.
    pub field: String,
    #[serde(default)]
    pub value: String,
    /// Matches the mail the condition otherwise wouldn't.
    #[serde(default)]
    pub negate: bool,
}

impl RuleCondition {
    fn new(field: &str, value: impl Into<String>) -> Self {
        RuleCondition {
            field: field.to_string(),
            value: value.into(),
            negate: false,
        }
    }

    /// Whether this reads the message body, which arrives after the headers.
    fn reads_body(&self) -> bool {
        matches!(self.field.as_str(), "body" | "subject_or_body")
    }

    /// The search that answers this condition, before any negation. Empty
    /// for one that [reads the body](Self::reads_body), which no search does.
    fn query(&self) -> String {
        // Search text as written: the form would re-space it.
        if self.field == "words" {
            return self.value.clone();
        }
        let mut fields = SearchFields::default();
        let value = self.value.clone();
        match self.field.as_str() {
            "from" => fields.from = value,
            "to" => fields.to = value,
            "subject" => fields.subject = value,
            "words" => fields.words = value,
            "tag" => fields.tags = value,
            "attachment" => fields.has_attachment = true,
            "unread" => fields.unread = true,
            "starred" => fields.starred = true,
            _ => {}
        }
        fields.to_query()
    }

    /// Tidied, or why it could never apply.
    fn clean(mut self) -> Result<Self> {
        self.value = self.value.trim().to_string();
        match self.field.as_str() {
            "from" | "to" | "subject" | "words" => {}
            "body" | "subject_or_body" => {
                anyhow::ensure!(!self.value.is_empty(), "a rule's body condition needs text");
                return Ok(self);
            }
            "tag" => {
                self.value = crate::tags::normalize_tag(&self.value)
                    .ok_or_else(|| anyhow::anyhow!("a rule's tag condition needs a tag"))?;
            }
            "attachment" | "unread" | "starred" => self.value.clear(),
            other => anyhow::bail!("unknown rule condition: {other}"),
        }
        anyhow::ensure!(
            !SearchQuery::parse(&self.query()).is_empty(),
            "a rule's condition means nothing"
        );
        Ok(self)
    }
}

/// A search, as conditions that all have to hold: split into a row per field
/// where that searches the same, else whole as words.
fn conditions_from_query(query: &str) -> Vec<RuleCondition> {
    // What reads as nothing on its own, as a dangling `AND`, was nothing.
    let split: Vec<RuleCondition> = split_query(query)
        .into_iter()
        .filter(|condition| !SearchQuery::parse(&condition.query()).is_empty())
        .collect();
    // A search that read as nothing at all matched nothing; no rows would be
    // all mail.
    if split.is_empty() {
        return vec![RuleCondition::new("words", query)];
    }
    let rejoined = split
        .iter()
        .map(RuleCondition::query)
        .collect::<Vec<_>>()
        .join(" AND ");
    if SearchQuery::parse(&rejoined) == SearchQuery::parse(query) {
        split
    } else {
        vec![RuleCondition::new("words", query)]
    }
}

fn split_query(query: &str) -> Vec<RuleCondition> {
    let fields = SearchFields::parse(query);
    let mut conditions = Vec::new();
    for (field, value) in [
        ("from", &fields.from),
        ("to", &fields.to),
        ("subject", &fields.subject),
    ] {
        if !value.is_empty() {
            conditions.push(RuleCondition::new(field, value.as_str()));
        }
    }
    conditions.extend(
        fields
            .tags
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|tag| !tag.is_empty())
            .map(|tag| RuleCondition::new("tag", tag)),
    );
    for (field, set) in [
        ("attachment", fields.has_attachment),
        ("unread", fields.unread),
        ("starred", fields.starred),
    ] {
        if set {
            conditions.push(RuleCondition::new(field, ""));
        }
    }
    // Dates have no condition of their own: they stay search text.
    let dates = SearchFields {
        after: fields.after,
        before: fields.before,
        ..SearchFields::default()
    }
    .to_query();
    let words = [dates.as_str(), fields.words.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !words.is_empty() {
        conditions.push(RuleCondition::new("words", words));
    }
    conditions
}

impl Rule {
    /// Read a rule from before conditions: its search becomes them.
    fn upgrade(mut self) -> Self {
        let query = std::mem::take(&mut self.query);
        if self.conditions.is_empty() && !query.trim().is_empty() {
            self.conditions = conditions_from_query(query.trim());
            self.match_any = false;
        }
        self
    }

    /// The search an older release, which reads only `query`, runs for this
    /// rule: the same one, or one that matches nothing when it can't say it,
    /// so a rule never acts on more mail there than here.
    fn compat_query(&self) -> String {
        if self
            .conditions
            .iter()
            .any(|condition| condition.negate || condition.reads_body())
            || (self.match_any && self.conditions.len() > 1)
        {
            return "is:read AND is:unread".to_string();
        }
        let queries: Vec<String> = self.conditions.iter().map(RuleCondition::query).collect();
        let joined = queries.join(" AND ");
        // A row's open quote or group would run on into the next one there; a
        // newline here ends it, as the rows are read apart.
        if SearchQuery::parse(&joined) != SearchQuery::parse(&queries.join("\nAND ")) {
            return "is:read AND is:unread".to_string();
        }
        joined
    }

    /// Whether a condition reads the body: then the rule waits for it.
    fn reads_body(&self) -> bool {
        self.conditions.iter().any(RuleCondition::reads_body)
    }

    /// Whether this acts right away on mail that arrives.
    fn on_arrival(&self) -> bool {
        !self.sent && self.trigger == RuleTrigger::Arrival && self.delay_seconds == 0
    }

    /// Whether this waits for the body: when it reads it, or acts right away
    /// on an account where another rule acting right away does (`held`).
    fn waits_for_body(&self, held: bool) -> bool {
        self.reads_body() || (held && self.on_arrival())
    }
}

/// Whether the rules acting right away on mail arriving in `account` wait for
/// its body: they do once one of them reads it, so that they still run
/// together, in their order. The first to move a message decides where it
/// goes, and two forwarding to one address send one forward.
fn arrivals_held(rules: &[Rule], account: &str) -> bool {
    rules.iter().any(|rule| {
        !rule.disabled
            && rule.on_arrival()
            && rule.reads_body()
            && (rule.accounts.is_empty() || rule.accounts.iter().any(|id| id == account))
    })
}

/// What decides which accounts' arrivals are [held](arrivals_held): when it
/// changes, so does when the rules acting right away run there.
fn arrival_holds(rules: &[Rule]) -> Vec<Vec<String>> {
    let mut holds: Vec<Vec<String>> = rules
        .iter()
        .filter(|rule| !rule.disabled && rule.on_arrival() && rule.reads_body())
        .map(|rule| rule.accounts.clone())
        .collect();
    holds.sort();
    holds.dedup();
    holds
}

impl Rule {
    /// What a cached nonmatch was checked against: it holds while this does.
    fn condition_key(&self) -> String {
        serde_json::json!([self.match_any, self.conditions]).to_string()
    }

    fn has_action(&self) -> bool {
        !self.tags.is_empty()
            || self.mark_read
            || self.star
            || !self.move_to.is_empty()
            || !self.copy_folder.is_empty()
            || !self.forward_to.is_empty()
    }
}

pub fn rules(conn: &Connection) -> Result<Vec<Rule>> {
    let stored = super::settings_get(conn, &[RULES_KEY.to_string()])?;
    let rules: Vec<Rule> = stored
        .get(RULES_KEY)
        .and_then(|rules| serde_json::from_value(rules.clone()).ok())
        .unwrap_or_default();
    Ok(rules.into_iter().map(Rule::upgrade).collect())
}

/// `rules` as the store keeps them: each with the search an older release
/// would run for it, as that is all such a release, or a backup restored
/// there, reads.
fn stored_rules(rules: &[Rule]) -> serde_json::Value {
    serde_json::Value::Array(
        rules
            .iter()
            .map(|rule| {
                let mut value = serde_json::json!(rule);
                value["query"] = rule.compat_query().into();
                value
            })
            .collect(),
    )
}

/// Which of `uids` in `folder` match `rule`'s conditions, in `uids`' order.
fn rule_matching_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    rule: &Rule,
    uids: &[u32],
) -> Result<Vec<u32>> {
    if rule.conditions.is_empty() {
        return Ok(uids.to_vec());
    }
    // Each condition looks only at what is still open: for all, what every
    // one before matched; for any, what none of them has yet.
    let mut open: Vec<u32> = uids.to_vec();
    // A search finds only cached mail, so "not" would take a message that is
    // gone: only those still here are looked at. And a rule that reads the
    // body looks only at mail whose body is here.
    let reads_body = rule.reads_body();
    if reads_body || rule.conditions.iter().any(|condition| condition.negate) {
        let list = uids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let body = if reads_body {
            format!(" AND {}", super::RULE_BODY_READABLE)
        } else {
            String::new()
        };
        let mut stmt = conn.prepare(&format!(
            "SELECT m.uid FROM messages m
             WHERE m.account = ?1 AND m.folder = ?2 AND m.uid IN ({list}){body}"
        ))?;
        let cached = stmt
            .query_map(params![account, folder], |row| row.get::<_, u32>(0))?
            .collect::<rusqlite::Result<BTreeSet<u32>>>()?;
        open.retain(|uid| cached.contains(uid));
    }
    let mut matched: BTreeSet<u32> = BTreeSet::new();
    for condition in &rule.conditions {
        if open.is_empty() {
            break;
        }
        let hits: BTreeSet<u32> = if condition.reads_body() {
            super::body_matching_uids(
                conn,
                account,
                folder,
                &condition.value,
                condition.field == "subject_or_body",
                &open,
            )?
        } else {
            super::matching_uids(conn, account, folder, &condition.query(), &open)?
        }
        .into_iter()
        .collect();
        let (these, rest): (Vec<u32>, Vec<u32>) = std::mem::take(&mut open)
            .into_iter()
            .partition(|uid| hits.contains(uid) != condition.negate);
        if rule.match_any {
            matched.extend(these);
            open = rest;
        } else {
            open = these;
        }
    }
    if !rule.match_any {
        matched.extend(open);
    }
    Ok(uids
        .iter()
        .copied()
        .filter(|uid| matched.contains(uid))
        .collect())
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
    for rule in incoming {
        let query = rule.query.trim().to_string();
        let mut rule = rule.upgrade();
        anyhow::ensure!(
            query.is_empty() || !SearchQuery::parse(&query).is_empty(),
            "a rule's search means nothing"
        );
        rule.conditions = rule
            .conditions
            .into_iter()
            .map(RuleCondition::clean)
            .collect::<Result<_>>()?;
        if rule.conditions.len() < 2 {
            rule.match_any = false;
        }
        anyhow::ensure!(
            (0..=315_360_000).contains(&rule.delay_seconds),
            "a rule's delay must be between zero and ten years"
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
        anyhow::ensure!(
            !rule.sent || (rule.trigger == RuleTrigger::Arrival && rule.delay_seconds == 0),
            "a rule for sent mail runs right away"
        );
        rule.forward_to = crate::smtp::normalize_recipients(&rule.forward_to)
            .map_err(|err| anyhow::anyhow!("a rule's forward: {err}"))?;
        anyhow::ensure!(
            !rule.sent
                || (!rule.mark_read && rule.move_to.is_empty() && rule.forward_to.is_empty()),
            "a rule for sent mail can't move it, mark it read or forward it"
        );
        anyhow::ensure!(
            !rule.sent || !rule.reads_body(),
            "a rule for sent mail can't check the body"
        );
        anyhow::ensure!(rule.has_action(), "a rule needs an action");
        if rule.id.is_empty() {
            rule.id = uuid::Uuid::new_v4().to_string();
        }
        clean.push(rule);
    }
    // Rules acting right away that start or stop waiting for the body start
    // afresh, as a delayed rule does, rather than take up events from before.
    let holds_changed = arrival_holds(&previous) != arrival_holds(&clean);
    for rule in &clean {
        let timing_unchanged = !(holds_changed && rule.on_arrival())
            && previous.iter().any(|old| {
                old.id == rule.id
                && old.trigger == rule.trigger
                // One that reads the body waits for it, as a delayed rule does,
                // from events since it started to.
                && old.reads_body() == rule.reads_body()
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
    let stored = stored_rules(&clean);
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
    /// Addresses to forward to, before any move. Emptied once the forward is
    /// out, as a forward can't be asked after, the way a copy's target is.
    pub forwards: Vec<String>,
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
/// Mail the user wrote or threw away has not arrived in that sense: what they
/// sent has rules of its own, in [`apply_sent_rules`].
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
        // Keep the arrival clocks recorded above.
        tx.commit()?;
        return Ok(());
    }
    run_immediate_rules(&tx, account, folder, uidvalidity, uids, rules, false)?;
    tx.commit()?;
    Ok(())
}

/// Run the rules for sent mail over copies that have just shown up in
/// `folder`, the account's Sent folder: see [`new_sent_uids`]. The caller
/// vouches for the folder, which a provider may name anything.
pub fn apply_sent_rules(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uids: &[u32],
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let rules = rules(&tx)?;
    run_immediate_rules(&tx, account, folder, uidvalidity, uids, rules, true)?;
    tx.commit()?;
    Ok(())
}

/// Whether any rule for sent mail would run on `account`.
pub fn has_sent_rules(conn: &Connection, account: &str) -> Result<bool> {
    Ok(rules(conn)?.iter().any(|rule| {
        rule.sent
            && !rule.disabled
            && (rule.accounts.is_empty() || rule.accounts.iter().any(|item| item == account))
    }))
}

/// The mail in `messages` that `folder` gained since it was last read: what a
/// Sent folder has to show for the user sending something, from here or from
/// another client. Call it before the read is stored.
///
/// Not [`super::first_seen_messages`]: the copy of mail the user sent to
/// themselves, or the one Gmail also lists under All Mail, may already have
/// been seen elsewhere on the account, and is no less sent for it. Nothing on
/// a first read, when all of the folder is new to the store.
pub fn new_sent_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uid_next: u32,
    messages: &[crate::imap::MessageHeader],
) -> Result<Vec<u32>> {
    let Some((validity, before)) = super::get_folder_state(conn, account, folder)? else {
        return Ok(Vec::new());
    };
    if validity != uidvalidity || before == 0 {
        return Ok(Vec::new());
    }
    Ok(messages
        .iter()
        .map(|message| message.uid)
        .filter(|uid| (before..uid_next).contains(uid))
        .collect())
}

/// The rules that act at once, for received mail or for `sent`, over `uids`.
fn run_immediate_rules(
    tx: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    uids: &[u32],
    rules: Vec<Rule>,
    sent: bool,
) -> Result<()> {
    let mut actions: BTreeMap<u32, RuleAction> = BTreeMap::new();
    let held = arrivals_held(&rules, account);
    for rule in rules {
        if rule.sent != sent
            || rule.trigger != RuleTrigger::Arrival
            || rule.delay_seconds != 0
            // Waits for the body: see [`apply_due_rules`].
            || (!sent && rule.waits_for_body(held))
            || rule.disabled
            || (!rule.accounts.is_empty() && !rule.accounts.iter().any(|item| item == account))
        {
            continue;
        }
        let matched = rule_matching_uids(tx, account, folder, &rule, uids)?;
        for uid in &matched {
            let subject = rule_message_subject(tx, account, folder, *uid)?;
            log_rule(tx, &rule.id, account, &subject, 1, None)?;
        }
        add_rule_effects(
            tx,
            account,
            folder,
            uidvalidity,
            &rule,
            matched,
            &mut actions,
        )?;
    }
    queue_rule_actions(tx, account, &actions, 0)?;
    trim_rule_log(tx)?;
    Ok(())
}

/// Check persistent event clocks during sync. Delayed work is only queued when
/// due, so edits, disabling a rule, or marking mail unread take effect first.
/// Each rule fires once per cached message; manual runs remain independent.
/// Whether `rule` is one [`apply_due_rules`] runs in `account`'s `folder`:
/// a delayed one, or one that waits for the body, as all acting right away do
/// where arrivals are `held`.
fn due_rule_applies(rule: &Rule, account: &str, folder: &str, held: bool) -> bool {
    !(rule.disabled
        || rule.sent
        || (rule.trigger == RuleTrigger::Arrival
            && rule.delay_seconds == 0
            && !rule.waits_for_body(held))
        || (!rule.accounts.is_empty() && !rule.accounts.iter().any(|id| id == account))
        || (rule.trigger == RuleTrigger::Age && folder != EXISTING_RULE_FOLDER))
}

/// The mail in `folder` that `rule` is due to check by `now`, each with the
/// sequence of its event, and not checked against these conditions for that
/// event yet. With `body`, only mail whose body a rule can read (`true`), or
/// that isn't downloaded (`false`).
fn due_candidates(
    conn: &Connection,
    rule: &Rule,
    account: &str,
    folder: &str,
    now: i64,
    body: Option<bool>,
) -> Result<Vec<(u32, i64)>> {
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
    let body = match body {
        None => String::new(),
        Some(true) => format!(" AND {}", super::RULE_BODY_READABLE),
        Some(false) => format!(" AND {} IS NULL", super::RULE_BODY_TEXT),
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT m.uid, {event_sequence} FROM messages m JOIN rule_events e ON e.message = m.id
         LEFT JOIN rule_activation a ON a.rule_id = ?4
         WHERE m.account = ?1 AND m.folder = ?2 AND m.uid <> 0
           AND {clock} <= ?3 AND ({eligible}){body}
           AND NOT EXISTS (SELECT 1 FROM rule_firings f WHERE f.message = m.id AND f.rule_id = ?4)
           AND NOT EXISTS (SELECT 1 FROM rule_evaluations v
             WHERE v.message = m.id AND v.rule_id = ?4 AND v.event_sequence = {event_sequence} AND v.query = ?5)
         ORDER BY m.date DESC, m.uid DESC"
    ))?;
    Ok(stmt
        .query_map(
            params![
                account,
                folder,
                now - rule.delay_seconds,
                rule.id,
                rule.condition_key()
            ],
            |row| Ok((row.get::<_, u32>(0)?, row.get::<_, i64>(1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Mail in `folder`, newest first and at most `limit`, that a rule due to
/// check it waits on the body of, for the engine to download. With
/// `arrivals`, only for rules that act as mail arrives.
pub fn rule_body_waiting_uids(
    conn: &Connection,
    account: &str,
    folder: &str,
    now: i64,
    limit: usize,
    arrivals: bool,
) -> Result<Vec<u32>> {
    let mut waiting = BTreeSet::new();
    let rules = rules(conn)?;
    let held = arrivals_held(&rules, account);
    for rule in rules {
        if rule.waits_for_body(held)
            && due_rule_applies(&rule, account, folder, held)
            && (rule.on_arrival() || !arrivals)
        {
            waiting.extend(
                due_candidates(conn, &rule, account, folder, now, Some(false))?
                    .into_iter()
                    .map(|(uid, _)| uid),
            );
        }
    }
    if waiting.is_empty() {
        return Ok(Vec::new());
    }
    let role = super::folder_role(conn, account, folder)?;
    if matches!(role.as_str(), "sent" | "drafts" | "trash" | "junk") {
        return Ok(Vec::new());
    }
    // UIDs grow with arrival: the newest are the highest.
    Ok(waiting.into_iter().rev().take(limit).collect())
}

pub fn apply_due_rules(
    conn: &Connection,
    account: &str,
    folder: &str,
    uidvalidity: u32,
    now: i64,
) -> Result<()> {
    let held = arrivals_held(&rules(conn)?, account);
    let applies = |rule: &Rule| due_rule_applies(rule, account, folder, held);
    // Runs after every folder sync: don't take the write lock without a
    // delayed rule. The rules are read again under the lock below.
    if !rules(conn)?.iter().any(applies) {
        return Ok(());
    }
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let role = super::folder_role(&tx, account, folder)?;
    if matches!(role.as_str(), "sent" | "drafts" | "trash" | "junk") {
        return Ok(());
    }
    let mut actions = BTreeMap::new();
    for rule in rules(&tx)? {
        if !applies(&rule) {
            continue;
        }
        let key = rule.condition_key();
        // Mail whose body is still on its way waits: it is no nonmatch yet.
        let body = rule.waits_for_body(held).then_some(true);
        let candidates = due_candidates(&tx, &rule, account, folder, now, body)?;
        let mut matched = Vec::new();
        // Bound the search's UID list, and remember checks that did not match.
        // Input changes invalidate those checks through the store's triggers.
        for chunk in candidates.chunks(500) {
            let uids: Vec<u32> = chunk.iter().map(|(uid, _)| *uid).collect();
            matched.extend(rule_matching_uids(&tx, account, folder, &rule, &uids)?);
            for (uid, sequence) in chunk {
                // A rule acting on arrival checks a message once, as it
                // arrives: what changes about it later is not news to it.
                if rule.on_arrival() {
                    tx.execute(
                        "INSERT OR IGNORE INTO rule_firings (message, rule_id)
                         SELECT id, ?4 FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                        params![account, folder, uid, rule.id],
                    )?;
                    continue;
                }
                tx.execute(
                    "INSERT OR REPLACE INTO rule_evaluations (message, rule_id, event_sequence, query)
                     SELECT id, ?4, ?5, ?6 FROM messages WHERE account = ?1 AND folder = ?2 AND uid = ?3",
                    params![account, folder, uid, rule.id, sequence, key],
                )?;
            }
        }
        for uid in &matched {
            tx.execute(
                "INSERT OR IGNORE INTO rule_firings (message, rule_id)
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
    super::setting_set(conn, RULES_KEY, &stored_rules(&stored))?;
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
        action.forwards.extend(
            rule.forward_to
                .split(',')
                .map(str::trim)
                .filter(|addr| !addr.is_empty())
                .map(str::to_string),
        );
        action.forwards.sort_by_key(|addr| addr.to_lowercase());
        action.forwards.dedup_by_key(|addr| addr.to_lowercase());
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
        if !action.mark_read
            && !action.star
            && !action.moves()
            && copies.is_empty()
            && action.forwards.is_empty()
        {
            continue;
        }
        conn.execute(
            "INSERT OR REPLACE INTO rule_actions
               (account, folder, uid, uidvalidity, mark_read, star, trash, target, copies,
                rule_ids, run, attempts, forwards)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12)",
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
                run,
                serde_json::to_string(&action.forwards)?
            ],
        )?;
    }
    Ok(())
}

/// Where running a rule by hand looks: the rest of a mailbox is mostly mail
/// already filed, on purpose.
pub const EXISTING_RULE_FOLDER: &str = "INBOX";

/// Where running `rule` by hand looks on `account`: the Inbox, or for a rule
/// for sent mail the Sent folder, if the account's folders are known.
fn existing_rule_folder(conn: &Connection, account: &str, rule: &Rule) -> Option<String> {
    if !rule.sent {
        return Some(EXISTING_RULE_FOLDER.to_string());
    }
    crate::engine::sent_folder_from_db(conn, account, EXISTING_RULE_FOLDER)
}

/// [`existing_rule_folder`] for a saved rule.
pub fn saved_rule_folder(conn: &Connection, id: &str, account: &str) -> Result<Option<String>> {
    Ok(existing_rule_folder(conn, account, &saved_rule(conn, id)?))
}

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

/// What `rule` matches among the mail cached in `account`'s `folder`.
fn existing_rule_matches(
    conn: &Connection,
    account: &str,
    folder: &str,
    rule: &Rule,
) -> Result<Vec<u32>> {
    let mut stmt =
        conn.prepare("SELECT uid FROM messages WHERE account = ?1 AND folder = ?2 AND uid <> 0")?;
    let uids = stmt
        .query_map(params![account, folder], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<u32>>>()?;
    rule_matching_uids(conn, account, folder, rule, &uids)
}

/// How many messages [`queue_existing_rule`] would look at.
pub fn count_existing_rule_matches(conn: &Connection, id: &str) -> Result<usize> {
    let rule = saved_rule(conn, id)?;
    let mut count = 0;
    for account in rule_accounts(conn, &rule)? {
        if let Some(folder) = existing_rule_folder(conn, &account, &rule) {
            count += existing_rule_matches(conn, &account, &folder, &rule)?.len();
        }
    }
    Ok(count)
}

/// How many messages a run of [`queue_existing_rule`] would skip, as a
/// condition of it reads the body and theirs isn't downloaded, or can't be
/// opened.
pub fn count_existing_rule_without_body(conn: &Connection, id: &str) -> Result<usize> {
    let rule = saved_rule(conn, id)?;
    if !rule.reads_body() {
        return Ok(0);
    }
    let mut count = 0;
    for account in rule_accounts(conn, &rule)? {
        if let Some(folder) = existing_rule_folder(conn, &account, &rule) {
            count += conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM messages m
                     WHERE m.account = ?1 AND m.folder = ?2 AND m.uid <> 0
                       AND NOT {}",
                    super::RULE_BODY_READABLE
                ),
                params![account, folder],
                |row| row.get::<_, i64>(0),
            )? as usize;
        }
    }
    Ok(count)
}

/// Run a saved rule over mail already in the Inbox, or in Sent for a rule for
/// sent mail, for
/// [`crate::engine::run_rule_on_existing`] to carry out. Answers with the
/// accounts and how many messages each queued. Mail this rule already has work
/// queued for keeps that work; other rules' work runs alongside, under its own
/// run. And [`crate::engine`] asks the target before each copy, so
/// running a rule twice doesn't double what it did.
pub fn queue_existing_rule(conn: &Connection, id: &str) -> Result<Vec<(String, usize)>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let rule = saved_rule(&tx, id)?;
    anyhow::ensure!(!rule.disabled, "the rule is turned off");
    let run = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_micros() as i64;
    let mut queued = Vec::new();
    for account in rule_accounts(&tx, &rule)? {
        let Some(folder) = existing_rule_folder(&tx, &account, &rule) else {
            continue;
        };
        let folder = folder.as_str();
        let Some((uidvalidity, _)) = super::get_folder_state(&tx, &account, folder)? else {
            continue;
        };
        let pending = pending_rule_actions(&tx, &account, folder)?;
        let matched: Vec<u32> = existing_rule_matches(&tx, &account, folder, &rule)?
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
        "SELECT folder, uid, uidvalidity, mark_read, star, trash, target, copies, rule_ids, run,
                forwards
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
            forwards: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
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

/// Note that `run`'s forward of `uid` is out, so retrying the rest of what it
/// queued doesn't send it again.
pub fn finish_rule_forward(
    conn: &Connection,
    account: &str,
    folder: &str,
    uid: u32,
    run: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE rule_actions SET forwards = '[]'
         WHERE account = ?1 AND folder = ?2 AND uid = ?3 AND run = ?4",
        params![account, folder, uid, run],
    )?;
    Ok(())
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
        // "Isn't tagged" would take all mail once the tag is gone: the rule
        // is turned off rather than left to act on far more than it said.
        if to.is_none()
            && rule.conditions.iter().any(|condition| {
                condition.negate
                    && SearchQuery::parse(&condition.query())
                        .tags
                        .iter()
                        .any(|item| item == tag)
            })
        {
            rule.disabled = true;
        }
        if let Some(to) = to {
            for condition in &mut rule.conditions {
                match condition.field.as_str() {
                    "tag" if condition.value == tag => condition.value = to.to_string(),
                    "words" => {
                        condition.value =
                            crate::search_query::rename_tag_terms(&condition.value, tag, to)
                    }
                    _ => {}
                }
            }
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
    fn forwards_are_queued_and_struck_off_once_out() {
        let conn = db();
        let forward = |id: &str, to: &str| Rule {
            id: id.into(),
            forward_to: to.into(),
            ..Default::default()
        };
        let saved = set_rules(
            &conn,
            vec![
                forward("one", "Dan Example <dan@example.org>; DAN@example.org"),
                Rule {
                    star: true,
                    ..forward("two", "eve@example.org, dan@example.org")
                },
            ],
        )
        .unwrap();
        assert_eq!(saved[0].forward_to, "dan@example.org");
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        let actions = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].forwards, ["dan@example.org", "eve@example.org"]);

        // What else the rules want done stays queued for a retry.
        finish_rule_forward(&conn, "a", "INBOX", 1, actions[0].run).unwrap();
        let actions = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert!(actions[0].forwards.is_empty());
        assert!(actions[0].star);
    }

    #[test]
    fn a_forward_needs_addresses_and_received_mail() {
        let conn = db();
        let forward = |to: &str, sent| Rule {
            forward_to: to.into(),
            sent,
            ..Default::default()
        };
        assert!(set_rules(&conn, vec![forward("not an address", false)]).is_err());
        assert!(set_rules(&conn, vec![forward("dan@example.org", true)]).is_err());
        assert!(set_rules(&conn, vec![forward("dan@example.org", false)]).is_ok());
    }

    #[test]
    fn arrivals_in_folders_rules_skip_are_still_recorded() {
        let conn = db();
        set_rules(&conn, vec![rule(RuleTrigger::Arrival, 0)]).unwrap();
        crate::store::upsert_messages(
            &conn,
            "a",
            "Trash",
            &[MessageHeader {
                uid: 1,
                message_id: "<1@example.com>".into(),
                ..Default::default()
            }],
        )
        .unwrap();
        assert_eq!(
            crate::store::folder_role(&conn, "a", "Trash").unwrap(),
            "trash"
        );
        apply_rules(&conn, "a", "Trash", 7, &[1]).unwrap();
        let arrived: Option<i64> = conn
            .query_row("SELECT arrived_at FROM rule_events", [], |r| r.get(0))
            .unwrap();
        assert!(arrived.is_some());
        assert!(
            pending_rule_actions(&conn, "a", "Trash")
                .unwrap()
                .is_empty()
        );
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

    fn body(conn: &Connection, uid: u32, text: &str) {
        conn.execute(
            "UPDATE messages SET body = ?2 WHERE account = 'a' AND folder = 'INBOX' AND uid = ?1",
            params![uid, text],
        )
        .unwrap();
    }

    fn body_rule(field: &str, value: &str, negate: bool) -> Rule {
        Rule {
            conditions: vec![RuleCondition {
                field: field.into(),
                value: value.into(),
                negate,
            }],
            ..rule(RuleTrigger::Arrival, 0)
        }
    }

    #[test]
    fn a_rule_reading_the_body_waits_for_it() {
        let conn = db();
        set_rules(
            &conn,
            vec![
                body_rule("body", "Invoice", false),
                Rule {
                    id: "not".into(),
                    ..body_rule("body", "invoice", true)
                },
            ],
        )
        .unwrap();
        mail(&conn, 1, false, 100);
        mail(&conn, 2, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1, 2]).unwrap();
        conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
            .unwrap();
        // Headers alone decide nothing, either way.
        assert!(due(&conn, 1000).is_empty());
        assert_eq!(
            rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 50, false).unwrap(),
            [2, 1]
        );
        assert_eq!(
            rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 1, true).unwrap(),
            [2]
        );

        body(&conn, 1, "please find the INVOICE attached");
        body(&conn, 2, "lunch?");
        let actions = due(&conn, 1000);
        let star = |uid| {
            let action = actions.iter().find(|action| action.uid == uid).unwrap();
            action.rule_ids.clone()
        };
        assert_eq!(star(1), ["timed"]);
        assert_eq!(star(2), ["not"]);
        assert!(
            rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 50, false)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn rules_acting_on_arrival_wait_together_once_one_reads_the_body() {
        let conn = db();
        let header = |id: &str| Rule {
            id: id.into(),
            conditions: vec![RuleCondition {
                field: "subject".into(),
                value: "login".into(),
                negate: false,
            }],
            star: false,
            ..rule(RuleTrigger::Arrival, 0)
        };
        set_rules(
            &conn,
            vec![
                Rule {
                    id: "body".into(),
                    star: false,
                    move_to: "trash".into(),
                    forward_to: "dan@example.org".into(),
                    ..body_rule("body", "code", false)
                },
                Rule {
                    move_to: "archive".into(),
                    forward_to: "dan@example.org".into(),
                    ..header("header")
                },
            ],
        )
        .unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        // The header rule matches already, but waits with the other.
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );
        conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
            .unwrap();
        body(&conn, 1, "your code is 1234");
        let actions = due(&conn, 1000);
        assert_eq!(actions.len(), 1, "one run");
        assert!(actions[0].trash, "the first in the list moves it");
        assert_eq!(actions[0].forwards, ["dan@example.org"]);
        assert_eq!(actions[0].rule_ids, ["body", "header"]);
    }

    #[test]
    fn a_rule_acting_on_arrival_checks_a_message_once() {
        let conn = db();
        let read = Rule {
            conditions: vec![
                RuleCondition {
                    field: "unread".into(),
                    value: String::new(),
                    negate: true,
                },
                RuleCondition {
                    field: "body".into(),
                    value: "code".into(),
                    negate: false,
                },
            ],
            ..rule(RuleTrigger::Arrival, 0)
        };
        set_rules(&conn, vec![read]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
            .unwrap();
        body(&conn, 1, "your code");
        assert!(due(&conn, 1000).is_empty(), "unread as it arrived");
        // Read later: no longer what arrived, and not news to this rule.
        crate::store::update_message_seen(&conn, "a", "INBOX", 1, true).unwrap();
        assert!(due(&conn, 1000).is_empty());
    }

    #[test]
    fn starting_to_wait_for_bodies_does_not_replay_arrivals() {
        let conn = db();
        let header = Rule {
            conditions: vec![RuleCondition {
                field: "subject".into(),
                value: "login".into(),
                negate: false,
            }],
            ..rule(RuleTrigger::Arrival, 0)
        };
        set_rules(&conn, vec![header.clone()]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        let ran = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        finish_rule_actions(&conn, "a", "INBOX", &[1], ran[0].run).unwrap();
        body(&conn, 1, "hello");
        set_rules(
            &conn,
            vec![
                header,
                Rule {
                    id: "body".into(),
                    ..body_rule("body", "nothing like it", false)
                },
            ],
        )
        .unwrap();
        assert!(
            due(&conn, store_now()).is_empty(),
            "the header rule ran on it already"
        );
    }

    #[test]
    fn mail_that_could_not_be_opened_has_no_body_to_read() {
        let conn = db();
        set_rules(&conn, vec![body_rule("body", "invoice", true)]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
            .unwrap();
        body(&conn, 1, "");
        conn.execute(
            "UPDATE messages SET json = json_set(json, '$.crypto', json('{\"encrypted\":true}'))",
            [],
        )
        .unwrap();
        assert!(due(&conn, 1000).is_empty(), "its body may well say invoice");
        assert!(
            rule_body_waiting_uids(&conn, "a", "INBOX", 1000, 50, false)
                .unwrap()
                .is_empty()
        );
        conn.execute(
            "UPDATE messages SET json = json_set(json, '$.crypto.decrypted_with', 'ABCD')",
            [],
        )
        .unwrap();
        assert_eq!(due(&conn, 1000).len(), 1);

        // Only part of it opened: the rest may still say it.
        conn.execute(
            "UPDATE messages SET json = json_set(json, '$.crypto', json('{\"problem\":\"partly_encrypted\"}'))",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM rule_firings", []).unwrap();
        conn.execute("DELETE FROM rule_evaluations", []).unwrap();
        conn.execute("DELETE FROM rule_actions", []).unwrap();
        assert!(due(&conn, 1000).is_empty());
    }

    #[test]
    fn only_bodies_rules_acting_on_arrival_wait_on_hold_up_a_sync() {
        let conn = db();
        let age = Rule {
            id: "age".into(),
            trigger: RuleTrigger::Age,
            delay_seconds: 60,
            ..body_rule("body", "x", false)
        };
        set_rules(&conn, vec![age]).unwrap();
        mail(&conn, 1, false, 100);
        let waiting = |arrivals| {
            rule_body_waiting_uids(&conn, "a", EXISTING_RULE_FOLDER, 1000, 50, arrivals).unwrap()
        };
        assert_eq!(waiting(false), [1]);
        assert!(waiting(true).is_empty());
    }

    #[test]
    fn mail_cached_with_only_html_is_read_as_shown() {
        let conn = db();
        set_rules(&conn, vec![body_rule("body", "invoice", false)]).unwrap();
        mail(&conn, 1, false, 100);
        mail(&conn, 2, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1, 2]).unwrap();
        conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
            .unwrap();
        let html = |uid: u32, html: &str| {
            conn.execute(
                "UPDATE messages SET json = json_set(json, '$.body_html', ?2) WHERE uid = ?1",
                params![uid, html],
            )
            .unwrap();
        };
        // As the body of mail downloaded now is: markup gone, text kept.
        html(1, "<p>Your <span class=\"x\">invoice</span> is ready</p>");
        html(2, "<p class=\"invoice\">Hello</p>");
        let uids: Vec<u32> = due(&conn, 1000).iter().map(|action| action.uid).collect();
        assert_eq!(uids, [1]);
    }

    #[test]
    fn subject_or_body_finds_either() {
        let conn = db();
        set_rules(&conn, vec![body_rule("subject_or_body", "LOGIN", false)]).unwrap();
        mail(&conn, 1, false, 100); // Its subject is "login code".
        mail(&conn, 2, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1, 2]).unwrap();
        conn.execute("UPDATE rule_events SET arrived_at = 1000", [])
            .unwrap();
        conn.execute("UPDATE messages SET subject = 'hello' WHERE uid = 2", [])
            .unwrap();
        body(&conn, 1, "");
        body(&conn, 2, "your login link");
        let uids: Vec<u32> = due(&conn, 1000).iter().map(|action| action.uid).collect();
        assert_eq!(uids, [1, 2]);
    }

    #[test]
    fn starting_to_read_the_body_does_not_replay_old_arrivals() {
        let conn = db();
        set_rules(&conn, vec![rule(RuleTrigger::Arrival, 0)]).unwrap();
        mail(&conn, 1, false, 100);
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        finish_rule_actions(&conn, "a", "INBOX", &[1], 0).unwrap();
        body(&conn, 1, "invoice");
        set_rules(&conn, vec![body_rule("body", "invoice", false)]).unwrap();
        assert!(due(&conn, store_now()).is_empty());
    }

    fn store_now() -> i64 {
        crate::store::now_unix() + 10
    }

    #[test]
    fn body_conditions_need_text_and_received_mail() {
        let conn = db();
        assert!(set_rules(&conn, vec![body_rule("body", " ", false)]).is_err());
        let sent = Rule {
            sent: true,
            ..body_rule("body", "x", false)
        };
        assert!(set_rules(&conn, vec![sent]).is_err());
        assert_eq!(
            body_rule("subject_or_body", "x", false).compat_query(),
            "is:read AND is:unread"
        );
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

#[cfg(test)]
mod sent_tests {
    use super::*;
    use crate::imap::MessageHeader;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::run_migrations(&conn).unwrap();
        conn
    }

    fn header(uid: u32) -> MessageHeader {
        MessageHeader {
            uid,
            seen: true,
            subject: "quote".into(),
            message_id: format!("<{uid}@example.com>"),
            ..Default::default()
        }
    }

    fn sent_rule() -> Rule {
        Rule {
            id: "sent".into(),
            sent: true,
            star: true,
            tags: vec!["client".into()],
            ..Default::default()
        }
    }

    fn received_rule() -> Rule {
        Rule {
            id: "received".into(),
            mark_read: true,
            ..Default::default()
        }
    }

    /// One read of `folder` that finds `uids`, as a sync or the read after a
    /// send does it. Answers with what it took for newly sent.
    fn read(conn: &Connection, folder: &str, uids: &[u32], uid_next: u32) -> Vec<u32> {
        let batch: Vec<MessageHeader> = uids.iter().map(|uid| header(*uid)).collect();
        let sent = new_sent_uids(conn, "a", folder, 7, uid_next, &batch).unwrap();
        crate::store::upsert_messages(conn, "a", folder, &batch).unwrap();
        apply_sent_rules(conn, "a", folder, 7, &sent).unwrap();
        crate::store::set_folder_state(conn, "a", folder, 7, uid_next).unwrap();
        sent
    }

    #[test]
    fn old_rules_are_for_received_mail_and_sent_rules_are_limited() {
        let old: Rule = serde_json::from_str(r#"{"id":"old","star":true}"#).unwrap();
        assert!(!old.sent);
        let conn = db();
        let refused = [
            Rule {
                mark_read: true,
                ..sent_rule()
            },
            Rule {
                move_to: "trash".into(),
                ..sent_rule()
            },
            Rule {
                trigger: RuleTrigger::Age,
                delay_seconds: 60,
                ..sent_rule()
            },
            Rule {
                delay_seconds: 60,
                ..sent_rule()
            },
        ];
        for rule in refused {
            assert!(set_rules(&conn, vec![rule.clone()]).is_err(), "{rule:?}");
        }
        assert!(set_rules(&conn, vec![sent_rule()]).unwrap()[0].sent);
    }

    #[test]
    fn sent_rules_run_once_on_copies_the_sent_folder_gains() {
        let conn = db();
        set_rules(&conn, vec![sent_rule(), received_rule()]).unwrap();
        // The first read is the mailbox as it was, not mail just sent.
        assert!(read(&conn, "Sent", &[1, 2], 3).is_empty());
        assert!(pending_rule_actions(&conn, "a", "Sent").unwrap().is_empty());

        assert_eq!(read(&conn, "Sent", &[2, 3], 4), vec![3]);
        let actions = pending_rule_actions(&conn, "a", "Sent").unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].uid, 3);
        assert!(actions[0].star);
        // Only the rule for sent mail: the other would have marked it read.
        assert!(!actions[0].mark_read);
        assert_eq!(actions[0].rule_ids, vec!["sent".to_string()]);
        let tagged: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_tags WHERE tag = 'client' AND tag_key = '3@example.com'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tagged, 1);

        // Reading the same tail again finds nothing new.
        finish_rule_actions(&conn, "a", "Sent", &[3], 0).unwrap();
        assert!(read(&conn, "Sent", &[2, 3], 4).is_empty());
        assert!(pending_rule_actions(&conn, "a", "Sent").unwrap().is_empty());
    }

    #[test]
    fn only_live_sent_rules_for_the_account_count() {
        let conn = db();
        set_rules(&conn, vec![received_rule()]).unwrap();
        assert!(!has_sent_rules(&conn, "a").unwrap());
        let mut rule = Rule {
            accounts: vec!["b".into()],
            ..sent_rule()
        };
        set_rules(&conn, vec![rule.clone()]).unwrap();
        assert!(!has_sent_rules(&conn, "a").unwrap());
        assert!(has_sent_rules(&conn, "b").unwrap());
        rule.disabled = true;
        set_rules(&conn, vec![rule]).unwrap();
        assert!(!has_sent_rules(&conn, "b").unwrap());
    }

    #[test]
    fn a_copy_already_seen_elsewhere_on_the_account_is_still_sent() {
        let conn = db();
        set_rules(&conn, vec![sent_rule()]).unwrap();
        read(&conn, "Sent", &[1], 2);
        // Mail to oneself, in the Inbox before its Sent copy is read.
        crate::store::upsert_messages(&conn, "a", "INBOX", &[header(2)]).unwrap();
        assert_eq!(read(&conn, "Sent", &[2], 3), vec![2]);
        assert_eq!(pending_rule_actions(&conn, "a", "Sent").unwrap().len(), 1);
    }

    #[test]
    fn sent_rules_leave_received_mail_alone_and_the_other_way_round() {
        let conn = db();
        set_rules(&conn, vec![sent_rule(), received_rule()]).unwrap();
        crate::store::upsert_messages(&conn, "a", "INBOX", &[header(1)]).unwrap();
        apply_rules(&conn, "a", "INBOX", 7, &[1]).unwrap();
        apply_due_rules(&conn, "a", "INBOX", 7, i64::MAX / 2).unwrap();
        let actions = pending_rule_actions(&conn, "a", "INBOX").unwrap();
        assert_eq!(actions.len(), 1);
        assert!(actions[0].mark_read && !actions[0].star);
        assert_eq!(actions[0].rule_ids, vec!["received".to_string()]);
    }

    #[test]
    fn a_sent_rule_run_by_hand_looks_in_the_sent_folder() {
        let conn = db();
        conn.execute(
            "INSERT INTO accounts(id, engine, provider, email, display_name, avatar_url, config, created_at, updated_at)
             VALUES('a', 'imap', 'imap', 'a', '', '', '{}', 0, 0)",
            [],
        )
        .unwrap();
        let folder = |name: &str, special_use: Option<&str>| crate::imap::Folder {
            name: name.into(),
            display_name: name.into(),
            special_use: special_use.map(str::to_string),
            ..Default::default()
        };
        crate::store::upsert_folders(
            &conn,
            "a",
            &[folder("INBOX", None), folder("Sent Items", Some("sent"))],
        )
        .unwrap();
        let id = set_rules(&conn, vec![sent_rule()]).unwrap()[0].id.clone();
        read(&conn, "Sent Items", &[1, 2], 3);
        crate::store::upsert_messages(&conn, "a", "INBOX", &[header(9)]).unwrap();
        crate::store::set_folder_state(&conn, "a", "INBOX", 7, 10).unwrap();
        assert_eq!(
            saved_rule_folder(&conn, &id, "a").unwrap().as_deref(),
            Some("Sent Items")
        );
        assert_eq!(count_existing_rule_matches(&conn, &id).unwrap(), 2);
        assert_eq!(
            queue_existing_rule(&conn, &id).unwrap(),
            vec![("a".to_string(), 2)]
        );
        assert_eq!(
            pending_rule_actions(&conn, "a", "Sent Items")
                .unwrap()
                .len(),
            2
        );
        assert!(
            pending_rule_actions(&conn, "a", "INBOX")
                .unwrap()
                .is_empty()
        );
    }
}
