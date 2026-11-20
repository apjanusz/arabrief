ALTER TABLE settings RENAME COLUMN news_region TO legacy_news_region;
ALTER TABLE settings ADD COLUMN news_region TEXT NOT NULL DEFAULT '';
UPDATE settings SET news_region=CASE WHEN legacy_news_region='auto' THEN '' ELSE legacy_news_region END;
ALTER TABLE settings DROP COLUMN legacy_news_region;
ALTER TABLE news_region_articles RENAME TO legacy_news_region_articles;
CREATE TABLE news_region_articles (
    article_id INTEGER NOT NULL REFERENCES news_articles(id),
    region TEXT NOT NULL,
    PRIMARY KEY(article_id,region)
);
INSERT INTO news_region_articles SELECT * FROM legacy_news_region_articles;
DROP TABLE legacy_news_region_articles;
