# Whalu landing page audit

Verified November 2, 2026 after the compact redesign. This supersedes the November 1 audit of the longer page. Scope is the static website; publication remains deferred.

## Structure and design

- Three sections: product demo, controls, and installation.
- Black header and hero with a shared content width. Desktop uses a split layout; tablet and phone layouts stack the content.
- Removed the pinned story, separate tiling demonstration, privacy pitch, FAQ, input-method tabs, oversized installation pitch, and their unused code and translations.
- Retained the five-window illustration, accessible selection status, detailed shortcuts in a native disclosure, command copying, permission instructions, language selection, and motion preference.
- English and Russian copy uses short instructions. The shortcut guide reflects the current arrow-key navigation instead of the removed same-app action.

## Verification

`npm run build` passes TypeScript, all eight localization tests, and the production build. `git diff --check` passes.

| Check              | Result                                                                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| Responsive widths  | No horizontal overflow at 320, 390, 768, 1024, 1440, and 4096 CSS pixels, including Russian copy.                                  |
| Visual review      | Reviewed desktop production layout and Russian phone layout. Narrow phones wrap demo selectors rather than truncating their names. |
| Window demo        | All five selector buttons update the window and live status. Home selects the first window; ArrowLeft wraps to the last.           |
| Language           | Switching to Russian preserves the selected window and updates labels and page metadata.                                           |
| Shortcuts          | Native disclosure expands and collapses; arrangement and Space-transfer shortcuts remain available.                                |
| Copy command       | Production build reports successful clipboard copying.                                                                             |
| Motion             | Footer preference changes the shared motion setting, persists after reload, and can be restored.                                   |
| Production console | No warnings or errors observed for the production build.                                                                           |

Changing translation modules during development can cause Vite Fast Refresh to invalidate the shared locale context; a full reload recovers it. Production verification used `vite preview`, independently of development hot reload. Touch gestures and the OS Reduce Motion setting were not emulated in this audit; their existing handlers remain in place.

The native macOS app bundle, dependencies, permissions, and installed executable are unaffected by this website-only change.
