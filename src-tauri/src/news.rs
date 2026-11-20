use crate::settings::SettingsRepository;
mod relevance;
mod selection;
use chrono::{DateTime, Utc};
use reqwest::{Url, blocking::Client};
use rusqlite::{OptionalExtension, params};
pub(crate) use selection::chronological;
use std::{
    collections::HashSet,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[cfg(test)]
pub(crate) const MAX_NEWS_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

#[cfg(test)]
pub(crate) fn within_news_window(item: &NewsItem, timestamp: i64) -> bool {
    within_days(item, timestamp, 7)
}

pub(crate) fn within_days(item: &NewsItem, timestamp: i64, days: u32) -> bool {
    item.published_at
        .as_ref()
        .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
        .is_some_and(|date| {
            date.timestamp() >= timestamp.saturating_sub(i64::from(days) * 86400)
                && date.timestamp() <= timestamp
        })
}

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
    fn fetch_range(
        &self,
        topic: &str,
        since: Option<i64>,
        _days: u32,
    ) -> Result<Vec<NewsItem>, FetchError> {
        self.fetch_since(topic, since)
    }
    fn fetch_since(&self, topic: &str, _since: Option<i64>) -> Result<Vec<NewsItem>, FetchError> {
        self.fetch_topic(topic)
    }
}
pub struct GoogleNews {
    client: Client,
    region: String,
    last_request: Mutex<Option<Instant>>,
}
impl GoogleNews {
    fn new() -> Result<Self, String> {
        Ok(Self {
            region: "PL".into(),
            last_request: Mutex::new(None),
            client: Client::builder()
                .timeout(Duration::from_secs(12))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("AraBrief/0.1 RSS reader")
                .build()
                .map_err(|e| e.to_string())?,
        })
    }
    #[cfg(test)]
    fn url_since(topic: &str, since: Option<i64>, timestamp: i64) -> Url {
        Self::regional_url(topic, since, timestamp, "PL")
    }
    fn regional_url(topic: &str, since: Option<i64>, timestamp: i64, region: &str) -> Url {
        Self::regional_url_days(topic, since, timestamp, region, 7)
    }
    fn regional_url_days(
        topic: &str,
        since: Option<i64>,
        timestamp: i64,
        region: &str,
        days: u32,
    ) -> Url {
        let country = crate::countries::find(region).expect("validated news country");
        let hl = &country.hl;
        let gl = &country.code;
        let ceid = format!("{}:{}", country.code, country.language);
        let cutoff = timestamp.saturating_sub(i64::from(days) * 86400);
        let since = if days == 7 { since } else { None };
        let lower = since
            .map(|date| date.saturating_sub(86400))
            .unwrap_or(cutoff)
            .clamp(cutoff, timestamp);
        let date = DateTime::<Utc>::from_timestamp(lower, 0).expect("valid search date");
        let mut url = Url::parse("https://news.google.com/rss/search").expect("constant URL");
        url.query_pairs_mut()
            .append_pair(
                "q",
                &format!(
                    "{} after:{}",
                    if region == "PL" {
                        relevance::query(topic)
                    } else {
                        topic.to_owned()
                    },
                    date.format("%Y-%m-%d")
                ),
            )
            .append_pair("hl", hl)
            .append_pair("gl", gl)
            .append_pair("ceid", &ceid);
        url
    }
}
impl NewsProvider for GoogleNews {
    fn fetch_range(
        &self,
        topic: &str,
        since: Option<i64>,
        days: u32,
    ) -> Result<Vec<NewsItem>, FetchError> {
        self.fetch_url(
            topic,
            Self::regional_url_days(topic, since, now(), &self.region, days),
        )
    }
    fn fetch_topic(&self, topic: &str) -> Result<Vec<NewsItem>, FetchError> {
        self.fetch_url(topic, Self::regional_url(topic, None, now(), &self.region))
    }
    fn fetch_since(&self, topic: &str, since: Option<i64>) -> Result<Vec<NewsItem>, FetchError> {
        let url = Self::regional_url(topic, since, now(), &self.region);
        self.fetch_url(topic, url)
    }
}
impl GoogleNews {
    fn fetch_url(&self, topic: &str, url: Url) -> Result<Vec<NewsItem>, FetchError> {
        // Pace every real HTTP request, independent of topic processing time.
        let mut last = self.last_request.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = *last {
            std::thread::sleep(Duration::from_secs(2).saturating_sub(previous.elapsed()));
        }
        *last = Some(Instant::now());
        let mut total = crate::performance::Span::new("rss.topic");
        let mut http = crate::performance::Span::new("rss.http");
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
                .map(|s| s.max(1));
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
        http.success();
        drop(http);
        let result = crate::performance::measure("rss.parse", || {
            parse(&bytes, url.as_str(), topic).map_err(failure)
        });
        if result.is_ok() {
            total.success();
        }
        result
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
pub(crate) fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
pub(crate) fn canonical(raw: &str) -> String {
    let mut url = Url::parse(raw).expect("validated URL");
    let google_news = url.host_str() == Some("news.google.com");
    url.set_fragment(None);
    let mut pairs: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| {
            !(key.starts_with("utm_")
                || ["gclid", "fbclid"].contains(&key.as_ref())
                || google_news && key == "oc")
        })
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort();
    url.set_query(None);
    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs);
    }
    url.into()
}
fn now() -> i64 {
    Utc::now().timestamp()
}
pub struct NewsRepository(Arc<SettingsRepository>);
impl NewsRepository {
    fn prune_irrelevant(&self, topics: &[String]) -> Result<(), String> {
        let region = self.0.load()?.effective_region().to_owned();
        self.0.with_database(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            for topic in topics {
                let records = {
                    let mut statement = tx.prepare("SELECT a.id,a.metadata_json FROM news_articles a JOIN news_topic_articles t ON t.article_id=a.id WHERE t.topic=?1 AND a.provider='google_news' AND NOT EXISTS(SELECT 1 FROM news_usage u WHERE u.article_id=a.id)").map_err(|e| e.to_string())?;
                    statement.query_map([normalized(topic)], |r| Ok((r.get::<_, i64>(0)?,r.get::<_, String>(1)?))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?
                };
                let mut removed = false;
                for (id, json) in records {
                    if serde_json::from_str::<NewsItem>(&json).is_ok_and(|item| !relevance::matches(topic, &item)) {
                        tx.execute("DELETE FROM news_topic_articles WHERE topic=?1 AND article_id=?2", params![normalized(topic),id]).map_err(|e|e.to_string())?;
                        removed = true;
                    }
                }
                if removed {
                    tx.execute("DELETE FROM news_pool_state WHERE topic=?1", [format!("{region}:{}", normalized(topic))]).map_err(|e|e.to_string())?;
                }
            }
            tx.commit().map_err(|e|e.to_string())
        })
    }
    #[cfg(test)]
    fn save(&self, topic: &str, items: &[NewsItem]) -> Result<(), String> {
        self.save_for_region(topic, items, self.0.load()?.effective_region())
    }
    fn save_for_region(&self, topic: &str, items: &[NewsItem], region: &str) -> Result<(), String> {
        let topic = normalized(topic);
        self.0.with_database(|db| {
            let tx = db.transaction().map_err(|e| e.to_string())?;
            for item in items {
                let url = canonical(&item.url);
                let json = serde_json::to_string(item).map_err(|e| e.to_string())?;
                let existing: Option<i64> = tx.query_row("SELECT article_id FROM news_aliases WHERE provider=?2 AND ((kind='url' AND value=?1) OR (kind='guid' AND value=?3)) UNION SELECT id FROM news_articles WHERE canonical_url=?1 OR (provider=?2 AND guid=?3) ORDER BY 1 LIMIT 1", params![url,item.provider,item.guid], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
                let id = if let Some(id) = existing { id } else {
                    tx.execute("INSERT INTO news_articles(provider,guid,canonical_url,metadata_json,first_seen_at) VALUES (?1,?2,?3,?4,?5)", params![item.provider,item.guid,url,json,now()]).map_err(|e| e.to_string())?;
                    tx.last_insert_rowid()
                };
                tx.execute("INSERT OR IGNORE INTO news_region_articles VALUES (?1,?2)",params![id,region]).map_err(|e|e.to_string())?;
                let published=item.published_at.as_ref().and_then(|date|DateTime::parse_from_rfc3339(date).ok()).map(|date|date.timestamp());
                tx.execute("INSERT OR IGNORE INTO article_queue(article_id,published_at) VALUES (?1,?2)",params![id,published]).map_err(|e|e.to_string())?;
                if let Some(published)=published {
                    tx.execute("INSERT INTO news_pool_state VALUES (?1,?2) ON CONFLICT(topic) DO UPDATE SET newest_at=MAX(newest_at,excluded.newest_at)",params![format!("{region}:{topic}"),published]).map_err(|e|e.to_string())?;
                }
                for (kind,value) in [("url",Some(&url)),("guid",item.guid.as_ref())] {
                    if let Some(value) = value { tx.execute("INSERT INTO news_aliases VALUES (?1,?2,?3,?4,?5) ON CONFLICT(provider,kind,value) DO UPDATE SET metadata_json=excluded.metadata_json",params![item.provider,kind,value,id,json]).map_err(|e| e.to_string())?; }
                }
                tx.execute("INSERT INTO news_topic_articles VALUES (?1,?2,?3) ON CONFLICT(topic,article_id) DO UPDATE SET last_seen_at=excluded.last_seen_at", params![topic,id,now()]).map_err(|e| e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())
        })
    }
    #[cfg(test)]
    fn candidates(&self, topics: &[String]) -> Result<Vec<NewsItem>, String> {
        self.candidates_days(topics, 7)
    }
    fn candidates_days(&self, topics: &[String], days: u32) -> Result<Vec<NewsItem>, String> {
        let settings = self.0.load()?;
        let key = crate::recovery::settings_key(&settings);
        let cutoff = now().saturating_sub(i64::from(days) * 86400);
        self.0.with_database(|db| {
            let mut items: Vec<NewsItem> = Vec::new();
            for topic in topics {
                let mut statement = db.prepare("SELECT a.id,a.metadata_json FROM news_articles a JOIN news_topic_articles t ON t.article_id=a.id JOIN article_queue q ON q.article_id=a.id WHERE t.topic=?1 AND (a.provider!='google_news' OR (q.published_at>=?3 AND EXISTS(SELECT 1 FROM news_region_articles e WHERE e.article_id=a.id AND e.region=?4))) AND q.skipped=0 AND q.retry_at<=unixepoch() AND NOT EXISTS(SELECT 1 FROM news_usage u WHERE u.article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c JOIN playback_receipts r ON r.broadcast_id=c.broadcast_id WHERE c.article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c JOIN pipeline_preparations p ON p.broadcast_id=c.broadcast_id JOIN content_broadcasts b ON b.id=c.broadcast_id WHERE c.article_id=a.id AND p.settings_key=?2 AND datetime(b.created_at)>datetime('now','-6 hours')) ORDER BY CASE WHEN q.published_at>=?5 THEN 0 WHEN q.published_at>=?6 THEN 1 ELSE 2 END,q.published_at IS NULL,q.published_at,a.id LIMIT 20").map_err(|e| e.to_string())?;
                let rows = statement.query_map(params![normalized(topic),key,cutoff,settings.effective_region(),now()-7*86400,now()-30*86400], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?))).map_err(|e| e.to_string())?;
                for row in rows {
                    let (id,json) = row.map_err(|e| e.to_string())?;
                    let id = id.to_string();
                    if let Some(existing) = items.iter_mut().find(|i| i.id == id) { existing.topics.push(topic.clone()); continue; }
                    let Ok(mut item)=serde_json::from_str::<NewsItem>(&json) else {crate::performance::diagnostic!("[recovery] stage=news id={id} action=skipped reason=invalid_metadata");continue;};
                    item.id = id; item.topics = vec![topic.clone()]; items.push(item);
                }
            }
            selection::order_candidates(&mut items, now());
            items.truncate(20);
            Ok(items)
        })
    }
    fn newest(&self, topic: &str) -> Result<Option<i64>, String> {
        let region = self.0.load()?.effective_region().to_owned();
        self.0.with_database(|db| {
            db.query_row(
                "SELECT newest_at FROM news_pool_state WHERE topic=?1",
                [format!("{region}:{}", normalized(topic))],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
        })
    }
    fn known(&self, item: &NewsItem) -> Result<bool, String> {
        self.0.with_database(|db|db.query_row("SELECT EXISTS(SELECT 1 FROM news_articles WHERE canonical_url=?1 OR (provider=?2 AND guid=?3) UNION SELECT 1 FROM news_aliases WHERE provider=?2 AND ((kind='url' AND value=?1) OR (kind='guid' AND value=?3)))",params![canonical(&item.url),item.provider,item.guid],|r|r.get(0)).map_err(|e|e.to_string()))
    }
    fn claim(&self, topics: &[String], timestamp: i64) -> Result<Vec<String>, String> {
        self.0.with_database(|db| {
            let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
            let next = |topic: &str| -> Result<i64,String> { tx.query_row("SELECT next_allowed_at FROM news_fetch_state WHERE provider='google_news' AND topic=?1",[topic],|r| r.get(0)).optional().map(|v| v.unwrap_or(0)).map_err(|e| e.to_string()) };
            if next("")? > timestamp { return Ok(vec![]); }
            let mut due = Vec::new();
            for topic in topics { if next(&normalized(topic))? <= timestamp { due.push(topic.clone()); } }
            due.truncate(3); // Bound a serial batch; leave remaining topics eligible for the next batch.
            if !due.is_empty() {
                tx.execute("INSERT INTO news_fetch_state VALUES ('google_news','',?1,NULL) ON CONFLICT(provider,topic) DO UPDATE SET next_allowed_at=excluded.next_allowed_at,last_error=NULL",[timestamp+60]).map_err(|e| e.to_string())?;
                for topic in &due { tx.execute("INSERT INTO news_fetch_state VALUES ('google_news',?1,?2,NULL) ON CONFLICT(provider,topic) DO UPDATE SET next_allowed_at=excluded.next_allowed_at,last_error=NULL",params![normalized(topic),timestamp+300]).map_err(|e| e.to_string())?; }
            }
            tx.commit().map_err(|e| e.to_string())?; Ok(due)
        })
    }
    fn error(&self, topic: &str, error: &FetchError) -> Result<(), String> {
        self.error_at(topic, error, now())
    }
    fn topic_started(&self, topic: &str) -> Result<(), String> {
        self.0.with_database(|db| db.execute("UPDATE news_fetch_state SET next_allowed_at=MAX(next_allowed_at,?1) WHERE provider='google_news' AND topic=?2",params![now()+300,normalized(topic)]).map(|_|()).map_err(|e|e.to_string()))
    }
    fn error_at(&self, topic: &str, error: &FetchError, timestamp: i64) -> Result<(), String> {
        self.0.with_database(|db| {
            let tx=db.transaction().map_err(|e|e.to_string())?;
            tx.execute("UPDATE news_fetch_state SET last_error=?1 WHERE provider='google_news' AND topic=?2",params![error.message,normalized(topic)]).map_err(|e|e.to_string())?;
            if let Some(server_delay)=error.retry_after {
                let failures:i64=tx.query_row("SELECT failures FROM news_retry_state WHERE id=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;
                let failures=(failures+1).min(32);
                let delay=server_delay.max((300_i64.saturating_mul(1_i64 << (failures-1).min(7))).min(21600));
                tx.execute("UPDATE news_retry_state SET failures=?1 WHERE id=1",[failures]).map_err(|e|e.to_string())?;
                tx.execute("UPDATE news_fetch_state SET next_allowed_at=MAX(next_allowed_at,?1),last_error=?2 WHERE provider='google_news' AND topic=''",params![timestamp.saturating_add(delay),error.message]).map_err(|e|e.to_string())?;
            }
            tx.commit().map_err(|e|e.to_string())
        })
    }
    fn finish_batch(&self, success: bool) -> Result<(), String> {
        self.0.with_database(|db| {
            let tx=db.transaction().map_err(|e|e.to_string())?;
            tx.execute("UPDATE news_fetch_state SET next_allowed_at=MAX(next_allowed_at,?1) WHERE provider='google_news' AND topic=''",[now()+60]).map_err(|e|e.to_string())?;
            if success {tx.execute("UPDATE news_retry_state SET failures=0 WHERE id=1",[]).map_err(|e|e.to_string())?;}
            tx.commit().map_err(|e|e.to_string())
        })
    }
    fn statuses(&self, topics: &[String]) -> Result<Vec<TopicStatus>, String> {
        self.0.with_database(|db| {
            let (global,global_error) = db.query_row("SELECT next_allowed_at,last_error FROM news_fetch_state WHERE provider='google_news' AND topic=''",[],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?))).optional().map_err(|e| e.to_string())?.unwrap_or((0,None));
            topics.iter().map(|topic| {
                let (next,error) = db.query_row("SELECT next_allowed_at,last_error FROM news_fetch_state WHERE provider='google_news' AND topic=?1",[normalized(topic)],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?))).optional().map_err(|e| e.to_string())?.unwrap_or((0,None));
                Ok(TopicStatus { topic: topic.clone(), next_fetch_at: next.max(global), error: if global > now() && global_error.is_some() {global_error.clone()} else {error} })
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
    pub(crate) items: Vec<NewsItem>,
    topics: Vec<TopicStatus>,
    fetched_topics: Vec<String>,
}
pub struct NewsService {
    repository: NewsRepository,
    gate: Mutex<()>,
    selection_gate: Mutex<()>,
}
impl NewsService {
    pub fn new(repository: Arc<SettingsRepository>) -> Self {
        Self {
            repository: NewsRepository(repository),
            gate: Mutex::new(()),
            selection_gate: Mutex::new(()),
        }
    }
    fn fetch(&self, topics: Vec<String>) -> Result<NewsResult, String> {
        let settings = self.repository.0.load()?;
        let mut provider = GoogleNews::new()?;
        provider.region = settings.effective_region().into();
        self.fetch_with_settings(topics, &provider, settings, 7, false)
    }
    #[cfg(test)]
    pub(crate) fn fetch_with(
        &self,
        topics: Vec<String>,
        provider: &dyn NewsProvider,
    ) -> Result<NewsResult, String> {
        self.fetch_with_settings(topics, provider, self.repository.0.load()?, 7, false)
    }
    fn fetch_with_settings(
        &self,
        topics: Vec<String>,
        provider: &dyn NewsProvider,
        settings: crate::settings::AppSettings,
        days: u32,
        force: bool,
    ) -> Result<NewsResult, String> {
        let mut total = crate::performance::Span::new("rss.total");
        let _gate = self
            .gate
            .try_lock()
            .map_err(|_| "Pobieranie wiadomości już trwa")?;
        let topics = validate_topics(topics)?;
        self.repository.prune_irrelevant(&topics)?;
        let existing = self.repository.candidates_days(&topics, days)?;
        let due = if force || existing.len() < 3 {
            self.repository.claim(&topics, now())?
        } else {
            vec![]
        };
        let mut fetched_topics = Vec::new();
        let results = crate::performance::measure("rss.fetch_queue", || {
            crate::work_queue::run(
                &due,
                1,
                |topic| {
                    self.repository
                        .topic_started(topic)
                        .and_then(|_| self.repository.newest(topic))
                        .map_err(|message| FetchError {
                            message,
                            retry_after: None,
                        })
                        .and_then(|since| provider.fetch_range(topic, since, days))
                },
                |result| {
                    result
                        .as_ref()
                        .err()
                        .is_some_and(|error| error.retry_after.is_some())
                },
            )
        })?;
        let mut persist = crate::performance::Span::new("rss.persist_sqlite");
        let mut incoming = Vec::new();
        let healthy_batch = !results.is_empty() && results.iter().all(|(_, result)| result.is_ok());
        for (index, result) in results {
            let topic = &due[index];
            fetched_topics.push(topic.clone());
            match result {
                Ok(items) => {
                    self.repository
                        .range_fetched(topic, settings.effective_region(), days)?;
                    incoming.extend(items.into_iter().map(|item| (topic.clone(), item)));
                }
                Err(error) => self.repository.error(topic, &error)?,
            }
        }
        if !due.is_empty() {
            self.repository.finish_batch(healthy_batch)?;
        }
        let timestamp = now();
        incoming.retain(|(topic, item)| {
            let keep = within_days(item, timestamp, days)
                && (item.provider != "google_news" || relevance::matches(topic, item));
            if !keep {
                crate::performance::diagnostic!(
                    "[pipeline] stage=rss action=skipped reason=publication_date_or_topic_mismatch"
                );
            }
            keep
        });
        incoming.sort_by_key(|(_, item)| {
            (
                selection::age_tier(item, timestamp),
                item.published_at
                    .as_ref()
                    .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
                    .map_or(i64::MAX, |date| date.timestamp()),
            )
        });
        let mut admitted = 0;
        for (topic, item) in incoming {
            let known = self.repository.known(&item)?;
            // Keep topic aliases for known records; only new identities consume pool slots.
            if known || admitted < 20 - existing.len() {
                self.repository.save_for_region(
                    &topic,
                    std::slice::from_ref(&item),
                    settings.effective_region(),
                )?;
                if !known {
                    admitted += 1;
                }
            }
        }
        persist.success();
        drop(persist);
        if crate::recovery::settings_key(&settings)
            != crate::recovery::settings_key(&self.repository.0.load()?)
        {
            return Err(
                "Broadcast language or news region changed. Prepare a new broadcast.".into(),
            );
        }
        let items = crate::performance::measure("rss.candidates_sqlite", || {
            self.repository.candidates_days(&topics, days)
        })?;
        let statuses = self.repository.statuses(&topics)?;
        if due.is_empty() {
            total.cached();
        } else {
            total.success();
        }
        Ok(NewsResult {
            items,
            topics: statuses,
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
    app: tauri::AppHandle,
    topics: Vec<String>,
    for_broadcast: Option<bool>,
    service: tauri::State<'_, Arc<NewsService>>,
) -> Result<NewsResult, String> {
    crate::background::ensure_running(&app)?;
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if for_broadcast.unwrap_or(false) {
            service.prepare_sources(topics)
        } else {
            service.fetch(topics)
        }
    })
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
    #[test]
    fn topic_and_batch_limits_hold_at_exact_boundaries() {
        let repo = repo();
        let topics = vec!["one".into(), "two".into(), "three".into(), "four".into()];
        assert_eq!(repo.claim(&topics, 1000).unwrap().len(), 3);
        assert!(repo.claim(&topics, 1059).unwrap().is_empty());
        assert_eq!(repo.claim(&topics, 1060).unwrap(), vec!["four"]);
        assert!(repo.claim(&topics, 1299).unwrap().is_empty());
        assert_eq!(repo.claim(&topics, 1300).unwrap().len(), 3);
    }
    #[test]
    fn repeated_provider_limits_back_off_and_never_shorten_retry_after() {
        let repo = repo();
        let topics = vec!["science".into()];
        repo.claim(&topics, 1000).unwrap();
        let error = FetchError {
            message: "Google News RSS: HTTP 429".into(),
            retry_after: Some(60),
        };
        repo.error_at("science", &error, 1000).unwrap();
        assert_eq!(repo.statuses(&topics).unwrap()[0].next_fetch_at, 1300);
        repo.error_at("science", &error, 1300).unwrap();
        assert_eq!(repo.statuses(&topics).unwrap()[0].next_fetch_at, 1900);
        repo.error_at(
            "science",
            &FetchError {
                message: error.message.clone(),
                retry_after: Some(172800),
            },
            1900,
        )
        .unwrap();
        assert_eq!(repo.statuses(&topics).unwrap()[0].next_fetch_at, 174700);
        assert!(
            repo.claim(&["different region topic".into()], 2000)
                .unwrap()
                .is_empty()
        );
        repo.finish_batch(true).unwrap();
        repo.0
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT failures FROM news_retry_state", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn rate_limit_stops_the_batch_and_cache_remains_usable() {
        struct Limited(std::sync::atomic::AtomicUsize);
        impl NewsProvider for Limited {
            fn fetch_topic(&self, _: &str) -> Result<Vec<NewsItem>, FetchError> {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(FetchError {
                    message: "Google News RSS: HTTP 429".into(),
                    retry_after: Some(600),
                })
            }
        }
        let repository = repo();
        let service = NewsService::new(repository.0.clone());
        let provider = Limited(std::sync::atomic::AtomicUsize::new(0));
        let result = service
            .fetch_with(vec!["one".into(), "two".into(), "three".into()], &provider)
            .unwrap();
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(result.fetched_topics.len(), 1);
        assert!(
            result
                .topics
                .iter()
                .all(|topic| topic.next_fetch_at > now())
        );
        let mut item = queue_fixture()[0].clone();
        item.title = "one science story".into();
        repository.save("one", &[item]).unwrap();
        let cached = service.fetch_with(vec!["one".into()], &provider).unwrap();
        assert_eq!(cached.items.len(), 1);
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    #[test]
    fn all_editions_use_correct_parameters_and_never_mix_cached_articles() {
        let repo = Arc::new(SettingsRepository::open(std::path::Path::new(":memory:")).unwrap());
        let pool = NewsRepository(repo.clone());
        let mut settings = repo.load().unwrap();
        settings.ui_language = "pl".into();
        for (region, hl, ceid) in [
            ("PL", "pl", "PL:pl"),
            ("US", "en-US", "US:en"),
            ("GB", "en-GB", "GB:en"),
        ] {
            let url = GoogleNews::regional_url("science & technology", None, now(), region);
            let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
            assert_eq!(query["gl"], region);
            assert_eq!(query["hl"], hl);
            assert_eq!(query["ceid"], ceid);
            let mut item = queue_fixture()[0].clone();
            item.url = format!("https://example.org/{region}");
            item.guid = Some(region.into());
            pool.save_for_region("Nauka", &[item], region).unwrap();
        }
        for region in ["PL", "US", "GB"] {
            settings.news_region = region.into();
            repo.save(&settings).unwrap();
            let candidates = pool.candidates(&["Nauka".into()]).unwrap();
            assert_eq!(candidates.len(), 1);
            assert!(candidates[0].url.ends_with(region));
            assert!(pool.newest("Nauka").unwrap().is_some());
        }
    }
    pub(super) fn fixture() -> Vec<NewsItem> {
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
    fn queue_fixture() -> Vec<NewsItem> {
        let mut items = fixture();
        let timestamp = now();
        for (index, item) in items.iter_mut().enumerate() {
            item.title = format!("Nauka: {}", item.title);
            if item.published_at.is_some() {
                item.published_at = Some(
                    DateTime::<Utc>::from_timestamp(timestamp - (3 - index as i64) * 86400, 0)
                        .unwrap()
                        .to_rfc3339(),
                );
            }
        }
        items
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
        let items = queue_fixture();
        repo.save("nauka", &items).unwrap();
        let mut copy = items[0].clone();
        copy.url = "https://news.google.com/rss/articles/other".into();
        copy.guid = Some("other".into());
        repo.save("świat", &[copy]).unwrap();
        repo.save("nauka", &items).unwrap();
        let topics = vec!["nauka".into(), "świat".into()];
        let results = repo.candidates(&topics).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, items[0].title);
        assert!(results.iter().all(|item| item.published_at.is_some()));
        assert_eq!(results[1].topics.len(), 1);
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
    fn corrupt_article_does_not_hide_healthy_candidates() {
        let repo = repo();
        repo.save("nauka", &queue_fixture()).unwrap();
        repo.0
            .with_database(|db| {
                db.execute(
                    "UPDATE news_articles SET metadata_json='invalid' WHERE id=1",
                    [],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        assert_eq!(repo.candidates(&["nauka".into()]).unwrap().len(), 1);
    }
    #[test]
    fn topic_filter_cleans_existing_queue_and_filters_new_rss_without_losing_history() {
        let repository = repo();
        let mut bad = queue_fixture()[0].clone();
        bad.title = "As Sportu: plebiscyt".into();
        bad.description_html = None;
        let mut good = queue_fixture()[1].clone();
        good.title = "Nowa ściana wspinaczkowa w Polsce".into();
        repository
            .save("Wspinaczka sportowa", &[bad.clone(), good.clone()])
            .unwrap();
        repository
            .mark_used(
                &[repository
                    .candidates(&["Wspinaczka sportowa".into()])
                    .unwrap()[0]
                    .id
                    .clone()],
                "old",
            )
            .unwrap();
        // A second, unused wrong result must be removed, while used history remains.
        let mut unused = bad.clone();
        unused.url = "https://example.org/wrong".into();
        unused.guid = Some("wrong".into());
        repository.save("Wspinaczka sportowa", &[unused]).unwrap();
        repository
            .prune_irrelevant(&["Wspinaczka sportowa".into()])
            .unwrap();
        let remaining = repository
            .candidates(&["Wspinaczka sportowa".into()])
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].title, good.title);
        assert_eq!(repository.newest("Wspinaczka sportowa").unwrap(), None);
        repository
            .0
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM news_usage", [], |row| row
                        .get::<_, i64>(0))
                        .unwrap(),
                    1
                );
                Ok(())
            })
            .unwrap();

        struct Feed(Vec<NewsItem>);
        impl NewsProvider for Feed {
            fn fetch_topic(&self, _: &str) -> Result<Vec<NewsItem>, FetchError> {
                Ok(self.0.clone())
            }
        }
        let service = NewsService::new(repo().0);
        let result = service
            .fetch_with(vec!["Wspinaczka sportowa".into()], &Feed(vec![bad, good]))
            .unwrap();
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].title.contains("wspinaczkowa"));
    }
    #[test]
    fn persistent_rate_claims_and_provider_cooldown() {
        let repo = repo();
        let topics = vec!["nauka".into()];
        assert_eq!(repo.claim(&topics, 1000).unwrap().len(), 1);
        assert!(repo.claim(&topics, 1001).unwrap().is_empty());
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
        let timestamp = 1_700_000_000;
        let url = GoogleNews::url_since("nauka & zdrowie # Polska", None, timestamp);
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "q").unwrap().1,
            format!(
                "\"nauka\" \"zdrowie\" \"Polska\" after:{}",
                DateTime::<Utc>::from_timestamp(timestamp - MAX_NEWS_AGE_SECONDS, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
            )
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
        let original = queue_fixture()[0].clone();
        repo.save("nauka", std::slice::from_ref(&original)).unwrap();
        let mut alias = original.clone();
        alias.guid = original.guid.clone();
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
            repo.save("nauka", &queue_fixture()).unwrap();
            let items = repo.candidates(&["nauka".into()]).unwrap();
            repo.mark_used(&[items[0].id.clone()], "broadcast-1")
                .unwrap();
            repo.claim(&["nauka".into()], 1000).unwrap();
        }
        {
            let repo = NewsRepository(Arc::new(SettingsRepository::open(&path).unwrap()));
            assert_eq!(repo.candidates(&["nauka".into()]).unwrap().len(), 1);
            assert!(repo.claim(&["nauka".into()], 1001).unwrap().is_empty());
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
    #[test]
    fn seven_day_boundary_is_exact_and_undated_results_are_excluded() {
        let timestamp = 1_700_000_000;
        let mut item = fixture()[0].clone();
        for (published, expected) in [
            (timestamp - MAX_NEWS_AGE_SECONDS, true),
            (timestamp - MAX_NEWS_AGE_SECONDS - 1, false),
            (timestamp - 1, true),
        ] {
            item.published_at = Some(
                DateTime::<Utc>::from_timestamp(published, 0)
                    .unwrap()
                    .to_rfc3339(),
            );
            assert_eq!(within_news_window(&item, timestamp), expected);
        }
        item.published_at = None;
        assert!(!within_news_window(&item, timestamp));
        item.published_at = Some("bad date".into());
        assert!(!within_news_window(&item, timestamp));
    }
    #[test]
    fn first_fetch_and_old_watermark_cannot_expand_the_seven_day_window() {
        let timestamp = 1_700_000_000;
        let query = |since| {
            GoogleNews::url_since("Nauka", since, timestamp)
                .query_pairs()
                .find(|(key, _)| key == "q")
                .unwrap()
                .1
                .into_owned()
        };
        assert_eq!(query(None), query(Some(timestamp - 30 * 86400)));
        let overlapping = DateTime::<Utc>::from_timestamp(timestamp - 3 * 86400, 0).unwrap();
        assert_eq!(
            query(Some(timestamp - 2 * 86400)),
            format!("\"Nauka\" after:{}", overlapping.format("%Y-%m-%d"))
        );
    }
    #[test]
    fn stale_feed_results_are_not_admitted_and_existing_history_is_preserved() {
        struct Mixed(Vec<NewsItem>);
        impl NewsProvider for Mixed {
            fn fetch_topic(&self, _: &str) -> Result<Vec<NewsItem>, FetchError> {
                Ok(self.0.clone())
            }
        }
        let mut items = queue_fixture();
        items[1].published_at = Some(
            DateTime::<Utc>::from_timestamp(now() - MAX_NEWS_AGE_SECONDS - 60, 0)
                .unwrap()
                .to_rfc3339(),
        );
        let repository =
            Arc::new(SettingsRepository::open(std::path::Path::new(":memory:")).unwrap());
        let service = NewsService::new(repository.clone());
        let result = service
            .fetch_with(vec!["nauka".into()], &Mixed(items.clone()))
            .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, items[0].title);
        repository
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT count(*) FROM news_articles", [], |r| r
                        .get::<_, u32>(0))
                        .unwrap(),
                    1
                );
                Ok(())
            })
            .unwrap();
        // Previously stored entries age out of selection, without deleting history.
        let repo = NewsRepository(repository.clone());
        repo.save("nauka", &items).unwrap();
        assert_eq!(repo.candidates(&["nauka".into()]).unwrap().len(), 1);
        repository
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT count(*) FROM news_articles", [], |r| r
                        .get::<_, u32>(0))
                        .unwrap(),
                    3
                );
                Ok(())
            })
            .unwrap();
    }
}
