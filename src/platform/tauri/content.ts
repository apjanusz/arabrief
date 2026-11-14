import { invoke, isTauri } from "@tauri-apps/api/core";
import type { BroadcastScript } from "../../domain/models";
import type { ContentGenerator } from "../../modules/content/ContentGenerator";

export const contentGenerator: ContentGenerator = {
  generate: articleIds => isTauri() ? invoke<BroadcastScript>("content_generate", { articleIds }) : Promise.reject(new Error("Generowanie wymaga aplikacji desktopowej.")),
};
