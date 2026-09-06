# Icon artwork

The [tray mascot](tray-icon.png) adapts the blue pants and round glasses from the
[GitHub logo](smartypants.png). The transparent 1254 × 1254 PNG is the master;
[app-icon.png](app-icon.png) is the 256 × 256 launcher export used by the binary
bundles and RPM. The GitHub logo remains the README artwork.

The daemon embeds RGBA8 exports at 16, 20, 22, 24, 32, 48, and 64 pixels under
`crates/daemon/assets/tray/`. It converts them once to the StatusNotifierItem ARGB
byte order. Both normal and attention states use this mascot. An installed icon
theme or an image decoder is not required.

To regenerate the exports after replacing the master, install ImageMagick 7 and
run from the repository root:

```sh
scripts/render-tray-icons.sh
```

ImageMagick is a maintainer tool only; normal builds use the committed exports.

## Artwork provenance

Created on September 6, 2026 using the built-in image-generation tool with
`smartypants.png` as the reference. Only resizing and format conversion were used
after generation. The generation prompt follows, with the reference path written
relative to the repository:

```text
Use case: logo-brand.
Asset type: production Linux system-tray icon for Smarty Pants, a writing assistant.
Reference image: docs/assets/smartypants.png supplies the existing mascot identity, blue denim palette and round glasses. Create a NEW simplified small-icon adaptation, not a screenshot or a complete logo lockup.
Primary request: one charming, polished pair of blue pants wearing oversized round nerd glasses, recognizably related to the reference. Front-facing, centered, almost filling a square canvas with only a small even transparent margin. Make the two separate pant legs and the glasses readable at 16–24 pixels. Use a few bold flat shapes, chunky clean dark-navy contours, bright medium denim-blue pants, pale warm-cream lens interiors and minimal highlights. Give the glasses especially clear bold rims and a short bridge. The pants silhouette should have a broad waistband and two distinct short legs with a generous transparent gap. No human head, face, eyes, arms or feet; the glasses sit across the top of the pants. Remove the pencil, floating checks, writing marks, fine stitching and pocket decoration from the reference: this is a tiny tray glyph. No gradients, texture, lighting effects, shadow, badge, border tile, lettering, words or watermark.
Background: actual transparent alpha around the isolated mascot, including the gap between the legs. No white background and no checkerboard baked into the pixels. Produce a single square icon, not a sheet of variations.
```
