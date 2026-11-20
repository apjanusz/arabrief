export type Language = "pl" | "en";
export type LanguagePreference = "auto" | Language;

export function isLanguagePreference(value: unknown): value is LanguagePreference {
  return value === "auto" || value === "pl" || value === "en";
}

export function resolveLanguage(preference: LanguagePreference, systemLanguage: Language): Language {
  return preference === "auto" ? systemLanguage : preference;
}
