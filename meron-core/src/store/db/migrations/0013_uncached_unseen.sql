-- Unseen server UIDs that the last sync found outside the cached window, so a
-- folder's unread total covers mail older than what the cache holds. A row is
-- dropped once its message is cached there, whether inserted or moved in from
-- another folder: the cached row
-- then carries the unread state, and deleting or moving it later can't bring
-- the stale row back into the count.
CREATE TABLE IF NOT EXISTS uncached_unseen (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;
         CREATE TRIGGER IF NOT EXISTS messages_uncached_unseen_ai AFTER INSERT ON messages
         WHEN new.uid <> 0 BEGIN
           DELETE FROM uncached_unseen
            WHERE account = new.account AND folder = new.folder AND uid = new.uid;
         END;
         CREATE TRIGGER IF NOT EXISTS messages_uncached_unseen_au
         AFTER UPDATE OF account, folder, uid ON messages
         WHEN new.uid <> 0 BEGIN
           DELETE FROM uncached_unseen
            WHERE account = new.account AND folder = new.folder AND uid = new.uid;
         END;
