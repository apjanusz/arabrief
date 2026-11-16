use super::*;
use crate::news::{FetchError, NewsProvider, NewsService};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Pool {
    calls: AtomicUsize,
    since: Mutex<Vec<Option<i64>>>,
}
impl Pool {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            since: Mutex::new(vec![]),
        }
    }
}
fn article(index: usize) -> NewsItem {
    NewsItem {
        id: String::new(),
        provider: "fixture".into(),
        title: format!("Podobne wydarzenie naukowe {index}"),
        url: format!("https://example.org/article/{index}"),
        source: Some("Wydawca".into()),
        source_url: Some("https://example.org".into()),
        published_at: Some(
            chrono::DateTime::<Utc>::from_timestamp(
                Utc::now().timestamp() - 86400 + index as i64 * 60,
                0,
            )
            .unwrap()
            .to_rfc3339(),
        ),
        raw_publication_date: None,
        description_html: Some("Wyłącznie krótka zajawka".into()),
        guid: Some(format!("id-{index}")),
        feed_url: "https://example.org/rss".into(),
        fetched_at: Utc::now().to_rfc3339(),
        topics: vec!["Nauka".into()],
    }
}
impl NewsProvider for Pool {
    fn fetch_topic(&self, _: &str) -> Result<Vec<NewsItem>, FetchError> {
        let start = self.calls.fetch_add(1, Ordering::SeqCst) * 20;
        Ok((start..start + 20).rev().map(article).collect())
    }
    fn fetch_since(&self, topic: &str, since: Option<i64>) -> Result<Vec<NewsItem>, FetchError> {
        self.since.lock().unwrap().push(since);
        self.fetch_topic(topic)
    }
}
struct Model {
    calls: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    bad: Vec<String>,
}
impl Model {
    fn new(bad: Vec<String>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            bad,
        }
    }
}
impl ChapterGenerator for Model {
    fn chapter(&self, source: &NewsItem, _: &AppSettings) -> Result<(Chapter, Value), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(8));
        self.active.fetch_sub(1, Ordering::SeqCst);
        if self.bad.contains(&source.id) {
            return Err("Brak dostępu do artykułu lub nieudane AI".into());
        }
        Ok((
            Chapter {
                article_read: true,
                article_url: source.url.clone(),
                title: source.title.clone(),
                text: format!(
                    "Szczegółowy rozdział artykułu {}. Badacze przedstawili wyniki badań i wyjaśnili ograniczenia oraz plan następnych pomiarów.",
                    source.id
                ),
                quotes: vec!["Potwierdzony szczegół z artykułu".into()],
                limitations: vec![],
            },
            json!([{"action":"open_page","url":source.url,"status":"completed"}]),
        ))
    }
}
fn repository(path: &std::path::Path) -> Arc<SettingsRepository> {
    let repo = Arc::new(SettingsRepository::open(path).unwrap());
    let mut settings = repo.load().unwrap();
    settings.topics = vec!["Nauka".into()];
    repo.save(&settings).unwrap();
    repo
}
fn ids(result: crate::news::NewsResult) -> Vec<String> {
    result.items.into_iter().map(|item| item.id).collect()
}
fn unthrottle(repo: &SettingsRepository) {
    repo.with_database(|db| {
        db.execute("UPDATE news_fetch_state SET next_allowed_at=0", [])
            .unwrap();
        Ok(())
    })
    .unwrap();
}
#[test]
fn fifo_twenty_then_next_three_without_refetch_and_refill_preserves_last_two() {
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let content = ContentService::new(repo.clone());
    let provider = Pool::new();
    let model = Model::new(vec![]);
    let first = news.fetch_with(vec!["Nauka".into()], &provider).unwrap();
    assert_eq!(first.items.len(), 20);
    assert_eq!(first.items[0].guid.as_deref(), Some("id-0"));
    let a = content.generate_chapters(ids(first), &model).unwrap();
    assert_eq!(
        a.sources.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    let second = news.fetch_with(vec!["Nauka".into()], &provider).unwrap();
    let b = content.generate_chapters(ids(second), &model).unwrap();
    assert_eq!(
        b.sources.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["4", "5", "6"]
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(model.calls.load(Ordering::SeqCst), 6);
    assert!(model.peak.load(Ordering::SeqCst) > 1);
    assert!(model.peak.load(Ordering::SeqCst) <= 3);
    // Simulate completed playback of the first eighteen; preparation itself must not mark usage.
    repo.with_database(|db| {
        assert_eq!(
            db.query_row("SELECT count(*) FROM news_usage", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
        db.execute(
            "INSERT INTO news_usage SELECT id,'played',0 FROM news_articles WHERE id<=18",
            [],
        )
        .unwrap();
        Ok(())
    })
    .unwrap();
    unthrottle(&repo);
    let next = news.fetch_with(vec!["Nauka".into()], &provider).unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert_eq!(next.items.len(), 20);
    assert_eq!(next.items[0].id, "19");
    assert_eq!(next.items[1].id, "20");
    assert!(provider.since.lock().unwrap()[1].is_some());
    repo.with_database(|db| {
        db.execute(
            "INSERT OR IGNORE INTO news_usage SELECT id,'played',0 FROM news_articles",
            [],
        )
        .unwrap();
        Ok(())
    })
    .unwrap();
    unthrottle(&repo);
    let replenished = news.fetch_with(vec!["Nauka".into()], &provider).unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert_eq!(replenished.items.len(), 20);
    assert_eq!(replenished.items[0].guid.as_deref(), Some("id-40"));
}
#[test]
fn restart_keeps_queue_history_and_chapter_cache() {
    let path = std::env::temp_dir().join(format!(
        "arabrief-pool-{}-{}.sqlite",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let provider = Pool::new();
    {
        let repo = repository(&path);
        let news = NewsService::new(repo.clone());
        let items = news.fetch_with(vec!["Nauka".into()], &provider).unwrap();
        let content = ContentService::new(repo.clone());
        content
            .generate_chapters(ids(items), &Model::new(vec![]))
            .unwrap();
        repo.with_database(|db| {
            db.execute("INSERT INTO news_usage VALUES(1,'played',0)", [])
                .unwrap();
            Ok(())
        })
        .unwrap();
    }
    {
        let repo = Arc::new(SettingsRepository::open(&path).unwrap());
        let news = NewsService::new(repo.clone());
        let items = news.fetch_with(vec!["Nauka".into()], &provider).unwrap();
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(items.items[0].id, "4");
        assert_eq!(items.items.len(), 17);
        repo.with_database(|db| {
            assert_eq!(
                db.query_row("SELECT count(*) FROM article_chapters", [], |r| r
                    .get::<_, u32>(0))
                    .unwrap(),
                3
            );
            Ok(())
        })
        .unwrap();
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn unavailable_article_and_model_error_do_not_lose_siblings_or_order() {
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let sources = news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap();
    let service = ContentService::new(repo);
    let model = Model::new(vec!["1".into(), "3".into()]);
    let broadcast = service.generate_chapters(ids(sources), &model).unwrap();
    assert_eq!(
        broadcast
            .sources
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        ["2", "4", "5"]
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 5);
}
#[test]
fn successful_chapters_survive_later_sql_failure_and_are_not_generated_again() {
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let source_ids = ids(news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap());
    let service = ContentService::new(repo.clone());
    let model = Model::new(vec![]);
    repo.with_database(|db|{db.execute_batch("CREATE TRIGGER fail_broadcast BEFORE INSERT ON content_broadcasts BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();Ok(())}).unwrap();
    assert!(
        service
            .generate_chapters(source_ids.clone(), &model)
            .is_err()
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 3);
    repo.with_database(|db| {
        db.execute("DROP TRIGGER fail_broadcast", []).unwrap();
        Ok(())
    })
    .unwrap();
    let broadcast = service.generate_chapters(source_ids, &model).unwrap();
    assert_eq!(model.calls.load(Ordering::SeqCst), 3);
    assert_eq!(broadcast.sources.len(), 3);
    super::super::load_saved(&repo, broadcast.id.parse().unwrap(), &repo.load().unwrap()).unwrap();
}
#[test]
fn partial_chapters_and_poison_articles_cannot_block_queue() {
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let items = news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap();
    let service = ContentService::new(repo.clone());
    let broadcast = service
        .generate_chapters(
            items.items.iter().take(3).map(|s| s.id.clone()).collect(),
            &Model::new(vec!["1".into(), "2".into()]),
        )
        .unwrap();
    assert_eq!(broadcast.sources.len(), 1);
    failed(&repo, "1", "read").unwrap();
    failed(&repo, "1", "read").unwrap();
    repo.with_database(|db| {
        assert_eq!(
            db.query_row(
                "SELECT skipped FROM article_queue WHERE article_id=1",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
        Ok(())
    })
    .unwrap();
    let next = news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap();
    assert_eq!(next.items[0].id, "4");
}
fn response(source: &NewsItem, action: &str, article_read: bool) -> Value {
    json!({"status":"completed","output":[{"id":"call","type":"web_search_call","status":"completed","action":{"type":action,"url":source.url}}, {"type":"message","content":[{"type":"output_text","text":json!({"article_read":article_read,"article_url":source.url,"title":source.title,"text":"Artykuł zawiera konkretne wyniki pomiarów, opis metodologii oraz ograniczenia badania.","quotes":["Zmierzone wyniki potwierdzają hipotezę"],"limitations":[]}).to_string()}]}]})
}
#[test]
fn requires_real_page_open_and_read_not_search_snippets_or_a_claim_in_json() {
    let item = article(1);
    assert!(parse(&response(&item, "search", true), &item).is_err());
    assert!(parse(&response(&item, "open_page", false), &item).is_err());
    assert!(parse(&response(&item, "open_page", true), &item).is_ok());
    let mut wrong = response(&item, "open_page", true);
    wrong["output"][0]["action"]["url"] = json!("https://other.example/story");
    assert!(parse(&wrong, &item).is_err());
    let body = request(
        &item,
        &repository(std::path::Path::new(":memory:")).load().unwrap(),
    );
    assert_eq!(body["tool_choice"], "required");
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][0]["external_web_access"], true);
}

#[test]
fn global_api_error_does_not_poison_articles_and_stops_replacement_requests() {
    struct Unavailable(AtomicUsize);
    impl ChapterGenerator for Unavailable {
        fn chapter(&self, _: &NewsItem, _: &AppSettings) -> Result<(Chapter, Value), String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err("OpenAI: niedostępny model. HTTP 404".into())
        }
    }
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let sources = ids(news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap());
    let model = Unavailable(AtomicUsize::new(0));
    assert!(
        ContentService::new(repo.clone())
            .generate_chapters(sources, &model)
            .unwrap_err()
            .contains("HTTP 404")
    );
    assert_eq!(model.0.load(Ordering::SeqCst), 3);
    repo.with_database(|db| {
        assert_eq!(
            db.query_row(
                "SELECT SUM(failures)+SUM(skipped) FROM article_queue",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn changed_topics_and_already_reserved_articles_cannot_start_duplicate_preparation() {
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let source_ids = ids(news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap());
    let service = ContentService::new(repo.clone());
    let model = Model::new(vec![]);
    let first = service
        .generate_chapters(source_ids[..3].to_vec(), &model)
        .unwrap();
    assert_eq!(first.sources.len(), 3);
    assert!(
        service
            .generate_chapters(source_ids[..3].to_vec(), &model)
            .is_err()
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 3);
    let mut settings = repo.load().unwrap();
    settings.topics = vec!["Kultura".into()];
    repo.save(&settings).unwrap();
    assert!(service.generate_chapters(source_ids, &model).is_err());
    assert_eq!(model.calls.load(Ordering::SeqCst), 3);
}
