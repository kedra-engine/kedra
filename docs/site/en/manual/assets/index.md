# Assets

Assets connect editable project files to resources an application can load. Keep their
identities and metadata with the sources, then build a delivery for loading at runtime.

For project directories and path forms, start with
[Paths and identity](../projects/paths-and-identity.md).

## Sources, deliveries and loaded resources

Keep three forms of an asset distinct:

1. **Source assets** are authored files and their adjacent `.meta` sidecars. These are
   the files to retain in version control, including identity and processing settings.
2. An **asset delivery** is a separately built directory of processed bytes, native loader
   metadata and the `kedra-assets.json` manifest. It is derived output, not an editing folder.
3. **Loaded resources** are Bevy assets and handles in a running application. Loading does
   not save changes back to sources. Handles are not persistent asset identities.

For example, `assets/editor/icons/spinner.svg` is an authored SVG. Its sidecar selects
multi-density rasterization. In an editor delivery, `icons/spinner.svg` still has that
logical name, but its bytes are a processed image package, not the original SVG.
`editor://icons/spinner.svg#2x` selects an image from that package at runtime.

A verified delivery can load without its original source checkout. Do not distribute an
unprocessed source folder as though it were a delivery, or edit delivered bytes in place.
Rebuild derived output after changing sources or processing settings.

## Identity belongs with the source

The sidecar for `checker.png` is `checker.png.meta`, not `checker.meta`.
It contains a UUID and a native Bevy action: load, process or ignore. Keep it with the
source when moving or renaming a file; this preserves the asset UUID across delivery builds.
For `.scene.ron` and `.prefab.ron`, initialization uses the document's own UUID, and build
rejects a sidecar whose UUID disagrees with that document.

`assets init` assigns missing sidecars. It does not overwrite existing IDs or settings,
repair malformed existing metadata, or add missing resources to a source catalog.
Deleting a sidecar and initializing again can assign a new identity to a non-document asset.
Copying a source and its sidecar into the same inventory can instead create duplicate IDs,
which build rejects. Treat identity changes as authored changes, not cache cleanup.

The editor hides sidecars as independent Asset Browser entries. A visible source file
does not necessarily have a delivered resource: an `Ignore` action excludes it from delivery.

## Logical references and source registration

A logical path names a source scope and a path inside it, rather than an absolute filesystem
location. For example:

```text
editor://icons/spinner.svg#2x
game://checker.png
```

`editor` and `game` are named asset sources. The host registers each delivery before Bevy's
`AssetPlugin` starts; the URI is meaningful only within that registered scope.
Source catalogs describe admitted source documents and their paths/identities; they are
not interchangeable with the processed delivery manifest.

Readable qualified paths are preferred. A UUID address such as
`source://UUID/asset.svg#2x` can also resolve through the delivery manifest. Here `UUID` is
the asset's actual sidecar identity, not a literal directory to create. The shape
`<uuid>/asset.<extension>` is reserved for that lookup and cannot be a physical asset path.
Native scene/prefab references use full source-qualified logical paths resolved through
the loaded source catalog; do not replace them with hardcoded UUIDs.

References are exact and case-sensitive, including Unicode spelling. Use `/` in portable
paths. Absolute paths, traversal and physical `#` characters are rejected; `#` separates a
subasset label. `%23` is a different literal filename, not an escape for `#`.
Moving an asset with its sidecar preserves identity, but readable path references still
need explicit updates. Automatic reference-aware rename is not implemented. Bevy caches
requested paths, so use one reference spelling per resource rather than mixing aliases.

## Choose your next step

- [Build and verify a delivery](pipeline.md) using the repository's editor assets or test project.
- [Check supported formats and settings](supported-formats.md) before adding a source.
- [Diagnose missing or stale resources](troubleshooting.md) without changing asset identity.
- [Scenes and prefabs](../scenes/index.md) explains the documents that reference these resources.

The editor cannot edit assets, save documents or run project Play. It uses explicit
deliveries for read-only previews;
their [matching rules](pipeline.md#editor-previews-need-matching-sources) are narrower than
the general runtime pipeline. Test previews with real input and frames in the editor;
the headless commands in this section do not check them.

Use the [asset CLI reference](../cli/editor-and-assets.md) for exact command options and
[Testing](../testing/index.md) to choose integrity, behavior or image checks.
