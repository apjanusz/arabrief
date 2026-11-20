ALTER TABLE settings ADD COLUMN news_region TEXT NOT NULL DEFAULT 'auto' CHECK(news_region IN ('auto','PL','US','GB'));
-- All broadcasts released before this migration were Polish, from the Polish edition.
ALTER TABLE content_broadcasts ADD COLUMN language TEXT NOT NULL DEFAULT 'pl';
ALTER TABLE content_broadcasts ADD COLUMN news_region TEXT NOT NULL DEFAULT 'PL';
CREATE TABLE news_region_articles (
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    region TEXT NOT NULL CHECK(region IN ('PL','US','GB')),
    PRIMARY KEY(article_id,region)
);
-- Legacy topic associations are intentionally not admitted into any new edition pool.
