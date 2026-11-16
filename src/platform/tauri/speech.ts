import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SpeechAsset } from "../../domain/models";
import type { SpeechProvider } from "../../modules/speech/SpeechProvider";

function validateAudio(asset: SpeechAsset): SpeechAsset {
  if (!asset || !Array.isArray(asset.chapters) || asset.chapters.length > 3) throw new Error("Nieprawidłowa lista rozdziałów audio.");
  let end = 0, position = -1;
  for (const chapter of asset.chapters) {
    if (!chapter || typeof chapter.articleId !== "string" || !/^\d+$/.test(chapter.articleId)
      || !Number.isSafeInteger(chapter.position) || chapter.position <= position
      || !Number.isSafeInteger(chapter.startMs) || chapter.startMs !== end
      || !Number.isSafeInteger(chapter.endMs) || chapter.endMs < chapter.startMs || chapter.endMs > asset.durationMs) {
      throw new Error("Nieprawidłowe granice rozdziału audio.");
    }
    end = chapter.endMs; position = chapter.position;
  }
  return asset;
}

export const speechProvider: SpeechProvider = {
  synthesize: (broadcastId, voice, jobId) => isTauri() ? invoke<SpeechAsset>("speech_generate", { broadcastId, voice, jobId }).then(validateAudio) : Promise.reject(new Error("Generowanie audio wymaga aplikacji desktopowej.")),
  cancel: jobId => invoke<boolean>("speech_cancel", { jobId }),
};
