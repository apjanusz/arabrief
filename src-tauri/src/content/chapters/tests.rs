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
        &ArticleText {
            url: item.url.clone(),
            title: item.title.clone(),
            body: "Treść pobrana natywnie, nie opis RSS.".into(),
        },
    );
    assert_eq!(body["tool_choice"], "none");
    assert_eq!(body["tools"][0]["type"], "web_search");
    assert_eq!(body["tools"][0]["external_web_access"], true);
    assert_eq!(body["max_tool_calls"], 1);
    let input: Value = serde_json::from_str(body["input"].as_str().unwrap()).unwrap();
    assert_eq!(input["max_words"], 100);
    assert!(input.get("min_words").is_none());
    assert_eq!(input["article"]["content_origin"], "native_http_article");
    assert!(
        input["article"]["body_text"]
            .as_str()
            .unwrap()
            .contains("natywnie")
    );
    let mut duplicate_call = response(&item, "open_page", true);
    let call = duplicate_call["output"][0].clone();
    duplicate_call["output"].as_array_mut().unwrap().push(call);
    assert!(parse(&duplicate_call, &item).is_err());
}

#[test]
fn chapter_word_cap_keeps_full_sentences_and_never_retries_for_text_length() {
    let exact = format!("{}koniec.", "słowo ".repeat(99));
    assert_eq!(limit_text(&exact).unwrap().split_whitespace().count(), 100);
    let long = format!(
        "Potwierdzono ważne wyniki. {}urwany fragment",
        "Naukowcy opisali badanie. ".repeat(40)
    );
    let limited = limit_text(&long).unwrap();
    assert!(limited.split_whitespace().count() <= 100);
    assert!(limited.ends_with("badanie."));
    assert!(limit_text(&"słowo ".repeat(110)).is_err());
    assert_eq!(
        limit_text(&format!(
            "Gotowe zdanie. {}np. dalszy ciąg",
            "słowo ".repeat(97)
        ))
        .unwrap(),
        "Gotowe zdanie."
    );
    assert_eq!(
        limit_text("Wynik wynosi 3.14 procent. Dalszy fragment").unwrap(),
        "Wynik wynosi 3.14 procent."
    );
    let source = article(1);
    let mut value = response(&source, "open_page", true);
    let mut chapter: Value =
        serde_json::from_str(value["output"][1]["content"][0]["text"].as_str().unwrap()).unwrap();
    chapter["text"] = json!(long);
    value["output"][1]["content"][0]["text"] = json!(chapter.to_string());
    assert_eq!(parse(&value, &source).unwrap().0.text, limited);
}

#[test]
fn native_body_needs_no_web_call_and_quotes_must_exist_in_downloaded_text() {
    let source = article(1);
    let material = ArticleText {
        url: source.url.clone(),
        title: source.title.clone(),
        body: "Zmierzone wyniki potwierdzają hipotezę. Zespół opublikował szczegóły badania."
            .into(),
    };
    let mut value = response(&source, "open_page", true);
    value["output"].as_array_mut().unwrap().remove(0);
    let (chapter, access) = parse_native(&value, &source, &material).unwrap();
    assert!(chapter.text.split_whitespace().count() <= 100);
    assert_eq!(access[0]["action"], "native_http_article");
    let wrong = ArticleText {
        body: "Nie ma tutaj cytowanych wyników badania.".into(),
        ..material.clone()
    };
    assert!(parse_native(&value, &source, &wrong).is_err());
    assert!(parse_native(&response(&source, "search", true), &source, &material).is_err());
    let wrong_url = ArticleText {
        url: "https://example.org/different-story".into(),
        ..material
    };
    assert!(parse_native(&value, &source, &wrong_url).is_err());
}

#[test]
fn article_web_budget_survives_restart_and_is_independent_of_other_articles() {
    let path = std::env::temp_dir().join(format!(
        "arabrief-web-budget-{}-{}.sqlite",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let ids = {
        let repo = repository(&path);
        let news = NewsService::new(repo.clone());
        let sources = news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap();
        let ids = ids(sources);
        reserve_web_attempt(&repo, &ids[0]).unwrap();
        assert!(reserve_web_attempt(&repo, &ids[0]).is_err());
        ids
    };
    {
        let repo = repository(&path);
        assert!(reserve_web_attempt(&repo, &ids[0]).is_err());
        reserve_web_attempt(&repo, &ids[1]).unwrap();
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_reader_recovery_preserves_played_history_and_spent_search_budgets() {
    let repo = repository(std::path::Path::new(":memory:"));
    let news = NewsService::new(repo.clone());
    let sources = ids(news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap());
    reserve_web_attempt(&repo, &sources[0]).unwrap();
    reserve_web_attempt(&repo, &sources[1]).unwrap();
    repo.with_database(|db| {
        db.execute("INSERT INTO news_usage VALUES (?1,'played',unixepoch())",[&sources[0]]).unwrap();
        db.execute("UPDATE article_queue SET failures=3,skipped=1,retry_at=unixepoch()+3600 WHERE article_id IN (?1,?2)",params![sources[0],sources[1]]).unwrap();
        db.execute_batch(include_str!("../../../migrations/013_native_article_recovery.sql")).unwrap();
        let skipped = |id:&str| db.query_row("SELECT skipped FROM article_queue WHERE article_id=?1",[id],|row|row.get::<_,i64>(0)).unwrap();
        assert_eq!(skipped(&sources[0]),1);
        assert_eq!(skipped(&sources[1]),0);
        assert_eq!(db.query_row("SELECT COUNT(*) FROM article_web_attempts",[],|r|r.get::<_,i64>(0)).unwrap(),2);
        Ok(())
    }).unwrap();
}

#[test]
fn global_api_error_does_not_poison_articles_and_stops_replacement_requests() {
    struct Unavailable(AtomicUsize, &'static str);
    impl ChapterGenerator for Unavailable {
        fn chapter(&self, _: &NewsItem, _: &AppSettings) -> Result<(Chapter, Value), String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(self.1.into())
        }
    }
    for error in [
        "OpenAI: niedostępny model. HTTP 404",
        "Błąd połączenia rozdziału z OpenAI",
        "Timeout odczytu artykułu i generowania rozdziału (90 s)",
    ] {
        let repo = repository(std::path::Path::new(":memory:"));
        let news = NewsService::new(repo.clone());
        let sources = ids(news.fetch_with(vec!["Nauka".into()], &Pool::new()).unwrap());
        let model = Unavailable(AtomicUsize::new(0), error);
        assert_eq!(
            ContentService::new(repo.clone())
                .generate_chapters(sources, &model)
                .unwrap_err(),
            error
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
