ALTER TABLE settings ADD COLUMN text_model TEXT NOT NULL DEFAULT 'gpt-5-mini';
CREATE TABLE content_broadcasts (
    id INTEGER PRIMARY KEY,
    created_at TEXT NOT NULL,
    title TEXT NOT NULL,
    model TEXT NOT NULL,
    text TEXT NOT NULL,
    estimated_duration_ms INTEGER NOT NULL,
    topics_json TEXT NOT NULL,
    draft_json TEXT NOT NULL
);
CREATE TABLE content_sources (
    broadcast_id INTEGER NOT NULL REFERENCES content_broadcasts(id),
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    snapshot_json TEXT NOT NULL,
    PRIMARY KEY(broadcast_id, article_id)
);
CREATE INDEX content_sources_article ON content_sources(article_id);
CREATE TABLE content_evidence (
    broadcast_id INTEGER NOT NULL REFERENCES content_broadcasts(id),
    normalized_quote TEXT NOT NULL,
    PRIMARY KEY(broadcast_id, normalized_quote)
);
