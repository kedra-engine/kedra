# Scene View controls contract

C-select/C-narrow controls are implemented within the read-only
[Scene View](scene-view.md) boundary of
[ADR 0017](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0017-read-only-scene-view.md).
Accessibility, gesture quality and native hardware checks remain incomplete. For commands and
bindings, see
[the Manual](../../../manual/editor/scene-view.md#camera-controls).

## Reference and exclusions

The selected Blender direction comes from
[Scene View studies](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/Kedra?node-id=182-1454):
C-select `186:4545` (1000 × 620 including dock header), C-narrow `186:4949` (480 × 360),
selected button separation A `192:4579` and Y-up interaction examples `186:5528`.
Previous A/B work-strip studies are not targets. Preserve
[canonical Figma provenance](../ui/design-provenance.md#scene-view-and-proposed-editing) and
[artwork sources/licenses](https://github.com/kedra-engine/kedra/blob/main/assets/README.md#scene-view-icons).
The illustration is a diagram, not a runtime frame or mesh-pixel baseline: the implementation
must render actual admitted project contents. Move/Rotate/Scale, transform handles, source
editing, Save, Play, multi-selection and marquee remain excluded.

## Retained layout and geometry

Reuse the dock header and single group ellipsis; source title stays in the document tab.
No second header, permanent World/units labels or settings action is added. Projection
and lighting are scoped commands. Existing Inter typography, semantic colors, focus,
control radii, Flat/Quiet widgets and popup placement remain authoritative.

The accepted reference requires a 38px toolbar with 6px padding, Quiet View/Select at left
and 22px Flat eye/grid targets at right with 2px spacing, then a 10px separator and 22px sun
target. The left canvas rail
is 38px wide at left 8px/top 12px, with 6px padding and a selected 22px selection target.
The 96px orientation control anchors right 12px/top 8px; a right-aligned 22px navigation
strip sits below it at 4px distance. The strip contains Zoom, Pan, 4px separator, Frame
selection, 4px separator and Projection, with 2px adjacent gaps (separation A).

Current authored spacing does not fully match that reference. `scene-view.css` declares
a 2px toolbar column gap and a `scene-shading-gap` spacer of `var(--space)`; `tokens.css`
defines `--space: 4px`. With the prefab's grid/spacer/sun sibling order, the spacer is 4px
and total grid-to-sun separation is 8px (2 + 4 + 2), not the accepted 10px requirement.
The navigation strip currently has 4px row gaps plus explicit 4px spacer nodes, not the
accepted 2px adjacent gaps. See the current
[Scene View CSS](https://github.com/kedra-engine/kedra/blob/main/assets/editor/styles/scene-view.css),
[tokens](https://github.com/kedra-engine/kedra/blob/main/assets/editor/styles/tokens.css) and
[source prefab](https://github.com/kedra-engine/kedra/blob/main/assets/editor/prefabs/source-document.prefab.ron).
The implementation must still meet the accepted reference. Existing source-order/density and
native geometry checks do not assert these separator or adjacent-gap values; the spacing
discrepancies also need verification in rendered frames.

`controls::layout` hides Select below measured content width 640px while Clear selection
remains in View. Navigation hides for nonspatial targets, width below 160px or height below
230px; the tool rail hides below 60px width or 50px height. Targets stay inside the canvas
without toolbar wrapping; clipped controls must not intercept scene input. Commands remain
available through View rather than unreachable targets. Readiness/failure diagnostics remain
visible and actionable; no permanent canvas labels, help/status footer or second chrome.

Exact local icons retain density metadata and 1×–4× packages: 16px controls, 21px endpoints,
86px disc slots. Eye `10:2441`, sun `10:2437`, cube `10:2426`, selection `186:2833`, zoom
`186:2812`, pan `186:2806`, frame `186:2818` and disc/signed endpoint backgrounds `186:2996`
retain their export authorities. Lucide hand, zoom-in, scan and square-dashed-mouse-pointer
retain the ISC notice at `assets/editor/icons/LICENSE-lucide.txt`. Artwork is downloaded
unchanged, not redrawn; camera-projected endpoint positions/axis lines are dynamic geometry,
not a frozen perspective screenshot. The scene-fixture illustration is not imported.

## Commands, state and camera motion

Existing popup infrastructure, selection coordinator, camera commands and PanOrbit motion
are reused; artwork does not imply additional capability. View exposes framing, six standard
views, explicit Perspective/Orthographic and Clear selection. Select exposes Clear selection,
not unsupported Select all. The left selection tool only focuses the canvas. Eye changes
per-view spatial/UI bounds display, not source visibility, picking, selection, grid or bounds
used for framing. Grid controls one-source-unit reference grid/axes. Sun reflects explicit
Scene/Editor lighting, with view-local replacement, not additive lighting.

The orientation widget follows actual camera rotation, including flight/other commands.
Signed endpoints and depth order remain distinguishable; Y-up Top has screen-up −Z. Disc
or endpoint drag orbits the retained pivot. Endpoint click aligns; repeated requested side
flips to the opposite, including during an unfinished turn. Hits follow rendered endpoint
positions/depth, not stale static geometry. Zoom/Pan are LMB-drag controls, not fake clicks;
Frame selection reports unavailable bounds and Projection preserves apparent pivot scale.

Pointer alignment uses 200ms cubic-Bezier `(0.77, 0, 0.175, 1)`. Bevy owns the easing solver
and quaternion interpolation. Camera and gizmo share the current interpolated pose and fixed
pivot radius. New requests retarget from current pose; dragging stays direct without trailing
animation. Escape, focus loss, direct manipulation and camera retirement interrupt. Existing
reduced-motion preference makes alignment immediate, including completion of a turn already
in progress; keyboard views remain immediate. No new animation/controller framework,
dependency or runtime-to-editor edge is introduced.

## Input ownership and cancellation

The ready, visible, focused viewport in its owning native window acquires a navigation lease
scoped to view, camera, window and layout revision. Hover alone cannot acquire keyboard focus.
RMB flight owns relative motion, WASD/QE, Shift speed and bounded wheel-speed adjustment;
cursor lock/hide lasts only for the lease and restores prior cursor options. Alt+LMB orbit,
MMB pan, focused wheel navigation and focused `F` retain the same ownership boundaries.
The viewport container owns input, not its decorative fitted image child.

Menus, fields, host creation and docking gestures block acquisition. A control consumes its
initiating LMB so it cannot also pick through or start docking. Release, Escape, focus loss,
menu opening, tab switch, resize/layout revision, host transfer, hidden/retired view and
workspace retirement cancel capture and clear locally owned keys. Another window cannot
drive the lease; foreign-window release must neither consume it nor leave sticky state.
`workspace::scene_input` is the host adapter; it never rebinds global input resources or
runs project input systems. Debug eligible-window identities crossing workspace/Luau
diagnostics are full-width decimal strings, not lossy floating-point IDs.

Unbound, dependency-pending, failed, retired, hidden, UI-only and image-only targets cannot
acquire or advertise spatial navigation. UI-only retains fixed fitted canvas and bounds
display. Selection actions require an active source document; raw GLB has no invented
authoring selection. Restoring bounds display needs no reselection and must preserve
Frame selection's bounds even while overlays are hidden.

## Accessibility and acceptance

Functional targets require accessible labels, keyboard focus and visible focus states.
Click commands remain reachable through keyboard/View; existing keyboard camera navigation
must not require dragging small targets. Implementation does not certify screen readers,
gesture quality, OS capture or physical input.

Production owners are
[`controls.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/scene_view/controls.rs),
[`orientation.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/scene_view/orientation.rs),
[`camera_motion.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/scene_view/camera_motion.rs)
and
[`scene_input.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/workspace/scene_input.rs).
Their local tests cover narrow/short geometry, disabled controls, retained bounds, signed
axis projection, turn retarget/flip, reduced motion, cancellation and rejected-request
non-mutation. Editor authored icon/density tests check sources, not native pixels.

Run focused Rust tests, icon/density contracts, `cargo xtask check` and the
[Scene View native scenarios](scene-view.md#verification-and-remaining-acceptance) on a fresh
checked-project copy. Acceptance requires real input/renderer frames across spatial/UI/mixed
and PNG/GLB targets, wide, 480 × 360 and narrower docks, all gestures/standard views,
cancellation, tab changes and Float/Redock with unchanged source bytes. Compare chrome
geometry/states against Figma, never illustrative mesh pixels. Keep evidence under `target/`.
Compilation/RPC acknowledgements are not visual acceptance. Physical RMB/MMB, cursor-lock
and monitor/DPI behavior remain unverified unless exercised on native hardware; documented
scenarios and headless ownership tests alone do not certify them.
