-- Per-folder opt-in to live sync and new-mail notifications. INBOX is always
-- watched and never consults the flag.
ALTER TABLE folders ADD COLUMN notify INTEGER NOT NULL DEFAULT 0;
