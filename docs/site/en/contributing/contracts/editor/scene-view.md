# Read-only Scene View contract

Scene View projects admitted sources into isolated read-only views under
[ADR 0017](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0017-read-only-scene-view.md)
and
[ADR 0018](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0018-scene-environment-and-preview-stages.md).
See the [Manual](../../../manual/editor/scene-view.md) for preview setup and camera/selection
instructions, and [controls](scene-view-controls.md) for control geometry and gesture leases.
Native verification remains incomplete. Transform gizmos, Inspector editing,
Save, animation playback, Play and project/plugin execution are not provided.

## Admission and supported targets

Source validity and render support are separate. Documents admit version-1
`scene.transform`, `scene.primitive` (cuboid/sphere), `scene.surface`, `scene.sun`,
`scene.environment`, `scene.model` and the explicitly supported passive authored UI schemas.
`projection::supported` rejects unknown rendering schemas, required external ports,
primitive/surface pairs missing either member, UI presentation without `ui.layout`, combined
UI/spatial data on one node and spatial/non-layout descendants of UI-layout nodes. It never
binds ports to serving-editor entities or silently drops unsupported components.

UI-only sources use a fixed 1280 × 720 native canvas fitted without aspect distortion;
docking does not change authored layout. Mixed documents overlay a separately fitted
transparent UI target on a viewport-sized spatial target. Both targets must report a frame;
UI fitting does not letterbox spatial rendering or its picking region. Image-only targets
use image aspect. UI/spatial forests are native presentation structures, not source rewrites.

Direct PNG/static-GLB Open creates distinct `asset-preview` views bound to admitted paths/
metadata, not editable sessions. Activating one clears the active source-document target;
Hierarchy cannot show the generated inspection rig as authored data. Raw GLB selects `Scene0`
through Bevy's native glTF loader and `WorldAsset` subassets. Imported cameras, lights and
animations are disabled; skins/morphs are rejected. Unsupported native camera/light data,
nonfinite transforms, missing standard materials and external model parents fail explicitly.
No external model, private prototype or project code is bundled by this mechanism.

## Explicit delivery binding

The host verifies the complete delivery before startup and binds the project source root,
namespace `game` and current `editor-desktop` profile explicitly with `--project-assets PATH`,
separate from editor `--assets PATH`. Asset sources register before AssetPlugin initialization;
changing binding requires relaunch. Another project's root fails rather than borrowing the
old delivery. Source Open never initializes sidecars, builds assets or rewrites catalogs.

`binding::Delivery::matches` checks admitted documents' path/UUID/hash against delivery
records/files and every other delivered asset against admitted path/identity/hash. Dependency
loading rechecks the requested native type against the manifest's loader identity and
matching inventory path/UUID/hash/byte length, including symbolic path requests. A record
admitted as an image cannot be requested as a model or stylesheet. The initial profile
supports byte-preserving documents, PNG, static GLB, CSS and fonts. Transformed outputs need
source provenance before support can be claimed. Missing/stale/mismatched delivery reports
a diagnostic while source navigation remains available. [Source limits](source-documents.md)
still apply: a GLB extension does not waive the 8 MiB per-asset admission boundary.

## Projection ownership and isolation

`kedra_editor::scene_view` owns the ordinary readiness model and native binding, projection,
GPU-readiness and target-lifetime adapters. One pipeline serves documents and disposable raw
inspection rigs; rigs never enter authoring inventory/history. The document coordinator owns
accepted sources and full scoped identities. Compiler/instance mappings own source remapping.
Imported visual data uses native `DynamicWorldBuilder`, an explicit audited component filter
and no resources. Bevy 0.20 resource entities are excluded. Native hierarchy remapping finishes
before complete model roots attach to their owner. Imported parts remain presentation children
of the source model node, not new source nodes; failed mapping reclaims staged allocations.

Visible serving previews require continuous Bevy updates, including after a frame
is ready; unchanged entities are not evidence that a viewport may be throttled.
Hidden, failed and retired owners release that demand. This conservatively keeps
the entire shared App active, not an independently ticking project App. The
Scene/Game execution boundary and proposed workflow are defined by
[ADR 0046](../../../../../decisions/0046-scene-game-runtime-boundaries.md); they do
not authorize project code execution or implement Play here.

The shared editor World runs only audited presentation hydration for these instances:

- Each view has its own render layer for meshes and directional lights. Shadows are disabled.
  Background/ambient come from camera-local `kedra_scene::rendering::environment`, never global
  ambient, fog or project post-processing. Unassigned Scene lighting is black/zero ambient,
  with no hidden studio fallback.
- View-owned cameras render to native images, not the serving window. Explicit no-LUT
  Reinhard tonemapping matches the minimal renderer feature set. Inert spatial schemas in
  `kedra_scene::spatial` add no gameplay/render systems; opt-in shared helpers hydrate native
  primitives, opaque standard surfaces and suns. Optional base-color texture, metallic and
  roughness are supported; shadows, arbitrary formats and material specializations are not.
- `UiView::mount_passive` shares hydration but installs no button/input, autofocus, modal
  ownership or public observation identities. Source `ui.id`/`ui.screen` cannot collide with
  the live shell. UI roots detach into native UI trees with explicit offscreen cameras;
  source mappings/hierarchy remain unchanged. Both forests have explicit idempotent cleanup.
  Passive resource failures stay instance-local, never the shell's global `UiFailure`.

## Readiness, allocations and native recipients

Unbound, waiting dependencies, frame-pending, current, failed and retired are distinct.
Native pipeline readiness and actual target readback gate a current frame, not source
compilation alone. Readbacks check camera identity and target epoch/size; stale callbacks
cannot revive readiness. Pipeline failures are conservatively shared-renderer failures,
not guessed source-shader attribution. Dependency/stage waits and renderer-readback timeouts
report failure while preserving accepted sources.

Targets are capped at 2048 pixels per edge and share 64 MiB of color targets. Depth/MSAA,
source assets and other GPU resources are additional. Hidden tabs disable cameras, cancel
readiness and release targets; retained instances recreate targets when shown. One view
rejects more than 4096 presentation nodes including imported model parts, or 256 generated
primitive meshes. These are admission failures, never silent truncation. Initial framing
uses native bounds/composed transforms; empty scenes receive a finite default pose.

Unextracted readback recipients are destroyed immediately on cancellation. Extracted native
recipients outlive logical camera retirement until Bevy completes/reclaims them. Their images
remain charged to the shared color budget until the final recipient releases them, including
multiple epochs sharing an image. Eight outstanding native readbacks, including cancelled
epochs, bound request/buffer accumulation. Non-completion retains capacity and reports a
diagnostic; at the limit new requests fail. Renderer completion or host restart is required,
not a timer that destroys a recipient still owned by native rendering.

## Lighting, navigation and input geometry

Raw GLB starts in Editor lighting using ordinary delivered
`editor://scenes/model-preview.scene.ron` and referenced `environments/studio.environment.ron`.
Documents start in Scene lighting. Editor mode replaces background/ambient and suppresses
that view's authored light contribution, not additive illumination. Returning to Scene mode
restores accepted settings. Environment assets, scene-root assignment, camera-owned settings
and preview stages follow
[ADR 0018](https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0018-scene-environment-and-preview-stages.md).
Version 1 supports solid background/ambient only, loaded by SceneAssetPlugin with a 64 KiB
bound and verified metadata/delivery. Only a scene root may declare, for example,
`scene.environment` with `(asset: Path("game://day.environment.ron"))`; prefabs/descendants
cannot replace the host environment. Editing/assignment/Save and skybox/IBL remain unsupported;
external changes require a fresh matching delivery/relaunch.

Pose, projection, pivot and flight speed survive tabs, resize and Float/Redock as view state,
not source or restart authority. UI/image targets retain their independent fixed canvas
without spatial navigation. The focusable viewport owns input; its fitted render-image child
is decorative. Window-scoped cursor samples shared with docking keep input/render geometry
in one logical coordinate domain. Focus, lease cancellation and control gating are specified
in [Scene View controls](scene-view-controls.md#input-ownership-and-cancellation).

Pinned Bevy 0.20 provides PanOrbit motion, mesh casting, retained gizmos and infinite grid.
Only PanOrbitCamera's motion system is installed, without global input or automatic
orthographic repositioning; framing/projection are editor policy. Stock FreeCamera is not
installed because it reads global input and can change other windows' cursors. A small
leased flight adapter applies bounded native Transform operations, not a second controller
framework. There is no custom ray/triangle engine, grid shader or scene scheduler.

## Scoped hits and selection overlays

A completed plain LMB click resolves the nearest eligible visible opaque native mesh
triangle; empty space clears selection. Imported descendants map to their exact source model
node and nested `NodeAddress`. Completion rechecks view/camera, readiness, layout revision
and host ownership; navigation drags never select. Helpers and other views are excluded.
UI, alpha-mask/blended surfaces, backfaces and pixel-accurate visual picking are not supported;
an eligible opaque surface behind an unsupported surface may win.

The coordinator remains selection authority. Hierarchy updates a disposable marker without
moving the camera. Spatial bounds use retained native gizmo assets on the view layer with
no source-material changes. Both spatial and UI selection use sRGB `(177, 210, 255)`.
UI selections use a separate native border on their own offscreen canvas camera. After native
layout, clipping, stacking and visibility propagation it copies the selected node's actual
`ComputedNode` box, full affine `UiGlobalTransform` and inherited `CalculatedClip`. A parent
uses its own box, not descendant union. No affine decomposition or custom layout solver runs.

The UI border is composited through the same fitted image, avoiding another mixed-scene/
target-scale coordinate conversion. It neither changes passive styles nor participates in
picking/focus. Native border rendering and `Pickable::IGNORE` own decoration. Missing, empty,
hidden, nonfinite or degenerate geometry suppresses it; deselection/non-UI selection removes
it. Camera retirement owns cleanup. Layout changes update without reselection, including
rotated/nonuniform transforms and inherited clipping. Hiding overlays preserves selection,
grid, source visibility and framing bounds; UI-only views retain this option without a
spatial camera.

## Verification and remaining acceptance

Production and local tests live under
[`scene_view`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/scene_view/mod.rs),
with window input in
[`workspace/scene_input.rs`](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/workspace/scene_input.rs).
Local tests cover readiness/delivery mismatch, native GLB mapping/bounds, budgets, render-layer
isolation, passive input/identity ownership, recipient retirement and disposal. UI-selection
tests run real Bevy layout for nested/parent boxes, unchanged-selection layout/scale changes,
rotated nonuniform transforms, clipping, hidden/empty/degenerate geometry and cleanup.
They do not establish rendered UI-border acceptance.

```sh
cargo xtask test -- features --only scene-view,scene-navigation --output target/test/scene
```

Native QA uses a fresh checked-project copy under `target/`, refuses overwrite, preserves
the Inter license and builds matching deliveries. Existing PNG, UI, mixed/spatial and static
GLB fixtures are not generated at startup and never depend on owner files. The checked model
was generated from Bevy cuboids with two opaque unlit cyan/yellow materials; its source-
geometry/two-material contract has its own test. Manual fixture/delivery preparation belongs
to [the delivery workflow](../../../manual/assets/pipeline.md) and
[Scene View](../../../manual/editor/scene-view.md), not a second launch recipe here.

The scenarios open PNG/GLB/UI/mixed targets, fence renderer completion, move mixed views
through Float/Redock, exercise serving text input and assert unchanged source bytes.
Navigation covers injected orbit/dolly, scoped nested selection/deselection, axis alignment/
flipping, orbit/zoom/pan controls, Escape/release, overlays, views, projection, lighting/grid
and retained transfer. Injection currently supplies LMB and tapped keys only; RMB flight,
MMB pan and OS cursor-lock quality require additional native input acceptance.

Scene View has no active screenshot goldens, saved PNG comparisons or synthetic screenshot
tests. GPU fences confirm that rendering completed; they do not compare rendered content.
UI-only/mixed border acceptance
still needs native Hierarchy input and actual frame inspection at fitted/resized viewports.
Physical mouse/trackpad, IME, monitor/DPI transitions and all material types are not certified.
Native verification of this revision remains pending. Keep evidence under `target/`.
