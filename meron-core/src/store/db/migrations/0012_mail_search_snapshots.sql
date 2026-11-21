-- Resumable live searches. One row per live search: the identity a snapshot
-- page is validated against. A first page fetches only the newest hits; the
-- rest wait in `mail_search_pending`, one row per server hit with its Date
-- header resolved up front, so batches are taken newest-first by date across
-- every folder rather than by UID. `cache_resume` is set when a batch cut off
-- its cached-only hits: the keyset cursor of the last one it took, from which
-- paging past the snapshot continues through the cache. Hit rows saved by
-- earlier versions have no snapshot row, so their cursors fall back to cached
-- keyset paging, as an expired snapshot does.

CREATE TABLE IF NOT EXISTS mail_search_snapshots (
  token        TEXT PRIMARY KEY,
  account      TEXT NOT NULL,
  query        TEXT NOT NULL,
  scope        TEXT NOT NULL,
  cache_resume TEXT,
  created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS mail_search_snapshots_created_idx
  ON mail_search_snapshots(account, created_at);
CREATE TABLE IF NOT EXISTS mail_search_pending (
  token   TEXT NOT NULL,
  account TEXT NOT NULL,
  folder  TEXT NOT NULL,
  uid     INTEGER NOT NULL,
  date    INTEGER NOT NULL,
  PRIMARY KEY (token, folder, uid)
);
CREATE INDEX IF NOT EXISTS mail_search_pending_order_idx
  ON mail_search_pending(token, date DESC, uid DESC, folder DESC);
CREATE INDEX IF NOT EXISTS mail_search_pending_account_idx
  ON mail_search_pending(account);
