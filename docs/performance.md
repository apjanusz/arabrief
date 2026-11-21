# Performance and diagnostics

[Architecture](architecture.md) · [Broadcast pipeline](broadcast-pipeline.md) · [Development](development.md)

## Current execution model

The current flow retrieves articles natively, generates independent text chapters through Responses API, and synthesizes them through Speech API before playback. RSS uses one HTTP worker and persistent cooldown/backoff; chapter retrieval/generation can use up to three workers. Full article retrieval is implemented.

The measurements below retain earlier development observations. They are not a benchmark of the current full pipeline, nor evidence of current Google/OpenAI latency. Realtime timing labels and older concurrency comparisons describe previous implementations.

## Enable native diagnostics

Routine `[perf]`, `[pipeline]`, and `[recovery]` console logs are disabled by default, including in development builds. To enable them for a PowerShell development session:

```powershell
$env:ARABRIEF_DIAGNOSTICS = '1'
npm run tauri dev
```

The native switch is read once on first use; restart the process after changing it. Remove the environment variable afterward if it should not apply to later launches:

```powershell
Remove-Item Env:ARABRIEF_DIAGNOSTICS
```

The frontend does not print routine pipeline/timing messages to the console. Measurement observers and the scheduler's bounded diagnostic history remain available; `getDiagnostics()` exposes the most recent 100 scheduler entries. Important restoration and lifecycle errors remain reportable without verbose logs. SQLite retains queue/cache state independently of console logging; it is not a full console-history store.

Diagnostics are designed to omit request bodies, credentials, full article text, narration, and PCM. Review metadata and errors before sharing them; see [Privacy](privacy.md#diagnostics).

## Interpreting timings

Native spans use `Instant`; frontend pipeline measurements use `performance.now()`. Each span includes an identifier, operation, elapsed milliseconds, and outcome. Nested timings must not be summed blindly, and parallel spans overlap.

| Operation family | What it measures |
| --- | --- |
| `prepare.total`, `prepare.news/content/speech` | Preparation as observed by orchestration, including IPC overhead |
| `broadcast.total` | Preparation/listening flow or listening to ready audio |
| `rss.http`, `rss.parse`, `rss.fetch_queue` | Feed retrieval, parsing, and the constrained fetch queue |
| `rss.persist_sqlite`, `rss.candidates_sqlite`, `rss.total` | Persistence, candidate selection, cache/limiter behavior |
| `content.context_sqlite`, `content.request_build`, `content.model` | Context construction and content generation |
| `content.responses_http`, `content.response_parse`, `content.validation`, `content.persist_sqlite`, `content.total` | Instrumented content stages; some labels belong to the earlier RSS-only path |
| `speech.prepare_sqlite`, `speech.cache_verify`, `speech.model`, `speech.persist_wav_sqlite`, `speech.total` | Speech preparation, cache checks, synthesis, and persistence |
| `realtime.*` | Historical Realtime adapter spans; not the current Speech API adapter |

Observe the operations actually emitted for a run instead of assuming every historical label still applies. Local fake-provider timings do not represent external service performance. Fewer serialized bytes do not prove fewer billable tokens or faster API responses.

## Historical controlled comparison — November 16, 2026

The original comparison used eight simulated delays, `[80, 5, 5, 5, 80, 5, 5, 5]` ms, four workers, five repetitions, and the median. Returned values and order were checked for equality. It compared a fixed batch barrier with a dynamic work queue; it did not call Google or OpenAI.

| Measurement | Before | After |
| --- | ---: | ---: |
| Simulated batch barrier / dynamic queue | 161.883 ms | 86.150 ms |
| Serialized sources on a two-item fixture | 704 bytes | 468 bytes |
| Real script generation and speech | Not measured | Not measured by this comparison |
| Full article reading | Not implemented at the time | Implemented subsequently; not measured by this comparison |

The queue experiment showed that a slow task could hold up a fixed batch while other workers were idle. The current single-worker RSS policy deliberately prioritizes rate limits over that historical concurrency strategy. The fixture serialization comparison removed technical fields while preserving facts and source associations; it is not a measurement of current article-body requests.

A recorded single live Google News fetch for the Polish topic “nauka” returned 100 entries: HTTP 520.401 ms, parse 9.923 ms, total 530.456 ms. It was one observation after instrumentation, not a before/after comparison or a repeatable service guarantee. No new live measurement was performed for this documentation update.

## Repeating the controlled comparisons

These existing local tests do not use paid OpenAI calls:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib work_queue::tests::compare_batch_barrier_and_work_queue -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib content::tests::compare_material_payload_without_removing_source_facts -- --nocapture
```

For a real profile, enable native diagnostics and perform one preparation in the desktop app. Separate cached and uncached runs, distinguish rate-limit waiting from work, and include the content model and speech stage. Real preparation can incur API charges. Avoid treating an old Realtime comparison as a Speech API benchmark.

## Scheduling and cancellation

Preparation starts at the beginning of the interval and completed audio waits for playback time. Listening to compatible ready audio avoids new news, text, and speech generation. Persistence preserves completed chapters and speech segments after partial failures.

Cancellation of blocking text/HTTP workers may wait for a response or timeout before later stages are suppressed. Speech cancellation also waits for cache writes. Timings expose these delays; they do not make blocking calls interruptible. See the [pipeline reference](broadcast-pipeline.md) for timeout/retry behavior and [Native integration](native-integration.md) for audio cleanup.
