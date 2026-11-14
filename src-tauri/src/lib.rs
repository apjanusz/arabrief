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
        .manage(player::AudioPlayer::new())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let repository =
                settings::SettingsRepository::open(&directory.join("settings.sqlite3"))
                    .map_err(std::io::Error::other)?;
            let preferences = repository.load().map_err(std::io::Error::other)?;
            settings::apply(
                &preferences,
                app.state::<audio::DuckingController>().inner(),
                app.state::<player::AudioPlayer>().inner(),
            );
            app.manage(Arc::new(repository));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            audio::audio_list_sessions,
            audio::ducking::audio_duck,
            audio::ducking::audio_restore,
            player::audio_play,
            player::audio_stop,
            player::audio_get_status,
            player::audio_set_volume,
            settings::settings_load,
            settings::settings_save,
            credentials::credentials_save,
            credentials::credentials_check,
            credentials::credentials_delete
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
                    let player = app.state::<player::AudioPlayer>().inner().clone();
                    let restored = restored.clone();
                    std::thread::spawn(move || {
                        player.shutdown();
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
mod credentials;
mod player;
mod settings;
