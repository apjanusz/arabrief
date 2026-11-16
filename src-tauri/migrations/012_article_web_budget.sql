-- Reserve before sending: an uncertain HTTP outcome must not spend again.
CREATE TABLE article_web_attempts (
    article_id INTEGER PRIMARY KEY REFERENCES news_articles(id),
    attempted_at INTEGER NOT NULL
);
INSERT INTO article_web_attempts SELECT DISTINCT article_id,unixepoch() FROM article_chapters;
