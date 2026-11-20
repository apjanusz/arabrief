import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Language } from "../../domain/language";

export async function readSystemLanguage(): Promise<Language> {
  if (!isTauri()) return "en";
  try {
    const language: unknown = await invoke("system_ui_language");
    return language === "pl" ? "pl" : "en";
  } catch {
    return "en";
  }
}
