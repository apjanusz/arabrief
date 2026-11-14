use super::*;
use crate::audio::{
    ducking::{Command, DuckResult},
    fade::{self, Outcome, Volume},
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

fn open(id: &str) -> std::result::Result<NativeVolume, String> {
    let control = unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let devices = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|e| e.to_string())?;
        let mut found = None;
        for i in 0..devices.GetCount().map_err(|e| e.to_string())? {
            let candidate = (|| -> Result<Option<IAudioSessionControl>> {
                let device = devices.Item(i)?;
                let device_id = take_string(device.GetId()?)?;
                let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
                let sessions = manager.GetSessionEnumerator()?;
                for j in 0..sessions.GetCount()? {
                    let control = sessions.GetSession(j)?;
                    if let Ok(Some(dto)) = read_session(&control, &device_id)
                        && dto.id == id
                    {
                        return Ok(Some(control));
                    }
                }
                Ok(None)
            })();
            if let Ok(Some(control)) = candidate {
                found = Some(control);
                break;
            }
        }
        found.ok_or("Session no longer available; refresh and select again")?
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
    Ok(NativeVolume {
        control,
        volume,
        events,
        changed,
        disconnected,
    })
}

struct Active {
    token: String,
    volume: NativeVolume,
    original: f32,
    expected: f32,
}
impl Active {
    fn restore(&mut self) -> std::result::Result<Outcome, String> {
        fade::fade(
            &self.volume,
            &mut self.expected,
            self.original,
            fade::FADE_DURATION,
        )
    }
}

pub(in crate::audio) fn run(receiver: mpsc::Receiver<Command>) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() };
    let _apartment = initialized.as_ref().ok().map(|_| Apartment);
    let mut active: Option<Active> = None;
    let mut generation = 0u64;
    for command in receiver {
        match command {
            Command::Duck(id, reply) => {
                let result = (|| -> std::result::Result<DuckResult, String> {
                    initialized.as_ref().map_err(|e| e.to_string())?;
                    if active.is_some() {
                        return Err("A ducking cycle is already active; restore it first".into());
                    }
                    let volume = open(&id)?;
                    let original = volume.get()?;
                    generation += 1;
                    let token = format!("duck-{generation}");
                    // Store snapshot before the first write, even if the fade fails.
                    active = Some(Active {
                        token: token.clone(),
                        volume,
                        original,
                        expected: original,
                    });
                    let current = active.as_mut().unwrap();
                    match fade::fade(
                        &current.volume,
                        &mut current.expected,
                        original * fade::DUCK_RATIO,
                        fade::FADE_DURATION,
                    ) {
                        Ok(outcome) => Ok(DuckResult {
                            restoration_token: token,
                            outcome,
                        }),
                        Err(error) => {
                            // Best effort rollback. Keep snapshot if rollback fails for retry at shutdown.
                            if current.restore().is_ok() {
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
                {
                    active = None;
                }
            }
            Command::Restore(token, reply) => {
                let result = match active.as_mut() {
                    Some(current) if token.is_empty() || current.token == token => {
                        current.restore()
                    }
                    Some(_) => Err("Invalid restoration token".into()),
                    None => Ok(Outcome::Completed),
                };
                if result.is_ok() {
                    active = None;
                }
                let _ = reply.send(result);
            }
            Command::Shutdown(reply) => {
                let result = active
                    .as_mut()
                    .map(|current| current.restore().map(|_| ()))
                    .unwrap_or(Ok(()));
                // Drop subscription and interfaces before acknowledging shutdown.
                active = None;
                let _ = reply.send(result);
                break;
            }
        }
    }
    // All COM objects must be released before CoUninitialize, including on channel disconnect.
    if let Some(mut current) = active.take() {
        let _ = current.restore();
    }
}
