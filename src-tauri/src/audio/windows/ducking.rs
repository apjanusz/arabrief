use super::*;
use crate::audio::{
    ducking::{Command, DuckResult},
    fade::{self, Outcome, Volume},
    journal::{self, Journal, JournaledVolume},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use windows::Win32::Media::Audio::{
    AudioSessionDisconnectReason, AudioSessionState, IAudioSessionEvents, IAudioSessionEvents_Impl,
};
use windows::core::{BOOL, GUID, PCWSTR, implement};

const CONTEXT: GUID = GUID::from_u128(0x2b89aa8a_2da4_4bc4_b897_c6e17ea7d834);

#[implement(IAudioSessionEvents)]
struct Events {
    changed: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>,
}
#[allow(non_snake_case)]
impl IAudioSessionEvents_Impl for Events_Impl {
    fn OnSimpleVolumeChanged(&self, _: f32, _: BOOL, context: *const GUID) -> Result<()> {
        if context.is_null() || unsafe { *context } != CONTEXT {
            self.changed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn OnSessionDisconnected(&self, _: AudioSessionDisconnectReason) -> Result<()> {
        self.disconnected.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn OnStateChanged(&self, state: AudioSessionState) -> Result<()> {
        if state == AudioSessionStateExpired {
            self.disconnected.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn OnDisplayNameChanged(&self, _: &PCWSTR, _: *const GUID) -> Result<()> {
        Ok(())
    }
    fn OnIconPathChanged(&self, _: &PCWSTR, _: *const GUID) -> Result<()> {
        Ok(())
    }
    fn OnChannelVolumeChanged(
        &self,
        _: u32,
        _: *const f32,
        _: u32,
        context: *const GUID,
    ) -> Result<()> {
        if context.is_null() || unsafe { *context } != CONTEXT {
            self.changed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn OnGroupingParamChanged(&self, _: *const GUID, _: *const GUID) -> Result<()> {
        Ok(())
    }
}

struct NativeVolume {
    control: IAudioSessionControl,
    volume: ISimpleAudioVolume,
    events: IAudioSessionEvents,
    changed: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>,
}
impl Drop for NativeVolume {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .control
                .UnregisterAudioSessionNotification(&self.events);
        }
    }
}
impl Volume for NativeVolume {
    fn get(&self) -> std::result::Result<f32, String> {
        if self.disconnected.load(Ordering::SeqCst) {
            return Err("Session disconnected; select the new session after refreshing".into());
        }
        unsafe {
            if self.control.GetState().map_err(|e| e.to_string())? == AudioSessionStateExpired {
                return Err("Session expired".into());
            }
            self.volume.GetMasterVolume().map_err(|e| e.to_string())
        }
    }
    fn set(&self, value: f32) -> std::result::Result<(), String> {
        unsafe {
            self.volume
                .SetMasterVolume(value, &CONTEXT)
                .map_err(|e| e.to_string())
        }
    }
    fn overridden(&self) -> bool {
        self.changed.load(Ordering::SeqCst)
    }
}

fn open(id: &str) -> std::result::Result<Option<NativeVolume>, String> {
    let control = unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let device_id = id
            .split_once('|')
            .ok_or("Invalid audio session identity")?
            .0;
        let wide: Vec<u16> = device_id.encode_utf16().chain(Some(0)).collect();
        let original_device = enumerator
            .GetDevice(PCWSTR(wide.as_ptr()))
            .map_err(|e| e.to_string())?;
        if original_device.GetState().map_err(|e| e.to_string())? != DEVICE_STATE_ACTIVE {
            return Err(
                "Wyjście muzyki jest niedostępne. Zapis głośności pozostaje do ponownej próby."
                    .into(),
            );
        }
        let manager: IAudioSessionManager2 = original_device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| e.to_string())?;
        let sessions = manager.GetSessionEnumerator().map_err(|e| e.to_string())?;
        let mut found = None;
        for index in 0..sessions.GetCount().map_err(|e| e.to_string())? {
            let control = sessions.GetSession(index).map_err(|e| e.to_string())?;
            if let Some(dto) = read_session(&control, device_id).map_err(|e| e.to_string())?
                && dto.id == id
            {
                found = Some(control);
                break;
            }
        }
        match found {
            Some(control) => control,
            None => return Ok(None),
        }
    };
    let changed = Arc::new(AtomicBool::new(false));
    let disconnected = Arc::new(AtomicBool::new(false));
    let events: IAudioSessionEvents = Events {
        changed: changed.clone(),
        disconnected: disconnected.clone(),
    }
    .into();
    let volume = control
        .cast::<ISimpleAudioVolume>()
        .map_err(|e| e.to_string())?;
    unsafe { control.RegisterAudioSessionNotification(&events) }.map_err(|e| e.to_string())?;
    Ok(Some(NativeVolume {
        control,
        volume,
        events,
        changed,
        disconnected,
    }))
}

struct Active {
    token: String,
    volume: JournaledVolume<NativeVolume>,
    original: f32,
    expected: f32,
    fade_in_ms: u32,
}
impl Active {
    fn restore(&mut self) -> std::result::Result<Outcome, String> {
        let result = fade::fade(
            &self.volume,
            &mut self.expected,
            self.original,
            std::time::Duration::from_millis(self.fade_in_ms as u64),
        );
        match result {
            Ok(outcome) => Ok(outcome),
            Err(error) => self
                .volume
                .restore_original()
                .map_err(|retry| format!("{error}; restore retry: {retry}")),
        }
    }
}

fn recover_saved(journal: &Journal) -> std::result::Result<Option<String>, String> {
    let Some(snapshot) = journal.load()? else {
        return Ok(None);
    };
    let Some(volume) = open(&snapshot.session_id)? else {
        // Instance identifiers cannot be applied to a restarted player/session.
        journal.clear()?;
        return Ok(Some("Poprzednia sesja muzyki już nie istnieje. Windows nie pozwala przywrócić jej głośności; nie zmieniono innych sesji.".into()));
    };
    let outcome = journal::recover(volume, journal, &snapshot)?;
    Ok(Some(
        match outcome {
            Outcome::Completed => {
                "Przywrócono głośność muzyki po nieoczekiwanym zakończeniu AraBrief."
            }
            Outcome::ManualOverride => {
                "Głośność zmieniła się po przerwaniu AraBrief. Zachowano aktualny poziom muzyki."
            }
        }
        .into(),
    ))
}

pub(in crate::audio) fn run(receiver: mpsc::Receiver<Command>) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() };
    let _apartment = initialized.as_ref().ok().map(|_| Apartment);
    let mut active: Option<Active> = None;
    let mut generation = 0u64;
    let mut journal: Option<Journal> = None;
    for command in receiver {
        match command {
            Command::Initialize(value, reply) => {
                let result = initialized
                    .as_ref()
                    .map_err(|e| e.to_string())
                    .and_then(|_| recover_saved(&value));
                journal = Some(value);
                let _ = reply.send(result);
            }
            Command::Duck(id, config, reply) => {
                let result = (|| -> std::result::Result<DuckResult, String> {
                    initialized.as_ref().map_err(|e| e.to_string())?;
                    if active.is_some() {
                        return Err("A ducking cycle is already active; restore it first".into());
                    }
                    let journal = journal.as_ref().ok_or("Audio recovery not initialized")?;
                    // Retry retained API failures before allowing any new ducking.
                    recover_saved(journal)?;
                    let volume = open(&id)?
                        .ok_or("Session no longer available; refresh and select again")?;
                    let original = volume.get()?;
                    let volume = JournaledVolume::new(volume, journal.clone(), id, original)?;
                    generation += 1;
                    let token = format!("duck-{generation}");
                    // Store snapshot before the first write, even if the fade fails.
                    active = Some(Active {
                        token: token.clone(),
                        volume,
                        original,
                        expected: original,
                        fade_in_ms: config.fade_in_ms,
                    });
                    let current = active.as_mut().unwrap();
                    match fade::fade(
                        &current.volume,
                        &mut current.expected,
                        original * config.ratio,
                        std::time::Duration::from_millis(config.fade_out_ms as u64),
                    ) {
                        Ok(outcome) => Ok(DuckResult {
                            restoration_token: token,
                            outcome,
                        }),
                        Err(error) => {
                            // Best effort rollback. Keep snapshot if rollback fails for retry at shutdown.
                            if current.restore().is_ok() && journal.clear().is_ok() {
                                active = None;
                            }
                            Err(error)
                        }
                    }
                })();
                // If UI disappeared while ducking, immediately restore the native snapshot.
                if reply.send(result).is_err()
                    && let Some(current) = active.as_mut()
                    && current.restore().is_ok()
                    && journal
                        .as_ref()
                        .is_some_and(|journal| journal.clear().is_ok())
                {
                    active = None;
                }
            }
            Command::Restore(token, reply) => {
                let mut result = match active.as_mut() {
                    Some(current) if token.is_empty() || current.token == token => {
                        current.restore()
                    }
                    Some(_) => Err("Invalid restoration token".into()),
                    None => match &journal {
                        Some(journal) => recover_saved(journal).map(|_| Outcome::Completed),
                        None => Ok(Outcome::Completed),
                    },
                };
                if result.is_ok()
                    && let Some(journal) = &journal
                {
                    result = journal.clear().map(|_| result.unwrap());
                }
                if result.is_ok() {
                    active = None;
                }
                let _ = reply.send(result);
            }
            Command::Shutdown(reply) => {
                let mut result = active
                    .as_mut()
                    .map(|current| current.restore().map(|_| ()))
                    .unwrap_or_else(|| match &journal {
                        Some(journal) => recover_saved(journal).map(|_| ()),
                        None => Ok(()),
                    });
                if result.is_ok()
                    && let Some(journal) = &journal
                {
                    result = journal.clear();
                }
                // Drop subscription and interfaces before acknowledging shutdown.
                active = None;
                let _ = reply.send(result);
                break;
            }
        }
    }
    // All COM objects must be released before CoUninitialize, including on channel disconnect.
    if let Some(mut current) = active.take()
        && current.restore().is_ok()
        && let Some(journal) = &journal
    {
        let _ = journal.clear();
    }
}
