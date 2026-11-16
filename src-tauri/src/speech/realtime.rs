use super::format::{self, INSTRUCTIONS, MAX_PCM, MODEL, RATE};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::{
    net::TcpStream,
    sync::watch,
    time::{Instant, timeout, timeout_at},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};
use zeroize::Zeroizing;
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub fn session(voice: &str) -> Value {
    json!({"type":"session.update","session":{"type":"realtime","instructions":INSTRUCTIONS,"output_modalities":["audio"],"tools":[],"tool_choice":"none",
        "audio":{"input":{"turn_detection":null},"output":{"format":{"type":"audio/pcm","rate":RATE},"voice":voice}},"max_output_tokens":4096}})
}
pub fn response(text: &str) -> Value {
    json!({"type":"response.create","response":{"conversation":"none","output_modalities":["audio"],"tools":[],"tool_choice":"none","instructions":INSTRUCTIONS,"max_output_tokens":4096,
        "input":[{"type":"message","role":"user","content":[{"type":"input_text","text":json!({"script":text}).to_string()}]}]}})
}
async fn send(socket: &mut Socket, value: Value) -> Result<(), String> {
    timeout(
        Duration::from_secs(10),
        socket.send(Message::Text(value.to_string().into())),
    )
    .await
    .map_err(|_| "Timeout zapisu Realtime")?
    .map_err(|_| "Błąd zapisu Realtime".into())
}
async fn event(
    socket: &mut Socket,
    cancel: &mut watch::Receiver<bool>,
    deadline: Instant,
) -> Result<Value, String> {
    loop {
        if *cancel.borrow() {
            return Err("Anulowano generowanie audio".into());
        }
        let message = tokio::select! {
            _ = cancel.changed() => return Err("Anulowano generowanie audio".into()),
            message=timeout_at(deadline,socket.next()) => message.map_err(|_|"Timeout odpowiedzi Realtime")?
        };
        match message {
            Some(Ok(Message::Text(text))) => {
                let value: Value =
                    serde_json::from_str(&text).map_err(|_| "Nieprawidłowe zdarzenie Realtime")?;
                if value["type"] == "error" {
                    return Err(
                        crate::openai_errors::classify(None, &value, None, None).user_message()
                    );
                }
                return Ok(value);
            }
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Ok(Message::Close(_))) | None => {
                return Err("Połączenie Realtime zamknięte przed końcem generowania".into());
            }
            Some(Ok(_)) => return Err("Nieoczekiwana ramka Realtime".into()),
            Some(Err(_)) => return Err("Błąd połączenia Realtime".into()),
        }
    }
}

#[derive(Default)]
pub(super) struct Collector {
    id: Option<String>,
    item: Option<(String, u64, u64)>,
    seen: HashSet<String>,
    pub pcm: Vec<u8>,
    transcript: Option<String>,
    audio_done: bool,
}
impl Collector {
    pub fn accept(&mut self, value: &Value) -> Result<bool, String> {
        if let Some(id) = value["event_id"].as_str() {
            if !self.seen.insert(id.into()) {
                return Ok(false);
            }
            if self.seen.len() > 20000 {
                return Err("Zbyt wiele zdarzeń Realtime".into());
            }
        }
        let kind = value["type"].as_str().unwrap_or("");
        if kind == "response.created" {
            if self.id.is_some() {
                return Err("Niespodziewana dodatkowa odpowiedź Realtime".into());
            }
            self.id = Some(
                value["response"]["id"]
                    .as_str()
                    .ok_or("Brak identyfikatora odpowiedzi")?
                    .into(),
            );
            return Ok(false);
        }
        if kind.starts_with("response.output_audio") {
            if self.id.as_deref() != value["response_id"].as_str() || self.id.is_none() {
                return Err("Fragment audio nie należy do bieżącej odpowiedzi".into());
            }
            let item = (
                value["item_id"]
                    .as_str()
                    .ok_or("Brak identyfikatora audio")?
                    .into(),
                value["output_index"].as_u64().ok_or("Brak indeksu audio")?,
                value["content_index"]
                    .as_u64()
                    .ok_or("Brak indeksu treści")?,
            );
            if self.item.as_ref().is_some_and(|current| current != &item) {
                return Err("Niespodziewana kolejność fragmentów audio".into());
            }
            self.item = Some(item);
            match kind {
                "response.output_audio.delta" => {
                    if self.audio_done {
                        return Err("Audio otrzymane po zakończeniu segmentu".into());
                    }
                    let bytes = STANDARD
                        .decode(value["delta"].as_str().ok_or("Brak PCM")?)
                        .map_err(|_| "Nieprawidłowy base64 PCM")?;
                    if self.pcm.len() + bytes.len() > 8 * 1024 * 1024 {
                        return Err("Segment audio przekracza limit".into());
                    }
                    self.pcm.extend(bytes);
                }
                "response.output_audio.done" => self.audio_done = true,
                "response.output_audio_transcript.done" => {
                    let text = value["transcript"]
                        .as_str()
                        .ok_or("Brak transkrypcji audio")?;
                    if text.len() > 16000 {
                        return Err("Transkrypcja przekracza limit".into());
                    }
                    self.transcript = Some(text.into());
                }
                _ => {}
            }
        }
        if kind == "response.done" {
            if self.id.as_deref() != value["response"]["id"].as_str()
                || self.id.is_none()
                || value["response"]["status"] != "completed"
            {
                return Err("Realtime nie ukończyło segmentu. Nie ponowiono generowania.".into());
            }
            if !self.audio_done
                || self.pcm.is_empty()
                || !self.pcm.len().is_multiple_of(2)
                || self.transcript.is_none()
            {
                return Err("Niepełne audio lub transkrypcja Realtime".into());
            }
            return Ok(true);
        }
        Ok(false)
    }
    fn finish(self, script: &str) -> Result<(Vec<u8>, String), String> {
        let transcript = self.transcript.ok_or("Brak transkrypcji Realtime")?;
        format::verify(script, &transcript)?;
        Ok((self.pcm, transcript))
    }
}
pub async fn synthesize(
    parts: &[String],
    voice: &str,
    cancel: &mut watch::Receiver<bool>,
    repository: Arc<crate::settings::SettingsRepository>,
) -> Result<(Vec<u8>, String), String> {
    super::check_cancel(cancel)?;
    let repo = repository.clone();
    let texts = parts.to_vec();
    let selected_voice = voice.to_string();
    let cached = tauri::async_runtime::spawn_blocking(move || {
        texts
            .iter()
            .map(|text| super::segment_cache::load(&repo, text, &selected_voice))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|_| "Błąd cache segmentów")??;
    if cached.iter().all(Option::is_some) {
        let mut pcm = Vec::new();
        let mut transcripts = Vec::new();
        for (audio, transcript) in cached.into_iter().flatten() {
            if pcm.len() + audio.len() > MAX_PCM {
                return Err("Nagranie przekracza 15 minut".into());
            }
            pcm.extend(audio);
            transcripts.push(transcript);
        }
        crate::performance::diagnostic!(
            "[recovery] stage=speech action=reused_all_segments count={}",
            parts.len()
        );
        return Ok((pcm, transcripts.join("\n\n")));
    }
    let mut connection_time = crate::performance::Span::new("realtime.connect_credentials_tls");
    let key = tauri::async_runtime::spawn_blocking(crate::credentials::read_openai_key)
        .await
        .map_err(|_| "Błąd magazynu klucza")??;
    if *cancel.borrow() {
        return Err("Anulowano generowanie audio".into());
    }
    let mut request = format!("wss://api.openai.com/v1/realtime?model={MODEL}")
        .into_client_request()
        .map_err(|_| "Nieprawidłowy adres Realtime")?;
    let bearer = Zeroizing::new(format!("Bearer {}", key.as_str()));
    let mut auth = tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&bearer)
        .map_err(|_| "Nieprawidłowy klucz OpenAI")?;
    auth.set_sensitive(true);
    request.headers_mut().insert("Authorization", auth);
    let config = WebSocketConfig::default()
        .max_message_size(Some(2 * 1024 * 1024))
        .max_frame_size(Some(2 * 1024 * 1024));
    let connection = tokio::select! {
        _=cancel.changed()=>return Err("Anulowano generowanie audio".into()),
        connection=timeout(Duration::from_secs(15),tokio_tungstenite::connect_async_with_config(request,Some(config),false))=>connection.map_err(|_|"Timeout połączenia Realtime")?
    };
    let (mut socket,_)=connection.map_err(|error| {
        if let tokio_tungstenite::tungstenite::Error::Http(response)=error {
            let body=response.body().as_ref().filter(|b|b.len()<=64*1024).and_then(|b|serde_json::from_slice(b).ok()).unwrap_or(Value::Null);
            crate::openai_errors::classify(Some(response.status().as_u16()),&body,response.headers().get("x-request-id").and_then(|v|v.to_str().ok()),response.headers().get("retry-after").and_then(|v|v.to_str().ok())).user_message()
        } else { "Nie można połączyć z Realtime: błąd sieci/TLS lub protokołu WebSocket. Nie potwierdzono problemu z kluczem.".into() }
    })?;
    connection_time.success();
    drop(connection_time);
    let result = async {
        let mut session_time = crate::performance::Span::new("realtime.session_setup");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if event(&mut socket, cancel, deadline).await?["type"] == "session.created" {
                break;
            }
        }
        send(&mut socket, session(voice)).await?;
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let value = event(&mut socket, cancel, deadline).await?;
            if value["type"] == "session.updated" {
                let output = &value["session"]["audio"]["output"];
                if output["format"]["type"] != "audio/pcm"
                    || output["format"]["rate"] != RATE
                    || output["voice"] != voice
                {
                    return Err("Realtime nie potwierdziło wybranego formatu PCM24k i głosu".into());
                }
                break;
            }
        }
        session_time.success();
        drop(session_time);
        let mut pcm = Vec::new();
        let mut transcripts = Vec::new();
        for (index,part) in parts.iter().enumerate() {
            let mut segment_time =
                crate::performance::Span::new("realtime.segment_audio_transcript");
            if *cancel.borrow() {
                return Err("Anulowano generowanie audio".into());
            }
            if let Some((audio,transcript))=&cached[index] {
                if pcm.len()+audio.len()>MAX_PCM{return Err("Nagranie przekracza 15 minut".into());}
                pcm.extend(audio);transcripts.push(transcript.clone());
                segment_time.cached();
                crate::performance::diagnostic!("[recovery] stage=speech segment={index} action=reused_verified_pcm");
                continue;
            }
            send(&mut socket, response(part)).await?;
            let deadline = Instant::now() + Duration::from_secs(120);
            let mut collector = Collector::default();
            loop {
                if collector.accept(&event(&mut socket, cancel, deadline).await?)? {
                    break;
                }
            }
            let (audio, transcript) = collector.finish(part)?;
            let repo=repository.clone();let text=part.clone();let voice=voice.to_string();let saved=(audio.clone(),transcript.clone());
            match tauri::async_runtime::spawn_blocking(move||super::segment_cache::save(&repo,&text,&voice,&saved)).await {
                Ok(Ok(()))=>crate::performance::diagnostic!("[recovery] stage=speech segment={index} action=saved_verified_pcm"),
                _=>crate::performance::diagnostic!("[recovery] stage=speech segment={index} action=continued reason=segment_cache_write_failed"),
            }
            segment_time.success();
            drop(segment_time);
            if pcm.len() + audio.len() > MAX_PCM {
                return Err("Nagranie przekracza 15 minut".into());
            }
            pcm.extend(audio);
            transcripts.push(transcript);
        }
        Ok((pcm, transcripts.join("\n\n")))
    }
    .await;
    if let Err(error) = &result {
        // Errors here have already been sanitized by the protocol/API adapter.
        crate::performance::diagnostic!(
            "[recovery] stage=speech action=deferred_preserved_verified_segments reason={error}"
        );
        let _ = timeout(
            Duration::from_secs(1),
            send(&mut socket, json!({"type":"response.cancel"})),
        )
        .await;
    }
    let _ = timeout(Duration::from_secs(1), socket.close(None)).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(kind: &str) -> Value {
        json!({"type":kind,"response_id":"r","item_id":"i","output_index":0,"content_index":0})
    }
    #[test]
    fn protocol_assembles_odd_pcm_chunks_and_checks_transcript() {
        let mut c = Collector::default();
        c.accept(&json!({"type":"response.created","response":{"id":"r"}}))
            .unwrap();
        for bytes in [&[1u8][..], &[2, 3, 4][..]] {
            let mut e = event("response.output_audio.delta");
            e["delta"] = STANDARD.encode(bytes).into();
            c.accept(&e).unwrap();
        }
        c.accept(&event("response.output_audio.done")).unwrap();
        let mut e = event("response.output_audio_transcript.done");
        e["transcript"] = "To jest test.".into();
        c.accept(&e).unwrap();
        assert!(
            c.accept(&json!({"type":"response.done","response":{"id":"r","status":"completed"}}))
                .unwrap()
        );
        assert_eq!(c.finish("To jest test.").unwrap().0, vec![1, 2, 3, 4]);
    }
    #[test]
    fn rejects_incomplete_wrong_response_and_no_microphone_in_contract() {
        let mut c = Collector::default();
        c.accept(&json!({"type":"response.created","response":{"id":"r"}}))
            .unwrap();
        assert!(
            c.accept(&json!({"type":"response.done","response":{"id":"r","status":"incomplete"}}))
                .is_err()
        );
        let mut wrong = event("response.output_audio.delta");
        wrong["response_id"] = "other".into();
        assert!(c.accept(&wrong).is_err());
        assert_eq!(
            session("alloy")["session"]["audio"]["input"]["turn_detection"],
            Value::Null
        );
        assert_eq!(response("tekst")["response"]["conversation"], "none");
        assert_eq!(
            response("tekst")["response"]["input"][0]["content"][0]["type"],
            "input_text"
        );
    }
}
