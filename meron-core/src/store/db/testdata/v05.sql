PRAGMA user_version = 5;

-- table account_secrets
CREATE TABLE account_secrets (
  account_id TEXT PRIMARY KEY,
  blob       TEXT NOT NULL
);

-- table accounts
CREATE TABLE accounts (
  id           TEXT PRIMARY KEY,
  engine       TEXT NOT NULL DEFAULT 'mail',     -- 'mail' | 'rss'
  provider     TEXT NOT NULL DEFAULT 'custom',   -- 'custom' | 'gmail' | 'rss'
  email        TEXT NOT NULL DEFAULT '',
  display_name TEXT NOT NULL DEFAULT '',
  avatar_url   TEXT NOT NULL DEFAULT '',
  sender_name TEXT NOT NULL DEFAULT '',         -- sender name
  config       TEXT NOT NULL DEFAULT '{}',       -- JSON connection metadata (mail)
  prefs        TEXT NOT NULL DEFAULT '{}',       -- user preferences (see AccountPrefs)
  sort_order   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL DEFAULT 0
);

-- table folder_state
CREATE TABLE folder_state (
  account        TEXT NOT NULL,
  folder         TEXT NOT NULL,
  uidvalidity    INTEGER,
  uid_next       INTEGER,
  highest_modseq INTEGER,
  PRIMARY KEY (account, folder)
);

-- table folders
CREATE TABLE folders (
  account   TEXT NOT NULL,
  name      TEXT NOT NULL,
  delimiter TEXT, special_use TEXT,
  PRIMARY KEY (account, name)
);

-- table messages
CREATE TABLE messages (
  id               INTEGER PRIMARY KEY,   -- stable surrogate rowid; survives VACUUM, is the FTS docid
  account          TEXT NOT NULL,
  folder           TEXT NOT NULL,   -- mail: IMAP folder; rss: subscription id
  msg_id           TEXT NOT NULL,   -- mail: uid as string; rss: item key
  uid              INTEGER NOT NULL DEFAULT 0,   -- mail uid; 0 for rss
  subject          TEXT,
  from_name        TEXT,
  from_addr        TEXT,
  date             INTEGER NOT NULL DEFAULT 0,   -- send time, epoch seconds (0 = unknown)
  seen             INTEGER NOT NULL DEFAULT 0,
  starred          INTEGER NOT NULL DEFAULT 0,
  thread_key       TEXT,
  body             TEXT,
  json             TEXT NOT NULL DEFAULT '{}',   -- JSON catch-all (recipients, body_html, rss fields)
  UNIQUE (account, folder, msg_id)   -- real message identity (mail UID / rss item key)
);

-- table messages_fts
CREATE VIRTUAL TABLE messages_fts USING fts5(
  subject, from_name, from_addr, body,
  content='messages', content_rowid='id',
  tokenize='trigram'
);

-- table meta
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- table observed_mail_identities
CREATE TABLE observed_mail_identities (
  account       TEXT NOT NULL,
  identity      TEXT NOT NULL,
  first_seen_at INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, identity)
);

-- table settings
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- table subscriptions
CREATE TABLE subscriptions (
  id            TEXT PRIMARY KEY,
  account       TEXT NOT NULL,
  url           TEXT NOT NULL UNIQUE,
  title         TEXT NOT NULL DEFAULT '',
  site_url      TEXT NOT NULL DEFAULT '',
  feed_title    TEXT NOT NULL DEFAULT '',
  enabled       INTEGER NOT NULL DEFAULT 1,
  last_sync_at  INTEGER NOT NULL DEFAULT 0,
  last_error    TEXT NOT NULL DEFAULT '',
  etag          TEXT NOT NULL DEFAULT '',
  last_modified TEXT NOT NULL DEFAULT '',
  created_at    INTEGER NOT NULL DEFAULT 0,
  updated_at    INTEGER NOT NULL DEFAULT 0
, json TEXT NOT NULL DEFAULT '{}');

-- index messages_date_idx
CREATE INDEX messages_date_idx    ON messages(account, folder, date, uid);

-- index messages_list_idx
CREATE INDEX messages_list_idx    ON messages(account, folder, uid);

-- index messages_starred_all_idx
CREATE INDEX messages_starred_all_idx ON messages(date, uid) WHERE starred <> 0;

-- index messages_starred_idx
CREATE INDEX messages_starred_idx ON messages(account, folder, date, uid) WHERE starred <> 0;

-- index messages_unread_idx
CREATE INDEX messages_unread_idx  ON messages(account, folder, date, uid) WHERE seen = 0;

-- index subscriptions_account_idx
CREATE INDEX subscriptions_account_idx ON subscriptions(account);

-- trigger messages_ad
CREATE TRIGGER messages_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
  VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
END;

-- trigger messages_ai
CREATE TRIGGER messages_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
  VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
END;

-- trigger messages_au
CREATE TRIGGER messages_au AFTER UPDATE OF subject, from_name, from_addr, body ON messages BEGIN
  INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
  VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
  INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
  VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
END;
