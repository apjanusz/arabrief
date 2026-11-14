ALTER TABLE settings ADD COLUMN language TEXT NOT NULL DEFAULT 'pl';
CREATE TABLE settings_news_sources (
    url TEXT PRIMARY KEY NOT NULL,
    position INTEGER NOT NULL UNIQUE
);
