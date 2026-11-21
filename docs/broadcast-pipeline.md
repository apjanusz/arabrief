# Broadcast pipeline

[Architecture](architecture.md) · [Native integration](native-integration.md) · [Performance](performance.md)

This reference describes current native article-based generation. The older RSS-only prompt and Realtime-related code support historical paths and tests; their presence does not mean new briefings use those flows.

## Discovery and queue selection

Google News RSS supplies metadata for a persistent SQLite queue. The country catalog defines the edition parameters. Search expressions preserve user topics; a local title/description filter checks relevance before storing topic associations. It is a lexical filter, not semantic AI selection.

Selection aims for three readable, unused articles. It first examines compatible local data in publication-age bands of 0–7, 8–30, and 31–90 days, preferring younger bands and selecting oldest-first within a band while rotating topics. A preparation checks at most nine candidates with up to three concurrent article workers. Invalid, future, or over-90-day publication dates are excluded.

If local sources are insufficient, discovery requests a wider range. Requested and searched ranges are persisted per topic and country; expansion can take multiple allowed RSS cycles. Already cached older sources can be used while RSS is blocked. One or two completed chapters can form a shorter briefing; no material means no paid text request and no music ducking.

RSS limits are independent of the briefing interval:

| Limit | Current behavior |
| --- | --- |
| Per-topic minimum | Five minutes |
| Gap after a batch | At least one minute |
| Topics per batch | At most three |
| HTTP workers | One, with at least two seconds between requests |
| Response size | At most 2 MiB; up to 100 entries per topic |
| HTTP 429/503 | Respect `Retry-After` and persistent exponential backoff: 5, 10, 20 minutes, up to six hours; a longer `Retry-After` still applies |

Backoff is shared across news editions and survives restarts. Changing country does not bypass it. Google News RSS is not a complete paginated archive, so discovery cannot guarantee every relevant publication.

Article identity uses stable identifiers, GUIDs, and normalized URLs, removing fragments and common tracking parameters. Titles or semantic similarity are not a semantic deduplication system. Topic changes preserve usage history.

## Native article retrieval

[content/article.rs](../src-tauri/src/content/article.rs) resolves Google News links to publishers, verifies the publisher domain and title, retrieves HTML, and extracts `articleBody` from JSON-LD or article paragraphs. Google link resolution relies on an undocumented protocol and may need maintenance when Google changes it.

The reader validates public HTTP(S) destinations, DNS results, and redirects. Limits include a 2 MiB page, 24,000 characters of extracted text, HTTP timeouts up to 12 seconds, and a 40-second reader budget; system DNS resolution has its own limitations. It uses direct connections without browser cookies. Paywalls, blocked responses, short teasers, and pages requiring JavaScript rendering are skipped.

Fetched text is saved in `article_materials` before a paid model request. Existing materials and compatible chapters can be reused. Readability preflight defers persistently unreadable candidates for 24 hours and transient failures for 30 minutes; other preparation failures have their own queue retry state. Repeated failures can remove a candidate from active selection without deleting history.

## Text generation

[content/chapters.rs](../src-tauri/src/content/chapters.rs) sends one independent Responses API request for each uncached chapter. The default text model is `gpt-5-mini`; Settings also offers other model choices and a custom identifier. Availability and request compatibility still depend on the API account.

Each request contains the retrieved text, final URL, source metadata, publication date, preparation time, and language instructions. It uses structured JSON output, `store: false`, and `tool_choice: none`: current chapter generation does not invoke paid web search. There is no additional AI selection, semantic deduplication, final summarization, or translation request.

Validation requires completed JSON, a usable chapter, `article_read=true`, a matching article URL, and evidence quotes found literally in the fetched text. Pages are treated as untrusted data. This checks source grounding, not the truth of every fact or paraphrase. There is no headline-only fallback if full content cannot be read.

Narration is capped at 100 whitespace-separated words per chapter. Excess is trimmed to the last complete sentence in that limit; absence of a complete sentence fails the chapter without an extra AI repair request. Target duration settings do not override this cap or guarantee a fixed length.

Text requests have a 90-second timeout and no automatic HTTP retry. Successful chapters are persisted independently, so a later failure does not erase earlier work. Global key/model/billing errors are not treated as proof that an article is permanently bad.

## Speech and partial success

[speech/tts.rs](../src-tauri/src/speech/tts.rs) uses Speech API with `gpt-4o-mini-tts`, PCM output, and language instructions. The exposed voices are `marin` and `cedar`. See [model availability](troubleshooting.md#speech-model-availability) for the announced shutdown date.

Text is segmented at the existing 800-character limit. Valid PCM segments are cached in `speech_segments`; completed audio is assembled into a mono 24 kHz, 16-bit WAV. Size, hash, and input-text compatibility checks protect cache reuse. The `tts_input_text` verification metadata records the text submitted to synthesis: it is not an independent transcription of the generated audio.

Only complete chapters are published, in article order. `SpeechAsset.chapters` records article IDs, positions, and PCM-derived start/end times. A failed chapter does not discard its siblings. If no chapter has complete speech, the failed preparation's reservation is released while reusable text and PCM remain.

Speech HTTP requests have a 90-second timeout. A chapter has a 300-second attempt budget and may retry once for connection/timeout errors, reusing completed segments. A worker gate prevents overlapping replacement requests for the same in-flight work. This is not a guarantee that an ambiguous request was unbilled.

## Preparation and playback recovery

`pipeline_preparations`, `playback_receipts`, and `speech_segments` preserve work independently of UI lifetime. Recovery examines up to 20 recent compatible preparations within six hours. Compatibility includes relevant topics, model, voice, duration preferences, language, and region; a music-session change alone does not invalidate content.

| Operation | Purpose |
| --- | --- |
| `broadcast_restore` | Recover an unused compatible script and, when available, audio |
| `broadcast_claim` | Persist an attempted receipt before playback |
| `broadcast_complete` | Record completion after native playback and volume restoration |
| `news_mark_used` | Record the articles included in completed playback |

A compatible prepared briefing can avoid repeat paid work. Preparing a new briefing may retain the previous ready material as a fallback. Cancellation does not activate a fallback. A saved script can proceed to speech without regenerating text.

Attempted receipts prevent automatic replay of partly heard audio after a crash. Completed playback remains recognized even if a later history write fails. Recovery of music volume is a separate native responsibility described in [Native integration](native-integration.md#audio-restoration).

## Live reader check

The opt-in reader test uses the network but does not call OpenAI:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib live_google_article_reader_without_openai -- --ignored --nocapture
```

It depends on live Google/publisher behavior and does not replace the deterministic fixture-based test suite described in [Development](development.md).
