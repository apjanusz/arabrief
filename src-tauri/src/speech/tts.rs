use super::{format, segment_cache};
use crate::settings::SettingsRepository;
use std::{io::Read, sync::Arc, time::Duration};
use tokio::sync::watch;
fn request(text: &str, voice: &str) -> serde_json::Value {
    serde_json::json!({"model":format::MODEL,"voice":voice,"input":text,"instructions":format::INSTRUCTIONS,"response_format":"pcm"})
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
        let work = tauri::async_runtime::spawn_blocking(move || {
            if let Some(cached) = segment_cache::load(&repo, &text, &voice)? {
                return Ok(cached);
            }
            let key = crate::credentials::read_openai_key()?;
            let bearer = zeroize::Zeroizing::new(format!("Bearer {}", key.as_str()));
            let mut auth = reqwest::header::HeaderValue::from_str(&bearer)
                .map_err(|_| "Nieprawidłowy klucz OpenAI")?;
            auth.set_sensitive(true);
            let body = request(&text, &voice);
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
            segment_cache::save(&repo, &text, &voice, &result)?;
            Ok::<_, String>(result)
        });
        let (audio, _) = tokio::select! {_ = cancel.changed()=>return Err("Anulowano generowanie audio".into()),result=work=>result.map_err(|_|"Błąd zadania TTS")??};
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
    fn speech_request_preserves_input_and_supplies_requested_model_voice_and_instructions() {
        for voice in ["marin", "cedar"] {
            let body = super::request("Łódź: dwadzieścia stopni.", voice);
            assert_eq!(body["model"], "gpt-4o-mini-tts");
            assert_eq!(body["voice"], voice);
            assert_eq!(body["input"], "Łódź: dwadzieścia stopni.");
            assert_eq!(body["instructions"], super::format::INSTRUCTIONS);
            assert_eq!(body["response_format"], "pcm");
        }
    }
}
