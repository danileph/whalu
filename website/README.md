# Whalu website

A local, static React + TypeScript landing page built with Vite. Three sections cover the product demo, controls, and installation. The header and hero share a black background; tablet and phone layouts stack the hero content above the demo.

## Work locally

Use Node.js 22.12+ (Node 24 is used in CI).

```sh
cd website
npm ci
npm run dev
```

Open the local URL printed by Vite.

```sh
npm run check   # TypeScript
npm test        # Localization checks
npm run build   # Checks, tests, and production build
npm run preview
```

Output is in `website/dist/`. Website dependencies and assets are separate from the macOS app. Website-only changes do not alter the native bundle or its permissions.

## Design and interaction

- The hero contains an interactive illustration, not a recording of the installed app. Choose any of five windows, click a thumbnail, swipe horizontally, or focus the demo and use arrows, J/K, Home, or End. Keyboard selection wraps; wheel input over the thumbnails stops at the ends. Vertical page scrolling remains available.
- The controls guide uses three short columns, stacked on phones. A native disclosure holds the detailed shortcuts, including native tiling and Space transfers.
- Installation retains the source-build command, copy feedback, requirements, and macOS permission instructions. Repository links appear when `VITE_REPOSITORY_URL` is configured; otherwise the instructions refer to the local source folder.
- The page respects system Reduce Motion and offers a persistent footer preference. There is no pinned scrolling or scroll-driven animation.
- English and Russian are selected in the header. Switching languages preserves demo state. See [LOCALIZATION.md](LOCALIZATION.md) for adding languages and preference behavior.
- The icon comes from the existing Whalu asset; the illustrated desktop and app windows use SVG, React, and CSS. No external fonts, runtime CDNs, analytics, or image services are used. Third-party license texts are in `public/THIRD_PARTY_NOTICES.txt`.

Colors are in `src/tokens.css`; layouts and demo styles are in `src/styles.css`. Preview responsiveness uses container queries, so the illustration adapts to its width inside the desktop hero.

## Verification

Run `npm run build` and `git diff --check`. Check desktop and phone layouts in both languages, all demo selections, keyboard wrapping, shortcut expansion, the copy button, and motion preference persistence. See [AUDIT.md](AUDIT.md) for the latest checks.

## GitHub Pages

Publication remains deferred. The workflow in `.github/workflows/pages.yml` builds the static website and supplies repository links. Enable GitHub Actions in the repository’s Pages settings before publishing. Relative Vite asset paths support repository subpaths and custom domains.

For repository links during local development, optionally add `website/.env.local` and restart Vite:

```dotenv
VITE_REPOSITORY_URL=https://github.com/YOUR-USERNAME/YOUR-REPOSITORY
```
