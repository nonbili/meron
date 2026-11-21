-- One Google authorization per address, shared by Calendar and
-- Contacts (`google_grant`). Its refresh token is a secret under `id`,
-- random so profiles sharing a keychain keep apart; `scopes` are the
-- ones Google last reported for it.
CREATE TABLE IF NOT EXISTS google_grants (
                email TEXT PRIMARY KEY, id TEXT NOT NULL UNIQUE, scopes TEXT NOT NULL DEFAULT ''
            );
