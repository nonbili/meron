//! CalDAV calendars (nonbili/meron#127): the accounts, their calendars, and
//! which server object each synced event is. See `caldav::store`.
//!
//! `events.calendar` is added only when missing, as this step always did.

use anyhow::Result;
use rusqlite::Connection;

const DDL: &str = "
CREATE TABLE IF NOT EXISTS dav_accounts (
  id           TEXT PRIMARY KEY,
  server_url   TEXT NOT NULL DEFAULT '',  -- as the user typed it
  username     TEXT NOT NULL DEFAULT '',
  display_name TEXT NOT NULL DEFAULT '',
  home         TEXT NOT NULL DEFAULT '',  -- calendar-home-set, found by discovery
  last_sync_at INTEGER NOT NULL DEFAULT 0,
  last_error   TEXT NOT NULL DEFAULT '',
  json         TEXT NOT NULL DEFAULT '{}',
  created_at   INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS dav_calendars (
  id         TEXT PRIMARY KEY,
  account    TEXT NOT NULL,
  url        TEXT NOT NULL,
  name       TEXT NOT NULL DEFAULT '',
  color      TEXT NOT NULL DEFAULT '',
  components TEXT NOT NULL DEFAULT '',     -- comma-separated: VEVENT,VTODO; empty = any
  ctag       TEXT NOT NULL DEFAULT '',     -- as of the last completed sync
  sync_token TEXT NOT NULL DEFAULT '',
  read_only  INTEGER NOT NULL DEFAULT 0,
  enabled    INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  json       TEXT NOT NULL DEFAULT '{}',
  created_at INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL DEFAULT 0,
  UNIQUE (account, url)
);
CREATE TABLE IF NOT EXISTS dav_objects (
  calendar TEXT NOT NULL,
  url      TEXT NOT NULL,
  etag     TEXT NOT NULL DEFAULT '',     -- empty: fetch again on the next sync
  local_id TEXT NOT NULL DEFAULT '',     -- events.id; empty for what is not shown (a VTODO)
  state    TEXT NOT NULL DEFAULT '',     -- '' in step | created | changed | deleted
  moved_to TEXT NOT NULL DEFAULT '',     -- a deletion that waits for this URL's create
  PRIMARY KEY (calendar, url)
);
CREATE INDEX IF NOT EXISTS dav_objects_local_idx ON dav_objects(local_id) WHERE local_id <> '';
";

pub(super) fn run(conn: &Connection) -> Result<()> {
    conn.execute_batch(DDL)?;
    let has_calendar: bool = conn.query_row(
        "SELECT count(*) > 0 FROM pragma_table_info('events') WHERE name = 'calendar'",
        [],
        |row| row.get(0),
    )?;
    if !has_calendar {
        conn.execute_batch("ALTER TABLE events ADD COLUMN calendar TEXT NOT NULL DEFAULT '';")?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS events_calendar_idx ON events(calendar) WHERE calendar <> '';",
    )?;
    Ok(())
}
