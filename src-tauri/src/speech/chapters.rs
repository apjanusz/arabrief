//! Preserve complete chapter audio and skip a failed chapter, never its siblings.
use super::*;
#[cfg(test)]
mod tests;
impl SpeechService {
    pub(super) fn cached_chapter_spans(
        &self,
        input: &Prepared,
    ) -> Result<Vec<ChapterAudio>, String> {
        if !input.chapters.is_empty() {
            return Ok(input.chapters.clone());
        }
        self.repository.with_database(|db| {
            let current=db.prepare("SELECT CAST(article_id AS TEXT),position,text FROM broadcast_chapters WHERE broadcast_id=?1 AND state!='audio_failed' ORDER BY position").map_err(|e|e.to_string())?.query_map([input.broadcast_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
            let previous=db.prepare("SELECT c.text,c.start_ms,c.end_ms FROM broadcast_chapters c JOIN speech_assets a ON a.broadcast_id=c.broadcast_id WHERE a.cache_key=?1 AND c.state='audio_ready' ORDER BY c.position").map_err(|e|e.to_string())?.query_map([&input.key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,u32>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
            if current.len()!=previous.len() || current.iter().zip(&previous).any(|(current,previous)|current.2!=previous.0){return Ok(vec![]);}
            Ok(current.into_iter().zip(previous).map(|((article_id,position,_),(_,start_ms,end_ms))|ChapterAudio{article_id,position,start_ms,end_ms}).collect())
        })
    }
    pub(super) fn has_chapters(&self, id: i64) -> Result<bool, String> {
        self.repository.with_database(|db| {
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM broadcast_chapters WHERE broadcast_id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
        })
    }
    pub(super) async fn generate_chapter_audio(
        self: Arc<Self>,
        root: PathBuf,
        input: Prepared,
        cancel: &mut watch::Receiver<bool>,
        provider: &impl SpeechProvider,
    ) -> Result<SpeechAsset, String> {
        let rows=self.repository.with_database(|db| {
            let mut statement=db.prepare("SELECT CAST(article_id AS TEXT),position,text FROM broadcast_chapters WHERE broadcast_id=?1 AND state!='audio_failed' ORDER BY position").map_err(|e|e.to_string())?;
            statement.query_map([input.broadcast_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
        })?;
        let mut pcm = Vec::new();
        let mut transcript = Vec::new();
        let mut spans = Vec::new();
        for (id, position, text) in rows {
            check_cancel(cancel)?;
            let result = match format::segments(&text) {
                Ok(parts) => {
                    self.synthesize_chapter(provider, &parts, &input.voice, cancel)
                        .await
                }
                Err(error) => Err(error),
            }
            .and_then(|(audio, spoken)| {
                format::verify(&text, &spoken)?;
                if audio.is_empty()
                    || !audio.len().is_multiple_of(2)
                    || pcm.len() + audio.len() > format::MAX_PCM
                {
                    return Err("Rozdział przekracza limit audio lub ma nieprawidłowy PCM".into());
                }
                Ok((audio, spoken))
            });
            match result {
                Ok((audio, spoken)) => {
                    let start_ms = (pcm.len() as u64 * 1000 / (RATE as u64 * 2)) as u32;
                    pcm.extend(audio);
                    transcript.push(spoken);
                    let end_ms = (pcm.len() as u64 * 1000 / (RATE as u64 * 2)) as u32;
                    spans.push(ChapterAudio {
                        article_id: id,
                        position,
                        start_ms,
                        end_ms,
                    });
                }
                Err(error) => {
                    check_cancel(cancel)?;
                    eprintln!("[pipeline] stage=tts article_id={id} error={error}");
                    if crate::content::chapters::systemic(&error) {
                        crate::content::chapters::defer(&self.repository, &id)?;
                    } else {
                        crate::content::chapters::failed(&self.repository, &id, "tts")?;
                    }
                }
            }
        }
        check_cancel(cancel)?;
        let included: Vec<_> = spans
            .iter()
            .map(|span| (span.article_id.clone(), span.start_ms, span.end_ms))
            .collect();
        crate::content::chapters::publish_audio(&self.repository, input.broadcast_id, &included)?;
        if spans.is_empty() {
            return Err("Nie udało się przygotować mowy żadnego rozdziału. Teksty i poprawne fragmenty audio pozostają zapisane.".into());
        }
        let mut prepared = self.prepare(&input.broadcast_id.to_string(), Some(input.voice))?;
        prepared.chapters = spans;
        let receiver = cancel.clone();
        tauri::async_runtime::spawn_blocking(move || {
            self.persist(&root, &prepared, &pcm, transcript.join("\n\n"), &receiver)
        })
        .await
        .map_err(|_| "Błąd zapisu audycji rozdziałowej")?
    }
    async fn synthesize_chapter(
        &self,
        provider: &impl SpeechProvider,
        parts: &[String],
        voice: &str,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<(Vec<u8>, String), String> {
        for attempt in 0..2 {
            check_cancel(cancel)?;
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(300),
                provider.synthesize(parts, voice, cancel),
            )
            .await
            .map_err(|_| "Timeout mowy rozdziału (300 s)".to_string())
            .and_then(|result| result);
            if let Err(error) = &result {
                check_cancel(cancel)?;
                if attempt == 0
                    && (error.contains("Timeout")
                        || error.contains("połączenia")
                        || error.contains("połączenie"))
                {
                    continue;
                }
            }
            return result;
        }
        Err("Wyczerpano próby TTS".into())
    }
}
