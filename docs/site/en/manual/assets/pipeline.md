# Process and verify assets

Use this workflow to turn initialized sources into a delivery and check it independently.
Run the examples from the repository root with the repository's Rust toolchain available.
Processing is headless: it does not open the editor or execute project code.

Before running commands, distinguish
[sources and deliveries](index.md#sources-deliveries-and-loaded-resources).
For build prerequisites and a first editor launch, start with
[Getting started](../getting-started/index.md).

## Initialize sources you own

For a new source tree, run `assets init SOURCE` before building. This operation writes
missing `.meta` sidecars beside source files. Review and retain those files with the sources.
It does not replace existing metadata, even if existing settings are invalid.
Each new sidecar is published independently; a failure can leave earlier sidecars in
place. Rerunning initialization preserves them rather than assigning those identities again.

The repository's `assets/editor` and `tests/fixtures/test_project/assets` trees already include
sidecars. **Do not initialize or edit those canonical fixtures to follow this walkthrough.**
Work on a complete private copy if you need to experiment with source changes.
You can inspect command syntax without writes:

```sh
cargo run -p kedra -- assets init --help
cargo run -p kedra -- assets build --help
cargo run -p kedra -- assets verify --help
```

Initialization does not convert formats. Unknown extensions receive `Ignore`
metadata; [supported formats](supported-formats.md) describes the actual defaults.

## Build a fresh delivery

The **output directory must not exist**, even as an empty directory. Its parent may exist.
Choose a different output name for every subsequent build; build never overwrites an old
delivery. Source and output roots must be disjoint, with neither containing the other.
Do not put the delivery inside `assets/`.

Build the editor resources:

```sh
cargo run -p kedra -- assets build assets/editor target/documentation/examples/assets/editor-delivery --namespace editor
cargo run -p kedra -- assets verify target/documentation/examples/assets/editor-delivery
```

Build the repository's test project separately:

```sh
cargo run -p kedra -- assets build tests/fixtures/test_project/assets target/documentation/examples/assets/project-delivery --namespace game
cargo run -p kedra -- assets verify target/documentation/examples/assets/project-delivery
```

Both examples intentionally use the default `editor-desktop` profile. This is also the
profile required by the editor's explicit project-preview binding.

`--namespace` is mandatory for build. It identifies the logical source used when validating
delivered references, for example `editor` or `game`. It must contain 1–64 ASCII letters,
digits, underscores or hyphens. It does not rename authored references automatically.

`--profile` defaults to `editor-desktop` for both build and verify. Currently it is an exact
manifest compatibility label, **not a selector for image sizes, compression or platforms**.
Build accepts a nonempty label of at most 64 bytes; verify requires the same label.
An application can use `game-desktop` when its host expects it, but that label does not
make the delivery acceptable to the current editor's project-preview binding.

For a standalone host that explicitly expects `game-desktop`, initialize only a new source
tree you own, then build and verify with that same profile:

```sh
cargo run -p kedra -- assets init /absolute/path/to/game/assets
cargo run -p kedra -- assets build /absolute/path/to/game/assets target/game-assets --profile game-desktop --namespace game
cargo run -p kedra -- assets verify target/game-assets --profile game-desktop
```

`target/game-assets` must be a fresh output, not an existing delivery. This standalone-host
example is not the recipe for the editor's `--project-assets` binding.

## What build validates

Build inventories sources and metadata, rejects invalid or duplicate identities, and runs
native Bevy processing in a fresh transaction directory. It then loads delivered assets
and their dependencies through the manifest-checked runtime source. Scene/prefab documents
also undergo catalog composition checks. Sources and their sidecars are hashed again
before publication to detect changed bytes or inventory.

The published delivery contains processed members, their `.meta` loader metadata and
`kedra-assets.json`. That marker is written last, after validation. A filesystem failure
during final copying can leave an incomplete output directory; absence of a valid marker
means it is not a usable delivery. Use a fresh destination rather than reusing that directory.

Before creating the output directory, build checks that the serialized manifest fits the
reader's 64 KiB limit. A larger source inventory may require a reviewed change to the format
or size limit. Do not publish a delivery that its reader cannot open.

Keep sources unchanged throughout processing. Static symlinks and special files, including
sidecars, are rejected. These checks are not a sandbox against concurrent file replacement.
The current builder uses a clean transaction each time, without promising incremental cache
reuse. Processing has a 60-second timeout; native dependency verification has a 30-second
timeout. Failure reports diagnostics instead of publishing a knowingly invalid delivery.

## What standalone verify proves

`assets verify ROOT` is read-only. It checks manifest format/profile, identities, member
paths, declared byte lengths, SHA-256 hashes and agreement between loader records and
processed metadata. It does **not** repeat build's native dependency-loading/composition
pass, compare the delivery to current authoring sources, render frames or authenticate
the publisher. A valid old delivery can therefore verify while failing editor source matching.

Prefer an explicit `ROOT` for diagnosis. If omitted, lookup uses `--assets`, then
`KEDRA_ASSETS`, then the platform's bundled asset location relative to the executable.

## Editor previews need matching sources

The editor's own `--assets` and project `--project-assets` bindings serve different sources.
The latter requires an explicit project root, namespace `game` and profile `editor-desktop`.
Open does not build resources or initialize metadata. After changing a source or delivery,
rebuild to a new directory and relaunch with that directory to change the binding.

On Open, document paths, identities and hashes must match the admitted source snapshot.
Every delivered non-document resource must also match its source path, identity and bytes.
This supports byte-preserving PNG, GLB, CSS, font and document resources, not arbitrary
processed outputs. In particular, runtime SVG processing works, but differing SVG source
and output bytes do not satisfy this preview profile without source provenance support.

Missing or stale resources produce diagnostics; accepted source navigation is retained.
Direct PNG/static-GLB previews are read-only inspections, not game execution or asset editors.
For loading a delivery in a custom host, register it before `AssetPlugin`, install the
appropriate native loaders and keep consumer handles alive. `KedraAssetsPlugin` alone does
not install every Bevy loader. Model hosts must explicitly enable `model-support`.

See [format limits](supported-formats.md) and [failure recovery](troubleshooting.md).
For preview controls after preparation, see [Scene View](../editor/scene-view.md).
For scene integration in a custom host, continue to
[Loading and lifecycle](../scenes/lifecycle.md#prepare-the-host).
