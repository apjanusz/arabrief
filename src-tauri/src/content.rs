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
        let request = request_body(context);
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
        let response = client
            .post("https://api.openai.com/v1/responses")
            .header(AUTHORIZATION, auth)
            .header(CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&request).map_err(|_| "Błąd przygotowania materiałów")?)
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
        parse_response(
            &serde_json::from_slice::<Value>(&bytes)
                .map_err(|_| "Nieprawidłowa odpowiedź OpenAI")?,
        )
    }
}
fn request_body(context: &Context) -> Value {
    let ids: Vec<_> = context.sources.iter().map(|s| &s.id).collect();
    json!({"model":context.settings.text_model,"store":false,"max_output_tokens":6000,
        "instructions":include_str!("arabrief_prompt.txt"),
        "input":[{"role":"user","content":[{"type":"input_text","text":json!({
            "target_min_words":context.settings.broadcast_min_duration_ms*140/60000,
            "target_max_words":context.settings.broadcast_max_duration_ms*140/60000,
            "sources":context.sources,"previous_broadcasts":context.previous
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
fn parse_response(value: &Value) -> Result<Draft, String> {
    if value["status"] != "completed" {
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
    serde_json::from_str(&output).map_err(|_| "Scenariusz nie spełnia kontraktu JSON".into())
}
fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn validate(draft: &Draft, context: &Context) -> Result<(), String> {
    if draft.title.trim().is_empty()
        || draft.title.len() > 300
        || draft.segments.is_empty()
        || draft.segments.len() > 12
        || draft.limitations.len() > 20
    {
        return Err("Brak wystarczających nowych materiałów lub nieprawidłowy scenariusz".into());
    }
    let mut referenced = HashSet::new();
    let mut total = 0;
    for segment in &draft.segments {
        total += segment.text.len();
        if segment.topic.trim().is_empty()
            || segment.topic.len() > 200
            || segment.text.trim().is_empty()
            || segment.evidence.is_empty()
            || segment.evidence.len() > 10
        {
            return Err("Nieprawidłowy segment scenariusza".into());
        }
        if context
            .previous
            .iter()
            .any(|previous| normalized(previous).contains(&normalized(&segment.text)))
        {
            return Err("Scenariusz powtarza poprzednią audycję".into());
        }
        for evidence in &segment.evidence {
            let source = context
                .sources
                .iter()
                .find(|s| s.id == evidence.source_id)
                .ok_or("Scenariusz wskazuje nieznane źródło")?;
            if evidence.quote.chars().count() < 12
                || evidence.quote.len() > 1200
                || !(source.title.contains(&evidence.quote)
                    || source
                        .description_html
                        .as_ref()
                        .is_some_and(|s| s.contains(&evidence.quote)))
            {
                return Err("Brak dosłownego potwierdzenia w przekazanych materiałach".into());
            }
            if context
                .previous_quotes
                .contains(&normalized(&evidence.quote))
            {
                return Err("Scenariusz powtarza informację z poprzedniego scenariusza".into());
            }
            referenced.insert(&source.id);
        }
    }
    if total > 30000 || referenced.len() < 2 || draft.limitations.iter().any(|s| s.len() > 2000) {
        return Err("Scenariusz wymaga co najmniej dwóch źródeł i poprawnej długości".into());
    }
    Ok(())
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
        if !(2..=8).contains(&ids.len()) || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
            return Err("Wybierz od 2 do 8 różnych wiadomości z RSS".into());
        }
        let settings = self.repository.load()?;
        self.repository.with_database(|db| {
            let mut sources=Vec::new();
            let mut seen=HashSet::new();
            for id in ids {
                let id: i64=id.parse().map_err(|_| "Nieprawidłowy identyfikator źródła")?;
                if !seen.insert(id) { return Err("Powtórzony identyfikator źródła".into()); }
                let raw: Option<String>=db.query_row("SELECT metadata_json FROM news_articles a WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM news_usage u WHERE u.article_id=a.id) AND NOT EXISTS(SELECT 1 FROM content_sources c WHERE c.article_id=a.id)",[id],|r| r.get(0)).optional().map_err(|e| e.to_string())?;
                let mut source: NewsItem=serde_json::from_str(&raw.ok_or("Źródło zostało już użyte lub nie istnieje. Pobierz i wybierz inne wiadomości.")?).map_err(|_| "Uszkodzone metadane źródła")?;
                source.id=id.to_string();
                // Bound material size before sending to API; stored source snapshot retains what was sent.
                if let Some(description)=source.description_html.as_mut() { *description=description.chars().take(4000).collect(); }
                sources.push(source);
            }
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
        let _gate = self
            .gate
            .try_lock()
            .map_err(|_| "Generowanie scenariusza już trwa")?;
        let context = self.context(&ids)?;
        let mut draft = generator.generate(&context)?;
        validate(&draft, &context)?;
        let text = draft
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let estimated_duration_ms = (text.split_whitespace().count() as u32) * 60000 / 140;
        if estimated_duration_ms > context.settings.broadcast_max_duration_ms * 12 / 10 {
            return Err(
                "Scenariusz znacznie przekracza docelową długość; nie został zapisany".into(),
            );
        }
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
            .into_iter()
            .filter(|s| referenced.contains(s.id.as_str()))
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
            model: context.settings.text_model,
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
        self.repository.with_database(|db| {
            let tx=db.transaction().map_err(|e| e.to_string())?;
            // Recheck availability after the network request, before committing a draft.
            for source in &broadcast.sources {
                let taken:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM content_sources WHERE article_id=?1 UNION SELECT 1 FROM news_usage WHERE article_id=?1)",[&source.id],|r| r.get(0)).map_err(|e| e.to_string())?;
                if taken { return Err("Materiały zostały wykorzystane podczas generowania. Scenariusz nie został zapisany.".into()); }
            }
            tx.execute("INSERT INTO content_broadcasts(created_at,title,model,text,estimated_duration_ms,topics_json,draft_json) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![broadcast.created_at,broadcast.title,broadcast.model,broadcast.text,broadcast.estimated_duration_ms,serde_json::to_string(&broadcast.topics).unwrap(),serde_json::to_string(&broadcast.draft).unwrap()]).map_err(|e| e.to_string())?;
            let id=tx.last_insert_rowid(); broadcast.id=id.to_string();
            for source in &broadcast.sources { tx.execute("INSERT INTO content_sources VALUES (?1,?2,?3)",params![id,source.id,serde_json::to_string(source).unwrap()]).map_err(|e| e.to_string())?; }
            for evidence in broadcast.draft.segments.iter().flat_map(|s| &s.evidence) { tx.execute("INSERT OR IGNORE INTO content_evidence VALUES (?1,?2)",params![id,normalized(&evidence.quote)]).map_err(|e| e.to_string())?; }
            tx.commit().map_err(|e| e.to_string())
        })?;
        Ok(broadcast)
    }
}
#[tauri::command]
pub async fn content_generate(
    article_ids: Vec<String>,
    service: tauri::State<'_, Arc<ContentService>>,
) -> Result<Broadcast, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.generate(article_ids, &OpenAiGenerator))
        .await
        .map_err(|_| "Błąd modułu generowania".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(service.context(&["1".into(), "01".into()]).is_err());
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
        assert!(service.context(&["1".into()]).is_err());
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
    fn detects_repeated_evidence_and_text() {
        let service = service();
        let mut context = service.context(&ids()).unwrap();
        let draft = Fake.generate(&context).unwrap();
        context
            .previous_quotes
            .insert(normalized(&draft.segments[0].evidence[0].quote));
        assert!(validate(&draft, &context).is_err());
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
}
