PRAGMA user_version = 23;

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

-- table contacts
CREATE TABLE contacts (
           account  TEXT NOT NULL,
           addr_key TEXT NOT NULL,   -- lower(trim(addr))
           name     TEXT NOT NULL,   -- '' when the address came without one
           addr     TEXT NOT NULL,
           count    INTEGER NOT NULL DEFAULT 0, last_date INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (account, addr_key, name)
         ) WITHOUT ROWID;

-- table events
CREATE TABLE events (
           id          TEXT PRIMARY KEY,
           title       TEXT NOT NULL DEFAULT '',
           notes       TEXT NOT NULL DEFAULT '',
           location    TEXT NOT NULL DEFAULT '',
           start_at    INTEGER NOT NULL DEFAULT 0,   -- all-day: UTC midnight
           end_at      INTEGER NOT NULL DEFAULT 0,   -- exclusive
           all_day     INTEGER NOT NULL DEFAULT 0,
           recurring   INTEGER NOT NULL DEFAULT 0,
           rrule       TEXT NOT NULL DEFAULT '',     -- daily | weekly | monthly | yearly
           tzid        TEXT NOT NULL DEFAULT '',     -- the zone a repeat keeps its time in
           account     TEXT NOT NULL DEFAULT '',     -- mail link; empty for an event made here
           thread_id   TEXT NOT NULL DEFAULT '',
           message_id  TEXT NOT NULL DEFAULT '',
           invite_uid  TEXT NOT NULL DEFAULT '',     -- iCalendar UID of an accepted invitation
           recurrence_id INTEGER NOT NULL DEFAULT 0, -- an invitation to one occurrence alone; 0 = the event
           response    TEXT NOT NULL DEFAULT '',     -- accepted | tentative
           ics         TEXT NOT NULL DEFAULT '',
           json        TEXT NOT NULL DEFAULT '{}',
           created_at  INTEGER NOT NULL DEFAULT 0,
           updated_at  INTEGER NOT NULL DEFAULT 0
         , alerts TEXT);

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

-- table invite_responses
CREATE TABLE invite_responses (
           account       TEXT NOT NULL,
           uid           TEXT NOT NULL,     -- iCalendar UID
           recurrence_id INTEGER NOT NULL DEFAULT 0,   -- 0 = the whole series
           response      TEXT NOT NULL,     -- accepted | tentative | declined
           sequence      INTEGER NOT NULL DEFAULT 0,
           responded_at  INTEGER NOT NULL DEFAULT 0, this_and_future INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (account, uid, recurrence_id)
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

-- table message_tags
CREATE TABLE message_tags (
            account    TEXT NOT NULL,
            tag_key    TEXT NOT NULL,
            tag        TEXT NOT NULL,
            state      INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account, tag_key, tag)
         ) WITHOUT ROWID;

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
  json             TEXT NOT NULL DEFAULT '{}', recipients TEXT, files TEXT, server_tags TEXT,   -- JSON catch-all (recipients, body_html, rss fields)
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

-- table reminder_deliveries
CREATE TABLE reminder_deliveries (
            key TEXT PRIMARY KEY,
            delivered_at INTEGER NOT NULL
         );

-- table removed_message_uids
CREATE TABLE removed_message_uids (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           epoch   INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;

-- table rule_actions
CREATE TABLE rule_actions (
            account     TEXT NOT NULL,
            folder      TEXT NOT NULL,
            uid         INTEGER NOT NULL,
            uidvalidity INTEGER NOT NULL,
            mark_read   INTEGER NOT NULL DEFAULT 0,
            star        INTEGER NOT NULL DEFAULT 0,
            trash       INTEGER NOT NULL DEFAULT 0,
            target      TEXT NOT NULL DEFAULT '',
            copies      TEXT NOT NULL DEFAULT '[]',
            rule_ids    TEXT NOT NULL DEFAULT '[]',
            run         INTEGER NOT NULL DEFAULT 0,
            attempts    INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account, folder, uid, run)
         ) WITHOUT ROWID;

-- table rule_log
CREATE TABLE rule_log (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            at      INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            account TEXT NOT NULL,
            subject TEXT NOT NULL DEFAULT '',
            count   INTEGER NOT NULL DEFAULT 1,
            failed  INTEGER NOT NULL DEFAULT 0,
            detail  TEXT NOT NULL DEFAULT ''
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

-- index events_invite_idx
CREATE UNIQUE INDEX events_invite_idx
           ON events(account, invite_uid, recurrence_id) WHERE invite_uid <> '';

-- index events_start_idx
CREATE INDEX events_start_idx ON events(start_at);

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

-- index message_tags_tag_idx
CREATE INDEX message_tags_tag_idx ON message_tags(account, tag);

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

-- index messages_tag_key_idx
CREATE INDEX messages_tag_key_idx ON messages(account, COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid)) WHERE uid <> 0;

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

-- trigger contacts_ad
CREATE TRIGGER contacts_ad AFTER DELETE ON messages BEGIN
           UPDATE contacts SET count = count - 1
               WHERE account = old.account AND old.uid <> 0 AND (addr_key, name) IN (SELECT lower(trim(addr)), trim(COALESCE(name, '')) FROM (SELECT old.from_name AS name, old.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.cc') WHERE type = 'object')
               WHERE trim(COALESCE(addr, '')) <> '');
             DELETE FROM contacts
               WHERE account = old.account AND count <= 0 AND (addr_key, name) IN (SELECT lower(trim(addr)), trim(COALESCE(name, '')) FROM (SELECT old.from_name AS name, old.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.cc') WHERE type = 'object')
               WHERE trim(COALESCE(addr, '')) <> '');
         END;

-- trigger contacts_ai
CREATE TRIGGER contacts_ai AFTER INSERT ON messages BEGIN
           INSERT INTO contacts(account, addr_key, name, addr, count, last_date)
               SELECT new.account, lower(trim(addr)), trim(COALESCE(name, '')), max(trim(addr)), 1, new.date
               FROM (SELECT new.from_name AS name, new.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.cc') WHERE type = 'object') WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
               GROUP BY lower(trim(addr)), trim(COALESCE(name, ''))
             ON CONFLICT(account, addr_key, name) DO UPDATE SET count = count + 1, last_date = max(last_date, excluded.last_date);
         END;

-- trigger contacts_au
CREATE TRIGGER contacts_au
           AFTER UPDATE OF account, uid, from_name, from_addr, json, date ON messages
           WHEN old.account IS NOT new.account
             OR (old.uid <> 0) IS NOT (new.uid <> 0)
             OR old.from_name IS NOT new.from_name
             OR old.from_addr IS NOT new.from_addr
             OR json_extract(old.json, '$.to', '$.cc') IS NOT json_extract(new.json, '$.to', '$.cc')
             OR old.date IS NOT new.date
         BEGIN
           UPDATE contacts SET count = count - 1
               WHERE account = old.account AND old.uid <> 0 AND (addr_key, name) IN (SELECT lower(trim(addr)), trim(COALESCE(name, '')) FROM (SELECT old.from_name AS name, old.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.cc') WHERE type = 'object')
               WHERE trim(COALESCE(addr, '')) <> '');
             DELETE FROM contacts
               WHERE account = old.account AND count <= 0 AND (addr_key, name) IN (SELECT lower(trim(addr)), trim(COALESCE(name, '')) FROM (SELECT old.from_name AS name, old.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(old.json, '$.cc') WHERE type = 'object')
               WHERE trim(COALESCE(addr, '')) <> '');
           INSERT INTO contacts(account, addr_key, name, addr, count, last_date)
               SELECT new.account, lower(trim(addr)), trim(COALESCE(name, '')), max(trim(addr)), 1, new.date
               FROM (SELECT new.from_name AS name, new.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.cc') WHERE type = 'object') WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
               GROUP BY lower(trim(addr)), trim(COALESCE(name, ''))
             ON CONFLICT(account, addr_key, name) DO UPDATE SET count = count + 1, last_date = max(last_date, excluded.last_date);
         END;

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
