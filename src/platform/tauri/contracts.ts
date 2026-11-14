import type { AudioSession } from "../../modules/audio/AudioController";

/** Session listing and duck/restore are implemented; playback is still a proposal. */
export interface NativeCommands {
  audio_list_sessions: { args: undefined; result: AudioSession[] };
  audio_duck: {
    args: { request: { sessionId: string } };
    result: { restorationToken: string; outcome: "completed" | "manual_override" };
  };
  audio_restore: { args: { request: { restorationToken: string } }; result: "completed" | "manual_override" };
  audio_play: { args: { request: { resourceId: string } }; result: null };
  audio_stop: { args: undefined; result: null };
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
