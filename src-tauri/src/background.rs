use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::ManagerExt;

#[derive(Default)]
pub struct Lifecycle {
    pub closing: AtomicBool,
    pub restored: AtomicBool,
    active: AtomicBool,
    stopped: Mutex<bool>,
    stopped_signal: Condvar,
    toggle: Mutex<Option<MenuItem<tauri::Wry>>>,
    language_items: Mutex<Option<(MenuItem<tauri::Wry>, MenuItem<tauri::Wry>)>>,
    recovery_message: Mutex<Option<String>>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchedulerStatus {
    active: bool,
    can_start: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchState {
    closing: bool,
    start_scheduler: bool,
    recovery_message: Option<String>,
}

pub fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn refresh_language(app: &tauri::AppHandle) {
    let state = app.state::<Arc<Lifecycle>>();
    let en = crate::settings::interface_is_english();
    if let Some((open, quit)) = state
        .language_items
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        let _ = open.set_text(if en {
            "Open AraBrief"
        } else {
            "Otwórz AraBrief"
        });
        let _ = quit.set_text(if en {
            "Quit application"
        } else {
            "Zakończ aplikację"
        });
    }
}

pub fn setup(app: &tauri::AppHandle, recovery_message: Option<String>) -> tauri::Result<()> {
    let en = crate::settings::interface_is_english();
    let open = MenuItem::with_id(
        app,
        "open",
        if en {
            "Open AraBrief"
        } else {
            "Otwórz AraBrief"
        },
        true,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(app, "toggle", "Start", false, None::<&str>)?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        if en {
            "Quit application"
        } else {
            "Zakończ aplikację"
        },
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(app, &[&open, &toggle, &quit])?;
    let state = app.state::<Arc<Lifecycle>>();
    *state
        .language_items
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some((open, quit));
    *state.toggle.lock().unwrap_or_else(|e| e.into_inner()) = Some(toggle);
    *state
        .recovery_message
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = recovery_message;
    TrayIconBuilder::with_id("arabrief")
        .icon(tauri::include_image!("icons/tray.png"))
        .tooltip("AraBrief")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "toggle" => {
                if !app.state::<Arc<Lifecycle>>().closing.load(Ordering::SeqCst) {
                    let _ = app.emit_to("main", "background-action", "toggle");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show(tray.app_handle());
            }
        })
        .build(app)?;
    // Native ticks also reach a hidden webview, independent of JS timer budgeting.
    let app = app.clone();
    std::thread::spawn(move || {
        while !app.state::<Arc<Lifecycle>>().closing.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));
            if app.state::<Arc<Lifecycle>>().active.load(Ordering::SeqCst) {
                let _ = app.emit_to("main", "background-tick", ());
            }
        }
    });
    Ok(())
}

pub fn request_exit(app: &tauri::AppHandle) {
    let lifecycle = app.state::<Arc<Lifecycle>>().inner().clone();
    if lifecycle.closing.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(toggle) = lifecycle
        .toggle
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        let _ = toggle.set_enabled(false);
    }
    let _ = app.emit_to("main", "background-action", "quit");
    let player = app.state::<crate::player::AudioPlayer>().inner().clone();
    player.begin_shutdown();
    let app = app.clone();
    std::thread::spawn(move || {
        // Close the native playback gate even when the webview is unresponsive.
        player.shutdown();
        let audio = app.state::<crate::audio::DuckingController>();
        if let Err(error) = audio.shutdown() {
            eprintln!("Audio restoration at shutdown failed; snapshot retained: {error}");
        }
        app.state::<Arc<crate::speech::SpeechService>>().shutdown();
        let stopped = lifecycle.stopped.lock().unwrap_or_else(|e| e.into_inner());
        let _ = lifecycle.stopped_signal.wait_timeout_while(
            stopped,
            Duration::from_secs(3),
            |stopped| !*stopped,
        );
        lifecycle.restored.store(true, Ordering::SeqCst);
        app.exit(0);
    });
}

#[tauri::command]
pub fn background_connect(app: tauri::AppHandle) -> Result<LaunchState, String> {
    let state = app.state::<Arc<Lifecycle>>();
    let settings = app
        .state::<Arc<crate::settings::SettingsRepository>>()
        .load()?;
    Ok(LaunchState {
        closing: state.closing.load(Ordering::SeqCst),
        start_scheduler: settings.start_scheduler_on_launch,
        recovery_message: state
            .recovery_message
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone(),
    })
}

#[tauri::command]
pub fn background_status(
    status: SchedulerStatus,
    lifecycle: tauri::State<'_, Arc<Lifecycle>>,
) -> Result<(), String> {
    if status.active && status.can_start {
        return Err("Nieprawidłowy stan harmonogramu".into());
    }
    lifecycle.active.store(status.active, Ordering::SeqCst);
    if let Some(toggle) = lifecycle
        .toggle
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        toggle
            .set_text(if status.active { "Stop" } else { "Start" })
            .map_err(|e| e.to_string())?;
        toggle
            .set_enabled(
                !lifecycle.closing.load(Ordering::SeqCst) && (status.active || status.can_start),
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn background_stopped(lifecycle: tauri::State<'_, Arc<Lifecycle>>) -> Result<(), String> {
    if !lifecycle.closing.load(Ordering::SeqCst) {
        return Err("Aplikacja nie jest zamykana".into());
    }
    *lifecycle.stopped.lock().unwrap_or_else(|e| e.into_inner()) = true;
    lifecycle.stopped_signal.notify_all();
    Ok(())
}

pub fn autostart_enabled(app: &tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

pub fn ensure_running(app: &tauri::AppHandle) -> Result<(), String> {
    if app.state::<Arc<Lifecycle>>().closing.load(Ordering::SeqCst) {
        Err("Aplikacja jest zamykana".into())
    } else {
        Ok(())
    }
}
pub fn set_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|e| e.to_string())
}
