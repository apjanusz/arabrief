/** Wire dates use ISO 8601 UTC strings; durations use milliseconds. */
export interface NewsItem {
  id: string;
  title: string;
  url: string;
  source: string;
  publishedAt: string;
  summary: string;
}

export interface BroadcastScript {
  text: string;
  language: string;
  sourceUrls: string[];
}

/** Opaque local resource identifier, never a remote URL or arbitrary file path. */
export interface SpeechAsset {
  resourceId: string;
  durationMs: number;
}

export interface AppSettings {
  version: 1;
  preferredAudioSource: { id: string; name: string } | null;
  intervalMs: number;
  duckVolume: number; // Relative multiplier in [0, 1]; default 0.15. Never increase music volume.
  fadeOutMs: number;
  fadeInMs: number;
  narratorVolume: number;
  broadcastMinDurationMs: number;
  broadcastMaxDurationMs: number;
  topics: string[];
  openAiVoice: string;
  autoStart: boolean;
  language: string;
  newsSourceUrls: string[];
}

export type BroadcastState =
  | { phase: "disabled" }
  | { phase: "waiting"; nextBroadcastAt: string }
  | { phase: "preparing" | "broadcasting" | "restoring" }
  | { phase: "error"; message: string };
