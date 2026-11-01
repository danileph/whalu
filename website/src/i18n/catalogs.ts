import { en } from "./messages/en.ts";
import { ru } from "./messages/ru.ts";
import type { Messages } from "./types.ts";

interface LanguageDefinition {
  nativeName: string;
  shortName: string;
  intlLocale: string;
  direction: "ltr" | "rtl";
  messages: Messages;
}

// The selector, locale type, and catalog checks all derive from this registry.
export const catalogs = {
  en: {
    nativeName: "English",
    shortName: "EN",
    intlLocale: "en-US",
    direction: "ltr",
    messages: en,
  },
  ru: {
    nativeName: "Русский",
    shortName: "RU",
    intlLocale: "ru-RU",
    direction: "ltr",
    messages: ru,
  },
} satisfies Record<string, LanguageDefinition>;

export type Locale = keyof typeof catalogs;
export const supportedLocales = Object.keys(catalogs) as Locale[];
export const defaultLocale: Locale = "en";
