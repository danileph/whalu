# Whalu landing page audit

Verified November 3, 2026 after reviewing the English and Russian copy. The existing product video and layout remain in place; publication remains deferred.

## Copy and localization

- Reworked the headlines, descriptions, navigation, control instructions, shortcut labels, installation guide, permission explanations, copy feedback, motion settings, and accessible labels. Each language uses its own phrasing and consistent terms. Standard app names and familiar interface labels remain unchanged.
- Split the arrow shortcuts into window navigation (up/down) and navigation within the current app (left/right). The instructions now explain immediate switching and overview dismissal rather than incorrectly promising a switch on release.
- Kept the initial HTML title, description, and Open Graph copy aligned with the English catalog. Removed the outdated interactive-demo wording from the JavaScript fallback.
- Production browser checks covered English and Russian at 320, 390, 768, and 1280 CSS pixels: no horizontal page overflow or overlapping shortcut labels. Reviewed the English controls and Russian installation layout visually, confirmed both languages’ copy-success feedback, and checked language switching and localized metadata. No browser console errors or warnings were recorded.
- `npm run build` passes TypeScript, all eight localization tests, and production asset generation. `git diff --check` passes.

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
