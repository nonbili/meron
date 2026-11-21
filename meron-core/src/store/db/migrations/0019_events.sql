-- The local calendar: events the user made, and invitations
-- they accepted. Like tasks, a table rather than a blob, queried by time.
-- `recurring` rows are expanded on read from `rrule` (an event made here) or
-- `ics` (an invitation, whose own rules and exceptions are kept whole), so a
-- range query takes every recurring row that starts before its end.
CREATE TABLE IF NOT EXISTS events (
           id          TEXT PRIMARY KEY,
           title       TEXT NOT NULL DEFAULT '',
           notes       TEXT NOT NULL DEFAULT '',
           location    TEXT NOT NULL DEFAULT '',
           start_at    INTEGER NOT NULL DEFAULT 0,   -- all-day: UTC midnight
           end_at      INTEGER NOT NULL DEFAULT 0,   -- exclusive
           all_day     INTEGER NOT NULL DEFAULT 0,
           recurring   INTEGER NOT NULL DEFAULT 0,
           rrule       TEXT NOT NULL DEFAULT '',     -- daily | weekly | monthly | yearly
           tzid        TEXT NOT NULL DEFAULT '',     -- the zone a repeat keeps its time in
           account     TEXT NOT NULL DEFAULT '',     -- mail link; empty for an event made here
           thread_id   TEXT NOT NULL DEFAULT '',
           message_id  TEXT NOT NULL DEFAULT '',
           invite_uid  TEXT NOT NULL DEFAULT '',     -- iCalendar UID of an accepted invitation
           recurrence_id INTEGER NOT NULL DEFAULT 0, -- an invitation to one occurrence alone; 0 = the event
           response    TEXT NOT NULL DEFAULT '',     -- accepted | tentative
           ics         TEXT NOT NULL DEFAULT '',
           json        TEXT NOT NULL DEFAULT '{}',
           created_at  INTEGER NOT NULL DEFAULT 0,
           updated_at  INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX IF NOT EXISTS events_start_idx ON events(start_at);
         -- A decline (or cancellation) of an occurrence that stands for every
         -- later one too (RANGE=THISANDFUTURE).
         ALTER TABLE invite_responses ADD COLUMN this_and_future INTEGER NOT NULL DEFAULT 0;
         CREATE UNIQUE INDEX IF NOT EXISTS events_invite_idx
           ON events(account, invite_uid, recurrence_id) WHERE invite_uid <> '';
