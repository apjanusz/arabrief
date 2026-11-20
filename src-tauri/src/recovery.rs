//! Durable recovery of unused work. A playback attempt is consumed conservatively:
//! after a crash we cannot know how much audio the user actually heard.
use crate::{
    content,
    settings::{AppSettings, SettingsRepository},
    speech::{SpeechAsset, SpeechService},
};
use rusqlite::{OptionalExtension, params};
use std::{path::Path, sync::Arc};
use tauri::Manager;

pub(crate) fn settings_key(settings: &AppSettings) -> String {
    let mut topics = settings.topics.clone();
    topics.sort();
    serde_json::json!([
        "article-chapters-v6-archive-context",
        topics,
        settings.text_model,
        settings.open_ai_voice,
        settings.broadcast_min_duration_ms,
        settings.broadcast_max_duration_ms,
        settings.broadcast_language(),
        settings.effective_region(),
        settings.news_source_urls
    ])
    .to_string()
}
#[derive(serde::Serialize)]
pub struct Recovery {
    script: content::Broadcast,
    audio: Option<SpeechAsset>,
}

pub(crate) fn restore(
    repository: &SettingsRepository,
    speech: &SpeechService,
    root: &Path,
) -> Result<Option<Recovery>, String> {
    let settings = repository.load()?;
    let ids=repository.with_database(|db| {
        let mut statement=db.prepare("SELECT b.id FROM content_broadcasts b JOIN pipeline_preparations p ON p.broadcast_id=b.id WHERE p.settings_key=?1 AND datetime(b.created_at)>datetime('now','-6 hours') AND NOT EXISTS(SELECT 1 FROM playback_receipts r WHERE r.broadcast_id=b.id) AND NOT EXISTS(SELECT 1 FROM content_sources c JOIN news_usage u ON u.article_id=c.article_id WHERE c.broadcast_id=b.id) ORDER BY b.id DESC LIMIT 20").map_err(|e|e.to_string())?;
        statement.query_map([settings_key(&settings)],|r|r.get::<_,i64>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
    })?;
    let mut pending = None;
    for id in ids {
        let script = match content::load_saved(repository, id, &settings) {
            Ok(script) => script,
            Err(_) => {
                crate::performance::diagnostic!(
                    "[recovery] stage=cache id={id} action=skipped reason=invalid_saved_script"
                );
                continue;
            }
        };
        match speech.restore_audio(root, &id.to_string(), settings.open_ai_voice.clone()) {
            Ok(Some(audio)) => {
                crate::performance::diagnostic!(
                    "[recovery] stage=cache id={id} action=reused_ready"
                );
                return Ok(Some(Recovery {
                    script,
                    audio: Some(audio),
                }));
            }
            Ok(None) => {
                if pending.is_none() {
                    pending = Some(Recovery {
                        script,
                        audio: None,
                    });
                }
            }
            Err(_) => {
                crate::performance::diagnostic!(
                    "[recovery] stage=cache id={id} action=kept_script reason=invalid_audio_cache"
                );
                if pending.is_none() {
                    pending = Some(Recovery {
                        script,
                        audio: None,
                    });
                }
            }
        }
    }
    if pending.is_some() {
        crate::performance::diagnostic!("[recovery] stage=cache action=resumed_script");
    }
    Ok(pending)
}
fn claim(repository: &SettingsRepository, id: &str) -> Result<(), String> {
    let id: i64 = id
        .parse()
        .map_err(|_| "Nieprawidłowy identyfikator audycji")?;
    let settings = repository.load()?;
    repository.with_database(|db| {
        let compatible:bool=db.query_row("SELECT language=?2 AND news_region=?3 FROM content_broadcasts WHERE id=?1",params![id,settings.broadcast_language(),settings.effective_region()],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !compatible {return Err("Broadcast language or news region changed. Prepare a new broadcast.".into());}
        let tx=db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM content_broadcasts WHERE id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
        let consumed:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM playback_receipts WHERE broadcast_id=?1 UNION SELECT 1 FROM content_sources c JOIN news_usage u ON u.article_id=c.article_id WHERE c.broadcast_id=?1)",[id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !exists||consumed{return Err("Audycja została już odtworzona lub rozpoczęta. Przygotuj nową audycję.".into());}
        tx.execute("INSERT INTO playback_receipts VALUES (?1,'attempted',?2)",params![id,chrono::Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())
    })
}
#[tauri::command]
pub async fn broadcast_restore(
    app: tauri::AppHandle,
    repository: tauri::State<'_, Arc<SettingsRepository>>,
    speech: tauri::State<'_, Arc<SpeechService>>,
) -> Result<Option<Recovery>, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "Brak katalogu danych")?
        .join("audio");
    let repository = repository.inner().clone();
    let speech = speech.inner().clone();
    tauri::async_runtime::spawn_blocking(move || restore(&repository, &speech, &root))
        .await
        .map_err(|_| "Błąd odzyskiwania audycji")?
}
#[tauri::command]
pub async fn broadcast_claim(
    broadcast_id: String,
    repository: tauri::State<'_, Arc<SettingsRepository>>,
) -> Result<(), String> {
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || claim(&repository, &broadcast_id))
        .await
        .map_err(|_| "Błąd rezerwacji audycji")?
}
#[tauri::command]
pub async fn broadcast_complete(
    broadcast_id: String,
    repository: tauri::State<'_, Arc<SettingsRepository>>,
) -> Result<(), String> {
    let id: i64 = broadcast_id
        .parse()
        .map_err(|_| "Nieprawidłowy identyfikator audycji")?;
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move||repository.with_database(|db| {
        let state:Option<String>=db.query_row("SELECT state FROM playback_receipts WHERE broadcast_id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        if state.is_none(){return Err("Brak potwierdzenia rozpoczęcia audycji".into());}
        db.execute("UPDATE playback_receipts SET state='completed',updated_at=?2 WHERE broadcast_id=?1",params![id,chrono::Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;Ok(())
    })).await.map_err(|_|"Błąd zapisu odsłuchu")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_language_or_region_rejects_playback_before_consuming_the_recording() {
        let repository = SettingsRepository::open(Path::new(":memory:")).unwrap();
        let mut settings = repository.load().unwrap();
        settings.ui_language = "pl".into();
        settings.news_region = "PL".into();
        repository.save(&settings).unwrap();
        seed(&repository, 1);
        settings.ui_language = "en".into();
        repository.save(&settings).unwrap();
        assert!(claim(&repository, "1").is_err());
        settings.ui_language = "pl".into();
        settings.news_region = "US".into();
        repository.save(&settings).unwrap();
        assert!(claim(&repository, "1").is_err());
        settings.news_region = "PL".into();
        repository.save(&settings).unwrap();
        claim(&repository, "1").unwrap();
    }
    const TEXT: &str = "Według wydawcy rozpoczęto program badań naukowych.";
    fn seed(repository: &SettingsRepository, id: i64) {
        let settings = repository.load().unwrap();
        let source = serde_json::json!({"id":id.to_string(),"provider":"fixture","title":TEXT,"url":format!("https://example.org/{id}"),"source":"Fixture","sourceUrl":null,"publishedAt":null,"rawPublicationDate":null,"descriptionHtml":null,"guid":null,"feedUrl":"https://example.org/rss","fetchedAt":chrono::Utc::now().to_rfc3339(),"topics":["Nauka"]});
        let draft = serde_json::json!({"title":"Nauka","segments":[{"topic":"Nauka","text":TEXT,"evidence":[{"source_id":id.to_string(),"quote":TEXT}]}],"limitations":[],"insufficient_material":true});
        repository.with_database(|db|{
            db.execute("INSERT INTO content_broadcasts(id,created_at,title,model,text,estimated_duration_ms,topics_json,draft_json) VALUES (?1,?2,'Nauka','fixture',?3,1000,'[\"Nauka\"]',?4)",params![id,chrono::Utc::now().to_rfc3339(),TEXT,draft.to_string()]).unwrap();
            db.execute("INSERT INTO news_articles(id,provider,canonical_url,metadata_json,first_seen_at) VALUES (?1,'fixture',?2,?3,0)",params![id,format!("https://example.org/{id}"),source.to_string()]).unwrap();
            db.execute("INSERT INTO content_sources VALUES (?1,?1,?2)",params![id,source.to_string()]).unwrap();
            db.execute("INSERT INTO pipeline_preparations VALUES (?1,?2)",params![id,settings_key(&settings)]).unwrap();Ok(())
        }).unwrap();
    }
    #[test]
    fn corrupt_newer_script_keeps_previous_unused_work_and_attempt_is_not_a_completed_play() {
        let repository = Arc::new(SettingsRepository::open(Path::new(":memory:")).unwrap());
        seed(&repository, 1);
        seed(&repository, 2);
        repository
            .with_database(|db| {
                db.execute(
                    "UPDATE content_broadcasts SET draft_json='broken' WHERE id=2",
                    [],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        let speech = SpeechService::new(repository.clone());
        let recovered = restore(&repository, &speech, Path::new("missing-test-audio"))
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::to_value(recovered.script).unwrap()["id"], "1");
        assert!(recovered.audio.is_none());
        claim(&repository, "1").unwrap();
        assert!(claim(&repository, "1").is_err());
        assert!(
            restore(&repository, &speech, Path::new("missing-test-audio"))
                .unwrap()
                .is_none()
        );
        repository
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM news_usage", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
                assert_eq!(
                    db.query_row("SELECT state FROM playback_receipts", [], |r| r
                        .get::<_, String>(0))
                        .unwrap(),
                    "attempted"
                );
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM content_broadcasts", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    2
                );
                Ok(())
            })
            .unwrap();
    }
    #[test]
    fn only_one_concurrent_claim_succeeds() {
        let repository = Arc::new(SettingsRepository::open(Path::new(":memory:")).unwrap());
        seed(&repository, 1);
        let results = std::thread::scope(|scope| {
            let a = scope.spawn(|| claim(&repository, "1"));
            let b = scope.spawn(|| claim(&repository, "1"));
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    }
    #[test]
    fn changed_preferences_and_expired_or_aired_sources_exclude_fallback() {
        let repository = Arc::new(SettingsRepository::open(Path::new(":memory:")).unwrap());
        seed(&repository, 1);
        let speech = SpeechService::new(repository.clone());
        let root = Path::new("missing-test-audio");
        let initial = repository.load().unwrap();
        let mut changed = initial.clone();
        changed.topics = vec!["Sport".into()];
        repository.save(&changed).unwrap();
        assert!(restore(&repository, &speech, root).unwrap().is_none());
        repository.save(&initial).unwrap();
        assert!(restore(&repository, &speech, root).unwrap().is_some());
        repository
            .with_database(|db| {
                db.execute("UPDATE content_broadcasts SET created_at='2020-01-01'", [])
                    .unwrap();
                Ok(())
            })
            .unwrap();
        assert!(restore(&repository, &speech, root).unwrap().is_none());
        seed(&repository, 2);
        repository
            .with_database(|db| {
                db.execute("INSERT INTO news_usage VALUES (2,'played',0)", [])
                    .unwrap();
                Ok(())
            })
            .unwrap();
        assert!(restore(&repository, &speech, root).unwrap().is_none());
    }
    #[test]
    fn playback_receipt_survives_reopening_database() {
        let path = std::env::temp_dir().join(format!(
            "arabrief-receipts-{}.sqlite",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        {
            let repository = SettingsRepository::open(&path).unwrap();
            seed(&repository, 1);
            claim(&repository, "1").unwrap();
        }
        {
            let repository = SettingsRepository::open(&path).unwrap();
            assert!(claim(&repository, "1").is_err());
        }
        std::fs::remove_file(path).unwrap();
    }
}
