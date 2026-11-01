# Whalu landing page audit

Audited and refined on November 1, 2026. Scope: the React website, preserving its Apple-inspired visual direction and local-only publication status.

## Resolved findings

| Area                | Finding                                                                                           | Change                                                                                                                                                |
| ------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Short windows       | The overview's 560px minimum height put chapter controls below a 390px-high viewport.             | Added compact landscape and small-phone layouts. Controls remain inside the visible scene.                                                            |
| Mobile navigation   | Overview and Controls links disappeared on phones.                                                | Added section links, Escape dismissal, outside-click dismissal, and close-on-navigation behavior.                                                     |
| Demo feedback       | The selected-window status used `display: none`, preventing live announcements.                   | Displayed the selection and counter in an atomic status region, with a named and described demo region.                                               |
| Touch targets       | Demo selector buttons were only 33px tall on phones.                                              | Increased demo and chapter buttons to at least 44px tall and retained narrow-screen fit.                                                              |
| Motion              | Only the system setting could disable the pinned experience.                                      | Added a persistent footer preference and a static layout showing every chapter. The system preference takes precedence.                               |
| Keyboard access     | The demo captured modified arrow shortcuts, and the skip target and tab panels weren't focusable. | Preserved modified browser shortcuts, made the main skip target and tab panels focusable, and added explicit control-group relationships.             |
| Input tabs          | Different descriptions could shift the card layout.                                               | Reserved panel height and centered the input-device artwork. All three mobile tab states measure the same card height.                                |
| Legibility          | Secondary text used a light gray with 3.33:1 contrast against the gray surface.                   | Darkened functional secondary text to 4.66:1. Inactive chapter controls now have 7.79:1 contrast against their dark background.                       |
| Finishing details   | Chapter text overlapped during long crossfades; controls lacked consistent hover feedback.        | Removed the overlapping text exit, shortened entry travel, added hover feedback, clarified demo instructions and hero copy, and refined hero spacing. |
| Earlier regressions | Wide-screen FAQ gutters and permanently hidden reveals had failed previously.                     | Rechecked capped FAQ gutters and visible-by-default content across the viewport matrix.                                                               |

## Browser verification

| Viewport    | Result                                                                                        |
| ----------- | --------------------------------------------------------------------------------------------- |
| 320 × 568   | No horizontal overflow; demo targets 44px; chapter controls end at y=556 inside the viewport. |
| 390 × 844   | Mobile menu, stable tabs, demo feedback, FAQ, and reduced-motion flow work.                   |
| 768 × 1024  | Tablet layout fits; images and control targets resolve.                                       |
| 844 × 390   | Chapter controls end around y=378 inside the viewport, replacing the previous y=593 overflow. |
| 1280 × 800  | Desktop layout, keyboard interaction, and static reduced-motion overview work.                |
| 2457 × 1270 | FAQ retains its 800px reading column; no horizontal overflow.                                 |
| 4096 × 1270 | FAQ remains 800px wide; no collapsed sections or horizontal overflow.                         |

Also checked:

- Mobile disclosure opening, Escape dismissal, and closing after following a section link.
- Reduced-motion activation, all chapter visibility, removal of pinning, persistence after reload, and restoring animations.
- Window selection announcements, keyboard wrapping, input-tab Home/End navigation, shortcut disclosure, tiling, clipboard feedback, and FAQ expansion.
- No missing images, unresolved `aria-controls` targets, or browser console warnings/errors.
- TypeScript and production build, whitespace checks, and npm dependency audit.
- Static output under a repository-style `/whalu/` URL before GitHub publication.

The horizontal touch-swipe path is implemented and code-reviewed; physical touch hardware was not available in the browser automation. System-level Reduce Motion changes are handled by the media-query listener; the shared reduced-motion layout was tested using the site control. These are targeted browser and source checks, not exhaustive device coverage.
