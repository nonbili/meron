//! Searchable `To`/`Cc` text plus the FTS index over it, so a lookup by
//! recipient finds cached mail the way a lookup by sender already does.
//! `To`/`Cc` live in the `json` catch-all, which FTS can't index, so
//! `messages.recipients` mirrors them as flat text. It gets its own FTS table
//! rather than a fifth column on `messages_fts`: adding a column there would
//! force a full rebuild of the body trigram index.
//!
//! Existing rows are backfilled, then indexed in one `rebuild`, which is
//! cheaper than letting the triggers fire per row. The decoding and
//! formatting below are frozen copies of what the write path did when this
//! shipped (`store::recipients_index_text`), so later changes there can't
//! change this upgrade.

use anyhow::Result;
use rusqlite::{Connection, params};

const FTS_DDL: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS messages_recipients_fts USING fts5(
  recipients,
  content='messages', content_rowid='id',
  tokenize='trigram'
);
CREATE TRIGGER IF NOT EXISTS messages_recipients_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
END;
CREATE TRIGGER IF NOT EXISTS messages_recipients_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
  VALUES ('delete', old.id, old.recipients);
END;
CREATE TRIGGER IF NOT EXISTS messages_recipients_au AFTER UPDATE OF recipients ON messages BEGIN
  INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
  VALUES ('delete', old.id, old.recipients);
  INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
END;
";

#[derive(serde::Deserialize)]
struct Recipient {
    name: String,
    addr: String,
}

/// A cached `$.to`/`$.cc` array; null or garbage reads as empty.
fn parse(json: Option<String>) -> Vec<Recipient> {
    json.and_then(|s| serde_json::from_str::<Vec<Recipient>>(&s).ok())
        .unwrap_or_default()
}

fn index_text(to: &[Recipient], cc: &[Recipient]) -> Option<String> {
    let text = to
        .iter()
        .chain(cc.iter())
        .map(|recipient| {
            format!("{} {}", recipient.name, recipient.addr)
                .trim()
                .to_string()
        })
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    (!text.is_empty()).then_some(text)
}

pub(super) fn run(conn: &Connection) -> Result<()> {
    conn.execute_batch("ALTER TABLE messages ADD COLUMN recipients TEXT;")?;
    let mut stmt = conn.prepare(
        "SELECT id, json_extract(json, '$.to'), json_extract(json, '$.cc') FROM messages
         WHERE json_extract(json, '$.to') IS NOT NULL OR json_extract(json, '$.cc') IS NOT NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                parse(row.get::<_, Option<String>>(1)?),
                parse(row.get::<_, Option<String>>(2)?),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, to, cc) in rows {
        conn.execute(
            "UPDATE messages SET recipients = ?2 WHERE id = ?1",
            params![id, index_text(&to, &cc)],
        )?;
    }
    conn.execute_batch(FTS_DDL)?;
    conn.execute_batch(
        "INSERT INTO messages_recipients_fts(messages_recipients_fts) VALUES('rebuild');",
    )?;
    Ok(())
}
