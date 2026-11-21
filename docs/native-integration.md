# Native audio and IPC

[Architecture](architecture.md) · [Broadcast pipeline](broadcast-pipeline.md)

## Contracts

[src/platform/tauri/contracts.ts](../src/platform/tauri/contracts.ts) is the TypeScript command reference. Native adapters validate responses; Rust validates requests and uses camelCase DTO serialization. TypeScript types alone are not runtime validation. Keep the contract, adapter validation, and registered Rust handlers synchronized.

| Command group | Commands and role |
| --- | --- |
| System and lifecycle | `system_ui_language`, `background_connect`, `background_status`, `background_stopped` |
| Sessions and ducking | `audio_list_sessions`, `audio_duck`, `audio_restore` |
| Narration playback | `audio_play`, `audio_stop`, `audio_set_paused`, `audio_get_status`, `audio_set_volume` |
| Settings | `settings_load`, `settings_save` |
| Credentials | `credentials_save`, `credentials_check`, `credentials_validate`, `credentials_delete` |
| News and content | `news_fetch`, `news_mark_used`, `content_generate` |
| Speech | `speech_generate`, `speech_cancel` |
| Preparation receipts | `broadcast_restore`, `broadcast_claim`, `broadcast_complete` |

Session commands reject with `NativeError` (`code`, `message`); playback commands reject with a string. Content failures distinguish unavailable usable material from generation failures. Consult the contract for exact argument/result types instead of maintaining a second DTO definition in documentation.

## Windows session discovery

[audio/windows.rs](../src-tauri/src/audio/windows.rs) uses Windows Core Audio through windows-rs. It enumerates active render endpoints and their sessions, not just the default output. Results include device/session instance identity, PID, optional process name, display name, volume, and active/inactive state. System sounds, expired sessions, and AraBrief's own process are excluded.

PID is metadata, not a durable selection key. Snapshots can become stale immediately. A player restart or device switch requires selecting the new session; the app does not silently choose a similarly named process. An inaccessible endpoint may be skipped, producing a partial result; top-level enumeration failures are reported.

COM is initialized and released on the appropriate native threads. COM pointers and process handles never cross IPC; allocated strings and handles are released natively.

## Session ducking

Ducking uses `ISimpleAudioVolume` for the selected session only. It never changes the master device, another application's session, or music playback state. The configured factor multiplies the original level: at 15%, an initial 80% becomes 12%.

A dedicated MTA worker owns the snapshot, COM state, and session-event subscription. Only one ducking cycle is allowed, so another operation cannot overwrite the snapshot with an already lowered level. Own writes use an event-context GUID. Independent volume/mute/channel changes yield control to the user; later restoration respects that override.

`audio_restore` is idempotent after a completed cycle. An empty restoration token requests restoration of the current cycle for recovery paths. A snapshot is never transferred to a different PID, endpoint, or replacement session.

## Audio restoration

The `audio_recovery` journal records the original volume, endpoint/session identity, and before/after values before each Core Audio write, including fade steps. SQLite uses `synchronous=FULL` for the recovery journal. Startup recovery runs before a new ducking operation.

Success or a detected independent user change clears the relevant journal entry. A native API failure preserves recovery information for another attempt. A journal-write failure during restoration still triggers a direct attempt to restore the original level while respecting user changes.

Recovery can only act on the same available session instance whose current level matches an expected saved value. If that session expired, the journal can be cleared without changing a replacement. An inaccessible endpoint or unresolved failure can block new ducking until recovery succeeds.

Windows does not provide an atomic volume read/write relative to another mixer client. Forced process termination cannot execute shutdown code; recovery is attempted on the next startup and cannot reconstruct every mixer change made while the app was absent.

## Narration player

[player.rs](../src-tauri/src/player.rs) uses rodio with WAV/MP3 playback and the Windows audio stack. It decodes and opens the output before ducking. `audio_play` resolves after playback and restoration; terminal status distinguishes natural end, Stop, and error. Status is available through `audio-playback-status` events and polling.

Resources use a validated filename under the application audio directory, not an arbitrary path or remote URL. The built-in `test.wav` is a locally generated two-tone sound, not generated speech. Limits include a 64 MiB resource file and 32 million decoded samples.

By default, each playback opens the current Windows default output. A request can specify the selected music session's device for tests. There is no automatic fallback to other speakers after a device error. An output failure stops narration and initiates music restoration. The output stream stays alive through restoration.

Natural completion follows the drained rodio queue; a 20 ms polling period is not a recording-duration timeout. Hardware buffering can add latency beyond queue consumption. Narrator volume is a separate sample multiplier.

`audio_set_paused { paused, playbackKey }` targets the current playback instance. Pause keeps the same decoded queue and restores music; resume takes a fresh snapshot, ducks, and continues without a new generation or decode. Stale commands cannot control the next briefing. Stop and exit also release paused playback.

## Settings and credentials

`settings_save` accepts a full settings snapshot, validates it natively, and commits it transactionally. Unknown fields are rejected. Current validation includes a 5-minute to 24-hour interval, volume values in `[0,1]`, fades up to 30 seconds, a 30-second to 15-minute ordered duration preference, and at most 30 unique topics. These are configuration limits, not guarantees of generated duration.

The schema includes fields retained for compatibility; their existence does not imply a UI or active integration for each field. In particular, the active news provider is Google News RSS, not a general custom-feed management interface.

Key handling and data locations are documented in [Privacy](privacy.md). The online credential check uses `/v1/models`; it classifies authentication, permissions, billing/quota, rate-limit, and network failures without forwarding raw secret-bearing API messages. It does not generate text/audio or guarantee future model calls.
