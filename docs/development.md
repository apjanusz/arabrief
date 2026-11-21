# Development

[Project overview](../README.md) · [Architecture](architecture.md) · [Releases](releases.md)

## Environment

Development targets Windows x64. Install:

- Git.
- Node.js 24 LTS and npm, matching the release workflow. The documented minimum for the current tooling is Node.js 22.12; use Node.js 24 for the commands below, including TypeScript-stripping tests.
- Rust stable with the `x86_64-pc-windows-msvc` target; the Tauri plugins require Rust 1.90 or newer.
- Visual Studio Build Tools with C++ desktop tools and Windows SDK.
- Microsoft Edge WebView2 Runtime.

Make the tools available in the shell's `PATH`. Commands below run from the repository root.

## Run the application

```powershell
git clone https://github.com/apjanusz/arabrief.git
cd arabrief
npm ci
npm run tauri dev
```

Use the native credential dialog for an API key. Do not place keys in `.env`, `VITE_*`, source files, SQLite, or test fixtures. The app's native integrations need Windows; there is no plaintext credential fallback on other platforms.

For interface-only work:

```powershell
npm run dev
```

The browser preview has no Tauri IPC, Windows sessions, database access, or API integration. Empty and unavailable states are intentional. Use the desktop app to exercise real integrations.

## Checks

Choose checks appropriate to the change. Documentation-only changes can be checked through links, source references, and `git diff --check` without compiling the app.

Frontend compilation:

```powershell
npm run build
```

Domain, lifecycle, notification, localization, and release-validation tests use Node's built-in test runner:

```powershell
node --experimental-strip-types --test tests/*.test.mjs
```

Individual scripts also exist for manual broadcasts, scheduling, background lifecycle, and notifications:

```powershell
npm run test:manual
npm run test:scheduler
npm run test:background
npm run test:notifications
```

Rust checks:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
```

Default unit tests use fixtures and fake providers rather than paid OpenAI calls or real music-volume changes. Opt-in live reader checks and performance comparisons are documented in [the broadcast pipeline](broadcast-pipeline.md#live-reader-check) and [Performance](performance.md).

Automated tests do not replace listening on Windows. Audio and lifecycle changes need manual verification of normal completion, errors, cancellation, pause/resume, device disconnection, sleep/wake, tray exit, and recovery after a forced termination.

## Project conventions

Read [AGENTS.md](../AGENTS.md) before changing the project. Keep UI separate from native adapters and Windows APIs. Keep domain models and ports independent of React and Tauri. TypeScript IPC contracts live in [contracts.ts](../src/platform/tauri/contracts.ts); keep Rust DTOs and boundary validation aligned.

Add schema changes as new files in [migrations](../src-tauri/migrations). Never rewrite a released migration. Settings are validated natively and saved transactionally. Keep lockfiles in version control and do not add dependencies without a concrete need.

Tailwind is integrated through `@tailwindcss/vite` and the import in `src/App.css`. Shared visual tokens live in `src/styles/tokens.css`; see [UI design](ui-design.md).

Generated frontend output, Cargo output, caches, local data, credentials, and editor state are ignored. Keep synthetic fixtures and development configuration tracked. Do not attach a real user's SQLite database or recordings to an issue or commit.

## Diagnostics and packaging

Routine native timing and pipeline logs are opt-in. See [Performance](performance.md) for enabling diagnostics and interpreting measurements.

Installer commands, version synchronization, the release workflow, and manual installer checks are maintained in [Releases](releases.md).
