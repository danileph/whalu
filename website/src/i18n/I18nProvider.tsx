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
  resolveLanguage,
} from "./locale.ts";
import type { LanguagePreference, LanguageSelection } from "./locale.ts";
import type { Translator } from "./types.ts";

interface I18nContextValue {
  locale: Locale;
  preference: LanguagePreference;
  setPreference: (preference: LanguagePreference) => void;
  t: Translator;
  formatNumber: (value: number, options?: Intl.NumberFormatOptions) => string;
  formatDate: (value: Date, options: Intl.DateTimeFormatOptions) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function browserLanguages(): readonly string[] {
  return navigator.languages?.length ? navigator.languages : [navigator.language];
}

function readLanguage(): LanguageSelection {
  let saved: string | null = null;
  try {
    saved = localStorage.getItem(LANGUAGE_STORAGE_KEY);
  } catch {
    /* Storage can be unavailable in private contexts. */
  }
  return resolveLanguage(
    new URL(location.href).searchParams.get("lang"),
    saved,
    browserLanguages(),
  );
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [{ locale, preference }, updateLanguage] = useState(readLanguage);
  const t = useMemo(() => createTranslator(locale), [locale]);
  const setPreference = useCallback((next: LanguagePreference) => {
    const url = new URL(location.href);
    if (next === "auto") {
      url.searchParams.delete("lang");
    } else {
      url.searchParams.set("lang", next);
    }
    try {
      if (next === "auto") {
        localStorage.removeItem(LANGUAGE_STORAGE_KEY);
      } else {
        localStorage.setItem(LANGUAGE_STORAGE_KEY, next);
      }
    } catch {
      /* Keep the choice for this visit. */
    }
    history.replaceState(history.state, "", url);
    updateLanguage(resolveLanguage(url.searchParams.get("lang"), null, browserLanguages()));
  }, []);

  useEffect(() => {
    const onPopState = () => updateLanguage(readLanguage());
    const onLanguageChange = () => {
      updateLanguage((current) =>
        current.preference === "auto" ? resolveLanguage(null, null, browserLanguages()) : current,
      );
    };
    window.addEventListener("popstate", onPopState);
    window.addEventListener("languagechange", onLanguageChange);
    return () => {
      window.removeEventListener("popstate", onPopState);
      window.removeEventListener("languagechange", onLanguageChange);
    };
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
      preference,
      setPreference,
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
    [locale, preference, setPreference, t],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useLocale(): I18nContextValue {
  const context = useContext(I18nContext);
  if (!context) throw new Error("useLocale must be used inside I18nProvider");
  return context;
}
