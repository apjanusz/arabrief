import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AppSettings } from "../../domain/models";
import type { SettingsRepository } from "../../modules/settings/SettingsRepository";
import type { CredentialStore, KeyValidation } from "../../modules/settings/CredentialStore";

export const settingsRepository: SettingsRepository = {
  load: () => isTauri() ? invoke<AppSettings>("settings_load") : Promise.reject(new Error("Ustawienia wymagają aplikacji desktopowej.")),
  save: settings => invoke("settings_save", { settings }),
};

/** Only metadata crosses IPC. Saving opens a native dialog; no key parameter. */
export const apiCredentials: CredentialStore = {
  save: () => invoke<boolean>("credentials_save"),
  check: () => invoke<KeyValidation>("credentials_validate"),
  delete: () => invoke<void>("credentials_delete"),
};
