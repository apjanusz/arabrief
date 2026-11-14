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
    Duck(String, mpsc::Sender<Result<DuckResult, String>>),
    Restore(String, mpsc::Sender<Result<Outcome, String>>),
    Shutdown(mpsc::Sender<Result<(), String>>),
}

#[derive(Clone)]
pub struct DuckingController {
    sender: mpsc::Sender<Command>,
    closing: Arc<AtomicBool>,
}

impl DuckingController {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            #[cfg(windows)]
            super::windows::ducking::run(receiver);
            #[cfg(not(windows))]
            for command in receiver {
                match command {
                    Command::Duck(_, reply) => {
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
) -> Result<DuckResult, NativeError> {
    let controller = controller.inner().clone();
    if request.session_id.is_empty() || controller.closing.load(Ordering::SeqCst) {
        return Err(NativeError::audio("Invalid session or application closing"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        controller.request(|reply| Command::Duck(request.session_id, reply))
    })
    .await
    .map_err(NativeError::audio)?
    .map_err(NativeError::audio)
}

#[tauri::command]
pub async fn audio_restore(
    request: RestoreRequest,
    controller: tauri::State<'_, DuckingController>,
) -> Result<Outcome, NativeError> {
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        controller.request(|reply| Command::Restore(request.restoration_token, reply))
    })
    .await
    .map_err(NativeError::audio)?
    .map_err(NativeError::audio)
}
