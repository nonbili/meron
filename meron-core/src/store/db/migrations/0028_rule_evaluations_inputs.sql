-- Negative checks remain reusable for mutable conditions until their inputs
-- change. Tag keys cover all mailbox copies carrying the same Message-ID.
DROP TRIGGER rule_evaluations_au;
         CREATE TRIGGER rule_evaluations_au AFTER UPDATE OF subject, from_name, from_addr, date, json, files, recipients, seen, starred ON messages
         WHEN new.subject IS NOT old.subject OR new.from_name IS NOT old.from_name
           OR new.from_addr IS NOT old.from_addr OR new.date IS NOT old.date
           OR new.json IS NOT old.json OR new.files IS NOT old.files
           OR new.recipients IS NOT old.recipients OR new.seen IS NOT old.seen
           OR new.starred IS NOT old.starred BEGIN
            DELETE FROM rule_evaluations WHERE message = new.id;
         END;
         CREATE TRIGGER rule_tag_evaluations_ai AFTER INSERT ON message_tags BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = new.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = new.tag_key);
         END;
         CREATE TRIGGER rule_tag_evaluations_ad AFTER DELETE ON message_tags BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = old.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = old.tag_key);
         END;
         CREATE TRIGGER rule_tag_evaluations_au AFTER UPDATE ON message_tags
         WHEN new.state IS NOT old.state OR new.tag IS NOT old.tag
           OR new.account IS NOT old.account OR new.tag_key IS NOT old.tag_key BEGIN
            DELETE FROM rule_evaluations WHERE message IN
                (SELECT id FROM messages WHERE uid <> 0 AND account = old.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = old.tag_key
                 UNION SELECT id FROM messages WHERE uid <> 0 AND account = new.account AND COALESCE(NULLIF(lower(trim(COALESCE(json_extract(json, '$.message_id'), ''), ' <>')), ''), 'uid:' || folder || ':' || uid) = new.tag_key);
         END;
