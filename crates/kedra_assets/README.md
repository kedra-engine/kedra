# kedra_assets

Runtime-layer verified delivery, stable Bevy named asset sources, generic image
variants and opaque data loaders. No editor, filesystem authoring writer, importer
or window dependency. Hosts own source registration, delivery trust and ordinary
Bevy asset/image plugins; `KedraAssetsPlugin` registers Kedra asset types/loaders.

`Manifest::verify` accepts a reader and does not require filesystem access.
`read_directory` is a native-host helper. Register a source before `AssetPlugin`;
load `game://UUID/asset.svg#2x` using normal `AssetServer` handles. Keep parent or
labeled handles alive for the lifetime of their consumer. Reads reject missing,
incorrect or corrupted members; checksums do not authenticate a publisher.
Physical member names must roundtrip losslessly through native asset-path syntax;
`#` belongs to URI labels and cannot appear in a physical delivery member name.
Manifest keys always use `/`. Runtime lookups translate the platform's native
separator before matching them, including Bevy-resolved relative CSS imports and
UUID references on Windows. This does not permit traversal or absolute paths;
backslashes remain invalid delivery references on Unix.

Reads stop at the declared size plus one byte before hashing. Declarations are
limited to 256 MiB per data member and 64 KiB per metadata member. Native filesystem
members (including the manifest) must be regular files inside the canonical root;
symlinks resolving outside it and FIFOs are rejected before opening. Deliveries
must remain immutable while registered: these portable preflight checks are not
an OS sandbox against concurrent filesystem replacement. Browser/Android packaged
readers retain their platform backend and enforce the same byte budgets.

## Optional model admission

The `model-support` feature exposes `model::validate_glb(&[u8]) -> Result<(), String>`
and guards verified model reads before returning bytes to Bevy. Editor and importer
consumers explicitly enable it; model-loading hosts must do the same. Minimal and
default builds do not include the parser. Matching hashes do not imply safe topology.
GLB extensions, GLB magic and native glTF-loader manifest records select admission;
metadata reads remain unchanged. The supported model container is GLB/glTF 2.0,
not JSON `.gltf` supplied through custom native loader metadata.

Admission rejects multiple parents/duplicate edges, cycles (even disconnected),
depth above 128 nodes and aggregate native scene expansion above 4096 entities.
Every scene container, repeated root/subtree, mesh primitive instance and possible
punctual-light child counts, independently of native loader settings. Separate
4096-node and 4096-mesh-primitive ceilings also bound disconnected native assets.
Iterative forest accounting avoids recursive traversal and saturates expansion
arithmetic. Schema/index validation precedes accessing graph references. Geometry,
buffers, textures and settings still require normal native-loader validation.

The optional MIT/Apache-2.0 `gltf` 1.4.1 dependency is already locked through Bevy
0.20.0 and uses its maintained binary/schema API rather than a custom parser.
Default import/image features are disabled; only punctual-light schema support is
requested, avoiding filesystem/image decoding in this runtime policy API. It shares
the native loader's parser/version and serde stack on supported Rust targets; the
small forest/budget policy is Kedra-specific, not a replacement loader or asset graph.
The API is shared because raw importer sources bypass verified delivery readers;
both trust boundaries need the same admission rules without runtime-to-import edges.

The importer integration test is an independent game-style host that loads after
authoring sources are unavailable. See [startup](../../docs/startup.md).
