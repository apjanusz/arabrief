import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AudioController } from "../../modules/audio/AudioController";
import type { NativeCommands } from "./contracts";

export const audioSessionReader: Pick<AudioController, "listSessions" | "duck" | "restore"> = {
  async duck(sessionId) {
    return invoke<NativeCommands["audio_duck"]["result"]>("audio_duck", { request: { sessionId } });
  },
  async restore(restorationToken) {
    return invoke<NativeCommands["audio_restore"]["result"]>("audio_restore", { request: { restorationToken } });
  },
  async listSessions() {
    if (!isTauri()) throw new Error("Lista sesji jest dostępna w aplikacji desktopowej Windows.");
    return invoke<NativeCommands["audio_list_sessions"]["result"]>("audio_list_sessions");
  },
};
