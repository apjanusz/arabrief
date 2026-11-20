use super::{format, segment_cache};
use crate::settings::SettingsRepository;
use std::{io::Read, sync::Arc, time::Duration};
use tokio::sync::watch;
// A cancelled async caller cannot abort a blocking HTTP request. Keep its
// cache check/request/save atomic against a subsequent attempt, even if the
// enclosing chapter timeout drops the caller before this worker completes.
static SEGMENT_WORKER: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn request(text: &str, voice: &str, language: &str) -> serde_json::Value {
    serde_json::json!({"model":format::MODEL,"voice":voice,"input":text,"instructions":format::instructions(language),"response_format":"pcm"})
}
async fn finish_worker(
    mut work: tauri::async_runtime::JoinHandle<Result<(Vec<u8>, String), String>>,
    cancel: &mut watch::Receiver<bool>,
) -> Result<(Vec<u8>, String), String> {
    tokio::select! {
        _ = cancel.changed()=>{
            // A blocking request cannot be aborted. Finish caching before ack.
            let _=work.await;
            Err("Anulowano generowanie audio".into())
        },
        result=&mut work=>result.map_err(|_|"Błąd zadania TTS")?
    }
}
pub(super) async fn synthesize(
    parts: &[String],
    voice: &str,
    cancel: &mut watch::Receiver<bool>,
    repository: Arc<SettingsRepository>,
) -> Result<(Vec<u8>, String), String> {
    let mut pcm = Vec::new();
    for part in parts {
        super::check_cancel(cancel)?;
        let repo = repository.clone();
        let text = part.clone();
        let voice = voice.to_owned();
        let worker_cancel = cancel.clone();
        let work = tauri::async_runtime::spawn_blocking(move || {
            let _guard = SEGMENT_WORKER
                .lock()
                .map_err(|_| "Zadanie TTS jest niedostępne")?;
            super::check_cancel(&worker_cancel)?;
            let settings = repo.load()?;
            if let Some(cached) = segment_cache::load_for_settings(&repo, &text, &voice, &settings)?
            {
                return Ok(cached);
            }
            let key = crate::credentials::read_openai_key()?;
            let bearer = zeroize::Zeroizing::new(format!("Bearer {}", key.as_str()));
            let mut auth = reqwest::header::HeaderValue::from_str(&bearer)
                .map_err(|_| "Nieprawidłowy klucz OpenAI")?;
            auth.set_sensitive(true);
            let body = request(&text, &voice, settings.broadcast_language());
            let response = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(90))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Błąd klienta TTS")?
                .post("https://api.openai.com/v1/audio/speech")
                .header(reqwest::header::AUTHORIZATION, auth)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_string())
                .send()
                .map_err(|_| "Błąd połączenia TTS lub timeout (90 s)")?;
            if !response.status().is_success() {
                return Err(crate::openai_errors::response_error(response).user_message());
            }
            let mut bytes = Vec::new();
            response
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Błąd odczytu TTS")?;
            if bytes.is_empty() || !bytes.len().is_multiple_of(2) || bytes.len() > 8 * 1024 * 1024 {
                return Err("Nieprawidłowe PCM TTS".into());
            }
            let result = (bytes, text.clone());
            if crate::recovery::settings_key(&settings)
                != crate::recovery::settings_key(&repo.load()?)
            {
                return Err(
                    "Broadcast language or news region changed. Prepare a new broadcast.".into(),
                );
            }
            segment_cache::save_for_settings(&repo, &text, &voice, &result, &settings)?;
            Ok::<_, String>(result)
        });
        let (audio, _) = finish_worker(work, cancel).await?;
        super::check_cancel(cancel)?;
        if pcm.len() + audio.len() > format::MAX_PCM {
            return Err("Przekroczono limit nagrania".into());
        }
        pcm.extend(audio);
    }
    Ok((pcm, parts.join("\n\n")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn english_speech_request_keeps_voice_and_cost_limits_without_polish_instruction() {
        for voice in ["marin", "cedar"] {
            let body = super::request("Today's news from Poland.", voice, "en");
            assert_eq!(body["voice"], voice);
            assert_eq!(body["model"], "gpt-4o-mini-tts");
            assert_eq!(body["input"], "Today's news from Poland.");
            assert_eq!(body["response_format"], "pcm");
            assert!(body["instructions"].as_str().unwrap().contains("English"));
        }
    }
    #[tokio::test]
    async fn cancellation_waits_for_inflight_worker_to_save_its_result() {
        let (release, wait) = std::sync::mpsc::channel();
        let saved = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_saved = saved.clone();
        let work = tauri::async_runtime::spawn_blocking(move || {
            wait.recv().unwrap();
            worker_saved.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok((vec![0, 0], "saved".into()))
        });
        let (sender, mut cancel) = tokio::sync::watch::channel(false);
        sender.send(true).unwrap();
        let finish = super::finish_worker(work, &mut cancel);
        tokio::pin!(finish);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut finish)
                .await
                .is_err()
        );
        assert!(!saved.load(std::sync::atomic::Ordering::SeqCst));
        release.send(()).unwrap();
        assert!(finish.await.unwrap_err().contains("Anulowano"));
        assert!(saved.load(std::sync::atomic::Ordering::SeqCst));
    }
    #[test]
    fn speech_request_preserves_input_and_supplies_requested_model_voice_and_instructions() {
        for voice in ["marin", "cedar"] {
            let body = super::request("Łódź: dwadzieścia stopni.", voice, "pl");
            assert_eq!(body["model"], "gpt-4o-mini-tts");
            assert_eq!(body["voice"], voice);
            assert_eq!(body["input"], "Łódź: dwadzieścia stopni.");
            assert_eq!(body["instructions"], super::format::INSTRUCTIONS);
            assert_eq!(body["response_format"], "pcm");
        }
    }
}
