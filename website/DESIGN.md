---
name: Kedra project landing
description: A charcoal and violet static landing with Inter typography and the original T3 bird.
colors:
  paper: "#242424"
  ink: "#eeeeee"
  muted: "#afafaf"
  violet: "#7950bc"
  panel: "#303030"
  line: "#1b1b1b"
  focus: "#d1a4ff"
  white: "white"
  primary-hover: "#895dcd"
  secondary-hover: "#3b3b3b"
typography:
  display:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "clamp(48px, 5.277778vw, 76px)"
    fontWeight: 600
    lineHeight: 1.210526
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "30px"
    fontWeight: 500
    lineHeight: "40px"
  body:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 400
    lineHeight: "25px"
  label:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 500
    lineHeight: "22px"
  wordmark:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "19px"
    fontWeight: 600
    lineHeight: "27px"
  status:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "20px"
  license:
    fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "18px"
rounded:
  corner: "6px"
spacing:
  compact: "8px"
  action-gap: "12px"
  small: "16px"
  control-inset: "20px"
  standard: "24px"
  group: "32px"
  mobile-section: "40px"
  section: "48px"
  section-end: "64px"
components:
  action-primary:
    backgroundColor: "{colors.violet}"
    textColor: "{colors.white}"
    typography: "{typography.label}"
    rounded: "{rounded.corner}"
    padding: "0 20px"
  action-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  action-secondary:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.corner}"
    padding: "0 20px"
  action-secondary-hover:
    backgroundColor: "{colors.secondary-hover}"
  text-link:
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.corner}"
  locale-link:
    textColor: "{colors.muted}"
    typography: "{typography.label}"
    width: "44px"
  locale-link-current:
    textColor: "{colors.ink}"
  skip-link:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.corner}"
    padding: "12px 16px"
---

# Design System: Kedra project landing

## Overview

**Creative North Star: "Charcoal, Inter and the T3 bird"** follows the owner-pinned reference.

This system covers only `website/`, not the native Kedra editor or the separate
documentation site. The owner-pinned Figma file `2EZlK40cfx2Mub7GmDyI70`, frame
`384:1497`, supplies its identity: quiet charcoal, direct typography and a faceted bird.
The implemented rules are grounded in `src/style.css`, both static HTML pages and
`vite.config.mjs`; the tokens above describe that implementation.

The flat ground and generous empty space leave the artwork as the expressive material.
Violet marks the primary destination; restrained secondary controls keep the hierarchy
clear without introducing decorative containers.

**Key Characteristics:**

- Charcoal ground, near-white hierarchy and muted supporting copy.
- Local Inter in three real weights, without synthetic faces.
- Flat controls and the unchanged, full-color T3 bird.
- Static English and Russian pages with the same visual system.

## Colors

### Primary

- **Violet** (`violet`): primary GitHub action and selected-text background.
- **Primary hover** (`primary-hover`): brighter primary action on hover-capable pointers.
- **Focus violet** (`focus`): keyboard outlines, distinct from the action fill.

### Neutral

- **Charcoal paper** (`paper`): page and browser theme ground.
- **Near-white ink** (`ink`): heading, purpose, ordinary links and current locale.
- **Muted gray** (`muted`): description, development status, licenses and inactive locale.
- **Panel gray** (`panel`): secondary documentation action.
- **Dark line** (`line`): secondary border and footer divider.
- **White** (`white`): primary action text, selected text and hovered text links.
- **Secondary hover** (`secondary-hover`): lighter secondary action background.

The SVG's purple/yellow facets and gradients belong to the artwork, not to additional
interface color tokens. `color-scheme: dark` and matching HTML color-scheme/theme-color
metadata declare the browser surface. The scrollbar uses muted gray over charcoal;
selection uses white over violet. No light theme or custom browser-chrome renderer exists.

## Typography

All roles use the frontmatter Inter stack. Local `Inter-Regular.ttf`, `Inter-Medium.ttf`
and `Inter-SemiBold.ttf` from `../assets/editor/fonts/` supply normal weights (400, 500,
600), with `font-display: swap`. Font synthesis is disabled; optimized legibility and
WebKit antialiasing are requested. There are no custom font metric overrides or variable
font axes. Preserve [the SIL OFL notice](../assets/editor/fonts/Inter-OFL.txt).

- **Display:** one page heading; tight tracking, fluid desktop size and semibold weight.
- **Headline:** medium purpose statement, maximum width (660px); the HTML contains an
  explicit line break in each locale.
- **Body:** regular muted description, maximum width (640px), not a character-count limit.
- **Label:** medium navigation, actions, roadmap and contribution links.
- **Wordmark:** compact semibold home link, not an image or a separate display face.
- **Status / license:** quieter regular supporting roles; license text preserves spaces.

At widths up to (1000px), the purpose becomes (26px / 36px). At widths up to (700px),
the display becomes `clamp(42px, 12.307692vw, 60px)` with line-height (1.2), purpose
becomes (24px / 32px), and body becomes (16px / 25px). Other roles retain their metrics.

## Layout

The centered shell spans the viewport up to (1440px), with vertical padding (24px),
fluid horizontal padding `clamp(24px, 8.333333vw, 120px)` and minimum height (100svh),
falling back to (100vh). Its grid is a (48px) header, flexible introduction and auto footer.
The body minimum width is (320px); content may scroll vertically rather than clip.

Desktop introduction columns are `minmax(0, 1.8fr) minmax(0, 1fr)`, with gap
`clamp(32px, 5.555556vw, 80px)` and block padding (48px 64px). At (1440px), this resolves
to a (1200px) content width, (720px) copy column, (80px) gap and (400px) artwork column.
Navigation distributes its contents across (410px), limited to available width.
The artwork is centered in its column and capped at (330px) with automatic height.

At widths up to (1000px), horizontal gutters become (40px), the column ratio becomes
(1.6fr / 1fr), and the gap becomes (32px). At widths up to (700px):

- Gutters use the larger of (24px) and each horizontal safe-area inset; top/bottom use
  the larger of (16px) and their safe-area insets.
- Header becomes two rows: home and locale first, documentation and GitHub second.
  There is no collapsed menu; navigation uses `display: contents` to place its children.
- Introduction stacks copy before artwork, with gap (40px) and padding (40px 48px).
- Artwork width becomes `min(70%, 240px)`; footer wraps with horizontal gap (16px).

Actions wrap with gap (12px), separated from the description by (32px). Description
starts (24px) after purpose; purpose starts (16px) after the title. Development status
starts (20px) after actions, wraps with horizontal gap (20px), and gives its text a
(180px) width. Footer uses a (1px) divider and (16px) top padding.

## Elevation & Depth

There are no interface shadows, blur, translucency or elevated cards. Secondary fill
and dark borders separate controls; empty space separates content. Depth, gradients
and angular facets are confined to the original bird artwork.

**The Flat Interface Rule.** Keep interface controls flat; preserve dimensional facets
inside the original bird artwork.

## Shapes

Controls and focus outlines use the single restrained corner token. The page has no
clipped hero shell or rounded content panels. The bird's angular silhouette remains
unaltered: `public/brand/kedra.svg` retains its intrinsic (330 × 291.608) geometry;
HTML reserves (330 × 292) while CSS preserves its actual aspect ratio.

## Components

### Destination actions

Both action variants are ordinary anchors, with minimum height (48px), horizontal
padding from the frontmatter, centered label text and the shared corner. Primary is
GitHub; secondary is documentation, with a (1px) dark border. These are not form buttons.
Hover changes only the fill and is gated by `(hover: hover)`. Pressed actions use
`filter: brightness(0.9)`. Only background color transitions (120ms ease-out), and only
under `(prefers-reduced-motion: no-preference)`; reduced-motion users get immediate changes.
No entrance, scrolling or artwork animation exists.

### Text navigation and locale

Text links have minimum height (48px); locale links additionally have width (44px).
The current locale is near-white and underlined via `aria-current="page"`; inactive
locale is muted. Hover-capable text and license links become white and underlined.
Underline offset is (4px). All anchors have manipulation touch behavior and transparent
WebKit tap highlight; this does not replace the keyboard focus outline.

English lives at `/`, Russian at `/ru/`. Locale changes are full-page anchor navigation,
not JavaScript state, automatic detection or persisted preference. Each page has its own
document language, alternate-language links and localized navigation names. Locale names
include their visible abbreviations: `RU — Русский` and `EN — English`.

Documentation is a separate site. `KEDRA_DOCS_URL` is resolved at build time; its default
is `http://127.0.0.1:4173/`. Configuration appends `en/` or `ru/`, then `contributing/`
for contribution links. It accepts an absolute HTTP(S) root with an optional base path,
normalizes the trailing slash, and rejects credentials, query and fragment. Landing
routes assume domain-root hosting. There is no runtime configuration or JavaScript UI.

### Keyboard entry and footer

Every anchor gets a focus-visible outline (2px) with offset (4px). The skip link uses
inverted colors, sits at top (8px) / left (24px), and is translated offscreen until
focused; it targets the main landmark with `tabindex="-1"`. Its reveal is immediate.
The footer preserves separate real MIT and Apache-2.0 links with (48px) minimum height,
and a contribution link. There are no disabled, loading, form or application-error states.

### T3 bird

The decorative image has empty alt text inside an `aria-hidden` wrapper; the nearby
wordmark and heading identify the project. The same SVG is the favicon. Preserve the
unchanged export rather than substituting the native compact asset or screenshot crop.
[Brand provenance](public/brand/README.md) records the source identities and differing
rights statements; it does not establish independent legal clearance. Confirm rights
before public distribution. Project license links do not relicense artwork or fonts.

## Do's and Don'ts

### Do

- **Do** keep these rules scoped to the standalone landing, not native UI or the manual.
- **Do** preserve the original bird export, font sources and their provenance/notices.
- **Do** keep every destination reachable in both static locales and narrow layouts.
- **Do** retain visible keyboard focus and immediate reduced-motion state changes.

### Don't

- **Don't** replace the pinned Inter face or redraw the bird as a generic icon.
- **Don't** add runtime JavaScript or merge the documentation build into this landing.
- **Don't** add unsupported download claims, statistics or extra marketing sections.
- **Don't** treat project licenses or owner selection as independent artwork clearance.
