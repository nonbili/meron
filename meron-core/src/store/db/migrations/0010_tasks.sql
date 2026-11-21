-- Tasks: lists and their items, for the optional Tasks surface. Local-only —
-- nothing here is synced to a server, so the rows are also carried by the
-- config backup.

CREATE TABLE IF NOT EXISTS task_lists (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS tasks (
  id           TEXT PRIMARY KEY,
  list_id      TEXT NOT NULL,
  title        TEXT NOT NULL DEFAULT '',
  notes        TEXT NOT NULL DEFAULT '',
  due_at       INTEGER NOT NULL DEFAULT 0,   -- 0 = no due date
  completed_at INTEGER NOT NULL DEFAULT 0,   -- 0 = not done
  sort_order   INTEGER NOT NULL DEFAULT 0,
  account      TEXT NOT NULL DEFAULT '',     -- mail link; empty for a plain task
  thread_id    TEXT NOT NULL DEFAULT '',
  message_id   TEXT NOT NULL DEFAULT '',
  json         TEXT NOT NULL DEFAULT '{}',
  created_at   INTEGER NOT NULL DEFAULT 0,
  updated_at   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS tasks_list_idx ON tasks(list_id, sort_order);
CREATE INDEX IF NOT EXISTS tasks_thread_idx ON tasks(thread_id);
