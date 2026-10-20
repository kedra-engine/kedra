# kedra_import

Import-layer policy over native Bevy processing, not a second scheduler/cache.
`KedraImportPlugin` registers generic SVG-to-image and SVG-to-variants processors.
Games and the editor use identical processors and runtime loaders.

`initialize` assigns missing UUID/native metadata sidecars without overwriting
authored settings. `build` creates a fresh transaction directory, processes,
validates native runtime loading and source hashes, then publishes a new delivery.
It rejects existing outputs and does not promise incremental cache reuse yet.
Hosts supply visible progress; failed processing does not modify authoring sources.
Static symlinks and special files (including sidecars) are rejected before content
reads. Physical paths must retain their exact native asset-path meaning, so `#`
is not permitted. The exact serialized delivery manifest must fit the runtime's
64 KiB limit before the public output directory is created. These checks are not
a sandbox against concurrent filesystem replacement.

Model admission enables `kedra_assets/model-support` explicitly and validates raw
GLB sources before constructing Bevy's `AssetProcessor`, including ignored GLBs.
GLB magic and native glTF loader metadata cannot bypass admission by renaming the
source extension. Unsupported JSON glTF containers fail that metadata path.
The shared maintained `gltf` 1.4.1 parser is already Bevy 0.20.0's locked dependency;
no separate model loader, graph or processing framework is introduced. Invalid
topology or excessive aggregate native expansion fails without publishing a
delivery or changing source bytes/sidecars. See the
[runtime package](../kedra_assets/README.md#optional-model-admission) for exact limits
and dependency/public-API rationale. Sources must remain immutable during build;
preflight and final source hashes are not an OS sandbox against concurrent writes.

```sh
cargo test -p kedra_import --locked
```

See [startup](../../docs/startup.md) for CLI usage and publication/failure limits.
