# Paths and identity

Kedra uses paths to locate sources and persistent identities to distinguish their contents.
The same source can have a filesystem location, a readable editor label and a qualified logical
path. These are related, but they are not interchangeable.

## Which path should I use?

The repository's test project includes `assets/Hud.prefab.ron`, which illustrates these forms:

| Form | Example | Purpose |
| --- | --- | --- |
| Native filesystem path | `/work/Kedra Test Project/assets/Hud.prefab.ron` | OS file access; this is an illustrative absolute location. |
| Project-relative path | `assets/Hud.prefab.ron` | Identify a file relative to the project root. |
| Asset-root-relative path | `Hud.prefab.ron` | Locate a member within the configured asset source directory. |
| Logical path | `game://Hud.prefab.ron` | Identify a member within the explicitly named source namespace. |
| Persistent document UUID | `7e570000-0000-4000-8000-000000000004` | Identify the authored document independently of its filename. |

The root in the native example is not a required installation path.
For this fixture, `assets = "assets"` connects the project-relative and asset-root-relative forms.
If a manifest uses another directory, resolve it from that project's root, not the process cwd.

In-project source labels use asset names or relative display paths rather than exposing absolute
filesystem locations. Such a display label is not a replacement for the native locator used by
file access. The Projects window is different: it must let you choose and distinguish project
locations, and can abbreviate the home directory as `~`.

## Logical paths name a source, not a disk

`game://Hud.prefab.ron` contains a source namespace (`game`) and a member path (`Hud.prefab.ron`).
It is not a URL to fetch and is not an absolute native filesystem path.
The editor's admitted project inventory uses the `game` namespace; its own authored interface
uses the separate `editor` source, for example `editor://prefabs/button.prefab.ron`.

A project UUID does not become a namespace. The project manifest does not register arbitrary
namespaces or dependency projects. A matching source must be supplied by the consuming application;
merely writing a plausible logical path does not make an asset available.

The source catalog associates admitted documents and assets with their paths and identities.
Native scene/prefab references use full source-qualified paths, resolved against loaded catalog
members and their published source revision. Do not use a bare filename or hardcoded document UUID
for these lookups. Persisted source references still retain UUID identities.

In the editor, project document admission creates its catalog in memory from the source inventory.
Opening does not generate a catalog file or initialize missing sidecars.
A source catalog is not the recent-project list and is not a processed asset delivery.
Catalog membership alone does not prove renderer readiness or authorize execution.

## Preserve identity when moving files

Three identity levels must stay distinct:

- The project UUID is stored in `kedra.toml`.
- Asset UUIDs are stored in native `.meta` sidecars. A scene/prefab sidecar, when present,
  must agree with that document's UUID.
- Documents also persist node and nested-instance identities. Runtime entities and loaded asset
  handles are projections, not replacement persistent identities.

In the test project, both `Hud.prefab.ron` and `Hud.prefab.ron.meta` carry the document UUID
shown above. Renaming or moving that source means keeping the sidecar with it.
Do not regenerate UUIDs just because a project's native directory changes.
Conversely, copying bytes preserves their identities; it does not create independent identities.

Readable path references need explicit updates after a rename. Preserving an asset UUID does not
automatically rewrite `game://Hud.prefab.ron` wherever it was used.
Automatic reference-aware rename is not implemented.

Logical member paths preserve case and Unicode. Do not rely on a case-insensitive filesystem
to correct a reference's spelling. A literal `#` in a filename or parent directory is rejected
where it would be interpreted as an asset label; `%23` is a different literal name, not an escape.
Spaces and Unicode do not require URI encoding in these source paths.

## When a valid project still rejects a source

Project validation checks that the asset root stays inside the project.
Document admission is stricter:
symlinked or special inventory members are rejected, duplicate/nil identities fail, and references
must resolve inside the admitted project source scope. A regular-file symlink for `kedra.toml`
can be accepted; that does not imply symlinked asset members are supported.

The read-only source workspace retains an admitted baseline, not a continuously refreshed file
view. Newly added documents or changed source bytes can require reopening the project before
admission. Existing documents are retained when a later source check fails; Open is not a repair
or overwrite operation. See [Create or open a project](create-open.md) for recovery boundaries.

## Related pages

- [Projects](index.md)
- [Create or open a project](create-open.md)
- [Asset identity and sidecars](../assets/index.md#identity-belongs-with-the-source)
- [Scene node identities and references](../scenes/index.md#identities-and-references)
