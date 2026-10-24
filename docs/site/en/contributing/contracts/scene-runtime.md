# Scene runtime contracts

`kedra_scene` validates and publishes scenes and manages native instance lifetime.
For learning the format and host setup, start with
[Scenes and prefabs](../../manual/scenes/index.md)
and [Loading and lifecycle](../../manual/scenes/lifecycle.md).

## Ownership and integration order

`kedra_scene` owns documents, composition, component policy, compilation, instances and native
loaders. `kedra_ui` owns native UI hydration/behavior; `kedra_authoring` owns mutable sources and
Save protocol; `kedra_import` owns metadata initialization, processing and delivery verification.
Runtime depends on none of the editor, importer or authoring writer. There are no optional
authoring features in `kedra_scene`. Public interfaces live in their owning modules, not
crate-root compatibility re-exports.

Use ordinary Bevy ECS, reflection, asset loading and UI layout. These packages introduce no
parallel ECS, reflection registry, layout solver or runtime filesystem catalog scanner.
[ADR 0005 (English)][scenes-adr] defines scene/source ownership;
[ADR 0006 (English)][css-adr] defines native CSS ownership.

Register an installed delivery with `kedra_assets::register_source` and its explicit namespace
before Bevy's `AssetPlugin`. Install component policies and `PrefabPlugin` with an explicit
catalog URI before requesting startup. `SceneAssetPlugin` only registers asset types and
loaders: it does not choose a catalog, scene, renderer or window.

Native call sites select full source-qualified logical paths, for example
`game://Scenes/Main.scene.ron`, through the existing native catalog. Check handle membership
and equality of the materialized document with the selected published revision before taking
its compilation. Do not introduce a parallel lookup table or hardcoded document UUIDs.
Persisted document/node/site identities and provenance remain UUIDs; the delivery reader's
[UUID addressing](../../manual/scenes/index.md#native-source-addressing) is not a native
application-selection convention.

## Admission and compilation

See [source format and limits](../../manual/scenes/index.md#source-validation) for validation rules.
RON handles parsing, typed values and serialization; there is no implicit legacy
reader or migration. Composition must retain exact transitive source values, provenance and
symbolic references, not merely the final visible component values.

`ComponentPolicies` is a schema allowlist over Bevy's `TypeRegistry`: canonical keys, exact
versions and side-effect-free validators are required. Authorable types must reflect
`Component` and support reflected construction. Lifecycle/insertion hooks fail registration.
Required components need their own side-effect-free policy except the explicitly admitted
native transform/visibility presentation types. Duplicate keys/types fail. Persisted entity
fields must participate in Bevy entity mapping; compilation checks every serialized reference.
See [Rust admission rules](../../manual/programming/rust-components.md#admission-rules).

`compile_catalog` validates the complete candidate, all component fields/versions, nested
bindings and document links before publication. Invalid fields, dangling references and missing
document-link members reject the whole candidate without mutating live instances. All nested
port bindings are validated even when no effective component uses them. Hierarchy parents must
directly name live composed nodes; reference aliases are valid for component/port targets,
not hierarchy parents. Request-local caching must preserve the depth/work limits and isolate
each site's override clone.

## Publication and pinned ownership

`PrefabSources` distinguishes pending source work, successful publication revision, native
dependency readiness and explicit errors. Merely holding `snapshot()` is not an unpinned
readiness check: callers use `ready()`. A rejected candidate leaves the exact published `Arc`
unchanged, records failure and does not report successful ordinary unpinned readiness.

A successful publication is a complete immutable `SourceRevision`, not a file timestamp or
incremental live patch. `pin()` requires readiness and rejects a second pin until `unpin()`.
The explicit pinned session retains its selected revision across publication; unpinning does
not invalidate existing instances. Native dependency failures remain separate errors and
instantiation checks dependencies again. Startup also checks `PrefabSources::error` explicitly;
a pin is not permission for startup to ignore a publication error.

## Allocation and retirement

`CompiledPrefab::instantiate` requires an `AppTypeRegistry`, native dependency readiness, an
optional live parent and named external bindings matching the compiled port-presence contract.
Unknown ports, stale targets, missing required bindings or mismatched optional presence fail
before graph allocation. A failed `DynamicWorld` write reclaims owned allocations without
despawning the external parent/port targets.

Successful materialization remaps source addresses to fresh native generational entities.
The root's `PrefabInstance` owns the complete node mapping and exact compilation `Arc`, including
dependencies; each `PrefabNode` records its materialization owner and scoped source address.
Despawning the hierarchy removes owned descendants, not other instances or external targets.
Publication never mutates an existing instance. Hosts own pending-view cancellation and
retirement; deferred ECS mutation is not a filesystem/source transaction.

`PrefabReady` means the complete live graph exists. `UiReady` is later native UI hydration;
`PrefabRenderReady` is CPU render-asset materialization. None proves a native window, GPU
readiness, a frame, physical input or platform certification.

## One-shot startup and host authority

`StartupScenePlugin` requires installed policies and `PrefabPlugin`. It loads the explicit
path, requires membership in that native catalog and `DocumentKind::Scene`, and waits for the
matching published source/dependencies. A newly materialized document must not be paired with
an older compilation. It mounts once, with no first-member fallback. `StartupScene::state()`
reports `Loading`, `Mounted`, `Failed` or `Retired`; removing the root retires startup without
remounting. The host owns diagnostics and exit policy.

The default mount performs ordinary instantiation. A trusted host can choose
`kedra_ui::authored::view::UiView::mount_scene`; windows, cameras and native behavior plugins
remain host-owned. Serving UI preserves `ui.id`/`ui.screen`; passive previews strip them.
See
[live and passive UI](../../manual/rendering-ui/authored-ui.md#live-controls-and-passive-previews)
for hydration ownership.

`LaunchOptions::configure` applies the normal host scene in every build. Only debug-assertion
builds admit `--scene` overrides and explicit remote flags; release retains configured scenes
and game-facing Luau execution. Selecting a scene does not discover or run scripts: hosts
separately register trusted programs and callers explicitly start a named scenario through
the application protocol. This is not editor Play or automatic project execution.
[ADR 0030 (English)][startup-adr] retains startup, catalog and docking rationale.

## Source owners and verification boundaries

Use the [Rust API guide](../../api/index.md#rust-api-guide) for exact module signatures.
Production owners are [assets][assets], [reflection][reflection], [composition][composition],
[instance][instance] and [startup][startup]. Rendering/environment contracts remain in
[Environments and lighting](../../manual/rendering-ui/environments-and-lighting.md) and
[Graphics and UI](../../manual/rendering-ui/index.md); native UI prerequisites remain in
[Authored UI](../../manual/rendering-ui/authored-ui.md) and the [UI crate contract (English)][ui].

Substantive test owners:

- [Scene runtime contracts][runtime-tests]: independent components, mapping, pinned lifetimes,
  rejected-publication `Arc` identity and failed-write allocation cleanup.
- [Instance contracts][instance-tests], [composition safety][composition-tests] and
  [catalog admission][catalog-tests]: unchanged live state, all port bindings and bounded input.
- [Startup contracts][startup-tests]: real asset loading, explicit membership, failure without
  instances and one-shot retirement.
- [Import scene contracts][import-tests]: installed delivery without source access, sidecar
  UUID agreement and missing-member rejection without output publication/sidecar changes.
- [UI hydration contracts][ui-tests]: inert source data, live widget ownership, scope and
  rejection before allocation. Native scenarios have separate input/frame obligations.

These Rust tests exercise compilation. They do not verify the assembled native application,
editor flows, physical DPI/IME behavior or Figma comparisons; those checks remain separate
and unverified here.
See the [testing guide (English)][testing] for execution and evidence ownership.

Focused owner checks (run separately from native acceptance):

```sh
cargo test -p kedra_scene --locked
cargo test -p kedra_ui --test contracts --locked authored::
cargo test -p kedra_import --test contracts --locked scenes::
```

Dependency policy, [third-party licenses and source provenance (English)][legal] remain
independent requirements, never consequences of passing scene tests.
The owner-authored prototype's [source provenance statement (English)][provenance] remains
authoritative independently of this runtime contract.

[scenes-adr]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0005-authored-scenes.md
[css-adr]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0006-native-css-styling.md
[startup-adr]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0030-explicit-startup-scenes.md
[assets]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/assets.rs
[reflection]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/reflection.rs
[composition]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/composition.rs
[instance]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/instance.rs
[startup]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/src/startup.rs
[ui]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_ui/README.md
[runtime-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/tests/contracts/runtime.rs
[instance-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/tests/contracts/instance_contracts.rs
[composition-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/tests/contracts/composition_safety.rs
[catalog-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/tests/contracts/catalog_contract.rs
[startup-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_scene/tests/contracts/startup.rs
[import-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_import/tests/contracts/scenes.rs
[ui-tests]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_ui/tests/contracts/authored/hydration.rs
[testing]: testing.md
[legal]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md
[provenance]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md#source-provenance-and-publication
