CREATE TABLE speech_assets (
    cache_key TEXT PRIMARY KEY,
    broadcast_id INTEGER NOT NULL REFERENCES content_broadcasts(id),
    model TEXT NOT NULL,
    voice TEXT NOT NULL,
    resource_id TEXT NOT NULL UNIQUE,
    local_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL,
    channels INTEGER NOT NULL,
    bits_per_sample INTEGER NOT NULL,
    segment_count INTEGER NOT NULL,
    transcript TEXT NOT NULL,
    verification TEXT NOT NULL
);
CREATE INDEX speech_assets_broadcast ON speech_assets(broadcast_id);
