# ADR 0017: Read-only Scene View and editor camera navigation

Status: direction agreed with the owner; the initial
[read-only projection profile](../ui/scene-view.md) implements resource binding,
supported spatial/UI rendering and PNG/static-GLB inspection. Spatial camera
navigation and scoped opaque-mesh selection are implemented; full native input
acceptance remains a gate before declaring the complete read-only slice accepted.

## Context and scope

Opening a scene or prefab should show its composed visual contents, not only a
source summary. The user must be able to inspect space, fly around and select an
object without changing authored data or starting project execution.

[ADR 0016](0016-document-preview-panel.md) specifies passive UI-document preview;
it explicitly excludes a 3D viewport. This ADR adds a distinct Scene View contract,
not permission to turn UI preview into an interactive application. Both use
[ADR 0012](0012-document-sessions-and-selection.md) document authority and existing
docking. Neither is Play.

The complete read-only slice includes:

- Rendering supported scene/prefab contents, nested instances, meshes, materials,
  textures and authored lighting from a pinned accepted source revision.
- Orbit, pan, zoom, free flight, adjustable flight speed, frame selection/all,
  perspective/orthographic projection and front/back/left/right/top/bottom views.
- An infinite reference grid, world axes and explicit orientation controls.
- Single-object viewport picking and selection feedback synchronized with Hierarchy.
- Per-document navigation state, independent dockable views, loading/error/stale
  states and verified native resize/Float/Redock behavior.

No transform gizmo, placement, rename, reparent, component editing, source writes,
Save, document Undo/Redo, simulation, animation playback, physics, project scripts
or project plugin loading is authorized. Camera movement and selection do not
change source revisions, dirty state or workspace history. Image/model-file asset
viewers are separate from opening existing scene/prefab documents.
The initial acceptance profile includes direct image/model viewers and passive
UI/mixed scenes. They share projection ownership but
remain separate raw-asset targets, not manufactured authoring documents.

## Ownership and projection

Keep a coherent `kedra_editor` Scene View feature module: ordinary Rust values own
navigation, options and lifecycle decisions; ECS adapters own entities, handles,
render targets and native input routing. No speculative crate or rendering framework.

The project-owned authoring workspace remains the only source authority. Each
document has one default document-bound view, using the existing document view
identity rather than opening an additional competing preview tab on every Open.
Scene View owns its disposable instance, editor camera, overlays and readiness.
Additional views of the same document are deferred. Navigation is view-local,
retained across tab switches and docking within the session; restart persistence
is not required for this slice.

Use existing composition, compilation, component policies and instance mappings.
Admission validates source graphs, not runtime resource readiness. The initial
profile registers the concrete spatial/UI schemas documented in the projection
profile. Register only the concrete,
side-effect-free render data needed by supported fixtures through existing policies.
Native renderer components with hooks/required components must use reviewed
presentation hydration, not bypass source component policy. Unknown types remain
explicit admission failures; no silent component dropping or name-based inference.

The initial renderer may share the editor's Bevy World only with explicit scoped
ownership and audited presentation systems. Merely parenting an instance does not
isolate global queries, lights, cameras, input observers or widget hydration.
Rendering isolation must cover view visibility, lights/shadows, environment,
post-processing and picking; a render-layer mask alone is not proof. If required
isolation cannot be achieved with native mechanisms, review a separate Bevy World
host before implementation rather than claiming isolation through convention.

Authored cameras are source objects, not permission to take over the display.
Use an editor-owned camera; hydrate authored cameras as inert inspection data or
explicitly disabled render cameras. Grid, selection overlays, reference lighting
and editor helpers are not document nodes and cannot enter source serialization.
Do not attach live editor actions, UI focus behavior or automation identities to
projected source objects. Loading data must not run project code or gameplay systems.

## Assets and readiness

Keep source admission separate from runtime loading. Reuse verified deliveries,
native AssetServer sources and existing import/build tools. Before rendering, bind
an explicit delivery namespace and dependency inventory to the admitted scope and
accepted revision. Check correspondence; an old delivery must not be reported as
the visual result of newer source data. Do not load arbitrary filesystem paths or
silently register a source directory as a runtime delivery.

Open remains read-only: it does not build assets, create sidecars, rewrite catalogs
or update the serving editor. If a matching delivery is unavailable, show the
preparation required. Any future in-editor Prepare action needs a separately
specified explicit build request, progress, diagnostics and cancellation contract.
Fixtures can prepare and verify deliveries through the existing CLI beforehand.
Unbound external ports remain explicit unsupported/missing requirements; never
bind them to live editor entities just to make a prefab render.

Track source-valid, delivery-verified, dependencies-ready, instantiated and
frame-ready independently. Report missing assets/subassets, unsupported render
data and render failures with actionable source/dependency context. Valid source
documents and Hierarchy remain usable when projection fails. Do not substitute a
demo scene or silently present incomplete geometry as a complete scene.

Pin each preparation to project/scope generation, document and accepted revision,
view generation and delivery identity. Stale completions cannot publish into a
newer view or steal activation. Pending/failed replacement can retain older pixels
only with a stale indication and disabled picking. Dependency failure preserves
accepted sources and selection, not a false rollback. Cleanup retires only owned
instances/targets/helpers; shared assets and other documents remain intact.

## Camera and input contract

Use an editor camera independent of panel placement and authored transforms.
Preserve pose, orbit pivot, projection, orthographic scale and flight speed through
tab switches, resize and host transfer. A new document initially frames finite
renderable bounds; an empty/unbounded scene uses a documented finite default pose
and reports unavailable bounds. Frame selection includes mapped descendants and
handles empty or degenerate bounds without nonfinite camera state.

Current bindings: RMB hold plus WASD/QE for flight, Shift for faster movement,
wheel for flight speed while captured; Alt+LMB for orbit, MMB for pan, wheel for
dolly/orthographic zoom otherwise. `F` frames selection; explicit toolbar/menu
actions expose Frame All, standard views and projection without requiring a
numeric keypad. The [current control profile](../ui/scene-view-controls-design.md)
uses compact 22px controls, a tool rail and orientation navigation; native
input/frame acceptance remains separate from implemented bindings.

Only the focused, visible viewport in the correct native window can acquire
navigation input. Hover alone must not consume typing or global shortcuts. UI
overlays, menus and fields take precedence; gestures cannot also select objects
or initiate docking. Use native controller input adapters gated per view/window,
not unrestricted global mouse/key readers controlling every camera.

Release capture and clear navigation holds on RMB release, Escape, focus loss,
tab change, hide/close, project switch or host retirement. Transfer cancels active
gestures before moving surfaces. Restore cursor visibility/ownership on failure;
never leave a hidden cursor, sticky movement or a camera driven from another host.

## Selection and presentation

Resolve native hits through the exact instance mapping to full scoped
`NodeAddress`, including nested prefab identities. Submit selection through the
document coordinator with matching revision/view generations; rendered entities
never become persistent selection IDs. A completed plain click selects the nearest
eligible visible surface; empty-space click clears selection. Navigation drags and
editor helpers do not select. Transparent/non-mesh picking must declare supported
behavior; do not silently claim pixel-accurate picking for all material types.

Hierarchy selection updates the Scene View marker without moving the camera;
only an explicit Frame command changes framing. Nonrenderable nodes remain valid
selections with an explicit unavailable-bound indication. Hidden/inactive or stale
views reject selection input. Multi-select, marquee and structural commands are
deferred. Highlighting is a reversible presentation overlay, not a source material
edit, and must not leak into another document's render.

Render into a view-owned native target displayed inside retained authored panel
content. Target size follows content extent and host DPR, with explicit allocation
limits and zero-size/hidden handling. UI overlays use shared CSS/tokens/controls;
camera viewport and picking coordinates use the same measured content geometry.
Preserve navigation through docking while acknowledging surface/target transfer.
An old frame is not proof the replacement target is ready. Hidden tabs stop input
and unnecessary rendering, while retained state permits safe recreation.

Authored lighting is the truthful scene mode. A clearly labeled optional inspection
light/environment can make unlit prefabs inspectable without altering their source
or pretending that helper illumination belongs to the scene. Grid/axes visibility
and inspection illumination are view options, not document edits.

[ADR 0018](0018-scene-environment-and-preview-stages.md) defines the shared lighting
contract: reusable environment assets, ordinary light entities, camera-owned display
settings and an authored preview stage. Editor lighting replaces, rather than adds
to, the scene's light/environment contribution for that view using an authored
studio stage and shared background/ambient hydration. Asset assignment
and environment editing are subsequent source-owned authoring work, not permission
to bypass this slice's read-only boundary.

## Reuse, alternatives and consequences

Evaluate pinned Bevy 0.20 `PanOrbitCamera`, `FreeCamera` and
`dev_tools::infinite_grid` first. The pinned examples demonstrate these mechanisms;
multi-window gating, target cameras and isolation still require verification.
Do not assume the presence of an example proves editor-ready integration. Prefer
narrow input/ownership adapters to custom camera mathematics or grid shaders.

Rejected alternatives: passive thumbnails do not support spatial inspection;
running project code to obtain a preview conflates Open with Play; editing live
transforms bypasses source authority; importing a whole editor duplicates docking,
document and UI ownership. A separate World remains a conditional isolation option,
not authorization for another ECS or scheduler.

The concrete navigation profile reuses Bevy 0.20's PanOrbit motion system, native
mesh casting, retained gizmos and infinite grid. Stock FreeCamera's global input/
cursor ownership is unsuitable for retained multi-window views; a small leased
flight adapter uses native Transform operations instead, with explicit per-view/
window input ownership. See the
[projection profile](../ui/scene-view.md#reuse-decisions) for reuse decisions
and supported limits. This avoids a second editor/input dependency stack and
adds no custom mesh-intersection implementation or grid shader.

## Delivery and acceptance

Deliver and demonstrate these slices in order; approval of this plan is not a
request to implement them all without owner inspection:

1. Supported render schemas and verified asset binding; one real scene/prefab
   projection with loading/error diagnostics and an actual renderer frame.
2. Complete camera navigation, grid/orientation/inspection controls and retained
   per-document state, with native focus/capture and dock-host transfer acceptance.
3. Viewport selection and highlighting synchronized with composed Hierarchy,
   including nested instances and failure/cleanup coverage.

The read-only reflected Inspector is connected to source selection. Editing/gizmos
require validated source transactions, graph revisions, selection remapping and history.
Passive UI preview and explicit own-editor source admission remain separate work;
this ADR does not make the current fixed `game` scope accept `editor` sources.

Tests must cover generation retirement, source/delivery mismatch, missing resources,
unsupported schemas, failed instantiation, empty/degenerate bounds, stale hits,
nested identities, visibility isolation and ownership cleanup. Native acceptance
requires input plus renderer frames for flight/orbit/pan/zoom, framing, projection,
standard views, grid, selection, resize/DPR and Float/Redock/tab transitions.
Exercise text-field/menu precedence, focus loss while captured, two open documents
and failure during host transfer. Assert source bytes/revisions/history remain
unchanged and project code never starts. Verify renderable fixtures beyond UI-only
documents and list supported schemas/assets and picking limitations explicitly.

Run `cargo xtask check` and relevant feature/target/native checks on implementation.
Keep evidence under `target/`; injected input does not certify physical mouse,
trackpad, OS capture or mixed-monitor behavior. Presentation must be reviewed
against an actual Scene View design before implementation; the existing Figma 3D
illustration is a reference, not a complete interaction/readiness specification.
