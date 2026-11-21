-- A later envelope or metadata fill can correct headers used by a rule.
-- Reuse negative checks only while those values remain unchanged.
CREATE TRIGGER rule_evaluations_au AFTER UPDATE OF subject, from_name, from_addr, date, json, files ON messages
         WHEN new.subject IS NOT old.subject OR new.from_name IS NOT old.from_name
           OR new.from_addr IS NOT old.from_addr OR new.date IS NOT old.date
           OR new.json IS NOT old.json OR new.files IS NOT old.files BEGIN
            DELETE FROM rule_evaluations WHERE message = new.id;
         END;
