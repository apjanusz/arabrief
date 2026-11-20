//! Readability-first selection. RSS limits remain owned by NewsRepository.
use super::*;

pub(super) fn age_tier(item: &NewsItem, timestamp: i64) -> u8 {
    if within_days(item, timestamp, 7) {
        0
    } else if within_days(item, timestamp, 30) {
        1
    } else {
        2
    }
}

pub(crate) fn chronological(items: &mut [NewsItem], timestamp: i64) {
    items.sort_by_key(|item| {
        let tier = age_tier(item, timestamp);
        (
            tier,
            item.published_at
                .as_ref()
                .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                .map_or(i64::MAX, |date| date.timestamp()),
            item.id.parse::<i64>().unwrap_or(i64::MAX),
        )
    });
}

pub(crate) fn order_candidates(items: &mut Vec<NewsItem>, timestamp: i64) {
    chronological(items, timestamp);
    // Oldest first per topic, rotating topics within each age tier.
    let mut remaining = std::mem::take(items);
    for days in [7, 30, 90] {
        let mut covered = HashSet::new();
        while let Some(first) = remaining.iter().position(|item| {
            within_days(item, timestamp, days) || (days == 90 && item.provider != "google_news")
        }) {
            let next = remaining.iter().position(|item| {
                (within_days(item, timestamp, days)
                    || (days == 90 && item.provider != "google_news"))
                    && item.topics.iter().any(|topic| !covered.contains(topic))
            });
            let index = next.unwrap_or_else(|| {
                covered.clear();
                first
            });
            let item = remaining.remove(index);
            covered.extend(item.topics.iter().cloned());
            items.push(item);
        }
    }
}

impl NewsRepository {
    fn plan_range(&self, topic: &str, region: &str, days: u32) -> Result<Option<u32>, String> {
        self.0.with_database(|db| {
            let tx=db.transaction().map_err(|e|e.to_string())?;
            tx.execute("INSERT OR IGNORE INTO news_search_ranges(region,topic) VALUES (?1,?2)",params![region,normalized(topic)]).map_err(|e|e.to_string())?;
            let (pending,searched,at):(u32,u32,i64)=tx.query_row("SELECT pending_days,searched_days,searched_at FROM news_search_ranges WHERE region=?1 AND topic=?2",params![region,normalized(topic)],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
            let result=if searched>=days && at>now()-300 {None} else {Some(if pending==0 {days} else {pending})};
            if let Some(pending)=result {tx.execute("UPDATE news_search_ranges SET pending_days=?3 WHERE region=?1 AND topic=?2",params![region,normalized(topic),pending]).map_err(|e|e.to_string())?;}
            tx.commit().map_err(|e|e.to_string())?;Ok(result)
        })
    }
    pub(super) fn range_fetched(&self, topic: &str, region: &str, days: u32) -> Result<(), String> {
        self.0.with_database(|db|db.execute("INSERT INTO news_search_ranges VALUES (?1,?2,0,?3,?4) ON CONFLICT(region,topic) DO UPDATE SET pending_days=CASE WHEN pending_days>excluded.searched_days THEN pending_days ELSE 0 END,searched_days=excluded.searched_days,searched_at=excluded.searched_at",params![region,normalized(topic),days,now()]).map(|_|()).map_err(|e|e.to_string()))
    }
    fn read_failed(&self, item: &NewsItem, error: &str) -> Result<(), String> {
        let transient = error.to_lowercase().contains("timeout")
            || error.to_lowercase().contains("połączen")
            || error.contains("HTTP 5")
            || error.contains("HTTP 429");
        self.0.with_database(|db| {
            let tx=db.transaction().map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO news_read_failures VALUES (?1,?2,unixepoch()) ON CONFLICT(article_id) DO UPDATE SET last_error=excluded.last_error,failed_at=excluded.failed_at",params![item.id,error]).map_err(|e|e.to_string())?;
            tx.execute("UPDATE article_queue SET failures=failures+1,retry_at=unixepoch()+?2,skipped=(failures+1>=3) WHERE article_id=?1",params![item.id,if transient {1800} else {86400}]).map_err(|e|e.to_string())?;
            tx.commit().map_err(|e|e.to_string())
        })
    }
}

#[derive(Default)]
struct Progress {
    selected: Vec<NewsItem>,
    attempted: HashSet<String>,
    publishers: HashSet<String>,
}
impl Progress {
    fn done(&self) -> bool {
        self.selected.len() >= 3 || self.attempted.len() >= 9
    }
    fn inspect(
        &mut self,
        repo: &NewsRepository,
        topics: &[String],
        days: u32,
        read: &(impl Fn(&NewsItem) -> Result<String, String> + Sync),
    ) -> Result<(), String> {
        let candidates = repo.candidates_days(topics, days)?;
        while !self.done() {
            let batch: Vec<_> = candidates
                .iter()
                .filter(|item| !self.attempted.contains(&item.id))
                .take((3 - self.selected.len()).min(9 - self.attempted.len()))
                .cloned()
                .collect();
            if batch.is_empty() {
                break;
            }
            for item in &batch {
                self.attempted.insert(item.id.clone());
            }
            for (index, result) in crate::work_queue::run(&batch, 3, read, |_| false)? {
                let item = &batch[index];
                match result {
                    Ok(publisher) if self.publishers.insert(canonical(&publisher)) => {
                        self.selected.push(item.clone())
                    }
                    Ok(_) => {}
                    Err(error) => repo.read_failed(item, &error)?,
                }
            }
        }
        Ok(())
    }
}

impl NewsService {
    pub(super) fn prepare_sources(&self, topics: Vec<String>) -> Result<NewsResult, String> {
        let settings = self.repository.0.load()?;
        let mut provider = GoogleNews::new()?;
        provider.region = settings.effective_region().into();
        let read_settings = settings.clone();
        self.prepare_with(
            topics,
            &provider,
            &|item| {
                if let Some(url) = crate::content::chapters::cached_readable(
                    &self.repository.0,
                    item,
                    &read_settings,
                )? {
                    return Ok(url);
                }
                crate::content::article::load(&self.repository.0, item).map(|material| material.url)
            },
            settings,
        )
    }
    fn prepare_with(
        &self,
        topics: Vec<String>,
        provider: &dyn NewsProvider,
        read: &(impl Fn(&NewsItem) -> Result<String, String> + Sync),
        settings: crate::settings::AppSettings,
    ) -> Result<NewsResult, String> {
        let _gate = self
            .selection_gate
            .try_lock()
            .map_err(|_| "Pobieranie wiadomości już trwa".to_owned())?;
        let topics = validate_topics(topics)?;
        self.repository.prune_irrelevant(&topics)?;
        let mut progress = Progress::default();
        let mut fetched = Vec::new();
        for days in [7, 30, 90] {
            if crate::recovery::settings_key(&settings)
                != crate::recovery::settings_key(&self.repository.0.load()?)
            {
                return Err(
                    "Broadcast language or news region changed. Prepare a new broadcast.".into(),
                );
            }
            progress.inspect(&self.repository, &topics, days, read)?;
            if progress.done() {
                break;
            }
            let plans: Vec<_> = topics
                .iter()
                .map(|topic| {
                    self.repository
                        .plan_range(topic, settings.effective_region(), days)
                        .map(|window| (topic.clone(), window))
                })
                .collect::<Result<_, _>>()?;
            for window in [7, 30, 90] {
                let requested: Vec<_> = plans
                    .iter()
                    .filter(|(_, planned)| *planned == Some(window))
                    .map(|(topic, _)| topic.clone())
                    .collect();
                if requested.is_empty() {
                    continue;
                }
                let result =
                    self.fetch_with_settings(requested, provider, settings.clone(), window, true)?;
                fetched.extend(result.fetched_topics);
            }
            progress.inspect(&self.repository, &topics, days, read)?;
            if progress.done() {
                break;
            }
        }
        if crate::recovery::settings_key(&settings)
            != crate::recovery::settings_key(&self.repository.0.load()?)
        {
            return Err(
                "Broadcast language or news region changed. Prepare a new broadcast.".into(),
            );
        }
        fetched.sort();
        fetched.dedup();
        chronological(&mut progress.selected, now());
        Ok(NewsResult {
            items: progress.selected,
            topics: self.repository.statuses(&topics)?,
            fetched_topics: fetched,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn item(index: usize, age: i64, topic: &str) -> NewsItem {
        NewsItem {
            id: String::new(),
            provider: "google_news".into(),
            title: format!("{topic} article {index}"),
            url: format!("https://example.org/{index}"),
            source: None,
            source_url: None,
            published_at: Some(
                DateTime::<Utc>::from_timestamp(now() - age * 86400, 0)
                    .unwrap()
                    .to_rfc3339(),
            ),
            raw_publication_date: None,
            description_html: None,
            guid: Some(index.to_string()),
            feed_url: "https://example.org/rss".into(),
            fetched_at: Utc::now().to_rfc3339(),
            topics: vec![topic.into()],
        }
    }
    fn service() -> NewsService {
        let repo = Arc::new(SettingsRepository::open(std::path::Path::new(":memory:")).unwrap());
        let mut settings = repo.load().unwrap();
        settings.ui_language = "pl".into();
        settings.news_region = "PL".into();
        settings.topics = vec!["science".into(), "climbing".into()];
        repo.save(&settings).unwrap();
        NewsService::new(repo)
    }
    fn store(service: &NewsService, items: &[NewsItem]) {
        for item in items {
            service
                .repository
                .save_for_region(&item.topics[0], std::slice::from_ref(item), "PL")
                .unwrap();
        }
    }
    fn prepare(
        service: &NewsService,
        provider: &dyn NewsProvider,
        read: &(impl Fn(&NewsItem) -> Result<String, String> + Sync),
    ) -> NewsResult {
        let settings = service.repository.0.load().unwrap();
        service
            .prepare_with(settings.topics.clone(), provider, read, settings)
            .unwrap()
    }
    struct Offline(AtomicUsize);
    impl NewsProvider for Offline {
        fn fetch_topic(&self, _: &str) -> Result<Vec<NewsItem>, FetchError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(FetchError {
                message: "HTTP 429".into(),
                retry_after: Some(600),
            })
        }
    }
    #[test]
    fn fresh_local_readable_items_prevent_archive_and_network_work() {
        let service = service();
        store(
            &service,
            &[
                item(1, 1, "science"),
                item(2, 6, "science"),
                item(3, 2, "climbing"),
                item(4, 20, "science"),
            ],
        );
        let provider = Offline(AtomicUsize::new(0));
        let result = prepare(&service, &provider, &|source| Ok(source.url.clone()));
        assert_eq!(result.items.len(), 3);
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);
        assert!(
            result
                .items
                .iter()
                .all(|source| within_days(source, now(), 7))
        );
        assert_eq!(result.items[0].guid.as_deref(), Some("2"));
        assert_eq!(result.items[1].topics, vec!["climbing"]);
    }
    #[test]
    fn local_archives_fill_shortages_even_during_provider_cooldown() {
        let service = service();
        store(
            &service,
            &[
                item(1, 6, "science"),
                item(2, 29, "science"),
                item(3, 50, "climbing"),
                item(4, 91, "science"),
            ],
        );
        let provider = Offline(AtomicUsize::new(0));
        let result = prepare(&service, &provider, &|source| Ok(source.url.clone()));
        assert_eq!(
            result
                .items
                .iter()
                .map(|source| source.guid.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
        assert_eq!(provider.0.load(Ordering::SeqCst), 1);
        assert!(
            result
                .items
                .iter()
                .all(|source| within_days(source, now(), 90))
        );
    }
    #[test]
    fn failed_reads_are_persisted_and_replacements_are_selected() {
        let service = service();
        store(
            &service,
            &[
                item(1, 6, "science"),
                item(2, 5, "science"),
                item(3, 20, "science"),
                item(4, 60, "climbing"),
            ],
        );
        let provider = Offline(AtomicUsize::new(0));
        let bad = AtomicUsize::new(0);
        let read = |source: &NewsItem| {
            if source.guid.as_deref() == Some("1") {
                bad.fetch_add(1, Ordering::SeqCst);
                Err("Title mismatch".into())
            } else {
                Ok(source.url.clone())
            }
        };
        assert_eq!(prepare(&service, &provider, &read).items.len(), 3);
        assert_eq!(prepare(&service, &provider, &read).items.len(), 3);
        assert_eq!(bad.load(Ordering::SeqCst), 1);
        service
            .repository
            .0
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM news_read_failures", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    1
                );
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn candidate_budget_and_used_articles_are_respected() {
        let service = service();
        let items: Vec<_> = (0..15).map(|i| item(i, 1, "science")).collect();
        store(&service, &items);
        let source = service
            .repository
            .candidates_days(&["science".into()], 7)
            .unwrap()
            .remove(0);
        service
            .repository
            .mark_used(std::slice::from_ref(&source.id), "used")
            .unwrap();
        let calls = AtomicUsize::new(0);
        let provider = Offline(AtomicUsize::new(0));
        let result = prepare(&service, &provider, &|candidate| {
            assert_ne!(candidate.id, source.id);
            calls.fetch_add(1, Ordering::SeqCst);
            Err("Unavailable".into())
        });
        assert!(result.items.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 9);
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn frontier_waits_for_limits_and_does_not_skip_the_thirty_day_step() {
        let service = service();
        let repo = &service.repository;
        repo.range_fetched("science", "PL", 7).unwrap();
        assert_eq!(repo.plan_range("science", "PL", 7).unwrap(), None);
        assert_eq!(repo.plan_range("science", "PL", 30).unwrap(), Some(30));
        assert_eq!(repo.plan_range("science", "PL", 90).unwrap(), Some(30));
        repo.range_fetched("science", "PL", 30).unwrap();
        assert_eq!(repo.plan_range("science", "PL", 7).unwrap(), None);
        assert_eq!(repo.plan_range("science", "PL", 90).unwrap(), Some(90));
        assert_eq!(repo.plan_range("science", "US", 7).unwrap(), Some(7));
    }
    #[test]
    fn rss_expands_only_after_readable_shortages_and_allowed_fetches() {
        struct Ranges(Mutex<Vec<u32>>);
        impl NewsProvider for Ranges {
            fn fetch_topic(&self, _: &str) -> Result<Vec<NewsItem>, FetchError> {
                panic!("range-aware request required")
            }
            fn fetch_range(
                &self,
                _: &str,
                _: Option<i64>,
                days: u32,
            ) -> Result<Vec<NewsItem>, FetchError> {
                self.0.lock().unwrap().push(days);
                Ok(vec![item(
                    days as usize,
                    match days {
                        7 => 1,
                        30 => 20,
                        _ => 60,
                    },
                    "science",
                )])
            }
        }
        let service = service();
        let mut settings = service.repository.0.load().unwrap();
        settings.topics = vec!["science".into()];
        service.repository.0.save(&settings).unwrap();
        let provider = Ranges(Mutex::new(vec![]));
        let read = |source: &NewsItem| Ok(source.url.clone());
        assert_eq!(prepare(&service, &provider, &read).items.len(), 1);
        assert_eq!(*provider.0.lock().unwrap(), vec![7]);
        prepare(&service, &provider, &read);
        assert_eq!(*provider.0.lock().unwrap(), vec![7]);
        service
            .repository
            .0
            .with_database(|db| {
                db.execute("UPDATE news_fetch_state SET next_allowed_at=0", [])
                    .unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(prepare(&service, &provider, &read).items.len(), 2);
        assert_eq!(*provider.0.lock().unwrap(), vec![7, 30]);
        service
            .repository
            .0
            .with_database(|db| {
                db.execute("UPDATE news_fetch_state SET next_allowed_at=0", [])
                    .unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(prepare(&service, &provider, &read).items.len(), 3);
        assert_eq!(*provider.0.lock().unwrap(), vec![7, 30, 90]);
        prepare(&service, &provider, &read);
        assert_eq!(*provider.0.lock().unwrap(), vec![7, 30, 90]);
    }
    #[test]
    fn dates_are_bounded_and_archive_search_ignores_the_recent_watermark() {
        let timestamp = now();
        let mut source = item(1, 90, "science");
        assert!(within_days(&source, timestamp, 90));
        source.published_at = Some(
            DateTime::<Utc>::from_timestamp(timestamp - 90 * 86400 - 1, 0)
                .unwrap()
                .to_rfc3339(),
        );
        assert!(!within_days(&source, timestamp, 90));
        source.published_at = None;
        assert!(!within_days(&source, timestamp, 90));
        source.published_at = Some(
            DateTime::<Utc>::from_timestamp(timestamp + 60, 0)
                .unwrap()
                .to_rfc3339(),
        );
        assert!(!within_days(&source, timestamp, 90));
        for days in [7, 30, 90] {
            let url = GoogleNews::regional_url_days(
                "science",
                Some(timestamp - 86400),
                timestamp,
                "US",
                days,
            );
            let query = url
                .query_pairs()
                .find(|(name, _)| name == "q")
                .unwrap()
                .1
                .into_owned();
            let date = DateTime::<Utc>::from_timestamp(
                timestamp
                    - if days == 7 {
                        2 * 86400
                    } else {
                        i64::from(days) * 86400
                    },
                0,
            )
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
            assert!(query.contains(&format!("after:{date}")));
        }
    }
    #[test]
    fn sql_limit_does_not_hide_fresh_articles_behind_a_large_archive() {
        let service = service();
        let mut items: Vec<_> = (0..25).map(|i| item(i, 60, "science")).collect();
        items.extend([
            item(30, 1, "science"),
            item(31, 2, "science"),
            item(32, 3, "science"),
        ]);
        store(&service, &items);
        let sources = service
            .repository
            .candidates_days(&["science".into()], 90)
            .unwrap();
        assert_eq!(sources.len(), 20);
        assert!(
            sources[..3]
                .iter()
                .all(|source| within_days(source, now(), 7))
        );
    }
}
