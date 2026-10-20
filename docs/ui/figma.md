# Figma source map

Design source: [Kedra UI-KIT](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT).
Use node identity rather than canvas position or an assumed page order.

## Current library navigation

| Page | Node | Contents |
| --- | --- | --- |
| [01 · Kit / Basics](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=49-438) | `49:438` | Basic masters, SVG icons, controls, feedback, navigation and data controls |
| [06 · Kit / Panels & Chrome](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=175-45356) | `175:45356` | Flat toolbar/menu guidance, docking, window chrome, hierarchy, panel and startup compositions |

Component entry points:

- Basic masters: `51:1082`; `Kedra/IconButton`: `27:530`.
- Flat toolbar and common dock menu guidance: `163:37771`.
- Docking masters: `92:2968`; `Kedra/DockHeader`: `97:3495`.
- Window chrome masters: `82:2870`.
- Unified window-header family: `82:3049`; Editor right/macOS `222:51056` /
  `222:51199`, Floating right/macOS `222:51320` / `222:51364`.
- Panel/startup masters: `176:38405`.
- Shared screen compositions: `107:3539`.
- Startup surface family: `109:9133`; Boot `109:8369`, Opening `109:8406`,
  Failed `109:8445`, Projects `109:8489`, Open `109:8873`.

## Startup identity and shared palette

The manager/loading uses exact compact T3 logo export `281:17461` (master
`267:2212`); owner-confirmed provenance is recorded in [assets](../../assets/README.md).
Boot and full-window artwork remain Figma design references, not implemented
startup surfaces. Owner direction replaces the initial splash with windowless
startup and omits heavy loading artwork; see
[ADR 0021](../decisions/0021-windowless-startup.md).

Foundations `51:1088` and primary Button states `27:443`, `27:445`, `27:447` pin
accent `#7950bc`, hover `#865cc3`, pressed `#603993`, selection `#57416f`, focus
`#d08bff`. These roles apply to the whole editor, including docking previews and
the startup spinner. Current source-only preparation retains truthful wording
instead of the design's future build/trust claims.

## Asset Browser source exports

The browser composition is `163:37852` (878 × 320), with flat toolbar guidance
`163:37771`. Authored toolbar, breadcrumb, folder and tile sources reuse project
tokens rather than screenshot imagery. These project UI-KIT SVG exports supply
the native 1x–4x icon packages; no community assets were imported:

| Source file under `assets/editor/icons/` | UI-KIT source node |
| --- | --- |
| `browser-back.svg` | `153:26096` |
| `browser-up.svg` | `153:26099` |
| `browser-grid.svg` | `10:2488` |
| `browser-list.svg` | `153:26093` |
| `browser-right.svg` | `10:2402` |
| `browser-down.svg` | `10:2399` |
| `browser-filter.svg` | `38:440` |
| `browser-search.svg` | `10:2419` |
| `browser-folder.svg` | `10:2423` (regular slot) |
| `browser-folder-selected.svg` | `10:2423` (selected slot) |

The regular/selected folder slots and search slot retain the composition's exact
exports, without replacing the older startup assets. Dynamic source previews in
the reference are not bundled as fake project thumbnails; current source tiles use
truthful type badges. See [browser behavior and limitations](asset-browser.md).

## Window header source exports

The [shared header mechanism](window-chrome.md) consumes the unified role family.
`toolbar-play.svg`, `toolbar-pause.svg` and `toolbar-stop.svg` are exact Figma
exports of `10:2387`, `10:2390` and `10:2393`, with 1x–4x density packages.
Existing `browser-search.svg` and `scene-down.svg` share canonical UI-KIT search/
down geometry. Native macOS button illustrations are not substitute controls.
These exports do not update the old pinned visual references.

## Hierarchy source exports

Read-only [Hierarchy](hierarchy.md) follows `128:32776` under `125:17354`:
22px rows, 12px depth indentation and existing semantic tokens. Its scene-root
icon `hierarchy-scene.svg` comes from UI-KIT `10:2429`, and the generic source-node
icon `hierarchy-node.svg` from `10:2426`. Both use native 1x–4x density packages.
The selected-focus specimen `128:32955` supplies `hierarchy-node-selected.svg`
(`10:2426`, selected export); `128:33170` supplies `hierarchy-link.svg` (`10:2467`).
Search `128:32885` supplies `hierarchy-close.svg` (`10:2396`); its exact search/filter
exports are already shared with the browser. All new icons retain native 1x–4x
packages and source UUIDs.
Existing exact down/right/folder exports remain shared. Component-specific light
or camera symbols are not fabricated for nodes without those semantics.
Default/search/focus/collapsed appearances follow the corresponding state specimens;
search, reveal and source details use the canonical group menu instead of persistent
text actions and a diagnostic footer. Read-only source diagnostics implement the ADR navigation
contract; no rename, reparent or prefab-editing affordance implies support.

## Inspector and shared property controls

[Inspector](inspector.md) uses Flat compact A `131:20213` (320px) and `131:20388`
(480px); the integrated editor reference is `52:438`. Shared row and section
sources remain `11:142` and `11:499`; read-only TextField is `49:554` (22px,
6px horizontal padding, 4px control gap). Current bound geometry supersedes stale
component descriptions mentioning raised bevels or 26/28px controls. Shared
controls/tools/tree rows are 22px, section/menu rows 24px, control radii 2px;
32px window/dock headers, 28px tabs and asset-card sizes are unchanged. Fields
retain 16px icons and 12/16 body text. Remove control highlights/shadows, not
popup external shadows or interaction-state feedback. No removed drafts,
comparison variants or unintegrated Scene View experiments are implementation
sources. Branding, palette and the windowless startup decision remain unchanged.
`property-lock.svg` is the exact UI-KIT export used by `49:554` and the property row,
not a fabricated icon. Sections reuse exact `browser-down.svg` / `browser-right.svg`
exports listed above. No community assets or screenshot backgrounds are used.
Diagnostic, unavailable, no-results and narrow states still require owner visual
review; native captures do not constitute pinned Figma pixel-equivalence evidence.

## Scene View and planned field editing

[Scene View](scene-view.md) follows the confirmed `108:9202` content shell and
`108:8644` toolbar: 38px height, projection/lighting selects, selected grid and
right-aligned settings. Local icons `scene-down.svg`, `scene-grid-selected.svg`
and `scene-settings.svg` are exact exports of `10:2399`, `10:2488` and `10:2482`.
Rendered assets are data, not the SVG illustration `108:8635`. The profile records
the read-only World / 1 unit adaptation. Flat compact A applies to existing
controls; removed Scene View experiments are not imported or used to add features.

[ADR 0009](../decisions/0009-property-editing-and-inspector.md) proposes shared
property-editing ownership. [ADR 0010](../decisions/0010-text-field-interaction.md)
specifies proposed single-line text behavior over TextField masters `49:536` and
state examples `51:1497`. Pending, conflicted and combined error/focus states need
reviewed compositions; the baseline kit does not certify those interactions.

## Pinned reference exports

Library navigation and an exported test revision are separate contracts.
[Startup](startup-layout-lock.md) policies and the
[docking manifest](docking/figma/manifest.json) retain the source frame IDs,
export dimensions, hashes and provenance for their exact offline snapshots.
Those export-page identifiers do not describe the current library page order.

Moving or regrouping library nodes does not authorize changing reference pixels,
comparison budgets or authored application structure. Reference refreshes require
inspection of the actual source frames and explicit review. Local Rust checks read the
checked-in exports without Figma access.
