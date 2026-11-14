pub(crate) mod ducking;
mod fade;
#[cfg(windows)]
mod windows;
pub use ducking::DuckingController;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSession {
    pub id: String,
    pub session_identifier: String,
    pub device_id: String,
    pub pid: u32,
    pub process_name: Option<String>,
    pub display_name: String,
    pub volume: f32,
    pub state: &'static str,
}

#[derive(Debug, serde::Serialize)]
pub struct NativeError {
    code: &'static str,
    message: String,
}

impl NativeError {
    fn audio(message: impl ToString) -> Self {
        Self {
            code: "audio_failure",
            message: message.to_string(),
        }
    }
}

#[tauri::command]
pub async fn audio_list_sessions() -> Result<Vec<AudioSession>, NativeError> {
    // COM objects never leave this worker. Only owned DTOs cross the thread boundary.
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        {
            windows::list_sessions().map_err(NativeError::audio)
        }
        #[cfg(not(windows))]
        {
            Err(NativeError {
                code: "not_supported",
                message: "Windows is required".into(),
            })
        }
    })
    .await
    .map_err(NativeError::audio)?
}
