import type { AudioSession } from "../../modules/audio/AudioController";

/** Proposed IPC only: no commands below are registered in Rust yet. */
export interface NativeCommands {
  audio_list_sessions: { args: undefined; result: AudioSession[] };
  audio_duck: {
    args: { request: { sessionIds: string[]; volume: number } };
    result: { restorationToken: string };
  };
  audio_restore: { args: { request: { restorationToken: string } }; result: null };
  audio_play: { args: { request: { resourceId: string } }; result: null };
  audio_stop: { args: undefined; result: null };
}

/** Rust Result<T, NativeError>: Ok resolves; Err rejects invoke with this object. */
export interface NativeError {
  code: "not_supported" | "invalid_input" | "session_unavailable" | "audio_failure";
  message: string;
}

/** Adapter signature; deliberately no runtime invoke until native implementation exists. */
export type NativeInvoker = <C extends keyof NativeCommands>(
  command: C,
  args: NativeCommands[C]["args"],
) => Promise<NativeCommands[C]["result"]>;
