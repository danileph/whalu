# Whalu landing page audit

Verified November 2, 2026 after the native-style desktop preview update. This supersedes the November 1 audit of the longer page. Scope is the static website; publication remains deferred.

## Structure and design

- Four sections: introduction, product demo, controls, and installation.
- Fixed black header with anchor offsets. The text-only first screen uses a animated silver-blue window-light backdrop. The MacBook illustration is in a separate preview section below the fold, reached by scrolling or How it works.
- Removed the pinned story, separate tiling demonstration, privacy pitch, FAQ, input-method tabs, oversized installation pitch, and their unused code and translations.
- Retained the five-window illustration, accessible selection status, always-visible shortcuts in contrasting Navigate and Arrange panels, command copying, permission instructions, language selection, and motion preference.
- English and Russian copy uses short instructions. The shortcut guide reflects current arrow-key navigation and includes both Command + Tab and Command + grave accent.

The hero retains its transform animation, with no SVG blur filters or CSS masks. It pauses during scrolling and offscreen, resuming 180ms after scrolling stops. These changes address the reported hero lag while preserving motion. Visual browser checks do not establish frame rate or energy usage on the user’s hardware.

The preview uses real application icons, flat window stacking, active/inactive title bars, mouse-driven title-bar dragging, Dock selection, and selection-relative Whalu thumbnail transforms. Native wheel dimensions and perspective constants are taken from `src/overview.rs`; the demo uses five illustrative windows on a scaled logical desktop, not a live macOS session.

## Verification

`npm run build` passes TypeScript, all eight localization tests, and the production build. `git diff --check` passes.

| Check               | Result                                                                                                                                                                                                                               |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Responsive widths   | No horizontal overflow at 320, 390, 768, 1024, 1440, and 4096 CSS pixels, including Russian copy.                                                                                                                                    |
| Visual review       | Reviewed desktop production layout and Russian phone layout. Narrow phones wrap demo selectors rather than truncating their names.                                                                                                   |
| Window demo         | Thumbnail buttons select windows and update a screen-reader-only live status. The external selector row, count, and helper copy have been removed. Home selects the first window; ArrowLeft wraps to the last.                       |
| Language            | Switching to Russian preserves the selected window and updates labels and page metadata.                                                                                                                                             |
| Shortcuts           | All six shortcut rows are visible without interaction. Keycaps and longer Russian labels fit at every tested width.                                                                                                                  |
| Hero and reveal     | At 1280 × 900, the hero ends and the preview begins at y=900. How it works lands the preview below the fixed header at y=76. The preview remains interactive.                                                                        |
| Fixed header        | Remains at y=0 while scrolling; transparent at the top, translucent after scrolling. Header measures 56px on desktop and 52px on phones. The desktop Controls link lands at y=76, below the fixed header.                            |
| Copy command        | Production build reports successful clipboard copying.                                                                                                                                                                               |
| Motion              | Footer preference changes the shared motion setting, persists after reload, and can be restored.                                                                                                                                     |
| Overlay scrollbar   | No reserved gutter at 320, 390, 768, and 1280px. Dragging moves the viewport; End reaches the bottom and updates the accessible value to 100. Native scrolling remains available through wheel, touch, and page keyboard input.      |
| Reload and anchors  | Reloading from a section anchor or the page bottom returns to y=0, including Russian on a phone. Fresh shared section links resolve after React mounts, with the fixed-header offset.                                                |
| Desktop interaction | All five Dock buttons select and raise the corresponding window. Dragging the Terminal title bar changes its position and preserves it after switching focus. Wheel input over thumbnails changes selection without moving the page. |
| Desktop assets      | All real app icons load locally. Logical desktop scaling and absence of horizontal page overflow checked from 320 to 1440px, including Russian.                                                                                      |
| Production console  | No warnings or errors observed for the production build.                                                                                                                                                                             |

Changing translation modules during development can cause Vite Fast Refresh to invalidate the shared locale context; a full reload recovers it. Production verification used `vite preview`, independently of development hot reload. Touch gestures and the OS Reduce Motion setting were not emulated in this audit; their existing handlers remain in place.

The native macOS app bundle, dependencies, permissions, and installed executable are unaffected by this website-only change.
