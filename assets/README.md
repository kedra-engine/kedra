# Kedra authored editor assets

`editor/` contains source assets and stable native Bevy `.meta` sidecars. They are
not included in Rust executables. Use the shared engine importer described in
[native startup](../docs/startup.md). Generated deliveries belong outside this
source tree and are not checked into Git.

## Provenance

- SVG: owner-supplied Kedra Figma UI kit, file `2EZlK40cfx2Mub7GmDyI70`, folder
  `10:2422`, search `10:2418`, warning `10:2454`, spinner `10:2502`. Artwork is
  downloaded from the supplied design, not redrawn or replaced with glyphs.
  Window glyphs use the selected dedicated controls: maximize `82:2874`, restore
  `82:2877`, with unchanged source identities for the existing maximize slot.
  The legacy 128px construction motif comes from startup `71:4556` / identity
  `109:8167`; it is no longer a startup dependency. Current manager/loading uses
  `startup/project-mark-compact.svg` (`281:17461`, approved T3 master `267:2212`).
  The owner confirms authorship of the logo and its source reference. The SVG is
  exported unchanged from Figma. The initial splash and large release illustration
  are not shipped as startup dependencies.
  `icons/startup-error.svg` is the exact error export used by Failed `109:8445`
  (`10:2459`); the spinner retains its identity and follows current focus purple.
  Dock artwork comes from page `92:3713`; the common Flat menu follows
  `Kedra/DockHeader` `97:3495`. Original frame identities are retained as provenance
  in `docs/ui/docking/figma/manifest.json`, not as active screenshot coverage.
- Inter: Google Fonts `ofl/inter`, Git blob
  `047c92f6e2212473dc436020afed689527076d44`, upstream source SHA-256
  `29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031`.
- JetBrains Mono: Google Fonts `ofl/jetbrainsmono`, Git blob
  `aa310be8b717fe3774f9444dd89d5f4101cc6d10`, upstream source SHA-256
  `48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda`.

Font license texts accompany the sources and are delivered as normal data assets.
They remain under SIL OFL 1.1, not the Rust workspace's MIT/Apache license.

## Scene View icons

Scene View Blender-direction icons are exact exports from the same owner-supplied
file, reference `186:4545`: eye `10:2441`, sun `10:2437`, cube `10:2426`,
selection `186:2833`, zoom `186:2812`, pan `186:2806`, frame `186:2818`.
The orbit disc and six signed endpoint backgrounds are exported from `186:2996`;
their positions and connecting lines follow the actual camera at runtime.
Selection, zoom, pan and frame originate in Lucide (ISC); the unchanged upstream
notice is retained at `editor/icons/LICENSE-lucide.txt`. These exports do not
include the illustrative scene fixture or transform-editing tools.

## Font derivation

FontTools 4.67.0 instantiated Inter at optical size 14 and weights 400/600,
JetBrains Mono at weight 400. Hinting was removed for native outline rendering.
Inter subset requests selected U+0020–00FF, U+0400–045F, U+2013–2014,
U+2018–2019, U+201C–201D and U+2026. JetBrains Mono requests selected
U+0020–017F, U+0400–045F and U+2000–206F. These are intersections with the
upstream fonts' actual Unicode mappings, not complete coverage of each range.
None of the four bundled fonts maps U+FFFD. JetBrains Mono also lacks some
printable letters within the requested Latin/Cyrillic ranges. Missing characters
must not be assumed to render from these faces; broader coverage requires a
deliberate licensed font/fallback choice and native verification.
These are coherent font-face assets, not chunks reconstructed at runtime. The
Rust build does not require Python/FontTools.

Inter Medium uses the same upstream Inter source at `wght=500`, `opsz=14`.
Its subset requests selected Basic Latin, Latin-1, Cyrillic U+0400–045F and
general punctuation U+2000–206F with `kern,liga` for `latn,cyrl`, without hinting.
The same upstream-mapping limitations apply; these are not complete range claims.
The 46,196-byte output SHA-256 is
`7304ac6809616f3d0c7a9969593b50ca4d61e1ce3e268c9b1e819cd09ea7ecad`.
Subsets do not guarantee every Unicode script or native IME behavior.

The repository's binary-size policy still applies. Wider font coverage needs an
explicitly reviewed asset policy; no exception or threshold was silently added.
Font outlines are resolution-independent. SVG variants are processed at their
native physical density, not obtained by enlarging an existing low-resolution PNG.
