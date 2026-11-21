# Troubleshooting

[Getting started](getting-started.md) · [Privacy](privacy.md) · [Development](development.md)

## Installation or startup fails

Use the Windows x64 installer from [GitHub Releases](https://github.com/apjanusz/arabrief/releases/latest), not a source archive. Check that WebView2 Runtime is installed; its bootstrapper needs internet access. Current MVP installers are unsigned, which can trigger SmartScreen. Verify the source of the download before deciding whether to run it.

If opening a second instance appears to do nothing, check the tray: AraBrief uses a single instance. Closing the main window hides it rather than exiting.

## No music session is available

Start audio in your music application, refresh the list in Settings, and select the session on the intended output device. Restarting a player or switching devices can create a new session; select and save it again. AraBrief excludes its own process, system sounds, and expired sessions. It does not guess a replacement from a process name or PID.

## There is no narration, or it plays on another device

Run the local sound test, check narrator volume, and check Windows' default playback device. Narration normally opens the current default output at the start of playback. The low-level test integration can also target the selected music session's device. Changing the default device takes effect on the next playback; an output disconnection can end the current one with an error.

The [native integration reference](native-integration.md) explains output selection and cleanup.

## Music volume was not restored

Use Stop or the tray's quit command and allow cleanup to finish. After a forced termination, restart AraBrief to allow journal recovery before deleting any local data.

Recovery targets the original device and session instance. If the music app restarted or the device disconnected, that instance may no longer exist. A user's independent mixer change takes precedence over the saved snapshot. Restore your preferred level in Windows' mixer if the original session cannot be recovered. See [audio restoration](native-integration.md#audio-restoration) for the limits of crash recovery.

## The API key exists, but generation fails

A local presence check confirms only that a credential exists. The online check uses OpenAI's model-list endpoint; it does not test a paid text or speech request.

Check the reported category: authentication or permissions, model availability, rate limit, or account billing/quota. Confirm that the selected text model supports the application's Responses API request. A custom model identifier is not proof of compatibility. Consult [OpenAI billing guidance](https://help.openai.com/en/articles/9039756-managing-billing-for-chatgpt-and-the-api-platform) and the API account's usage settings. Repeatedly preparing new briefings can incur additional charges.

## Speech model availability

The current adapter calls `/v1/audio/speech` using `gpt-4o-mini-tts`, with `marin` or `cedar` exposed in Settings. OpenAI has announced that this model will shut down on **January 6, 2027**. This information was checked on November 21, 2026 against the [official text-to-speech guide](https://developers.openai.com/api/docs/guides/text-to-speech).

The API key connection check cannot guarantee speech-model availability. After the shutdown, this adapter will require an application update; changing the text model in Settings does not replace the speech adapter. Realtime-related source files and older data formats are not an automatic fallback for current speech generation.

## No articles, older articles, or a shorter briefing

Check your topics and news country. Use search terms suited to the selected edition. The topic filter uses local matching rather than an AI semantic search and can miss relevant synonyms.

RSS discovery is rate-limited. HTTP 429/503 responses persist a backoff across restarts, and restarting does not reset it. Existing compatible local sources may remain usable. Allow the displayed retry period to pass instead of repeatedly requesting new preparation.

Unreadable pages, paywalls, JavaScript-only pages, invalid dates, and unavailable publishers are skipped. There is no headline-only fallback. Selection can expand from the last seven days to 30 and then 90 days; available material may produce one or two chapters rather than three. The [pipeline reference](broadcast-pipeline.md) documents these rules.

## A scheduled briefing did not play

The first scheduled briefing waits the full interval. Check that the selected music session remains available and audio is ready. Missed deadlines after computer sleep and incomplete preparations are skipped without lowering music. Resuming Windows does not replay every missed briefing.

If automatic preparation did not start after launch, check the separate scheduler-on-launch setting. Windows autostart alone does not authorize automatic paid preparation.

## Stopping takes time

Text generation and speech use blocking HTTP workers with request timeouts up to 90 seconds. Stop prevents later stages, but an in-flight request may finish or time out first. Speech cancellation waits for the worker to finish writing reusable cache data. A request already submitted to OpenAI may still be billed.

Avoid repeatedly starting replacement jobs while cleanup is pending. Complete cached work is kept for a compatible later attempt.

## Language or country appears wrong

Save a manual language choice in Settings, or restart after changing the Windows interface language in Automatic mode. News country is a separate saved choice and does not follow later language changes. Source titles and user topics are not translated in the interface. Prepare new material after changing language or country; an incompatible preview is not reused.

## Collecting diagnostics

See [Performance](performance.md) for opt-in native diagnostics. Before sharing logs or an issue report, inspect them for article URLs, topic text, local paths, and identifying metadata. Never attach an API key, Credential Manager export, user database, or real recordings. Use a minimal synthetic reproduction where possible.
