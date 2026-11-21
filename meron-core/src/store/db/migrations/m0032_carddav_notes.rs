//! Contact notes sync as the vCard NOTE. Forget each book's sync token and
//! each card's version so the next sync reads every card again and fills
//! them in: a card still showing no note must not be written over a note it
//! has on the server. `note` is added only when missing, as this step always
//! did.

use anyhow::Result;
use rusqlite::Connection;

pub(super) fn run(conn: &Connection) -> Result<()> {
    let has_note: bool = conn.query_row(
        "SELECT count(*) > 0 FROM pragma_table_info('carddav_cards') WHERE name = 'note'",
        [],
        |row| row.get(0),
    )?;
    if !has_note {
        conn.execute_batch("ALTER TABLE carddav_cards ADD COLUMN note TEXT NOT NULL DEFAULT '';")?;
    }
    conn.execute_batch(
        "UPDATE carddav_books SET sync_token = '';
        UPDATE carddav_cards SET etag = '';",
    )?;
    Ok(())
}
