CREATE TABLE IF NOT EXISTS google_task_accounts (
    email TEXT PRIMARY KEY,
    last_error TEXT NOT NULL DEFAULT '',
    last_sync_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS google_task_lists (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL REFERENCES google_task_accounts(email) ON DELETE CASCADE,
    remote_id TEXT NOT NULL,
    UNIQUE(email, remote_id)
);
CREATE TABLE IF NOT EXISTS google_task_items (
    id TEXT PRIMARY KEY,
    list_id TEXT NOT NULL REFERENCES google_task_lists(id) ON DELETE CASCADE,
    remote_id TEXT,
    etag TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT '',
    UNIQUE(list_id, remote_id)
);
