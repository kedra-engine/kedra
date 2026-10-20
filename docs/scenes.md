# Authored scenes and source editing

`kedra_scene` and `kedra_authoring` provide the shared authored runtime and guarded
source-editing APIs. Splash, startup forms, repeated rows, frames, captions and dock
chrome use external sources and CSS. A filesystem authoring writer and editable
scene feature are not implemented. [ADR 0005](decisions/0005-authored-scenes.md)
defines the ownership contract; platform acceptance is separate.

## Responsibilities

| Package | Layer | Responsibility |
| --- | --- | --- |
| `kedra_scene` | runtime | Documents, composition, policy, compilation, instances, native asset loading |
| `kedra_ui` | runtime | Native UI projection and behavior over scene instances |
| `kedra_authoring` | authoring | Mutable sources, revisions/history, overrides, Unpack, save protocol |
| `kedra_import` | import | Metadata initialization, processing, installed delivery verification |

None adds another ECS, reflection registry, layout solver or runtime filesystem
catalog scanner. Runtime packages do not depend on authoring, import or editor.
There are no optional authoring features in the runtime crate. Public APIs live
in their owning modules, without crate-root compatibility re-exports.

## Runtime lifecycle

1. Install Bevy AssetPlugin and register the application's native asset source.
   For an installed delivery, register `kedra_assets::register_source` before
   AssetPlugin, using the verified manifest and an explicit namespace.
2. Install `kedra_scene::assets::PrefabPlugin` with an explicit catalog URI.
   `SceneAssetPlugin` alone registers loaders without selecting a catalog.
3. Register the application's side-effect-free components through
   `kedra_scene::reflection::ComponentPolicies`, with canonical schema keys,
   exact versions and validators. Native Bevy TypeRegistry owns reflection.
   Components with lifecycle hooks or unregistered required components fail.
4. Read `kedra_scene::assets::PrefabSources`: pending source loading, published
   revision, native dependency readiness and explicit errors are distinct.
   A failed candidate leaves the published Arc unchanged, but does not become
   a successful unpinned readiness result.
5. Select a compiled document from a ready snapshot and instantiate its Arc
   with an explicit optional parent and named external port bindings. All
   persistent addresses are remapped to new native generational entity IDs.
   Invalid/stale parents/ports and dependency failures allocate no live graph;
   a failed DynamicWorld write reclaims owned allocations.
6. The root's `kedra_scene::instance::PrefabInstance` owns its exact immutable
   compilation and node mapping. Despawning its hierarchy leaves external port
   targets and other instances intact. A pinned source session is explicit;
   publishing a new revision never mutates an existing instance.

`PrefabReady` means a complete live graph exists, not that a native window or
render frame exists. `UiReady` is a later hydration milestone and is likewise
not frame evidence. Callers own retirement/cancellation of pending views.

## Source format and delivery

There is one version-1 RON document format, `document::PrefabDocument`, with
`DocumentKind::Prefab` or `DocumentKind::Scene`. Only prefabs can be nested.
Variants retain their base root identity. Stable document, node and nesting
UUIDs are not runtime entity handles. No legacy readers or implicit migrations
are installed.

Canonical filename extensions select native loaders:

- `.prefab.ron` and `.scene.ron`: `kedra_scene::assets::DocumentLoader`;
- `.prefabs.ron`: `kedra_scene::assets::CatalogLoader`.

The catalog is an explicit RON list of native asset references, preferably
`["game://prefabs/actor.prefab.ron"]`. UUID references such as
`game://<document-uuid>/asset.prefab.ron` are also supported. It is not a directory
listing. Source paths own authoring placement; document UUIDs retain identity.
Both address forms use the same manifest-verified delivery reader. Paths must
be updated after renames; UUID references survive moving source and sidecar
together. The UUID-address shape is reserved to prevent path/identity ambiguity.
Metadata initialization uses the document UUID rather than inventing a second
identity. Import rejects a mismatch with an existing sidecar. Build requires an
explicit `--namespace`, matching the source used by the installed application.

Loader input is bounded to 64 KiB, catalogs to 128 documents, composition depth
to 64 and each graph to 4096 nodes. A complete compiled catalog is bounded to
65536 effective nodes. Nil/duplicate identities, cycles, unknown fields/schema
keys/versions, duplicate source map keys, dangling references and invalid field
patches fail explicitly. RON owns parsing/typed values and serialization.
Each composition request also has a 65,536-item aggregate source/operation/graph
work limit. Request-local caching avoids repeated dependency traversal; instance
overrides operate on independent clones and cached depth cannot bypass the depth
limit. The work allowance is not a wall-clock deadline or exact memory quota.
Hierarchy parents must directly identify live nodes. Symbolic alias remaps remain
supported for component and port references, not as hierarchy parent identities.
All declared nested port bindings are validated even when no component uses them.

## Shared environment and spatial rendering

See [shared environments](decisions/0018-scene-environment-and-preview-stages.md)
for the rendering direction. The implemented first profile provides native
`kedra_scene::environment::{Environment, EnvironmentLoader, SceneEnvironment}`.
`SceneAssetPlugin` registers the bounded `.environment.ron` loader. A scene-root
version-1 `scene.environment` component references it as
`(asset: Path("game://day.environment.ron"))`; prefabs/descendants cannot set it.

```ron
(
    version: 1,
    background: Srgba((red: 0.1, green: 0.15, blue: 0.2, alpha: 1.0)),
    ambient_color: Srgba((red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0)),
    ambient_brightness: 80.0,
)
```

Use existing asset init/build/verify commands; the importer selects the native
loader and runtime can load the verified delivery without source access. Unknown
fields, versions, nonfinite/out-of-range colors/brightness and oversized input
are errors. Version 1 deliberately has no skybox, IBL or camera settings.

The opt-in `kedra_scene` feature `render` exposes explicit native hydration in
`rendering`: primitive meshes, standard materials, supported suns, shared no-LUT
tonemapping and camera-local environment application. It enables existing Bevy
PBR/core-pipeline features, with no new rendering library; data-only consumers do
not enable it. The editor is a consumer, not the owner. Calls affect only their
specified camera/assets; they install no global gameplay or synchronization loop.
Unassigned environment means black/zero ambient, not a hidden studio fallback.
Shadows and skybox/IBL remain unsupported by this initial profile. Native cameras,
lights and custom material/render plugins remain available to games directly.

Environment source editing, in-editor assignment, asset Save and make-unique
commands are not implemented. External edits need an explicit new delivery;
runtime camera changes are never a Save source.

## Shared UI

Install `kedra_ui::authored::AuthoredUiPlugin` alongside Bevy's UI, text,
input-focus, picking and widget plugins. Sources use side-effect-free
`authored::components` data. `UiLayout` contains the actual native Bevy Node
schema, including native default-field semantics, not a proprietary UI language.
Unknown native layout fields and nonfinite values are rejected.

`authored::view::UiView::mount` requires a compiled native-layout root, explicit
stylesheet/image dependencies, unique local semantic IDs and a supported density.
Authored text requires a stylesheet scope; CSS supplies the delivered font faces.
There is no manual-layout substitute or lower-density error path.
Materialization installs native widget behavior only in the live world. Local
element lookup fails for missing names; an explicit observation scope namespaces
IDs for repeated independently owned instances. Features bind typed actions and
domain data to the retained native components; serialized data cannot execute
code or arbitrary remote operations.

Styling uses Bevy Flair through shared `kedra_ui::styles` plugins. `UiStylesheet`
and `UiClasses` are inert source data; hooked Flair components exist only in the
live view. CSS owns geometry, type, borders and visual states; feature bindings
own text/input values, selected/disabled state and dynamic classes. Games use
their own stylesheets and theme boundaries. See [ADR 0006](decisions/0006-native-css-styling.md).

Shared UI projection has headless tests and macOS native consumer checks. This is
not platform-wide input/frame certification. Failed authored views remain explicit
failures; there is no alternative manual editor layout.

## Authoring and Save

`kedra_authoring::workspace::WorkingSet` owns working documents and monotonic
revisions. Every edit supplies revision preconditions and a candidate validator;
the host should compile the entire candidate with `assets::compile_catalog`.
Failed edits do not replace sources or consume history. Apply/Revert and complete
scoped Unpack operate on source values/provenance and remap identities explicitly.
Undo/redo preserve unrelated instance links and provide selection mappings.
Apply and Unpack require a compilation matching the exact current transitive
source values, including shadowed fields and metadata. Unpack retains symbolic
optional-port intent and effective child ordering, not just lowered runtime values.

`kedra_authoring::persistence::Workspace` retains the exact host-supplied source
baseline, asset namespace and diagnostics. Invalid source bytes stay available
for repair; they are never replaced with a runnable default. Save prepares one
immutable `CommitRequest` with source/asset read preconditions and exact coupled
write bytes. A host writer, not the runtime, owns filesystem compare-and-swap,
durability and actual publication. No filesystem writer is currently installed.
The exact proposed document and catalog bytes must satisfy the native readers'
bounds before a request is admitted. Reconciliation and Save reject identity/path
collisions without choosing which source to overwrite.
Document paths must retain their exact native asset-path meaning; label/source
delimiters such as `#` are rejected, not percent-encoded into another filename.

Lost responses must retry/reconcile the same operation and bytes. A conclusively
refused operation can be released with `refused`. Only an exact matching receipt
advances the source baseline; a receipt never marks a newer local edit saved.
The saved catalog prefers namespace-qualified paths, while admission accepts
either declared path or UUID references and rejects duplicate references to the
same document. Gameplay World state is never a Save input.

## Verification

```sh
cargo test -p kedra_scene --locked
cargo test -p kedra_authoring --locked
cargo test -p kedra_ui --test authored --locked
cargo test -p kedra_import --test scenes --locked
cargo xtask check --non-interactive
```

The scene delivery fixture removes source access before loading and instantiating
an independent game-style component. Negative fixtures reject missing catalog
members and UUID mismatches without publishing output or changing sidecars.
Native editor flows, physical DPI/IME behavior and Figma comparisons are separate
acceptance checks. Dependency policies and
[third-party license obligations](third-party.md) remain independent requirements,
not implied by passing tests. Owner-authored prototype provenance is recorded in
[the source provenance statement](third-party.md#source-provenance-and-publication).
