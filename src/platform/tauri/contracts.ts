import type { AudioSession } from "../../modules/audio/AudioController";
import type { PlayRequest, PlaybackStatus } from "../../modules/audio/AudioPlayer";
import type { AppSettings } from "../../domain/models";

/** Playback commands reject with a string; session commands use NativeError. */
export interface NativeCommands {
  settings_load: { args: undefined; result: AppSettings };
  settings_save: { args: { settings: AppSettings }; result: null };
  credentials_save: { args: undefined; result: boolean };
  credentials_check: { args: undefined; result: boolean };
  credentials_delete: { args: undefined; result: null };
  audio_list_sessions: { args: undefined; result: AudioSession[] };
  audio_duck: {
    args: { request: { sessionId: string } };
    result: { restorationToken: string; outcome: "completed" | "manual_override" };
  };
  audio_restore: { args: { request: { restorationToken: string } }; result: "completed" | "manual_override" };
  audio_play: { args: { request: PlayRequest }; result: PlaybackStatus };
  audio_stop: { args: undefined; result: null };
  audio_get_status: { args: undefined; result: PlaybackStatus };
  audio_set_volume: { args: { volume: number }; result: null };
}

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
