# Scene View controls contract

C-select/C-narrow controls inform the retained
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
must render actual admitted project contents. Single-node Move/Rotate/Scale use the source
gesture lifecycle in ADR 0053; source Save remains a separate command. Play, group
transforms and marquee remain excluded.

## Retained layout and geometry

Reuse the dock header and single group ellipsis; source title stays in the document tab.
No second header or permanent World/units labels are added. Projection
and lighting are scoped commands. Existing Inter typography, semantic colors, focus,
control radii, Flat/Quiet widgets and popup placement remain authoritative.

The viewport fills the document content without a separate toolbar. Its control panel
anchors right 12px/top 8px. It contains the 96px orientation control, the 96×22 projection
button, and one 166px strip of seven 22px Hand/Move/Rotate/Scale/Snap/Frame selection/Settings
icon buttons with 2px gaps and one shared background.
The vertical groups have 4px gaps. Grid visibility and lighting are selected through Settings. See the
[Scene View CSS](https://github.com/kedra-engine/kedra/blob/main/assets/editor/styles/scene-view.css),
[tokens](https://github.com/kedra-engine/kedra/blob/main/assets/editor/styles/tokens.css) and
[source prefab](https://github.com/kedra-engine/kedra/blob/main/assets/editor/prefabs/source-document.prefab.ron).
`controls::layout` hides the orientation disc and projection button below 282px height.
Spatial tools remain visible down to 190×120. Settings remains available in nonspatial
and smaller views down to 46×38. Targets stay inside the canvas without wrapping;
clipped controls must not intercept scene input. Readiness/failure diagnostics remain
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
are reused; artwork does not imply additional capability. The hand disables transforms;
Move/Rotate/Scale select editing modes, and Snap toggles relative increments. Frame selection
has its own button. Settings offers Frame all, Clear selection, Toggle selection outlines,
Toggle grid and Scene/Editor lighting. Outlines do not alter source visibility, picking or
framing bounds. Grid uses one-source-unit spacing. Lighting is view-local replacement,
not additive lighting. Orientation endpoints select standard views; the projection button
toggles Perspective/Orthographic.

The orientation widget follows actual camera rotation, including flight/other commands.
Signed endpoints and depth order remain distinguishable; Y-up Top has screen-up −Z. Disc
or endpoint drag orbits the retained pivot. Endpoint click aligns; repeated requested side
flips to the opposite, including during an unfinished turn. Hits follow rendered endpoint
positions/depth, not stale static geometry. Pan uses MMB dragging and zoom uses the wheel;
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
cursor lock/hide lasts only for the lease and restores prior cursor options. Alt+LMB orbit
and MMB pan retain the same ownership boundaries. Wheel navigation targets the visible ready
viewport under the pointer without acquiring keyboard focus. `F` targets the explicitly
focused viewport or the active content view, including after Hierarchy selection; editable
text and modal focus suppress it. Window, layout, menu and readiness gates still apply.
The viewport container owns input, not its decorative fitted image child.

Menus, host creation and docking gestures block acquisition. Text fields retain keyboard
input while wheel navigation over the viewport leaves their focus intact. A control consumes its
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
Click commands require keyboard reachability; existing keyboard camera navigation
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
