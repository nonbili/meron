//! The server half of rules: reading, starring and moving what a sync matched.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::store::{RuleAction, RuleCopy};

/// Small enough to finish well inside a sync's budget even on a server without
/// MOVE, which needs a few round trips per message.
const RULE_MOVE_CHUNK: usize = 50;

/// How much queued work one turn takes. What a turn finishes is off the queue, so
/// running out of time costs only the turn in hand; tag pushes, which share its
/// lock, aren't kept out of a long run; and stopping a run takes effect soon.
const RULE_RUN_TURN: usize = 25;

/// How long starting a run waits for a turn on its accounts to end.
const RULE_RUN_START_WAIT: Duration = Duration::from_secs(3);

/// How long downloading a message to copy may take. Well past a sync's budget,
/// which is why a sync leaves what it can't finish to a worker.
const RULE_COPY_BUDGET: Duration = Duration::from_secs(600);

/// Run a saved rule over mail already in the Inbox. Answers with how many
/// messages it took on once they are queued; the server work goes on in the
/// background, and whatever an interruption leaves is done by the next sync.
pub async fn run_rule_on_existing(engine: &Arc<Engine>, id: &str) -> anyhow::Result<usize> {
    // With every turn on these accounts out of the way: one mid-move would take a
    // message away from under what is queued here. In a set order, so that two
    // runs can't each hold what the other is waiting for.
    let mut accounts = store::saved_rule_accounts(&engine.db.lock().unwrap(), id)?;
    accounts.sort();
    let mut turns = Vec::with_capacity(accounts.len());
    for account in &accounts {
        let turn = engine
            .tag_pushes
            .lock()
            .unwrap()
            .entry(account.clone())
            .or_default()
            .clone();
        // Within what a caller waits for an answer: late, it would be told this
        // failed and then have it run all the same.
        turns.push(
            tokio::time::timeout(RULE_RUN_START_WAIT, turn.lock_owned())
                .await
                .map_err(|_| anyhow::anyhow!("the account is busy with rules"))?,
        );
    }
    let queued = store::queue_existing_rule(&engine.db.lock().unwrap(), id)?;
    drop(turns);
    let total = queued.iter().map(|(_, count)| count).sum();
    for (account, _) in queued {
        // Tags are already saved, and a run of nothing else queues no turns.
        report_rule_progress(engine, &account, store::EXISTING_RULE_FOLDER);
        spawn_tag_push(engine, &account);
        spawn_rule_worker(engine, &account, store::EXISTING_RULE_FOLDER);
    }
    Ok(total)
}

/// Go through `folder`'s queued actions in the background, free of any sync's
/// deadline. One per mailbox: it takes up newly queued work as well, and a second
/// would spend another of the tries of whatever just failed for the first.
pub fn spawn_rule_worker(engine: &Arc<Engine>, account: &str, folder: &str) {
    let key = format!("{account}\n{folder}");
    if !engine.rule_workers.lock().unwrap().insert(key.clone()) {
        return;
    }
    let engine = engine.clone();
    let account = account.to_string();
    let folder = folder.to_string();
    tokio::spawn(async move {
        let mut tried = BTreeSet::new();
        loop {
            let before = tried.len();
            work_off_rule_actions(&engine, &account, &folder, &mut tried).await;
            engine.rule_workers.lock().unwrap().remove(&key);
            // A run that came along just now saw this worker and left it to it.
            // Not when this got nowhere, as on a paused account: that's for a
            // later sync, and going round again would only spin.
            if tried.len() == before
                || !has_untried_rule_actions(&engine, &account, &folder, &tried)
                || !engine.rule_workers.lock().unwrap().insert(key.clone())
            {
                break;
            }
        }
    });
}

fn report_rule_progress(engine: &Engine, account: &str, folder: &str) {
    if let Some(report) = engine.rule_progress.get() {
        report(account, folder);
    }
}

fn has_untried_rule_actions(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    tried: &BTreeSet<(u32, i64)>,
) -> bool {
    store::pending_rule_actions(&engine.db.lock().unwrap(), account, folder).is_ok_and(|actions| {
        actions
            .iter()
            .any(|action| !tried.contains(&(action.uid, action.run)))
    })
}

async fn work_off_rule_actions(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    tried: &mut BTreeSet<(u32, i64)>,
) {
    let mut silenced = BTreeSet::new();
    let after_turn = || report_rule_progress(engine, account, folder);
    if let Err(err) =
        run_rule_turns(engine, account, folder, tried, &mut silenced, &after_turn).await
    {
        crate::mlog!(
            crate::log::Level::Warn,
            "mail.rules",
            "account={account}: rule run: {err:#}"
        );
    }
}

/// Queued actions that want the same things done, moves after the rest, each set
/// able to fail alone: a deleted destination must not use up the tries of mail
/// bound elsewhere.
fn rule_action_batches(actions: Vec<RuleAction>) -> Vec<Vec<RuleAction>> {
    let mut batches: BTreeMap<_, Vec<RuleAction>> = BTreeMap::new();
    for action in actions {
        let plan = (
            // Last: runs can overlap on a message, and once it has moved, its UID
            // here no longer reaches it.
            action.trash || !action.target.is_empty(),
            action.mark_read,
            action.star,
            action.trash,
            action.target.clone(),
            action.copies.clone(),
            action.run,
        );
        batches.entry(plan).or_default().push(action);
    }
    batches.into_values().collect()
}

/// What of `actions`, in UID order, one turn takes: up to [`RULE_RUN_TURN`]
/// messages, but a message to copy alone, as it means downloading all of it and
/// what is done must outlast the time running out. Never between two runs' work on
/// one message, which has an order to keep.
fn one_turn(mut actions: Vec<RuleAction>) -> Vec<RuleAction> {
    let copying = actions.iter().position(|action| !action.copies.is_empty());
    let last = match copying {
        Some(0) => 0,
        Some(at) => (at - 1).min(RULE_RUN_TURN - 1),
        None => RULE_RUN_TURN - 1,
    };
    if let Some(last) = actions.get(last).map(|action| action.uid) {
        actions.retain(|action| action.uid <= last);
    }
    actions
}

/// Move batches earliest run first, as the first move of a message is the one
/// that counts, and what it is mustn't depend on what else a run wants done.
fn in_queue_order(mut batches: Vec<Vec<RuleAction>>) -> Vec<Vec<RuleAction>> {
    batches.sort_by_key(|batch| batch.first().map_or(0, |action| action.run.unsigned_abs()));
    batches
}

/// What of `batch` can go ahead, given the messages that have work left in this
/// turn: a move takes a message out of reach of whatever else is queued for its
/// UID here, so it waits, untried, until that work is done or given up on.
fn ready_actions(batch: Vec<RuleAction>, unfinished: &BTreeSet<u32>) -> Vec<RuleAction> {
    batch
        .into_iter()
        .filter(|action| !action.moves() || !unfinished.contains(&action.uid))
        .collect()
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
    let mut tried = BTreeSet::new();
    run_rule_turns(engine, account, folder, &mut tried, silenced, &|| {}).await
}

/// Turn after turn until everything queued has had its try. One each: tries used
/// up back to back wouldn't outlast even a short outage, so what fails waits for
/// the next sync, without holding up mail behind it.
async fn run_rule_turns(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    tried: &mut BTreeSet<(u32, i64)>,
    silenced: &mut BTreeSet<u32>,
    after_turn: &(dyn Fn() + Sync),
) -> anyhow::Result<()> {
    while run_rule_turn(engine, account, folder, tried, silenced).await? {
        after_turn();
    }
    Ok(())
}

/// The work of [`RULE_RUN_TURN`] queued messages not `tried` yet. Answers with
/// whether there was any.
async fn run_rule_turn(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    tried: &mut BTreeSet<(u32, i64)>,
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<bool> {
    {
        let db = engine.db.lock().unwrap();
        if store::account_paused(&db, account)?
            || store::pending_rule_actions(&db, account, folder)?
                .iter()
                .all(|action| tried.contains(&(action.uid, action.run)))
        {
            return Ok(false);
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

    let mut actions = store::pending_rule_actions(&engine.db.lock().unwrap(), account, folder)?;
    actions.retain(|action| !tried.contains(&(action.uid, action.run)));
    let actions = one_turn(actions);
    tried.extend(actions.iter().map(|action| (action.uid, action.run)));
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
    for action in &stale {
        store::finish_rule_actions(
            &engine.db.lock().unwrap(),
            account,
            folder,
            &[action.uid],
            action.run,
        )?;
    }
    // In passes, as runs can overlap on a message: every run's flags, so that
    // copies carry them, then every copy, then the moves that take it out of
    // reach of the rest.
    let seen: BTreeSet<(u32, i64)> = current.iter().map(|it| (it.uid, it.run)).collect();
    let mut unfinished = BTreeSet::new();
    let mut marked = Vec::new();
    for batch in rule_action_batches(current) {
        let run = batch.first().map_or(0, |action| action.run);
        match mark_rule_mail(engine, account, folder, &batch, silenced).await {
            Ok(()) => marked.push(batch),
            Err(err) => {
                unfinished.extend(uids_of(&batch));
                fail_batch(engine, account, folder, &batch, run, &err)?;
            }
        }
    }
    let mut moving = Vec::new();
    let mut moved = BTreeSet::new();
    for batch in marked {
        // A copy takes the flags the message has now, and can't be revisited.
        let mut batch: Vec<_> = batch
            .into_iter()
            .filter(|action| action.copies.is_empty() || !unfinished.contains(&action.uid))
            .collect();
        let run = batch.first().map_or(0, |action| action.run);
        match copy_batch(engine, account, folder, &batch, run).await {
            Ok(gone) => {
                batch.retain(|action| !gone.contains(&action.uid));
                if batch.iter().any(RuleAction::moves) {
                    moving.push(batch);
                } else {
                    finish_batch(engine, account, folder, &batch, run)?;
                }
            }
            Err(err) => {
                unfinished.extend(uids_of(&batch));
                fail_batch(engine, account, folder, &batch, run, &err)?;
            }
        }
    }
    for batch in in_queue_order(moving) {
        // Work queued since this turn began has had no chance to run yet.
        unfinished.extend(
            store::pending_rule_actions(&engine.db.lock().unwrap(), account, folder)?
                .into_iter()
                .filter(|action| !seen.contains(&(action.uid, action.run)))
                .map(|action| action.uid),
        );
        let batch = ready_actions(batch, &unfinished);
        let Some(run) = batch.first().map(|action| action.run) else {
            continue;
        };
        // A message can only go to one place, and the first move decides.
        let (lost, batch): (Vec<_>, Vec<_>) = batch
            .into_iter()
            .partition(|action| moved.contains(&action.uid));
        store::log_rule_lost_moves(&engine.db.lock().unwrap(), account, &lost)?;
        finish_batch(engine, account, folder, &lost, run)?;
        match move_batch(engine, account, folder, &batch, run, silenced).await {
            Ok(()) => {
                moved.extend(uids_of(&batch));
                finish_batch(engine, account, folder, &batch, run)?;
            }
            Err(err) => {
                unfinished.extend(uids_of(&batch));
                fail_batch(engine, account, folder, &batch, run, &err)?;
            }
        }
    }
    Ok(true)
}

fn finish_batch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    batch: &[RuleAction],
    run: i64,
) -> anyhow::Result<()> {
    let uids: Vec<u32> = batch.iter().map(|action| action.uid).collect();
    store::finish_rule_actions(&engine.db.lock().unwrap(), account, folder, &uids, run)
}

fn fail_batch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    batch: &[RuleAction],
    run: i64,
    err: &anyhow::Error,
) -> anyhow::Result<()> {
    let uids: Vec<u32> = batch.iter().map(|action| action.uid).collect();
    let dropped = store::retry_rule_actions(
        &engine.db.lock().unwrap(),
        account,
        folder,
        &uids,
        run,
        &format!("{err:#}"),
    )?;
    crate::mlog!(
        crate::log::Level::Warn,
        "mail.rules",
        "account={account} folder={folder}: rule actions failed, {dropped} given up: {err:#}"
    );
    Ok(())
}

async fn mark_rule_mail(
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

    Ok(())
}

/// The copies of `actions`, all of `run`. Before any move: only here does the
/// queue's UID still mean this message. Answers with the messages found gone from
/// here, whose work is over, so none of it goes on to the move.
async fn copy_batch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    actions: &[RuleAction],
    run: i64,
) -> anyhow::Result<BTreeSet<u32>> {
    let mut gone = BTreeSet::new();
    let mut failure = None;
    let mut copies: BTreeMap<&RuleCopy, Vec<u32>> = BTreeMap::new();
    for action in actions {
        for copy in &action.copies {
            copies.entry(copy).or_default().push(action.uid);
        }
    }
    for (copy, uids) in copies {
        // Each on its own: a bad destination mustn't cost the good ones the mail.
        match copy_rule_mail(engine, account, folder, copy, &uids).await {
            Ok(missing) => gone.extend(missing),
            Err(err) => failure = Some(err),
        }
    }
    let finished: Vec<u32> = gone.iter().copied().collect();
    store::finish_rule_actions(&engine.db.lock().unwrap(), account, folder, &finished, run)?;
    failure.map_or(Ok(gone), Err)
}

/// The moves of `actions`, all of `run`.
async fn move_batch(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    actions: &[RuleAction],
    run: i64,
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<()> {
    let mut moves: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for action in actions.iter().filter(|action| !action.target.is_empty()) {
        moves.entry(&action.target).or_default().push(action.uid);
    }
    for (target, uids) in moves {
        move_rule_mail(engine, account, folder, target, &uids, run, silenced).await?;
    }

    let trashed: Vec<u32> = actions
        .iter()
        .filter(|action| action.trash)
        .map(|action| action.uid)
        .collect();
    if !trashed.is_empty() {
        let trash = engine
            .with_write_session(account, |session| {
                Box::pin(async move { imap::find_trash_folder(session).await })
            })
            .await?
            .ok_or_else(|| anyhow::anyhow!("Trash folder not found for this account"))?;
        move_rule_mail(engine, account, folder, &trash, &trashed, run, silenced).await?;
    }
    Ok(())
}

/// Copy mail for a rule. The target is asked first, on every try: the answer to
/// an earlier APPEND may have been lost, the rule may have run before, and an
/// earlier copy may be gone again by the time a failed move is retried, which is
/// why the queue keeps a copy until all of its action is done. Like a move, this
/// leaves the target for its own sync to read. Another client may have taken a
/// message away meanwhile: it is answered with, not failed on, so the rest go on.
async fn copy_rule_mail(
    engine: &Arc<Engine>,
    account: &str,
    folder: &str,
    copy: &RuleCopy,
    uids: &[u32],
) -> anyhow::Result<BTreeSet<u32>> {
    let mut gone = BTreeSet::new();
    for chunk in uids.chunks(RULE_MOVE_CHUNK) {
        let messages = engine
            .with_transfer_session(account, RULE_COPY_BUDGET, |session| {
                let folder = folder.to_string();
                let uids = chunk.to_vec();
                Box::pin(
                    async move { imap::fetch_raw_messages_for_copy(session, &folder, &uids).await },
                )
            })
            .await?;
        {
            let db = engine.db.lock().unwrap();
            let copies = BTreeMap::from([(folder.to_string(), chunk.to_vec())]);
            let tags = store::carried_tags(&db, account, &copies)?;
            store::apply_carried_tags(&db, &copy.account, tags, true)?;
        }
        let ids: BTreeMap<u32, String> = {
            let db = engine.db.lock().unwrap();
            let ids = store::cached_message_ids(&db, account, folder, chunk)?;
            chunk
                .iter()
                .copied()
                .zip(ids.into_iter().map(Option::unwrap_or_default))
                .collect()
        };
        // Other accounts may be copying the same message here right now: asking
        // the target and answering it have to be one step.
        let turn = engine
            .tag_pushes
            .lock()
            .unwrap()
            .entry(format!("rule-copy:{}", copy.account))
            .or_default()
            .clone();
        for message in &messages {
            let id = ids.get(&message.uid).cloned().unwrap_or_default();
            let _turn = turn.lock().await;
            engine
                .with_write_session(&copy.account, |session| {
                    let target = copy.folder.clone();
                    let message = message.clone();
                    let id = id.clone();
                    Box::pin(async move {
                        if !id.is_empty()
                            && imap::folder_holds_message(session, &target, &id, &message).await?
                        {
                            return Ok(());
                        }
                        imap::append_copied_message(session, &target, &message).await
                    })
                })
                .await?;
        }
        let fetched: BTreeSet<u32> = messages.iter().map(|message| message.uid).collect();
        gone.extend(chunk.iter().filter(|uid| !fetched.contains(uid)));
    }
    Ok(gone)
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
    run: i64,
    silenced: &mut BTreeSet<u32>,
) -> anyhow::Result<()> {
    for chunk in uids.chunks(RULE_MOVE_CHUNK) {
        move_rule_chunk(engine, account, folder, target, chunk, run, silenced).await?;
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
    run: i64,
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
    {
        // The queue too: a retry of the rest must not look for these here.
        let db = engine.db.lock().unwrap();
        store::delete_messages_by_uid(&db, account, folder, &outcome.moved)?;
        store::finish_rule_actions(&db, account, folder, &outcome.moved, run)?;
    }
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

    #[test]
    fn moves_come_after_what_other_runs_want_done_to_the_same_mail() {
        let batches = rule_action_batches(vec![
            RuleAction {
                uid: 1,
                target: "Archive".into(),
                run: 1,
                ..RuleAction::default()
            },
            RuleAction {
                uid: 1,
                trash: true,
                mark_read: true,
                star: true,
                run: 2,
                ..RuleAction::default()
            },
            RuleAction {
                uid: 1,
                star: true,
                run: 3,
                ..RuleAction::default()
            },
        ]);
        assert_eq!(batches[0][0].run, 3);
        assert_eq!(batches.len(), 3);

        let order = in_queue_order(batches);
        assert_eq!(
            order.iter().map(|batch| batch[0].run).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn scheduled_and_manual_moves_keep_their_queue_order() {
        let batches = [0, -20, 10, -5]
            .into_iter()
            .map(|run| {
                vec![RuleAction {
                    run,
                    ..Default::default()
                }]
            })
            .collect();
        assert_eq!(
            in_queue_order(batches)
                .iter()
                .map(|b| b[0].run)
                .collect::<Vec<_>>(),
            [0, -5, 10, -20]
        );
    }

    #[test]
    fn a_message_to_copy_takes_a_turn_of_its_own() {
        let plain = |uid| RuleAction {
            uid,
            ..RuleAction::default()
        };
        let copying = |uid| RuleAction {
            uid,
            copies: vec![RuleCopy::default()],
            ..RuleAction::default()
        };
        let uids = |actions: Vec<RuleAction>| {
            one_turn(actions)
                .iter()
                .map(|it| it.uid)
                .collect::<Vec<_>>()
        };
        assert_eq!(uids(vec![copying(1), copying(2)]), [1]);
        assert_eq!(uids(vec![plain(1), plain(2), copying(3)]), [1, 2]);
        assert_eq!(uids((1..40).map(plain).collect()).len(), RULE_RUN_TURN);
        // Other runs' work on the message stays with it.
        assert_eq!(uids(vec![copying(1), plain(1), plain(2)]), [1, 1]);
    }

    #[test]
    fn a_move_waits_for_failed_work_on_the_same_message() {
        let action = |uid, target: &str| RuleAction {
            uid,
            target: target.into(),
            ..RuleAction::default()
        };
        let unfinished = BTreeSet::from([1]);
        let ready = ready_actions(
            vec![action(1, "Archive"), action(2, "Archive")],
            &unfinished,
        );
        assert_eq!(ready.iter().map(|it| it.uid).collect::<Vec<_>>(), [2]);
        assert_eq!(ready_actions(vec![action(1, "")], &unfinished).len(), 1);
    }

    #[test]
    fn mail_copied_to_different_places_fails_separately() {
        let action = |uid, folder: &str| RuleAction {
            uid,
            copies: vec![RuleCopy {
                account: "b".into(),
                folder: folder.into(),
            }],
            ..RuleAction::default()
        };
        let batches = rule_action_batches(vec![action(1, "Gone"), action(2, "Work")]);
        assert_eq!(batches.len(), 2);
    }
}
