-- Prepared scripts and verified PCM survive failures in later stages.
CREATE TABLE pipeline_preparations (
    broadcast_id INTEGER PRIMARY KEY REFERENCES content_broadcasts(id),
    settings_key TEXT NOT NULL
);
CREATE TABLE playback_receipts (
    broadcast_id INTEGER PRIMARY KEY REFERENCES content_broadcasts(id),
    state TEXT NOT NULL CHECK(state IN ('attempted','completed')),
    updated_at TEXT NOT NULL
);
CREATE TABLE speech_segments (
    cache_key TEXT PRIMARY KEY,
    pcm BLOB NOT NULL,
    transcript TEXT NOT NULL,
    pcm_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
