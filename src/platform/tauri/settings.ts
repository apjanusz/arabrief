import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppSettings } from "../../domain/models";
import type { SettingsRepository } from "../../modules/settings/SettingsRepository";

export const settingsRepository: SettingsRepository = {
  load: () => isTauri() ? invoke<AppSettings>("settings_load") : Promise.reject(new Error("Ustawienia wymagają aplikacji desktopowej.")),
  save: settings => invoke("settings_save", { settings }),
};

/** Only metadata crosses IPC. Saving opens a native dialog; no key parameter. */
export const apiCredentials = {
  save: () => invoke<boolean>("credentials_save"),
  check: () => invoke<boolean>("credentials_check"),
  delete: () => invoke<void>("credentials_delete"),
};
