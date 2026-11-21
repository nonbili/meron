//! Who a queued rule action still forwards the message to (`store::rules`): a
//! JSON list of addresses, emptied once the forward has gone out so that a
//! retry of the rest doesn't send it again. Added only when missing.

use anyhow::Result;
use rusqlite::Connection;

pub(super) fn run(conn: &Connection) -> Result<()> {
    let has_forwards: bool = conn.query_row(
        "SELECT count(*) > 0 FROM pragma_table_info('rule_actions') WHERE name = 'forwards'",
        [],
        |row| row.get(0),
    )?;
    if !has_forwards {
        conn.execute_batch(
            "ALTER TABLE rule_actions ADD COLUMN forwards TEXT NOT NULL DEFAULT '[]';",
        )?;
    }
    Ok(())
}
