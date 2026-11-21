# Changelog

This file describes published AraBrief releases. Changes on the default branch after a release are not retroactively part of that release.

## [0.1.0] - 2026-11-20

Initial published Windows desktop MVP. GitHub records publication at `2026-11-20T23:57:01Z` (November 21 in Europe/Warsaw).

### Included

- Windows x64, per-user NSIS installer: `AraBrief_0.1.0_x64-setup.exe`, published through a version-tag-triggered GitHub Actions workflow.
- Topic-based Google News RSS discovery, an independent news country picker, native article retrieval, and persistent article queues.
- Adaptive source selection across the last 7, 30, and 90 days when readable material is scarce, with persistent RSS backoff.
- Article-based scripts through OpenAI Responses API and narration through OpenAI Speech API, with complete-chapter publication and reusable text/audio caches.
- Polish and English interface and narration, including automatic interface-language selection from Windows.
- Scheduled briefings, manual preparation and listening, script/source previews, and separate narrator volume.
- Selected-session music ducking and restoration, without pausing music or changing the global device volume.
- Briefing/countdown pause and resume, Stop, tray operation, single-instance behavior, and separate opt-in application/scheduler startup settings.
- SQLite settings and history, preparation/playback receipts, and native volume-recovery journaling.
- Native Windows API-key entry and storage in Windows Credential Manager, with a model-list connection check.

### Release limitations

- Requires Windows 10/11 x64, WebView2, internet access, and the user's OpenAI API access/billing. AI preparation can incur charges even without playback.
- Installer is unsigned. Source availability and API limits can shorten or prevent a briefing. AI output is not independently fact-checked.
- Speech uses `gpt-4o-mini-tts`; current provider availability is documented separately in [Troubleshooting](docs/troubleshooting.md#speech-model-availability).

This entry is based on the existing GitHub release and the code at tag `v0.1.0`, commit `e2511b29803cfe870aa15c6c1946c0a56e95f5b5`. It expands the brief original release description; it is not evidence of additional installer testing. Subsequent README and repository-documentation work is excluded from this release entry.

[0.1.0]: https://github.com/apjanusz/ara-brief/releases/tag/v0.1.0
