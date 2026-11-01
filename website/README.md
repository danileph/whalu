# Whalu website

A static React + TypeScript landing page built with Vite. Its window wheel, keyboard controls, shortcut sheet, native-tiling illustration, FAQ, and command-copy button work entirely in the browser. The desktop and window content are illustrations, not recordings of the installed app.

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

- DM Sans and Instrument Serif are served locally through Fontsource packages under the SIL Open Font License. No Google Fonts requests, analytics, external image services, or runtime CDNs.
- Redistributed software and font license texts are included in `public/THIRD_PARTY_NOTICES.txt` and copied into the production build.
- `public/whalu-icon.png` is a resized copy of the existing Whalu brand asset. It is only used by the website.
- The landscape, miniature app windows, and input-device illustrations are authored in SVG, React, and CSS. CSS perspective supplies the wheel depth without WebGL or a continuous render loop.
- Colors are in `src/tokens.css`; responsive layouts and reduced-motion behavior are in `src/styles.css`.
- The hero supports thumbnail selection, previous/next buttons, arrow keys and J/K while focused, horizontal touch swipes, and wheel input over the thumbnail strip. Keyboard navigation wraps; buttons and wheel input stop at the ends. Page scrolling remains available.
- When changing the native app version or installation requirements, update the corresponding copy in `src/App.tsx`.

## Verification

Run `npm run build` and `git diff --check`. Check the page at desktop and mobile widths, select all five demo windows, expand the shortcut sheet and FAQ, try both tiling positions, and copy the install command. Verify keyboard focus and that the layout still works with reduced motion. Before deploying, serve `dist/` under a repository-style URL path to check assets and anchors.

Verified on 2026-11-01: production build and TypeScript checks pass, `npm audit` reports no vulnerabilities, and browser checks pass at desktop, 768px, 390px, and 320px widths. Checked window selection, keyboard wrapping and focus, the shortcut sheet, tiling controls, clipboard feedback, FAQ, and section anchors. The production build also loads its fonts and images and switches windows under `/whalu/`, with no browser console warnings or errors. Physical touch gestures and the eventual GitHub-hosted deployment have not been tested.
