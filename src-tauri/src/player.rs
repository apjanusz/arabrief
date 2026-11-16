use crate::audio::DuckingController;
use rodio::{
    Decoder, DeviceSinkBuilder, DeviceTrait, Player, Source, buffer::SamplesBuffer,
    cpal::traits::HostTrait,
};
use std::{
    io::Cursor,
    path::Path,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Duration,
};
use tauri::{Emitter, Manager};

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: String,
    pub playback_id: u64,
    pub volume: f32,
    pub output_device_id: Option<String>,
    pub error: Option<String>,
}
struct State {
    status: Status,
    busy: bool,
    stop: bool,
    closing: bool,
    pause_requested: bool,
    playback_key: Option<String>,
    pending_pause: Option<(String, bool)>,
}
#[derive(Clone)]
pub struct AudioPlayer(Arc<(Mutex<State>, Condvar)>);
impl AudioPlayer {
    pub fn new() -> Self {
        Self(Arc::new((
            Mutex::new(State {
                status: Status {
                    phase: "idle".into(),
                    playback_id: 0,
                    volume: 0.7,
                    output_device_id: None,
                    error: None,
                },
                busy: false,
                stop: false,
                closing: false,
                pause_requested: false,
                playback_key: None,
                pending_pause: None,
            }),
            Condvar::new(),
        )))
    }
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn get_status(&self) -> Status {
        self.state().status.clone()
    }
    fn phase(&self, app: &tauri::AppHandle, phase: &str) {
        let status = {
            let mut state = self.state();
            state.status.phase = phase.into();
            self.0.1.notify_all();
            state.status.clone()
        };
        let _ = app.emit("audio-playback-status", status);
    }
    pub fn stop(&self) {
        let mut state = self.state();
        state.stop = true;
        state.pause_requested = false;
        state.pending_pause = None;
        self.0.1.notify_all();
    }
    fn set_paused(&self, paused: bool, playback_key: String) -> Result<Status, String> {
        validate_playback_key(&playback_key)?;
        let mut state = self.state();
        if state.closing {
            return Err("Aplikacja jest zamykana".into());
        }
        if !state.busy {
            // Commands arriving after completion cannot pause another recording.
            if state.playback_key.as_ref() != Some(&playback_key) {
                state.pending_pause = Some((playback_key, paused));
            }
            return Ok(state.status.clone());
        }
        if state.playback_key.as_ref() != Some(&playback_key) {
            return Err("Żądanie pauzy dotyczy innej audycji".into());
        }
        state.pause_requested = paused;
        self.0.1.notify_all();
        while state.busy && !state.stop {
            if state.playback_key.as_ref() != Some(&playback_key) {
                return Err("Odtwarzana audycja została zmieniona".into());
            }
            if state.pause_requested != paused {
                return Err("Stan pauzy został zmieniony przez nowsze żądanie".into());
            }
            if state.status.phase == if paused { "paused" } else { "playing" } {
                break;
            }
            state = self.0.1.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if let Some(error) = &state.status.error {
            return Err(error.clone());
        }
        if state.stop || state.closing {
            return Err("Odtwarzanie zostało zatrzymane".into());
        }
        Ok(state.status.clone())
    }
    pub fn shutdown(&self) {
        let mut state = self.state();
        state.closing = true;
        state.stop = true;
        state.pause_requested = false;
        self.0.1.notify_all();
        while state.busy {
            state = self.0.1.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
    pub fn begin_shutdown(&self) {
        let mut state = self.state();
        state.closing = true;
        state.stop = true;
        state.pause_requested = false;
        self.0.1.notify_all();
    }
    pub(crate) fn set_volume(&self, volume: f32) -> Result<(), String> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err("Głośność lektora musi być w zakresie 0–1".into());
        }
        self.state().status.volume = volume;
        Ok(())
    }
    pub fn is_busy(&self) -> bool {
        self.state().busy
    }

    fn reserve(&self, playback_key: Option<String>) -> Result<(), String> {
        if let Some(key) = &playback_key {
            validate_playback_key(key)?;
        }
        {
            let mut state = self.state();
            if state.busy || state.closing {
                return Err("Odtwarzacz jest zajęty lub aplikacja jest zamykana".into());
            }
            state.busy = true;
            state.stop = false;
            state.pause_requested = state
                .pending_pause
                .take()
                .is_some_and(|(key, paused)| Some(key) == playback_key && paused);
            state.playback_key = playback_key;
            state.status.playback_id += 1;
            state.status.error = None;
            state.status.output_device_id = None;
            state.status.phase = "preparing".into();
        }
        Ok(())
    }

    fn play(
        &self,
        app: tauri::AppHandle,
        request: PlayRequest,
        ducking: DuckingController,
    ) -> Result<Status, String> {
        self.phase(&app, "preparing");
        // All error paths after ducking go through restore, independent of React lifetime.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.execute(&app, request, ducking)
        }))
        .unwrap_or_else(|_| Err("Wewnętrzny błąd odtwarzacza".into()));
        let status = {
            let mut state = self.state();
            state.status.phase = match &result {
                Ok(true) => "stopped",
                Ok(false) => "ended",
                Err(_) => "error",
            }
            .into();
            state.status.error = result.as_ref().err().cloned();
            state.busy = false;
            state.pause_requested = false;
            self.0.1.notify_all();
            state.status.clone()
        };
        let _ = app.emit("audio-playback-status", &status);
        result.map(|_| status)
    }

    fn execute(
        &self,
        app: &tauri::AppHandle,
        request: PlayRequest,
        ducking: DuckingController,
    ) -> Result<bool, String> {
        let bytes = if request.resource_id == "test.wav" {
            test_wav()
        } else {
            let root = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("audio");
            read_resource(&root, &request.resource_id)?
        };
        let source = decode(bytes)?;
        let host = rodio::cpal::default_host();
        let device = if let Some(id) = request.output_device_id {
            host.output_devices()
                .map_err(|e| e.to_string())?
                .find(|device| {
                    device
                        .id()
                        .is_ok_and(|candidate| candidate.id() == id || candidate.to_string() == id)
                })
        } else {
            host.default_output_device()
        }
        .ok_or("Brak wskazanego urządzenia wyjściowego Windows")?;
        self.state().status.output_device_id = device.id().ok().map(|id| id.to_string());
        let (errors_tx, errors_rx) = mpsc::channel();
        // No fallback to an unrelated output device.
        let stream = DeviceSinkBuilder::from_device(device)
            .map_err(|e| e.to_string())?
            .with_error_callback(move |error| {
                let _ = errors_tx.send(error.to_string());
            })
            .open_stream()
            .map_err(|e| e.to_string())?;
        let player = Player::connect_new(stream.mixer());
        let restore = std::cell::RefCell::new(RestoreGuard {
            ducking,
            token: None,
        });
        let result = with_restore(
            || {
                if self.state().stop {
                    return Ok(true);
                }
                if let Ok(error) = errors_rx.try_recv() {
                    return Err(error);
                }
                player.set_volume(self.get_status().volume);
                // Append once and keep the same source/position across pauses.
                player.pause();
                player.append(source);
                loop {
                    // This interval only checks the queue, stop and stream errors; it is not a duration timeout.
                    if let Ok(error) = errors_rx.try_recv() {
                        player.stop();
                        return Err(error);
                    }
                    if self.state().stop {
                        player.stop();
                        return Ok(true);
                    }
                    player.set_volume(self.get_status().volume);
                    if player.empty() {
                        return Ok(false);
                    }
                    if self.state().pause_requested {
                        player.pause();
                        if self.get_status().phase != "paused" {
                            let mut restore = restore.borrow_mut();
                            if restore.token.is_some() {
                                self.phase(app, "restoring");
                            }
                            restore.finish()?;
                            // Ack only after the original music volume is restored.
                            self.phase(app, "paused");
                        }
                    } else if player.is_paused() {
                        if let Some(session_id) = &request.music_session_id {
                            self.phase(app, "ducking");
                            let mut restore = restore.borrow_mut();
                            restore.token =
                                Some(restore.ducking.duck(session_id.clone())?.restoration_token);
                        }
                        // Stop/pause may have arrived during the native fade.
                        let interrupted = {
                            let state = self.state();
                            state.stop || state.pause_requested
                        };
                        if interrupted {
                            continue;
                        }
                        player.play();
                        self.phase(app, "playing");
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            },
            || {
                let mut restore = restore.borrow_mut();
                if restore.token.is_some() {
                    self.phase(app, "restoring");
                }
                restore.finish()
            },
        );
        // Keep the output stream alive through restoration as well.
        drop(player);
        drop(stream);
        result
    }
}

struct RestoreGuard {
    ducking: DuckingController,
    token: Option<String>,
}
impl RestoreGuard {
    fn finish(&mut self) -> Result<(), String> {
        if let Some(token) = &self.token {
            // Keep the token on error so the final cleanup/Drop can retry.
            self.ducking.restore(token.clone())?;
            self.token = None;
        }
        Ok(())
    }
}
impl Drop for RestoreGuard {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("Playback cleanup failed: {error}");
        }
    }
}

fn with_restore<T>(
    play: impl FnOnce() -> Result<T, String>,
    restore: impl FnOnce() -> Result<(), String>,
) -> Result<T, String> {
    let playback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(play))
        .unwrap_or_else(|_| Err("Wewnętrzny błąd odtwarzania".into()));
    combine(playback, restore())
}

fn combine<T>(playback: Result<T, String>, restore: Result<(), String>) -> Result<T, String> {
    match (playback, restore) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!("Przywracanie muzyki: {error}")),
        (Err(error), Err(restore)) => Err(format!("{error}; przywracanie muzyki: {restore}")),
    }
}

fn read_resource(root: &Path, id: &str) -> Result<Vec<u8>, String> {
    if id.is_empty()
        || id.contains(['/', '\\', ':'])
        || !["wav", "mp3"].contains(
            &Path::new(id)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
        )
    {
        return Err("Dozwolone są lokalne zasoby WAV i MP3".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = root.join(id).canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err("Zasób poza katalogiem audio".into());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Nagranie przekracza 64 MiB".into());
    }
    Ok(bytes)
}

fn decode(bytes: Vec<u8>) -> Result<SamplesBuffer, String> {
    let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let channels = decoder.channels();
    let rate = decoder.sample_rate();
    let samples: Vec<_> = decoder.take(32_000_001).collect();
    if samples.is_empty() || samples.len() > 32_000_000 {
        return Err("Nagranie puste lub zbyt długie".into());
    }
    Ok(SamplesBuffer::new(channels, rate, samples))
}

// A self-contained original PCM recording (two-tone chime), not generated speech.
fn test_wav() -> Vec<u8> {
    let rate = 24_000u32;
    let samples = rate * 2;
    let size = samples * 2;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * 2).to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for index in 0..samples {
        let time = index as f32 / rate as f32;
        let envelope = (time / 0.03).min(1.0) * ((2.0 - time) / 0.15).clamp(0.0, 1.0);
        let frequency = if time < 1.0 { 440.0 } else { 660.0 };
        let sample = ((time * frequency * std::f32::consts::TAU).sin() * envelope * 6000.0) as i16;
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRequest {
    resource_id: String,
    music_session_id: Option<String>,
    output_device_id: Option<String>,
    playback_key: Option<String>,
}
fn validate_playback_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.len() > 128 {
        return Err("Nieprawidłowy identyfikator odtwarzania".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn audio_play(
    app: tauri::AppHandle,
    player: tauri::State<'_, AudioPlayer>,
    ducking: tauri::State<'_, DuckingController>,
    request: PlayRequest,
) -> Result<Status, String> {
    crate::background::ensure_running(&app)?;
    player.reserve(request.playback_key.clone())?;
    let player = player.inner().clone();
    let ducking = ducking.inner().clone();
    tauri::async_runtime::spawn_blocking(move || player.play(app, request, ducking))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn audio_stop(player: tauri::State<'_, AudioPlayer>) {
    player.stop();
}
#[tauri::command]
pub async fn audio_set_paused(
    player: tauri::State<'_, AudioPlayer>,
    paused: bool,
    playback_key: String,
) -> Result<Status, String> {
    let player = player.inner().clone();
    tauri::async_runtime::spawn_blocking(move || player.set_paused(paused, playback_key))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn audio_get_status(player: tauri::State<'_, AudioPlayer>) -> Status {
    player.get_status()
}
#[tauri::command]
pub fn audio_set_volume(player: tauri::State<'_, AudioPlayer>, volume: f32) -> Result<(), String> {
    player.set_volume(volume)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_recording_decodes_and_finishes() {
        let source = decode(test_wav()).unwrap();
        assert_eq!(source.count(), 48_000);
    }
    #[test]
    fn invalid_recording_is_rejected() {
        assert!(decode(vec![1, 2, 3]).is_err());
    }
    #[test]
    fn restoration_failure_is_not_hidden() {
        assert_eq!(combine(Ok(false), Ok(())), Ok(false));
        assert!(combine(Ok(false), Err("restore".into())).is_err());
        let error = combine::<()>(Err("play".into()), Err("restore".into())).unwrap_err();
        assert!(error.contains("play") && error.contains("restore"));
    }
    #[test]
    fn independent_volume_is_validated() {
        let player = AudioPlayer::new();
        assert!(player.set_volume(f32::NAN).is_err());
        assert!(player.set_volume(1.1).is_err());
        player.set_volume(0.4).unwrap();
        assert_eq!(player.get_status().volume, 0.4);
    }
    #[test]
    fn mp3_silent_frames_decode() {
        // Original synthetic MPEG1 layer III silent frames, no external recording.
        let mut bytes = Vec::new();
        for _ in 0..10 {
            bytes.extend([0xff, 0xfb, 0x90, 0x00]);
            bytes.extend([0u8; 413]);
        }
        assert!(decode(bytes).unwrap().count() > 0);
    }
    #[test]
    fn restore_runs_after_completion_stop_and_error() {
        for playback in [Ok(false), Ok(true), Err("device disconnected".into())] {
            let events = std::cell::RefCell::new(Vec::new());
            let _ = with_restore(
                || {
                    events.borrow_mut().push("play finished");
                    playback
                },
                || {
                    events.borrow_mut().push("restore");
                    Ok(())
                },
            );
            assert_eq!(*events.borrow(), ["play finished", "restore"]);
        }
    }
    #[test]
    fn cannot_start_two_playbacks_or_play_after_shutdown() {
        let player = AudioPlayer::new();
        player.reserve(None).unwrap();
        assert!(player.reserve(None).is_err());
        player.stop();
        assert!(player.state().stop);
        player.begin_shutdown();
        assert!(player.reserve(None).is_err());
        player.state().busy = false;
        player.shutdown();
        assert!(player.reserve(None).is_err());
    }

    #[test]
    fn paused_queue_retains_every_sample_and_resumes_without_restarting() {
        // Drive rodio directly, without a Windows output device or audible sound.
        let expected: Vec<f32> = (1..=48_000).map(|i| i as f32 / 48_000.0).collect();
        let (player, mut output) = Player::new();
        player.append(SamplesBuffer::new(
            1.try_into().unwrap(),
            24_000.try_into().unwrap(),
            expected.clone(),
        ));
        let mut heard = Vec::new();
        for _ in 0..3 {
            for _ in 0..720 {
                if let Some(sample) = output.next().filter(|sample| *sample != 0.0) {
                    heard.push(sample);
                }
            }
            player.pause();
            // Allow rodio's 5 ms control interval, then require stable silence.
            for _ in 0..24_000 {
                if let Some(sample) = output.next().filter(|sample| *sample != 0.0) {
                    heard.push(sample);
                }
            }
            for _ in 0..1_000 {
                assert_eq!(output.next(), Some(0.0));
            }
            assert!(!player.empty());
            player.play();
        }
        for _ in 0..60_000 {
            if player.empty() {
                break;
            }
            if let Some(sample) = output.next().filter(|sample| *sample != 0.0) {
                heard.push(sample);
            }
        }
        assert!(player.empty());
        assert_eq!(heard, expected);
    }

    #[test]
    fn pause_ack_waits_for_restoration_and_resume_ack_waits_for_ducking() {
        let player = AudioPlayer::new();
        player.reserve(Some("current".into())).unwrap();
        for (paused, intermediate, completed) in
            [(true, "restoring", "paused"), (false, "ducking", "playing")]
        {
            let (tx, rx) = mpsc::channel();
            let worker = player.clone();
            let task = std::thread::spawn(move || {
                tx.send(worker.set_paused(paused, "current".into()))
                    .unwrap();
            });
            {
                let mut state = player.state();
                while state.pause_requested != paused {
                    state = player.0.1.wait(state).unwrap();
                }
                state.status.phase = intermediate.into();
                player.0.1.notify_all();
            }
            assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
            player.state().status.phase = completed.into();
            player.0.1.notify_all();
            assert_eq!(
                rx.recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .unwrap()
                    .phase,
                completed
            );
            task.join().unwrap();
        }
    }

    #[test]
    fn pause_handles_ipc_start_gap_and_rejects_stale_playback_commands() {
        let player = AudioPlayer::new();
        player.set_paused(true, "first".into()).unwrap();
        player.reserve(Some("first".into())).unwrap();
        assert!(player.state().pause_requested);
        {
            let mut state = player.state();
            state.busy = false;
            state.pause_requested = false;
            state.status.phase = "ended".into();
        }
        player.set_paused(true, "first".into()).unwrap();
        assert!(!player.state().pause_requested);
        player.reserve(Some("second".into())).unwrap();
        assert!(!player.state().pause_requested);
        assert!(player.set_paused(true, "first".into()).is_err());
        assert!(!player.state().pause_requested);
        assert!(player.set_paused(true, "".into()).is_err());
    }

    #[test]
    fn native_error_and_shutdown_release_pending_pause_without_a_resume() {
        for shutdown in [false, true] {
            let player = AudioPlayer::new();
            player.reserve(Some("current".into())).unwrap();
            let (tx, rx) = mpsc::channel();
            let worker = player.clone();
            let task = std::thread::spawn(move || {
                tx.send(worker.set_paused(true, "current".into())).unwrap();
            });
            {
                let mut state = player.state();
                while !state.pause_requested {
                    state = player.0.1.wait(state).unwrap();
                }
            }
            if shutdown {
                player.begin_shutdown();
            } else {
                let mut state = player.state();
                state.busy = false;
                state.status.phase = "error".into();
                state.status.error = Some("output disconnected".into());
                player.0.1.notify_all();
            }
            assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap().is_err());
            task.join().unwrap();
        }
    }
}
