-- Tags (see `store::tags`). `server_tags` is what a mailbox copy last showed,
-- NULL until it has been looked at. The key lives in an expression index
-- rather than a column so every path that writes a message row is covered.
-- Rules (see `store::rules`): `rule_actions` is what they still have to do on
-- the server, so a failed sync keeps it, and `rule_log` what they did.
ALTER TABLE messages ADD COLUMN server_tags TEXT;
         CREATE INDEX messages_tag_key_idx ON messages(account, COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid)) WHERE uid <> 0;
         CREATE TABLE message_tags (
            account    TEXT NOT NULL,
            tag_key    TEXT NOT NULL,
            tag        TEXT NOT NULL,
            state      INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (account, tag_key, tag)
         ) WITHOUT ROWID;
         CREATE INDEX message_tags_tag_idx ON message_tags(account, tag);
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
