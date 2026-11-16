-- Keep the released history and aliases; stop treating same-title stories as duplicates.
UPDATE news_articles SET fingerprint=NULL;
CREATE TABLE article_queue (
    article_id INTEGER PRIMARY KEY REFERENCES news_articles(id),
    published_at INTEGER,
    failures INTEGER NOT NULL DEFAULT 0,
    retry_at INTEGER NOT NULL DEFAULT 0,
    skipped INTEGER NOT NULL DEFAULT 0 CHECK(skipped IN (0,1))
);
INSERT INTO article_queue(article_id,published_at)
SELECT id,unixepoch(json_extract(CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,'$.publishedAt')) FROM news_articles;
CREATE TABLE news_pool_state(topic TEXT PRIMARY KEY, newest_at INTEGER NOT NULL);
CREATE TABLE article_chapters (
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    config_key TEXT NOT NULL,
    chapter_json TEXT NOT NULL,
    access_json TEXT NOT NULL,
    PRIMARY KEY(article_id,config_key)
);
CREATE TABLE broadcast_chapters (
    broadcast_id INTEGER NOT NULL REFERENCES content_broadcasts(id),
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    position INTEGER NOT NULL,
    text TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'text_ready' CHECK(state IN ('text_ready','audio_ready','audio_failed')),
    start_ms INTEGER,
    end_ms INTEGER,
    PRIMARY KEY(broadcast_id,article_id),
    UNIQUE(broadcast_id,position)
);
CREATE INDEX article_queue_order ON article_queue(skipped,published_at,article_id);
