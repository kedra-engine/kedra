# Application configuration

Kedra reads standalone build settings from `Cargo.toml`, not `kedra.toml`.
The selected package must have exactly one binary target and both native package metadata
and Kedra distribution metadata. Cargo defines the package name, version and binary identity.

Distinguish this from [creating an empty editor project](../projects/create-open.md).
Review [host configuration](../programming/index.md), [asset preparation](../assets/pipeline.md)
and [build/launch prerequisites](build-and-run.md) before using the metadata below.

## The project-local launcher

In this repository, `.cargo/config.toml` contains:

```toml
[alias]
xtask = "run --locked --package xtask --target-dir target/xtask-bootstrap --"
kedra = "run --locked --package kedra-build-tool --target-dir target/kedra-build-bootstrap --"
```

The `tools/kedra` launcher depends on the tooling crate `kedra_build`. Its entire entry
point forwards native arguments without converting them to UTF-8 strings:

```rust
fn main() -> std::process::ExitCode {
    kedra_build::run_cli(std::env::args_os())
}
```

Keep the launcher separate from the application's runtime dependencies. Its isolated
bootstrap target directory prevents a workspace build from overwriting the running tool
on Windows. Retain an existing `xtask` alias; it has a different responsibility.
An independent project needs its own reviewed launcher/dependency configuration and lockfile;
copying the root alias alone does not install Kedra or create an application.

## A real metadata example

The following is the current editor configuration from `apps/kedra/Cargo.toml`:

```toml
[package.metadata.packager]
productName = "Kedra"
identifier = "dev.kedra.editor"
description = "Open-source game development environment built on Bevy"
publisher = "Kedra contributors"

[package.metadata.packager.macos]
minimumSystemVersion = "15.0"

[package.metadata.kedra.distribution]
assets = "../../assets/editor"
profile = "editor-desktop"
namespace = "editor"
icon = "../../assets/editor/startup/project-mark-compact.svg"
notices = ["../../LICENSE-MIT", "../../LICENSE-APACHE", "../../docs/third-party.md"]
license-config = "../../about.toml"
```

These relative paths belong to the package directory `apps/kedra`, not your terminal's
working directory. Parent components are allowed here if the resolved input remains inside
the Cargo workspace, including after filesystem links are resolved.

Do not copy the editor's identifier, namespace or notices as an application's own identity
and legal inventory. Choose the appropriate values and review the actual shipped content.

## Field reference

| Setting | Required meaning |
| --- | --- |
| Cargo `package.name` / `package.version` | Application identity recorded in build/package results |
| `packager.productName` | Nonempty display name; no path separators or control characters |
| `packager.identifier` | Nonempty native identifier using ASCII letters, digits, `.` or `-` |
| `packager.version` | Omit it or match Cargo's version exactly |
| `distribution.assets` | Existing source asset directory inside the workspace |
| `distribution.profile` | Processing/delivery profile, such as `editor-desktop` or `game-desktop` |
| `distribution.namespace` | Logical source namespace used for the generated delivery |
| `distribution.icon` | Existing self-contained SVG input |
| `distribution.notices` | List of existing license/notice files inside the workspace |
| `distribution.license-config` | Optional package-relative cargo-about configuration |

All distribution fields except `license-config` are required. Unknown distribution fields
are rejected. Notices are copied by basename into `licenses/`; use distinct filenames to
avoid collisions. The shared tool determines output paths, formats, generated icons and
resource mappings; you cannot supply an alternative assembly pipeline through these fields.

Nonempty `packager.resources` is rejected. Declare supported inputs through `assets` and
`notices`; arbitrary extra configuration/resource files are not currently supported.
Additional AppImage `libs`, `bins`, `files` and linuxdeploy plugins are also rejected before
compilation because their native provenance/source handling is not implemented.

## Prepare sources deliberately

Keep existing `.meta` sidecars with their assets. Build and package process sources but
never implicitly initialize identities. For your own source directory only, explicit
initialization is available:

```sh
cargo kedra assets init assets
```

This authoring operation adds missing sidecars and preserves existing metadata. It is not
a required preparation step for every build and must not be run over checked fixtures
merely to make an example pass. For delivery concepts, see [Assets](../assets/index.md).

## Understand the checked fixtures

These are acceptance fixtures, not application templates or an installed SDK:

- `tests/fixtures/distribution` uses placeholder registry dependency versions for an isolated
  validation copy with explicitly selected local or pinned Git dependencies. Its
  [fixture contract (English)][distribution-fixture] explains preparation and runtime isolation.
- `tests/fixtures/test_project` has no `packager`/`distribution` metadata and is not a direct
  `cargo kedra package` example. Its [fixture contract (English)][native-fixture] and
  [native guide (English)][native-guide] explain the explicit editor/game deliveries and setup.

Editor project validity and distribution configuration are different contracts.

Next: [build and run](build-and-run.md), then [packaging](packaging-and-licenses.md).

[distribution-fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/distribution/README.md
[native-fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
[native-guide]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/native.md
