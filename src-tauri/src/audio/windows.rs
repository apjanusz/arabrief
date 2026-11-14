use super::AudioSession;
pub(super) mod ducking;
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, S_OK, STILL_ACTIVE},
        Media::Audio::{
            AudioSessionStateActive, AudioSessionStateExpired, DEVICE_STATE_ACTIVE,
            IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
            IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator, eRender,
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
                CoUninitialize,
            },
            Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
    },
    core::{Interface, PWSTR, Result},
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // Balanced only after successful initialization (including S_FALSE).
        unsafe { CoUninitialize() };
    }
}

struct ProcessHandle(HANDLE);
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

// All strings returned by these Core Audio methods use the COM task allocator.
fn take_string(value: PWSTR) -> Result<String> {
    let result = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Ok(result?)
}

fn process_name(pid: u32) -> Option<String> {
    // A process may terminate or be protected. Never unwrap a PID lookup, cache a
    // handle, or ask for PROCESS_ALL_ACCESS. Keep the handle alive for the query.
    let handle =
        ProcessHandle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?);
    let mut exit_code = 0;
    unsafe { GetExitCodeProcess(handle.0, &mut exit_code) }.ok()?;
    if exit_code != STILL_ACTIVE.0 as u32 {
        return None;
    }
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    path.rsplit('\\').next().map(str::to_owned)
}

fn read_session(control: &IAudioSessionControl, device_id: &str) -> Result<Option<AudioSession>> {
    // SAFETY: interfaces are created, queried and released on this COM-initialized worker.
    unsafe {
        if control.GetState()? == AudioSessionStateExpired {
            return Ok(None);
        }
        let session: IAudioSessionControl2 = control.cast()?;
        if session.IsSystemSoundsSession() == S_OK {
            return Ok(None);
        }
        let pid = session.GetProcessId()?;
        if pid == 0 || pid == std::process::id() {
            return Ok(None);
        }
        let instance_id = take_string(session.GetSessionInstanceIdentifier()?)?;
        let session_identifier = take_string(session.GetSessionIdentifier()?)?;
        let process_name = process_name(pid);
        let name = take_string(control.GetDisplayName()?).unwrap_or_default();
        let display_name = if name.trim().is_empty() {
            process_name
                .clone()
                .unwrap_or_else(|| format!("Proces {pid} (nazwa niedostępna)"))
        } else {
            name
        };
        let volume: ISimpleAudioVolume = control.cast()?;
        let volume = volume.GetMasterVolume()?;
        // Re-check after process lookup: sessions can expire during enumeration.
        let state = control.GetState()?;
        if state == AudioSessionStateExpired {
            return Ok(None);
        }
        Ok(Some(AudioSession {
            id: format!("{device_id}|{instance_id}"),
            session_identifier,
            device_id: device_id.into(),
            pid,
            process_name,
            display_name,
            volume,
            state: if state == AudioSessionStateActive {
                "active"
            } else {
                "inactive"
            },
        }))
    }
}

pub fn list_sessions() -> Result<Vec<AudioSession>> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
    }
    let _apartment = Apartment;
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let devices = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let mut result = Vec::new();
        for index in 0..devices.GetCount()? {
            // Hot unplug and session termination are normal snapshot races.
            let device_result = (|| -> Result<()> {
                let device = devices.Item(index)?;
                let device_id = take_string(device.GetId()?)?;
                let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
                let sessions = manager.GetSessionEnumerator()?;
                for index in 0..sessions.GetCount()? {
                    if let Ok(control) = sessions.GetSession(index)
                        && let Ok(Some(session)) = read_session(&control, &device_id)
                    {
                        result.push(session);
                    }
                }
                Ok(())
            })();
            if let Err(error) = device_result {
                eprintln!("Audio endpoint unavailable during enumeration: {error}");
            }
        }
        result.sort_by(|a, b| a.display_name.cmp(&b.display_name).then(a.id.cmp(&b.id)));
        result.dedup_by(|a, b| a.id == b.id);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn enumerate_real_endpoints_and_serialize_ipc_snapshot() {
        let sessions = tauri::async_runtime::block_on(super::super::audio_list_sessions())
            .expect("Core Audio command failed");
        let mut ids = std::collections::HashSet::new();
        for session in &sessions {
            assert!(ids.insert(&session.id));
            assert_ne!(session.pid, std::process::id());
            assert!((0.0..=1.0).contains(&session.volume));
            assert!(!session.device_id.is_empty());
        }
        let json = serde_json::to_value(&sessions).unwrap();
        if let Some(first) = json.as_array().unwrap().first() {
            assert!(first.get("deviceId").is_some());
            assert!(first.get("processName").is_some());
        }
        println!("Core Audio smoke test: {} sessions", sessions.len());
    }
}
