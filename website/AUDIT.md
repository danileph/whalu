# Whalu landing page audit

Verified November 3, 2026 after combining the English and Russian copy review with the complete shortcut guide. The existing product video and layout remain in place. GitHub distribution results are recorded below.

## Copy and localization

- Reworked the headlines, descriptions, navigation, control instructions, shortcut labels, installation guide, permission explanations, copy feedback, motion settings, and accessible labels. Each language uses its own phrasing and consistent terms. Standard app names and familiar interface labels remain unchanged.
- Split the arrow shortcuts into window navigation (up/down) and navigation within the current app (left/right). The instructions now explain immediate switching and overview dismissal rather than incorrectly promising a switch on release.
- Kept the initial HTML title, description, and Open Graph copy aligned with the English catalog. Removed the outdated interactive-demo wording from the JavaScript fallback.
- Production browser checks covered English and Russian at 320, 390, 768, and 1280 CSS pixels: no horizontal page overflow or overlapping shortcut labels. Reviewed the English controls and Russian installation layout visually, confirmed both languages’ copy-success feedback, and checked language switching and localized metadata. No browser console errors or warnings were recorded.
- `npm run build` passes TypeScript, all eight localization tests, and production asset generation. `git diff --check` passes.

## Shortcut guide

- All 37 default keyboard bindings were compared against `Config::default` in `src/config.rs`, including Command + Tab/grave, arrows, H/J/K/L aliases, same-app navigation, stack edits, Space transfers, all four halves, fill, center, restore, and all four arrangements. No bindings are missing or duplicated.
- The overview modifier hold has its own row. The guide has 22 rows across four always-visible groups, with 38 rendered key combinations in each language. The native Fn + Control + R restore alternative is described below the guide.
- Each direction has a separate label. A key-symbol legend and native Window-menu support note are provided in English and Russian. The keyboard and trackpad instructions describe immediate switching, and mouse instructions include side-button navigation and thumbnail reordering.
- Production browser checks passed in both languages at 320, 390, 850, and 1280 CSS pixels. Every width renders the complete list without horizontal page overflow or clipped shortcut rows/key combinations. Visual review covered English desktop and Russian narrow-phone layouts, and language switching updates the guide.
- `npm run build` passes TypeScript, all eight localization tests, and the production Vite build. `git diff --check` passes.

## Video and frame

- The existing CSS MacBook display, camera notch, bezel, and base remain in place. Its 16:10 screen contains the full 1920 × 1248 recording without cropping or stretching.
- The 74.7-second H.264 MP4 includes the corrected Command–Tab / Command–backtick cues and vertical held-mouse dragging animation. The bundled MP4 and poster match the reviewed v2 export.
- Native controls provide playback, seeking, and fullscreen. The recording is muted, loops, and plays inline. It uses a local poster and metadata preloading.
- Playback starts when the video is visible, pauses offscreen or while the document is hidden, and preserves a visitor's manual pause. Reduce Motion prevents autoplay and still allows manual playback.
- The former desktop illustration, synthetic windows, window dragging, Dock selection, and related CSS have been removed.

## Verification

- `npm run build` passes TypeScript, all eight localization tests, and the production Vite build. `git diff --check` passes.
- Production browser review confirmed the 74.7-second video loads at 1920 × 1248, with native controls, muted inline playback, and `object-fit: contain` inside the MacBook.
- Checked English desktop layout at 1280 × 900 and Russian phone layout at 390 × 844. No horizontal page overflow at 320, 390, or 1280 CSS pixels.
- Checked normal playback, manual pause persistence after leaving and returning to the preview, and no autoplay after reloading with Reduce Motion enabled.
- Language switching updates the video label without replacing the video element or resetting its playback position.
- The local video URL supports byte-range requests: a 1 KiB request returns HTTP 206 with 1 KiB of data.

The copy changes do not alter native behavior. The required release-app reinstall and permission verification are recorded below.


## Required native installation check

- Rebuilt the release bundle with `./scripts/build-app.sh` and reinstalled `~/Applications/Whalu.app` after stopping the old process. The prior bundle, configuration, and state are preserved at `~/Library/Application Support/Whalu/rollback/20261103-180530-website-copy/`.
- Strict signature verification and byte comparison of the installed and freshly built executable pass (SHA-256 `c2c254586eeaa879b374e22f0a46c1d0b04fd145141491106f91045d3393722f`).
- Reset only Whalu’s Accessibility, ScreenCapture, and ListenEvent registrations. Confirmed stale entries were gone, re-added the installed bundle in all three panes, and verified each fresh entry enabled. The user completed the required Touch ID authentication. Whalu was stopped throughout registration; other applications’ grants were preserved.
- The LaunchServices-launched installed GUI native smoke test captured real content from every overlapping fixture window, confirming preview authorization. Native focus, live selection, tiling/restore, adjacent-Space transfer, event-tap creation, and non-key overview rendering passed. The later keyboard-fixture check stopped at the previously documented `No keyboard fixture focus` failure; the complete native suite is not claimed as passed.
- Relaunched normal Whalu through LaunchServices. Its own startup log reports `Whalu running` and one active trackpad listener. Final strict signature verification and comparison with the freshly built executable pass. Activation and real window-preview access are complete.


## Integration into main

- Preserved the complete shortcut guide from `a8abcf6` while integrating the copy review from `2af4387`. All shortcut definitions match the existing guide exactly; labels and descriptions use the revised terminology, including side-button navigation, preview dragging, native Window-menu support, and the Fn + Control + R restore alternative.
- The merged website passes `npm run build` and all eight localization tests. Production browser checks in both languages at 320, 850, and 1280 CSS pixels confirm 22 shortcut rows and 38 key combinations, with no clipped rows/combinations or horizontal page overflow.
- Rebuilt and reinstalled the release app from the main checkout. The previous bundle, configuration, and state are preserved at `~/Library/Application Support/Whalu/rollback/20261103-184905-merge-copy-main/`. Strict signature verification and executable comparison pass (SHA-256 `019383ad18078186f0a51f6d17a373e6712c7fbf02f6c09681c34a02f04175e6`).
- Reset only Whalu’s Accessibility, ScreenCapture, and ListenEvent registrations, confirmed the stale entries were removed, and re-added the installed bundle with fresh enabled entries in all three panes. The macOS authentication prompt was completed before registration. Whalu was stopped throughout; other applications’ grants were preserved.
- The installed GUI native smoke test captured real screenshot content for every overlapping fixture and passed enumeration, focus, live selection, and native left-half/restore checks. It then stopped at `Center retained the filled rectangle`; the full native suite is not claimed as passed. No native application code changed in this merge.
- Normal Whalu was relaunched through LaunchServices and reports `Whalu running` with one active trackpad listener. Final strict signature verification and installed/build executable comparison pass; activation and window-preview access are complete.


## GitHub release downloads and publication setup

- Added direct latest-release download buttons to the header, hero, and installation guide. All three use the fixed `Whalu-macOS-universal.zip` asset name. The guide includes release notes, installation steps, an Apple-notarization disclosure, and expandable source-build instructions. Both languages use the same destination without querying GitHub’s API or displaying a hardcoded release version.
- Local production review at 320, 390, and 1280 CSS pixels in English and Russian confirms three release-download links and no horizontal page overflow. The source disclosure opens and the install command’s copy feedback works.
- `npm run build` passes TypeScript, all eight localization tests, and Vite production output. Rust formatting, all 108 tests, and strict Clippy pass. Workflow YAML parses; shell syntax and invalid-argument/version guards pass.
- The universal release package builds both architectures with a macOS 15 deployment target, verifies the archive after extraction, and produces a SHA-256 checksum. Stable tag releases require matching Cargo/bundle versions and a commit already in `main`. CI checks `develop` and `main`; Pages deploys only from `main`.
- Reinstalled the locally built universal app after stopping Whalu, preserving the previous bundle/configuration/state at `~/Library/Application Support/Whalu/rollback/20261103-190901-github-releases/`. Reset only Whalu’s Accessibility, ScreenCapture, and ListenEvent registrations, confirmed stale entries absent, and re-added the installed bundle with fresh enabled entries. Strict signature and installed/build executable comparison pass.
- The installed GUI captures real screenshot content for every overlapping fixture. Native focus, live selection, tiling/restore, Space transfer, event-tap creation, and non-key overview rendering pass; the combined suite later stops at the previously documented `No keyboard fixture focus`. Normal Whalu was relaunched through LaunchServices and reports `Whalu running` with one active trackpad listener. Real preview access and activation are complete.


## Public GitHub distribution verification

- Published the existing history to `danileph/whalu`, with `develop` as the GitHub default and local working branch. Both branches track `origin`; `main` publishes production changes. HTTPS fetches use the requested repository URL; pushes use the existing account’s SSH key via GitHub’s port-443 endpoint after verifying the host fingerprint.
- The `v0.5.1` release workflow passed on GitHub and published `Whalu-macOS-universal.zip` plus its checksum. Downloaded the ZIP and checksum through the same public latest-release route used by the landing page: checksum verification, extracted signature verification, both architecture checks, and bundle version `0.5.1` all pass.
- Development CI passed its complete website and macOS checks. The initial main CI’s website passed; GitHub could not assign its ARM Mac runner and canceled that job with a capacity annotation. A fresh main check was dispatched. The separate Intel release workflow passed all Rust checks and both release builds.
- GitHub Pages is enabled with Actions as the build source and HTTPS enforced. Site builds pass. The first deployment timed out in `updating_pages`; its failed-job rerun became queued without jobs and GitHub refused force cancellation with HTTP 409. Fresh deployment attempts and a reset of the newly created Pages configuration were attempted. At this verification point the URL still returned HTTP 404, so public site availability is not claimed. A fresh documentation commit supplies a new deployment version for the next attempt.
