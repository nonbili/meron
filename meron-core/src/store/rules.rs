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
    Ok(decode_rules(rule_data(conn)?).unwrap_or_default())
}

/// An editor must report unreadable rules so a subsequent save preserves them.
pub fn rules_for_editing(conn: &Connection) -> Result<Vec<Rule>> {
    decode_rules(rule_data(conn)?)
        .map_err(|err| anyhow::anyhow!("could not read saved mail rules: {err}"))
}

fn rule_data(conn: &Connection) -> Result<Option<serde_json::Value>> {
    let mut stored = super::settings_get(conn, &[RULES_KEY.to_string()])?;
    Ok(stored
        .as_object_mut()
        .and_then(|settings| settings.remove(RULES_KEY)))
}

fn decode_rules(value: Option<serde_json::Value>) -> serde_json::Result<Vec<Rule>> {
    let rules: Vec<Rule> = value
        .map(serde_json::from_value)
        .transpose()?
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

/// A rule as text to share, indented so a field can be edited by hand. Only
/// what differs from a new rule is written, as an import fills in the rest.
/// Its id and whether it is disabled stay behind: an import gets a new id and
/// starts disabled anyway.
pub fn rule_source(rule: &Rule) -> Result<String> {
    fn without_defaults(value: &mut serde_json::Value, default: serde_json::Value) {
        if let (Some(fields), serde_json::Value::Object(default)) = (value.as_object_mut(), default)
        {
            fields.retain(|name, value| default.get(name) != Some(value));
        }
    }
    let mut rule = serde_json::to_value(rule)?;
    without_defaults(&mut rule, serde_json::to_value(Rule::default())?);
    if let Some(fields) = rule.as_object_mut() {
        fields.remove("id");
        fields.remove("disabled");
    }
    // A condition always names its field, even an empty one.
    let mut default_condition = serde_json::to_value(RuleCondition::default())?;
    if let Some(fields) = default_condition.as_object_mut() {
        fields.remove("field");
    }
    for condition in rule
        .get_mut("conditions")
        .and_then(|conditions| conditions.as_array_mut())
        .into_iter()
        .flatten()
    {
        without_defaults(condition, default_condition.clone());
    }
    Ok(serde_json::to_string_pretty(
        &serde_json::json!({ "meron_rule": 1, "rule": rule }),
    )?)
}

/// Read a single shared rule into a new, disabled draft.
pub fn parse_rule_source(source: &str) -> Result<Rule> {
    let value: serde_json::Value = serde_json::from_str(source)?;
    anyhow::ensure!(
        value["meron_rule"].as_u64() == Some(1),
        "unknown rule format"
    );
    let rule = value
        .get("rule")
        .filter(|rule| rule.is_object())
        .ok_or_else(|| anyhow::anyhow!("expected a single rule"))?;
    let mut rule = clean_rule(serde_json::from_value(rule.clone())?)?;
    rule.id.clear();
    rule.disabled = true;
    Ok(rule)
}

fn clean_rule(rule: Rule) -> Result<Rule> {
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
        !rule.sent || (!rule.mark_read && rule.move_to.is_empty() && rule.forward_to.is_empty()),
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
    Ok(rule)
}

/// Replace the rules, refusing any that could never apply.
pub fn set_rules(conn: &Connection, incoming: Vec<Rule>) -> Result<Vec<Rule>> {
    // Also used inside tag-renaming transactions.
    if !conn.is_autocommit() {
        return replace_rules(conn, &rules(conn)?, incoming);
    }
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let saved = replace_rules(&tx, &rules(&tx)?, incoming)?;
    tx.commit()?;
    Ok(saved)
}

fn replace_rules(conn: &Connection, previous: &[Rule], incoming: Vec<Rule>) -> Result<Vec<Rule>> {
    let clean = incoming
        .into_iter()
        .map(clean_rule)
        .collect::<Result<Vec<_>>>()?;
    save_rules(conn, previous, &clean, None)?;
    Ok(clean)
}

/// Replace the list only if it still matches the editor's snapshot.
pub fn set_rules_if_current(
    conn: &Connection,
    expected: &[Rule],
    incoming: Vec<Rule>,
) -> Result<(Vec<Rule>, bool)> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let current = rules_for_editing(&tx)?;
    if current != expected {
        return Ok((current, false));
    }
    let saved = replace_rules(&tx, &current, incoming)?;
    tx.commit()?;
    Ok((saved, true))
}

pub enum RuleEdit {
    Create(Rule),
    Update(String, serde_json::Map<String, serde_json::Value>),
    Delete(String),
}

/// Change a single rule without overwriting concurrent edits to the list.
pub fn edit_rule(conn: &Connection, edit: RuleEdit) -> Result<Rule> {
    edit_rule_with_local_folders(conn, edit, &[])
}

/// Local destination IDs come from the archive, which has its own database.
pub fn edit_rule_with_local_folders(
    conn: &Connection,
    edit: RuleEdit,
    local_folders: &[String],
) -> Result<Rule> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let previous = rules_for_editing(&tx)?;
    let mut next = previous.clone();
    let mut restart_events = false;
    let changed = match edit {
        RuleEdit::Create(mut rule) => {
            rule.id.clear();
            let rule = clean_rule(rule)?;
            validate_rule_edit(&tx, None, &rule, local_folders)?;
            next.push(rule.clone());
            rule
        }
        RuleEdit::Update(id, changes) => {
            let index = rule_index(&next, &id)?;
            let mut values = serde_json::to_value(&next[index])?;
            values.as_object_mut().unwrap().extend(changes);
            let mut rule: Rule = serde_json::from_value(values)?;
            rule.id = id;
            let rule = clean_rule(rule)?;
            validate_rule_edit(&tx, Some(&next[index]), &rule, local_folders)?;
            restart_events = rule != next[index];
            next[index] = rule.clone();
            rule
        }
        RuleEdit::Delete(id) => {
            let index = rule_index(&next, &id)?;
            next.remove(index)
        }
    };
    save_rules(
        &tx,
        &previous,
        &next,
        restart_events.then_some(changed.id.as_str()),
    )?;
    tx.commit()?;
    Ok(changed)
}

fn rule_index(rules: &[Rule], id: &str) -> Result<usize> {
    anyhow::ensure!(!id.trim().is_empty(), "rule_id is required");
    let mut matches = rules.iter().enumerate().filter(|(_, rule)| rule.id == id);
    let (index, _) = matches
        .next()
        .ok_or_else(|| anyhow::anyhow!("no such rule: {id}"))?;
    anyhow::ensure!(matches.next().is_none(), "more than one rule has ID {id}");
    Ok(index)
}

fn validate_rule_edit(
    conn: &Connection,
    previous: Option<&Rule>,
    rule: &Rule,
    local_folders: &[String],
) -> Result<()> {
    anyhow::ensure!(
        rule.trigger != RuleTrigger::Age || rule.disabled || previous == Some(rule),
        "age rules can act on existing Inbox mail. Save the rule disabled and enable it in Meron."
    );

    // Recheck all targets when turning a rule back on. Otherwise validate the
    // changed targets so a missing account or folder can't prevent turning it off.
    let previous = previous.filter(|old| !old.disabled || rule.disabled);
    let scope_changed = previous.is_none_or(|old| old.accounts != rule.accounts);
    let move_changed = previous
        .is_none_or(|old| old.folder != rule.folder || old.folder_account != rule.folder_account)
        || (scope_changed && rule.folder_account.is_empty());
    let copy_changed = previous.is_none_or(|old| {
        old.copy_folder != rule.copy_folder || old.copy_account != rule.copy_account
    });
    if !scope_changed && !move_changed && !copy_changed {
        return Ok(());
    }
    let accounts = mail_rule_account_ids(conn)?;
    if scope_changed {
        for id in &rule.accounts {
            anyhow::ensure!(accounts.contains(id), "unknown mail account: {id}");
        }
    }
    if move_changed && !rule.folder.is_empty() {
        if rule.folder_account.is_empty() {
            validate_relative_move(conn, &accounts, rule)?;
        } else {
            validate_rule_folder(
                conn,
                &accounts,
                &rule.folder_account,
                &rule.folder,
                local_folders,
            )?;
        }
    }
    if copy_changed && !rule.copy_folder.is_empty() {
        validate_rule_folder(
            conn,
            &accounts,
            &rule.copy_account,
            &rule.copy_folder,
            local_folders,
        )?;
    }
    Ok(())
}

fn validate_rule_folder(
    conn: &Connection,
    accounts: &[String],
    id: &str,
    folder: &str,
    local_folders: &[String],
) -> Result<()> {
    if super::is_local_account(id) {
        let wanted = super::normalize_local_folder_name(folder)?;
        anyhow::ensure!(
            super::has_local_account(conn)? && local_folders.contains(&wanted),
            "unknown local folder {folder}"
        );
        return Ok(());
    }
    anyhow::ensure!(
        accounts.iter().any(|account| account == id),
        "unknown mail account: {id}"
    );
    let wanted = crate::mail_model::canon_folder(folder);
    anyhow::ensure!(
        super::folder_exists(conn, id, &wanted)?,
        "unknown folder {folder} in account {id}. Refresh its folder list first."
    );
    Ok(())
}

fn validate_relative_move(conn: &Connection, accounts: &[String], rule: &Rule) -> Result<()> {
    // The engine skips source accounts where the folder is absent.
    for account in accounts
        .iter()
        .filter(|id| rule.accounts.is_empty() || rule.accounts.contains(id))
    {
        if matching_rule_folder(conn, account, &rule.folder)?.is_some() {
            return Ok(());
        }
    }
    anyhow::bail!(
        "unknown folder {} in the rule's mail accounts. Refresh their folder lists first.",
        rule.folder
    )
}

/// Persist the rules together with their event cutoffs.
fn save_rules(
    conn: &Connection,
    previous: &[Rule],
    clean: &[Rule],
    restart_events_for: Option<&str>,
) -> Result<()> {
    // Rules acting right away that start or stop waiting for the body start
    // afresh, as a delayed rule does, rather than take up events from before.
    let holds_changed = arrival_holds(previous) != arrival_holds(clean);
    for rule in clean {
        let timing_unchanged = restart_events_for != Some(rule.id.as_str())
            && !(holds_changed && rule.on_arrival())
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
    let stored = stored_rules(clean);
    for table in ["rule_activation", "rule_evaluations", "rule_firings"] {
        conn.execute(&format!(
            "DELETE FROM {table} WHERE rule_id NOT IN (SELECT json_extract(value, '$.id') FROM json_each(?1))"
        ), [stored.to_string()])?;
    }
    super::setting_set(conn, RULES_KEY, &stored)?;
    Ok(())
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
        "folder" => matching_rule_folder(conn, account, &rule.folder)?,
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

fn matching_rule_folder(conn: &Connection, account: &str, folder: &str) -> Result<Option<String>> {
    let wanted = crate::mail_model::canon_folder(folder);
    Ok(super::get_folders(conn, account)?
        .into_iter()
        .find(|item| item.name == wanted || item.display_name == folder)
        .map(|item| item.name))
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
    // A server copy without a Message-ID can set off rules again where it
    // lands. Local copies deduplicate by raw bytes and never trigger rules.
    let server_copy = |copy: &RuleCopy| !super::is_local_account(&copy.account);
    let copies_to_server = copy.as_ref().is_some_and(server_copy);
    let moves_to_server = destination
        .as_ref()
        .and_then(|it| it.copy.as_ref())
        .is_some_and(server_copy);
    let ids = if copies_to_server || moves_to_server {
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
            .filter(|_| identified || !moves_to_server);
        if let Some(destination) = destination.filter(|_| !action.moves()) {
            action.trash = destination.trash;
            action.target = destination.target.clone();
            action.copies.extend(destination.copy.clone());
        }
        if identified || !copies_to_server {
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
pub(crate) fn rule_accounts(conn: &Connection, rule: &Rule) -> Result<Vec<String>> {
    Ok(mail_rule_account_ids(conn)?
        .into_iter()
        .filter(|id| rule.accounts.is_empty() || rule.accounts.contains(id))
        .collect())
}

fn mail_rule_accounts(conn: &Connection) -> Result<Vec<serde_json::Value>> {
    Ok(super::list_accounts(conn)?
        .into_iter()
        .filter(|account| {
            account["engine"].as_str() != Some("rss")
                && account["provider"].as_str() != Some("rss")
                && account["id"]
                    .as_str()
                    .is_some_and(|id| !super::is_local_account(id))
        })
        .collect())
}

fn mail_rule_account_ids(conn: &Connection) -> Result<Vec<String>> {
    Ok(mail_rule_accounts(conn)?
        .iter()
        .filter_map(|account| account["id"].as_str().map(str::to_owned))
        .collect())
}

/// The cached account and folder identifiers used by saved mail rules.
pub fn rule_targets(conn: &Connection) -> Result<serde_json::Value> {
    let mut targets = Vec::new();
    for account in mail_rule_accounts(conn)? {
        let id = account["id"].as_str().unwrap_or_default();
        let name = account["display_name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .or_else(|| account["email"].as_str().filter(|email| !email.is_empty()))
            .unwrap_or(id);
        let folders: Vec<_> = super::get_folders(conn, id)?
            .into_iter()
            .map(|folder| serde_json::json!({ "id": folder.name, "name": folder.display_name }))
            .collect();
        targets.push(serde_json::json!({ "id": id, "name": name, "folders": folders }));
    }
    Ok(serde_json::json!({ "accounts": targets }))
}

pub(crate) fn saved_rule(conn: &Connection, id: &str) -> Result<Rule> {
    rules(conn)?
        .into_iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| anyhow::anyhow!("no such rule: {id}"))
}

/// Check the rule that the editor displayed before applying it to existing mail.
pub fn require_current_rule(conn: &Connection, id: &str, expected: &Rule) -> Result<Rule> {
    let rule = saved_rule(conn, id)?;
    anyhow::ensure!(
        rule == *expected,
        "review the changed rule before running it"
    );
    Ok(rule)
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
    let rule = saved_rule(conn, id)?;
    Ok(queue_existing_rule_checked(conn, &rule)?
        .into_iter()
        .map(|(account, _, count)| (account, count))
        .collect())
}

pub(crate) fn queue_existing_rule_checked(
    conn: &Connection,
    expected: &Rule,
) -> Result<Vec<(String, String, usize)>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let rule = require_current_rule(&tx, &expected.id, expected)?;
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
            queued.push((account, folder.to_string(), count));
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
mod timing_tests;

#[cfg(test)]
mod sent_tests;

#[cfg(test)]
mod edit_tests;
