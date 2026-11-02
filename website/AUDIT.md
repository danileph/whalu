# Whalu landing page audit

Verified November 2, 2026 after the stacked-hero and shortcut refinements. This supersedes the November 1 audit of the longer page. Scope is the static website; publication remains deferred.

## Structure and design

- Three sections: product demo, controls, and installation.
- Fixed black header with anchor offsets. The centered hero title sits above a restored MacBook bezel, camera notch, and aluminum base at every screen size.
- Removed the pinned story, separate tiling demonstration, privacy pitch, FAQ, input-method tabs, oversized installation pitch, and their unused code and translations.
- Retained the five-window illustration, accessible selection status, always-visible shortcuts in contrasting Navigate and Arrange panels, command copying, permission instructions, language selection, and motion preference.
- English and Russian copy uses short instructions. The shortcut guide reflects current arrow-key navigation and includes both Command + Tab and Command + grave accent.

## Verification

`npm run build` passes TypeScript, all eight localization tests, and the production build. `git diff --check` passes.

| Check              | Result                                                                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| Responsive widths  | No horizontal overflow at 320, 390, 768, 1024, 1440, and 4096 CSS pixels, including Russian copy.                                  |
| Visual review      | Reviewed desktop production layout and Russian phone layout. Narrow phones wrap demo selectors rather than truncating their names. |
| Window demo        | All five selector buttons update the window and live status. Home selects the first window; ArrowLeft wraps to the last.           |
| Language           | Switching to Russian preserves the selected window and updates labels and page metadata.                                           |
| Shortcuts          | All six shortcut rows are visible without interaction. Keycaps and longer Russian labels fit at every tested width.                |
| Fixed header       | Remains at y=0 while scrolling. The desktop Controls link clears the 81px header with a 104px anchor offset.                       |
| Copy command       | Production build reports successful clipboard copying.                                                                             |
| Motion             | Footer preference changes the shared motion setting, persists after reload, and can be restored.                                   |
| Production console | No warnings or errors observed for the production build.                                                                           |

Changing translation modules during development can cause Vite Fast Refresh to invalidate the shared locale context; a full reload recovers it. Production verification used `vite preview`, independently of development hot reload. Touch gestures and the OS Reduce Motion setting were not emulated in this audit; their existing handlers remain in place.

The native macOS app bundle, dependencies, permissions, and installed executable are unaffected by this website-only change.
