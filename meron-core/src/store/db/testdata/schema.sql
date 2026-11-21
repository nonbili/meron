PRAGMA user_version = 35;

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

-- table carddav_accounts
CREATE TABLE carddav_accounts (
                id TEXT PRIMARY KEY, server TEXT NOT NULL, username TEXT NOT NULL,
                google INTEGER NOT NULL DEFAULT 0, UNIQUE(server, username)
            );

-- table carddav_books
CREATE TABLE carddav_books (
                id TEXT PRIMARY KEY, account TEXT NOT NULL REFERENCES carddav_accounts(id) ON DELETE CASCADE,
                url TEXT NOT NULL, name TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1,
                sync_token TEXT NOT NULL DEFAULT '', UNIQUE(account, url)
            );

-- table carddav_cards
CREATE TABLE carddav_cards (
                book TEXT NOT NULL REFERENCES carddav_books(id) ON DELETE CASCADE,
                url TEXT NOT NULL, etag TEXT NOT NULL, name TEXT NOT NULL, emails TEXT NOT NULL,
                invalid INTEGER NOT NULL DEFAULT 0, note TEXT NOT NULL DEFAULT '', PRIMARY KEY(book, url)
            );

-- table contact_participants
CREATE TABLE contact_participants (
           message_id INTEGER NOT NULL,
           account    TEXT NOT NULL,
           folder     TEXT NOT NULL,
           addr_key   TEXT NOT NULL,
           name       TEXT NOT NULL,
           addr       TEXT NOT NULL,
           date       INTEGER NOT NULL,
           PRIMARY KEY (message_id, addr_key, name)
         ) WITHOUT ROWID;

-- table contacts
CREATE TABLE contacts (
           account   TEXT NOT NULL,
           folder    TEXT NOT NULL,
           addr_key  TEXT NOT NULL,
           name      TEXT NOT NULL,
           addr      TEXT NOT NULL,
           count     INTEGER NOT NULL,
           last_date INTEGER NOT NULL,
           PRIMARY KEY (account, folder, addr_key, name)
         ) WITHOUT ROWID;

-- table dav_accounts
CREATE TABLE dav_accounts (
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

-- table dav_calendars
CREATE TABLE dav_calendars (
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

-- table dav_objects
CREATE TABLE dav_objects (
  calendar TEXT NOT NULL,
  url      TEXT NOT NULL,
  etag     TEXT NOT NULL DEFAULT '',     -- empty: fetch again on the next sync
  local_id TEXT NOT NULL DEFAULT '',     -- events.id; empty for what is not shown (a VTODO)
  state    TEXT NOT NULL DEFAULT '',     -- '' in step | created | changed | deleted
  moved_to TEXT NOT NULL DEFAULT '',     -- a deletion that waits for this URL's create
  PRIMARY KEY (calendar, url)
);

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
         , alerts TEXT, calendar TEXT NOT NULL DEFAULT '');

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

-- table google_grants
CREATE TABLE google_grants (
                email TEXT PRIMARY KEY, id TEXT NOT NULL UNIQUE, scopes TEXT NOT NULL DEFAULT ''
            );

-- table google_task_accounts
CREATE TABLE google_task_accounts (
    email TEXT PRIMARY KEY,
    last_error TEXT NOT NULL DEFAULT '',
    last_sync_at INTEGER NOT NULL DEFAULT 0
);

-- table google_task_items
CREATE TABLE google_task_items (
    id TEXT PRIMARY KEY,
    list_id TEXT NOT NULL REFERENCES google_task_lists(id) ON DELETE CASCADE,
    remote_id TEXT,
    etag TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT '',
    UNIQUE(list_id, remote_id)
);

-- table google_task_lists
CREATE TABLE google_task_lists (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL REFERENCES google_task_accounts(email) ON DELETE CASCADE,
    remote_id TEXT NOT NULL,
    UNIQUE(email, remote_id)
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

-- table openpgp_accounts
CREATE TABLE openpgp_accounts (
  account_id TEXT PRIMARY KEY,
  enabled    INTEGER NOT NULL DEFAULT 0
);

-- table openpgp_key_used
CREATE TABLE openpgp_key_used (
  addr        TEXT PRIMARY KEY,
  fingerprint TEXT NOT NULL,
  used_at     INTEGER NOT NULL
);

-- table openpgp_offer_dismissed
CREATE TABLE openpgp_offer_dismissed (
  account_id TEXT NOT NULL,
  addr       TEXT NOT NULL,
  PRIMARY KEY (account_id, addr)
);

-- table openpgp_own_keys
CREATE TABLE openpgp_own_keys (
  fingerprint TEXT PRIMARY KEY,
  account_id  TEXT NOT NULL,
  secret_key  BLOB NOT NULL,
  is_default  INTEGER NOT NULL DEFAULT 1,
  added_at    INTEGER NOT NULL
);

-- table openpgp_peer_keys
CREATE TABLE openpgp_peer_keys (
  addr        TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  public_key  BLOB NOT NULL,
  source      TEXT NOT NULL,
  verified    INTEGER NOT NULL DEFAULT 0,
  first_seen  INTEGER NOT NULL,
  last_seen   INTEGER NOT NULL,
  PRIMARY KEY (addr, fingerprint)
);

-- table openpgp_sign_plain
CREATE TABLE openpgp_sign_plain (
  account_id TEXT PRIMARY KEY
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

-- table rule_activation
CREATE TABLE rule_activation (rule_id TEXT PRIMARY KEY, event_sequence INTEGER NOT NULL) WITHOUT ROWID;

-- table rule_evaluations
CREATE TABLE rule_evaluations (
            message INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            event_sequence INTEGER NOT NULL,
            query TEXT NOT NULL,
            PRIMARY KEY (message, rule_id)
         ) WITHOUT ROWID;

-- table rule_event_sequence
CREATE TABLE rule_event_sequence (id INTEGER PRIMARY KEY CHECK (id = 1), value INTEGER NOT NULL);

-- table rule_events
CREATE TABLE rule_events (
            message INTEGER PRIMARY KEY,
            arrived_at INTEGER,
            read_at INTEGER
         , arrived_sequence INTEGER NOT NULL DEFAULT 1, read_sequence INTEGER NOT NULL DEFAULT 1);

-- table rule_firings
CREATE TABLE rule_firings (
            message INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            PRIMARY KEY (message, rule_id)
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

-- index contact_participants_date_idx
CREATE INDEX contact_participants_date_idx
           ON contact_participants(account, folder, addr_key, name, date DESC);

-- index dav_objects_local_idx
CREATE INDEX dav_objects_local_idx ON dav_objects(local_id) WHERE local_id <> '';

-- index events_calendar_idx
CREATE INDEX events_calendar_idx ON events(calendar) WHERE calendar <> '';

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

-- index openpgp_own_keys_account
CREATE INDEX openpgp_own_keys_account ON openpgp_own_keys(account_id);

-- index subscriptions_account_idx
CREATE INDEX subscriptions_account_idx ON subscriptions(account);

-- index tasks_list_idx
CREATE INDEX tasks_list_idx ON tasks(list_id, sort_order);

-- index tasks_thread_idx
CREATE INDEX tasks_thread_idx ON tasks(thread_id);

-- trigger contact_participants_ad
CREATE TRIGGER contact_participants_ad AFTER DELETE ON contact_participants BEGIN
           DELETE FROM contacts
             WHERE account = old.account AND folder = old.folder
               AND addr_key = old.addr_key AND name = old.name AND count = 1;
           UPDATE contacts SET count = count - 1,
             last_date = CASE WHEN last_date = old.date THEN (
               SELECT date FROM contact_participants
               WHERE account = old.account AND folder = old.folder
                 AND addr_key = old.addr_key AND name = old.name
               ORDER BY date DESC LIMIT 1
             ) ELSE last_date END
             WHERE account = old.account AND folder = old.folder
               AND addr_key = old.addr_key AND name = old.name;
         END;

-- trigger contact_participants_ai
CREATE TRIGGER contact_participants_ai AFTER INSERT ON contact_participants BEGIN
           INSERT INTO contacts(account, folder, addr_key, name, addr, count, last_date)
           VALUES(new.account, new.folder, new.addr_key, new.name, new.addr, 1, new.date)
           ON CONFLICT(account, folder, addr_key, name) DO UPDATE SET
             count = count + 1, addr = max(addr, excluded.addr),
             last_date = max(last_date, excluded.last_date);
         END;

-- trigger contacts_ad
CREATE TRIGGER contacts_ad AFTER DELETE ON messages BEGIN
           DELETE FROM contact_participants WHERE message_id = old.id;
         END;

-- trigger contacts_ai
CREATE TRIGGER contacts_ai AFTER INSERT ON messages BEGIN
           INSERT INTO contact_participants(message_id, account, folder, addr_key, name, addr, date)
           SELECT new.id, new.account, new.folder, lower(trim(addr)), trim(COALESCE(name, '')),
                  max(trim(addr)), new.date
           FROM (SELECT new.from_name AS name, new.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.cc') WHERE type = 'object') WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
           GROUP BY lower(trim(addr)), trim(COALESCE(name, ''));
         END;

-- trigger contacts_au
CREATE TRIGGER contacts_au
           AFTER UPDATE OF id, account, folder, uid, from_name, from_addr, json, date ON messages
           WHEN old.id IS NOT new.id OR old.account IS NOT new.account
             OR old.folder IS NOT new.folder OR (old.uid <> 0) IS NOT (new.uid <> 0)
             OR old.from_name IS NOT new.from_name OR old.from_addr IS NOT new.from_addr
             OR json_extract(old.json, '$.to', '$.cc') IS NOT json_extract(new.json, '$.to', '$.cc')
             OR old.date IS NOT new.date
         BEGIN
           DELETE FROM contact_participants WHERE message_id = old.id;
           INSERT INTO contact_participants(message_id, account, folder, addr_key, name, addr, date)
           SELECT new.id, new.account, new.folder, lower(trim(addr)), trim(COALESCE(name, '')),
                  max(trim(addr)), new.date
           FROM (SELECT new.from_name AS name, new.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.cc') WHERE type = 'object') WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
           GROUP BY lower(trim(addr)), trim(COALESCE(name, ''));
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

-- trigger rule_arrival_au
CREATE TRIGGER rule_arrival_au AFTER UPDATE OF arrived_at ON rule_events
         WHEN old.arrived_at IS NULL AND new.arrived_at IS NOT NULL BEGIN
            UPDATE rule_event_sequence SET value = value + 1;
            UPDATE rule_events SET arrived_sequence = (SELECT value FROM rule_event_sequence) WHERE message = new.message;
         END;

-- trigger rule_evaluations_au
CREATE TRIGGER rule_evaluations_au AFTER UPDATE OF subject, from_name, from_addr, date, json, files, recipients, seen, starred ON messages
         WHEN new.subject IS NOT old.subject OR new.from_name IS NOT old.from_name
           OR new.from_addr IS NOT old.from_addr OR new.date IS NOT old.date
           OR new.json IS NOT old.json OR new.files IS NOT old.files
           OR new.recipients IS NOT old.recipients OR new.seen IS NOT old.seen
           OR new.starred IS NOT old.starred BEGIN
            DELETE FROM rule_evaluations WHERE message = new.id;
         END;

-- trigger rule_events_ad
CREATE TRIGGER rule_events_ad AFTER DELETE ON messages BEGIN
            DELETE FROM rule_events WHERE message = old.id;
            DELETE FROM rule_firings WHERE message = old.id;
            DELETE FROM rule_evaluations WHERE message = old.id;
         END;

-- trigger rule_events_ai
CREATE TRIGGER rule_events_ai AFTER INSERT ON messages WHEN new.uid <> 0 BEGIN
            INSERT INTO rule_events (message) VALUES (new.id);
         END;

-- trigger rule_events_au
CREATE TRIGGER rule_events_au AFTER UPDATE OF seen ON messages
         WHEN new.uid <> 0 AND old.seen <> new.seen BEGIN
            UPDATE rule_event_sequence SET value = value + 1 WHERE new.seen <> 0;
            UPDATE rule_events SET
                read_at = CASE WHEN new.seen <> 0 THEN unixepoch() ELSE NULL END,
                read_sequence = (SELECT value FROM rule_event_sequence)
            WHERE message = new.id;
         END;

-- trigger rule_tag_evaluations_ad
CREATE TRIGGER rule_tag_evaluations_ad AFTER DELETE ON message_tags BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = old.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = old.tag_key);
         END;

-- trigger rule_tag_evaluations_ai
CREATE TRIGGER rule_tag_evaluations_ai AFTER INSERT ON message_tags BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = new.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = new.tag_key);
         END;

-- trigger rule_tag_evaluations_au
CREATE TRIGGER rule_tag_evaluations_au AFTER UPDATE ON message_tags
         WHEN new.state IS NOT old.state OR new.tag IS NOT old.tag
           OR new.account IS NOT old.account OR new.tag_key IS NOT old.tag_key BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = old.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = old.tag_key
                 UNION SELECT id FROM messages WHERE uid <> 0 AND account = new.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = new.tag_key);
         END;
