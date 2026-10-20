# Native project startup

Kedra starts an engine splash, opens a project manager or prepares an explicitly
selected project, then enters the native dockable editor. It does not build or
execute project code, scaffold a game, or offer an editor session without a
validated project.

## Run

Build and verify a new processed asset delivery before launching:

```sh
cargo run -p kedra -- assets build assets/editor target/editor-assets --namespace editor
cargo run -p kedra -- assets verify target/editor-assets
cargo run -p kedra -- --assets target/editor-assets
cargo run -p kedra -- --assets target/editor-assets /absolute/path/to/project
```

Build output must be a new directory. An existing delivery is never deleted or
overwritten. Without `--assets`, startup uses `KEDRA_ASSETS`, then `assets` next to
the executable. Installation supplies both executable and delivery; asset
resolution does not depend on the working directory or authoring checkout.

Game asset sources use the same importer and runtime loaders:

```sh
cargo run -p kedra -- assets init /absolute/path/to/game/assets
cargo run -p kedra -- assets build /absolute/path/to/game/assets target/game-assets --profile game-desktop --namespace game
cargo run -p kedra -- assets verify target/game-assets --profile game-desktop
```

## Asset contract

Authored sources and native `.meta` sidecars own identity and processing settings.
Initialization assigns missing sidecars without overwriting settings. UUIDs
survive source renames when source and sidecar move together.

References prefer readable qualified paths such as `editor://icons/spinner.svg#1x`.
`editor://<uuid>/asset.svg#1x` is also supported. Both resolve through the same
manifest-verified reader, with exact case-sensitive Unicode delivery paths.
Path references need explicit updates after renames; automatic reference-aware
rename is not implemented. `<uuid>/asset.<extension>` is reserved for UUID lookup.
Bevy caches requested AssetPaths, so consumers should use one spelling per resource.
Physical member paths must roundtrip through native Bevy addressing without a
source or label change. A literal `#` is therefore rejected in filenames or parent
directories; `%23` is a different literal filename, not an escape for `#`.

Delivery contains processed bytes, native loader metadata and hashes. No Kedra UI
assets are embedded in executables. SVG becomes a native Bevy Image in the importer;
`SvgToImageVariants` supports authored sizes and integer densities 1–8. Editor
icons use 1x–4x. SVG external-file references are disabled. Fonts and their license
notices are separate delivered assets; see [asset provenance](../assets/README.md).
The exact serialized delivery manifest is limited to 64 KiB. Import rejects an
oversized marker before publishing an output; large source inventories may need a
future reviewed format/bound change, not a delivery that its reader cannot open.

A runtime consumer can install `KedraAssetsPlugin` and a verified source without
depending on editor, importer, window or filesystem authoring-writer crates.
Hashes detect content changes; they do not authenticate publishers.

## Application ownership

```text
apps/kedra: CLI and plugin composition
  kedra_import -> kedra_assets: processing and runtime delivery
  kedra_editor: startup model, requests, jobs and views
    kedra_project: project files and recent-project preferences
    kedra_ui / kedra_scene: authored native views
    kedra_desktop: window, camera and root ownership
```

Startup models and drafts are ordinary Rust values. Bevy resources/states adapt
them to scheduling; entity handles stay in presentation code. Semantic requests
and asynchronous completions carry generations, preventing obsolete results or
duplicate activation from entering the wrong project.

Each desktop surface owns its window, targeted camera and root, including pending
scene instances. Transitions retire only that ownership. Stale close events do
not exit a replacement surface. Floating editor hosts have separate ownership;
see [docking](ui/docking-implementation.md).

## Boot and preparation

The 640×360 engine splash uses `scenes/boot.prefabs.ron`. Startup initially reads
only a bounded manifest; each bootstrap resource read still verifies its hash.
After native creation, hydration and a nonblank renderer readback, complete
delivery verification starts on the I/O pool and the full catalog is requested.
Project workflow waits for both. Bootstrap failure reports diagnostics and exits;
there is no embedded substitute UI.

Bevy/GPU initialization precedes the splash. Cold first-visible-frame latency and
OS presentation timing are not certified by renderer readback.

Project preparation has a separate loading surface. Success waits for completion,
native creation and hydration frame boundaries, with no artificial minimum duration.
The task is not delayed; failures appear immediately. The spinner turns linearly
once per second. `KEDRA_REDUCED_MOTION=1` keeps it static; automatic native preference
detection is not implemented.

## Project sources and failures

Creation writes `parent/name/kedra.toml` and an empty `assets/` directory. Any
existing destination, including an empty directory or symlink, is rejected.
The manifest is synchronized and published last without clobbering existing files.
Interrupted creation can leave an invalid directory; it is not silently reused
or recursively removed. Cancellation does not promise rollback of published files.

Opening validates bounded UTF-8 TOML, project identity, format compatibility and
an asset root contained in the project. It is read-only and never executes code
or migrates implicitly. Source format, not the informational editor release,
determines compatibility. Only format 1 exists.

Recent projects are bounded local preferences, separate from sources. Only
successful opens are remembered. Corrupted history remains available and is
reported. A history-write failure does not turn a successful open into project
failure. Concurrent recent-history writers currently have last-writer-wins behavior.

## Native UI and limitations

External scenes/prefabs define splash, loading, manager, modals, rows, captions
and workspace structure. Flair styles them; native Bevy widgets supply text
editing, clipboard and modal navigation. The editor has [Assets](ui/asset-browser.md),
Hierarchy, Document Preview and Inspector panels. Assets provides navigation,
search and read-only source document Open. Hierarchy displays composed source
nodes; Inspector and the initial Preview slot are empty. Source tabs host
read-only [Scene View](ui/scene-view.md) for supported spatial/UI documents, with
an explicit matching verified project delivery. PNG/static-GLB Open uses separate
asset previews. Spatial navigation and scoped opaque-mesh selection are available;
Inspector editing, document Save, scene editing and Play are not implemented.

The custom 32px caption is the only painted caption. On macOS the native window
retains AppKit's titled/resizable structure with hidden native title/buttons and
full-client content. This avoids Winit frame churn from truly borderless maximize
queries. Platform windowing still owns move, resize, minimize, maximize and close.

[Startup references](ui/startup-layout-lock.md) pins visual expectations.
[Automation](automation.md) documents optional native input/capture checks.
Bevy input injection and renderer captures have macOS evidence, including DPR
overrides 1/2. This is not physical OS input or cross-platform certification.

Remaining acceptance includes deterministic startup fixture data, full control
states and selected/missing recent-row presentation, physical picker/chrome/IME
behavior, native DPI/monitor transitions, capture reliability across environments
and pixel equivalence on supported platforms. Font subsets do not guarantee every
Unicode script. Invalid frames fail rather than being replaced with synthetic pixels.
See the [roadmap](roadmap.md) for subsequent product work.
