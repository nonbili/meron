-- Stable server-search snapshots. IMAP SEARCH returns an unordered UID set,
-- while the UI pages by message date; retaining the resolved order prevents a
-- later page from losing messages whose Date header does not follow UID order.

CREATE TABLE IF NOT EXISTS mail_search_hits (
  token      TEXT NOT NULL,
  account    TEXT NOT NULL,
  query      TEXT NOT NULL,
  scope      TEXT NOT NULL,
  position   INTEGER NOT NULL,
  folder     TEXT NOT NULL,
  uid        INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (token, position)
);
CREATE INDEX IF NOT EXISTS mail_search_hits_created_idx
  ON mail_search_hits(account, created_at);
