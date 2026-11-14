/** Wire dates use ISO 8601 UTC strings; durations use milliseconds. */
export interface NewsItem {
  id: string;
  provider: string;
  title: string;
  url: string;
  source: string | null;
  sourceUrl: string | null;
  publishedAt: string | null;
  rawPublicationDate: string | null;
  /** Raw RSS description, possibly HTML. This is not full article content. */
  descriptionHtml: string | null;
  guid: string | null;
  feedUrl: string;
  fetchedAt: string;
  topics: string[];
}

export interface BroadcastScript {
  id: string;
  createdAt: string;
  model: string;
  title: string;
  text: string;
  language: string;
  topics: string[];
  estimatedDurationMs: number;
  targetMinDurationMs: number;
  targetMaxDurationMs: number;
  draft: {
    title: string;
    segments: { topic: string; text: string; evidence: { source_id: string; quote: string }[] }[];
    limitations: string[];
    insufficient_material: boolean;
  };
  sources: NewsItem[];
}

/** Opaque local resource identifier, never a remote URL or arbitrary file path. */
export interface SpeechAsset {
  resourceId: string;
  durationMs: number;
  model: string;
  voice: string;
  transcript: string;
  verification: "realtime_transcript_normalized_exact";
  segmentCount: number;
  cached: boolean;
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
  textModel: string;
  autoStart: boolean;
  language: string;
  newsSourceUrls: string[];
}

export type BroadcastState =
  | { phase: "disabled" }
  | { phase: "waiting"; nextBroadcastAt: string }
  | { phase: "preparing" | "broadcasting" | "restoring" }
  | { phase: "error"; message: string };
