# Design source provenance

Design exports and their license obligations trace back to
[Kedra UI-KIT](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT),
file `2EZlK40cfx2Mub7GmDyI70`. Navigate by node, never assumed canvas position/page order.
The map is static; it does not track live design changes.

## Library navigation

| Page | Node | Contents |
| --- | --- | --- |
| [01 · Kit / Basics](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=49-438) | `49:438` | Basic masters, SVG icons, controls, feedback, navigation/data |
| [06 · Kit / Panels & Chrome](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=175-45356) | `175:45356` | Flat menus/toolbars, docking, chrome, Hierarchy, panels/startup |

Entry points:

- Basic masters `51:1082`; `Kedra/IconButton` `27:530`.
- Flat toolbar/common dock menu `163:37771`.
- Docking masters `92:2968`; `Kedra/DockHeader` `97:3495`.
- Window chrome `82:2870`; unified family `82:3049`.
- Editor right/macOS `222:51056` / `222:51199`; Floating right/macOS
  `222:51320` / `222:51364`.
- Panel/startup masters `176:38405`; shared screens `107:3539`.
- Startup family `109:9133`: Boot `109:8369`, Opening `109:8406`, Failed `109:8445`,
  Projects `109:8489`, Open `109:8873`.

## Startup identity and palette

Manager/loading uses compact T3 export `281:17461`, approved master `267:2212`; owner-confirmed
logo authorship/export provenance remains in
[Asset provenance (English)](https://github.com/kedra-engine/kedra/blob/main/assets/README.md#provenance).
Boot/full-window artwork is design reference, not implemented startup surfaces. Accepted
[ADR 0021](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0021-windowless-startup.md)
requires windowless preparation/no heavy loading artwork. Startup status text must describe source
preparation, not promise a future build or trust validation. Foundations `51:1088` and primary
states
`27:443`, `27:445`, `27:447` pin accent `#7950bc`, hover `#865cc3`, pressed `#603993`,
selection `#57416f`, focus `#d08bff` across editor/dock previews/spinner.

## Asset Browser exports

Composition `163:37852` is 878×320; toolbar guidance `163:37771`. Native toolbar/breadcrumb/
folder/tile sources use project tokens, not screenshot imagery. Exact UI-KIT SVGs below supply
1x–4x packages under `assets/editor/icons/`, without community imports:

| File | Node |
| --- | --- |
| `browser-back.svg` | `153:26096` |
| `browser-up.svg` | `153:26099` |
| `browser-grid.svg` | `10:2488` |
| `browser-list.svg` | `153:26093` |
| `browser-right.svg` | `10:2402` |
| `browser-down.svg` | `10:2399` |
| `browser-filter.svg` | `38:440` |
| `browser-search.svg` | `10:2419` |
| `browser-folder.svg` | `10:2423`, regular slot |
| `browser-folder-preview.svg` | `10:2423`, exact vector; native 54px processing for the tile preview slot |
| `browser-folder-selected.svg` | `10:2423`, selected slot |

Folder/search slots preserve composition exports without replacing older startup assets.
Dynamic reference previews are not bundled thumbnails. PNG/static-GLTF tiles use
generated cached snapshots; unsupported types retain truthful type badges.
See [Asset Browser](../../../manual/editor/asset-browser.md).

## Headers and update indicator

Scene View's existing viewport panel hosts Move, Rotate, Scale and step snapping as icon-only
controls. `scene-move.svg`, `scene-rotate.svg`, `scene-scale.svg` and `scene-snap.svg`
adapt Lucide's `move`, `rotate-3d`, `scaling` and `magnet`
SVGs (16px output, Kedra neutral stroke), with 1x–4x packages and the retained
`assets/editor/icons/LICENSE-lucide.txt` ISC notice. Sources:
<https://github.com/lucide-icons/lucide/blob/main/icons/move.svg> and
<https://github.com/lucide-icons/lucide/blob/main/icons/magnet.svg>.
Rotation and scale sources: <https://github.com/lucide-icons/lucide/blob/main/icons/rotate-3d.svg>
and <https://github.com/lucide-icons/lucide/blob/main/icons/scaling.svg>.
Unity's [Tools Overlay](https://docs.unity3d.com/6000.0/Documentation/Manual/PositioningGameObjects.html)
and Blender's [object tools](https://docs.blender.org/manual/en/latest/scene_layout/object/tools/toolbar.html)
inform the compact tool-mode organization; their artwork is not bundled.

The [shared header](window-chrome.md) uses the unified family. `toolbar-play.svg`,
`toolbar-pause.svg`, `toolbar-stop.svg` export `10:2387`, `10:2390`, `10:2393` with 1x–4x
packages. Shared `browser-search.svg`/`scene-down.svg` retain canonical search/down geometry.
macOS button illustrations never replace native controls. These exports do not refresh pinned
visual references.

Update family `327:17756`, indicator `327:17796`, status-only popover `327:17900` use
`update.svg` from `327:17759` (1x–4x), no third-party artwork/background. The authored shared-button
indicator reuses spinner/warning assets: 22px indicator, 64×22 Primary Ready, hover/focus-only
popover per
[ADR 0026](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0026-editor-self-update.md).
The update indicator still needs rendered-frame and native-input verification.

## Hierarchy exports

[Hierarchy](../../../manual/editor/hierarchy.md) follows `128:32776` under `125:17354`:
22px rows, 12px depth indentation, shared semantic tokens. `hierarchy-scene.svg` is `10:2429`;
`hierarchy-node.svg` is `10:2426`. Selected-focus `128:32955` supplies
`hierarchy-node-selected.svg` (`10:2426`, selected export); `128:33170` supplies
`hierarchy-link.svg` (`10:2467`). Search `128:32885` supplies `hierarchy-close.svg` (`10:2396`);
search/filter share browser exports. Native 1x–4x packages and source UUIDs remain.
Down/right/folder exports are shared. Do not fabricate component-specific light/camera icons for
nodes without those semantics. State specimens own default/search/focus/collapse appearances;
search/reveal/details use the common group menu, not persistent diagnostic footer/text actions.
Read-only navigation does not imply rename/reparent/prefab editing.

## Inspector and shared controls

[Inspector](../../../manual/editor/inspector.md): Flat compact A `131:20213` (320px),
`131:20388` (480px); editor reference `52:438`. Shared row/section `11:142` / `11:499`;
read-only TextField `49:554`: 22px, 6px horizontal padding, 4px gap. Current bound geometry
supersedes stale raised-bevel/26–28px descriptions. Controls/tools/tree rows 22px, section/menu
rows 24px, radii 2px; window/dock headers remain 32px, tabs 28px, asset cards unchanged.
Fields keep 16px icons and 12/16 body text. Remove control highlights/shadows, not popup exterior
shadows/state feedback. Removed drafts/comparison variants/unintegrated Scene View experiments
are not implementation sources. Branding/palette/windowless startup are unchanged.
`property-lock.svg` is the exact `49:554`/property-row export, not fabricated; sections share
down/right exports above. No community artwork/backgrounds. Diagnostic/unavailable/no-results/
narrow states still need owner visual review; captures are not pinned pixel-equivalence evidence.

## Scene View and proposed editing

[Scene View](../../../manual/editor/scene-view.md) uses C-select `186:4545`, C-narrow
`186:4949`, selected-button separation A `192:4579`. Toolbar is 38px with View/Select menus and
22px Flat eye/grid/sun. Canvas controls cover selection/orientation/navigation. Select hides below
640px; View retains Clear selection. Detailed geometry/gestures/pending acceptance remain in
[Controls design](../editor/scene-view-controls.md).

`108:9202` shell, `108:8644` toolbar and `108:8635` illustration remain provenance, not current
toolbar authority. `scene-down.svg` / `scene-grid-selected.svg` / `scene-settings.svg` retain
`10:2399` / `10:2488` / `10:2482`; icon availability does not imply settings support. Rendered
assets are project data, never an illustrative SVG or scene baseline.
[ADR 0009](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0009-property-editing-and-inspector.md)
proposes full property editing;
[ADR 0010](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0010-text-field-interaction.md)
proposes full single-line interaction over TextField `49:536`/states `51:1497`.
Pending/conflicted/error+focus need reviewed compositions; baseline masters do not certify them.

## Independent references and provenance

- [UI corpus](ui-corpus.md#evidence-and-source-package) owns the active 44 independent full/shape
  images for 22 cases, exact source/text bindings, hashes and policy. Offline comparison needs no
  design connection; navigation identities are not baseline approval.
- [Startup geometry](startup-visual.md#independent-geometry-and-evidence) owns
  `docs/ui/startup-reference.json`: geometry/nodes/tolerances, not PNG comparisons.
- [Dock manifest](https://github.com/kedra-engine/kedra/blob/main/docs/ui/docking/figma/manifest.json)
  preserves schema 1, file `2EZlK40cfx2Mub7GmDyI70`, page `92:3713`, export `2026-10-19`,
  revision `flat-icon-buttons-common-dock-menu` and 20 frame records (`i00-baseline` through
  `i18-macos`, with separate `i14-float-main`/`i14-float-window`). Every original node, dimension
  and SHA-256 remains in that canonical JSON. Figma supplied the independent native-resolution
  PNGs, without cropping or resizing. The owner requested their refresh. They are not application
  goldens; their PNG files are absent and cannot serve as comparison inputs. The adjacent
  [provenance warning (English)](https://github.com/kedra-engine/kedra/blob/main/docs/ui/docking/figma/README.md)
  must remain interpretable beside the manifest. Do not treat these source hashes as disposable
  run history or active screenshot acceptance.

Export pages do not describe current library ordering. Regrouping nodes never authorizes changes
to reference pixels, budgets or authored application structure. Refresh requires actual source
inspection and explicit review. This static map does not establish live synchronization/native
visual acceptance.

## Licensing and source authority

Owner-supplied artwork/logo authorization does not grant third-party rights. Preserve
[Asset provenance and font derivation (English)](https://github.com/kedra-engine/kedra/blob/main/assets/README.md)
and
[Third-party obligations (English)](https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md).
Inter and JetBrains Mono remain SIL OFL-1.1 with source identifiers/notices beside assets, not the
Rust license. Inter Medium derives from the same pinned Inter source at `wght=500`, `opsz=14`;
its exact output hash is pinned by the corpus. Subsets do not promise complete Unicode/CJK/IME.
Scene View selection/zoom/pan/frame originate in Lucide (ISC); keep
`assets/editor/icons/LICENSE-lucide.txt`. CDLA-Permissive-2.0 applies separately to reviewed Mozilla
root-certificate data in packaging-only `webpki-roots` / `webpki-root-certs` `1.0.10`,
not UI artwork;
retain its text when sharing that data, per `deny.toml` and
[ADR 0023](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0023-desktop-distribution.md).
A design export, source hash or Cargo license field is not an independent rights grant.
Manual development-plugin export/review is supported; no AI subscription is required for sources,
offline comparison or CI.
