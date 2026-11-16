mod repair;
use crate::{
    news::NewsItem,
    settings::{AppSettings, SettingsRepository},
};
use chrono::Utc;
use reqwest::{
    blocking::Client,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    io::Read,
    sync::{Arc, Mutex},
    time::Duration,
};
use zeroize::Zeroizing;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    source_id: String,
    quote: String,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    topic: String,
    text: String,
    evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    title: String,
    segments: Vec<Segment>,
    limitations: Vec<String>,
    insufficient_material: bool,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Broadcast {
    id: String,
    created_at: String,
    model: String,
    title: String,
    text: String,
    language: String,
    topics: Vec<String>,
    estimated_duration_ms: u32,
    target_min_duration_ms: u32,
    target_max_duration_ms: u32,
    draft: Draft,
    sources: Vec<NewsItem>,
}
pub struct Context {
    sources: Vec<NewsItem>,
    previous: Vec<String>,
    previous_quotes: HashSet<String>,
    settings: AppSettings,
}
/// Replaceable native generator; the model never controls repository IDs or URLs.
pub trait ContentGenerator {
    fn generate(&self, context: &Context) -> Result<Draft, String>;
}
struct OpenAiGenerator;
impl ContentGenerator for OpenAiGenerator {
    fn generate(&self, context: &Context) -> Result<Draft, String> {
        let mut request_time = crate::performance::Span::new("content.request_build");
        let request = request_body(context);
        let body = serde_json::to_vec(&request).map_err(|_| "Błąd przygotowania materiałów")?;
        request_time.success();
        drop(request_time);
        let key = crate::credentials::read_openai_key()?;
        let bearer = Zeroizing::new(format!("Bearer {}", key.as_str()));
        let mut auth = HeaderValue::from_str(&bearer).map_err(|_| "Nieprawidłowy klucz OpenAI")?;
        auth.set_sensitive(true);
        let client = Client::builder()
            .timeout(Duration::from_secs(90))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Nie można utworzyć klienta OpenAI")?;
        let mut http_time = crate::performance::Span::new("content.responses_http");
        let response = client
            .post("https://api.openai.com/v1/responses")
            .header(AUTHORIZATION, auth)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    "OpenAI: przekroczono czas 90 s. Nie ponowiono płatnego żądania."
                } else {
                    "Błąd połączenia z OpenAI"
                }
            })?;
        if !response.status().is_success() {
            return Err(crate::openai_errors::response_error(response).user_message());
        }
        // Never expose API error bodies, headers or credential values to the UI/logs.
        let mut bytes = Vec::new();
        response
            .take(512 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Błąd odczytu odpowiedzi OpenAI")?;
        if bytes.len() > 512 * 1024 {
            return Err("Odpowiedź OpenAI przekracza limit".into());
        }
        http_time.success();
        drop(http_time);
        crate::performance::measure("content.response_parse", || {
            parse_response(
                &serde_json::from_slice::<Value>(&bytes)
                    .map_err(|_| "Nieprawidłowa odpowiedź OpenAI")?,
            )
        })
    }
}
fn request_body(context: &Context) -> Value {
    let ids: Vec<_> = context.sources.iter().map(|s| &s.id).collect();
    let sources: Vec<_> = context.sources.iter().map(source_material).collect();
    json!({"model":context.settings.text_model,"store":false,"max_output_tokens":6000,
        "instructions":include_str!("arabrief_prompt.txt"),
        "input":[{"role":"user","content":[{"type":"input_text","text":json!({
            "target_min_words":context.settings.broadcast_min_duration_ms*140/60000,
            "target_max_words":context.settings.broadcast_max_duration_ms*140/60000,
            "sources":sources,"previous_broadcasts":context.previous
        }).to_string()}]}],
        "text":{"format":{"type":"json_schema","name":"arabrief_script","strict":true,"schema":{
            "type":"object","additionalProperties":false,"required":["title","segments","limitations","insufficient_material"],
            "properties":{"title":{"type":"string"},"insufficient_material":{"type":"boolean"},"limitations":{"type":"array","items":{"type":"string"}},
                "segments":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["topic","text","evidence"],"properties":{
                    "topic":{"type":"string"},"text":{"type":"string"},"evidence":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["source_id","quote"],"properties":{"source_id":{"type":"string","enum":ids},"quote":{"type":"string"}}}}
                }}}
            }
        }}}
    })
}
fn source_material(source: &NewsItem) -> Value {
    json!({"id":source.id,"title":source.title,"url":source.url,"source":source.source,"sourceUrl":source.source_url,"publishedAt":source.published_at,"rawPublicationDate":source.raw_publication_date,"descriptionHtml":source.description_html,"topics":source.topics})
}
fn parse_response(value: &Value) -> Result<Draft, String> {
    let incomplete = value["status"] == "incomplete";
    if value["status"] != "completed" && !incomplete {
        return Err("OpenAI nie ukończyło generowania; scenariusz nie został zapisany".into());
    }
    let mut output = String::new();
    for item in value["output"]
        .as_array()
        .ok_or("Brak treści w odpowiedzi OpenAI")?
    {
        if item["type"] != "message" || item["role"] != "assistant" {
            continue;
        }
        for content in item["content"]
            .as_array()
            .ok_or("Nieprawidłowa treść OpenAI")?
        {
            match content["type"].as_str() {
                Some("refusal") => return Err("OpenAI odmówiło wygenerowania scenariusza".into()),
                Some("output_text") => {
                    output.push_str(content["text"].as_str().ok_or("Brak tekstu OpenAI")?)
                }
                _ => {}
            }
        }
    }
    let value: Value =
        serde_json::from_str(&output).map_err(|_| "Scenariusz nie spełnia kontraktu JSON")?;
    let segments = value["segments"]
        .as_array()
        .ok_or("Scenariusz nie zawiera listy segmentów")?;
    let mut valid = Vec::new();
    for (index, value) in segments.iter().take(100).enumerate() {
        match serde_json::from_value::<Segment>(value.clone()) {
            Ok(segment) => valid.push(segment),
            Err(_) => eprintln!(
                "[recovery] stage=content segment={index} action=skipped reason=invalid_segment_json"
            ),
        }
    }
    let mut limitations: Vec<String> = value["limitations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .take(18)
        .collect();
    if incomplete {
        limitations.push("Odpowiedź nie została w pełni ukończona. Zachowano wyłącznie kompletne, zweryfikowane segmenty JSON.".into());
        eprintln!(
            "[recovery] stage=content action=salvaged_complete_json reason=incomplete_response"
        );
    }
    if valid.len() != segments.len() {
        limitations.push("Pominięto fragmenty o nieprawidłowej strukturze odpowiedzi.".into());
    }
    Ok(Draft {
        title: value["title"].as_str().unwrap_or("").into(),
        segments: valid,
        limitations,
        insufficient_material: incomplete
            || value["insufficient_material"].as_bool().unwrap_or(true),
    })
}
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
#[cfg(test)]
fn validate(draft: &Draft, context: &Context) -> Result<(), String> {
    repair::validate(draft, context)
}
pub struct ContentService {
    repository: Arc<SettingsRepository>,
    gate: Mutex<()>,
}
impl ContentService {
    pub fn new(repository: Arc<SettingsRepository>) -> Self {
        Self {
            repository,
            gate: Mutex::new(()),
        }
    }
    fn context(&self, ids: &[String]) -> Result<Context, String> {
        if !(1..=8).contains(&ids.len()) {
            return Err("Wybierz od 1 do 8 wiadomości z RSS".into());
        }
        let settings = self.repository.load()?;
        self.repository.with_database(|db| {
            let mut sources=Vec::new();
            let mut seen=HashSet::new();
            for id in ids {
                let Ok(id)=id.parse::<i64>() else {eprintln!("[recovery] stage=source action=skipped reason=invalid_id");continue;};
                if !seen.insert(id) { continue; }
                let raw: Option<String>=db.query_row("SELECT metadata_json FROM news_articles a WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM news_usage u WHERE u.article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c WHERE c.article_id=a.id)",[id],|r| r.get(0)).optional().map_err(|e| e.to_string())?;
                let Some(raw)=raw else {eprintln!("[recovery] stage=source id={id} action=skipped reason=missing_or_reserved");continue;};
                let Ok(mut source)=serde_json::from_str::<NewsItem>(&raw) else {eprintln!("[recovery] stage=source id={id} action=skipped reason=invalid_metadata");continue;};
                if source.title.trim().is_empty() {continue;}
                source.id=id.to_string();
                // Bound material size before sending to API; stored source snapshot retains what was sent.
                if let Some(description)=source.description_html.as_mut() { *description=description.chars().take(4000).collect(); }
                sources.push(source);
            }
            if sources.is_empty(){return Err("Brak bezpiecznych materiałów: nie znaleziono nowych poprawnych źródeł.".into());}
            let previous=db.prepare("SELECT text FROM content_broadcasts ORDER BY id DESC LIMIT 5").map_err(|e| e.to_string())?.query_map([],|r| r.get::<_,String>(0)).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
            let previous_quotes=db.prepare("SELECT normalized_quote FROM content_evidence").map_err(|e| e.to_string())?.query_map([],|r| r.get::<_,String>(0)).map_err(|e| e.to_string())?.collect::<Result<HashSet<_>,_>>().map_err(|e| e.to_string())?;
            Ok(Context { sources, previous, previous_quotes, settings })
        })
    }
    fn generate(
        &self,
        ids: Vec<String>,
        generator: &dyn ContentGenerator,
    ) -> Result<Broadcast, String> {
        let mut total = crate::performance::Span::new("content.total");
        let _gate = self
            .gate
            .try_lock()
            .map_err(|_| "Generowanie scenariusza już trwa")?;
        let context = crate::performance::measure("content.context_sqlite", || self.context(&ids))?;
        let mut draft =
            crate::performance::measure("content.model", || generator.generate(&context))?;
        crate::performance::measure("content.validation", || {
            repair::repair(&mut draft, &context)
        })?;
        let text = draft
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let estimated_duration_ms = (text.split_whitespace().count() as u32) * 60000 / 140;
        if estimated_duration_ms < context.settings.broadcast_min_duration_ms {
            draft.insufficient_material = true;
            draft.limitations.push(
                "Scenariusz jest krótszy od docelowego czasu; nie uzupełniano brakujących faktów."
                    .into(),
            );
        }
        let referenced: HashSet<_> = draft
            .segments
            .iter()
            .flat_map(|s| s.evidence.iter().map(|e| e.source_id.as_str()))
            .collect();
        let sources: Vec<_> = context
            .sources
            .iter()
            .filter(|s| referenced.contains(s.id.as_str()))
            .cloned()
            .collect();
        let topics: Vec<_> = draft
            .segments
            .iter()
            .map(|s| s.topic.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut broadcast = Broadcast {
            id: String::new(),
            created_at: Utc::now().to_rfc3339(),
            model: context.settings.text_model.clone(),
            title: draft.title.clone(),
            text,
            language: "pl".into(),
            topics,
            estimated_duration_ms,
            target_min_duration_ms: context.settings.broadcast_min_duration_ms,
            target_max_duration_ms: context.settings.broadcast_max_duration_ms,
            draft,
            sources,
        };
        let mut save = crate::performance::Span::new("content.persist_sqlite");
        self.repository.with_database(|db| {
            let tx=db.transaction().map_err(|e| e.to_string())?;
            // Recheck availability after the network request, before committing a draft.
            let mut unavailable=HashSet::new();
            for source in &broadcast.sources {
                let taken:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM content_sources WHERE article_id=?1 UNION SELECT 1 FROM news_usage WHERE article_id=?1)",[&source.id],|r| r.get(0)).map_err(|e| e.to_string())?;
                if taken {unavailable.insert(source.id.clone());}
            }
            if !unavailable.is_empty() {
                broadcast.draft.segments.retain(|segment|!segment.evidence.iter().any(|e|unavailable.contains(&e.source_id)));
                repair::repair(&mut broadcast.draft,&context)?;
                broadcast.text=broadcast.draft.segments.iter().map(|s|s.text.as_str()).collect::<Vec<_>>().join("\n\n");
                broadcast.estimated_duration_ms=broadcast.text.split_whitespace().count() as u32*60000/140;
                let used:HashSet<_>=broadcast.draft.segments.iter().flat_map(|s|s.evidence.iter().map(|e|e.source_id.as_str())).collect();
                broadcast.sources.retain(|source|used.contains(source.id.as_str()));
                broadcast.topics=broadcast.draft.segments.iter().map(|s|s.topic.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                broadcast.draft.limitations.push("Pominięto materiały wykorzystane w trakcie przygotowania.".into());
                eprintln!("[recovery] stage=content action=removed_concurrently_used_sources count={}",unavailable.len());
            }
            tx.execute("INSERT INTO content_broadcasts(created_at,title,model,text,estimated_duration_ms,topics_json,draft_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![broadcast.created_at,broadcast.title,broadcast.model,broadcast.text,broadcast.estimated_duration_ms,serde_json::to_string(&broadcast.topics).unwrap(),serde_json::to_string(&broadcast.draft).unwrap()]).map_err(|e| e.to_string())?;
            let id=tx.last_insert_rowid(); broadcast.id=id.to_string();
            tx.execute("INSERT INTO pipeline_preparations VALUES (?1,?2)",params![id,crate::recovery::settings_key(&context.settings)]).map_err(|e|e.to_string())?;
            for source in &broadcast.sources { tx.execute("INSERT INTO content_sources VALUES (?1,?2,?3)",params![id,source.id,serde_json::to_string(source).unwrap()]).map_err(|e| e.to_string())?; }
            for evidence in broadcast.draft.segments.iter().flat_map(|s| &s.evidence) { tx.execute("INSERT OR IGNORE INTO content_evidence VALUES (?1,?2)",params![id,normalized(&evidence.quote)]).map_err(|e| e.to_string())?; }
            tx.commit().map_err(|e| e.to_string())
        })?;
        save.success();
        drop(save);
        total.success();
        Ok(broadcast)
    }
}
#[tauri::command]
pub async fn content_generate(
    app: tauri::AppHandle,
    article_ids: Vec<String>,
    service: tauri::State<'_, Arc<ContentService>>,
) -> Result<Broadcast, ContentFailure> {
    crate::background::ensure_running(&app).map_err(ContentFailure::from)?;
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.generate(article_ids, &OpenAiGenerator))
        .await
        .map_err(|_| ContentFailure::from("Błąd modułu generowania".to_string()))?
        .map_err(ContentFailure::from)
}
#[derive(serde::Serialize)]
pub struct ContentFailure {
    code: &'static str,
    message: String,
}
impl From<String> for ContentFailure {
    fn from(message: String) -> Self {
        Self {
            code: if message.starts_with("Brak bezpiecznych materiałów:") {
                "no_usable_material"
            } else {
                "content_failure"
            },
            message,
        }
    }
}

pub(crate) fn load_saved(
    repository: &SettingsRepository,
    id: i64,
    settings: &AppSettings,
) -> Result<Broadcast, String> {
    repository.with_database(|db| {
        let (created_at,title,model,text,estimated_duration_ms,topics_json,draft_json)=db.query_row("SELECT created_at,title,model,text,estimated_duration_ms,topics_json,draft_json FROM content_broadcasts WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,u32>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?))).map_err(|e|e.to_string())?;
        let draft:Draft=serde_json::from_str(&draft_json).map_err(|_|"Uszkodzony zapis scenariusza")?;
        let raw=db.prepare("SELECT snapshot_json FROM content_sources WHERE broadcast_id=?1").map_err(|e|e.to_string())?.query_map([id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        let sources:Vec<NewsItem>=raw.iter().map(|s|serde_json::from_str(s)).collect::<Result<_,_>>().map_err(|_|"Uszkodzony zapis źródeł")?;
        let context=Context{sources:sources.clone(),previous:vec![],previous_quotes:HashSet::new(),settings:settings.clone()};
        repair::validate(&draft,&context)?;
        if text!=draft.segments.iter().map(|s|s.text.as_str()).collect::<Vec<_>>().join("\n\n"){return Err("Niespójny tekst scenariusza".into());}
        Ok(Broadcast{id:id.to_string(),created_at,title,model,text,estimated_duration_ms,topics:serde_json::from_str(&topics_json).map_err(|_|"Uszkodzony zapis tematów")?,draft,sources,language:"pl".into(),target_min_duration_ms:settings.broadcast_min_duration_ms,target_max_duration_ms:settings.broadcast_max_duration_ms})
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compare_material_payload_without_removing_source_facts() {
        let service = service();
        let context = service.context(&ids()).unwrap();
        let before = serde_json::to_vec(&context.sources).unwrap();
        let after: Vec<_> = context.sources.iter().map(source_material).collect();
        let after_bytes = serde_json::to_vec(&after).unwrap();
        for (source, material) in context.sources.iter().zip(&after) {
            let original = serde_json::to_value(source).unwrap();
            for key in [
                "id",
                "title",
                "url",
                "source",
                "sourceUrl",
                "publishedAt",
                "rawPublicationDate",
                "descriptionHtml",
                "topics",
            ] {
                assert_eq!(original[key], material[key]);
            }
        }
        assert!(after_bytes.len() < before.len());
        println!(
            "BENCH content_source_payload before_bytes={} after_bytes={}",
            before.len(),
            after_bytes.len()
        );
    }
    fn service() -> ContentService {
        let repository =
            Arc::new(SettingsRepository::open(std::path::Path::new(":memory:")).unwrap());
        seed(&repository);
        ContentService::new(repository)
    }
    fn seed(repository: &SettingsRepository) {
        repository.with_database(|db| {
            for (index,title) in ["Nowy program badań naukowych", "Rozpoczęto projekt obserwacji kosmosu"].iter().enumerate() {
                let item=NewsItem { id:String::new(),provider:"fixture".into(),title:(*title).into(),url:format!("https://example.org/{index}"),source:Some("Testowy wydawca".into()),source_url:Some("https://example.org".into()),published_at:None,raw_publication_date:None,description_html:None,guid:Some(index.to_string()),feed_url:"https://example.org/rss".into(),fetched_at:Utc::now().to_rfc3339(),topics:vec!["Nauka".into()] };
                db.execute("INSERT INTO news_articles(provider,guid,canonical_url,metadata_json,first_seen_at) VALUES (?1,?2,?3,?4,0)",params![item.provider,item.guid,item.url,serde_json::to_string(&item).unwrap()]).unwrap();
            }
            Ok(())
        }).unwrap();
    }
    struct Fake;
    impl ContentGenerator for Fake {
        fn generate(&self, context: &Context) -> Result<Draft, String> {
            Ok(Draft {
                title: "Krótki przegląd nauki".into(),
                segments: context
                    .sources
                    .iter()
                    .map(|source| Segment {
                        topic: "Nauka".into(),
                        text: format!("Według nagłówka testowego wydawcy: {}.", source.title),
                        evidence: vec![Evidence {
                            source_id: source.id.clone(),
                            quote: source.title.clone(),
                        }],
                    })
                    .collect(),
                limitations: vec!["Dostępne są tylko nagłówki.".into()],
                insufficient_material: true,
            })
        }
    }
    fn ids() -> Vec<String> {
        vec!["1".into(), "2".into()]
    }
    fn count(service: &ContentService) -> i64 {
        service
            .repository
            .with_database(|db| {
                db.query_row("SELECT COUNT(*) FROM content_broadcasts", [], |r| r.get(0))
                    .map_err(|e| e.to_string())
            })
            .unwrap()
    }
    #[test]
    fn persists_script_sources_and_excludes_reused_material() {
        let service = service();
        let result = service.generate(ids(), &Fake).unwrap();
        assert_eq!(result.sources.len(), 2);
        assert_eq!(count(&service), 1);
        assert!(result.draft.insufficient_material);
        assert!(result.estimated_duration_ms < 120000);
        assert!(service.context(&ids()).is_err());
        service
            .repository
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM content_sources", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    2
                );
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM news_usage", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                ); // generated is not aired
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn validates_evidence_and_never_persists_bad_results() {
        let service = service();
        let context = service.context(&ids()).unwrap();
        let valid = Fake.generate(&context).unwrap();
        let mut invalid = valid.clone();
        invalid.segments[0].evidence[0].source_id = "unknown".into();
        assert!(validate(&invalid, &context).is_err());
        invalid = valid.clone();
        invalid.segments[0].evidence[0].quote = "Niepotwierdzony szczegół artykułu".into();
        assert!(validate(&invalid, &context).is_err());
        invalid = valid;
        invalid.segments.clear();
        assert!(validate(&invalid, &context).is_err());
        assert_eq!(count(&service), 0);
        assert_eq!(
            service
                .context(&["1".into(), "01".into()])
                .unwrap()
                .sources
                .len(),
            1
        );
    }
    #[test]
    fn no_history_write_on_provider_error_or_insufficient_news() {
        struct Failure;
        impl ContentGenerator for Failure {
            fn generate(&self, _: &Context) -> Result<Draft, String> {
                Err("HTTP timeout".into())
            }
        }
        let service = service();
        assert!(service.generate(ids(), &Failure).is_err());
        assert_eq!(count(&service), 0);
        assert_eq!(service.context(&["1".into()]).unwrap().sources.len(), 1);
    }
    #[test]
    fn rejects_incomplete_refused_and_invalid_responses() {
        assert!(parse_response(&json!({"status":"incomplete","output":[]})).is_err());
        assert!(parse_response(&json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"refusal","refusal":"no"}]}]})).is_err());
        assert!(parse_response(&json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"invalid JSON"}]}]})).is_err());
        let service = service();
        let draft = Fake.generate(&service.context(&ids()).unwrap()).unwrap();
        let value = json!({"status":"completed","output":[{"type":"reasoning"},{"type":"message","role":"assistant","content":[{"type":"output_text","text":serde_json::to_string(&draft).unwrap()}]}]});
        assert_eq!(parse_response(&value).unwrap().segments.len(), 2);
    }
    #[test]
    fn request_uses_responses_schema_and_configured_model_without_secrets() {
        let service = service();
        let mut context = service.context(&ids()).unwrap();
        context.settings.text_model = "configured-model".into();
        let request = request_body(&context);
        assert_eq!(request["model"], "configured-model");
        assert_eq!(request["store"], false);
        assert_eq!(request["text"]["format"]["type"], "json_schema");
        assert_eq!(request["text"]["format"]["strict"], true);
        assert_eq!(
            request["text"]["format"]["schema"]["properties"]["segments"]["items"]["properties"]["evidence"]
                ["items"]["properties"]["source_id"]["enum"],
            json!(["1", "2"])
        );
        assert!(request.get("api_key").is_none());
        assert!(request.get("tools").is_none());
    }
    #[test]
    fn shared_quote_is_allowed_but_repeated_narration_is_rejected() {
        let service = service();
        let mut context = service.context(&ids()).unwrap();
        let draft = Fake.generate(&context).unwrap();
        context
            .previous_quotes
            .insert(normalized(&draft.segments[0].evidence[0].quote));
        assert!(validate(&draft, &context).is_ok());
        context.previous_quotes.clear();
        context.previous.push(draft.segments[0].text.clone());
        assert!(validate(&draft, &context).is_err());
    }
    #[test]
    fn source_save_failure_rolls_back_entire_broadcast() {
        let service = service();
        service.repository.with_database(|db| {
            db.execute_batch("CREATE TRIGGER fail_content_source BEFORE INSERT ON content_sources WHEN NEW.article_id=2 BEGIN SELECT RAISE(ABORT, 'test failure'); END;").map_err(|e| e.to_string())
        }).unwrap();
        assert!(service.generate(ids(), &Fake).is_err());
        assert_eq!(count(&service), 0);
        assert!(service.context(&ids()).is_ok());
    }
    #[test]
    fn repairs_order_and_discards_only_invalid_or_repeated_units() {
        let service = service();
        let mut context = service.context(&ids()).unwrap();
        context.sources[0].published_at = Some("2026-11-15T10:00:00Z".into());
        context.sources[1].published_at = Some("2026-11-16T10:00:00Z".into());
        let mut draft = Fake.generate(&context).unwrap();
        let mut bad = draft.segments[0].clone();
        bad.evidence[0].quote = "Nieznane szczegóły, których nie ma w RSS".into();
        draft.segments.push(bad);
        draft.segments.push(draft.segments[0].clone());
        repair::repair(&mut draft, &context).unwrap();
        assert_eq!(draft.segments.len(), 2);
        assert_eq!(draft.segments[0].evidence[0].source_id, "2");
        assert!(draft.insufficient_material);
        context.previous.push(draft.segments[0].text.clone());
        repair::repair(&mut draft, &context).unwrap();
        assert_eq!(draft.segments.len(), 1);
        assert_eq!(draft.segments[0].evidence[0].source_id, "1");
        context.previous.push(draft.segments[0].text.clone());
        assert!(
            repair::repair(&mut draft, &context)
                .unwrap_err()
                .starts_with("Brak bezpiecznych materiałów:")
        );
    }
    #[test]
    fn corrupt_missing_and_duplicate_sources_do_not_discard_valid_source() {
        let service = service();
        service
            .repository
            .with_database(|db| {
                db.execute(
                    "UPDATE news_articles SET metadata_json='broken' WHERE id=2",
                    [],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        let result = service
            .generate(
                vec![
                    "2".into(),
                    "missing".into(),
                    "999".into(),
                    "1".into(),
                    "01".into(),
                ],
                &Fake,
            )
            .unwrap();
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.draft.segments.len(), 1);
        assert!(result.estimated_duration_ms < result.target_min_duration_ms);
        assert_eq!(count(&service), 1);
    }
    #[test]
    fn malformed_segment_does_not_destroy_parseable_sibling() {
        let service = service();
        let context = service.context(&ids()).unwrap();
        let mut draft = serde_json::to_value(Fake.generate(&context).unwrap()).unwrap();
        draft["segments"][0]["evidence"] = "invalid".into();
        let value = json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":draft.to_string()}]}]});
        let mut parsed = parse_response(&value).unwrap();
        repair::repair(&mut parsed, &context).unwrap();
        assert_eq!(parsed.segments.len(), 1);
    }
    #[test]
    fn incomplete_envelope_can_salvage_complete_validated_json() {
        let service = service();
        let context = service.context(&ids()).unwrap();
        let draft = Fake.generate(&context).unwrap();
        let value = json!({"status":"incomplete","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":serde_json::to_string(&draft).unwrap()}]}]});
        let mut recovered = parse_response(&value).unwrap();
        repair::repair(&mut recovered, &context).unwrap();
        assert_eq!(recovered.segments.len(), 2);
        assert!(recovered.insufficient_material);
    }
    #[test]
    fn unavailable_during_generation_removes_only_affected_story() {
        struct Race(Arc<SettingsRepository>);
        impl ContentGenerator for Race {
            fn generate(&self, context: &Context) -> Result<Draft, String> {
                self.0
                    .with_database(|db| {
                        db.execute("INSERT INTO news_usage VALUES (1,'other',0)", [])
                            .unwrap();
                        Ok(())
                    })
                    .unwrap();
                Fake.generate(context)
            }
        }
        let service = service();
        let result = service
            .generate(ids(), &Race(service.repository.clone()))
            .unwrap();
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.sources[0].id, "2");
        assert!(!result.text.contains("Nowy program badań naukowych"));
    }
    #[test]
    fn all_unusable_sources_prevent_model_call_and_saved_script_can_resume() {
        struct Never;
        impl ContentGenerator for Never {
            fn generate(&self, _: &Context) -> Result<Draft, String> {
                panic!("must not call API without materials")
            }
        }
        let service = service();
        assert!(service.generate(vec!["999".into()], &Never).is_err());
        let broadcast = service.generate(vec!["1".into()], &Fake).unwrap();
        let restored = load_saved(
            &service.repository,
            broadcast.id.parse().unwrap(),
            &service.repository.load().unwrap(),
        )
        .unwrap();
        assert_eq!(restored.text, broadcast.text);
    }
    #[test]
    fn new_numbers_are_not_blindly_classified_as_repeated_update() {
        let service = service();
        let mut context = service.context(&ids()).unwrap();
        context.sources[0].description_html =
            Some("W regatach na jeziorze wystartowały 42 załogi z całej Polski.".into());
        context
            .previous
            .push("W regatach na jeziorze wystartowały 40 załogi z całej Polski.".into());
        let mut draft = Fake.generate(&context).unwrap();
        draft.segments[0].text = context.sources[0].description_html.clone().unwrap();
        draft.segments[0].evidence[0].quote = draft.segments[0].text.clone();
        repair::repair(&mut draft, &context).unwrap();
        assert_eq!(draft.segments.len(), 2);
    }
}
