use super::{NativeError, fade::Outcome};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuckResult {
    pub restoration_token: String,
    pub outcome: Outcome,
}

pub(super) enum Command {
    Duck(String, DuckConfig, mpsc::Sender<Result<DuckResult, String>>),
    Restore(String, mpsc::Sender<Result<Outcome, String>>),
    Shutdown(mpsc::Sender<Result<(), String>>),
}
#[derive(Clone, Copy)]
pub(super) struct DuckConfig {
    pub ratio: f32,
    pub fade_out_ms: u32,
    pub fade_in_ms: u32,
}

#[derive(Clone)]
pub struct DuckingController {
    sender: mpsc::Sender<Command>,
    closing: Arc<AtomicBool>,
    config: Arc<std::sync::RwLock<DuckConfig>>,
}

impl DuckingController {
    pub(crate) fn configure(&self, ratio: f32, fade_out_ms: u32, fade_in_ms: u32) {
        *self.config.write().unwrap_or_else(|e| e.into_inner()) = DuckConfig {
            ratio,
            fade_out_ms,
            fade_in_ms,
        };
    }
    pub(crate) fn duck(&self, session_id: String) -> Result<DuckResult, String> {
        if self.closing.load(Ordering::SeqCst) {
            return Err("Application closing".into());
        }
        let config = *self.config.read().unwrap_or_else(|e| e.into_inner());
        self.request(|reply| Command::Duck(session_id, config, reply))
    }

    pub(crate) fn restore(&self, token: String) -> Result<Outcome, String> {
        self.request(|reply| Command::Restore(token, reply))
    }
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            #[cfg(windows)]
            super::windows::ducking::run(receiver);
            #[cfg(not(windows))]
            for command in receiver {
                match command {
                    Command::Duck(_, _, reply) => {
                        let _ = reply.send(Err("Windows required".into()));
                    }
                    Command::Restore(_, reply) => {
                        let _ = reply.send(Err("Windows required".into()));
                    }
                    Command::Shutdown(reply) => {
                        let _ = reply.send(Ok(()));
                        break;
                    }
                }
            }
        });
        Self {
            sender,
            closing: Arc::new(AtomicBool::new(false)),
            config: Arc::new(std::sync::RwLock::new(DuckConfig {
                ratio: super::fade::DUCK_RATIO,
                fade_out_ms: super::fade::FADE_DURATION.as_millis() as u32,
                fade_in_ms: super::fade::FADE_DURATION.as_millis() as u32,
            })),
        }
    }

    pub fn shutdown(&self) -> Result<(), String> {
        self.closing.store(true, Ordering::SeqCst);
        self.request(Command::Shutdown)
    }

    fn request<T>(
        &self,
        command: impl FnOnce(mpsc::Sender<Result<T, String>>) -> Command,
    ) -> Result<T, String> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(command(sender))
            .map_err(|_| "Audio worker unavailable".to_string())?;
        receiver
            .recv()
            .map_err(|_| "Audio worker stopped".to_string())?
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuckRequest {
    session_id: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    restoration_token: String,
}

#[tauri::command]
pub async fn audio_duck(
    request: DuckRequest,
    controller: tauri::State<'_, DuckingController>,
    player: tauri::State<'_, crate::player::AudioPlayer>,
) -> Result<DuckResult, NativeError> {
    if player.is_busy() {
        return Err(NativeError::audio("Trwa odtwarzanie audycji"));
    }
    let controller = controller.inner().clone();
    if request.session_id.is_empty() || controller.closing.load(Ordering::SeqCst) {
        return Err(NativeError::audio("Invalid session or application closing"));
    }
    tauri::async_runtime::spawn_blocking(move || controller.duck(request.session_id))
        .await
        .map_err(NativeError::audio)?
        .map_err(NativeError::audio)
}

#[tauri::command]
pub async fn audio_restore(
    request: RestoreRequest,
    controller: tauri::State<'_, DuckingController>,
    player: tauri::State<'_, crate::player::AudioPlayer>,
) -> Result<Outcome, NativeError> {
    if player.is_busy() {
        return Err(NativeError::audio("Najpierw zatrzymaj audycję"));
    }
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        controller.request(|reply| Command::Restore(request.restoration_token, reply))
    })
    .await
    .map_err(NativeError::audio)?
    .map_err(NativeError::audio)
}
