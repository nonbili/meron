-- Event clocks are independent of envelope JSON, which is replaced on sync.
-- Read transitions include bulk updates and changes observed from other clients.
CREATE TABLE rule_events (
            message INTEGER PRIMARY KEY,
            arrived_at INTEGER,
            read_at INTEGER
         );
         INSERT INTO rule_events (message) SELECT id FROM messages WHERE uid <> 0;
         CREATE TABLE rule_firings (
            message INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            PRIMARY KEY (message, rule_id)
         ) WITHOUT ROWID;
         CREATE TRIGGER rule_events_ai AFTER INSERT ON messages WHEN new.uid <> 0 BEGIN
            INSERT INTO rule_events (message) VALUES (new.id);
         END;
         CREATE TRIGGER rule_events_au AFTER UPDATE OF seen ON messages
         WHEN new.uid <> 0 AND old.seen <> new.seen BEGIN
            UPDATE rule_events SET read_at = CASE WHEN new.seen <> 0
                THEN unixepoch() ELSE NULL END WHERE message = new.id;
         END;
         CREATE TRIGGER rule_events_ad AFTER DELETE ON messages BEGIN
            DELETE FROM rule_events WHERE message = old.id;
            DELETE FROM rule_firings WHERE message = old.id;
         END;
