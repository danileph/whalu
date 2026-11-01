# Whalu website

A static React + TypeScript landing page built with Vite. Its scroll-driven window overview, interactive desktop, input-method tabs, shortcut sheet, native-tiling illustration, FAQ, and command-copy button work entirely in the browser. The desktop and window content are illustrations, not recordings of the installed app.

## Work locally

Use Node.js 22.12+ (Node 24 is used in CI).

```sh
cd website
npm ci
npm run dev
```

Open the printed local URL, usually `http://127.0.0.1:5173/`.

```sh
npm run check   # TypeScript
npm run build   # TypeScript + production build
npm run preview
```

The build is in `website/dist/`. Website dependencies and output are separate from the macOS app. The website doesn't change the native bundle or require macOS permissions.

## Publish on GitHub Pages later

1. Push this project to a GitHub repository with `main` as its default branch. If using a different branch, update `.github/workflows/pages.yml`.
2. In the repository's **Settings → Pages → Build and deployment**, select **GitHub Actions**.
3. Run **Publish Whalu website** from the Actions tab. Later pushes to `main` that change the website also deploy it.

The workflow builds only the static website. Relative Vite asset paths support both `username.github.io/repository/` and a custom domain. It automatically supplies the repository URL for source links. No repository URL, GitHub credentials, or hosted service is needed for local development. Nothing has been published yet.

To show repository links locally, optionally create `website/.env.local`:

```dotenv
VITE_REPOSITORY_URL=https://github.com/YOUR-USERNAME/YOUR-REPOSITORY
```

Restart Vite after changing environment variables. Without a repository URL, calls to action lead to the local-source installation instructions.

## Design and assets

- The design follows Apple's product-page visual language: white, near-black, neutral gray, blue controls, centered product typography, and generous spacing. The native system font stack uses San Francisco on macOS and appropriate system fallbacks elsewhere. No downloaded fonts, analytics, external image services, or runtime CDNs.
- Redistributed software license texts are included in `public/THIRD_PARTY_NOTICES.txt` and copied into the production build.
- `public/whalu-icon.png` is a resized copy of the existing Whalu brand asset. It is only used by the website.
- The wallpaper, laptop, miniature app windows, and input-device illustrations are authored in SVG, React, and CSS. CSS perspective supplies the wheel depth without WebGL.
- `useScrollMotion` reads scroll position at most once per animation frame and writes CSS variables without rendering React on every scroll. The pinned overview separates windows into a wheel and moves focus between them. Section reveals use IntersectionObserver and the Web Animations API; content is visible by default, so missed observations and full-page captures cannot leave blank sections. The hero and native-tiling display respond to scroll position too. No artificial scrolling, wheel interception, or continuously running animation loop.
- Colors are in `src/tokens.css`; responsive layouts and reduced-motion behavior are in `src/styles.css`.
- The hero supports thumbnail selection, app selector buttons, horizontal touch swipes, arrow keys and J/K while focused, and wheel input over the thumbnail strip. Keyboard navigation wraps; wheel input stops at the ends. Modified browser shortcuts and vertical page scrolling remain available. The current selection is visible and announced through a status region.
- The system Reduce Motion setting disables animated transitions and reveals, removes pinning, and lays out all overview chapters as static readable content. The footer also offers a persistent local preference, respecting the system setting. `useMotionPreference` applies both paths through the same layout rules.
- Mobile navigation has an expandable section menu with Escape and outside-click dismissal. Input-method tabs support arrow keys and Home/End, and their panels keep a stable height.
- When changing the native app version or installation requirements, update the corresponding copy in `src/App.tsx`.

## Verification

See [AUDIT.md](AUDIT.md) for the polish audit, resolved issues, and verification matrix. Include short landscape windows in layout checks, and use the footer motion control to exercise the static alternative. Restore animations after testing.

Run `npm run build` and `git diff --check`. Check the page at desktop and mobile widths. Scroll through the three overview chapters in both directions and try their navigation buttons. Select all five demo windows, switch input-method tabs, expand the shortcut sheet and FAQ, try all three tiling positions, and copy the install command. Verify keyboard focus and the Reduce Motion fallback. Before deploying, serve `dist/` under a repository-style URL path to check assets and anchors.

The site remains local. GitHub publication is deferred until a repository is available.

Wide-display regression checks: at 2457px, 3332px, and 4096px viewport widths, the FAQ retains its 800px reading column and capped gutters. At 390px, it fits the available mobile width. Checked FAQ expansion, native-tiling controls, visible offscreen reveal content, and absence of horizontal overflow and console errors. The FAQ uses its own bounded padding instead of the full-width sections' viewport-based gutters.

Verified on 2026-11-01: TypeScript and production builds pass, `npm audit` reports no vulnerabilities, and browser checks cover desktop, tablet, phone, short landscape, and ultrawide layouts. The production build also loads and switches windows under `/whalu/`, with no console warnings or errors. Reduced-motion layouts and preference persistence are exercised through the footer control. System preference changes use the same layout path; OS preference emulation was unavailable in the browser tools. GitHub deployment remains untested until publication.
