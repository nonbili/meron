-- Per-subscription extra metadata that doesn't warrant its own typed column
-- (feed icon/logo, …), stored as a JSON object. Mirrors the `messages.json`
-- approach; defaults to an empty object so existing rows stay valid.
ALTER TABLE subscriptions ADD COLUMN json TEXT NOT NULL DEFAULT '{}';
