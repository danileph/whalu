# Whalu website

A local, static React + TypeScript landing page built with Vite. Four sections cover the introduction, product demo, controls, and installation. The fixed header and hero share a black background. The first screen contains the title and install action over a silver-blue window-light effect. The MacBook preview begins below the fold and is reached by scrolling or the Watch the demo button.

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

- The MacBook frame below the hero plays the recorded Whalu demo with animated shortcut, trackpad, and mouse overlays. The video keeps its original proportions inside the existing 16:10 screen, with native play/pause, seeking, and fullscreen controls. It is muted and loops while visible; it pauses offscreen or when the browser tab is hidden, preserves a visitor's manual pause, and does not autoplay with Reduce Motion enabled. The poster and metadata load before playback. Both files are served locally from `public/media/`.
- The controls guide uses three columns, stacked on phones. All 37 default keyboard bindings and the overview modifier hold stay visible in Navigate, Size & position, Stack & Spaces, and Arrange windows panels. Each direction has its own label and individual keycaps, with arrow and Vim-style alternatives shown together. A symbol legend and native tiling availability note accompany the list. On phones, labels sit above wrapping shortcut alternatives.
- Download buttons in the header, hero, and download section point to the latest GitHub Release’s universal ZIP. The download section pairs the Mac and GitHub buttons above compatibility and package details, release notes, and an expandable installation guide. The guide contains brief setup steps and a copyable Homebrew preview command marked as coming soon. Links default to `danileph/whalu`; `VITE_REPOSITORY_URL` overrides the repository for forks.
- The fixed header is 56px high on desktop and 52px on phones. It overlays the animated hero without reserved space, with a transparent background at the page top and a translucent background after scrolling. The video has a localized accessible label and native playback controls. A draggable, keyboard-accessible scroll thumb overlays the page without a visible track or reserved gutter; native scrollbars remain the fallback if initialization fails.
- Reloads start at the hero. Fresh shared section links still open their targets below the header, and browser Back/Forward restoration stays available.
- The page respects system Reduce Motion and offers a persistent footer preference. The hero backdrop animates only its transform, using SVG gradient fills and strokes without blur filters or masking. Motion pauses during scrolling and while offscreen, then resumes after scrolling stops. The preview entrance respects the shared motion preference. No packages or external assets are required for these SVG/CSS effects. There is no pinned scrolling.
- English and Russian are selected in the header. Switching languages preserves video playback position. See [LOCALIZATION.md](LOCALIZATION.md) for adding languages and preference behavior.
- The icon comes from the existing Whalu asset. The demo is a 74.7-second, 1920 × 1248, 30 fps H.264 recording with no audio and fast-start metadata for progressive playback. The MacBook shell remains CSS. App icons shown in the recording belong to their respective owners. No external fonts, runtime CDNs, analytics, or image services are used. Third-party license texts are in `public/THIRD_PARTY_NOTICES.txt`.

Colors are in `src/tokens.css`; layouts and demo styles are in `src/styles.css`. The video scales with the MacBook frame without cropping the recording or its input overlays.

## Verification

Run `npm run build` and `git diff --check`. Check desktop and phone layouts in both languages, video playback and seeking, manual pause preservation, offscreen pausing, reduced-motion playback, shortcut readability, reload position, section links, scrollbar dragging and keyboard controls, the copy button, and motion preference persistence. See [AUDIT.md](AUDIT.md) for the latest checks.

## GitHub Pages

The configured site URL is [danileph.github.io/whalu](https://danileph.github.io/whalu/). Check the [deployment workflow](https://github.com/danileph/whalu/actions/workflows/pages.yml) for publication status. The workflow in `.github/workflows/pages.yml` builds the static website, supplies the repository URL, and deploys to GitHub Pages on website changes pushed to `main` or a manual run on `main`. Set the repository’s Pages source to GitHub Actions. `develop` is checked by CI but never deployed to production. Relative Vite asset paths support the `/whalu/` repository subpath and custom domains.

The download URL uses GitHub’s stable `/releases/latest/download/Whalu-macOS-universal.zip` route. Release tags publish that fixed asset name, so the page needs no GitHub API requests, credentials, or hardcoded release version. The compatibility label describes supported systems and architectures.

To target another repository during local development, optionally add `website/.env.local` and restart Vite:

```dotenv
VITE_REPOSITORY_URL=https://github.com/YOUR-USERNAME/YOUR-REPOSITORY
```
