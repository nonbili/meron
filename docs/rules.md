# Mail rules

In Settings → Rules, choose the conditions, actions, and **When to run**. Every choice except **Right away** takes an amount in minutes, hours, or days:

- **Right away** runs as new mail arrives.
- **After it arrives** starts the wait when new mail is observed. Existing rules keep their immediate arrival behavior. The initial mailbox download does not count as new mail.
- **After it's read** starts the wait when this device observes an unread message becoming read, including changes synced from another client. Already-read mail on the initial download does not start a delay. Marking it unread before the deadline cancels the clock; reading it again starts a new one.
- **When it's older than** uses the email's date and includes older mail cached in the Inbox. Mail with an unknown date is skipped.

For one-time codes, match the sender or subject, choose **After it's read**, enter **5** minutes, and choose **Move to → Trash**. For automatic archiving after 30 days, choose **When it's older than**, enter **30** days, and choose **Move to → Archive**.

Rules are checked during mailbox sync, so actions can occur later than their deadline while the app is closed, offline, or waiting for its next sync. Event clocks survive restarts and envelope refreshes. Conditions and account scope are checked when the rule is due. Arrival and read rules only consider events that happen after the rule is created, re-enabled, has its account scope expanded, or has its trigger or delay changed. Expanding account scope starts a fresh event window for all accounts covered by that rule. Each delayed rule runs once per cached message. Nonmatches are cached until the conditions, event, or message state change. Read state, star, tag, attachment, and header changes invalidate cached checks so the next sync evaluates them again. Sent, Drafts, Trash, and Junk are excluded.

Editing or disabling a rule before it becomes due changes what will run. Once actions are queued, they use the existing retry mechanism. **Run on existing mail** applies the rule immediately to cached Inbox mail, independently of its trigger and delay.
