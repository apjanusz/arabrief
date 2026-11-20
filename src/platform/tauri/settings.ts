import { invoke, isTauri } from "@tauri-apps/api/core";
import { isLanguagePreference } from "../../domain/language.ts";
import { isNewsRegionPreference } from "../../domain/newsRegion.ts";
import type { AppSettings } from "../../domain/models";
import type { SettingsRepository } from "../../modules/settings/SettingsRepository";
import type { CredentialStore, KeyValidation } from "../../modules/settings/CredentialStore";

function validateBackgroundSettings(settings: AppSettings): AppSettings {
  if (!settings || typeof settings.autoStart !== "boolean" || typeof settings.startSchedulerOnLaunch !== "boolean") {
    throw new Error("Nieprawidłowe ustawienia działania w tle.");
  }
  if (!Number.isSafeInteger(settings.intervalMs) || settings.intervalMs < 300000 || settings.intervalMs > 86400000) throw new Error("Invalid broadcast interval. Minimum: 5 minutes.");
  const uiLanguage = settings.uiLanguage ?? "auto";
  if (!isLanguagePreference(uiLanguage)) throw new Error("Nieprawidłowy język interfejsu.");
  const newsRegion = settings.newsRegion;
  if (!isNewsRegionPreference(newsRegion)) throw new Error("Invalid news region.");
  if (settings.language !== "pl" && settings.language !== "en") throw new Error("Invalid broadcast language.");
  return { ...settings, uiLanguage, newsRegion };
}

export const settingsRepository: SettingsRepository = {
  load: () => isTauri() ? invoke<AppSettings>("settings_load").then(validateBackgroundSettings) : Promise.reject(new Error("Ustawienia wymagają aplikacji desktopowej.")),
  save: async settings => { await invoke("settings_save", { settings: validateBackgroundSettings(settings) }); },
};

/** Only metadata crosses IPC. Saving opens a native dialog; no key parameter. */
export const apiCredentials: CredentialStore = {
  save: () => invoke<boolean>("credentials_save"),
  check: () => invoke<KeyValidation>("credentials_validate"),
  delete: () => invoke<void>("credentials_delete"),
};
