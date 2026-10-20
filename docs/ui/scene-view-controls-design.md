# Scene View controls: Blender direction

## Target and scope

Planned replacement for the initial toolbar described in [Scene View](scene-view.md),
within [ADR 0017](../decisions/0017-read-only-scene-view.md)'s read-only boundary.
This document specifies intended behavior, not verified implementation.

The visual reference is the owner's selected Blender direction on
[Scene View studies](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/Kedra?node-id=182-1454):

- `186:4545`: C-select, 1000 × 620 including dock header.
- `186:4949`: C-narrow, 480 × 360 including dock header.
- `192:4579`: selected button separation A.
- `186:5528`: Y-up navigation interaction examples.

Previous A/B work-strip designs are not the target. Move, Rotate, Scale, transform
handles, source editing, Save, Play, multi-selection and marquee are excluded.
The illustration is a diagram, not a runtime image or a scene-render baseline.
The implementation must continue rendering actual admitted project contents.

## Layout

Reuse the existing dock header and its single group ellipsis; do not create a
second header or change other panels. Keep the source title in the document tab.
Remove Perspective/Lit selects, World/1 unit and settings from the old toolbar.

The content toolbar is 38 logical pixels high, with 6px padding. Quiet View and
Select menus occupy its left side. The right side contains 26px Flat eye/grid
targets at 2px spacing, then a 10px separator and a 26px sun target. Use existing
Inter typography, semantic colors, focus treatment and control radii.

The canvas contains a 38px-wide panel at left 8px/top 12px, with 6px padding and
one selected 26px selection-tool target. At right 12px/top 8px, a 96px orientation
control sits above a right-aligned 26px-wide navigation strip, separated by 4px.
The strip contains Zoom, Pan, a 4px separator, Frame selection, another 4px
separator and Projection; adjacent controls have 2px gaps (separation A).

At 480 × 360 the Select menu disappears, as in C-narrow; its action remains in
View. Implementation choice: use a measured content-width threshold of 640px.
Below the reference size, keep targets inside the canvas, avoid toolbar wrapping
and never let a clipped control intercept scene input. At insufficient canvas
height, expose the same commands through View instead of unreachable targets.
No permanent informational canvas labels or help/status footer are added.
Loading and failure diagnostics remain visible and actionable.

## Commands and navigation

Reuse existing popup, selection coordinator, camera commands and PanOrbitCamera.
The following interpretations fill gaps in the visual mockup; they are
implementation choices rather than additional capabilities implied by artwork:

- View offers Frame selection, Frame all, six standard views and explicit
  Perspective/Orthographic commands. It also exposes Clear selection in narrow
  layouts. Select offers Clear selection; no unsupported Select all is advertised.
- The eye toggles selection-bound overlays only, not source visibility, picking,
  document selection or the independently controlled grid. Preserve that option
  per view. Include both spatial markers and Hierarchy-selected UI borders.
- Grid toggles the existing reference grid/axes. Sun opens the existing explicit
  Scene lighting / Editor lighting choices; its state reflects editor lighting.
- The left selected tool represents the existing single-object selection mode;
  activating it returns focus to the canvas, without editing sources.
- Dragging the orientation disc orbits around the retained pivot. Signed X/Y/Z
  endpoints align the camera to their respective directions. Repeating an aligned
  direction switches to its opposite, matching the Front/Back example.
- Orientation display follows the actual camera rotation, including flight and
  other camera commands. It is not a static perspective SVG. Use Y-up; Top has
   screen-up −Z. Depth ordering and signed endpoints remain distinguishable.
- Pointer axis alignment uses a 200ms cubic-Bezier `(0.77, 0, 0.175, 1)` turn.
  Camera rotation and gizmo share the same interpolated pose and retained pivot
  radius. A new click retargets from the current pose; repeating the requested
  side flips it even during a turn. Dragging remains direct, without a trailing
  animation. Escape, focus loss and camera retirement stop the turn. The existing
  reduced-motion preference applies the target immediately; keyboard commands
  remain immediate. Hits follow rendered endpoint geometry and depth order.
- Zoom and Pan are LMB-drag controls, not fake click buttons. Frame selection and
  Projection reuse existing commands, including failure on unavailable bounds and
  apparent-scale preservation when changing projection.
- Retain RMB flight/WASDQE, Shift speed, flight-wheel speed, Alt+LMB orbit, MMB pan,
  focused wheel navigation and focused F. New controls must not weaken ownership.

Use exact local icon exports with density metadata. Preserve Lucide ISC notices
for hand, zoom-in, scan and square-dashed-mouse-pointer. Camera-projected axes and
disc are dynamic geometry, not screenshot assets. Do not import the fixture drawing.
No new dependency, camera-controller framework or runtime/editor edge is intended.

## Ownership, failures and accessibility

Control gestures belong to one view, camera, window and layout revision. They
must consume the initiating LMB rather than picking through or starting docking.
Release, Escape, focus loss, menu opening, tab switch, resize/host transfer and
retirement cancel capture and clear locally owned keys. A foreign-window release
must not consume the owning gesture or make it sticky. Restore cursor state.

Unready, failed, retired, hidden, UI-only and image-only views must not advertise
or acquire spatial navigation. UI-only previews retain their fixed fitted canvas.
Selection actions remain scoped to an active document; raw GLB views have no
manufactured authoring selection. Overlay hiding must not discard selection bounds
needed by Frame selection or require reselection when overlays are restored.

All functional targets need accessible labels, keyboard focus and visible focus
states. Expose click commands via the keyboard and View; existing keyboard camera
navigation remains available without dragging small controls.

## Acceptance

Run focused Rust regression tests, icon/density tests, Python QA-tool tests and
`cargo xtask check`. Validate real renderer frames and input on the generated
Scene View fixture: spatial, mixed/UI and raw PNG/GLB targets, wide and 480 × 360
layouts, narrower docks, standard views, all new gestures, cancellation, tab
switching, Float/Redock and unchanged project source bytes.

Compare chrome geometry and states against Figma, not the illustrative mesh pixels.
Record actual evidence under `target/`. Compilation and RPC acknowledgements alone
do not establish visual acceptance. Physical RMB/MMB, cursor-lock and monitor/DPI
behavior remain explicitly unverified unless exercised on native hardware.
