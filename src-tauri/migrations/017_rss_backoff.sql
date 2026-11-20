UPDATE settings SET interval_ms=300000 WHERE interval_ms<300000;
CREATE TABLE news_retry_state (
    id INTEGER PRIMARY KEY CHECK(id=1),
    failures INTEGER NOT NULL DEFAULT 0
);
INSERT INTO news_retry_state(id) VALUES (1);
-- Preserve existing provider cooldowns and extend recently attempted topics.
UPDATE news_fetch_state SET next_allowed_at=MAX(next_allowed_at,unixepoch()+300) WHERE topic<>'';
UPDATE news_fetch_state SET next_allowed_at=MAX(next_allowed_at,unixepoch()+60) WHERE topic='';
