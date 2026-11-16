import type { AudioSession } from "../../modules/audio/AudioController";
import type { PlayRequest, PlaybackStatus } from "../../modules/audio/AudioPlayer";
import type { AppSettings, BroadcastScript, SpeechAsset } from "../../domain/models";
import type { NewsResult } from "../../modules/news/NewsProvider";
import type { KeyValidation } from "../../modules/settings/CredentialStore";
import type { BackgroundLaunch, BackgroundStatus } from "../../modules/scheduler/BackgroundLifecycle";

/** Playback commands reject with a string; session commands use NativeError. */
export interface NativeCommands {
  background_connect: { args: undefined; result: BackgroundLaunch };
  background_status: { args: { status: BackgroundStatus }; result: null };
  background_stopped: { args: undefined; result: null };
  broadcast_restore: { args: undefined; result: { script: BroadcastScript; audio: SpeechAsset | null } | null };
  broadcast_claim: { args: { broadcastId: string }; result: null };
  broadcast_complete: { args: { broadcastId: string }; result: null };
  /** Rust SpeechAsset includes ordered article IDs and PCM-derived chapter time ranges. */
  speech_generate: { args: { broadcastId: string; voice: string | null; jobId: string }; result: SpeechAsset };
  speech_cancel: { args: { jobId: string }; result: boolean };
  content_generate: { args: { articleIds: readonly string[] }; result: BroadcastScript };
  news_fetch: { args: { topics: string[] }; result: NewsResult };
  news_mark_used: { args: { articleIds: string[]; broadcastId: string }; result: null };
  settings_load: { args: undefined; result: AppSettings };
  settings_save: { args: { settings: AppSettings }; result: null };
  credentials_save: { args: undefined; result: boolean };
  credentials_check: { args: undefined; result: boolean };
  credentials_validate: { args: undefined; result: KeyValidation };
  credentials_delete: { args: undefined; result: null };
  audio_list_sessions: { args: undefined; result: AudioSession[] };
  audio_duck: {
    args: { request: { sessionId: string } };
    result: { restorationToken: string; outcome: "completed" | "manual_override" };
  };
  audio_restore: { args: { request: { restorationToken: string } }; result: "completed" | "manual_override" };
  audio_play: { args: { request: PlayRequest }; result: PlaybackStatus };
  audio_stop: { args: undefined; result: null };
  audio_set_paused: { args: { paused: boolean; playbackKey: string }; result: PlaybackStatus };
  audio_get_status: { args: undefined; result: PlaybackStatus };
  audio_set_volume: { args: { volume: number }; result: null };
}

/** content_generate rejects with a structured error; lack of material is normal. */
export interface ContentFailure { code: "no_usable_material" | "content_failure"; message: string }

/** Rust Result<T, NativeError>: Ok resolves; Err rejects invoke with this object. */
export interface NativeError {
  code: "not_supported" | "invalid_input" | "session_unavailable" | "audio_failure";
  message: string;
}

/** Typed signature for native adapters; only registered commands may be invoked. */
export type NativeInvoker = <C extends keyof NativeCommands>(
  command: C,
  args: NativeCommands[C]["args"],
) => Promise<NativeCommands[C]["result"]>;
