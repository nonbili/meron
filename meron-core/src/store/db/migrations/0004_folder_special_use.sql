-- RFC 6154 special-use role reported by LIST for a folder ("drafts", "sent",
-- …), NULL when the server doesn't advertise one. Lets role lookups (which
-- folder holds drafts?) trust the server over name heuristics.
ALTER TABLE folders ADD COLUMN special_use TEXT;
