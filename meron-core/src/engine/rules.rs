//! The server half of rules: reading, starring and moving what a sync matched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::store::RuleAction;

/// Small enough to finish well inside a sync's budget even on a server without
/// MOVE, which needs a few round trips per message.
const RULE_MOVE_CHUNK: usize = 50;

/// Queued actions that want the same things done, each set able to fail alone:
/// a deleted destination must not use up the tries of mail bound elsewhere.
fn rule_action_batches(actions: Vec<RuleAction>) -> Vec<Vec<RuleAction>> {
    let mut batches: BTreeMap<_, Vec<RuleAction>> = BTreeMap::new();
    for action in actions {
        let plan = (
            action.mark_read,
            action.star,
            action.trash,
            action.target.clone(),
        );
        batches.entry(plan).or_default().push(action);
    }
    batches.into_values().collect()
}

/// Carry out the queued rule actions of `folder`, adding the UIDs that no longer
/// need a notification to `silenced` as each is done, so a caller that gives up
/// waiting still knows. Every step is safe to repeat, so what fails stays queued
/// for the next sync.
pub async fn run_rule_actions(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<()> {
    {
        let db = engine.db.lock().unwrap();
        if store::account_paused(&db, account)?
            || store::pending_rule_actions(&db, account, folder)?.is_empty()
        {
            return Ok(());
        }
    }
    // Before a move: tags go to the copy they were saved on.
    if let Err(err) = push_pending_tags(engine, account).await {
        crate::mlog!(
            crate::log::Level::Warn,
            "mail.rules",
            "account={account}: tag push failed: {err:#}"
        );
    }
    let turn = engine
        .tag_pushes
        .lock()
        .unwrap()
        .entry(account.to_string())
        .or_default()
        .clone();
    let _turn = turn.lock().await;

    let actions = store::pending_rule_actions(&engine.db.lock().unwrap(), account, folder)?;
    // The server's word, not the cache's: UIDs mean nothing past their
    // UIDVALIDITY, and acting on them could read, move or bin other mail.
    let validity = engine
        .with_write_session(account, |session| {
            let folder = folder.to_string();
            Box::pin(async move { imap::folder_uidvalidity(session, &folder).await })
        })
        .await?;
    let (current, stale): (Vec<_>, Vec<_>) = actions
        .into_iter()
        .partition(|action| action.uidvalidity == validity);
    let uids_of = |actions: &[RuleAction]| actions.iter().map(|a| a.uid).collect::<Vec<_>>();
    store::finish_rule_actions(
        &engine.db.lock().unwrap(),
        account,
        folder,
        &uids_of(&stale),
    )?;
    for batch in rule_action_batches(current) {
        let uids = uids_of(&batch);
        match run_folder_actions(engine, account, folder, &batch, silenced).await {
            Ok(()) => {
                store::finish_rule_actions(&engine.db.lock().unwrap(), account, folder, &uids)?;
            }
            Err(err) => {
                let dropped =
                    store::retry_rule_actions(&engine.db.lock().unwrap(), account, folder, &uids)?;
                crate::mlog!(
                    crate::log::Level::Warn,
                    "mail.rules",
                    "account={account} folder={folder}: rule actions failed, {dropped} given up: {err:#}"
                );
            }
        }
    }
    Ok(())
}

async fn run_folder_actions(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    actions: &[RuleAction],
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<()> {
    let picked = |wanted: fn(&RuleAction) -> bool| -> Vec<u32> {
        actions
            .iter()
            .filter(|action| wanted(action))
            .map(|action| action.uid)
            .collect()
    };
    // Flags first: a moved message has a new UID here that the queue doesn't know.
    let read = picked(|action| action.mark_read);
    if !read.is_empty() {
        let targets = BTreeMap::from([(folder.to_string(), read.clone())]);
        engine.store_seen_by_folder(account, &targets, true).await?;
        let db = engine.db.lock().unwrap();
        for uid in &read {
            store::update_message_seen(&db, account, folder, *uid, true)?;
        }
        silenced.extend(read);
    }
    let starred = picked(|action| action.star);
    if !starred.is_empty() {
        let targets = BTreeMap::from([(folder.to_string(), starred)]);
        mark_starred_copies(engine, account, &targets, true).await?;
    }

    let mut moves: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for action in actions.iter().filter(|action| !action.target.is_empty()) {
        moves.entry(&action.target).or_default().push(action.uid);
    }
    for (target, uids) in moves {
        move_rule_mail(engine, account, folder, target, &uids, silenced).await?;
    }

    let trashed = picked(|action| action.trash);
    if !trashed.is_empty() {
        let trash = engine
            .with_write_session(account, |session| {
                Box::pin(async move { imap::find_trash_folder(session).await })
            })
            .await?
            .ok_or_else(|| anyhow::anyhow!("Trash folder not found for this account"))?;
        move_rule_mail(engine, account, folder, &trash, &trashed, silenced).await?;
    }
    Ok(())
}

/// Move mail for a rule, repeatably and a few at a time: what each step did is
/// recorded before the next, so running out of time keeps it, and a later sync
/// finds that mail gone from here and carries on with the rest.
async fn move_rule_mail(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    target: &str,
    uids: &[u32],
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<()> {
    for chunk in uids.chunks(RULE_MOVE_CHUNK) {
        move_rule_chunk(engine, account, folder, target, chunk, silenced).await?;
    }
    Ok(())
}

/// One step of [`move_rule_mail`]. Unlike a move the user asked for, this leaves
/// `target` for its own sync to read: refreshing it here would swallow mail that
/// has just arrived there, before its rules and notifications ran.
async fn move_rule_chunk(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    target: &str,
    uids: &[u32],
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<()> {
    let cached = store::cached_message_ids(&engine.db.lock().unwrap(), account, folder, uids)?;
    // One message gone unreadable must not hold the others back.
    let known: Vec<String> = cached
        .iter()
        .map(|id| id.clone().unwrap_or_default())
        .collect();
    let ids = moved_message_ids(engine, account, folder, uids, cached)
        .await
        .unwrap_or(known);
    let outcome = engine
        .with_write_session(account, |session| {
            let folder = folder.to_string();
            let target = target.to_string();
            let uids = uids.to_vec();
            let ids = ids.clone();
            Box::pin(async move {
                imap::move_to_folder_once(session, &folder, &target, &uids, &ids).await
            })
        })
        .await?;
    store::delete_messages_by_uid(&engine.db.lock().unwrap(), account, folder, &outcome.moved)?;
    silenced.extend(outcome.moved);
    outcome.failed.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_bound_for_different_places_fails_separately() {
        let action = |uid, target: &str| RuleAction {
            uid,
            target: target.into(),
            ..RuleAction::default()
        };
        let batches = rule_action_batches(vec![
            action(1, "Gone"),
            action(2, "Work"),
            action(3, "Gone"),
        ]);
        let uids: Vec<Vec<u32>> = batches
            .iter()
            .map(|batch| batch.iter().map(|action| action.uid).collect())
            .collect();
        assert_eq!(uids, [vec![1, 3], vec![2]]);
    }
}
