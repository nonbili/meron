PRAGMA user_version = 16;

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

-- table active_message_syncs
CREATE TABLE active_message_syncs (
           id    INTEGER PRIMARY KEY AUTOINCREMENT,
           epoch INTEGER NOT NULL,
           owner TEXT NOT NULL
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
  delimiter TEXT, special_use TEXT, notify INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, name)
);

-- table mail_search_hits
CREATE TABLE mail_search_hits (
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

-- table mail_search_pending
CREATE TABLE mail_search_pending (
  token   TEXT NOT NULL,
  account TEXT NOT NULL,
  folder  TEXT NOT NULL,
  uid     INTEGER NOT NULL,
  date    INTEGER NOT NULL,
  PRIMARY KEY (token, folder, uid)
);

-- table mail_search_snapshots
CREATE TABLE mail_search_snapshots (
  token        TEXT PRIMARY KEY,
  account      TEXT NOT NULL,
  query        TEXT NOT NULL,
  scope        TEXT NOT NULL,
  cache_resume TEXT,
  created_at   INTEGER NOT NULL
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
  json             TEXT NOT NULL DEFAULT '{}', recipients TEXT, files TEXT,   -- JSON catch-all (recipients, body_html, rss fields)
  UNIQUE (account, folder, msg_id)   -- real message identity (mail UID / rss item key)
);

-- table messages_fts
CREATE VIRTUAL TABLE messages_fts USING fts5(
  subject, from_name, from_addr, body,
  content='messages', content_rowid='id',
  tokenize='trigram'
);

-- table messages_recipients_fts
CREATE VIRTUAL TABLE messages_recipients_fts USING fts5(
  recipients,
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

-- table removed_message_uids
CREATE TABLE removed_message_uids (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           epoch   INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;

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

-- table task_lists
CREATE TABLE task_lists (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL DEFAULT 0
);

-- table tasks
CREATE TABLE tasks (
  id           TEXT PRIMARY KEY,
  list_id      TEXT NOT NULL,
  title        TEXT NOT NULL DEFAULT '',
  notes        TEXT NOT NULL DEFAULT '',
  due_at       INTEGER NOT NULL DEFAULT 0,   -- 0 = no due date
  completed_at INTEGER NOT NULL DEFAULT 0,   -- 0 = not done
  sort_order   INTEGER NOT NULL DEFAULT 0,
  account      TEXT NOT NULL DEFAULT '',     -- mail link; empty for a plain task
  thread_id    TEXT NOT NULL DEFAULT '',
  message_id   TEXT NOT NULL DEFAULT '',
  json         TEXT NOT NULL DEFAULT '{}',
  created_at   INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL DEFAULT 0
);

-- table uncached_unseen
CREATE TABLE uncached_unseen (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;

-- index mail_search_hits_created_idx
CREATE INDEX mail_search_hits_created_idx
  ON mail_search_hits(account, created_at);

-- index mail_search_pending_account_idx
CREATE INDEX mail_search_pending_account_idx
  ON mail_search_pending(account);

-- index mail_search_pending_order_idx
CREATE INDEX mail_search_pending_order_idx
  ON mail_search_pending(token, date DESC, uid DESC, folder DESC);

-- index mail_search_snapshots_created_idx
CREATE INDEX mail_search_snapshots_created_idx
  ON mail_search_snapshots(account, created_at);

-- index messages_date_idx
CREATE INDEX messages_date_idx    ON messages(account, folder, date, uid);

-- index messages_list_idx
CREATE INDEX messages_list_idx    ON messages(account, folder, uid);

-- index messages_message_id_idx
CREATE INDEX messages_message_id_idx
  ON messages(account, lower(COALESCE(json_extract(json, '$.message_id'), '')));

-- index messages_starred_all_idx
CREATE INDEX messages_starred_all_idx ON messages(date, uid) WHERE starred <> 0;

-- index messages_starred_idx
CREATE INDEX messages_starred_idx ON messages(account, folder, date, uid) WHERE starred <> 0;

-- index messages_thread_key_idx
CREATE INDEX messages_thread_key_idx
  ON messages(account, lower(COALESCE(thread_key, '')));

-- index messages_unread_idx
CREATE INDEX messages_unread_idx  ON messages(account, folder, date, uid) WHERE seen = 0;

-- index subscriptions_account_idx
CREATE INDEX subscriptions_account_idx ON subscriptions(account);

-- index tasks_list_idx
CREATE INDEX tasks_list_idx ON tasks(list_id, sort_order);

-- index tasks_thread_idx
CREATE INDEX tasks_thread_idx ON tasks(thread_id);

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
CREATE TRIGGER messages_au AFTER UPDATE OF subject, from_name, from_addr, body ON messages
         WHEN old.subject IS NOT new.subject OR old.from_name IS NOT new.from_name
           OR old.from_addr IS NOT new.from_addr OR old.body IS NOT new.body
         BEGIN
           INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
           VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
           INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
           VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
         END;

-- trigger messages_recipients_ad
CREATE TRIGGER messages_recipients_ad AFTER DELETE ON messages BEGIN
  INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
  VALUES ('delete', old.id, old.recipients);
END;

-- trigger messages_recipients_ai
CREATE TRIGGER messages_recipients_ai AFTER INSERT ON messages BEGIN
  INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
END;

-- trigger messages_recipients_au
CREATE TRIGGER messages_recipients_au AFTER UPDATE OF recipients ON messages
         WHEN old.recipients IS NOT new.recipients
         BEGIN
           INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
           VALUES ('delete', old.id, old.recipients);
           INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
         END;

-- trigger messages_uncached_unseen_ai
CREATE TRIGGER messages_uncached_unseen_ai AFTER INSERT ON messages
         WHEN new.uid <> 0 BEGIN
           DELETE FROM uncached_unseen
            WHERE account = new.account AND folder = new.folder AND uid = new.uid;
         END;

-- trigger messages_uncached_unseen_au
CREATE TRIGGER messages_uncached_unseen_au
         AFTER UPDATE OF account, folder, uid ON messages
         WHEN new.uid <> 0 BEGIN
           DELETE FROM uncached_unseen
            WHERE account = new.account AND folder = new.folder AND uid = new.uid;
         END;

-- trigger observe_mail_identity_ai
CREATE TRIGGER observe_mail_identity_ai AFTER INSERT ON messages WHEN new.uid <> 0 BEGIN
             INSERT INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(new.account, CASE
               WHEN json_extract(new.json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(new.json, '$.gmail_msg_id')
               WHEN COALESCE(json_extract(new.json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(new.json, '$.message_id'))
               ELSE 'uid:' || lower(new.folder) || ':' || new.uid END, unixepoch())
             ON CONFLICT DO NOTHING;
             END;

-- trigger observe_mail_identity_au
CREATE TRIGGER observe_mail_identity_au AFTER UPDATE OF json, uid, folder, account ON messages WHEN new.uid <> 0 AND (old.json IS NOT new.json OR old.uid IS NOT new.uid OR old.folder IS NOT new.folder OR old.account IS NOT new.account) BEGIN
             INSERT INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(new.account, CASE
               WHEN json_extract(new.json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(new.json, '$.gmail_msg_id')
               WHEN COALESCE(json_extract(new.json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(new.json, '$.message_id'))
               ELSE 'uid:' || lower(new.folder) || ':' || new.uid END, unixepoch())
             ON CONFLICT DO NOTHING;
             END;
