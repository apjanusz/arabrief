import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SpeechAsset } from "../../domain/models";
import type { SpeechProvider } from "../../modules/speech/SpeechProvider";

export const speechProvider: SpeechProvider = {
  synthesize: (broadcastId, voice, jobId) => isTauri() ? invoke<SpeechAsset>("speech_generate", { broadcastId, voice, jobId }) : Promise.reject(new Error("Generowanie audio wymaga aplikacji desktopowej.")),
  cancel: jobId => invoke<boolean>("speech_cancel", { jobId }),
};
