---
version: 1
slug: "index-html"
primary_target: "index.html"
related_targets: ["ru/index.html","src/style.css"]
---

# Kedra landing surface

Mode: Persuade. Scope: `website/` only; separate static English/Russian landing, no
documentation restyle or publication. The owner chose framework-free HTML/CSS.

## Direction contract

THESIS: Introduce the early-development Rust/Bevy project and make the repository and
manual immediately reachable. Do not add feature-card grids, download claims or statistics.

OWN-WORLD: Preserve Figma `384:1497`: charcoal ground, near-white Inter type, muted gray
supporting copy, purple primary action, restrained 6px corners and the exact T3 bird SVG.

STORY: Visitors learn what Kedra is, see that development is ongoing, and choose GitHub,
documentation, roadmap or contribution. RU/EN switches to the corresponding landing.

FIRST VIEWPORT: At 1440 × 900, a 1200px content area with 120px side gutters, a 48px header,
two-column introduction (720px copy, 80px gap, 400px artwork area), 76px heading and a
330 × 291.608 bird. Main content is vertically centered between header and footer.
On phones, retain every control and stack copy before the bird without horizontal overflow.

FORM: Owner-pinned Figma composition, not a new concept round. Code-led implementation
from its high-fidelity design context and exact SVG asset. seed: none (user-pinned reference).

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review,
the verdict, DESIGN.md, and every shipping raster carrying its provenance.

## Constraints and proof

No runtime JavaScript is necessary. Use normal links and static locale pages. Reuse local
Inter sources, preserve OFL and project licenses, and record both artwork rights statements.
The documentation root is build-time configuration; local default is loopback port 4173.
Keep the manual untouched, do not commit or publish, and verify desktop/mobile plus both locales.
