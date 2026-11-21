-- Avoid reindexing unchanged headers and observe cached identities immediately.
-- Sync classifies arrivals before writing its fetched batch.
DROP TRIGGER messages_au;
         CREATE TRIGGER messages_au AFTER UPDATE OF subject, from_name, from_addr, body ON messages
         WHEN old.subject IS NOT new.subject OR old.from_name IS NOT new.from_name
           OR old.from_addr IS NOT new.from_addr OR old.body IS NOT new.body
         BEGIN
           INSERT INTO messages_fts(messages_fts, rowid, subject, from_name, from_addr, body)
           VALUES ('delete', old.id, old.subject, old.from_name, old.from_addr, old.body);
           INSERT INTO messages_fts(rowid, subject, from_name, from_addr, body)
           VALUES (new.id, new.subject, new.from_name, new.from_addr, new.body);
         END;
         DROP TRIGGER messages_recipients_au;
         CREATE TRIGGER messages_recipients_au AFTER UPDATE OF recipients ON messages
         WHEN old.recipients IS NOT new.recipients
         BEGIN
           INSERT INTO messages_recipients_fts(messages_recipients_fts, rowid, recipients)
           VALUES ('delete', old.id, old.recipients);
           INSERT INTO messages_recipients_fts(rowid, recipients) VALUES (new.id, new.recipients);
         END;
INSERT OR IGNORE INTO observed_mail_identities(account, identity, first_seen_at)
         SELECT account, CASE
           WHEN json_extract(json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(json, '$.gmail_msg_id')
           WHEN COALESCE(json_extract(json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(json, '$.message_id'))
           ELSE 'uid:' || lower(folder) || ':' || uid END, unixepoch()
         FROM messages WHERE uid <> 0;
         DROP TRIGGER IF EXISTS pending_mail_identity_ai;
         DROP TRIGGER IF EXISTS pending_mail_identity_au;
         DROP TRIGGER IF EXISTS pending_mail_identity_ad;
         DROP TABLE IF EXISTS pending_mail_identities;
         DROP TRIGGER IF EXISTS observe_mail_identity_ai;
         DROP TRIGGER IF EXISTS observe_mail_identity_au;
CREATE TRIGGER observe_mail_identity_ai AFTER INSERT ON messages WHEN new.uid <> 0 BEGIN
             INSERT INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(new.account, CASE
               WHEN json_extract(new.json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(new.json, '$.gmail_msg_id')
               WHEN COALESCE(json_extract(new.json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(new.json, '$.message_id'))
               ELSE 'uid:' || lower(new.folder) || ':' || new.uid END, unixepoch())
             ON CONFLICT DO NOTHING;
             END;
CREATE TRIGGER observe_mail_identity_au AFTER UPDATE OF json, uid, folder, account ON messages WHEN new.uid <> 0 AND (old.json IS NOT new.json OR old.uid IS NOT new.uid OR old.folder IS NOT new.folder OR old.account IS NOT new.account) BEGIN
             INSERT INTO observed_mail_identities(account, identity, first_seen_at)
             VALUES(new.account, CASE
               WHEN json_extract(new.json, '$.gmail_msg_id') IS NOT NULL THEN 'gmail:' || json_extract(new.json, '$.gmail_msg_id')
               WHEN COALESCE(json_extract(new.json, '$.message_id'), '') <> '' THEN 'message-id:' || lower(json_extract(new.json, '$.message_id'))
               ELSE 'uid:' || lower(new.folder) || ':' || new.uid END, unixepoch())
             ON CONFLICT DO NOTHING;
             END;
