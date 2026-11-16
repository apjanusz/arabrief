-- Retry unused articles rejected by the former web-only reader once, using
-- native HTTP instead. Preserve receipts, usage and all spent search budgets.
UPDATE article_queue SET failures=0,retry_at=0,skipped=0
WHERE article_id IN (SELECT article_id FROM article_web_attempts)
AND NOT EXISTS (SELECT 1 FROM article_chapters c WHERE c.article_id=article_queue.article_id)
AND NOT EXISTS (SELECT 1 FROM news_usage u WHERE u.article_id=article_queue.article_id)
AND NOT EXISTS (SELECT 1 FROM content_sources c JOIN playback_receipts r ON r.broadcast_id=c.broadcast_id WHERE c.article_id=article_queue.article_id);
