import catalog from "./catalog.json" with { type: "json" };
import type { Language } from "../domain/language";

export type CopyKey = keyof typeof catalog;
const entries = Object.values(catalog).sort((a, b) => b.pl.length - a.pl.length);

export function translate(key: string, language: Language): string {
  if (key === "locale") return language === "pl" ? "pl-PL" : "en-US";
  const entry = catalog[key as CopyKey];
  return entry ? entry[language] : localizeMessage(key, language);
}

/** Translate application diagnostics only, never publisher or generated content. */
export function localizeMessage(message: string, language: Language): string {
  const waiting = /^news_wait:(limited|normal):(\d+)$/.exec(message);
  if (waiting) {
    const template = catalog[waiting[1] === "limited" ? "newsRateLimited" : "newsCoolingDown"][language];
    return template.replace("{time}", new Date(Number(waiting[2]) * 1000).toLocaleString(language === "pl" ? "pl-PL" : "en-US"));
  }
  const source = language === "en" ? "pl" : "en";
  const exact = entries.find(entry => entry[source] === message);
  if (exact) return exact[language];
  for (const entry of entries) {
    if (!entry[source].includes("{")) continue;
    const fields: string[] = [];
    const pattern = entry[source].split(/(\{[^}]*\})/g).map(part => {
      if (part.startsWith("{")) { fields.push(part); return "([\\s\\S]*?)"; }
      return part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    }).join("");
    const match = message.match(new RegExp(`^${pattern}$`));
    if (match) return entry[language].replace(/\{[^}]*\}/g, field => localizeMessage(match[fields.indexOf(field) + 1] ?? field, language));
  }
  // Diagnostic messages may contain dynamic IDs, HTTP codes and nested causes.
  // Replace whole known phrases without touching those technical values.
  const phrases = entries.filter(entry => entry[source].length > 8 && !entry[source].includes("{")).sort((a,b) => b[source].length - a[source].length);
  const escaped = phrases.map(entry => entry[source].replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const lookup = new Map(phrases.map(entry => [entry[source], entry[language]]));
  return message.replace(new RegExp(escaped.join("|"), "g"), text => lookup.get(text) ?? text);
}
