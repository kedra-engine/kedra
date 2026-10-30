# Scenes and prefabs

A scene describes a top-level authored hierarchy. A prefab describes a reusable hierarchy
that another document can include. Both store nodes, component values and references;
neither stores a running Bevy World.

Use a scene for an application entry point and a prefab for repeatable content such as
a cuboid, a HUD or a control. Scenes may contain prefab instances. Prefabs may contain
other prefabs, but a scene cannot be nested as a prefab.

## Choose your workflow

If project paths or delivered resources are unfamiliar, first read
[Paths and identity](../projects/paths-and-identity.md) and
[Assets](../assets/index.md#sources-deliveries-and-loaded-resources).

- To inspect the test fixture in the editor, follow
  [your first document](../getting-started/first-document.md).
- To understand inherited content and repeated objects, read
  [Composition and overrides](composition.md).
- To load a document into a Bevy application, read [Loading and lifecycle](lifecycle.md).
- To build source-editing tools, read [Source editing and Save](source-editing.md).

The editor supports scene/prefab creation, eligible Hierarchy edits, Inspector properties
and Scene View transforms through shared source history and explicit Save.
Scene View projects supported content when its resource prerequisites are met.
Play remains unavailable: a preview does not execute the project's gameplay.

For preview prerequisites, see
[matching asset deliveries](../assets/pipeline.md#editor-previews-need-matching-sources).

## Keep the three representations distinct

**Source document.** The version-1 RON file stores the authored data. It contains local
nodes, prefab composition sites and ordered overrides, not every inherited value copied
into one file.

**Composed result.** Composition resolves bases, nested prefabs and overrides into an
effective hierarchy, retaining provenance. Typed compilation validates component values
and references in an isolated source World. The resulting `CompiledPrefab` is immutable.

**Runtime instance.** Instantiation creates ordinary Bevy entities from a compilation.
Every materialization receives fresh entity handles and an owner identity. Changing a
live component does not change its source or another instance. Publishing new sources
does not retrofit existing instances.

This distinction also explains why selecting a node in Hierarchy is not selecting a
persistent gameplay entity, and why saving cannot mean serializing Scene View entities.

## Read a source document

`.scene.ron` and `.prefab.ron` share the `PrefabDocument` schema. `kind`, rather than a
different node format, distinguishes `Scene` from `Prefab`.

This excerpt from the `Cuboid.prefab.ron` test fixture shows the document header.
Other fields and components are omitted; this is not a complete file:

```ron
version: 1,
id: "7e570000-0000-4000-8000-000000000001",
kind: Prefab,
title: "Textured cuboid",
root: "7e570000-0000-4000-8000-00000000000b",
base: None,
```

Its root node has the same node UUID as `root`, `parent: None`, and these component data:

```ron
"scene.primitive": (
    version: 1,
    value: Cuboid(size:(2.0,2.0,2.0)),
),
"scene.transform": (
    version: 1,
    value: (translation:(0.0,1.0,0.0),rotation:(0.0,0.0,0.0,1.0),scale:(1.0,1.0,1.0)),
),
```

The fixture also assigns `scene.surface`, including `Path("game://checker.png")` as its
texture reference. Interpreting and rendering these keys requires the host's supported
component policies and presentation adapter; a RON entry alone installs no renderer.

### Document fields

- `version`: document format version; currently only `1` is accepted.
- `id`, `root`: persistent document and root-node UUIDs.
- `kind`, `title`: document role and human-readable title.
- `base`: optional document of the same kind; variants retain its root identity.
- `nodes`: locally authored nodes, each with `id`, `name`, `parent` and `components`.
- `instances`: direct prefab composition sites with bindings and site overrides.
- `overrides`: ordered operations over the composed hierarchy.
- `ports`: named external references; `true` requires a binding, `false` permits absence.

A node's component map uses registered schema keys, with an exact schema `version` and
typed RON `value` for each entry. This is not an arbitrary map of Rust type names.
Unknown keys, versions or fields are errors, not silently ignored extensions.

### Identities and references

Document, node and nested-instance UUIDs preserve source identity. A node address combines
its node UUID with the outer-to-inner nesting identities. Two copies of the same prefab
therefore share an originating node identity but have different scoped addresses.
Names and titles are labels, not unique lookup keys.

`Reference::Local` targets a scoped source node; `Reference::Port` names an external
binding. Compilation and instantiation remap these references to entities. Asset paths,
such as `game://checker.png`, are a different kind of reference.

The UUIDs above are serialized source identities, not constants to paste into native
application lookup code. Native consumers select full logical paths such as
`game://Cuboid.prefab.ron` through the loaded source catalog and its published revision.

## Native source addressing

Canonical extensions select native loaders: `.prefab.ron` and `.scene.ron` use
`kedra_scene::assets::DocumentLoader`; `.prefabs.ron` uses `CatalogLoader`.
A catalog is an explicit RON list, for example `["game://prefabs/actor.prefab.ron"]`,
not a directory listing. It accepts 1–128 references, at most 4096 bytes per path and
64 KiB for its exact source. The complete compiled catalog permits at most 65,536
effective nodes across all entries.

The installed delivery reader also supports `game://<document-uuid>/asset.prefab.ron`.
Both path and UUID addressing use the same manifest-verified reader. Source paths determine
placement; UUIDs preserve identity. Update path references after renames; UUID references
survive moving the source and its sidecar together. The UUID-address shape is reserved to
avoid ambiguity between paths and identity. Native application code still selects full
logical paths through the catalog, rather than embedding UUID constants.

Metadata initialization uses the scene/prefab document UUID for its sidecar, not a second
identity. Import rejects disagreement with an existing sidecar. Build requires an explicit
`--namespace` matching the application's registered source. See the
[asset pipeline](../assets/pipeline.md) for delivery preparation.

## Source validation

Parsing accepts at most 64 KiB per document. Composition and typed compilation validate
a single ordered, connected hierarchy and reject duplicate/nil identities, missing sources,
cycles, dangling references and invalid operations. Graphs are limited to 4096 effective nodes,
with a composition depth limit of 64 and an aggregate work allowance of 65,536 items.
These are rejection limits, not a performance or memory guarantee.

The aggregate allowance counts source, operation and graph work per composition request;
it is not a wall-clock deadline or exact memory quota. Request-local caching avoids repeated
dependency traversal, applies instance overrides to independent clones and still enforces
depth when reusing a cached subtree. Duplicate source-map keys fail rather than using the
last value. Hierarchy parents must directly identify live composed nodes; symbolic remap
aliases remain supported for component/port references, not hierarchy parents. Every
declared nested binding is validated even when no component uses it.

There is no implicit legacy reader or migration. Keep an incompatible or invalid source
for repair rather than replacing it with an empty runnable scene.

For the difference between contract tests and native application checks, see
[Testing](../testing/index.md); startup arguments are in the
[editor CLI reference](../cli/editor-and-assets.md).

Contributors changing these guarantees should use the
[scene runtime contract](../../contributing/contracts/scene-runtime.md).
