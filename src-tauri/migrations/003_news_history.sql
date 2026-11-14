CREATE TABLE news_articles (
    id INTEGER PRIMARY KEY,
    provider TEXT NOT NULL,
    guid TEXT,
    canonical_url TEXT NOT NULL UNIQUE,
    fingerprint TEXT UNIQUE,
    metadata_json TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    UNIQUE(provider, guid)
);
CREATE TABLE news_topic_articles (
    topic TEXT NOT NULL,
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    last_seen_at INTEGER NOT NULL,
    PRIMARY KEY(topic, article_id)
);
CREATE TABLE news_usage (
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    broadcast_id TEXT NOT NULL,
    used_at INTEGER NOT NULL,
    PRIMARY KEY(article_id, broadcast_id)
);
CREATE TABLE news_fetch_state (
    provider TEXT NOT NULL,
    topic TEXT NOT NULL,
    next_allowed_at INTEGER NOT NULL,
    last_error TEXT,
    PRIMARY KEY(provider, topic)
);
CREATE INDEX news_usage_article ON news_usage(article_id);
CREATE INDEX news_topic_seen ON news_topic_articles(last_seen_at);
CREATE TABLE news_aliases (
    provider TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('url', 'guid')),
    value TEXT NOT NULL,
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    metadata_json TEXT NOT NULL,
    PRIMARY KEY(provider, kind, value)
);
