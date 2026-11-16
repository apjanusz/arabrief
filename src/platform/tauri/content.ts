import { invoke, isTauri } from "@tauri-apps/api/core";
import type { BroadcastScript } from "../../domain/models";
import type { ContentGenerator } from "../../modules/content/ContentGenerator";
import { NoUsableMaterial } from "../../modules/scheduler/PreparationRecovery";
import type { ContentFailure } from "./contracts";

export const contentGenerator: ContentGenerator = {
  async generate(articleIds) {
    if (!isTauri()) throw new Error("Generowanie wymaga aplikacji desktopowej.");
    try { return await invoke<BroadcastScript>("content_generate", { articleIds }); }
    catch (cause) {
      if (cause && typeof cause === "object" && "code" in cause && "message" in cause) {
        const error=cause as ContentFailure;
        if(error.code === "no_usable_material") throw new NoUsableMaterial(error.message);
        throw new Error(error.message);
      }
      throw cause;
    }
  },
};
