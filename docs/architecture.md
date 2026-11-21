# Architecture

[Project overview](../README.md) · [Development](development.md)

AraBrief is a Windows desktop application built with Tauri 2, React, TypeScript, Vite, Tailwind CSS, and Rust. There is no project-hosted server backend. Rust calls news publishers and OpenAI directly; React interacts with native services through Tauri IPC.

## Layout and dependency boundaries

| Location | Responsibility |
| --- | --- |
| `src/app/` | Views, application composition, localization, notifications, and UI state |
| `src/domain/` | Shared domain models, language rules, and the country catalog |
| `src/modules/` | Provider interfaces, scheduler logic, and feature components |
| `src/platform/tauri/` | Native adapters, response validation, and IPC contracts |
| `src/styles/`, `src/assets/brand/` | Design tokens and original brand assets |
| `src-tauri/src/` | Windows audio, playback, news, generation, persistence, credentials, and lifecycle |
| `src-tauri/migrations/` | Ordered SQLite migrations |
| `src-tauri/capabilities/`, `src-tauri/tauri.conf.json` | Tauri permissions and application/bundle configuration |
| `tests/` | Node tests for orchestration, UI behavior, localization, and release validation |
| `scripts/` | Release-version validation |
| `.github/workflows/` | Windows installer release automation |

The UI depends on domain services and ports. `useAraBrief` composes UI state and adapters. Windows APIs stay in Rust. Provider contracts include `AudioController`, `AudioPlayer`, `NewsProvider`, `ContentGenerator`, `SpeechProvider`, `SettingsRepository`, and `CredentialStore`.

Scheduler orchestration lives outside React in `src/modules/scheduler/`. `ManualBroadcastRunner` coordinates one preparation/listening flow; `AutomaticBroadcastScheduler` uses it for timed cycles. `BroadcastScheduler`, `BackgroundLifecycle`, and `PreparationRecovery` define supporting orchestration and lifecycle boundaries.

The Rust composition root is [lib.rs](../src-tauri/src/lib.rs). It opens the repository, initializes audio recovery, applies settings, constructs native services, registers commands, and sets up the tray and shutdown handling.

## Broadcast lifecycle

```text
topics and news country
  → local article queue / Google News RSS
  → native article retrieval and extraction
  → independent AI-written chapters
  → speech segments and completed WAV
  → wait for scheduled time
  → lower selected music session → play narration → restore volume
```

Preparation finishes before music is lowered. Native preparation, synthesis, and playback guards prevent overlapping work; only one briefing plays at a time. The first scheduled playback waits a full interval. Each completed playback and restoration starts the next interval. A missed deadline after sleep or audio that is not ready causes the scheduled playback to be skipped.

Pause preserves the countdown or native narration queue. Pausing narration restores music, and resuming captures a fresh volume snapshot before ducking again. Pause state is process-local. Stop cancels work and waits for native cleanup; blocking network calls may finish before cancellation is acknowledged.

The [broadcast pipeline](broadcast-pipeline.md) owns the detailed discovery, generation, caching, and recovery rules. [Native integration](native-integration.md) owns audio behavior and the IPC reference.

## Persistence and recovery

SQLite stores settings, topics, source metadata, article material, generated chapters, audio metadata and segment caches, playback receipts, and the audio-recovery journal. Schema files are append-only migrations; the current sequence runs through `018_archive_selection.sql`. The settings DTO's `version: 1` is separate from the database schema version.

Settings are validated in Rust and saved as a full snapshot inside a transaction. Windows autostart registration and SQLite cannot form a single atomic transaction; the native settings service attempts to compensate if persistence fails after registration changes.

Playback receipts distinguish attempted from completed playback. An attempted receipt protects against replaying a partly heard briefing after a crash; history records the articles included in completed playback. The UI's last-played card describes actual playback in the current UI session, rather than presenting a full historical archive.

The music-volume snapshot belongs to Rust and is journaled before native volume writes. Recovery runs at startup before another ducking operation. Restoration is idempotent and respects a user's independent mixer changes. See [audio restoration](native-integration.md#audio-restoration).

API keys are separate from SQLite. Data paths, external disclosures, and reset instructions are maintained in [Privacy](privacy.md).

## Language and news edition

`uiLanguage` is `auto`, `pl`, or `en`. Rust resolves Automatic from the Windows user interface language; Polish resolves to `pl`, other languages to `en`. The DTO's `language` is the effective narration language, not a separate editable narration preference.

The saved `newsRegion` is a supported country code, independent of the interface language. The shared [newsCountries.json](../src/domain/newsCountries.json) catalog supplies Google News edition parameters to TypeScript and Rust. New installations initialize Poland for Polish Windows and the United States otherwise.

`src/app/catalog.json` holds interface copy. `i18n.tsx` supplies React translation hooks; localization maps native diagnostic messages for display. Publisher content and user-entered topics remain unchanged in the UI. Narration prompts request the effective application language while evidence quotes remain in the original source language.

Language and region participate in preparation and audio compatibility keys. Changing either invalidates incompatible previews; historical data remains in the database.

## Tray, background operation, and exit

The app is single-instance. Closing its window hides it; the webview and native services remain alive. Tray controls open the app, start or stop the scheduler, and request a real exit. Native lifecycle events keep the scheduler informed while the window is hidden. Computer sleep still suspends execution.

Windows autostart and scheduler-on-launch are independent, disabled by default. Autostart uses `--autostart` to open in the tray. Scheduler-on-launch is a separate consent to automatic preparation and API use.

Exit blocks new preparation and playback, stops narration, cancels scheduler/speech work, and restores music in Rust. Native cleanup does not depend on the UI responding. The UI shutdown acknowledgement has a bounded wait after audio cleanup. Forced process termination cannot run exit handlers; the persisted journal supports a later recovery attempt.

## Interface behavior

Views cover Studio, Interests, and Settings. The interface provides explicit unavailable states when IPC or native resources are missing. Notifications use a shared portal with keyboard access and reduced-motion support; success/info messages expire, while errors/warnings require dismissal. Topics save immediately, while Settings exposes an explicit save action.

The existing [UI design notes](ui-design.md) describe tokens, responsive layout, and accessibility decisions. Measurement behavior and historical benchmarks are maintained separately in [Performance](performance.md).
