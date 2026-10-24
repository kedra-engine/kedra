# Scene View

Scene View previews supported scene/prefab sources and raw PNG/static-GLB assets.
Its camera, lighting choices, grid and selection overlays are inspection tools. They do
not move source nodes, modify component values or provide Play, Save or transform gizmos.

Open a source through [Assets](asset-browser.md). A document tab shares its selection
with [Hierarchy](hierarchy.md) and [Inspector](inspector.md); raw asset previews do not
create source-document nodes.

For the rendering data behind these previews, read [Graphics and UI](../rendering-ui/index.md);
for lighting replacement and canvas ownership, see
[Preview stages](../rendering-ui/preview-stages.md).

## Prepare resources

Source admission and rendering are separate. Hierarchy and Inspector can work without a
rendered frame. Scene View additionally needs a verified project delivery bound to the
same project's source root, namespace `game`, and matching admitted bytes and identities.

Build the project delivery, then launch with the project path, editor `--assets PATH`
and project `--project-assets PATH`. Follow the complete
[delivery workflow](../assets/pipeline.md#editor-previews-need-matching-sources).
Open does not build resources or initialize missing metadata. Changing the delivery
binding requires relaunch; opening another project cannot silently reuse the old binding.

The preview profile accepts byte-preserving documents, PNG, static GLB, CSS and fonts.
A generally valid processed delivery is not necessarily suitable: transformed output
without the required source provenance is rejected. The matching check covers the
admitted inventory, not just the currently visible node's texture.

## Supported content

| Target | Preview behavior |
| --- | --- |
| Spatial scene/prefab | Supported transforms, cuboids/spheres, opaque surfaces, directional suns, scene environments and static models. |
| Passive UI document | Authored UI on a fixed 1280 × 720 canvas fitted without aspect distortion. |
| Mixed spatial/UI document | Viewport-sized spatial view with a separately fitted transparent UI overlay. |
| `.png` | Image-aspect preview; no spatial camera controls. |
| Static `.glb` | Inspect `Scene0` in an editor-owned model-preview stage, initially using Editor lighting. |

Supported spatial keys are `scene.transform`, `scene.primitive`, `scene.surface`,
`scene.sun`, `scene.environment` and `scene.model`. Primitive and surface data must occur
together. UI rendering data requires `ui.layout`; UI and spatial data require separate
nodes, and a spatial/non-layout node cannot descend from a UI-layout node. Required
external ports and unsupported component schemas produce diagnostics rather than being
silently ignored. Source navigation may still be available.

The passive UI is not an interactive running interface. Its buttons do not issue
commands, text fields do not take focus, and scripts do not run. Resizing/docking fits
the fixed canvas rather than changing the authored layout. In mixed documents this
fitting does not letterbox the spatial camera or shrink its mesh-picking region.

Imported model cameras, lights and animations are disabled; skins and morphs are rejected.
This is not support for every Bevy renderer feature or model format. Shadows, skybox/IBL
and arbitrary material specializations are outside this profile.

## Camera controls

Controls apply only to a ready spatial view. Click the viewport to give it focus before
using the wheel or `F`; hovering alone must not steal keyboard focus from a search field.
LMB, MMB and RMB below mean the left, middle and right mouse buttons.

| Gesture/key | Action |
| --- | --- |
| Alt + LMB drag | Orbit around the retained pivot. |
| MMB drag | Pan. |
| Wheel over the focused viewport | Dolly in perspective; zoom in orthographic projection. |
| Hold RMB and move the mouse | Look around with a temporarily locked/hidden cursor. |
| W/A/S/D while holding RMB | Move along the camera's forward/right axes. |
| Q / E while holding RMB | Move down/up along world Y. |
| Shift while flying | Increase movement speed. |
| Wheel while holding RMB | Adjust bounded flight speed. |
| `F` with viewport focus | Frame selection; unavailable bounds produce a diagnostic. |
| Escape | Cancel navigation/alignment; release the navigation hold. |

Release the initiating button to end a drag or flight. Focus loss, docking changes and
hidden/retired views also cancel navigation. A gesture belongs to one view and window;
typing in another panel or window must not drive that camera.

The orientation control follows the camera in a Y-up coordinate system. Click a signed
axis endpoint to align; repeat the aligned direction to flip to its opposite. Drag the
disc or an endpoint to orbit instead. Pointer alignment turns smoothly and can be
interrupted; reduced-motion preference makes it immediate.
The adjacent strip provides LMB-drag Zoom/Pan, Frame selection and projection toggle.
These controls change the view, not the selected object's transform.

## Toolbar and menus

**View** offers **Frame selection**, **Frame all**, **Front**, **Back**, **Left**,
**Right**, **Top**, **Bottom**, **Perspective**, **Orthographic** and **Clear selection**.
Standard-view commands are available without a numeric keypad. Projection switches
preserve apparent scale at the pivot. Frame all provides a useful way back after navigation.

**Select** offers **Clear selection**; it is disabled without a document selection.
Below 640 logical pixels of viewport width, Select hides, but Clear selection remains
in View. Short/small viewports hide unreachable navigation controls. The left selection
tool focuses object selection; it is not a transform tool or an additional editing mode.

- **Eye:** hide/restore selection bounds, including UI borders. It does not hide source
  objects, clear selection, change grid state or remove framing bounds.
- **Grid:** toggle the infinite grid and colored X/Z axes. Spacing is one source unit,
  not an automatic conversion to project units.
- **Sun / lighting menu:** choose **Scene lighting** or **Editor lighting**.
  Documents start with Scene lighting; raw GLB inspection starts with Editor lighting.

Editor lighting replaces this view's scene background/ambient and suppresses its authored
light contribution; it does not add another light on top. Scene lighting restores the
accepted authored settings. A scene without an environment does not receive a hidden
studio fallback. These choices never edit environment assets or source lights.

Camera pose, projection, pivot and flight speed survive tab switches, resize and
Float/Redock within the session. They are not source values or restart-restored settings.
UI-only and image previews keep their fixed presentation and disable spatial commands.

## Select source nodes

A completed plain LMB click selects the nearest eligible **opaque mesh triangle** in
that document view. Empty space clears selection. Navigation drags and modified clicks
do not select through the viewport. Imported model parts map back to their exact source
model node, including nested prefab instance scope; helpers and other views are excluded.

This is not UI picking or pixel-accurate visual picking. UI, alpha-masked/blended surfaces
and backfaces are not supported selection targets. An eligible opaque mesh behind an
unsupported surface may be selected. Use Hierarchy for nonrenderable nodes and UI nodes.

Hierarchy-selected spatial nodes receive bounds without moving the camera. Selected UI
nodes receive a noninteractive border following their actual layout box, transform and
inherited clip. Selecting a UI parent outlines its own box, not a union of descendants.
Hidden, empty or degenerate UI boxes may have no border. Layout updates do not require
reselection. The same UI-border behavior applies to UI-only and mixed documents.

## Example: inspect a mixed scene

For fixture and delivery setup, follow [launch instructions](../getting-started/launch-editor.md)
and [your first document](../getting-started/first-document.md).
For source versus projection, see [Scenes and prefabs](../scenes/index.md).

With the repository's test project and a matching delivery, open `Composed.scene.ron`:

1. Select **Warm sphere** in Hierarchy; Inspector follows, and the camera stays put.
2. Choose **View → Frame selection**, then Alt-drag to orbit the sphere.
3. Choose **Orthographic**, then **Front** to compare the view without changing sources.
4. Select the **title** node under **Passive UI fixture** in Hierarchy. Its UI border
   follows the fitted HUD; clicking that caption in Scene View is not UI-node selection.
5. Open `ship.glb` in Assets. A model preview opens without an authored Hierarchy target.
   Return to the document tab to recover the document's selected UI node.

The fixture's `ship.glb` is a generated two-material model, not an external game asset.
`Hud.prefab.ron` offers the same passive UI without the spatial scene.

## Loading and diagnostics

The view becomes ready only after resources load and the renderer supplies a current frame.
Controls disable while unavailable, failed or retired. Resizing can require a fresh
frame; hidden tabs stop rendering and rebuild targets when shown. Missing/stale delivery,
unsupported schemas, dependency failures and exhausted resource budgets report diagnostics.
Do not treat a black view alone as proof of bad source admission or successful rendering.

Targets are capped at 2048 pixels per edge and share a 64 MiB color-target budget.
Each view rejects more than 4096 presentation nodes or 256 generated primitive meshes.
These are rejection limits, not truncation or total GPU-memory guarantees.

Physical mouse/trackpad behavior, RMB cursor lock, MMB pan, IME and monitor/DPI transitions
still need separate native checks.
