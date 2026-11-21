# Contributing to AraBrief

Bug reports, focused proposals, and documentation corrections are welcome. Reports may be written in English or Polish.

## Report a bug

Search the repository's existing GitHub issues first. For a new report, include:

- AraBrief version or commit, Windows version, and whether this is an installed or development build.
- Minimal reproduction steps, expected behavior, and actual behavior.
- Relevant language/news country settings and audio-device/session conditions, if applicable.
- A short, sanitized error excerpt or screenshot, when useful.

Do not attach API keys, Credential Manager exports, local databases, real recordings, or identifying topic/history data. For a suspected vulnerability, follow [SECURITY.md](SECURITY.md) instead of posting technical details publicly.

## Propose a change

Open an issue describing the problem, intended behavior, and scope before starting a substantial change. Explain user impact and any new service, permission, dependency, API cost, or data flow. Avoid unrelated refactoring in a focused fix.

## Prepare a pull request

Read [AGENTS.md](AGENTS.md) and [Development](docs/development.md) for setup, commands, and project conventions. Keep native Windows operations out of UI/domain code, preserve music-restoration guarantees, and keep IPC contracts and migrations compatible. Include both Polish and English interface copy when changing user-visible text.

Describe what changed, why, and how it was checked. Run the checks relevant to the change, report checks that could not run, and update the affected documentation. Documentation-only changes do not need an installer build. Live API checks must be deliberate; use synthetic fixtures for automated tests.

## Licensing status

AraBrief is licensed under the [MIT License](LICENSE). Submit contributions only when you have the right to make them available under these terms. Preserve applicable third-party license and attribution notices, and identify any separately licensed code or assets in your proposal. Dependencies retain their own licenses; see the [licensing review](docs/licensing.md).
