#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use std::sync::{Arc, atomic::Ordering};
    use tauri::Manager;
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--autostart") { background::show(app); }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().args(["--autostart"]).build())
        .manage(Arc::new(background::Lifecycle::default()))
        .manage(audio::DuckingController::new())
        .manage(player::AudioPlayer::new())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let repository =
                settings::SettingsRepository::open(&directory.join("settings.sqlite3"))
                    .map_err(std::io::Error::other)?;
            let preferences = repository.load().map_err(std::io::Error::other)?;
            let repository = Arc::new(repository);
            let recovery_message = match app.state::<audio::DuckingController>().initialize(repository.clone()) {
                Ok(message) => message,
                Err(error) => Some(format!("Nie udało się odzyskać głośności muzyki. Zapis pozostaje do ponownej próby: {error}")),
            };
            settings::apply(
                &preferences,
                app.state::<audio::DuckingController>().inner(),
                app.state::<player::AudioPlayer>().inner(),
            );
            app.manage(Arc::new(news::NewsService::new(repository.clone())));
            app.manage(Arc::new(content::ContentService::new(repository.clone())));
            app.manage(Arc::new(speech::SpeechService::new(repository.clone())));
            app.manage(repository);
            background::setup(app.handle(), recovery_message)?;
            if !std::env::args().any(|arg| arg == "--autostart") { background::show(app.handle()); }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            background::background_connect,
            background::background_status,
            background::background_stopped,
            audio::audio_list_sessions,
            audio::ducking::audio_duck,
            audio::ducking::audio_restore,
            player::audio_play,
            player::audio_stop,
            player::audio_set_paused,
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
        .on_window_event(|window, event| {
            if window.label() == "main" && let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.hide() { eprintln!("Nie udało się ukryć AraBrief: {error}"); }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && !app.state::<Arc<background::Lifecycle>>().restored.load(Ordering::SeqCst)
            {
                api.prevent_exit();
                background::request_exit(app);
            }
        });
}
mod audio;
mod background;
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
