-- Calendar invitations the user answered from the card, keyed
-- by the event rather than the message so every copy of the invite — the
-- original, an update, the one in another folder — shows the same answer.
-- `sequence` is the revision answered: a later one asks again.
CREATE TABLE IF NOT EXISTS invite_responses (
           account       TEXT NOT NULL,
           uid           TEXT NOT NULL,     -- iCalendar UID
           recurrence_id INTEGER NOT NULL DEFAULT 0,   -- 0 = the whole series
           response      TEXT NOT NULL,     -- accepted | tentative | declined
           sequence      INTEGER NOT NULL DEFAULT 0,
           responded_at  INTEGER NOT NULL DEFAULT 0,
           PRIMARY KEY (account, uid, recurrence_id)
         );
