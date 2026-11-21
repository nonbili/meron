-- v0.4.9 and v0.5.0 reached v29 before `openpgp_sign_plain` joined the
-- OpenPGP schema, and the engine created it at startup since. This creates it
-- for those stores instead; a store that already has it is left alone.
CREATE TABLE IF NOT EXISTS openpgp_sign_plain (
  account_id TEXT PRIMARY KEY
);
