ALTER TABLE settings ADD COLUMN start_scheduler_on_launch INTEGER NOT NULL DEFAULT 0 CHECK (start_scheduler_on_launch IN (0, 1));

-- Rust writes the original volume and both sides of an in-flight Core Audio write
-- durably BEFORE changing the selected session. Never store process-wide volume.
CREATE TABLE audio_recovery (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    session_id TEXT NOT NULL,
    original REAL NOT NULL CHECK (original BETWEEN 0 AND 1),
    expected REAL NOT NULL CHECK (expected BETWEEN 0 AND 1),
    pending REAL NOT NULL CHECK (pending BETWEEN 0 AND 1)
);
