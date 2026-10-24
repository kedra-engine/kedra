# ADR 0018: Shared environment assets and authored model-preview stages

Status: direction agreed with the owner; an initial bounded environment asset,
shared native hydration and authored editor lighting stage are implemented.
Version 1 supports solid background and ambient illumination only. Skybox/IBL,
environment editing, source Save and make-unique commands remain unimplemented.
Native rendering parity and platform isolation acceptance are separate gates.

## Context

Scene View consumes shared native spatial/environment hydration, disables shadows
and selects Reinhard tonemapping. Unassigned Scene lighting has zero camera-local
ambient illumination; an assigned environment supplies its authored values.
Direct model inspection loads and compiles
`editor://scenes/model-preview.scene.ron`, whose environment reference and light
nodes own the inspection lighting. Supported rendering and remaining limitations
are documented in the
[current profile](../site/en/contributing/contracts/editor/scene-view.md).

A project needs reusable environment settings, editable scene assignments and a
predictable model-preview stage without losing ordinary Bevy components, systems,
materials or render plugins. Opening data must not execute project code.

## Decision and responsibilities

Use three responsibilities, not one asset containing a second scene graph or an
entire renderer configuration:

| Owner | Authored data |
| --- | --- |
| Environment asset | Background, ambient illumination, image-based lighting and reflection inputs, their intensity/orientation |
| Scene and prefabs | Ordinary light entities, their transforms, hierarchy and supported light/shadow properties |
| Camera | Exposure, tonemapping, projection and camera-specific rendering settings |

An environment is a versioned, reusable native Bevy asset, referenced through
ordinary typed handles and the existing verified delivery mechanism. Its image
dependencies are explicit. Background and image-based illumination are distinct:
changing visible sky brightness must not implicitly change reflection intensity.
The implementation must document color spaces, physical units, map preparation
requirements and supported image layouts. It must not invent an automatic HDR
prefiltering pipeline as a side effect of Open.

The concrete supported fields are incremental. An unsupported setting produces an
explicit diagnostic; accepting a reference is not proof that its visual behavior
is implemented. Environment assets contain data, not executable plugin names.

Ordinary lights remain scene entities, not lists embedded in the environment
asset. Existing prefab composition can supply reusable light rigs when needed;
this decision requires no second light-rig asset format. Camera settings are not
smuggled into the environment asset. Renderer/backend capability and quality
settings remain explicit host inputs, not a claim that a lighting asset can
override every Bevy renderer setting.

## Assignment and shared materialization

An authored scene has one optional environment reference at its scene root.
Multiple conflicting environment declarations in a composed scene are errors,
not last-writer-wins behavior. Nested prefabs cannot silently replace the host
scene's environment. A standalone prefab is inspected in a preview stage, not
promoted into an authoritative scene environment.

No reference means explicitly unassigned: no skybox, no image-based illumination
and zero ambient illumination from this mechanism. Authored light entities still
work. Do not silently substitute the editor's studio environment. A declared but
missing/invalid reference is a dependency failure, not an unassigned environment.

Shared runtime materialization resolves the environment, lights and supported
camera settings into ordinary Bevy components/assets. Scene View consumes
`kedra_scene::rendering`; independent game-style acceptance must exercise that
same implementation, not a matching editor/game copy.
Use existing packages where responsibilities fit; do not introduce a speculative
renderer crate or generic projection framework.

Runtime depends on neither editor nor authoring/import/filesystem writers. A game
can load the environment through AssetServer, attach it to its scene/view context
and use the shared adapter without running the editor. It can also use native
Bevy lighting directly. The adapter only owns its opted-in bindings and generated
outputs; it must not continuously overwrite unrelated cameras, resources or lights.
Gameplay may explicitly update or retire that binding and drive native components.
These runtime changes do not become source edits.

Scene View retains a navigation camera. Its camera-rendering settings have an
explicit selected source or a documented default also available to runtime hosts;
there must be no hidden editor-only tone curve. Different camera settings are
allowed, but they invalidate a claim of matching the game's camera output.

## Authored preview stage and Editor lighting

Ship an editor-owned, ordinary authored preview scene. It references a delivered
studio environment asset and contains its light entities. Camera-rendering defaults
must also be authored/explicit, not private constants scattered through the viewer.
The current stage is `editor://scenes/model-preview.scene.ron`; it uses the existing
scene format, not a separate stage schema.

Direct model Open instantiates this stage, attaches the model to a view-owned
presentation slot and frames its bounds. The attachment does not edit the stage
or model source. Imported model cameras/lights do not take ownership implicitly;
the existing supported static-import filter remains explicit. The model's asset
identity remains the preview target; the generated stage is not inserted into the
project's authoring workspace, Hierarchy or history as a fabricated document.

Preview stages and their shared environment are ordinary data, so future selection
of another stage does not require a new lighting implementation. Shipping a stage
picker or editing the serving editor's assets is not part of this decision.
Authoring an editor-owned stage requires an isolated own-source workflow; never
grant a project write access to the serving editor's delivery.

For an opened authored scene, distinguish two view modes:

- **Scene lighting:** use the scene's environment and authored lights unchanged.
- **Editor lighting:** temporarily replace the environment and exclude the scene's
  light contribution from this view, using the preview stage's lighting instead.
  Do not add studio lights on top of the scene lights. Do not mount preview-stage
  decorative geometry over the project scene.

Replacement is view-local, clearly indicated and reversible. It changes no source,
dirty state, selection identity or other view. Returning to Scene lighting restores
the latest accepted scene assignment. A stale or failed replacement is labeled and
cannot masquerade as a current scene frame. PNG and passive UI previews are not
implicitly converted into lit 3D stages.

## Source editing and shared assets

Keep three explicit commands and destinations:

1. Assign another environment: edit the scene's reference; dirty the scene only.
2. Edit the referenced environment: open its own source target; dirty that asset,
   not every referencing scene. Make shared usage and edit destination visible.
3. Make unique: create a new environment source/identity and reassign this scene
   through a validated authoring operation. Never modify the shared asset silently.

Do not initially add per-field scene overrides or environment inheritance. Separate
assets plus explicit references avoid a second provenance/override system.

Environment source editing needs an asset source owner with revisions, drafts,
validation, history, conflict handling and source-owned Save. The current
prefab-document workspace is not automatically an arbitrary-asset editor: extend
the authoring contract explicitly, without encoding environments as fake prefabs
or writing files directly from Inspector. Reuse existing edit-session and Save
mechanisms where applicable. Make unique must couple new-source creation and
reference changes with safe failure/recovery semantics before exposing the action.

Accepted unsaved environment revisions can update referencing editor projections;
they do not mutate an immutable delivery or a running game's assets. Show dependency
revision/staleness explicitly. Saved runtime deliveries still require the existing
explicit build/verification path; Open does not build or rewrite metadata.

## Isolation and failure semantics

Each materialized environment/light binding has an explicit scene/view owner,
dependency revision and lifetime. Native camera-local environment components are
preferred. Global ambient resources or broad light queries must not let one open
scene change another scene or the serving shell. RenderLayers alone do not prove
shadow/environment isolation.

Before enabling each feature, verify the pinned Bevy implementation for per-view
ambient light, skybox, image-based lighting, light visibility and shadows. If a
feature cannot be isolated in the shared World, keep it explicitly unsupported and
review a separate native World host; do not claim isolation by convention or build
another ECS. Project code/process execution remains a separate Play decision.

Loading and replacement distinguish source-valid, dependencies-ready and actual
frame-ready states. Reject invalid/nonfinite values, incompatible images, stale
revisions and retired targets. Cancellation/cleanup removes only owned lights,
bindings and targets; shared asset handles remain valid for other consumers.

## Delivery and acceptance

Implement and inspect these slices in order:

1. A bounded, versioned environment asset and explicit scene reference, delivered
   through existing native loaders/import tools. Start with a stated supported
   field set and negative dependency/validation tests; no editor-only loader.
2. Shared runtime materialization with a concrete editor consumer and an independent
   game-style host. Verify equal settings without requiring project execution.
3. Authored editor preview stage for raw models; Scene/Editor lighting replacement,
   retained docking behavior and native multi-view isolation checks.
4. Environment assignment and asset editing through validated authoring targets,
   then source-owned Save/reopen and make-unique support. Editing requires Inspector
   and writer prerequisites; it is not part of the earlier read-only acceptance.

Compare actual renderer captures using identical source/dependency revisions,
camera pose/projection/exposure/tonemapping, target resolution, render capabilities
and simulation state. Cover two scenes with different environments, two views of
one scene in different lighting modes, reflective materials, a dark scene with no
assigned environment, direct model Open, missing assets, replacement failure,
tab/Float/Redock transitions and retirement. Prove unchanged source bytes on Open.

Prove that changing an environment updates its referencing projections, assigning
another environment does not edit the previous asset, and make-unique/undo/Save
preserve shared consumers and reject conflicts without partial publication.
Standalone fixture loading must work without editor or authoring-source access.

Matching supported static rendering is the acceptance goal, not bitwise identity
across GPUs or automatic support for arbitrary project shaders, animation or
gameplay-generated visuals. Unsupported shadows/settings remain visible limitations
until independently implemented and verified. This ADR does not select Play hosting.

## Alternatives rejected

- One lighting asset containing entities, cameras and global renderer configuration:
  duplicates scene composition and conflates camera, host and environment ownership.
- Separate editor/game lighting adapters: visual drift remains possible even when
  serialized settings have identical field names.
- Hardcoded inspection lights or implicit fallback for broken scene dependencies:
  hides source errors and prevents ordinary authoring of the preview stage.
- Additive Editor lighting: results depend on the underlying scene light and do
  not provide a predictable inspection setup.
- Mandatory Kedra wrappers around all Bevy rendering: restricts games without being
  necessary for this shared, opt-in data/materialization contract.
