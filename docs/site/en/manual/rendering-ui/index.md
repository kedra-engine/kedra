# Graphics and UI

For exact native interfaces, use the [Rust API guide](../../api/index.md) alongside
the rendering and UI contracts linked in this chapter.

Kedra projects use ordinary Bevy rendering and retained UI. Kedra adds a small authored
data profile and shared adapters; it does not replace Bevy's renderer or promise that
every Bevy feature can be saved, inspected or previewed in the editor.

Choose the source data and presentation adapters your application needs. An editor
preview may differ from the application's camera even when both use the same source.

- [Environments and lighting](environments-and-lighting.md): reusable background and
  ambient data, scene lights and camera ownership.
- [Preview stages](preview-stages.md): model inspection, lighting replacement and
  spatial/UI canvases.
- [Authored UI](authored-ui.md): source components, native widgets and passive views.
- [Styling and layout](styling-and-layout.md): CSS, native layout and window integration.

For opening documents and camera gestures, use [Scene View](../editor/scene-view.md).
For source structure and composition, start with [Scenes and prefabs](../scenes/index.md).
For host policy registration, see [Rust components](../programming/rust-components.md);
for delivered dependencies, check [supported asset formats](../assets/supported-formats.md).

## The supported spatial profile

Spatial source components are inert data. Their version-1 schema keys are:

| Key | Describes | Current shared interpretation |
| --- | --- | --- |
| `scene.transform` | Translation, unit quaternion and scale | Native Bevy `Transform` |
| `scene.primitive` | Cuboid full extents or sphere radius | Generated native mesh |
| `scene.surface` | Color, optional base-color image, metallic and roughness | Opaque `StandardMaterial` |
| `scene.sun` | Color and illuminance | Directional light along the node's local −Z axis; no shadows |
| `scene.model` | Native scene subasset handle | Static model presentation in Scene View |
| `scene.environment` | Environment asset handle | Scene-root background/ambient assignment |

`scene.primitive` and `scene.surface` must occur together in Scene View. A sphere has
fixed 32 × 16 UV tessellation in the shared mesh helper; there is no authored subdivision
control. Cuboid extents and sphere radius must be finite, greater than zero and at most
10,000 local scene units. Units are not automatically converted to metres.

Transforms require finite values: translation magnitude per axis is at most 1,000,000,
absolute scale per axis at most 10,000, and quaternion squared length must be within
0.001 of one. Surface color channels, metallic and roughness are bounded to 0–1.
These are compilation constraints, not transform-gizmo controls.

### Example: a textured primitive

The following is the `components` map excerpt from
`tests/fixtures/test_project/assets/Cuboid.prefab.ron`, reformatted without changing
values. It is not a complete prefab document:

```ron
"scene.primitive": (
    version: 1,
    value: Cuboid(size:(2.0,2.0,2.0)),
),
"scene.surface": (
    version: 1,
    value: (
        color:Srgba((red:0.25,green:0.55,blue:0.9,alpha:1.0)),
        texture:Some(Path("game://checker.png")),
        metallic:0.1,
        roughness:0.5,
    ),
),
"scene.transform": (
    version: 1,
    value: (
        translation:(0.0,1.0,0.0),
        rotation:(0.0,0.0,0.0,1.0),
        scale:(1.0,1.0,1.0),
    ),
),
```

The image is a delivered dependency, not a filename that the renderer searches for on
disk. Prepare a matching [asset delivery](../assets/pipeline.md) before previewing it.
Use a private copy of the fixture for experiments; do not modify the checked test assets.

## Integrating a renderer

A host registers `kedra_scene::spatial::register` with its component policies and Bevy
type registry, installs its native rendering plugins, then chooses a compiled instance.
The `kedra_scene` `render` feature exposes `rendering::mesh`, `material`, `sun`,
`environment` and `tonemapping`. The host calls these helpers explicitly, retains generated
mesh/material handles and retires its own outputs.

This feature enables existing Bevy PBR/core-pipeline support, not a new rendering library;
data-only consumers leave it disabled. The editor consumes these shared helpers;
it does not own them.

Enabling the feature does **not** install a projection plugin, discover scenes, hydrate
entities automatically or synchronize source data every frame. `scene.model` likewise
does not automatically spawn an imported scene; Scene View supplies its own bounded
static-model adapter. See [loading and lifecycle](../scenes/lifecycle.md) for catalog
selection and instance ownership.

Native scene/prefab lookups use full logical paths, for example
`game://Cuboid.prefab.ron`, through the loaded source catalog. Verify membership and the
published source revision before taking its compiled entry. Persisted UUID identity is
not an invitation to hardcode UUIDs or numeric document constants in native code.

An application may use additional native Bevy components or render plugins. That does
not make those components supported by source compilation or Scene View. The editor
rejects unknown presentation schemas instead of partially rendering a misleading scene.
The current profile does not imply complete camera, material, lighting or animation
coverage; arbitrary shaders, shadows and skybox/IBL are not supported preview features.

## Readiness is not visual acceptance

Source compilation, dependency readiness and CPU materialization are separate from a
GPU frame. `PrefabReady`, `UiReady` and `PrefabRenderReady` describe earlier milestones,
not proof of correct visible output or physical input. Scene View separately waits for
renderer pipelines and a current-target readback.

Headless tests do not check rendered frames or platform-specific interaction.
Full application/editor frame parity and platform behavior remain unverified.
Opening a preview neither executes project scripts nor enables Play, source-owned Save
or visual asset editing.
