import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import type { LanguagePreference } from "../api/types";
import {
  I18nContext,
  interpolate,
  messageSets,
  resolveLocale,
} from "./runtime";
import type { I18nContextValue } from "./runtime";

export function I18nProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState<LanguagePreference>("system");
  const [systemLanguages, setSystemLanguages] = useState(readSystemLanguages);
  const locale = useMemo(
    () => resolveLocale(preference, systemLanguages),
    [preference, systemLanguages],
  );
  const value = useMemo<I18nContextValue>(() => ({
    locale,
    preference,
    setPreference,
    t: (key, values) => interpolate(messageSets[locale][key], values),
  }), [locale, preference]);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    const handleLanguageChange = () => setSystemLanguages(readSystemLanguages());
    window.addEventListener("languagechange", handleLanguageChange);
    return () => window.removeEventListener("languagechange", handleLanguageChange);
  }, []);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

function readSystemLanguages(): readonly string[] {
  return navigator.languages?.length ? [...navigator.languages] : [navigator.language];
}
