CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    preferred_audio_id TEXT,
    preferred_audio_name TEXT,
    interval_ms INTEGER NOT NULL DEFAULT 1800000 CHECK (interval_ms BETWEEN 60000 AND 86400000),
    duck_volume REAL NOT NULL DEFAULT 0.15 CHECK (duck_volume BETWEEN 0 AND 1),
    fade_out_ms INTEGER NOT NULL DEFAULT 3000 CHECK (fade_out_ms BETWEEN 0 AND 30000),
    fade_in_ms INTEGER NOT NULL DEFAULT 3000 CHECK (fade_in_ms BETWEEN 0 AND 30000),
    narrator_volume REAL NOT NULL DEFAULT 0.7 CHECK (narrator_volume BETWEEN 0 AND 1),
    broadcast_min_ms INTEGER NOT NULL DEFAULT 120000 CHECK (broadcast_min_ms >= 30000),
    broadcast_max_ms INTEGER NOT NULL DEFAULT 180000 CHECK (broadcast_max_ms BETWEEN broadcast_min_ms AND 900000),
    openai_voice TEXT NOT NULL DEFAULT 'alloy',
    auto_start INTEGER NOT NULL DEFAULT 0 CHECK (auto_start IN (0, 1)),
    CHECK ((preferred_audio_id IS NULL) = (preferred_audio_name IS NULL))
);
CREATE TABLE settings_topics (
    topic TEXT PRIMARY KEY NOT NULL,
    position INTEGER NOT NULL UNIQUE
);
INSERT INTO settings (id) VALUES (1);
