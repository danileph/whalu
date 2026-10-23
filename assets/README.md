# Whalu brand assets

Whalu uses a graphite glass tile, three staggered frosted-silver window panes, and a thin silver orbit, based on the supplied visual references.

- `Whalu.png`: 1024×1024 app-icon master with transparent outer margins.
- `Whalu.icns`: macOS icon containing standard and Retina representations from 16 to 1024 pixels. Used by the app bundle and setup window.
- `Whalu-wordmark.png`: horizontal icon and silver Whalu wordmark on charcoal, used in the project README.

Regenerate the ICNS after replacing the PNG master:

```sh
./scripts/build-icon.sh
```

This uses macOS `sips` and `iconutil`. `./scripts/build-app.sh` packages the committed ICNS without regenerating artwork or requiring image-generation access.

The icon and wordmark were made with the built-in `image_gen` tool. The generated master was resized for packaging; its alpha channel is retained. The following prompts document the artwork. The original specification PDF and historical validation entries retain the former project name.

## Icon generation prompt

> Use case: logo-brand. Asset type: production macOS application icon for Whalu. Use the supplied image as visual reference. Create a very faithful, polished rendition of this exact concept: dark graphite rounded-square glass tile with three overlapping, diagonally staggered frosted silver glass window panes and a slender elliptical silver orbit wrapping around them. Same perspective, proportions, monochrome silver and charcoal palette, bright fine glass edges, soft controlled reflections. Single icon, centered, straight-on rounded tile. Transparent background OUTSIDE the rounded-square tile; the graphite tile interior stays dark and opaque. Tile occupies roughly 82% of square canvas with even transparent margin, macOS app icon silhouette. No surrounding black rectangular background. No text, letters, mockup, dock, grid, extra icon or watermark. Preserve readability at small sizes. Deliver one square high-resolution PNG.

## Final icon refinement prompt

> Use case: precise-object-edit. Edit target: attached Whalu app icon. Preserve the graphite tile, three frosted silver panes, ellipse orbit, lighting and layout exactly. Fix ONLY the silhouette: remove all stray white flecks, colored artifacts, noisy halo, and debris above and outside the rounded tile. Make its outer edge one smooth, crisp continuous rounded square with subtle antialiasing. Outside the tile must be entirely transparent with no shadow haze or isolated pixels. Inside the tile must stay fully opaque. Center the tile with balanced transparent margins, tile width 82% of square canvas. No text. Production macOS icon PNG with flawless clean alpha edges.

## Wordmark generation prompt

> Use case: logo-brand. Asset type: wide horizontal README brand banner. Input image 1 is the final Whalu icon, preserve it exactly. Input image 2 is the wordmark style and layout reference. Create ONLY the top icon-and-wordmark lockup, with no lower examples, no divider line and no dock. Place the exact icon from input 1 on the left, and text verbatim 'Whalu' on the right. Large rounded geometric sans serif letters with restrained silver-white glass reflections, like reference. Text W-h-a-l-u must be spelled correctly, only initial W uppercase. Near-black charcoal background with subtle diffuse light at upper left. Wide landscape composition about 3:1, balanced generous margin, icon roughly same visual height as wordmark. Production crisp minimal brand asset, no subtitle, no extra objects, no watermark. Icon and wordmark must match reference proportions and monochrome graphite / frosted silver identity.
