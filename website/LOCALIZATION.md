# Website localization

The site ships with English and Russian. The header's native language selector works with a keyboard and screen reader, displays each language's own name, and switches content without remounting the demos. All interface copy, accessible labels, illustrative window content, and page metadata live in `src/i18n/messages/`.

## Add a language

1. Copy `src/i18n/messages/en.ts` to a new catalog, export it under a new name, and use `satisfies Messages` as in `ru.ts`. English defines the message-key schema. TypeScript reports missing or unknown keys.
2. Translate every value, preserving named placeholders such as `{app}` and `{version}`. Keep placeholders named so sentences can change word order. Brands, keyboard symbols, filenames, commands, and actual source-code examples retain their original spelling.
3. Import the catalog in `src/i18n/catalogs.ts` and add one entry with its language tag, `nativeName`, `shortName`, `intlLocale`, `direction`, and `messages`. Use lowercase BCP 47 language tags, such as `de` or `pt-br`.
4. Run `npm run build`. Check the new language at phone, tablet, and desktop widths, including every interactive state. Right-to-left languages can set `direction: "rtl"`, but need their own layout review; the initial languages are both left-to-right.

The locale type, selector options, language matching, and catalog tests derive from the registry. Components require no new language branches. Use semantic, stable IDs for list keys so translated text never resets component state.

## Use translated content

```tsx
const { t, formatNumber, formatDate } = useLocale();

t("hero.learn");
t("demo.switch", { app: t("apps.notes.name") });
formatNumber(5, { minimumIntegerDigits: 2 });
formatDate(date, { day: "numeric", month: "long" });
```

`Translator` checks message keys and required interpolation arguments at compile time. Catalog tests also compare placeholder names across languages and reject empty values. Text is rendered through React, without injected HTML. A runtime missing-key fallback uses English. Date and number formatting use the registry's Intl locale; illustrative dates default to UTC to stay consistent across time zones.

The current catalog uses plain messages and named interpolation. If future content needs plurals, add a plural-aware formatter and tests rather than concatenating translated sentence fragments.

## Language preference and links

The initial choice follows this order:

1. A supported `?lang=` URL parameter.
2. The saved `whalu-language` local-storage preference.
3. The first supported language in `navigator.languages`.
4. English.

Regional variants are matched to supported parent tags (`ru-RU` matches `ru`). Unknown values fall through to the next preference. Storage failures do not prevent rendering or switching languages.

Selecting a language saves it and replaces the current URL's `lang` parameter while preserving the path, other query parameters, and section hash. For example, `/whalu/?lang=ru#controls` works on GitHub Pages without server routing. Browser history navigation re-resolves the language. A URL override alone does not overwrite a saved preference; changing the selector does.

The provider updates `html.lang`, `html.dir`, the title, description, and Open Graph title, description, and locale. This is a static client-rendered site: HTML and social previews for crawlers that do not execute JavaScript remain English. Localized server-rendered previews and search-engine language routes would require a separate prerendering step.

## Verification

`npm test` checks registry completeness, placeholders, locale matching, preference precedence, fallback behavior, Intl configuration, and interpolation. `npm run check` checks component usage and catalog types. `npm run build` runs both before producing static assets.

Browser verification covers switching and reloading, shared language URLs, translated metadata and accessible controls, demo-state preservation, copy feedback, and responsive layouts. Check longer translations without truncating essential text. Legal third-party license notices remain in their original language.
