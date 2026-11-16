CREATE TABLE article_materials (
    article_id INTEGER PRIMARY KEY REFERENCES news_articles(id),
    publisher_url TEXT,
    title TEXT,
    publisher TEXT,
    published_at TEXT,
    body_text TEXT,
    status TEXT NOT NULL CHECK(status IN ('usable','skipped')),
    reason TEXT,
    fetched_at INTEGER NOT NULL
);
CREATE INDEX article_materials_fetched ON article_materials(fetched_at);
