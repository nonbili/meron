-- Sequence numbers distinguish events before and after a rule is saved even
-- within the same second. Existing v25 clocks remain eligible for saved rules.
CREATE TABLE rule_event_sequence (id INTEGER PRIMARY KEY CHECK (id = 1), value INTEGER NOT NULL);
         INSERT INTO rule_event_sequence VALUES (1, 1);
         ALTER TABLE rule_events ADD COLUMN arrived_sequence INTEGER NOT NULL DEFAULT 1;
         ALTER TABLE rule_events ADD COLUMN read_sequence INTEGER NOT NULL DEFAULT 1;
         CREATE TABLE rule_activation (rule_id TEXT PRIMARY KEY, event_sequence INTEGER NOT NULL) WITHOUT ROWID;
         CREATE TABLE rule_evaluations (
            message INTEGER NOT NULL,
            rule_id TEXT NOT NULL,
            event_sequence INTEGER NOT NULL,
            query TEXT NOT NULL,
            PRIMARY KEY (message, rule_id)
         ) WITHOUT ROWID;
         CREATE TRIGGER rule_arrival_au AFTER UPDATE OF arrived_at ON rule_events
         WHEN old.arrived_at IS NULL AND new.arrived_at IS NOT NULL BEGIN
            UPDATE rule_event_sequence SET value = value + 1;
            UPDATE rule_events SET arrived_sequence = (SELECT value FROM rule_event_sequence) WHERE message = new.message;
         END;
         DROP TRIGGER rule_events_au;
         CREATE TRIGGER rule_events_au AFTER UPDATE OF seen ON messages
         WHEN new.uid <> 0 AND old.seen <> new.seen BEGIN
            UPDATE rule_event_sequence SET value = value + 1 WHERE new.seen <> 0;
            UPDATE rule_events SET
                read_at = CASE WHEN new.seen <> 0 THEN unixepoch() ELSE NULL END,
                read_sequence = (SELECT value FROM rule_event_sequence)
            WHERE message = new.id;
         END;
         DROP TRIGGER rule_events_ad;
         CREATE TRIGGER rule_events_ad AFTER DELETE ON messages BEGIN
            DELETE FROM rule_events WHERE message = old.id;
            DELETE FROM rule_firings WHERE message = old.id;
            DELETE FROM rule_evaluations WHERE message = old.id;
         END;
