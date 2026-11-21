-- Local notification delivery receipts; deliberately excluded from backups.
CREATE TABLE IF NOT EXISTS reminder_deliveries (
            key TEXT PRIMARY KEY,
            delivered_at INTEGER NOT NULL
         );
