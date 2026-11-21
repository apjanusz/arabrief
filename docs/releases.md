# Releases and installers

[Development](development.md) · [User installation guide](getting-started.md)

## Existing release and workflow

[v0.1.0](https://github.com/apjanusz/arabrief/releases/tag/v0.1.0) is an existing published release with the Windows x64 NSIS installer. Versioned artifacts are distinct from the current default branch, which can contain later documentation changes.

[release-windows.yml](../.github/workflows/release-windows.yml) runs only when a matching version tag is pushed. Ordinary commits and pull requests do not invoke it. It uses Windows, Node.js 24, and the Rust MSVC x64 target to build an NSIS installer through `tauri-apps/tauri-action`.

The workflow installs dependencies with `npm ci`, checks version consistency and the tag, runs release-version tests, and builds with Cargo's lockfile enforced. Tauri's `beforeBuildCommand` compiles the frontend. The release job receives `contents: write` to publish the release and upload the installer; no OpenAI key or personal access token is needed. Checkout does not persist credentials.

The current configuration publishes a regular release, not a draft or prerelease. It does not upload a plain application binary, updater manifest, or updater signatures. No automatic updater is configured.

## Version consistency

The version must match in:

- `package.json` and the root entries in `package-lock.json`.
- `src-tauri/Cargo.toml` and the application package entry in `src-tauri/Cargo.lock`.
- `src-tauri/tauri.conf.json`.
- The pushed tag, in the form `vX.Y.Z`.

Validate a candidate release, for example a future `0.1.1`:

```powershell
npm run release:check -- --tag v0.1.1
node --test tests/release-version.test.mjs
```

The command intentionally fails until every version file matches the candidate. Update manifests and regenerate their lockfile entries using the normal package tooling. `npm version 0.1.1 --no-git-tag-version` updates the npm files; updating the Cargo manifest and running a Cargo check refreshes its application lockfile entry. Review all changes and run the relevant checks from [Development](development.md) before publishing.

## Local packaging

From a configured Windows development environment:

```powershell
npm ci
npm run tauri -- build --bundles nsis
```

Output normally appears under `src-tauri/target/release/bundle/nsis/`; a custom `CARGO_TARGET_DIR` changes the output location. The workflow's explicit target adds a target-specific directory level. The bundle embeds frontend assets, logos/icons, prompts, and migrations. SQLite is bundled at compilation, and `bundle.resources` does not include the working directory or local user data.

Current installers are per-user, use Polish NSIS prompts, download WebView2's bootstrapper when needed, and are unsigned. Users do not need Rust or Node.js.

## Publishing a new version

After the candidate commit and version checks are complete, push that commit and create a new version tag pointing to it. Pushing the tag starts publication automatically. Review the resulting Actions run and release assets, then perform the manual installer checks below.

Keep existing published tags and releases intact. If application changes are needed after publication, prepare a new version rather than moving a published tag. A transient download/upload failure can be investigated in Actions and retried through the failed-job controls.

## Manual installer checks

- Install on a clean Windows account without development tools; verify shortcuts, branding, launch, and WebView2 setup.
- Save credentials, preferences, and topics; restart and verify persisted state.
- Prepare and play a briefing; verify voice, source links, and music restoration after completion, Stop, and errors.
- Exercise pause/resume during playback and countdown, including independent music-volume changes while paused.
- Check tray hiding, real exit, sleep/wake, startup settings, and device disconnection.
- Reinstall or upgrade with the app closed; check startup registration and the documented handling of user data and credentials when uninstalling.

These are manual validation steps, not a claim that every Windows configuration has been tested. Data handling is described in [Privacy](privacy.md).
