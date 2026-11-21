-- Access paths for thread-key inheritance, which until now scanned the whole
-- account cache per upserted reply. Thread-key inheritance looks a message up
-- by its cached Message-ID, and looks up the rows whose thread key names one,
-- both compared case-insensitively, and neither a plain column, since the id
-- lives in the `json` catch-all. The index expressions must stay
-- character-identical to the ones in `store::resolve_message_thread_key` /
-- `store::reconcile_thread_keys_from`, or SQLite won't match a query to them.

CREATE INDEX IF NOT EXISTS messages_message_id_idx
  ON messages(account, lower(COALESCE(json_extract(json, '$.message_id'), '')));
CREATE INDEX IF NOT EXISTS messages_thread_key_idx
  ON messages(account, lower(COALESCE(thread_key, '')));
