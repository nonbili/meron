-- An event's own alerts (JSON minutes before start); NULL follows the
-- app-wide default.
ALTER TABLE events ADD COLUMN alerts TEXT;
