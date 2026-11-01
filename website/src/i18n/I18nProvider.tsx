import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from "react";
import type { ReactNode } from "react";
import { catalogs } from "./catalogs.ts";
import type { Locale } from "./catalogs.ts";
import {
  createTranslator,
  LANGUAGE_STORAGE_KEY,
  resolveLocale,
} from "./locale.ts";
import type { Translator } from "./types.ts";

interface I18nContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: Translator;
  formatNumber: (value: number, options?: Intl.NumberFormatOptions) => string;
  formatDate: (value: Date, options: Intl.DateTimeFormatOptions) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function readLocale(): Locale {
  let saved: string | null = null;
  try {
    saved = localStorage.getItem(LANGUAGE_STORAGE_KEY);
  } catch {
    /* Storage can be unavailable in private contexts. */
  }
  return resolveLocale(
    new URL(location.href).searchParams.get("lang"),
    saved,
    navigator.languages,
  );
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, updateLocale] = useState(readLocale);
  const t = useMemo(() => createTranslator(locale), [locale]);
  const setLocale = useCallback((next: Locale) => {
    updateLocale(next);
    try {
      localStorage.setItem(LANGUAGE_STORAGE_KEY, next);
    } catch {
      /* Keep the choice for this visit. */
    }
    const url = new URL(location.href);
    url.searchParams.set("lang", next);
    history.replaceState(history.state, "", url);
  }, []);

  useEffect(() => {
    const onPopState = () => updateLocale(readLocale());
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  useLayoutEffect(() => {
    const definition = catalogs[locale];
    document.documentElement.lang = locale;
    document.documentElement.dir = definition.direction;
    document.title = t("meta.title");
    const metadata = {
      'meta[name="description"]': t("meta.description"),
      'meta[property="og:title"]': t("meta.title"),
      'meta[property="og:description"]': t("meta.socialDescription"),
      'meta[property="og:locale"]': definition.intlLocale.replaceAll("-", "_"),
    };
    for (const [selector, content] of Object.entries(metadata))
      document.querySelector(selector)?.setAttribute("content", content);
  }, [locale, t]);

  const value = useMemo<I18nContextValue>(
    () => ({
      locale,
      setLocale,
      t,
      formatNumber: (number, options) =>
        new Intl.NumberFormat(catalogs[locale].intlLocale, options).format(
          number,
        ),
      formatDate: (date, options) =>
        new Intl.DateTimeFormat(catalogs[locale].intlLocale, {
          timeZone: "UTC",
          ...options,
        }).format(date),
    }),
    [locale, setLocale, t],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useLocale(): I18nContextValue {
  const context = useContext(I18nContext);
  if (!context) throw new Error("useLocale must be used inside I18nProvider");
  return context;
}
