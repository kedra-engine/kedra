# Environments and lighting

An environment is a reusable asset for the scene's background and ambient illumination.
It is not a container for all rendering settings. Keep three choices separate:

- The **environment asset** supplies its supported background and ambient values.
- **Scene nodes** supply lights, transforms and hierarchy; a prefab can supply a light rig.
- The **camera and host** supply projection, exposure, tonemapping and renderer settings.

This makes it possible to reuse one environment in several scenes without moving their
lights or changing their cameras. The present format is deliberately smaller than the
longer-term rendering direction: version 1 has no skybox, IBL or reflection-map fields.

## Version-1 environment asset

The complete shipped source `assets/editor/environments/studio.environment.ron` is:

```ron
(
    version: 1,
    background: Srgba((red: 0.14117648, green: 0.16862746, blue: 0.20392157, alpha: 1.0)),
    ambient_color: Srgba((red: 0.78, green: 0.85, blue: 1.0, alpha: 1.0)),
    ambient_brightness: 80.0,
)
```

`EnvironmentLoader` handles `.environment.ron`; `SceneAssetPlugin` registers it for
native asset loading. The [asset pipeline](../assets/pipeline.md) prepares and verifies
its delivery. Merely loading the asset does not change a camera.

`background` and `ambient_color` use Bevy color serialization, not hex strings in a
custom theme language. The example uses sRGB channels. Validation converts colors to
linear space and requires every channel, including alpha, to be finite and in 0–1.
`ambient_brightness` is Bevy's ambient multiplier, finite and in 0–1,000,000. Upstream
Bevy interprets the multiplied ambient color in cd/m²; it is not the sun's lux value.

The source must fit 64 KiB. Unknown fields, an unsupported version, negative brightness
or nonfinite values are errors. Adding `skybox` to a version-1 file does not opt into a
new capability: it makes the source invalid.

## Assign it to a scene

An optional `scene.environment` component belongs only on the root of a `Scene`, not
on a child or a prefab. This prevents a nested prefab from replacing its host's environment.
The preview stage uses this component-map excerpt:

```ron
"scene.environment": (
    version: 1,
    value: (asset: Path("editor://environments/studio.environment.ron")),
),
```

This excerpt is from `assets/editor/scenes/model-preview.scene.ron`; it is not a complete
scene. A game's assignment uses its own admitted source and dependency. Keep the normal
document wrapper and identities described in [Scenes and prefabs](../scenes/index.md).

No assignment means a black background and zero camera-local ambient light when the
shared rendering adapter is applied. Authored sun nodes can still illuminate geometry.
A declared reference that is missing, invalid or not materialized is a dependency failure,
not permission to substitute the studio. A dark scene is therefore not necessarily broken.

## Apply settings to a camera

`kedra_scene::rendering::environment(world, camera, source)` takes one existing camera
and an optional validated `Environment`. It sets that camera's clear color and native
`AmbientLight`; it leaves `GlobalAmbientLight` and other cameras unchanged.

Validation happens before writes. An invalid value or unavailable camera returns an
error without replacing the previous camera settings. The helper is an explicit
apply/refresh operation, not a permanent controller: application systems remain free
to drive the native components afterward. Those runtime changes do not edit the asset.

Scene suns use `scene.sun`: finite normalized color and illuminance from 0 to 1,000,000
lux, oriented along local −Z. The shared adapter produces a native `DirectionalLight`
with shadow maps disabled. Point/spot lights and configurable shadows are not additional
authored schemas in this profile.

`rendering::tonemapping()` returns Reinhard, an explicit no-LUT default used by Scene
View. Hosts can choose other native camera settings, but then equal environment data
does not imply equal images. Compare camera pose, projection, exposure, tone curve,
target resolution, dependencies and renderer capabilities before diagnosing a mismatch.

## Reuse and current editing limits

For a production scene, treat the environment reference and the environment asset as
different edit destinations. Reassigning a scene would change its reference; changing a
shared environment would affect its consumers. Neither operation is an implemented
visual editor workflow today. Environment Inspector editing, Make unique, source-owned
Save and unsaved environment propagation are not available.

Files can be prepared through the existing source and delivery workflow, but Open does
not rebuild deliveries or write sources. Immutable delivered assets are not live drafts.
Skybox/IBL preparation and full application/editor lighting parity remain future work,
not hidden fields in the current schema.

[ADR 0018 (English)][environment-adr] explains the rendering design and division of
responsibilities. The proposed capabilities listed above are not implemented, and the ADR
does not verify native rendering.

For source-side changes, see [Source editing and Save](../scenes/source-editing.md);
for the distinction from live instances, see [Loading and lifecycle](../scenes/lifecycle.md).

For temporary inspection without changing a scene, use
[Scene lighting and Editor lighting](preview-stages.md#scene-lighting-and-editor-lighting).

[environment-adr]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0018-scene-environment-and-preview-stages.md
