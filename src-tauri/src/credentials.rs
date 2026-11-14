// No credential value is accepted from or returned to the webview.
#[cfg(windows)]
mod native {
    use windows::{
        Win32::{
            Foundation::{ERROR_CANCELLED, ERROR_NOT_FOUND, ERROR_SUCCESS, HWND},
            Security::Credentials::*,
        },
        core::{HRESULT, PWSTR, w},
    };
    use zeroize::{Zeroize, Zeroizing};
    const TARGET: windows::core::PCWSTR = w!("com.janus.ara-brief/openai-api-key");
    struct Credential(*mut CREDENTIALW);
    impl Drop for Credential {
        fn drop(&mut self) {
            unsafe {
                let credential = &mut *self.0;
                if !credential.CredentialBlob.is_null() {
                    std::slice::from_raw_parts_mut(
                        credential.CredentialBlob,
                        credential.CredentialBlobSize as usize,
                    )
                    .zeroize();
                }
                CredFree(self.0.cast());
            }
        }
    }
    fn missing(error: &windows::core::Error) -> bool {
        error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0)
    }
    pub fn exists() -> Result<bool, String> {
        let mut ptr = std::ptr::null_mut();
        match unsafe { CredReadW(TARGET, CRED_TYPE_GENERIC, None, &mut ptr) } {
            Ok(()) => {
                let credential = Credential(ptr);
                Ok(unsafe { (*credential.0).CredentialBlobSize > 0 })
            }
            Err(error) if missing(&error) => Ok(false),
            Err(_) => Err("Nie można odczytać Windows Credential Manager".into()),
        }
    }
    pub fn delete() -> Result<(), String> {
        match unsafe { CredDeleteW(TARGET, CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if missing(&error) => Ok(()),
            Err(_) => Err("Nie można usunąć klucza z Windows Credential Manager".into()),
        }
    }
    pub fn prompt_and_save(hwnd: isize) -> Result<bool, String> {
        let mut username = Zeroizing::new(vec![0u16; CREDUI_MAX_USERNAME_LENGTH as usize + 1]);
        username[..6].copy_from_slice(&[79, 112, 101, 110, 65, 73]);
        let mut secret = Zeroizing::new(vec![0u16; 1024]);
        let info = CREDUI_INFOW {
            cbSize: std::mem::size_of::<CREDUI_INFOW>() as u32,
            hwndParent: HWND(hwnd as *mut _),
            pszCaptionText: w!("AraBrief — klucz OpenAI"),
            pszMessageText: w!(
                "W polu hasła wpisz klucz OpenAI API. Nazwę użytkownika pozostaw bez zmian. Klucz zostanie zapisany wyłącznie w Windows Credential Manager."
            ),
            ..Default::default()
        };
        let result = unsafe {
            CredUIPromptForCredentialsW(
                Some(&info),
                TARGET,
                None,
                0,
                &mut username,
                &mut secret,
                None,
                CREDUI_FLAGS_GENERIC_CREDENTIALS
                    | CREDUI_FLAGS_ALWAYS_SHOW_UI
                    | CREDUI_FLAGS_DO_NOT_PERSIST
                    | CREDUI_FLAGS_KEEP_USERNAME
                    | CREDUI_FLAGS_EXCLUDE_CERTIFICATES,
            )
        };
        if result == ERROR_CANCELLED {
            return Ok(false);
        }
        if result != ERROR_SUCCESS {
            return Err("Nie można otworzyć natywnego okna zapisu klucza".into());
        }
        let length = secret.iter().position(|c| *c == 0).unwrap_or(secret.len());
        // Only local structural validation; never claim the key was authenticated.
        if length == 0 || secret[..length].iter().any(|c| *c <= 32 || *c > 126) {
            return Err("Klucz nie może być pusty ani zawierać białych znaków".into());
        }
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(TARGET.0 as *mut _),
            CredentialBlobSize: (length * 2) as u32,
            CredentialBlob: secret.as_mut_ptr().cast(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(w!("OpenAI").0 as *mut _),
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }
            .map_err(|_| "Nie można zapisać klucza w Windows Credential Manager".to_string())?;
        Ok(true)
    }
}

#[tauri::command]
pub async fn credentials_save(window: tauri::Window) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let hwnd = window.hwnd().map_err(|_| "Brak okna aplikacji")?.0 as isize;
        tauri::async_runtime::spawn_blocking(move || native::prompt_and_save(hwnd))
            .await
            .map_err(|_| "Błąd natywnego magazynu".to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Err("Windows Credential Manager wymaga Windows".into())
    }
}
#[tauri::command]
pub async fn credentials_check() -> Result<bool, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(native::exists)
            .await
            .map_err(|_| "Błąd natywnego magazynu".to_string())?
    }
    #[cfg(not(windows))]
    {
        Err("Windows Credential Manager wymaga Windows".into())
    }
}
#[tauri::command]
pub async fn credentials_delete() -> Result<(), String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(native::delete)
            .await
            .map_err(|_| "Błąd natywnego magazynu".to_string())?
    }
    #[cfg(not(windows))]
    {
        Err("Windows Credential Manager wymaga Windows".into())
    }
}
