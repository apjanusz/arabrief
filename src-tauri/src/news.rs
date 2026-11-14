use crate::settings::SettingsRepository;
use chrono::{DateTime, Utc};
use reqwest::{Url, blocking::Client};
use rusqlite::{OptionalExtension, params};
use std::{
    collections::HashSet,
    io::Read,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: String,
    pub provider: String,
    pub title: String,
    pub url: String,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub published_at: Option<String>,
    pub raw_publication_date: Option<String>,
    pub description_html: Option<String>,
    pub guid: Option<String>,
    pub feed_url: String,
    pub fetched_at: String,
    pub topics: Vec<String>,
}
#[derive(Debug)]
pub struct FetchError {
    pub message: String,
    pub retry_after: Option<i64>,
}
/// Provider-neutral port. Implementations return only supplied source metadata.
pub trait NewsProvider: Send + Sync {
    fn fetch_topic(&self, topic: &str) -> Result<Vec<NewsItem>, FetchError>;
}
pub struct GoogleNews {
    client: Client,
}
impl GoogleNews {
    fn new() -> Result<Self, String> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(12))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("AraBrief/0.1 RSS reader")
                .build()
                .map_err(|e| e.to_string())?,
        })
    }
    fn url(topic: &str) -> Url {
        let mut url = Url::parse("https://news.google.com/rss/search").expect("constant URL");
        url.query_pairs_mut()
            .append_pair("q", topic)
            .append_pair("hl", "pl")
            .append_pair("gl", "PL")
            .append_pair("ceid", "PL:pl");
        url
    }
}
impl NewsProvider for GoogleNews {
    fn fetch_topic(&self, topic: &str) -> Result<Vec<NewsItem>, FetchError> {
        self.fetch_url(topic, Self::url(topic))
    }
}
impl GoogleNews {
    fn fetch_url(&self, topic: &str, url: Url) -> Result<Vec<NewsItem>, FetchError> {
        let failure = |message: String| FetchError {
            message,
            retry_after: None,
        };
        let response = self.client.get(url.clone()).send().map_err(|e| {
            failure(if e.is_timeout() {
                "Timeout RSS (12 s)".into()
            } else {
                "Błąd połączenia z Google News RSS".into()
            })
        })?;
        if !response.status().is_success() {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| {
                    v.parse::<i64>().ok().or_else(|| {
                        DateTime::parse_from_rfc2822(v)
                            .ok()
                            .map(|t| t.timestamp() - Utc::now().timestamp())
                    })
                })
                .map(|s| s.clamp(60, 86400));
            return Err(FetchError {
                message: format!("Google News RSS: HTTP {}", response.status().as_u16()),
                retry_after: if response.status().as_u16() == 429
                    || response.status().as_u16() == 503
                {
                    Some(retry_after.unwrap_or(300))
                } else {
                    None
                },
            });
        }
        let mut bytes = Vec::new();
        response
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failure("Błąd odczytu RSS".into()))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(failure("RSS przekracza limit 2 MiB".into()));
        }
        parse(&bytes, url.as_str(), topic).map_err(failure)
    }
}
fn http_url(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| raw.to_owned())
}
fn parse(bytes: &[u8], feed_url: &str, topic: &str) -> Result<Vec<NewsItem>, String> {
    let channel =
        rss::Channel::read_from(bytes).map_err(|_| "Nieprawidłowy dokument RSS".to_string())?;
    let mut items = Vec::new();
    for entry in channel.items().iter().take(100) {
        let Some(title) = entry
            .title()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 8192)
        else {
            continue;
        };
        let Some(url) = entry.link().and_then(http_url) else {
            continue;
        };
        let published_at = entry
            .pub_date()
            .and_then(|date| DateTime::parse_from_rfc2822(date).ok())
            .map(|t| t.with_timezone(&Utc).to_rfc3339());
        items.push(NewsItem {
            id: String::new(),
            provider: "google_news".into(),
            title: title.into(),
            url,
            source: entry
                .source()
                .and_then(|s| s.title())
                .map(str::to_owned)
                .filter(|s| !s.is_empty()),
            source_url: entry.source().and_then(|s| http_url(s.url())),
            published_at,
            raw_publication_date: entry.pub_date().map(str::to_owned),
            description_html: entry.description().map(str::to_owned),
            guid: entry
                .guid()
                .map(|g| g.value().to_owned())
                .filter(|s| !s.is_empty()),
            feed_url: feed_url.into(),
            fetched_at: Utc::now().to_rfc3339(),
            topics: vec![topic.into()],
        });
    }
    Ok(items)
}
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn canonical(raw: &str) -> String {
    let mut url = Url::parse(raw).expect("validated URL");
    let google_news = url.host_str() == Some("news.google.com");
    url.set_fragment(None);
    let pairs: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| {
            !(key.starts_with("utm_")
                || ["gclid", "fbclid"].contains(&key.as_ref())
                || google_news && key == "oc")
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.set_query(None);
    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs);
    }
    url.into()
}
fn fingerprint(item: &NewsItem) -> Option<String> {
    // Conservative fallback: same title, publisher and UTC day. Never semantic/AI deduplication.
    Some(
        serde_json::to_string(&(
            normalized(&item.title),
            normalized(item.source.as_ref()?),
            item.published_at.as_ref()?.get(..10)?,
        ))
        .expect("string tuple"),
    )
}
fn now() -> i64 {
    Utc::now().timestamp()
}
pub struct NewsRepository(Arc<SettingsRepository>);
impl NewsRepository {
    fn save(&self, topic: &str, items: &[NewsItem]) -> Result<(), String> {
        let topic = normalized(topic);
        self.0.with_database(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            for item in items {
                let url = canonical(&item.url); let fingerprint = fingerprint(item);
                let json = serde_json::to_string(item).map_err(|e| e.to_string())?;
                let existing: Option<i64> = tx.query_row("SELECT article_id FROM news_aliases WHERE provider=?3 AND ((kind='url' AND value=?1) OR (kind='guid' AND value=?4)) UNION SELECT id FROM news_articles WHERE canonical_url=?1 OR fingerprint=?2 OR (provider=?3 AND guid=?4) ORDER BY 1 LIMIT 1", params![url,fingerprint,item.provider,item.guid], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
                let id = if let Some(id) = existing { id } else {
                    tx.execute("INSERT INTO news_articles(provider,guid,canonical_url,fingerprint,metadata_json,first_seen_at) VALUES (?1,?2,?3,?4,?5,?6)", params![item.provider,item.guid,url,fingerprint,json,now()]).map_err(|e| e.to_string())?;
                    tx.last_insert_rowid()
                };
                for (kind,value) in [("url",Some(&url)),("guid",item.guid.as_ref())] {
                    if let Some(value) = value { tx.execute("INSERT INTO news_aliases VALUES (?1,?2,?3,?4,?5) ON CONFLICT(provider,kind,value) DO UPDATE SET metadata_json=excluded.metadata_json",params![item.provider,kind,value,id,json]).map_err(|e| e.to_string())?; }
                }
                tx.execute("INSERT INTO news_topic_articles VALUES (?1,?2,?3) ON CONFLICT(topic,article_id) DO UPDATE SET last_seen_at=excluded.last_seen_at", params![topic,id,now()]).map_err(|e| e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())
        })
    }
    fn candidates(&self, topics: &[String]) -> Result<Vec<NewsItem>, String> {
        self.0.with_database(|db| {
            let mut items: Vec<NewsItem> = Vec::new();
            for topic in topics {
                let mut statement = db.prepare("SELECT a.id,a.metadata_json FROM news_articles a JOIN news_topic_articles t ON t.article_id=a.id WHERE t.topic=?1 AND NOT EXISTS(SELECT 1 FROM news_usage u WHERE u.article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c WHERE c.article_id=a.id) ORDER BY t.last_seen_at DESC LIMIT 100").map_err(|e| e.to_string())?;
                let rows = statement.query_map([normalized(topic)], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(|e| e.to_string())?;
                for row in rows {
                    let (id,json) = row.map_err(|e| e.to_string())?;
                    let id = id.to_string();
                    if let Some(existing) = items.iter_mut().find(|i| i.id == id) { existing.topics.push(topic.clone()); continue; }
                    let mut item: NewsItem = serde_json::from_str(&json).map_err(|e| e.to_string())?;
                    item.id = id; item.topics = vec![topic.clone()]; items.push(item);
                }
            }
            items.sort_by(|a,b| b.published_at.cmp(&a.published_at).then(a.id.cmp(&b.id)));
            Ok(items)
        })
    }
    fn claim(&self, topics: &[String], timestamp: i64) -> Result<Vec<String>, String> {
        self.0.with_database(|db| {
            let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
            let next = |topic: &str| -> Result<i64,String> { tx.query_row("SELECT next_allowed_at FROM news_fetch_state WHERE provider='google_news' AND topic=?1",[topic],|r| r.get(0)).optional().map(|v| v.unwrap_or(0)).map_err(|e| e.to_string()) };
            if next("")? > timestamp { return Ok(vec![]); }
            let mut due = Vec::new();
            for topic in topics { if next(&normalized(topic))? <= timestamp { due.push(topic.clone()); } }
            if !due.is_empty() {
                tx.execute("INSERT INTO news_fetch_state VALUES ('google_news','',?1,NULL) ON CONFLICT(provider,topic) DO UPDATE SET next_allowed_at=excluded.next_allowed_at,last_error=NULL",[timestamp+60]).map_err(|e| e.to_string())?;
                for topic in &due { tx.execute("INSERT INTO news_fetch_state VALUES ('google_news',?1,?2,NULL) ON CONFLICT(provider,topic) DO UPDATE SET next_allowed_at=excluded.next_allowed_at,last_error=NULL",params![normalized(topic),timestamp+300]).map_err(|e| e.to_string())?; }
            }
            tx.commit().map_err(|e| e.to_string())?; Ok(due)
        })
    }
    fn error(&self, topic: &str, error: &FetchError) -> Result<(), String> {
        self.0.with_database(|db| {
            db.execute("UPDATE news_fetch_state SET last_error=?1 WHERE provider='google_news' AND topic=?2",params![error.message,normalized(topic)]).map_err(|e| e.to_string())?;
            if let Some(delay) = error.retry_after { db.execute("UPDATE news_fetch_state SET next_allowed_at=MAX(next_allowed_at,?1),last_error=?2 WHERE provider='google_news' AND topic=''",params![now()+delay,error.message]).map_err(|e| e.to_string())?; }
            Ok(())
        })
    }
    fn statuses(&self, topics: &[String]) -> Result<Vec<TopicStatus>, String> {
        self.0.with_database(|db| {
            let (global,global_error) = db.query_row("SELECT next_allowed_at,last_error FROM news_fetch_state WHERE provider='google_news' AND topic=''",[],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?))).optional().map_err(|e| e.to_string())?.unwrap_or((0,None));
            topics.iter().map(|topic| {
                let (next,error) = db.query_row("SELECT next_allowed_at,last_error FROM news_fetch_state WHERE provider='google_news' AND topic=?1",[normalized(topic)],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?))).optional().map_err(|e| e.to_string())?.unwrap_or((0,None));
                Ok(TopicStatus { topic: topic.clone(), next_fetch_at: next.max(global), error: error.or_else(|| if global > now() { global_error.clone() } else { None }) })
            }).collect()
        })
    }
    pub fn mark_used(&self, ids: &[String], broadcast_id: &str) -> Result<(), String> {
        if ids.is_empty()
            || ids.len() > 100
            || broadcast_id.trim().is_empty()
            || broadcast_id.len() > 200
        {
            return Err("Nieprawidłowa historia audycji".into());
        }
        self.0.with_database(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            for id in ids {
                let id: i64 = id
                    .parse()
                    .map_err(|_| "Nieprawidłowy identyfikator artykułu")?;
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM news_articles WHERE id=?1)",
                        [id],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if !exists {
                    return Err("Nieznany artykuł".into());
                }
                tx.execute(
                    "INSERT OR IGNORE INTO news_usage VALUES (?1,?2,?3)",
                    params![id, broadcast_id, now()],
                )
                .map_err(|e| e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())
        })
    }
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicStatus {
    topic: String,
    next_fetch_at: i64,
    error: Option<String>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsResult {
    items: Vec<NewsItem>,
    topics: Vec<TopicStatus>,
    fetched_topics: Vec<String>,
}
pub struct NewsService {
    repository: NewsRepository,
    gate: Mutex<()>,
}
impl NewsService {
    pub fn new(repository: Arc<SettingsRepository>) -> Self {
        Self {
            repository: NewsRepository(repository),
            gate: Mutex::new(()),
        }
    }
    fn fetch(&self, topics: Vec<String>) -> Result<NewsResult, String> {
        let _gate = self
            .gate
            .try_lock()
            .map_err(|_| "Pobieranie wiadomości już trwa")?;
        let topics = validate_topics(topics)?;
        let provider = GoogleNews::new()?;
        let due = self.repository.claim(&topics, now())?;
        let mut fetched_topics = Vec::new();
        for batch in due.chunks(4) {
            let throttled = std::thread::scope(|scope| -> Result<bool, String> {
                let mut throttled = false;
                let handles: Vec<_> = batch
                    .iter()
                    .map(|topic| {
                        let provider: &dyn NewsProvider = &provider;
                        (topic, scope.spawn(move || provider.fetch_topic(topic)))
                    })
                    .collect();
                for (topic, handle) in handles {
                    fetched_topics.push(topic.clone());
                    match handle.join().map_err(|_| "Błąd wątku RSS")? {
                        Ok(items) => self.repository.save(topic, &items)?,
                        Err(error) => {
                            throttled |= error.retry_after.is_some();
                            self.repository.error(topic, &error)?;
                        }
                    }
                }
                Ok(throttled)
            })?;
            // A provider-wide 429/503 prevents starting further chunks.
            if throttled {
                break;
            }
        }
        Ok(NewsResult {
            items: self.repository.candidates(&topics)?,
            topics: self.repository.statuses(&topics)?,
            fetched_topics,
        })
    }
}
fn validate_topics(topics: Vec<String>) -> Result<Vec<String>, String> {
    if topics.is_empty() || topics.len() > 30 {
        return Err("Podaj od 1 do 30 tematów".into());
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for topic in topics {
        let topic = topic.trim();
        if topic.is_empty() || topic.len() > 160 || topic.chars().any(char::is_control) {
            return Err("Nieprawidłowy temat".into());
        }
        if seen.insert(normalized(topic)) {
            result.push(topic.to_owned());
        }
    }
    Ok(result)
}
#[tauri::command]
pub async fn news_fetch(
    topics: Vec<String>,
    service: tauri::State<'_, Arc<NewsService>>,
) -> Result<NewsResult, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.fetch(topics))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn news_mark_used(
    article_ids: Vec<String>,
    broadcast_id: String,
    service: tauri::State<'_, Arc<NewsService>>,
) -> Result<(), String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.repository.mark_used(&article_ids, &broadcast_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<NewsItem> {
        parse(
            include_bytes!("fixtures/news.xml"),
            "https://news.google.com/rss/search?q=test",
            "nauka",
        )
        .unwrap()
    }
    fn repo() -> NewsRepository {
        NewsRepository(Arc::new(
            SettingsRepository::open(std::path::Path::new(":memory:")).unwrap(),
        ))
    }
    #[test]
    fn preserves_metadata_without_inventing_content() {
        let items = fixture();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "Nauka & świat");
        assert_eq!(items[0].source.as_deref(), Some("Wydawca"));
        assert_eq!(items[0].source_url.as_deref(), Some("https://example.org"));
        assert!(
            items[0]
                .description_html
                .as_ref()
                .unwrap()
                .contains("<script>")
        ); // preserved as data, never rendered HTML
        assert!(items[1].description_html.is_none());
        assert!(items[1].source.is_none());
        assert!(items[2].published_at.is_none());
        assert_eq!(items[2].raw_publication_date.as_deref(), Some("unknown"));
        assert!(parse(b"not XML", "https://example.org", "test").is_err());
    }
    #[test]
    fn dedup_sort_and_usage_history_are_separate_from_fetching() {
        let repo = repo();
        let items = fixture();
        repo.save("nauka", &items).unwrap();
        let mut copy = items[0].clone();
        copy.url = "https://news.google.com/rss/articles/other".into();
        copy.guid = Some("other".into());
        repo.save("świat", &[copy]).unwrap();
        repo.save("nauka", &items).unwrap();
        let topics = vec!["nauka".into(), "świat".into()];
        let results = repo.candidates(&topics).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "Nowsza wiadomość");
        assert!(results.last().unwrap().published_at.is_none());
        assert_eq!(results[1].topics.len(), 2);
        repo.mark_used(&[results[1].id.clone()], "broadcast-1")
            .unwrap();
        repo.save("nauka", &items).unwrap();
        assert_eq!(repo.candidates(&topics).unwrap().len(), 2);
        assert!(
            repo.mark_used(&[results[0].id.clone(), "999999".into()], "bad-broadcast")
                .is_err()
        );
        assert_eq!(repo.candidates(&topics).unwrap().len(), 2); // atomic rollback
    }
    #[test]
    fn persistent_rate_claims_and_provider_cooldown() {
        let repo = repo();
        let topics = vec!["nauka".into()];
        assert_eq!(repo.claim(&topics, 1000).unwrap().len(), 1);
        assert!(repo.claim(&topics, 1061).unwrap().is_empty());
        assert!(repo.claim(&["inny".into()], 1001).unwrap().is_empty());
        assert_eq!(repo.claim(&topics, 1300).unwrap().len(), 1);
        repo.error(
            "nauka",
            &FetchError {
                message: "HTTP 429".into(),
                retry_after: Some(600),
            },
        )
        .unwrap();
        assert!(repo.claim(&["inny".into()], now() + 10).unwrap().is_empty());
        assert_eq!(
            repo.statuses(&topics).unwrap()[0].error.as_deref(),
            Some("HTTP 429")
        );
    }
    #[test]
    fn url_query_is_encoded_and_topics_validated() {
        let url = GoogleNews::url("nauka & zdrowie # Polska");
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "q").unwrap().1,
            "nauka & zdrowie # Polska"
        );
        assert_eq!(
            validate_topics(vec![" Nauka ".into(), "nauka".into()]).unwrap(),
            vec!["Nauka"]
        );
        assert!(validate_topics(vec![]).is_err());
        assert_eq!(
            canonical("https://example.org/a?utm_source=x&id=1#part"),
            "https://example.org/a?id=1"
        );
    }
    #[test]
    fn aliases_keep_used_articles_excluded_after_title_changes() {
        let repo = repo();
        let original = fixture()[0].clone();
        repo.save("nauka", std::slice::from_ref(&original)).unwrap();
        let mut alias = original.clone();
        alias.guid = Some("alias-guid".into());
        alias.url = "https://example.org/alias".into();
        repo.save("nauka", std::slice::from_ref(&alias)).unwrap();
        let items = repo.candidates(&["nauka".into()]).unwrap();
        assert_eq!(items.len(), 1);
        repo.mark_used(&[items[0].id.clone()], "broadcast").unwrap();
        alias.title = "Zaktualizowany nagłówek".into();
        repo.save("nauka", &[alias]).unwrap();
        assert!(repo.candidates(&["nauka".into()]).unwrap().is_empty());
    }
    #[test]
    fn history_and_fetch_limits_survive_database_reopen() {
        let path = std::env::temp_dir().join(format!(
            "arabrief-news-{}-{}.sqlite",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let repo = NewsRepository(Arc::new(SettingsRepository::open(&path).unwrap()));
            repo.save("nauka", &fixture()).unwrap();
            let items = repo.candidates(&["nauka".into()]).unwrap();
            repo.mark_used(&[items[0].id.clone()], "broadcast-1")
                .unwrap();
            repo.claim(&["nauka".into()], 1000).unwrap();
        }
        {
            let repo = NewsRepository(Arc::new(SettingsRepository::open(&path).unwrap()));
            assert_eq!(repo.candidates(&["nauka".into()]).unwrap().len(), 2);
            assert!(repo.claim(&["nauka".into()], 1100).unwrap().is_empty());
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[ignore = "requires public Google News network access"]
    fn live_google_news() {
        let items = GoogleNews::new().unwrap().fetch_topic("nauka").unwrap();
        assert!(!items.is_empty());
        println!("Google News live: {} RSS items", items.len());
    }
}
