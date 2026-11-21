# Publication review — November 21, 2026

[Licensing review](licensing.md) · [Security policy](../SECURITY.md) · [Changelog](../CHANGELOG.md)

## Scope and results

The review started from a clean working tree at `e6b7c29`. It inspected the 161 currently tracked files, the npm/Cargo dependency declarations, the existing release metadata, and the implementation paths relevant to public documentation. New documentation was also checked before handoff. It did not change application code, workflow configuration, manifests, lockfiles, tags, repository settings, or Releases. No build, installer build, paid API request, or application test was run.

### Release evidence

GitHub reports `v0.1.0` published at `2026-11-20T23:57:01Z`, with `AraBrief_0.1.0_x64-setup.exe`, targeting commit `e2511b29803cfe870aa15c6c1946c0a56e95f5b5`. The asset name/size and uploaded state were checked through release metadata; the installer was not downloaded or unpacked. At review time, application source, configuration, and lockfiles matched the tagged version. The subsequent MIT addition updates license metadata only. [CHANGELOG.md](../CHANGELOG.md) describes the release, not later documentation or licensing changes.

### API, privacy, and limitations

The existing documentation matches the checked implementation on the following points:

- Native credential entry, Windows Credential Manager target `com.janus.ara-brief/openai-api-key`, and no IPC operation returning the key.
- Native `/v1/models` validation, `/v1/responses` chapter generation with `store: false`, and `/v1/audio/speech` synthesis. A successful model-list check does not prove billing or speech/text permissions.
- Separate API billing, including charges for preparation without playback; no promise of a fixed briefing price or app-enforced spending cap.
- Local SQLite, article/script/PCM caches, WAV recordings, and shared development/installed application identity. No application-level encryption of the database or recordings.
- Disclosure of topic/source/text transfers to external services, without implying that `store: false` prevents all provider retention. See [OpenAI data controls](https://developers.openai.com/api/docs/guides/your-data) and [billing guidance](https://help.openai.com/en/articles/9039756-managing-billing-for-chatgpt-and-the-api-platform).
- Polish/English interface and narration with independent news country, delayed first scheduled playback, source/backoff limits, incomplete-material handling, blocking cancellation, and bounded crash recovery.
- The known January 6, 2027 shutdown of the current speech model is already documented with the [official speech guide](https://developers.openai.com/api/docs/guides/text-to-speech). It requires a future application change rather than a documentation-only fix.

No correction to those implementation descriptions was needed. Links to licensing, contribution, release-history, and security documents were added to both READMEs. The security policy also records current CSP/WebView2 hardening limitations rather than implying a hardened deployment.

### Tracked-file secret and personal-data check

Git index content and tracked filenames were checked for API-token/private-key patterns, credential-like constants, personal email/path patterns, environment files, local databases, logs, recordings, and installer/temp artifacts. This was a heuristic review, not a guarantee against every secret format or hidden binary content.

- No real API key, private key, local user database, log, recording, installer, or temporary file was identified among tracked files.
- Three token-pattern matches in `src-tauri/src/openai_errors.rs` are the deliberate synthetic `sk-fake-sensitive` redaction-test value. Retain this test.
- The email-pattern match in `src-tauri/tauri.conf.json` is the icon filename `128x128@2x.png`, not an email address.
- Seventeen tracked files are binary application icons; they are not user recordings. Text scanning does not inspect their rendered content or prove authorship.
- The repository owner's public handle and the stable `com.janus.ara-brief` identifier are intentional project identifiers, not credentials.

The earlier audit identified a personal author/tagger email in Git metadata. That is separate from current file contents and remains relevant when making the full repository public. It was not copied into these documents, removed, or rewritten. The file scan does not clear earlier history, remote refs, CI logs, release payloads, or an installed application's user directory.

## Decisions and follow-up

1. **Project license — resolved:** after this review, the owner selected MIT. [LICENSE](../LICENSE), npm/Cargo metadata, and the contribution guide now reflect that decision. Confirming rights to externally supplied assets and satisfying dependency obligations remain separate; see [Licensing](licensing.md).
2. **Third-party notices:** an artifact-specific notice/source-availability bundle still needs to be verified for installer distribution, especially MPL-covered dependencies. The existing release payload was not audited or modified.
3. **Private vulnerability reporting:** reading its GitHub setting returned HTTP 404; this does not establish whether it is enabled or unavailable through current permissions/repository visibility. The owner should verify it when publishing. `SECURITY.md` provides a conditional route without inventing a private contact or claiming the feature is enabled.
4. **Metadata privacy:** consciously accept or address the previously identified personal Git author/tagger email before public exposure. This task did not authorize rewriting history.
5. **Runtime maintenance:** the documented speech-model shutdown and current native/webview security configuration remain application maintenance items, outside these publication-documentation changes.

Local checks cover documentation links/anchors, suspicious-file patterns, and whitespace correctness. Repository privacy and remote security controls remain owner-managed settings; this review does not change visibility or publish anything.
