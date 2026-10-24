# Rust components and host integration

Use ordinary Bevy code for gameplay. To expose a type in source documents, Inspector or
scripts, install each integration explicitly in the Rust host. Kedra does not discover
a class and automatically turn it into a component, inspector, source schema and script API.

## Runtime types are not authored schemas

A Bevy `Component` can be used by native systems without being stored in Kedra sources.
Reflection registration alone is also insufficient for authoring: `ComponentPolicies`
adds an explicit source key, exact schema version and validator to Bevy's type registry.
This is an allowlist over Bevy reflection, not a second reflection system.

For the serialized side of this registration, see
[document fields and component maps](../scenes/index.md#document-fields).

The following **excerpt**, adapted from the independent runtime contract's `Tuning`
component and `setup`, shows the registration boundary. `app` is the host's initialized
Bevy application; `policies` must be supplied to source compilation or the host's service.

```rust
use bevy::prelude::*;
use kedra_scene::reflection::ComponentPolicies;

#[derive(Component, Clone, Reflect, Debug, PartialEq)]
#[reflect(Component)]
struct Tuning {
    speed: f32,
    health: u32,
}

// Excerpt from a fallible host setup function.
app.init_resource::<AppTypeRegistry>();
let registry = app.world().resource::<AppTypeRegistry>().clone();
let mut policies = ComponentPolicies::default();
policies.register::<Tuning>(&mut registry.write(), "game.tuning", 1, |value| {
    let tuning = Tuning::from_reflect(value)
        .ok_or_else(|| kedra_scene::Error("invalid tuning".into()))?;
    if !tuning.speed.is_finite() || tuning.speed < 0. {
        return Err(kedra_scene::Error(
            "speed must be finite and nonnegative".into(),
        ));
    }
    Ok(())
})?;
```

The matching source payload is `ComponentData::ron(1, "(speed:1.0,health:10)")` under
the key `game.tuning`. This is typed component data, not a script or a request to run code.
The runtime contract changes a variant's `speed` to `3.0`, instantiates it twice and
checks that changing one live instance's `health` does not change its sibling.
Live changes also leave the immutable compiled source snapshot intact.

### Admission rules

- Unknown keys and mismatched versions fail; the runtime does not skip unknown data
  or install compatibility readers. A schema mismatch requires migration.
- Authorable types must reflect `Component` and support reflected construction.
- Insertion/lifecycle hooks are rejected. Required components need their own
  side-effect-free policy, except for the explicitly allowed engine presentation types.
- Validators must be side-effect-free. Gameplay activation belongs after `PrefabReady`,
  not in a hook that could execute during isolated source validation.
- Persisted `Entity` fields need Bevy's `#[entities]` mapping. Compilation verifies that
  mapping visits every serialized reference; a plain reflected entity field is not enough.

Keep source references symbolic: local scoped node addresses and external ports become
runtime entities during materialization. Keep asset references as native asset paths.
Persisted document, node and instance identities remain UUIDs; runtime `Entity` handles
are neither replacements for them nor suitable saved identities.

Registering this policy does not promise visual editing, a custom Inspector presentation,
rendering support or a Luau binding. The current editor Inspector is read-only and has
its own supported presentations. These integrations must be evaluated separately.

## Source compilation and live systems

`CompiledPrefab::compile` works in an isolated World. It checks composition, component
versions, field overrides, validators, symbolic references and native dependencies before
a live instance is created. Its compiled source snapshot is immutable.
Source publication creates a new revision; it never silently patches existing instances.

`PrefabInstance` retains the exact compilation and a mapping from source addresses to
its owned live entities. Two instances share source meaning, not runtime entity identity.
`PrefabReady` means instantiation succeeded completely, not that assets have reached a
GPU-ready frame. Host-selected hydration may impose additional readiness requirements.

Native systems operate on those live Bevy components normally. Put initialization that
requires a complete prefab after its readiness boundary. For scripts, order domain
systems around `ScriptSystems::Update`, `ScriptSystems::FixedUpdate` and
`ScriptSystems::Run` rather than assuming an order among script instances.

Live mutations are gameplay effects. They do not create source overrides, dirty a document
or implement Save. To persist an authored change, use validated source transactions and
the [source-owned Save protocol](../scenes/source-editing.md).

## Use qualified paths at native call sites

Native host references to scenes/prefabs use full logical paths, for example the fixture's
`game://Native/switch.scene.ron` and `application://catalogs/native.prefabs.ron`.
These are not absolute filesystem locators. Resolve through the loaded source catalog,
check membership and use the published revision; do not create a second UUID lookup table
or hardcode a document UUID/numeric ID in gameplay code.

This rule does not rewrite serialized references: document/node/instance UUIDs and source
provenance remain intact. See [scene lifecycle](../scenes/lifecycle.md) for the startup
path-to-catalog contract and explicit mount ownership.

## Diagnose a component that will not load

First check its authored key and version, then the host's policy registration, reflected
construction and required-component policies. Check the validator after applying overrides:
a valid base does not make a negative or nonfinite overridden speed valid. For entity
links, check port bindings, node scope and `#[entities]`, not numeric entity equality.

A rejected compilation must not mutate a live instance. Scene contract tests cover invalid
data/reference paths and independent instance ownership; they do not test native rendering.
Continue with [Luau](luau.md) only when the host also intends
to authorize interpreted execution.

## Repository references

Use the [Rust API guide](../../api/index.md) to generate exact workspace API docs;
the source and contract links below provide the component example's full context.

See the [component-policy implementation][policies] and
[runtime component contracts][contracts] for the exact registration API and `Tuning` context.

[policies]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/reflection.rs
[contracts]:
  <https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/tests/contracts/runtime.rs>
