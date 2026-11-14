#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tauri::Manager;
    let closing = Arc::new(AtomicBool::new(false));
    let restored = Arc::new(AtomicBool::new(false));
    tauri::Builder::default()
        .manage(audio::DuckingController::new())
        .invoke_handler(tauri::generate_handler![
            audio::audio_list_sessions,
            audio::ducking::audio_duck,
            audio::ducking::audio_restore
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !restored.load(Ordering::SeqCst)
            {
                api.prevent_exit();
                if !closing.swap(true, Ordering::SeqCst) {
                    let controller = app.state::<audio::DuckingController>().inner().clone();
                    let app = app.clone();
                    let restored = restored.clone();
                    std::thread::spawn(move || {
                        if let Err(error) = controller.shutdown() {
                            eprintln!("Audio restoration at shutdown failed: {error}");
                        }
                        restored.store(true, Ordering::SeqCst);
                        app.exit(0);
                    });
                }
            }
        });
}
mod audio;
