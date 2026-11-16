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
            let repository = Arc::new(repository);
            app.manage(Arc::new(news::NewsService::new(repository.clone())));
            app.manage(Arc::new(content::ContentService::new(repository.clone())));
            app.manage(Arc::new(speech::SpeechService::new(repository.clone())));
            app.manage(repository);
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
            credentials::credentials_validate,
            credentials::credentials_delete,
            news::news_fetch,
            news::news_mark_used,
            content::content_generate,
            recovery::broadcast_restore,
            recovery::broadcast_claim,
            recovery::broadcast_complete,
            speech::speech_generate,
            speech::speech_cancel
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
                    let speech = app.state::<Arc<speech::SpeechService>>().inner().clone();
                    let restored = restored.clone();
                    std::thread::spawn(move || {
                        speech.shutdown();
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
mod content;
mod credentials;
mod news;
mod openai_errors;
mod performance;
mod player;
mod recovery;
mod settings;
mod speech;
mod work_queue;
