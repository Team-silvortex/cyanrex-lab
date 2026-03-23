import {
  createContext,
  PropsWithChildren,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import {
  DEFAULT_LOCALE,
  Locale,
  SUPPORTED_LOCALES,
  TranslationDict,
  translations,
} from "./translations";

type I18nContextValue = {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
  supportedLocales: typeof SUPPORTED_LOCALES;
};

const I18nContext = createContext<I18nContextValue | null>(null);
const STORAGE_KEY = "cyanrex_locale";

export function I18nProvider({ children }: PropsWithChildren) {
  const [locale, setLocaleState] = useState<Locale>(DEFAULT_LOCALE);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const stored = window.localStorage.getItem(STORAGE_KEY) as Locale | null;
    if (stored && SUPPORTED_LOCALES.some((entry) => entry.code === stored)) {
      setLocaleState(stored);
      return;
    }

    const browser = window.navigator.language;
    if (browser.startsWith("zh")) setLocaleState("zh-CN");
    else if (browser.startsWith("es")) setLocaleState("es");
    else if (browser.startsWith("ja")) setLocaleState("ja");
    else setLocaleState("en");
  }, []);

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, next);
    }
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => {
      const raw =
        lookupTranslation(translations[locale], key) ??
        lookupTranslation(translations.en, key) ??
        key;
      if (!vars) return raw;
      return raw.replace(/\{(\w+)\}/g, (_, name: string) => {
        const value = vars[name];
        return value === undefined ? `{${name}}` : String(value);
      });
    },
    [locale],
  );

  const value = useMemo(
    () => ({
      locale,
      setLocale,
      t,
      supportedLocales: SUPPORTED_LOCALES,
    }),
    [locale, setLocale, t],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used inside I18nProvider");
  }
  return ctx;
}

function lookupTranslation(dict: TranslationDict, key: string): string | null {
  const parts = key.split(".");
  let current: string | TranslationDict | undefined = dict;

  for (const part of parts) {
    if (!current || typeof current === "string") return null;
    current = current[part];
  }

  return typeof current === "string" ? current : null;
}
