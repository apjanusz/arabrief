import { createContext, useContext, useEffect, useState } from "react";
import { translate, localizeMessage } from "./localization";
import type { ReactNode } from "react";
import type { Language, LanguagePreference } from "../domain/language";
import { resolveLanguage } from "../domain/language";
import { readSystemLanguage } from "../platform/tauri/language";

const messages = {
  pl: {
    languageTitle: "Język interfejsu",
    languageDescription: "Wybierz język aplikacji lub użyj języka Windows.",
    languageLabel: "Język",
    languageHint: "Wybór zostanie zapamiętany po zapisaniu ustawień. Nowe audycje powstaną w języku aplikacji.",
    automatic: "Automatycznie",
  },
  en: {
    languageTitle: "Interface language",
    languageDescription: "Choose the app language or use the Windows language.",
    languageLabel: "Language",
    languageHint: "Save settings to remember your selection. New broadcasts use the app language.",
    automatic: "Automatically",
  },
} as const;
type TranslationKey = keyof typeof messages.pl;
const LanguageContext = createContext<Language>("en");
export function useCopy() {
  const language = useContext(LanguageContext);
  return (key: string | null): string => translate(key ?? "", language);
}

export function LanguageProvider({ preference, children }: { preference: LanguagePreference; children: ReactNode }) {
  const [systemLanguage, setSystemLanguage] = useState<Language>("en");
  useEffect(() => {
    let active = true;
    void readSystemLanguage().then(language => { if (active) setSystemLanguage(language); });
    return () => { active = false; };
  }, []);
  const language = resolveLanguage(preference, systemLanguage);
  useEffect(() => { document.documentElement.lang = language; }, [language]);
  return <LanguageContext.Provider value={language}>{children}</LanguageContext.Provider>;
}

export function useTranslation() {
  const language = useContext(LanguageContext);
  return { language, locale: language === "pl" ? "pl-PL" : "en-US", message: (text: string) => localizeMessage(text, language), t: (key: TranslationKey): string => messages[language][key] };
}
