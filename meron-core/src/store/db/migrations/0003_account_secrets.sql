-- Per-account secret blob (IMAP password / OAuth tokens) for platforms without
-- an OS keychain (Android, iOS sandbox). Desktop continues to use the keychain
-- via the `secrets` module; only the mobile FFI path reads/writes this table.
CREATE TABLE IF NOT EXISTS account_secrets (
           account_id TEXT PRIMARY KEY,
           blob       TEXT NOT NULL
         );
