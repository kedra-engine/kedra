# Read-only Scene View

Scene View implements the read-only projection, spatial navigation and scoped
mesh-selection profile of [ADR 0017](../decisions/0017-read-only-scene-view.md).
It uses retained document tabs and the existing docking system. Transform gizmos,
Inspector editing and Play are not implemented. Native acceptance is separate
from compilation; the opt-in scenarios below describe its verified boundary.

## Supported targets

- Admitted scene/prefab documents using version-1 `scene.transform`,
  `scene.primitive` (cuboid/sphere), `scene.surface`, `scene.sun`,
  `scene.environment` and `scene.model`
  schemas, plus existing authored UI schemas.
- Passive UI prefabs and UI subtrees in mixed spatial documents. UI renders on
  an independent 1280 × 720 canvas, fitted without aspect distortion to the panel;
  docking does not change authored layout. In mixed documents, a transparent UI
  target overlays a separate viewport-sized spatial target: fitting the UI never
  letterboxes the 3D scene or its picking region. Both targets must report a frame.
  Image-only previews use image aspect.
  Spatial and UI data require separate nodes; UI branches contain native layout
  nodes rather than spatial descendants. Unsupported mixtures fail explicitly.
- Direct `.png` image and static `.glb` model Open from Assets. These are distinct
  `asset-preview` views bound to admitted asset paths/metadata, not editable source
  document sessions. Activating one clears the active document target, so Hierarchy
  does not pretend the asset's generated inspection rig is an authored document.

Static models use Bevy's native glTF loader and `WorldAsset` scene subassets.
Direct GLB inspection selects `Scene0` and starts in Editor lighting, using the
ordinary delivered `scenes/model-preview.scene.ron` stage and its referenced
`environments/studio.environment.ron` asset. Document projections start in Scene
lighting. Imported cameras, lights and
animations are disabled. Skins and morphs are rejected, not partially activated.
No model file, project code or private prototype implementation is bundled here.

The spatial schemas are inert runtime data in `kedra_scene::spatial`; registration
adds no rendering/gameplay systems. Shared opt-in runtime helpers hydrate meshes, materials
and directional lights, rather than authorizing hooked renderer types in source
worlds. Surface materials are opaque, with optional base-color texture, metallic
and roughness. Shadows, arbitrary model formats and material specializations are
not supported by this slice.

## Explicit asset binding

Build a project delivery with the existing CLI, then launch with an explicit
project root and `--project-assets PATH` as well as the editor's `--assets PATH`.
The project delivery uses namespace `game` and the current `editor-desktop`
profile. Source registration must occur before AssetPlugin initialization, so
changing that binding currently requires relaunch. Switching to another project's
source root fails binding explicitly; it never silently borrows the old delivery.

The complete delivery is verified before startup. On Open, its document bytes,
identities and delivered asset bytes must match the immutable admitted inventory.
This initial profile supports byte-preserving documents, PNG, GLB, CSS and fonts.
Processed resources whose output differs from their source need source provenance
before support can be claimed. A missing/stale/mismatched delivery produces a
diagnostic while source navigation remains usable. Open never builds assets,
initializes metadata or rewrites catalogs. Admission still has its existing
[scope and size limits](source-documents.md); a raw research model exceeding those
limits is not made admissible merely because its extension is GLB.

## Ownership and isolation

`kedra_editor::scene_view` owns a small readiness model and native adapters for
resource binding, model projection, GPU readiness and target lifetime. There is
one pipeline for documents and disposable raw-asset inspection rigs. Rigs are
never inserted into the authoring workspace, document inventory or history.

The source coordinator owns accepted sources and composed scoped identities.
Bevy's compiler/instance path owns source entity remapping. Static imported visual
data uses native `DynamicWorldBuilder` and mapping, with an explicit component
filter and no resources. Native hierarchy remapping completes before model roots
attach to the view owner. Bevy 0.20 resource entities are excluded, matching the
native `WorldAsset` contract. Additional model parts are presentation children of
their source model node, not new authored nodes.

Rendering uses the editor's World with audited presentation-only data and systems:

- A distinct render layer per view isolates native meshes and directional lights.
  Shadows are disabled. Background and ambient illumination are camera-local,
  applied by `kedra_scene::rendering::environment`; no global ambient resource,
  fog or project post-processing is installed. Unassigned scenes get black/zero
  ambient, never a hidden studio fallback.
- View-owned cameras render into native image targets, not the serving window.
  The explicit no-LUT Reinhard tonemapper matches the minimal renderer features.
- `UiView::mount_passive` shares native hydration but installs no button/input,
  autofocus, modal ownership or public observation IDs. UI roots detach from
  spatial parents into native UI trees, each with an explicit offscreen target
  camera. Source hierarchy/mappings remain unchanged. Both native forests have
  explicit, idempotent cleanup. Passive UI resource errors are instance-local,
  never the serving shell's global `UiFailure`.
- Readiness distinguishes unbound, dependencies, frame-pending, current, failed
  and retired. Native pipeline state and actual target readback gate the current
  frame. A readback's camera identity and target epoch/size reject stale results.
  Pipeline errors are conservatively reported as shared-renderer failures, not
  falsely attributed to one source shader.

Targets are capped at 2048 pixels per edge and a shared 64 MiB color-target budget
(depth/MSAA and source resource memory are additional). Hidden tabs cancel readiness
and disable cameras; retained instances recreate targets when shown. Unextracted
readbacks are destroyed immediately. Extracted native recipients outlive retired
cameras until Bevy completes and reclaims them; stale callbacks cannot restore
readiness. Their target images remain within the shared color-target budget until
the final outstanding recipient is reclaimed. Eight outstanding Scene View native
readbacks (including cancelled epochs) bound request/buffer accumulation. Native
non-completion retains that capacity, emits a diagnostic and rejects further
readbacks at the limit; it requires renderer completion or a host restart, not
another timer that destroys an unsafe recipient.
One view admits at most 4096 presentation nodes, including imported model parts,
and 256 generated primitive meshes. These are rejection limits, not truncation.
Initial finite framing uses native mesh bounds and composed transforms; an empty
scene has a finite default pose.

## Spatial navigation and selection

The initial environment asset and authored lighting stage follow
[ADR 0018](../decisions/0018-scene-environment-and-preview-stages.md). The native
`.environment.ron` version-1 asset supports a solid background, ambient color and
brightness; skybox/IBL fields are not accepted. Only a scene root can declare
`scene.environment` with `(asset: Path("game://day.environment.ron"))`.
Prefabs/descendants cannot replace the host environment. Loading uses the shared
SceneAssetPlugin loader, bounded to 64 KiB, and explicit verified metadata/delivery.
Changing source settings currently requires an external edit and a fresh matching
delivery/relaunch. In-editor asset assignment/editing/Save is not implemented.

Spatial views retain their camera pose, projection, pivot and flight speed through
tab switches, target resize and Float/Redock. This state is presentation-only and
is not restored as source authority after restarting the editor. Passive UI and
standalone images retain their independent fixed canvas; spatial camera controls
are disabled for UI-only targets.

- RMB holds a window-owned flight lease. Relative mouse motion looks around;
  WASD moves in the view plane, Q/E moves vertically, Shift increases speed and
  the wheel adjusts bounded flight speed. The native cursor is locked/hidden only
  for the lease, and the previous cursor options are restored on release.
- Alt+LMB orbits about the retained pivot; MMB pans. Wheel dollies or zooms an
  orthographic view only when that viewport owns keyboard focus. Hover alone
  never acquires keyboard ownership.
- `F` frames selection. Views exposes Frame selection, Frame all and six standard
  Y-up views without requiring a numeric keypad. Perspective/Orthographic switches
  preserve the apparent scale at the pivot. Empty/nonrenderable selection keeps
  its source identity and reports unavailable bounds rather than inventing geometry.
- Grid toggles Bevy's infinite grid and colored X/Z axes. Its spacing is one source
  unit, not a project-unit conversion. Lighting explicitly switches between authored
  scene lighting and view-owned Editor lighting. Editor lighting replaces ambient/
  background and suppresses this view's authored light contribution; it is not
  additive. Returning to Scene lighting restores the scene's accepted settings.

The focusable viewport container owns input; its fitted render-image child is
decorative. Window-scoped cursor samples are shared with docking, so input and
renderer geometry use the same logical coordinate domain. Menus, fields, host
creation and docking gestures block acquisition. Focus loss, Escape, release,
layout revision changes, hidden/retired views and workspace retirement cancel a
lease and clear its locally owned keys; another window's input cannot drive it.

A completed plain LMB click selects the nearest eligible opaque mesh triangle;
empty space clears selection. Native Bevy mesh casting handles intersection.
Imported mesh descendants resolve to their exact source model node through the
instance mapping, retaining nested prefab address scopes. Click completion
rechecks view/camera identity, layout revision, readiness and native host ownership.
Navigation drags do not select. The coordinator remains selection authority;
Hierarchy and Scene View consume the same optional source address. Hierarchy
selection updates a disposable bounds marker without moving the camera. Markers
use native retained gizmo assets for spatial nodes, distinct render layers and no
source-material edits. Hierarchy-selected UI nodes get a separate native UI border
in the same selection color, on their own offscreen canvas camera. After each
native layout pass it copies the selected node's actual computed box, full affine
transform and inherited clipping; parent selection uses that parent's box, not a
descendant union. The frame is composited through the same fitted image as the UI,
so mixed-scene fitting and target scale changes do not introduce another coordinate
conversion. It neither changes passive element styles nor participates in picking,
focus or source editing. Missing, empty, hidden or degenerate geometry suppresses
the frame; deselection/non-UI selection removes it, and camera retirement owns
its final cleanup. Layout changes do not require another selection event.

This profile does not claim UI, alpha-mask/blended-material, backface or
pixel-accurate visual picking. Those surfaces do not participate in opaque-mesh
selection; an eligible opaque surface behind them may be selected. Helpers and
objects from other views never enter the hit filter.

### Reuse decisions

Pinned Bevy 0.20 supplies `PanOrbitCamera`, native mesh casting, retained gizmos
and `dev_tools::infinite_grid`. The adapter installs PanOrbitCamera's motion system
without default global input or automatic orthographic camera repositioning;
framing and projection are editor-owned policies. No custom ray/triangle engine,
grid shader or alternative scene scheduler is introduced.

UI frames reuse Bevy's `Node`, `ComputedNode`, `UiGlobalTransform`, native border
renderer and `CalculatedClip`. Only the view-owned overlay's computed geometry is
updated after native layout, clipping, stacking and visibility propagation. No
custom layout solver, affine decomposition, shader or new dependency is needed.

Bevy's stock FreeCamera reads global input and can alter cursor options across
windows. It is not installed: the small flight adapter instead receives only
leased window input and applies bounded native Transform operations. This is
viewport policy, not a new reusable controller framework.

## Design and acceptance

The panel follows UI-KIT `108:9202` / toolbar `108:8644`: existing dock chrome,
38px toolbar, 128px projection select, 68px Lit select, 26px grid target, flexible
space and right-aligned settings. Editor lighting expands its select to 88px so
the value remains readable. Projection and lighting open native shared menus;
settings exposes framing and standard views. Source title stays in the dock tab,
not duplicated in the toolbar. Readiness diagnostics retain their overlay.

Static icons are exact local Figma exports: `scene-down.svg` (`10:2399`),
`scene-grid-selected.svg` (`10:2488`) and `scene-settings.svg` (`10:2482`), with
1×–4× density packages and fixed 16px slots. The existing raised/flat button
families and tokens own states. Native Flair does not accept inset box shadows;
pressed top-edge treatment uses the existing border mechanism, not a new shader.
Read-only World / 1 unit replaces the reference's Local / 1 m: there is no local
transform tool or project-unit conversion yet. It is not a fake active selector.
The Figma illustration is not used as an implementation image or visual baseline.

Generate isolated fixture sources under `target/`. The fixture generates a static
GLB from native Bevy cuboids with two opaque unlit materials (cyan/yellow); no
external model, old metadata/cache or project code is imported:

```sh
cargo run -p kedra_editor --example scene_view_fixture -- target/scene-fixture
cargo run -p kedra -- assets init 'target/scene-fixture/Scene View QA/assets'
cargo run -p kedra -- assets build 'target/scene-fixture/Scene View QA/assets' target/scene-game --namespace game
cargo run -p kedra -- assets build assets/editor target/scene-editor --namespace editor
cargo run -p kedra -- 'target/scene-fixture/Scene View QA' --assets target/scene-editor --project-assets target/scene-game \
  --remote-port 0 --remote-credentials target/scene-credentials.json \
  --remote-artifacts target/scene-captures --state-directory target/scene-state
python3 tools/qa/scene_view.py target/scene-credentials.json \
  --project 'target/scene-fixture/Scene View QA' --state-directory target/scene-state --report target/scene-report.json
KEDRA_SCENE_REPORT=target/scene-report.json cargo test -p kedra_editor --test scene_view_frames -- --ignored
python3 tools/qa/scene_navigation.py target/scene-credentials.json \
  --project 'target/scene-fixture/Scene View QA' --state-directory target/scene-state --report target/navigation-report.json
```

The fixture refuses to overwrite its parent, records generated-model provenance/hash
under `target/`, and retains the copied Inter font license. It creates PNG,
UI-prefab, spatial and mixed-scene examples, including the generated model in the
mixed scene. Tests never depend on the owner's model/files.

Headless coverage exercises readiness, delivery mismatch, native GLB metadata,
hierarchy remapping, bounded allocation, isolated render layers, passive input/
observation ownership and disposal. Native QA opens PNG/GLB/UI/mixed targets,
captures actual renderer frames, transfers a mixed view through Float/Redock and
checks serving text input and unchanged source bytes. Frame assertions retain
foreground, nonuniform, image/UI, warm-material and error-color checks and require
both generated model color families independently in raw-GLB and mixed captures.
Synthetic negative cases reject clear-only, grid-only and model-hidden frames;
the old raw-GLB foreground predicate demonstrably accepts the grid-only case.
Unlit materials avoid dependence on lighting intensity; color-family thresholds
allow native tonemapping/antialiasing, not substitute geometry or synthetic native
evidence. Compilation or an RPC receipt
alone is not evidence of rendered contents. Injected input does not certify
physical mouse/trackpad, IME, monitor transitions or all material types. The
navigation scenario exercises injected orbit/dolly, nested mesh selection,
deselection, standard views, projection, lighting/grid and retained host transfer.
The current shared injection API only supplies LMB and tapped keys: RMB flight,
MMB pan and OS cursor-lock quality require additional native input acceptance;
headless camera/ownership tests alone do not certify those interactions.

Headless UI-selection regressions run native Bevy layout for nested/parent boxes,
unchanged-selection layout/scale changes, rotated nonuniform transforms, clipping,
empty/hidden/degenerate geometry and disposal. These checks are not rendered-frame
acceptance of the UI border: UI-only and mixed targets still require native
Hierarchy input and actual frame inspection at fitted/resized viewports. UI visual
picking remains unsupported.
