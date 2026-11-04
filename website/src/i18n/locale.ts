import { catalogs, defaultLocale, supportedLocales } from "./catalogs.ts";
import type { Locale } from "./catalogs.ts";
import type { MessageParameters, Translator } from "./types.ts";

export const LANGUAGE_STORAGE_KEY = "whalu-language";
export type LanguagePreference = Locale | "auto";

export interface LanguageSelection {
  locale: Locale;
  preference: LanguagePreference;
}

export function matchLocale(
  value: string | null | undefined,
): Locale | undefined {
  let candidate = value?.trim().replaceAll("_", "-").toLowerCase() ?? "";
  while (candidate) {
    const match = supportedLocales.find(
      (code) => code.toLowerCase() === candidate,
    );
    if (match) return match;
    candidate = candidate.split("-").slice(0, -1).join("-");
  }
  return undefined;
}

export function resolveLanguage(
  query: string | null,
  saved: string | null,
  preferred: readonly string[],
): LanguageSelection {
  const explicit = matchLocale(query) ?? matchLocale(saved);
  if (explicit) return { locale: explicit, preference: explicit };
  for (const language of preferred) {
    const match = matchLocale(language);
    if (match) return { locale: match, preference: "auto" };
  }
  return { locale: defaultLocale, preference: "auto" };
}

export function formatMessage(
  message: string,
  parameters?: MessageParameters,
): string {
  return message.replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (_, name: string) => {
    if (parameters?.[name] === undefined)
      throw new Error(`Missing translation parameter: ${name}`);
    return String(parameters[name]);
  });
}

export function createTranslator(locale: Locale): Translator {
  return (key, ...args) =>
    formatMessage(
      catalogs[locale].messages[key] ?? catalogs[defaultLocale].messages[key],
      args[0],
    );
}
