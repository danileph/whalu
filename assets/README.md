# Whalu brand assets

Whalu uses a dark graphite panel with a centered, compact symbol of three frosted-silver window panes and a thin silver orbit.

- `Whalu.png`: 1024×1024 opaque, square app-icon master. The graphite background reaches all four edges; the panes and orbit occupy about 56% of the width.
- `Whalu.icns`: macOS icon containing standard and Retina representations from 16 to 1024 pixels. Used by the app bundle and setup window.
- `Whalu-wordmark.png`: horizontal icon and silver Whalu wordmark on charcoal, used in the project README.

Keep the app-icon master fully opaque and square, without an inset rounded tile, external margin, or pale backing. macOS applies the icon mask. The previous transparent artwork caused macOS to place the icon inside an additional pale rounded square. See [Apple’s app-icon guidance](https://developer.apple.com/design/human-interface-guidelines/app-icons).

Regenerate the ICNS after replacing the PNG master:

```sh
./scripts/build-icon.sh
```

This uses macOS `sips` and `iconutil`. `./scripts/build-app.sh` packages the committed ICNS without regenerating artwork or requiring image-generation access.

The icon and wordmark were refined with the built-in `image_gen` tool using the earlier Whalu artwork as the edit target. The generated icon was resized for packaging. The prompts below describe the current assets; the earlier artwork and prompts remain in Git history. The original specification PDF and historical validation entries retain the former project name.

## Icon refinement prompt

> Use case: precise-object-edit. Edit target: attached Whalu icon. Produce a polished production macOS icon texture. Make TWO explicit changes: 1) scale the ENTIRE silver symbol (all three panes AND orbit together) down to 72% of its current size, preserving its proportions, perspective and optical centering; give it generous DARK breathing room inside the panel. 2) Replace the enclosing rounded-square tile and transparent outer margins with a SINGLE FULL-BLEED OPAQUE CHARCOAL BACKGROUND covering EVERY pixel of the square canvas INCLUDING ALL FOUR CORNERS. The dark panel is the entire image now, with NO visible inset tile boundary, NO frame and NO rounded corners baked into the image. macOS will round the actual app icon itself. The only objects are the smaller centered three frosted silver window panes and slender silver orbit, on a refined very subtle black/graphite gradient. Preserve the beautiful material concept and spatial arrangement of the panes. Softer controlled lighting, crisp clean edges without noise, no glare bloom. Background is nearly black across the whole canvas, slightly lighter upper-left, not light gray. NO white background, NO white border, NO white padding, NO transparency, NO external drop shadow, NO isolated pixels, NO text or mockup. Square output, full bleed dark graphite.

## Wordmark refinement prompt

> Use case: precise-object-edit. Edit target: image 1, the Whalu horizontal wordmark banner. Image 2 is the newly corrected app icon artwork. Preserve image 1's exact text 'Whalu', typography, text scale, layout and background. Change ONLY the app icon at left to match image 2: same three silver glass panes plus orbit at their SMALLER scale, with much more charcoal breathing room around the silver shapes inside the dark tile. Keep the tile same size and position in the banner; its dark rounded-square silhouette is appropriate here for the marketing image, with a very fine subtle dark glass edge, NO heavy white border or pale plate. Render the inset symbol at approximately 56% of the tile width. Use the material, perspective and smaller proportions of image 2. No additional text, no layout changes, no white padding, no extra objects.
