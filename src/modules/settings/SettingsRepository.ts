import type { AppSettings } from "../../domain/models";

/** Only non-secret preferences; validate on both load and save. */
export interface SettingsRepository {
  load(): Promise<AppSettings | null>;
  save(settings: AppSettings): Promise<void>;
}
