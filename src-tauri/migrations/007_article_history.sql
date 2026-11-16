ALTER TABLE article_materials ADD COLUMN input_fingerprint TEXT;
CREATE INDEX article_materials_url ON article_materials(publisher_url);
-- Retain only hashes of material, not publisher article bodies, with broadcast history.
CREATE TABLE article_history (
    broadcast_id INTEGER NOT NULL REFERENCES content_broadcasts(id),
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    publisher_url TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    fingerprints_json TEXT NOT NULL,
    PRIMARY KEY (broadcast_id, article_id)
);
