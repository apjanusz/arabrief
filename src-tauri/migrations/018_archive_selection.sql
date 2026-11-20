CREATE TABLE news_search_ranges (
    region TEXT NOT NULL,
    topic TEXT NOT NULL,
    pending_days INTEGER NOT NULL DEFAULT 0 CHECK(pending_days IN (0,7,30,90)),
    searched_days INTEGER NOT NULL DEFAULT 0 CHECK(searched_days IN (0,7,30,90)),
    searched_at INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(region,topic)
);
CREATE TABLE news_read_failures (
    article_id INTEGER PRIMARY KEY REFERENCES news_articles(id),
    last_error TEXT NOT NULL,
    failed_at INTEGER NOT NULL
);
