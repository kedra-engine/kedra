# Supported formats and processing settings

Kedra's asset tools accept the formats and settings below. Other Bevy formats and features
are not automatically supported, and editor previews have additional limits.
Initialization preserves settings in existing sidecars.

## Default source handling

| Source | Default action from `assets init` | Important limit |
| --- | --- | --- |
| `.svg` | `SvgToImage` processing | Produces one PNG-backed Bevy image, not runtime vector graphics |
| `.png` | Native `ImageLoader` | Invalid image bytes fail build |
| `.glb` | Native `GltfLoader` | GLB/glTF 2.0 admission; cameras, lights and animations disabled by default |
| `.ttf`, `.otf` | Native `FontLoader` | Font data is validated; ship required license notices separately |
| `.css` | Flair `CssStyleSheetLoader` | Strict `ReturnError` mode; source at most 64 KiB |
| `.scene.ron`, `.prefab.ron` | `DocumentLoader` | Parsed documents; document UUID must equal sidecar UUID |
| `.prefabs.ron` | `CatalogLoader` | Scene/prefab source catalog, not a delivery manifest |
| `.environment.ron` | `EnvironmentLoader` | Version-1 environment data; bounded loader |
| Other `.txt`, `.json`, `.ron` | `DataLoader` | Opaque bytes, not automatic domain parsing |
| Other extensions | `Ignore` | Not included in the resulting delivery |

Suffix-specific RON loaders take precedence over the generic data loader. Merely naming
a file `.json` or `.ron` does not validate its contents as a project configuration or scene.
CSS imports must resolve through the verified source; invalid CSS or a missing import fails
native runtime verification during build. Recovery-mode CSS metadata is not admitted.

For document fields, see the [scene source reference](../scenes/index.md#document-fields);
for `.prefabs.ron` catalogs, see [host preparation](../scenes/lifecycle.md#prepare-the-host).

The repository's test project provides examples: `checker.png`, `ship.glb`, `preview.css`,
`day.environment.ron` and `Fixture-Inter.ttf` under `tests/fixtures/test_project/assets`.
Its browser-only text files with authored `Ignore` metadata remain source inventory entries
without becoming delivered data assets. Use the [pipeline walkthrough](pipeline.md) to build
that inventory unchanged.

## SVG image variants

`SvgToImage` and `SvgToImageVariants` share these settings:

- `size: None` uses the authored SVG dimensions; `Some([width, height])` sets logical pixels.
- `densities` is a nonempty, strictly increasing list of integers in 1–8.
- Each rasterized output must contain 1–16,777,216 pixels, with nonzero dimensions.
- External image/font files are not loaded. Keep SVG sources self-contained.

`SvgToImage` requires exactly `densities: [1]`, the initialization default.
`SvgToImageVariants` produces a parent `ImageVariants` asset with labeled native images
such as `#1x` and `#2x`. Request only labels authored in the package; keep the parent or
labeled image handle alive for as long as it is consumed.

For a real settings example, `assets/editor/icons/spinner.svg.meta` selects
`kedra_import::processors::SvgToImageVariants` with this settings value:

```ron
(size: None, densities: [1, 2, 3, 4])
```

This is a settings excerpt, not a complete sidecar. Preserve the existing UUID and native
metadata wrapper when changing settings in a private source copy. After processing,
`editor://icons/spinner.svg#2x` addresses the 2x image, even though the physical delivered
path retains `.svg`. Native metadata selects the processed loader.

New variant packages use version 2 with Base64-encoded lossless PNG payloads; the runtime
also reads version 1 numeric-byte payloads. This is not GPU texture compression, automatic
DPI selection or automatic generation of every density from 1 through 8.

The editor icons in the repository use densities 1x–4x. The format decision and compatibility
rationale remain in [ADR 0022 (English)][compact-images].

## Fonts and notices

Fonts and their required license notices are separate delivered assets. Retain those notices
when distributing resources; successful font loading is not license clearance. The exact
source identities, licenses and derivation belong to [asset provenance (English)][provenance].
Bundled subsets do not guarantee every Unicode script; missing glyphs require a deliberate
licensed font/fallback choice and native verification.

## GLB model admission versus editor presentation

The model container is binary GLB with glTF asset version `2.0`. JSON `.gltf` is not an
admitted substitute, even with metadata naming the native glTF loader.
Before native loading, admission enforces:

- No multiple parents, duplicate child edges or cycles, including disconnected nodes.
- At most 128 nodes on a root-to-leaf path.
- Separate ceilings of 4096 nodes, 4096 mesh primitives and 4096 aggregate native scene entities.

Expansion includes every scene container, repeated root/subtree, primitive instance and
possible punctual-light child, regardless of native loader flags. Valid checksums cannot
bypass these structural limits. Build checks GLBs even when their metadata says `Ignore`;
GLB magic and native glTF-loader metadata also trigger admission after a file rename.
Geometry, buffers, textures and loader settings still need normal native-loader validation.

These admission limits do not promise full editor model support. Direct GLB preview uses
`Scene0`; imported cameras, lights and animations are disabled, and skins/morphs are rejected.
Rendering has additional presentation limits. A model passing asset verification is not
proof that it can be rendered in every editor view.

## Delivery and reader limits

The exact serialized `kedra-assets.json` must fit 64 KiB. A data member may declare at most
256 MiB; a `.meta` member at most 64 KiB. The verified reader bounds reads before hashing,
rejects undeclared resources and does not offer general filesystem directory enumeration.
Native members must resolve to regular files inside the canonical delivery root; escaping
symlinks and special files are rejected. Keep registered deliveries immutable.

For errors at any of these boundaries, use [troubleshooting](troubleshooting.md).

[compact-images]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0022-compact-image-delivery.md
[provenance]: https://github.com/kedra-engine/kedra/blob/main/assets/README.md
