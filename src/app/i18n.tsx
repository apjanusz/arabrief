import { createContext, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import type { Language, LanguagePreference } from "../domain/language";
import { resolveLanguage } from "../domain/language";
import { readSystemLanguage } from "../platform/tauri/language";

const messages = {
  pl: {
    languageTitle: "Język interfejsu",
    languageDescription: "Wybierz język aplikacji lub użyj języka Windows.",
    languageLabel: "Język",
    languageHint: "Zmiana obowiązuje po zapisaniu ustawień. Na razie przetłumaczona jest tylko ta sekcja. Język audycji pozostaje bez zmian.",
    automatic: "Automatycznie",
  },
  en: {
    languageTitle: "Interface language",
    languageDescription: "Choose the app language or use the Windows language.",
    languageLabel: "Language",
    languageHint: "Changes apply after saving settings. Only this section is translated for now. Broadcast language remains unchanged.",
    automatic: "Automatycznie",
  },
} as const;
type TranslationKey = keyof typeof messages.pl;
const LanguageContext = createContext<Language>("en");

export function LanguageProvider({ preference, children }: { preference: LanguagePreference; children: ReactNode }) {
  const [systemLanguage, setSystemLanguage] = useState<Language>("en");
  useEffect(() => {
    let active = true;
    void readSystemLanguage().then(language => { if (active) setSystemLanguage(language); });
    return () => { active = false; };
  }, []);
  const language = resolveLanguage(preference, systemLanguage);
  return <LanguageContext.Provider value={language}>{children}</LanguageContext.Provider>;
}

export function useTranslation() {
  const language = useContext(LanguageContext);
  return { language, t: (key: TranslationKey): string => messages[language][key] };
}
