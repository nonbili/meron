-- Stable message identities we've already seen, used to suppress "new mail"
-- notifications when Gmail restores an older message with a fresh INBOX UID.

CREATE TABLE IF NOT EXISTS observed_mail_identities (
  account       TEXT NOT NULL,
  identity      TEXT NOT NULL,
  first_seen_at INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, identity)
);
