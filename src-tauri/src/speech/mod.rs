mod chapters;
mod format;
#[cfg(test)]
mod realtime;
mod segment_cache;
mod tts;
use crate::settings::SettingsRepository;
use format::{MODEL, RATE};
use rusqlite::{OptionalExtension, params};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::Manager;
use tokio::sync::watch;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechAsset {
    resource_id: String,
    duration_ms: u32,
    model: String,
    voice: String,
    transcript: String,
    verification: String,
    segment_count: usize,
    cached: bool,
    #[serde(default)]
    chapters: Vec<ChapterAudio>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChapterAudio {
    article_id: String,
    position: u32,
    start_ms: u32,
    end_ms: u32,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct Publication {
    asset: SpeechAsset,
    file_hash: String,
}
struct Job {
    id: String,
    cancel: watch::Sender<bool>,
}
struct Jobs {
    active: Mutex<Option<Job>>,
    finished: Condvar,
    closing: AtomicBool,
    cancelled: Mutex<std::collections::VecDeque<String>>,
}
struct JobGuard(Arc<Jobs>);
impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        self.0.finished.notify_all();
    }
}
pub struct SpeechService {
    repository: Arc<SettingsRepository>,
    jobs: Arc<Jobs>,
}
struct Prepared {
    broadcast_id: i64,
    text: String,
    voice: String,
    key: String,
    parts: Vec<String>,
    chapters: Vec<ChapterAudio>,
}
trait SpeechProvider {
    fn synthesize<'a>(
        &'a self,
        parts: &'a [String],
        voice: &'a str,
        cancel: &'a mut watch::Receiver<bool>,
    ) -> impl std::future::Future<Output = Result<(Vec<u8>, String), String>> + Send + 'a;
}
struct OpenAiTts(Arc<SettingsRepository>);
impl SpeechProvider for OpenAiTts {
    async fn synthesize<'a>(
        &'a self,
        parts: &'a [String],
        voice: &'a str,
        cancel: &'a mut watch::Receiver<bool>,
    ) -> Result<(Vec<u8>, String), String> {
        tts::synthesize(parts, voice, cancel, self.0.clone()).await
    }
}
impl SpeechService {
    pub(crate) fn restore_audio(
        &self,
        root: &Path,
        id: &str,
        voice: String,
    ) -> Result<Option<SpeechAsset>, String> {
        let input = self.prepare(id, Some(voice))?;
        self.cached(root, &input)
    }
    pub fn new(repository: Arc<SettingsRepository>) -> Self {
        Self {
            repository,
            jobs: Arc::new(Jobs {
                active: Mutex::new(None),
                finished: Condvar::new(),
                closing: AtomicBool::new(false),
                cancelled: Mutex::new(std::collections::VecDeque::new()),
            }),
        }
    }
    fn start(&self, id: &str) -> Result<(JobGuard, watch::Receiver<bool>), String> {
        if id.is_empty()
            || id.len() > 64
            || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("Nieprawidłowy identyfikator generowania".into());
        }
        let mut active = self
            .jobs
            .active
            .lock()
            .map_err(|_| "Moduł mowy niedostępny")?;
        let mut cancelled = self
            .jobs
            .cancelled
            .lock()
            .map_err(|_| "Moduł mowy niedostępny")?;
        if let Some(position) = cancelled.iter().position(|entry| entry == id) {
            cancelled.remove(position);
            return Err("Anulowano generowanie audio".into());
        }
        if self.jobs.closing.load(Ordering::SeqCst) || active.is_some() {
            return Err("Generowanie audio jest zajęte lub aplikacja jest zamykana".into());
        }
        let (cancel, receiver) = watch::channel(false);
        *active = Some(Job {
            id: id.into(),
            cancel,
        });
        Ok((JobGuard(self.jobs.clone()), receiver))
    }
    fn cancel(&self, id: &str) -> bool {
        let active = self.jobs.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = active.as_ref().filter(|job| job.id == id) {
            let _ = job.cancel.send(true);
            true
        } else {
            if !id.is_empty()
                && id.len() <= 64
                && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                let mut cancelled = self
                    .jobs
                    .cancelled
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                if !cancelled.iter().any(|entry| entry == id) {
                    if cancelled.len() >= 64 {
                        cancelled.pop_front();
                    }
                    cancelled.push_back(id.into());
                }
                true
            } else {
                false
            }
        }
    }
    pub fn shutdown(&self) {
        self.jobs.closing.store(true, Ordering::SeqCst);
        let mut active = self.jobs.active.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = active.as_ref() {
            let _ = job.cancel.send(true);
        }
        while active.is_some() {
            active = self
                .jobs
                .finished
                .wait(active)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
    fn prepare(&self, broadcast_id: &str, voice: Option<String>) -> Result<Prepared, String> {
        let id: i64 = broadcast_id
            .parse()
            .map_err(|_| "Nieprawidłowy identyfikator scenariusza")?;
        let voice = voice.unwrap_or(self.repository.load()?.open_ai_voice);
        let voice = if format::VOICES.contains(&voice.as_str()) {
            voice
        } else {
            "marin".into()
        };
        if !format::VOICES.contains(&voice.as_str()) {
            return Err("Głos nie jest obsługiwany przez Realtime".into());
        }
        let text = self.repository.with_database(|db| {
            db.query_row(
                "SELECT text FROM content_broadcasts WHERE id=?1",
                [id],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| "Nie znaleziono zapisanego scenariusza".into())
        })?;
        let chapter_rows=self.repository.with_database(|db| {
            let mut statement=db.prepare("SELECT CAST(article_id AS TEXT),position,text,start_ms,end_ms FROM broadcast_chapters WHERE broadcast_id=?1 AND state!='audio_failed' ORDER BY position").map_err(|e|e.to_string())?;
            statement.query_map([id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<u32>>(3)?,r.get::<_,Option<u32>>(4)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
        })?;
        let parts = if chapter_rows.is_empty() {
            format::segments(&text)?
        } else {
            chapter_rows
                .iter()
                .filter_map(|row| format::segments(&row.2).ok())
                .flatten()
                .collect()
        };
        let chapters = chapter_rows
            .into_iter()
            .filter_map(|(article_id, position, _, start_ms, end_ms)| {
                Some(ChapterAudio {
                    article_id,
                    position,
                    start_ms: start_ms?,
                    end_ms: end_ms?,
                })
            })
            .collect();
        let key = format::cache_key(&text, &voice);
        Ok(Prepared {
            broadcast_id: id,
            text,
            voice,
            key,
            parts,
            chapters,
        })
    }
    fn cached(&self, root: &Path, input: &Prepared) -> Result<Option<SpeechAsset>, String> {
        let found=self.repository.with_database(|db| db.query_row("SELECT resource_id,duration_ms,transcript,verification,segment_count,file_hash FROM speech_assets WHERE cache_key=?1",[&input.key],|r| Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,u32>(4)? as usize,r.get::<_,String>(5)?))).optional().map_err(|e|e.to_string()))?;
        if let Some((
            resource_id,
            duration_ms,
            transcript,
            verification,
            segment_count,
            file_hash,
        )) = found
        {
            if resource_id != resource(&input.key) {
                return Err("Nieprawidłowy wpis cache audio".into());
            }
            let size = std::fs::metadata(root.join(&resource_id))
                .map_err(|_| "Plik cache audio jest niedostępny; nie ponowiono płatnej generacji")?
                .len();
            if size > (format::MAX_PCM + 44) as u64 {
                return Err("Plik cache audio przekracza limit".into());
            }
            let bytes = std::fs::read(root.join(&resource_id)).map_err(
                |_| "Plik cache audio jest niedostępny; nie ponowiono płatnej generacji",
            )?;
            if bytes.len() > format::MAX_PCM + 44 || format::hash(&bytes) != file_hash {
                return Err(
                    "Plik cache audio jest uszkodzony; nie ponowiono płatnej generacji".into(),
                );
            }
            format::verify(&input.text, &transcript)?;
            return Ok(Some(SpeechAsset {
                resource_id,
                duration_ms,
                model: MODEL.into(),
                voice: input.voice.clone(),
                transcript,
                verification,
                segment_count,
                cached: true,
                chapters: self.cached_chapter_spans(input)?,
            }));
        }
        if root.join(resource(&input.key)).exists() {
            // WAV publication succeeded but the SQLite write may have failed or been interrupted.
            let journal = root.join(format!("{}.json", resource(&input.key)));
            if std::fs::metadata(&journal)
                .map_err(|_| "Brak dziennika publikacji WAV")?
                .len()
                > 128 * 1024
            {
                return Err("Nieprawidłowy dziennik WAV".into());
            }
            let publication: Publication =
                serde_json::from_slice(&std::fs::read(journal).map_err(|_| "Brak dziennika WAV")?)
                    .map_err(|_| "Uszkodzony dziennik WAV")?;
            let path = root.join(resource(&input.key));
            if std::fs::metadata(&path).map_err(|_| "Brak WAV")?.len()
                > (format::MAX_PCM + 44) as u64
            {
                return Err("WAV przekracza limit".into());
            }
            let bytes = std::fs::read(&path).map_err(|_| "Nie można odczytać WAV")?;
            let mut asset = publication.asset;
            if asset.resource_id != resource(&input.key)
                || asset.voice != input.voice
                || asset.model != MODEL
                || asset.segment_count != input.parts.len()
                || bytes.len() < 46
                || format::hash(&bytes) != publication.file_hash
                || format::wav(&bytes[44..])? != bytes
            {
                return Err("Nieprawidłowy zachowany WAV".into());
            }
            format::verify(&input.text, &asset.transcript)?;
            asset.duration_ms = ((bytes.len() - 44) as u64 * 1000 / (RATE as u64 * 2)) as u32;
            asset.verification = "tts_input_text".into();
            self.index_asset(&path, input, &asset, &publication.file_hash)?;
            asset.cached = true;
            eprintln!("[recovery] stage=speech action=reindexed_verified_wav");
            return Ok(Some(asset));
        }
        Ok(None)
    }
    fn persist(
        &self,
        root: &Path,
        input: &Prepared,
        pcm: &[u8],
        transcript: String,
        cancel: &watch::Receiver<bool>,
    ) -> Result<SpeechAsset, String> {
        format::verify(&input.text, &transcript)?;
        check_cancel(cancel)?;
        let bytes = format::wav(pcm)?;
        std::fs::create_dir_all(root).map_err(|_| "Nie można utworzyć katalogu audio")?;
        let resource_id = resource(&input.key);
        let path = root.join(&resource_id);
        let temp = root.join(format!(
            "{}-{}.part",
            input.key,
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let _cleanup = Cleanup(vec![temp.clone()]);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "Nie można utworzyć pliku tymczasowego WAV")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "Nie można zapisać WAV")?;
        drop(file);
        check_cancel(cancel)?;
        let duration_ms = (pcm.len() as u64 * 1000 / (RATE as u64 * 2)) as u32;
        let asset = SpeechAsset {
            resource_id,
            duration_ms,
            model: MODEL.into(),
            voice: input.voice.clone(),
            transcript,
            verification: "tts_input_text".into(),
            segment_count: input.parts.len(),
            cached: false,
            chapters: input.chapters.clone(),
        };
        let journal = root.join(format!("{}.json", asset.resource_id));
        let publication = Publication {
            asset: asset.clone(),
            file_hash: format::hash(&bytes),
        };
        let mut metadata =
            std::fs::File::create(&journal).map_err(|_| "Nie można zapisać dziennika WAV")?;
        metadata
            .write_all(&serde_json::to_vec(&publication).map_err(|_| "Błąd dziennika WAV")?)
            .and_then(|_| metadata.sync_all())
            .map_err(|_| "Nie można zapisać dziennika WAV")?;
        drop(metadata);
        check_cancel(cancel)?;
        // Keep the complete WAV and its journal even if a subsequent database write fails.
        std::fs::hard_link(&temp,&path).map_err(|_|"Nie można opublikować WAV; plik już istnieje lub system plików nie obsługuje dowiązań")?;
        check_cancel(cancel)?;
        self.index_asset(&path, input, &asset, &publication.file_hash)?;
        Ok(asset)
    }
    fn index_asset(
        &self,
        path: &Path,
        input: &Prepared,
        asset: &SpeechAsset,
        file_hash: &str,
    ) -> Result<(), String> {
        self.repository.with_database(|db| {
            db.execute("INSERT INTO speech_assets VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,16,?11,?12,?13)",params![input.key,input.broadcast_id,MODEL,input.voice,asset.resource_id,path.to_string_lossy(),file_hash,chrono::Utc::now().to_rfc3339(),asset.duration_ms,RATE,input.parts.len() as u32,asset.transcript,asset.verification]).map_err(|_|"Nie można zapisać metadanych audio")?;
            Ok(())
        })
    }
    fn repair_cached(
        &self,
        root: &Path,
        input: &Prepared,
        cancel: &watch::Receiver<bool>,
    ) -> Result<Option<SpeechAsset>, String> {
        let mut pcm = Vec::new();
        let mut transcripts = Vec::new();
        for text in &input.parts {
            let Some((audio, transcript)) =
                segment_cache::load(&self.repository, text, &input.voice)?
            else {
                return Ok(None);
            };
            if pcm.len() + audio.len() > format::MAX_PCM {
                return Ok(None);
            }
            pcm.extend(audio);
            transcripts.push(transcript);
        }
        let transcript = transcripts.join("\n\n");
        format::verify(&input.text, &transcript)?;
        check_cancel(cancel)?;
        let path = root.join(resource(&input.key));
        if path.exists() {
            let archive = root.join(format!(
                "{}.corrupt-{}",
                resource(&input.key),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
            ));
            std::fs::rename(&path, archive)
                .map_err(|_| "Nie można odłożyć uszkodzonego cache WAV")?;
        }
        self.repository.with_database(|db| {
            db.execute("DELETE FROM speech_assets WHERE cache_key=?1", [&input.key])
                .map_err(|e| e.to_string())?;
            Ok(())
        })?;
        let asset = self.persist(root, input, &pcm, transcript, cancel)?;
        eprintln!("[recovery] stage=speech action=rebuilt_wav_from_verified_segments");
        Ok(Some(SpeechAsset {
            cached: true,
            ..asset
        }))
    }
    async fn generate(
        self: Arc<Self>,
        root: PathBuf,
        broadcast_id: String,
        voice: Option<String>,
        cancel: &mut watch::Receiver<bool>,
        provider: &impl SpeechProvider,
    ) -> Result<SpeechAsset, String> {
        let mut total = crate::performance::Span::new("speech.total");
        check_cancel(cancel)?;
        let service = self.clone();
        let id = broadcast_id.clone();
        let directory = root.clone();
        let receiver = cancel.clone();
        let (input, cached) = tauri::async_runtime::spawn_blocking(move || {
            let input = crate::performance::measure("speech.prepare_sqlite", || {
                service.prepare(&id, voice)
            })?;
            let cached = crate::performance::measure("speech.cache_verify", || {
                match service.cached(&directory, &input) {
                    Ok(value) => Ok(value),
                    Err(error) => match service.repair_cached(&directory, &input, &receiver)? {
                        Some(asset) => Ok(Some(asset)),
                        None => Err(error),
                    },
                }
            })?;
            Ok::<_, String>((input, cached))
        })
        .await
        .map_err(|_| "Błąd przygotowania audio")??;
        check_cancel(cancel)?;
        if let Some(cached) = cached {
            total.cached();
            return Ok(cached);
        }
        if self.has_chapters(input.broadcast_id)? {
            return self
                .generate_chapter_audio(root, input, cancel, provider)
                .await;
        }
        let mut model = crate::performance::Span::new("speech.model");
        let (pcm, transcript) = tokio::time::timeout(
            std::time::Duration::from_secs(15 * 60),
            provider.synthesize(&input.parts, &input.voice, cancel),
        )
        .await
        .map_err(|_| "Przekroczono całkowity czas generowania audio")??;
        model.success();
        drop(model);
        check_cancel(cancel)?;
        let receiver = cancel.clone();
        let asset = tauri::async_runtime::spawn_blocking(move || {
            crate::performance::measure("speech.persist_wav_sqlite", || {
                self.persist(&root, &input, &pcm, transcript, &receiver)
            })
        })
        .await
        .map_err(|_| "Błąd zapisu audio")??;
        total.success();
        Ok(asset)
    }
}
struct Cleanup(Vec<PathBuf>);
impl Drop for Cleanup {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}
fn resource(key: &str) -> String {
    format!("speech-{key}.wav")
}
fn check_cancel(cancel: &watch::Receiver<bool>) -> Result<(), String> {
    if *cancel.borrow() {
        Err("Anulowano generowanie audio".into())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn speech_generate(
    app: tauri::AppHandle,
    broadcast_id: String,
    voice: Option<String>,
    job_id: String,
    service: tauri::State<'_, Arc<SpeechService>>,
) -> Result<SpeechAsset, String> {
    crate::background::ensure_running(&app)?;
    let service = service.inner().clone();
    let (_job, mut cancel) = service.start(&job_id)?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "Brak katalogu danych aplikacji")?
        .join("audio");
    let provider = OpenAiTts(service.repository.clone());
    service
        .generate(root, broadcast_id, voice, &mut cancel, &provider)
        .await
}
#[tauri::command]
pub fn speech_cancel(job_id: String, service: tauri::State<'_, Arc<SpeechService>>) -> bool {
    service.cancel(&job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    const SCRIPT: &str =
        "To jest spokojna audycja po polsku. Wszystkie słowa pozostają w scenariuszu.";
    fn setup() -> (Arc<SpeechService>, PathBuf) {
        let repo = Arc::new(SettingsRepository::open(Path::new(":memory:")).unwrap());
        repo.with_database(|db| { db.execute("INSERT INTO content_broadcasts(created_at,title,model,text,estimated_duration_ms,topics_json,draft_json) VALUES ('2026-01-01','Test','fake',?1,1000,'[]','{}')",[SCRIPT]).unwrap(); Ok(()) }).unwrap();
        let root = std::env::temp_dir().join(format!(
            "arabrief-speech-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        (Arc::new(SpeechService::new(repo)), root)
    }
    pub(super) fn clean(root: &Path) {
        if let Ok(files) = std::fs::read_dir(root) {
            for file in files {
                std::fs::remove_file(file.unwrap().path()).unwrap();
            }
            std::fs::remove_dir(root).unwrap();
        }
    }
    struct Fake {
        calls: AtomicUsize,
        failure: bool,
        mismatch: bool,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                failure: false,
                mismatch: false,
            }
        }
    }
    impl SpeechProvider for Fake {
        async fn synthesize<'a>(
            &'a self,
            parts: &'a [String],
            _: &'a str,
            _: &'a mut watch::Receiver<bool>,
        ) -> Result<(Vec<u8>, String), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.failure {
                return Err("API error".into());
            }
            Ok((
                vec![0; 48000],
                if self.mismatch {
                    "Nieznane dodatkowe fakty".into()
                } else {
                    parts.join("\n\n")
                },
            ))
        }
    }
    #[tokio::test]
    async fn caches_verified_wav_without_second_provider_call_and_detects_corruption() {
        let (service, root) = setup();
        let fake = Fake::new();
        let (guard, mut cancel) = service.start("first").unwrap();
        let first = service
            .clone()
            .generate(
                root.clone(),
                "1".into(),
                Some("marin".into()),
                &mut cancel,
                &fake,
            )
            .await
            .unwrap();
        drop(guard);
        assert!(!first.cached);
        assert_eq!(first.duration_ms, 1000);
        let wav = std::fs::read(root.join(&first.resource_id)).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 48044);
        let (guard, mut cancel) = service.start("second").unwrap();
        let cached = service
            .clone()
            .generate(
                root.clone(),
                "1".into(),
                Some("marin".into()),
                &mut cancel,
                &fake,
            )
            .await
            .unwrap();
        drop(guard);
        assert!(cached.cached);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        service
            .repository
            .with_database(|db| {
                assert_eq!(
                    db.query_row("SELECT COUNT(*) FROM speech_assets", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    1
                );
                Ok(())
            })
            .unwrap();
        std::fs::write(root.join(&first.resource_id), b"bad WAV").unwrap();
        let (_guard, mut cancel) = service.start("third").unwrap();
        assert!(
            service
                .clone()
                .generate(
                    root.clone(),
                    "1".into(),
                    Some("marin".into()),
                    &mut cancel,
                    &fake
                )
                .await
                .is_err()
        );
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        clean(&root);
    }
    #[tokio::test]
    async fn api_failure_and_transcript_mismatch_are_not_retried_or_saved() {
        for mismatch in [false, true] {
            let (service, root) = setup();
            let fake = Fake {
                failure: !mismatch,
                mismatch,
                ..Fake::new()
            };
            let (_guard, mut cancel) = service.start("test").unwrap();
            assert!(
                service
                    .clone()
                    .generate(root.clone(), "1".into(), None, &mut cancel, &fake)
                    .await
                    .is_err()
            );
            assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
            assert!(!root.exists());
            service
                .repository
                .with_database(|db| {
                    assert_eq!(
                        db.query_row("SELECT COUNT(*) FROM speech_assets", [], |r| r
                            .get::<_, i64>(0))
                            .unwrap(),
                        0
                    );
                    Ok(())
                })
                .unwrap();
        }
    }
    #[tokio::test]
    async fn cancellation_is_scoped_and_prevents_calls_before_start() {
        let (service, root) = setup();
        let fake = Fake::new();
        service.cancel("early");
        assert!(service.start("early").is_err());
        let (guard, mut cancel) = service.start("real").unwrap();
        assert!(service.start("duplicate").is_err());
        service.cancel("other");
        assert!(!*cancel.borrow());
        service.cancel("real");
        assert!(*cancel.borrow());
        assert!(
            service
                .clone()
                .generate(root.clone(), "1".into(), None, &mut cancel, &fake)
                .await
                .is_err()
        );
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
        drop(guard);
        service.shutdown();
        assert!(service.start("after-close").is_err());
    }
    #[tokio::test]
    async fn cancelled_generation_in_flight_never_publishes_audio() {
        struct Waiting;
        impl SpeechProvider for Waiting {
            async fn synthesize<'a>(
                &'a self,
                _: &'a [String],
                _: &'a str,
                cancel: &'a mut watch::Receiver<bool>,
            ) -> Result<(Vec<u8>, String), String> {
                let _ = cancel.changed().await;
                Err("cancelled".into())
            }
        }
        let (service, root) = setup();
        let (_guard, mut cancel) = service.start("flight").unwrap();
        let task = service
            .clone()
            .generate(root.clone(), "1".into(), None, &mut cancel, &Waiting);
        let stop = async {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            service.cancel("flight");
        };
        let (result, _) = tokio::join!(task, stop);
        assert!(result.is_err());
        assert!(!root.exists());
    }
    #[tokio::test]
    async fn sqlite_failure_preserves_verified_wav_and_reindexes_without_paid_retry() {
        let (service, root) = setup();
        service.repository.with_database(|db|db.execute_batch("CREATE TRIGGER fail_speech BEFORE INSERT ON speech_assets BEGIN SELECT RAISE(ABORT,'test'); END;").map_err(|e|e.to_string())).unwrap();
        let (guard, mut cancel) = service.start("failure").unwrap();
        let fake = Fake::new();
        assert!(
            service
                .clone()
                .generate(root.clone(), "1".into(), None, &mut cancel, &fake)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        drop(guard);
        service
            .repository
            .with_database(|db| {
                db.execute_batch("DROP TRIGGER fail_speech").unwrap();
                Ok(())
            })
            .unwrap();
        let (_guard, mut cancel) = service.start("resume").unwrap();
        let recovered = service
            .clone()
            .generate(root.clone(), "1".into(), None, &mut cancel, &fake)
            .await
            .unwrap();
        assert!(recovered.cached);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        clean(&root);
    }
    #[tokio::test]
    async fn corrupt_wav_rebuilds_locally_when_all_verified_segments_survived() {
        let (service, root) = setup();
        let fake = Fake::new();
        let (guard, mut cancel) = service.start("first").unwrap();
        let first = service
            .clone()
            .generate(
                root.clone(),
                "1".into(),
                Some("marin".into()),
                &mut cancel,
                &fake,
            )
            .await
            .unwrap();
        drop(guard);
        segment_cache::save(
            &service.repository,
            SCRIPT,
            "marin",
            &(vec![0; 48000], SCRIPT.into()),
        )
        .unwrap();
        std::fs::write(root.join(&first.resource_id), b"broken").unwrap();
        let (_guard, mut cancel) = service.start("repair").unwrap();
        let repaired = service
            .clone()
            .generate(
                root.clone(),
                "1".into(),
                Some("marin".into()),
                &mut cancel,
                &fake,
            )
            .await
            .unwrap();
        assert!(repaired.cached);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            &std::fs::read(root.join(&repaired.resource_id)).unwrap()[..4],
            b"RIFF"
        );
        assert!(std::fs::read_dir(&root).unwrap().any(|file| {
            file.unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".corrupt-")
        }));
        clean(&root);
    }
    #[tokio::test]
    async fn native_recovery_returns_verified_ready_audio_not_newer_incomplete_work() {
        let (service, root) = setup();
        let settings = service.repository.load().unwrap();
        let source = serde_json::json!({"id":"1","provider":"fixture","title":SCRIPT,"url":"https://example.org/news","source":"Fixture","sourceUrl":null,"publishedAt":null,"rawPublicationDate":null,"descriptionHtml":null,"guid":null,"feedUrl":"https://example.org/rss","fetchedAt":"2026-11-16T00:00:00Z","topics":["Nauka"]});
        let draft = serde_json::json!({"title":"Nauka","segments":[{"topic":"Nauka","text":SCRIPT,"evidence":[{"source_id":"1","quote":SCRIPT}]}],"limitations":[],"insufficient_material":true});
        service.repository.with_database(|db|{
            db.execute("UPDATE content_broadcasts SET created_at=?1,draft_json=?2",params![chrono::Utc::now().to_rfc3339(),draft.to_string()]).unwrap();
            db.execute("INSERT INTO news_articles(id,provider,canonical_url,metadata_json,first_seen_at) VALUES (1,'fixture','https://example.org/news',?1,0)",[source.to_string()]).unwrap();
            db.execute("INSERT INTO content_sources VALUES (1,1,?1)",[source.to_string()]).unwrap();
            db.execute("INSERT INTO pipeline_preparations VALUES (1,?1)",[crate::recovery::settings_key(&settings)]).unwrap();Ok(())
        }).unwrap();
        let (_guard, mut cancel) = service.start("verified").unwrap();
        let fake = Fake::new();
        service
            .clone()
            .generate(root.clone(), "1".into(), None, &mut cancel, &fake)
            .await
            .unwrap();
        service.repository.with_database(|db|{
            db.execute("INSERT INTO content_broadcasts SELECT 2,created_at,title,model,text,estimated_duration_ms,topics_json,'broken' FROM content_broadcasts WHERE id=1",[]).unwrap();
            db.execute("INSERT INTO pipeline_preparations VALUES (2,?1)",[crate::recovery::settings_key(&settings)]).unwrap();Ok(())
        }).unwrap();
        let recovered = crate::recovery::restore(&service.repository, &service, &root)
            .unwrap()
            .unwrap();
        let wire = serde_json::to_value(recovered).unwrap();
        assert_eq!(wire["script"]["id"], "1");
        assert_eq!(wire["audio"]["cached"], true);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        clean(&root);
    }
    #[tokio::test]
    async fn cache_is_reused_after_reopening_sqlite() {
        let nonce = format!(
            "{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        );
        let database = std::env::temp_dir().join(format!("arabrief-speech-{nonce}.sqlite"));
        let root = std::env::temp_dir().join(format!("arabrief-speech-audio-{nonce}"));
        {
            let repository = Arc::new(SettingsRepository::open(&database).unwrap());
            repository.with_database(|db| { db.execute("INSERT INTO content_broadcasts(created_at,title,model,text,estimated_duration_ms,topics_json,draft_json) VALUES ('2026-01-01','Test','fake',?1,1000,'[]','{}')",[SCRIPT]).unwrap(); Ok(()) }).unwrap();
            let service = Arc::new(SpeechService::new(repository));
            let (_guard, mut cancel) = service.start("first").unwrap();
            service
                .clone()
                .generate(root.clone(), "1".into(), None, &mut cancel, &Fake::new())
                .await
                .unwrap();
        }
        {
            let repository = Arc::new(SettingsRepository::open(&database).unwrap());
            let service = Arc::new(SpeechService::new(repository));
            let (_guard, mut cancel) = service.start("reopen").unwrap();
            let fake = Fake {
                failure: true,
                ..Fake::new()
            };
            let asset = service
                .clone()
                .generate(root.clone(), "1".into(), None, &mut cancel, &fake)
                .await
                .unwrap();
            assert!(asset.cached);
            assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
        }
        clean(&root);
        std::fs::remove_file(database).unwrap();
    }
}
