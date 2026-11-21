-- Folder-scoped contacts let readers apply the *current* folder roles, even
-- when LIST metadata arrives after the mail. Participant rows are a compact
-- reverse index: removing the newest message can find the next date with an
-- indexed lookup rather than scanning/parsing that correspondent's mail.
DROP TRIGGER IF EXISTS contacts_ai;
         DROP TRIGGER IF EXISTS contacts_ad;
         DROP TRIGGER IF EXISTS contacts_au;
         DROP TABLE contacts;
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
INSERT INTO contact_participants(message_id, account, folder, addr_key, name, addr, date)
           SELECT id, account, folder, lower(trim(addr)), trim(COALESCE(name, '')),
                  max(trim(addr)), date
           FROM (
             SELECT id, account, folder, date, from_name AS name, from_addr AS addr
             FROM messages WHERE uid <> 0
             UNION ALL SELECT m.id, m.account, m.folder, m.date,
                    json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
             FROM messages m, json_each(m.json, '$.to') e
             WHERE m.uid <> 0 AND e.type = 'object' UNION ALL SELECT m.id, m.account, m.folder, m.date,
                    json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
             FROM messages m, json_each(m.json, '$.cc') e
             WHERE m.uid <> 0 AND e.type = 'object'
           )
           WHERE trim(COALESCE(addr, '')) <> ''
           GROUP BY id, lower(trim(addr)), trim(COALESCE(name, ''));
         CREATE INDEX contact_participants_date_idx
           ON contact_participants(account, folder, addr_key, name, date DESC);
         INSERT INTO contacts(account, folder, addr_key, name, addr, count, last_date)
           SELECT account, folder, addr_key, name, max(addr), count(*), max(date)
           FROM contact_participants GROUP BY account, folder, addr_key, name;
CREATE TRIGGER contact_participants_ai AFTER INSERT ON contact_participants BEGIN
           INSERT INTO contacts(account, folder, addr_key, name, addr, count, last_date)
           VALUES(new.account, new.folder, new.addr_key, new.name, new.addr, 1, new.date)
           ON CONFLICT(account, folder, addr_key, name) DO UPDATE SET
             count = count + 1, addr = max(addr, excluded.addr),
             last_date = max(last_date, excluded.last_date);
         END;
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
         CREATE TRIGGER contacts_ad AFTER DELETE ON messages BEGIN
           DELETE FROM contact_participants WHERE message_id = old.id;
         END;
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
