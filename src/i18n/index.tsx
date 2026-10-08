/** English and Arabic UI strings, the current language, and the page direction that follows it. */
import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import ar from "./ar.json";
import en from "./en.json";

export type Lang = "en" | "ar";

const STRINGS: Record<Lang, Record<string, string>> = { en, ar };
const STORAGE_KEY = "qiyas.lang";

function storedLang(): Lang {
  try {
    return localStorage.getItem(STORAGE_KEY) === "ar" ? "ar" : "en";
  } catch {
    return "en";
  }
}

type LanguageState = { lang: Lang; setLang(lang: Lang): void };

// Outside a provider (pure component tests) the UI is English.
const LanguageContext = createContext<LanguageState>({ lang: "en", setLang: () => {} });

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(storedLang);

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = lang === "ar" ? "rtl" : "ltr";
  }, [lang]);

  function setLang(next: Lang) {
    setLangState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Storage blocked: the choice lasts for this session only.
    }
  }

  return <LanguageContext value={{ lang, setLang }}>{children}</LanguageContext>;
}

export const useLanguage = () => useContext(LanguageContext);

/** Translates `key` in the current language, filling `{name}` from `vars`; an unknown key is returned as is. */
export function useT() {
  const { lang } = useLanguage();
  return (key: string, vars?: Record<string, string | number>) => {
    const text = STRINGS[lang][key] ?? key;
    return vars ? text.replace(/\{(\w+)\}/g, (m, name: string) => String(vars[name] ?? m)) : text;
  };
}
