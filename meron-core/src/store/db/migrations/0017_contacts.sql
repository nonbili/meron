-- Recipient autocomplete's directory: one row per account, address and
-- display name seen as a mail message's sender, To or Cc, counting the
-- messages it appears on. Triggers on `messages` keep it in step with every
-- write path. Replaced by 0024.
CREATE TABLE IF NOT EXISTS contacts (
           account  TEXT NOT NULL,
           addr_key TEXT NOT NULL,   -- lower(trim(addr))
           name     TEXT NOT NULL,   -- '' when the address came without one
           addr     TEXT NOT NULL,
           count    INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (account, addr_key, name)
         ) WITHOUT ROWID;
         CREATE TRIGGER IF NOT EXISTS contacts_ai AFTER INSERT ON messages BEGIN
           INSERT INTO contacts(account, addr_key, name, addr, count)
               SELECT new.account, lower(trim(addr)), trim(COALESCE(name, '')), max(trim(addr)), 1
               FROM (SELECT new.from_name AS name, new.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.cc') WHERE type = 'object') WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
               GROUP BY lower(trim(addr)), trim(COALESCE(name, ''))
             ON CONFLICT(account, addr_key, name) DO UPDATE SET count = count + 1;
         END;
         CREATE TRIGGER IF NOT EXISTS contacts_ad AFTER DELETE ON messages BEGIN
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
         CREATE TRIGGER IF NOT EXISTS contacts_au
           AFTER UPDATE OF account, uid, from_name, from_addr, json ON messages
           WHEN old.account IS NOT new.account
             OR (old.uid <> 0) IS NOT (new.uid <> 0)
             OR old.from_name IS NOT new.from_name
             OR old.from_addr IS NOT new.from_addr
             OR json_extract(old.json, '$.to', '$.cc') IS NOT json_extract(new.json, '$.to', '$.cc')
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
           INSERT INTO contacts(account, addr_key, name, addr, count)
               SELECT new.account, lower(trim(addr)), trim(COALESCE(name, '')), max(trim(addr)), 1
               FROM (SELECT new.from_name AS name, new.from_addr AS addr
         UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.to') WHERE type = 'object' UNION ALL SELECT json_extract(value, '$.name'), json_extract(value, '$.addr')
               FROM json_each(new.json, '$.cc') WHERE type = 'object') WHERE new.uid <> 0 AND trim(COALESCE(addr, '')) <> ''
               GROUP BY lower(trim(addr)), trim(COALESCE(name, ''))
             ON CONFLICT(account, addr_key, name) DO UPDATE SET count = count + 1;
         END;
INSERT INTO contacts(account, addr_key, name, addr, count)
           SELECT account, addr_key, name, max(addr), count(*) FROM (
             SELECT id, account, lower(trim(addr)) AS addr_key, trim(COALESCE(name, '')) AS name,
                    max(trim(addr)) AS addr
             FROM (
               SELECT id, account, from_name AS name, from_addr AS addr FROM messages WHERE uid <> 0
               UNION ALL SELECT m.id, m.account, json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
               FROM messages m, json_each(m.json, '$.to') e
               WHERE m.uid <> 0 AND e.type = 'object' UNION ALL SELECT m.id, m.account, json_extract(e.value, '$.name'), json_extract(e.value, '$.addr')
               FROM messages m, json_each(m.json, '$.cc') e
               WHERE m.uid <> 0 AND e.type = 'object'
             )
             WHERE trim(COALESCE(addr, '')) <> ''
             GROUP BY id, lower(trim(addr)), trim(COALESCE(name, ''))
           )
           WHERE true
           GROUP BY account, addr_key, name;
