import type { Language } from "./language";
import countries from "./newsCountries.json" with { type: "json" };

export const newsCountries = countries;
export type NewsRegion = string;
export type NewsRegionPreference = NewsRegion;
export function isNewsRegionPreference(value: unknown): value is NewsRegionPreference {
  return typeof value === "string" && countries.some(country => country.code === value);
}
export function defaultNewsRegion(systemLanguage: Language): NewsRegion {
  return systemLanguage === "pl" ? "PL" : "US";
}
