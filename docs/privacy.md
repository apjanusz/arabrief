# Privacy and data flow

[Project overview](../README.md) · [Getting started](getting-started.md)

AraBrief stores application state on your Windows account and connects directly to external services. It has no project-hosted account service or server backend. The current application does not implement a separate analytics or telemetry upload service. News discovery and AI generation still require external network requests.

## What is stored locally

The application data directory is `%APPDATA%\com.janus.ara-brief`:

| Data | Storage and purpose |
| --- | --- |
| Settings and interests | `settings.sqlite3`: language, news country, topics, models, voice, timing, volume, and startup preferences |
| Music source | Selected session identity/name, plus recovery data needed to restore the original volume |
| News and source material | Article metadata, URLs, retrieved article text, queue state, and history in SQLite |
| Generated material | Scripts, chapter metadata, and reusable PCM segment data in SQLite |
| Finished narration | WAV recordings under the `audio` subdirectory, with metadata and hashes in SQLite |
| Webview state | WebView2 profile data in user directories managed by Tauri |

This database and the audio files are not application-encrypted. Windows account and filesystem permissions provide their local access boundary. Topics, history, and scripts can reveal personal interests; avoid sharing the data directory as a bug-report attachment. There is no user-facing retention-period setting or one-click full-data reset. Do not assume older material is automatically erased on a schedule.

Development and installed builds use the same application identifier. Under the same Windows account they can share data and credentials. The installer bundles application assets, prompts, and migrations, not the author's database, recordings, or API key.

## OpenAI API key

The key is entered in a native Windows dialog and stored as a generic Windows Credential Manager entry named `com.janus.ara-brief/openai-api-key`. It belongs to the current Windows user's credential store and persists across sign-ins.

The key does not pass through the HTML form, React state, or IPC arguments/results, and it is not saved to SQLite, `.env`, frontend variables, or the installer. Rust reads it when needed for native API calls. Credentials are sent to OpenAI as authentication for those requests, not to Google News or publishers. Rust uses sensitive authorization headers and zeroizing credential buffers.

The app exposes key save, presence check, online validation, and deletion operations. It provides no command to return the key to the webview. Canceling the native save dialog preserves an existing key. Presence alone does not confirm validity or billing status.

## External services

| Recipient | Data sent and reason |
| --- | --- |
| Google News | Topic search terms, edition/country parameters, date ranges, and article-link resolution requests |
| Publisher websites | Requested article URLs and ordinary HTTP/network metadata to retrieve readable article content |
| OpenAI Responses API | API authentication, configured model, prompts, source metadata/URLs, topic context, and retrieved article text to generate chapters |
| OpenAI Speech API | API authentication, narration text, voice, model, and language instructions to generate speech |
| OpenAI model-list endpoint | API authentication for the online connection check |
| GitHub | Release browsing and installer downloads initiated by the user |
| Microsoft | WebView2 bootstrapper/runtime installation when required; WebView2 itself is a Microsoft runtime |

Network recipients can observe connection metadata such as your IP address. The native article reader does not reuse your browser's cookies or signed-in publisher session. It does not bypass paywalls or access restrictions.

AraBrief sends text for speech synthesis, not microphone recordings or a recording of the music session. Windows audio-session enumeration and volume control are used locally; the app does not upload the session list as part of its AI requests.

Responses requests set `store: false`. This does **not** promise zero provider retention: OpenAI's abuse monitoring, caching, and account-specific data controls may still apply. Review the current [OpenAI API data controls](https://developers.openai.com/api/docs/guides/your-data). Google, publishers, GitHub, and Microsoft apply their own service policies.

## Diagnostics

Routine native timing, pipeline, and recovery logs are disabled by default, including during development. They can be enabled through `ARABRIEF_DIAGNOSTICS=1`; instructions are in [Performance](performance.md).

Diagnostics are designed to avoid API keys, authorization headers, full request/response bodies, narration, and raw audio. Identifiers, error messages, source-related metadata, and paths may still be sensitive. Important audio-restoration and lifecycle errors can be reported even when verbose diagnostics are off. No automatic diagnostic upload is implemented.

## Backups and removal

For a consistent backup, stop the scheduler and quit through the tray, then copy the application data directory while the app is closed. A live SQLite database can have companion WAL/journal files; copying only the main database while running can lose recent changes. A data backup does not include the Credential Manager key.

To reset your local installation deliberately:

1. Stop playback and scheduling; let music restoration finish. If recovering from a forced termination, first reopen AraBrief so it can attempt journal recovery.
2. Turn off Windows autostart if you no longer want the startup registration.
3. Delete the saved API key using Settings. Alternatively, remove the named entry from Windows Credential Manager.
4. Quit from the tray. Back up any material you want to keep, then remove the application data directory to clear settings, history, caches, and recordings.
5. Remove any remaining WebView2 profile data only after identifying the directories belonging to AraBrief.

Uninstalling the application is not a guarantee that these data or credentials are removed. Deleting a local key does not revoke it at OpenAI; revoke an exposed key in the API account as well. Local deletion does not erase data already processed by external services.
