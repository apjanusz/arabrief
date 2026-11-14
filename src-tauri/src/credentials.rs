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
    pub fn read_key() -> Result<Zeroizing<String>, String> {
        let mut ptr = std::ptr::null_mut();
        unsafe { CredReadW(TARGET, CRED_TYPE_GENERIC, None, &mut ptr) }.map_err(|_| {
            "Brak dostępnego klucza OpenAI w Windows Credential Manager".to_string()
        })?;
        let credential = Credential(ptr);
        let blob = unsafe { &*credential.0 };
        if blob.CredentialBlob.is_null()
            || blob.CredentialBlobSize == 0
            || blob.CredentialBlobSize % 2 != 0
            || blob.CredentialBlobSize > 2560
        {
            return Err("Nieprawidłowy wpis klucza OpenAI".into());
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(blob.CredentialBlob, blob.CredentialBlobSize as usize)
        };
        let utf16 = Zeroizing::new(
            bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        );
        let key = Zeroizing::new(
            String::from_utf16(&utf16).map_err(|_| "Nieprawidłowy wpis klucza OpenAI")?,
        );
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("Nieprawidłowy wpis klucza OpenAI".into());
        }
        Ok(key)
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

// Native-only: deliberately not a Tauri command and never serialized.
pub(crate) fn read_openai_key() -> Result<zeroize::Zeroizing<String>, String> {
    #[cfg(windows)]
    {
        native::read_key()
    }
    #[cfg(not(windows))]
    {
        Err("Klucz OpenAI wymaga Windows Credential Manager".into())
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

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValidation {
    status: &'static str,
    message: String,
    text_model: String,
    text_model_listed: Option<bool>,
    realtime_model_listed: Option<bool>,
    diagnostic: Option<crate::openai_errors::ApiError>,
}

/// Read-only metadata probe. Does not generate tokens or determine credit balance.
pub(crate) fn validate_saved_key(text_model: String) -> Result<KeyValidation, String> {
    #[cfg(windows)]
    if !native::exists()? {
        return Ok(KeyValidation {
            status: "not_configured",
            message: "Brak klucza w Windows Credential Manager.".into(),
            text_model,
            text_model_listed: None,
            realtime_model_listed: None,
            diagnostic: None,
        });
    }
    let key = read_openai_key()?;
    let bearer = zeroize::Zeroizing::new(format!("Bearer {}", key.as_str()));
    let mut auth = reqwest::header::HeaderValue::from_str(&bearer)
        .map_err(|_| "Nieprawidłowy wpis klucza OpenAI")?;
    auth.set_sensitive(true);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .connect_timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Nie można utworzyć klienta diagnostycznego OpenAI")?;
    let response = client
        .get("https://api.openai.com/v1/models")
        .header(reqwest::header::AUTHORIZATION, auth)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "Test klucza: timeout 15 s; nie potwierdzono ważności klucza."
            } else {
                "Test klucza: błąd sieci/TLS; nie uzyskano odpowiedzi OpenAI."
            }
        })?;
    if !response.status().is_success() {
        let diagnostic = crate::openai_errors::response_error(response);
        return Ok(KeyValidation {
            status: if diagnostic.category == "invalid_key" {
                "rejected"
            } else {
                "inconclusive"
            },
            message: diagnostic.user_message(),
            text_model,
            text_model_listed: None,
            realtime_model_listed: None,
            diagnostic: Some(diagnostic),
        });
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Nie można odczytać metadanych OpenAI")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Odpowiedź testu klucza przekracza limit".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Nieprawidłowa odpowiedź testu klucza")?;
    let data = value["data"]
        .as_array()
        .ok_or("Brak listy modeli w odpowiedzi testu klucza")?;
    let listed = |model: &str| data.iter().any(|entry| entry["id"].as_str() == Some(model));
    let text_model_listed = Some(listed(&text_model));
    let realtime_model_listed = Some(listed("gpt-realtime-2.1-mini"));
    Ok(KeyValidation {status:"accepted",message:"OpenAI przyjęło klucz (HTTP 200 na GET /v1/models). Sprawdzenie nie generowało treści. To nie potwierdza salda API, limitów TPM/RPM ani uprawnień do generowania scenariusza lub audio.".into(),text_model,text_model_listed,realtime_model_listed,diagnostic:None})
}

#[tauri::command]
pub async fn credentials_validate(
    repository: tauri::State<'_, std::sync::Arc<crate::settings::SettingsRepository>>,
) -> Result<KeyValidation, String> {
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || validate_saved_key(repository.load()?.text_model))
        .await
        .map_err(|_| "Błąd testu klucza")?
}

#[cfg(all(test, windows))]
mod validation_tests {
    #[test]
    #[ignore = "explicit live credential probe; metadata only, no generation"]
    fn live_saved_key_metadata() {
        let database = std::path::PathBuf::from(std::env::var_os("APPDATA").expect("APPDATA"))
            .join("com.janus.ara-brief")
            .join("settings.sqlite3");
        let model = if database.exists() {
            let db = rusqlite::Connection::open_with_flags(
                database,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .expect("read-only settings");
            db.query_row("SELECT text_model FROM settings WHERE id=1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap_or_else(|_| "gpt-5-mini".into())
        } else {
            "gpt-5-mini".into()
        };
        match super::validate_saved_key(model) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string(&report).expect("safe diagnostic metadata")
            ),
            Err(error) => panic!("{error}"),
        }
    }
}
