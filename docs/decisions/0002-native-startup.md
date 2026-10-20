# ADR 0002: Native project startup on shared modules

Status: architecture in use. Native platform acceptance is separate.

## Context

Project selection, creation and preparation need a native entry point without a
separate launcher, version manager or permission to execute untrusted project code.
Editor resources must use the same asset runtime as non-editor consumers.

## Decision

- Use one `kedra` executable and pinned Bevy 0.20.0 with explicit features.
  Splash, manager, project loading and editor workspace are distinct phases with
  separately owned native surfaces.
- `kedra_project` owns project files and validation, `kedra_desktop` owns native
  window/camera/root lifetimes, `kedra_ui` owns shared authored UI projection and
  `kedra_editor` composes startup behavior and feature panels.
- Models and drafts are ordinary Rust values. Bevy resources/states adapt them
  to scheduling; persistent feature rules do not contain entity handles.
- Presentation adapters own entities, native events, focus and asynchronous jobs.
  Generations reject obsolete completions; duplicate requests cannot enter a
  second or stale project session.
- Update retained text/value components. Structural replacement retires only
  owned subtrees. Do not introduce a second ECS, reducer framework or layout engine.
- `kedra_assets` and `kedra_import` serve editor and game-style consumers through
  native Bevy sources, loaders and processors. Authored SVG/fonts and metadata
  remain outside executables; installed applications use verified deliveries.
- Use preprocessed density variants and native labels. SVG rasterization belongs
  in the importer, not the UI runtime.
- Source format and informational editor release are distinct. Opening is
  read-only; unsupported formats fail without implicit migration or code execution.
- Client-drawn captions invoke native window operations. The macOS backend keeps
  a hidden titled/resizable AppKit structure. [ADR 0003](0003-application-automation.md)
  covers the native input/capture boundary.

Authored source ownership and styling are defined in
[ADR 0005](0005-authored-scenes.md) and [ADR 0006](0006-native-css-styling.md).
Docking has its own [ownership contract](0008-docking-ownership.md).

## Reuse and alternatives

Bevy owns ECS, scheduling, UI widgets, text input, clipboard, assets, rendering and
native window components. `rfd` supplies platform pickers; `directories` locates
local data; `tempfile` supplies publication primitives; `walkdir` inventories source
files. Serde/TOML/RON/JSON own their schemas. `resvg` rasterizes importer-side SVG,
`image` writes PNG and `read-fonts` validates font containers.

A separate launcher needs an extra process protocol without a current version
selection requirement. Embedded editor resources prevent ordinary delivery
validation. Domain rules encoded in entity queries couple features to tree shape;
full-tree replacement loses focus and drafts. These alternatives are not used.

## Guarantees and limits

Sources, derived deliveries, preferences and unsaved drafts have distinct owners.
No runtime filesystem authoring writer is installed. Build publishes a fresh
immutable delivery; it does not promise incremental cache reuse or live replacement.
Manifest-last project/delivery publication prevents partial directories from being
accepted as valid, but does not imply multi-file power-loss rollback.

Scene editing, project builds/execution and Play are outside startup's implemented
scope. Dependency exceptions are explicit in [ADR 0007](0007-dependency-policy.md),
not bypassed by native UI work.

## Verification

Project failure-path tests, generation/cancellation tests, native ownership tests
and independent source-free installed consumers are required. Run repository gates
and relevant feature/target checks. Physical keyboard, picker, chrome, DPI and
monitor behavior require native input and frames on the claimed platform.
See [startup](../startup.md) for commands and current limitations.
