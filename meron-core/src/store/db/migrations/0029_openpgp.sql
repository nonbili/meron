-- OpenPGP keys (nonbili/meron#46): own and peer keys, per-account switch,
-- dismissed encryption offers. See `openpgp::store`.

CREATE TABLE IF NOT EXISTS openpgp_accounts (
  account_id TEXT PRIMARY KEY,
  enabled    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS openpgp_own_keys (
  fingerprint TEXT PRIMARY KEY,
  account_id  TEXT NOT NULL,
  secret_key  BLOB NOT NULL,
  is_default  INTEGER NOT NULL DEFAULT 1,
  added_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS openpgp_own_keys_account ON openpgp_own_keys(account_id);
CREATE TABLE IF NOT EXISTS openpgp_peer_keys (
  addr        TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  public_key  BLOB NOT NULL,
  source      TEXT NOT NULL,
  verified    INTEGER NOT NULL DEFAULT 0,
  first_seen  INTEGER NOT NULL,
  last_seen   INTEGER NOT NULL,
  PRIMARY KEY (addr, fingerprint)
);
CREATE TABLE IF NOT EXISTS openpgp_offer_dismissed (
  account_id TEXT NOT NULL,
  addr       TEXT NOT NULL,
  PRIMARY KEY (account_id, addr)
);
CREATE TABLE IF NOT EXISTS openpgp_key_used (
  addr        TEXT PRIMARY KEY,
  fingerprint TEXT NOT NULL,
  used_at     INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS openpgp_sign_plain (
  account_id TEXT PRIMARY KEY
);
