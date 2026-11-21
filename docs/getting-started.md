# Getting started

[Project overview](../README.md) · [Troubleshooting](troubleshooting.md) · [Privacy](privacy.md)

## Install on Windows

AraBrief targets Windows 10/11 x64 and requires an internet connection and Microsoft Edge WebView2 Runtime. Node.js, Rust, and the source repository are unnecessary for the installed application.

1. Open [the latest GitHub Release](https://github.com/apjanusz/arabrief/releases/latest).
2. Under **Assets**, download the Windows installer ending in `x64-setup.exe`. The v0.1.0 asset is named `AraBrief_0.1.0_x64-setup.exe`.
3. Run the NSIS installer and follow its prompts. It installs for the current Windows user; the installer itself uses Polish.
4. If WebView2 is missing, setup downloads its bootstrapper. This requires internet access and may require additional permissions.
5. Open AraBrief from its shortcut.

Current MVP installers are unsigned. Windows may show a SmartScreen warning: check the download's origin and the release details before deciding whether to run it.

## Connect OpenAI

In **Settings**, use the API key control to open the native Windows credential dialog. Enter your own OpenAI API key there, then run the connection check.

The key is stored in Windows Credential Manager. The connection check contacts OpenAI's model-list endpoint without generating a briefing; a successful check does not guarantee available credits, model permissions, or successful speech generation.

Your API account needs billing or available credits and access to the configured models. Text generation and speech synthesis are charged separately from a ChatGPT subscription. Preparing a briefing without listening can still incur charges. Consult the [current pricing](https://developers.openai.com/api/docs/pricing) and [billing guidance](https://help.openai.com/en/articles/9039756-managing-billing-for-chatgpt-and-the-api-platform). Model availability is covered in [troubleshooting](troubleshooting.md#speech-model-availability).

## Choose language and news country

The interface supports **Automatic**, **Polski**, and **English**:

- Automatic uses the Windows user interface language: Polish selects Polish; other languages select English.
- A manual choice takes precedence after saving settings. New scripts and narration use the effective application language.
- Restart AraBrief after changing the Windows language while using Automatic.

The news country is independent of the interface and narration language. Search the country list by its localized name or code. At first launch, Polish Windows defaults to Poland (`PL`); other Windows languages default to the United States (`US`). The saved country remains selected when the application language changes. There is no Automatic country option.

Topic text and publishers' content are not translated in the interface. For a foreign news edition, use search terms appropriate to that edition's language. Newly generated narration follows the application language even when the source article uses another language.

## Set up your listening session

1. Start music in another application so Windows exposes its audio session.
2. In Settings, refresh the available sessions, select the music source, and save your settings.
3. Adjust narrator volume, music ducking, voice, text model, and briefing interval as needed.
4. Add topics in **Interests**. Topic changes save immediately; the Settings form uses its save button.
5. Use the local sound test to check output without generating AI speech.

A music player's restart or an audio-device change can invalidate the selected session. Refresh and select it again if that happens. AraBrief controls the selected session's volume, never the global output device volume.

## Prepare and listen

Start the schedule from **Studio**. Preparation begins in the background, but the first scheduled playback waits the full configured interval. The default interval is 30 minutes, with a minimum five-minute gap after a completed briefing.

For a manual preview, prepare without playback, expand the script and source details, then use the listening control when audio is ready. This preparation uses the same paid services as scheduled preparation. Existing compatible audio can be used without generating it again; changing settings may require a new preparation.

Pause/resume applies to the briefing or countdown. During a paused briefing, the music volume is restored; resuming lowers it again using a fresh snapshot. Music itself is never paused. Stop ends the current work and restores music volume. Stopping during a blocking network request may take time; see [cancellation delays](troubleshooting.md#stopping-takes-time).

If there is insufficient readable material, AraBrief may produce a shorter briefing or skip playback. It can expand discovery to older articles, so a briefing is not guaranteed to contain only today's news. AI narration can contain errors; check the linked sources for important details.

## Tray and startup

Closing the window hides AraBrief in the system tray; scheduled work can continue. Use the tray's quit command to exit the application and finish audio cleanup.

Two separate settings are off by default:

- Start AraBrief with Windows: launches the application in the tray after sign-in.
- Start the scheduler on launch: authorizes automatic preparation, including paid API use, after launch. The first playback still waits the full interval.

## Updating and local data

Download a newer installer from Releases and close AraBrief through the tray before installing. The application identifier remains `com.janus.ara-brief`, so installed and development builds under the same Windows account can share existing data and credentials.

See [Privacy](privacy.md) for data locations, backups, credential removal, and a full local reset. Do not assume uninstalling removes your database, recordings, or API key.
