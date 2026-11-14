mod format;
mod realtime;
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

#[derive(Clone, Debug, serde::Serialize)]
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
    text: String,
    voice: String,
    key: String,
    parts: Vec<String>,
}
trait SpeechProvider {
    fn synthesize<'a>(
        &'a self,
        parts: &'a [String],
        voice: &'a str,
        cancel: &'a mut watch::Receiver<bool>,
    ) -> impl std::future::Future<Output = Result<(Vec<u8>, String), String>> + Send + 'a;
}
struct Realtime;
impl SpeechProvider for Realtime {
    async fn synthesize<'a>(
        &'a self,
        parts: &'a [String],
        voice: &'a str,
        cancel: &'a mut watch::Receiver<bool>,
    ) -> Result<(Vec<u8>, String), String> {
        realtime::synthesize(parts, voice, cancel).await
    }
}
impl SpeechService {
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
        let parts = format::segments(&text)?;
        let key = format::cache_key(&text, &voice);
        Ok(Prepared {
            text,
            voice,
            key,
            parts,
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
            }));
        }
        if root.join(resource(&input.key)).exists() {
            return Err("Istnieje plik audio bez poprawnego indeksu SQLite. Nie ponowiono płatnego żądania.".into());
        }
        Ok(None)
    }
    fn persist(
        &self,
        root: &Path,
        broadcast_id: &str,
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
        let mut cleanup = Cleanup(vec![temp.clone()]);
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
        // Atomic publication without overwriting an existing cache file (same directory/volume).
        std::fs::hard_link(&temp,&path).map_err(|_|"Nie można opublikować WAV; plik już istnieje lub system plików nie obsługuje dowiązań")?;
        cleanup.0.push(path.clone());
        let duration_ms = (pcm.len() as u64 * 1000 / (RATE as u64 * 2)) as u32;
        let asset = SpeechAsset {
            resource_id,
            duration_ms,
            model: MODEL.into(),
            voice: input.voice.clone(),
            transcript,
            verification: "realtime_transcript_normalized_exact".into(),
            segment_count: input.parts.len(),
            cached: false,
        };
        self.repository.with_database(|db| {
            check_cancel(cancel)?;
            db.execute("INSERT INTO speech_assets VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,16,?11,?12,?13)",params![input.key,broadcast_id,MODEL,input.voice,asset.resource_id,path.to_string_lossy(),format::hash(&bytes),chrono::Utc::now().to_rfc3339(),duration_ms,RATE,input.parts.len() as u32,asset.transcript,asset.verification]).map_err(|_|"Nie można zapisać metadanych audio")?;
            Ok(())
        })?;
        cleanup.0.retain(|p| p != &path); // keep completed file, remove only .part
        Ok(asset)
    }
    async fn generate(
        self: Arc<Self>,
        root: PathBuf,
        broadcast_id: String,
        voice: Option<String>,
        cancel: &mut watch::Receiver<bool>,
        provider: &impl SpeechProvider,
    ) -> Result<SpeechAsset, String> {
        check_cancel(cancel)?;
        let service = self.clone();
        let id = broadcast_id.clone();
        let directory = root.clone();
        let (input, cached) = tauri::async_runtime::spawn_blocking(move || {
            let input = service.prepare(&id, voice)?;
            let cached = service.cached(&directory, &input)?;
            Ok::<_, String>((input, cached))
        })
        .await
        .map_err(|_| "Błąd przygotowania audio")??;
        check_cancel(cancel)?;
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let (pcm, transcript) = tokio::time::timeout(
            std::time::Duration::from_secs(15 * 60),
            provider.synthesize(&input.parts, &input.voice, cancel),
        )
        .await
        .map_err(|_| "Przekroczono całkowity czas generowania audio")??;
        check_cancel(cancel)?;
        let receiver = cancel.clone();
        tauri::async_runtime::spawn_blocking(move || {
            self.persist(&root, &broadcast_id, &input, &pcm, transcript, &receiver)
        })
        .await
        .map_err(|_| "Błąd zapisu audio")?
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
    let service = service.inner().clone();
    let (_job, mut cancel) = service.start(&job_id)?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(|_| "Brak katalogu danych aplikacji")?
        .join("audio");
    service
        .generate(root, broadcast_id, voice, &mut cancel, &Realtime)
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
    fn clean(root: &Path) {
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
                Some("alloy".into()),
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
                Some("alloy".into()),
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
                    Some("alloy".into()),
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
    async fn sqlite_failure_removes_published_and_partial_files() {
        let (service, root) = setup();
        service.repository.with_database(|db|db.execute_batch("CREATE TRIGGER fail_speech BEFORE INSERT ON speech_assets BEGIN SELECT RAISE(ABORT,'test'); END;").map_err(|e|e.to_string())).unwrap();
        let (_guard, mut cancel) = service.start("failure").unwrap();
        assert!(
            service
                .clone()
                .generate(root.clone(), "1".into(), None, &mut cancel, &Fake::new())
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
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
