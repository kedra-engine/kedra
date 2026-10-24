# Preview stages and viewport presentation

A preview is a disposable presentation of accepted data, not another editable scene or
a running game. Scene View owns its cameras, generated meshes/materials, render targets
and overlays. Close/Hide hides the retained view, disables rendering and logically releases
its render targets and frame-readiness state. Extracted native readback recipients can retain
images until native completion. Cameras and generated resources retire only when the projection
or workspace actually retires or the projection is replaced, not merely when the view is hidden.
None of these operations saves presentation into sources. See the
[Scene View lifetime contract](../../contributing/contracts/editor/scene-view.md)
for resource ownership and the lifetime of native readback recipients.
For opening resources, binding a matching delivery and camera gestures, see
[Scene View](../editor/scene-view.md).

## Inspect a static model

Direct `.glb` Open creates an inspection rig referring to `game://…#Scene0`, then uses
the ordinary delivered scene `editor://scenes/model-preview.scene.ron` for lighting.
The model remains the selected asset; the rig is not inserted into the project's
document workspace, Hierarchy or undo history.

The shipped stage has a root referencing the studio environment and one child sun with
white color and 15,000 lux. Its transform determines the light direction. It is an ordinary
version-1 scene, not a separate preview-stage format. Its admitted stage components are
only `scene.transform`, `scene.sun` and `scene.environment`; decorative geometry is not
mounted over the inspected content.

The editor loads, compiles and waits for the stage's native dependencies. Missing stage
resources fail explicitly; there is no hardcoded replacement light rig. Spatial document
previews also prepare this stage so that Editor lighting is available, even when they
initially show Scene lighting. A broken editor delivery can therefore block a spatial
preview independently of the project's resources.

Current GLB initialization disables imported cameras, lights and animations. The static
presentation adapter rejects skins, morph weights, imported cameras/lights, nonfinite
transforms and mesh nodes without a supported standard material. It copies only audited
visual components through Bevy's native entity remapping; it does not activate arbitrary
imported behaviors. See [model admission](../assets/supported-formats.md)
for the separate source-format limits.

## Scene lighting and Editor lighting

Authored spatial documents start with **Scene lighting**. Direct GLB inspection starts
with **Editor lighting**. The lighting menu changes the current view, not the source.

**Scene lighting** uses the accepted scene environment and its authored suns. No
environment assignment means zero ambient from this mechanism and a black background,
not an automatic studio fallback.

**Editor lighting** replaces the camera's background/ambient with the stage environment,
hides this projection's authored suns and enables the stage lights. It does not add
studio illumination on top of the scene lights. Returning to Scene lighting restores
the projection's accepted environment and authored light contribution.

Replacement invalidates the current frame confirmation and requests a fresh readback.
It must not present a stale image as the new lighting mode. Other views retain their
own environment/light bindings. The implementation uses view-local cameras, owned light
entities and render layers. GPU/platform combinations still require separate testing.

The navigation camera, grid and selection bounds are editor presentation. They are not
authored cameras, meshes or source materials, and they do not establish game/editor
camera parity. The stage has no user-facing stage picker or visual editing workflow.

## UI-only, image and mixed scenes

A UI-only document uses a fixed 1280 × 720 canvas, fitted into the viewport without
aspect distortion. Resize and docking change the fit, not the authored layout size.
An image preview instead preserves the image's aspect, capped at 2048 pixels on its
longest canvas dimension without upscaling the source canvas.

In a mixed document, the spatial target follows the viewport while UI renders to a
separate transparent 1280 × 720 canvas over it. Fitting the overlay does not letterbox
the spatial camera or narrow its mesh-picking region. This is a screen overlay, not
world-space UI attached to a mesh. UI layout nodes and spatial components must occupy
separate nodes; spatial/non-layout nodes cannot descend from a UI-layout node.

Passive UI roots are detached from spatial parents for native layout and stylesheet
processing. Source addresses still map to the same selected source nodes. The preview
does not install buttons, text-input focus, modal ownership or serving observation IDs;
see [authored UI](authored-ui.md#live-controls-and-passive-previews).

Selecting a UI node through Hierarchy shows a noninteractive border using native layout,
affine transform and clipping on the same canvas. It tracks layout changes without
rewriting source styles. Viewport clicking remains scoped opaque-mesh selection, not a
UI hit-testing or control-activation feature. The current CPU triangle picker cannot
resolve alpha texture holes; nonopaque surfaces are excluded from this selection path.

## Limits and failure states

Scene View rejects unsupported component schemas and required external ports rather
than borrowing content from the serving editor. A valid source can remain navigable
through Hierarchy/Inspector when its presentation is unsupported.

Each view allows at most 256 generated primitive meshes. Models share an aggregate
4096-node presentation allowance with the source graph. Spatial render targets cap their
longest dimension at 2048 pixels. Visible targets and retained readbacks share a
16,777,216-pixel color-target allowance (64 MiB for the counted RGBA targets), not a
promise about total GPU memory use.

Dependency waiting has explicit timeouts; pipeline errors and renderer readback timeout
are reported without changing sources. A successful current-target readback establishes
a rendered-target milestone, not that every intended object is visible or correctly lit,
nor that OS presentation, physical input, DPI or IME behavior has passed acceptance.
Those properties still require separate native verification.
