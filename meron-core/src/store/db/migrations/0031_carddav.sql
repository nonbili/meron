-- CardDAV address books: accounts, their books, and each synced card.
CREATE TABLE IF NOT EXISTS carddav_accounts (
                id TEXT PRIMARY KEY, server TEXT NOT NULL, username TEXT NOT NULL,
                google INTEGER NOT NULL DEFAULT 0, UNIQUE(server, username)
            );
            CREATE TABLE IF NOT EXISTS carddav_books (
                id TEXT PRIMARY KEY, account TEXT NOT NULL REFERENCES carddav_accounts(id) ON DELETE CASCADE,
                url TEXT NOT NULL, name TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1,
                sync_token TEXT NOT NULL DEFAULT '', UNIQUE(account, url)
            );
            CREATE TABLE IF NOT EXISTS carddav_cards (
                book TEXT NOT NULL REFERENCES carddav_books(id) ON DELETE CASCADE,
                url TEXT NOT NULL, etag TEXT NOT NULL, name TEXT NOT NULL, emails TEXT NOT NULL,
                invalid INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(book, url)
            );
