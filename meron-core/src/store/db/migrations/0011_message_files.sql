-- `messages.files`: the cached body's real attachments as a JSON array of
-- `{filename, mime}`, NULL when it has none, so list cards can name them and
-- the attachments filter can page without parsing every row's `json`. Written
-- with header BODYSTRUCTURE metadata and refined by `save_cached_message`.
--
-- Rows cached before this are filled by `store::backfill_files_batch` after
-- startup, not here: deriving them parses every row's `json` (HTML bodies
-- included), which on a large cache holds the store long enough for the
-- frontend's first requests to time out.
ALTER TABLE messages ADD COLUMN files TEXT;
