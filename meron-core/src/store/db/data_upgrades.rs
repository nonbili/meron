//! One-time rewrites of existing rows after a data-format change — a new body
//! renderer, a repeat-rule editor that can say more — kept off the schema
//! version so a format change doesn't look like a schema change. Each is
//! tracked by its own `meta` key and runs as the store opens.
//!
//! The local-folder archive runs none of these: it has no synced events, and
//! its rendered bodies stay as they were rendered (their search text follows
//! the renderer of the day they were archived; `archive_raw` keeps the
//! originals, should that ever need redoing).

use super::{meta_get, meta_set};
use anyhow::Result;
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

/// Render format of the cached `body` of HTML mail.
const BODY_CACHE: (&str, &str) = ("body_cache_version", "1");

/// Which repeat rules the editor can say, as `events.rrule` was last worked
/// out for synced events (see `calendar::rrule`).
const SYNCED_RULES: (&str, &str) = ("synced_rules_version", "3");

/// Rows rewritten per batch, bounding how much is held in memory at once.
const BATCH: i64 = 200;

/// Whether opening this store would run a rewrite. Only decides whether to
/// tell the user, so a failed read answers "no".
pub(super) fn pending(conn: &Connection) -> bool {
    [BODY_CACHE, SYNCED_RULES]
        .into_iter()
        .any(|(key, version)| {
            meta_get(conn, key).is_ok_and(|current| current.as_deref() != Some(version))
        })
}

/// Run `upgrade` unless `key` already records `version`. Checks cheaply
/// first, then again under the writer lock, so two connections opening at
/// once don't both do the work; the new version commits with the rewrite.
fn run_once(
    conn: &Connection,
    (key, version): (&str, &str),
    upgrade: impl FnOnce(&Connection) -> Result<()>,
) -> Result<()> {
    if meta_get(conn, key)?.as_deref() == Some(version) {
        return Ok(());
    }
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    if meta_get(&tx, key)?.as_deref() == Some(version) {
        return Ok(());
    }
    upgrade(&tx)?;
    meta_set(&tx, key, version)?;
    tx.commit()?;
    Ok(())
}

/// Re-render every HTML-derived body in place (`body_is_rendered = 1`) from
/// its stored `body_html` when the render format changes. Re-rendering rather
/// than nulling keeps the FTS `body` column populated, so full-text search
/// over these messages keeps working across a version bump instead of
/// degrading until each message is reopened.
pub(crate) fn invalidate_body_cache_if_needed(conn: &Connection) -> Result<()> {
    run_once(conn, BODY_CACHE, |tx| {
        let mut next = i64::MIN;
        loop {
            let rows = tx
                .prepare_cached(
                    "SELECT id, json_extract(json, '$.body_html') FROM messages
                     WHERE id >= ?1 AND json_extract(json, '$.body_is_rendered') = 1
                     ORDER BY id LIMIT ?2",
                )?
                .query_map(params![next, BATCH], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let Some(&(last, _)) = rows.last() else {
                break;
            };
            for (id, html) in rows {
                // No HTML source to re-render from: drop the stale body and
                // let the read path fall back if a source ever reappears.
                let body = html.as_deref().map(crate::parse::render_body);
                tx.execute(
                    "UPDATE messages SET body = ?2 WHERE id = ?1",
                    params![id, body],
                )?;
            }
            let Some(after) = last.checked_add(1) else {
                break;
            };
            next = after;
        }
        Ok(())
    })
}

/// A synced event's `rrule` says whether the editor can say its repeat, as
/// that was when the event was last fetched. Once the editor changes what it
/// can say, every synced series' rule is read again from its own object,
/// rather than waiting for its server to send a change: one it can now say
/// is filled in, and one it no longer takes on (a zone only the object
/// defines) goes back to its server.
pub(crate) fn reread_synced_repeat_rules(conn: &Connection) -> Result<()> {
    run_once(conn, SYNCED_RULES, |tx| {
        // A batch at a time: a calendar's objects can be large.
        let mut after = String::new();
        loop {
            let rows = tx
                .prepare_cached(
                    "SELECT id, ics, rrule FROM events
                     WHERE calendar <> '' AND recurring = 1 AND ics <> '' AND id > ?1
                     ORDER BY id LIMIT ?2",
                )?
                .query_map(params![after, BATCH], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let Some((last, _, _)) = rows.last() else {
                break;
            };
            after = last.clone();
            for (id, ics, stored) in rows {
                // An object that cannot be read is left as it was.
                let Some(rule) = crate::caldav::ics::event_from_ics(&ics).map(|event| event.rrule)
                else {
                    continue;
                };
                if rule != stored {
                    tx.execute(
                        "UPDATE events SET rrule = ?2 WHERE id = ?1",
                        params![id, rule],
                    )?;
                }
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_rerender_reaches_every_id_across_batches() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        // Negative, zero and positive ids, more than one batch of them.
        let ids = (-3..BATCH * 2).collect::<Vec<i64>>();
        for id in &ids {
            conn.execute(
                "INSERT INTO messages(id, account, folder, msg_id, uid, body, json)
                 VALUES(?1, 'acc', 'INBOX', ?1, ?1, 'stale',
                   '{\"body_is_rendered\":1,\"body_html\":\"<p>fresh</p>\"}')",
                [id],
            )
            .unwrap();
        }
        invalidate_body_cache_if_needed(&conn).unwrap();
        let stale: i64 = conn
            .query_row(
                "SELECT count(*) FROM messages WHERE body = 'stale'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale, 0);
        assert_eq!(
            meta_get(&conn, BODY_CACHE.0).unwrap().as_deref(),
            Some(BODY_CACHE.1)
        );
    }

    #[test]
    fn a_rules_only_upgrade_is_pending() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::run_migrations(&conn).unwrap();
        invalidate_body_cache_if_needed(&conn).unwrap();
        reread_synced_repeat_rules(&conn).unwrap();
        assert!(!pending(&conn));
        meta_set(&conn, SYNCED_RULES.0, "0").unwrap();
        assert!(pending(&conn));
    }
}
