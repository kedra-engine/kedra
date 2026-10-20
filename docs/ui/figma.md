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
- Panel/startup masters: `176:38405`.
- Shared screen compositions: `107:3539`.

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

## Hierarchy source exports

Read-only [Hierarchy](hierarchy.md) follows `128:32776` under `125:17354`:
26px rows, 12px depth indentation and existing semantic tokens. Its scene-root
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

## Planned field behavior

[Scene View](scene-view.md) follows the confirmed `108:9202` content shell and
`108:8644` toolbar: 38px height, projection/lighting selects, selected grid and
right-aligned settings. Local icons `scene-down.svg`, `scene-grid-selected.svg`
and `scene-settings.svg` are exact exports of `10:2399`, `10:2488` and `10:2482`.
Rendered assets are data, not the SVG illustration `108:8635`. The profile records
the read-only World / 1 unit adaptation and native inset-shadow limitation.

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
