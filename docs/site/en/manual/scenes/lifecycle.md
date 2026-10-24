# Loading and lifecycle

Loading a scene into your Bevy application requires explicit source registration,
component policies and a mount step. Use the [Rust API guide](../../api/index.md)
to look up the exact signatures.

Contributors changing publication, allocation or startup should also read the
[scene runtime contract](../../contributing/contracts/scene-runtime.md).

Loading source data, instantiating entities and showing a frame are separate milestones.
A host application chooses each capability explicitly; a document cannot install plugins,
create a window or authorize scripts simply by being loaded.

Prerequisites:
[source, composed result and runtime instance](index.md)
and a [prepared asset delivery](../assets/pipeline.md).

## Prepare the host

1. Register the native asset sources. For a verified delivery,
   `kedra_assets::register_source` must run before Bevy's `AssetPlugin`.
2. Install `AssetPlugin` and the application's component policies. Policies use canonical
   schema keys, exact versions and validators over Bevy reflection. Source components
   must be side-effect-free; lifecycle hooks and unsupported required components fail.
3. Install `kedra_scene::assets::PrefabPlugin` with an explicit `catalog_path`.
   `SceneAssetPlugin` alone registers asset types/loaders without selecting a catalog.
4. Select an entry scene or wait for `PrefabSources` and explicitly instantiate a member.

A `.prefabs.ron` catalog is a RON list of asset references, not a directory scan.
Include the documents needed by composition and document links. The native catalog
accepts 1–128 entries; compilation also limits total effective nodes to 65,536.
Resource delivery preparation is separate from selecting and mounting the scene.

## Select an explicit startup scene

After installing the catalog and policies, the host can add
`StartupScenePlugin::new("game://Scenes/Main.scene.ron")`. This API expression is an
integration fragment, not a complete application: that path must exist in the host's
installed native source catalog.

The startup plugin loads the path through `AssetServer`, verifies catalog membership,
requires `DocumentKind::Scene`, and waits until the loaded document matches the
published source revision. It then mounts once. It never selects the first available
scene or substitutes a prefab when the requested entry is missing.

Read `kedra_scene::startup::StartupScene::state()` to distinguish:

- `Loading`: the catalog, source revision or dependencies are not yet ready.
- `Mounted { document, root }`: the complete source instance exists.
- `Failed(error)`: a loading or mount failure; the host decides how to report or exit.
- `Retired`: the mounted root was removed; there is no automatic remount or fallback.

`kedra_app::launch::LaunchOptions::configure` uses the host's normal scene in every
build. A `--scene` override is debug-only; release still supports the configured scene.
Changing an entry path chooses composition, not a different set of native capabilities.

## Instantiate a prefab explicitly

For repeated or dynamically mounted content, use the ready catalog rather than a second
UUID lookup registry. Resolve a full logical path such as `game://Cuboid.prefab.ron`
through `AssetServer` and the loaded `PrefabCatalogAsset`. Verify its native handle is
a catalog member and that its materialized document matches the selected `SourceRevision`.
Take the corresponding compiled entry from that same snapshot, not a newer document
paired with an older compilation. Startup's resolver implements this matching pattern.

`CompiledPrefab::instantiate` takes the target World, an optional live parent and named
[external port bindings](composition.md#ports-connect-to-content-outside-a-prefab).
It checks asset readiness, parent lifetime and binding presence
before allocating the graph. An invalid/stale target allocates no live graph; a failed
Bevy dynamic write reclaims its owned allocations without deleting external targets.

On success, the root carries `PrefabInstance`: the materialization identity, source
document, complete source-address-to-entity map and an `Arc` to the exact compilation.
Each node carries `PrefabNode` with its owner and scoped address. Despawning the root's
hierarchy removes its owned nodes, not another instance, external parent or port target.
Keep the root as the lifetime handle; do not persist its Bevy entity ID in a source file.

## Publication is not hot replacement

`PrefabSources` separates pending loading, published revision, dependency readiness and
errors. `ready()` is the gate, not merely the existence of a snapshot. A failed candidate
retains the previous published `Arc`, but exposes an error instead of reporting ordinary
unpinned readiness.

A host can explicitly `pin()` one ready revision and later `unpin()` it. Pinned sessions
and existing instances retain their compilations across new publication. This mechanism
does not implement editor Play, live entity patching or automatic scene restart. The host
is responsible for cancelling pending work and retiring views it no longer needs.

## Hydration, scripts and visible readiness

For host setup and authorization, continue with [Programming](../programming/index.md).
For the presentation adapters, see [Graphics and UI](../rendering-ui/index.md) and
[authored UI](../rendering-ui/authored-ui.md).

The default startup mount performs ordinary source instantiation. UI hosts can choose
`kedra_ui::authored::view::UiView::mount_scene`; they still install native UI behavior,
windows and cameras. Passive editor projections strip serving UI identities rather than
impersonating the application's live controls.

The `kedra_scene` `render` feature exposes native spatial-hydration helpers for the host
to call explicitly. Data-only loading does not require it; enabling it neither hydrates
entities automatically nor installs a gameplay synchronization loop.
Hosts separately authorize Luau programs and their bindings. Selecting a scene does not
discover or execute project scripts. A trusted host mount can bind an authorized program;
that is host code, not execution permission carried by the document.

`PrefabReady` means a complete entity graph exists. `UiReady` is a later UI hydration
milestone; `PrefabRenderReady` marks CPU render-asset materialization. None proves GPU
readiness, a rendered frame or physical input. Editor previews and headless lifecycle
tests must not be treated as native gameplay or interaction acceptance.
