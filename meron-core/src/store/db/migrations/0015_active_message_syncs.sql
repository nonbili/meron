-- Folder syncs with a snapshot in flight, shared by every connection so one
-- engine does not prune removal markers another engine's sync still needs.
CREATE TABLE IF NOT EXISTS active_message_syncs (
           id    INTEGER PRIMARY KEY AUTOINCREMENT,
           epoch INTEGER NOT NULL,
           owner TEXT NOT NULL
         );
