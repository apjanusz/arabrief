//! One grounded Responses request per article. No editorial/semantic AI passes.
use super::*;
use reqwest::Url;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Chapter {
    article_read: bool,
    article_url: String,
    title: String,
    text: String,
    quotes: Vec<String>,
    limitations: Vec<String>,
}
pub(super) trait ChapterGenerator: Sync {
    fn chapter(
        &self,
        source: &NewsItem,
        settings: &AppSettings,
    ) -> Result<(Chapter, Value), String>;
}
pub(super) struct OpenAiChapters;
fn request(source: &NewsItem, settings: &AppSettings) -> Value {
    json!({"model":settings.text_model,"store":false,"max_output_tokens":5000,
    "tools":[{"type":"web_search","external_web_access":true,"search_context_size":"high"}],
    "tool_choice":"required","include":["web_search_call.action.sources"],
    "instructions":include_str!("chapter_prompt.txt"),
    "input":json!({"article":{"id":source.id,"title":source.title,"url":source.url,"publisher":source.source,"publisher_url":source.source_url,"published_at":source.published_at,"topics":source.topics},"min_words":settings.broadcast_min_duration_ms*140/180000,"max_words":settings.broadcast_max_duration_ms*140/180000}).to_string(),
    "text":{"format":{"type":"json_schema","name":"article_chapter","strict":true,"schema":{
        "type":"object","additionalProperties":false,"required":["article_read","article_url","title","text","quotes","limitations"],
        "properties":{"article_read":{"type":"boolean"},"article_url":{"type":"string"},"title":{"type":"string"},"text":{"type":"string"},"quotes":{"type":"array","items":{"type":"string"}},"limitations":{"type":"array","items":{"type":"string"}}}
    }}}})
}
fn same_article(source: &NewsItem, opened: &str) -> bool {
    let Ok(url) = Url::parse(opened) else {
        return false;
    };
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    let Ok(original) = Url::parse(&source.url) else {
        return false;
    };
    if original.host_str() != Some("news.google.com") {
        return crate::news::canonical(&source.url) == crate::news::canonical(opened);
    }
    // Google RSS links are envelopes, not publisher content. Require an actual
    // publisher article, never a search snippet or the publisher's home page.
    let Some(publisher) = source
        .source_url
        .as_ref()
        .and_then(|value| Url::parse(value).ok())
    else {
        return false;
    };
    let host = |url: &Url| {
        url.host_str()
            .unwrap_or("")
            .trim_start_matches("www.")
            .to_lowercase()
    };
    let actual = host(&url);
    let expected = host(&publisher);
    (actual == expected || actual.ends_with(&format!(".{expected}")))
        && url.path().trim_matches('/').len() > 1
}
fn parse(value: &Value, source: &NewsItem) -> Result<(Chapter, Value), String> {
    if value["status"] != "completed" {
        return Err("Nieukończona odpowiedź rozdziału".into());
    }
    let output = value["output"]
        .as_array()
        .ok_or("Brak odpowiedzi rozdziału")?;
    let mut text = String::new();
    let mut opened = Vec::new();
    let mut access = Vec::new();
    for item in output {
        if item["type"] == "web_search_call"
            && item["status"] == "completed"
            && item["action"]["type"] == "open_page"
            && let Some(url) = item["action"]["url"].as_str()
        {
            opened.push(url.to_owned());
            access
                .push(json!({"id":item["id"],"url":url,"action":"open_page","status":"completed"}));
        }
        if item["type"] == "message"
            && let Some(contents) = item["content"].as_array()
        {
            for content in contents {
                if content["type"] == "refusal" {
                    return Err("Model odmówił przygotowania rozdziału".into());
                }
                if content["type"] == "output_text" {
                    text.push_str(content["text"].as_str().unwrap_or(""));
                }
            }
        }
    }
    let chapter: Chapter =
        serde_json::from_str(&text).map_err(|_| "Nieprawidłowy JSON rozdziału")?;
    if !chapter.article_read
        || chapter.text.trim().chars().count() < 40
        || chapter.text.len() > 20000
        || chapter.title.trim().is_empty()
        || chapter.title.len() > 500
        || chapter.quotes.is_empty()
        || chapter.quotes.len() > 5
        || chapter
            .quotes
            .iter()
            .any(|quote| !(12..=250).contains(&quote.chars().count()))
        || chapter.limitations.len() > 8
        || chapter.limitations.iter().any(|value| value.len() > 1000)
        || !same_article(source, &chapter.article_url)
        || !opened.iter().any(|url| {
            same_article(source, url)
                && crate::news::canonical(url) == crate::news::canonical(&chapter.article_url)
        })
    {
        return Err(
            "Nie potwierdzono odczytu treści wybranego artykułu; pominięto rozdział".into(),
        );
    }
    Ok((chapter, json!(access)))
}
impl ChapterGenerator for OpenAiChapters {
    fn chapter(
        &self,
        source: &NewsItem,
        settings: &AppSettings,
    ) -> Result<(Chapter, Value), String> {
        let key = crate::credentials::read_openai_key()?;
        let bearer = Zeroizing::new(format!("Bearer {}", key.as_str()));
        let mut auth = HeaderValue::from_str(&bearer).map_err(|_| "Nieprawidłowy klucz OpenAI")?;
        auth.set_sensitive(true);
        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Błąd klienta OpenAI")?;
        let body =
            serde_json::to_vec(&request(source, settings)).map_err(|_| "Błąd JSON żądania")?;
        for attempt in 0..2 {
            let response = client
                .post("https://api.openai.com/v1/responses")
                .header(AUTHORIZATION, auth.clone())
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone())
                .send()
                .map_err(|error| {
                    if error.is_timeout() {
                        "Timeout odczytu artykułu i generowania rozdziału (90 s)"
                    } else {
                        "Błąd połączenia rozdziału z OpenAI"
                    }
                })?;
            let status = response.status();
            if attempt == 0 && (status.as_u16() == 429 || status.is_server_error()) {
                let delay = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1);
                if delay <= 2 {
                    std::thread::sleep(Duration::from_secs(delay));
                    continue;
                }
            }
            if !status.is_success() {
                return Err(crate::openai_errors::response_error(response).user_message());
            }
            let mut bytes = Vec::new();
            response
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Błąd odczytu odpowiedzi rozdziału")?;
            if bytes.len() > 1024 * 1024 {
                return Err("Przekroczono limit odpowiedzi rozdziału".into());
            }
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|_| "Nieprawidłowa odpowiedź OpenAI")?;
            return parse(&value, source);
        }
        Err("Wyczerpano próby rozdziału".into())
    }
}

fn config_key(settings: &AppSettings) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(
            json!([
                "article-chapter-v1",
                settings.text_model,
                settings.broadcast_min_duration_ms,
                settings.broadcast_max_duration_ms,
                settings.language
            ])
            .to_string()
            .as_bytes()
        )
    )
}
pub(crate) fn failed(repository: &SettingsRepository, id: &str, stage: &str) -> Result<(), String> {
    eprintln!("[pipeline] stage={stage} article_id={id} action=deferred_or_skipped");
    repository.with_database(|db|db.execute("UPDATE article_queue SET failures=failures+1,retry_at=unixepoch()+60,skipped=(failures+1>=3) WHERE article_id=?1",[id]).map(|_|()).map_err(|e|e.to_string()))
}
pub(crate) fn systemic(error: &str) -> bool {
    error.starts_with("OpenAI:") || error.to_lowercase().contains("klucz")
}
pub(crate) fn defer(repository: &SettingsRepository, id: &str) -> Result<(), String> {
    repository.with_database(|db| {
        db.execute(
            "UPDATE article_queue SET retry_at=unixepoch()+60 WHERE article_id=?1",
            [id],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    })
}
pub(crate) fn publish_audio(
    repository: &SettingsRepository,
    id: i64,
    included: &[(String, u32, u32)],
) -> Result<(), String> {
    repository.with_database(|db| {
        let tx=db.transaction().map_err(|e|e.to_string())?;
        tx.execute("UPDATE broadcast_chapters SET state='audio_failed',start_ms=NULL,end_ms=NULL WHERE broadcast_id=?1",[id]).map_err(|e|e.to_string())?;
        if included.is_empty() {
            // No playable chapter: release reservations so this failed preparation
            // cannot block fresh FIFO work forever. The chapter cache is retained.
            tx.execute("DELETE FROM pipeline_preparations WHERE broadcast_id=?1",[id]).map_err(|e|e.to_string())?;
        } else {
            for (article,start,end) in included {
                tx.execute("UPDATE broadcast_chapters SET state='audio_ready',start_ms=?3,end_ms=?4 WHERE broadcast_id=?1 AND article_id=?2",params![id,article,start,end]).map_err(|e|e.to_string())?;
                tx.execute("UPDATE article_queue SET failures=0,retry_at=0,skipped=0 WHERE article_id=?1",[article]).map_err(|e|e.to_string())?;
            }
            let raw:String=tx.query_row("SELECT draft_json FROM content_broadcasts WHERE id=?1",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
            let mut draft:Draft=serde_json::from_str(&raw).map_err(|_|"Nieprawidłowy zapis tekstu rozdziałów")?;
            draft.segments.retain(|segment|segment.evidence.iter().any(|e|included.iter().any(|(article,_,_)|article==&e.source_id)));
            let text=draft.segments.iter().map(|segment|segment.text.as_str()).collect::<Vec<_>>().join("\n\n");
            if included.len()<3 {draft.limitations.push("Nagranie zawiera dostępne kompletne rozdziały. Teksty rozdziałów z błędem TTS zachowano do ponownej próby.".into());draft.insufficient_material=true;}
            tx.execute("UPDATE content_broadcasts SET text=?2,draft_json=?3,estimated_duration_ms=?4 WHERE id=?1",params![id,text,serde_json::to_string(&draft).map_err(|e|e.to_string())?,text.split_whitespace().count() as u32*60000/140]).map_err(|e|e.to_string())?;
            tx.execute("DELETE FROM content_sources WHERE broadcast_id=?1 AND article_id NOT IN (SELECT article_id FROM broadcast_chapters WHERE broadcast_id=?1 AND state='audio_ready')",[id]).map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|e|e.to_string())
    })
}
impl ContentService {
    pub(super) fn generate_chapters(
        &self,
        ids: Vec<String>,
        generator: &impl ChapterGenerator,
    ) -> Result<Broadcast, String> {
        let _gate = self
            .gate
            .try_lock()
            .map_err(|_| "Przygotowanie audycji już trwa")?;
        if ids.is_empty() || ids.len() > 20 || ids.iter().any(|id| id.parse::<i64>().is_err()) {
            return Err("Wybierz od 1 do 20 poprawnych identyfikatorów artykułów".into());
        }
        let settings = self.repository.load()?;
        let key = config_key(&settings);
        let preparation_key = crate::recovery::settings_key(&settings);
        let mut sources=self.repository.with_database(|db| {
            let mut sources=Vec::new();let mut seen=HashSet::new();
            for id in &ids {
                if !seen.insert(id) {continue;}
                let raw:Option<String>=db.query_row("SELECT metadata_json FROM news_articles a JOIN article_queue q ON q.article_id=a.id WHERE a.id=?1 AND q.skipped=0 AND q.retry_at<=unixepoch() AND NOT EXISTS(SELECT 1 FROM news_usage WHERE article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c JOIN playback_receipts r ON r.broadcast_id=c.broadcast_id WHERE c.article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c JOIN pipeline_preparations p ON p.broadcast_id=c.broadcast_id JOIN content_broadcasts b ON b.id=c.broadcast_id WHERE c.article_id=a.id AND p.settings_key=?2 AND datetime(b.created_at)>datetime('now','-6 hours'))",params![id,preparation_key],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
                if let Some(raw)=raw && let Ok(mut item)=serde_json::from_str::<NewsItem>(&raw) {
                    if item.provider=="google_news" && !crate::news::within_news_window(&item,Utc::now().timestamp()) {continue;}
                    let matches=settings.topics.iter().any(|topic|db.query_row("SELECT EXISTS(SELECT 1 FROM news_topic_articles WHERE article_id=?1 AND topic=?2)",params![id,crate::news::normalized(topic)],|r|r.get::<_,bool>(0)).unwrap_or(false));
                    if matches {item.id=id.clone();item.topics=settings.topics.iter().filter(|topic|item.topics.iter().any(|t|crate::news::normalized(t)==crate::news::normalized(topic))).cloned().collect();if item.topics.is_empty(){item.topics=settings.topics.clone();}sources.push(item);}
                }
            }
            Ok(sources)
        })?;
        sources.sort_by_key(|source| {
            (
                source
                    .published_at
                    .as_ref()
                    .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
                    .map_or(i64::MAX, |date| date.timestamp()),
                source.id.parse::<i64>().unwrap_or(i64::MAX),
            )
        });
        let mut ready = Vec::new();
        let mut resolved_urls = HashSet::new();
        let mut offset = 0;
        // At most nine candidates in one preparation; failures have a durable
        // cooldown/three-attempt limit and cannot starve the rest of the queue.
        while ready.len() < 3 && offset < sources.len().min(9) {
            let end = (offset + 3 - ready.len()).min(sources.len().min(9));
            let results = crate::work_queue::run(
                &sources[offset..end],
                3,
                |source| {
                    let cached=self.repository.with_database(|db|db.query_row("SELECT chapter_json,access_json FROM article_chapters WHERE article_id=?1 AND config_key=?2",params![source.id,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(|e|e.to_string()))?;
                    if let Some((chapter, access)) = cached
                        && let (Ok(chapter), Ok(access)) = (
                            serde_json::from_str::<Chapter>(&chapter),
                            serde_json::from_str::<Value>(&access),
                        )
                        && chapter.article_read
                        && chapter.text.len() >= 40
                        && same_article(source, &chapter.article_url)
                        && access.as_array().is_some_and(|access| !access.is_empty())
                    {
                        eprintln!(
                            "[pipeline] stage=chapter article_id={} action=reused",
                            source.id
                        );
                        return Ok((chapter, access));
                    }
                    let result = generator.chapter(source, &settings);
                    match result {
                        Ok((chapter, access)) => {
                            self.repository.with_database(|db| {
                                db.execute(
                                    "INSERT OR REPLACE INTO article_chapters VALUES (?1,?2,?3,?4)",
                                    params![
                                        source.id,
                                        key,
                                        serde_json::to_string(&chapter)
                                            .map_err(|e| e.to_string())?,
                                        access.to_string()
                                    ],
                                )
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                            })?;
                            eprintln!(
                                "[pipeline] stage=chapter article_id={} action=saved",
                                source.id
                            );
                            Ok((chapter, access))
                        }
                        Err(error) => {
                            eprintln!(
                                "[pipeline] stage=article_or_model article_id={} error={error}",
                                source.id
                            );
                            if systemic(&error) {
                                self.repository.with_database(|db|db.execute("UPDATE article_queue SET retry_at=unixepoch()+60 WHERE article_id=?1",[&source.id]).map(|_|()).map_err(|e|e.to_string()))?;
                            } else {
                                failed(&self.repository, &source.id, "article_or_model")?;
                            }
                            Err(error)
                        }
                    }
                },
                |_| false,
            )?;
            let systemic_error = results
                .iter()
                .filter_map(|(_, result)| result.as_ref().err())
                .find(|error| systemic(error))
                .cloned();
            for (index, result) in results {
                if let Ok((chapter, _)) = result {
                    let source = &sources[offset + index];
                    if resolved_urls.insert(crate::news::canonical(&chapter.article_url)) {
                        ready.push((source.clone(), chapter));
                    } else {
                        self.repository.with_database(|db| {
                            db.execute(
                                "UPDATE article_queue SET skipped=1 WHERE article_id=?1",
                                [&source.id],
                            )
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                        })?;
                        eprintln!(
                            "[pipeline] stage=dedup article_id={} action=exact_publisher_url_duplicate",
                            source.id
                        );
                    }
                }
            }
            if let Some(error) = systemic_error {
                if ready.is_empty() {
                    return Err(error);
                }
                break;
            }
            offset = end;
        }
        if ready.is_empty() {
            return Err("Brak bezpiecznych materiałów: nie uzyskano treści żadnego artykułu. Kolejka i zapisane rozdziały zostały zachowane.".into());
        }
        self.save_chapters(ready, &settings)
    }
    fn save_chapters(
        &self,
        ready: Vec<(NewsItem, Chapter)>,
        settings: &AppSettings,
    ) -> Result<Broadcast, String> {
        let mut segments = Vec::new();
        let mut sources = Vec::new();
        let mut limitations = Vec::new();
        for (mut source, chapter) in ready {
            source.url = chapter.article_url;
            segments.push(Segment {
                topic: source
                    .topics
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Wiadomości".into()),
                text: chapter.text,
                evidence: chapter
                    .quotes
                    .into_iter()
                    .map(|quote| Evidence {
                        source_id: source.id.clone(),
                        quote,
                    })
                    .collect(),
            });
            limitations.extend(chapter.limitations);
            sources.push(source);
        }
        if sources.len() < 3 {
            limitations.push(format!("Przygotowano {} z 3 rozdziałów; pozostałe materiały zachowano lub odłożono po błędzie.",sources.len()));
        }
        let text = segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let mut broadcast = Broadcast {
            id: String::new(),
            created_at: Utc::now().to_rfc3339(),
            model: settings.text_model.clone(),
            title: "Przegląd wiadomości AraBrief".into(),
            estimated_duration_ms: text.split_whitespace().count() as u32 * 60000 / 140,
            text,
            language: "pl".into(),
            topics: settings.topics.clone(),
            target_min_duration_ms: settings.broadcast_min_duration_ms,
            target_max_duration_ms: settings.broadcast_max_duration_ms,
            draft: Draft {
                title: "Przegląd wiadomości AraBrief".into(),
                segments,
                limitations,
                insufficient_material: sources.len() < 3,
            },
            sources,
        };
        self.repository.with_database(|db| {
            let tx=db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO content_broadcasts(created_at,title,model,text,estimated_duration_ms,topics_json,draft_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![broadcast.created_at,broadcast.title,broadcast.model,broadcast.text,broadcast.estimated_duration_ms,json!(broadcast.topics).to_string(),serde_json::to_string(&broadcast.draft).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
            let id=tx.last_insert_rowid();broadcast.id=id.to_string();
            tx.execute("INSERT INTO pipeline_preparations VALUES (?1,?2)",params![id,crate::recovery::settings_key(settings)]).map_err(|e|e.to_string())?;
            for (position,source) in broadcast.sources.iter().enumerate() {
                tx.execute("INSERT INTO content_sources VALUES (?1,?2,?3)",params![id,source.id,serde_json::to_string(source).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
                tx.execute("INSERT INTO broadcast_chapters(broadcast_id,article_id,position,text) VALUES (?1,?2,?3,?4)",params![id,source.id,position as u32,broadcast.draft.segments[position].text]).map_err(|e|e.to_string())?;
            }
            tx.commit().map_err(|e|e.to_string())
        })?;
        Ok(broadcast)
    }
}
