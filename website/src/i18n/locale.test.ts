import assert from "node:assert/strict";
import test from "node:test";
import { catalogs, supportedLocales } from "./catalogs.ts";
import {
  createTranslator,
  formatMessage,
  matchLocale,
  resolveLanguage,
} from "./locale.ts";

test("every registered language has every message and the same named placeholders", () => {
  const keys = Object.keys(catalogs.en.messages).sort();
  const placeholders = (value: string) =>
    [...value.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\}/g)]
      .map((match) => match[1])
      .sort();
  for (const locale of supportedLocales) {
    assert.deepEqual(
      Object.keys(catalogs[locale].messages).sort(),
      keys,
      locale,
    );
    for (const key of keys as (keyof typeof catalogs.en.messages)[]) {
      const text = catalogs[locale].messages[key];
      assert.ok(text.trim(), `${locale}: ${key} is empty`);
      assert.deepEqual(
        placeholders(text),
        placeholders(catalogs.en.messages[key]),
        `${locale}: ${key}`,
      );
    }
  }
});

test("language matching supports regional tags and case differences", () => {
  assert.equal(matchLocale("RU-ru"), "ru");
  assert.equal(matchLocale(" en_US "), "en");
  assert.equal(matchLocale("de-DE"), undefined);
  assert.equal(matchLocale("constructor"), undefined);
});

test("registered languages have valid Intl locales and discoverable native names", () => {
  for (const locale of supportedLocales) {
    assert.doesNotThrow(() => new Intl.Locale(catalogs[locale].intlLocale));
    assert.ok(catalogs[locale].nativeName.trim());
    assert.ok(catalogs[locale].shortName.trim());
  }
});

test("an explicit supported URL language takes precedence over saved and browser languages", () => {
  assert.deepEqual(resolveLanguage("ru", "en", ["en-US"]), { locale: "ru", preference: "ru" });
});

test("a saved choice takes precedence over browser language", () => {
  assert.deepEqual(resolveLanguage(null, "en", ["ru-RU"]), { locale: "en", preference: "en" });
});

test("unknown URL and stored values do not prevent browser-language detection", () => {
  assert.deepEqual(resolveLanguage("xx", "zz", ["de-DE", "ru-RU", "en"]), {
    locale: "ru",
    preference: "auto",
  });
});

test("automatic selection follows the first supported language regardless of region", () => {
  assert.deepEqual(resolveLanguage(null, null, ["en-RU", "ru-US"]), {
    locale: "en",
    preference: "auto",
  });
  assert.deepEqual(resolveLanguage(null, null, ["ru-US", "en-RU"]), {
    locale: "ru",
    preference: "auto",
  });
});

test("unsupported or empty preferences safely fall back to English", () => {
  assert.deepEqual(resolveLanguage(null, null, ["de-DE"]), { locale: "en", preference: "auto" });
  assert.deepEqual(resolveLanguage(null, null, []), { locale: "en", preference: "auto" });
});

test("named interpolation translates labels without losing Unicode or parameter values", () => {
  assert.equal(
    createTranslator("ru")("demo.switch", { app: "Заметки" }),
    "Открыть окно Заметки",
  );
  assert.equal(
    formatMessage("Version {version}", { version: "0.5.1" }),
    "Version 0.5.1",
  );
  assert.equal(
    formatMessage("{count} / {total}", { count: 0, total: 5 }),
    "0 / 5",
  );
  assert.throws(
    () => formatMessage("Show {app}"),
    /Missing translation parameter/,
  );
});
