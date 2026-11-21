-- UIDs this process removed from a folder. A folder sync that fetched its
-- snapshot before the removal must not write those rows back, or an archived
-- thread reappears in the mailbox it just left.
CREATE TABLE IF NOT EXISTS removed_message_uids (
           account TEXT NOT NULL,
           folder  TEXT NOT NULL,
           uid     INTEGER NOT NULL,
           epoch   INTEGER NOT NULL,
           PRIMARY KEY (account, folder, uid)
         ) WITHOUT ROWID;
