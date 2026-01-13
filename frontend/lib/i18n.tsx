"use client";

import { createContext, useContext, useEffect, useMemo, useState } from "react";
import { zh } from "./translations/zh";
import { en } from "./translations/en";

type Lang = "zh" | "en";
type Messages = Record<string, string>;
const translations: Record<Lang, Messages> = { zh, en };

type I18nContextValue = {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: (key: string) => string;
  toggle: () => void;
};

const I18nContext = createContext<I18nContextValue | undefined>(undefined);

const LANG_KEY = "rw3p_lang";

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLangState] = useState<Lang>("zh");

  useEffect(() => {
    const stored = typeof window !== "undefined" ? (localStorage.getItem(LANG_KEY) as Lang) : null;
    if (stored === "en" || stored === "zh") {
      setLangState(stored);
    }
  }, []);

  const setLang = (l: Lang) => {
    setLangState(l);
    if (typeof window !== "undefined") {
      localStorage.setItem(LANG_KEY, l);
    }
  };

  const toggle = () => setLang(lang === "zh" ? "en" : "zh");

  const t = (key: string) => translations[lang][key] ?? key;

  const value = useMemo(() => ({ lang, setLang, toggle, t }), [lang]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return ctx;
}
