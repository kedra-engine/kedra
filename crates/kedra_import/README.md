# kedra_import

`ingest::create_environment` publishes a validated `.environment.ron` and fresh
metadata through the existing staged importer. It shares the portable filename
policy from `kedra_authoring_fs`, refuses existing source/sidecar destinations and
does not build a runtime delivery. The same per-file publication limits as external
import apply; callers readmit sources after completion.

This import-layer crate applies Kedra policy to native Bevy processing without a
second scheduler or cache.
`ingest::import_files` copies explicit external files/folders into an existing asset
directory. WalkDir handles traversal, tempfile handles staged no-clobber publication,
and the existing initializer assigns fresh metadata. Incoming sidecars are excluded;
original files and existing destination identities are preserved. Preparation failures
publish nothing. Publication I/O failures report the number of source files already
copied; a batch is not a filesystem transaction. Import does not build a delivery.
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
16 MiB inventory budget before the public output directory is created. This is
independent of repository source-file growth limits. Generated manifests use
compact JSON; formatting whitespace consumes none of the inventory budget. These checks are not
a sandbox against concurrent filesystem replacement.

Model admission enables `kedra_assets/model-support` explicitly and validates raw
GLB sources before constructing Bevy's `AssetProcessor`, including ignored GLBs.
GLB magic and native glTF loader metadata cannot bypass admission by renaming the
source extension. Unsupported JSON glTF containers fail that metadata path.
The shared maintained `gltf` 1.4.1 parser is Bevy 0.20.0's locked dependency;
no separate model loader, graph or processing framework is introduced. Invalid
topology or excessive aggregate native expansion fails without publishing a
delivery or changing source bytes/sidecars. See the
[runtime package](../kedra_assets/README.md#optional-model-admission) for exact limits
and dependency/public-API rationale. Sources must remain immutable during build;
preflight and final source hashes are not an OS sandbox against concurrent writes.

```sh
cargo test -p kedra_import --locked
```

See [startup](../../docs/site/en/contributing/contracts/startup.md)
for CLI usage and publication/failure limits.
