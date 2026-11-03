# Whalu landing page audit

Verified November 3, 2026 after completing the shortcut guide. Scope is the static website; publication remains deferred.

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

The native macOS app bundle, dependencies, permissions, and installed executable are unaffected by this website-only change.
