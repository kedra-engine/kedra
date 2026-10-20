# Desktop application distribution

Native editor distribution is under implementation. There is no verified public
release yet. Package/process checks do not certify physical input, OS integration
or all rendered UI states. Current packages are unsigned and not notarized.
Native renderer acceptance remains unverified on the local macOS host: its desktop
session is locked, and both installed-editor and independent pure-Bevy control
captures are black. Resource/process checks cannot substitute for unlocked-screen
interaction and frame acceptance. This is not a confirmed product-rendering fix.

## Shared application tooling

`kedra_build` is tooling-only, independent of editor/xtask. A project-local
launcher forwards `std::env::args_os()` unchanged to `kedra_build::run_cli`.
The Cargo alias isolates the running executable in `target/kedra-build-bootstrap`,
so workspace builds cannot overwrite it on Windows. `cargo xtask` remains
repository validation; application tooling does not replace that alias.

```sh
cargo kedra build --release --package kedra --output target/editor-build
cargo kedra package --package kedra --target aarch64-apple-darwin --output target/editor-package
```

Outputs must be new directories. Failure retains partial evidence and never
reports an inventory as a successful distribution. Without `--output`, `build`
reserves a fresh directory under Cargo's target directory. Neither default nor
explicit output may be inside source assets. Packaging always builds release.
Cargo executes project build scripts: use these commands only on trusted projects.
They do not launch the editor or run project gameplay.

Supported native hosts/targets are Windows x64 MSVC, Linux x64 GNU, macOS ARM64
and macOS Intel. Cross-platform SDK installation is not provided. Native builds
need the normal [platform prerequisites](../CONTRIBUTING.md). macOS DMG creation
uses system disk-image tools; Windows installers use upstream NSIS helpers;
Linux AppImage uses upstream linuxdeploy/AppRun tools. The reviewed packager
downloads helpers to its OS cache. Some helper downloads are not cryptographically
pinned upstream; this is not a hermetic build claim.

Native packaging runs in a self-reexecuted launcher child, from the application's
manifest directory. Relative native configuration belongs to the selected app.
Only this child gets unattended `CI=true` mode, avoiding Finder AppleScript for
DMGs. The caller's environment and working directory remain unchanged. Backend
failure, missing reports and malformed reports fail the command.

## Application metadata and resources

Cargo owns package name/version and binary identity. Applications must have one
binary target and standard `[package.metadata.packager]` with `productName` and
`identifier`. A native version must match Cargo or be omitted. The shared tool
owns outputs, formats, generated icons and verified resource mappings.

`[package.metadata.kedra.distribution]` contains `assets`, `profile`, `namespace`,
`icon` and `notices`. Paths resolve from the application's package directory
and must stay inside its Cargo workspace. `assets` is an existing source directory;
`icon` is a self-contained SVG; `notices` lists existing license files. Preparation
reuses the editor's importer and runtime verifier. Build/package never implicitly
initialize or replace source identities.

```sh
cargo kedra assets init assets
cargo kedra icons icon.svg --output target/native-icons
```

Initialization explicitly adds missing sidecars without replacing existing ones.
Icons require a new output directory and produce PNG sizes 16–1024, Windows ICO
and a macOS ICNS family including 2x densities. Ordinary Windows Cargo builds use
the app's checked-in icon; packaging supplies generated inputs through child build
environment. Build hooks compile local resources only.

Installed resources live in macOS `Contents/Resources/assets`, Linux
`usr/lib/<binary>/assets` for `usr/bin` layouts, or beside portable executables.
Lookup never searches the launch directory or source checkout. Editor `--assets`
takes precedence over `KEDRA_ASSETS`, then the executable-relative default.

## Outputs and identity

- Windows x64: NSIS installer `.exe` and portable `.zip`.
- Linux x64: `.AppImage` and portable `.tar.gz`.
- macOS: separate ARM64 and Intel `.dmg`, containing an ordinary `.app`.

`artifact-inventory.json` and `SHA256SUMS` bind application/version, target,
basenames, sizes and SHA-256 hashes. A clean Git workspace root supplies its exact
source revision. Dirty/non-Git builds have `source_revision: null` and are not
eligible for publication. Checksums provide integrity, not publisher authentication.

The editor specifies macOS 15. Linux glibc/GPU dependencies, Windows OS floor,
installation/uninstallation and each architecture still require native acceptance.
AppImage does not make graphics drivers, glibc or FUSE universally compatible.
Do not disable OS protections globally to run unsigned software.

## Independent consumer acceptance

`tests/fixtures/distribution` is a separate Cargo application, not a game runtime
or published template. The helper copies it into an isolated workspace with its
own lockfile and project-local launcher:

```sh
python3 tools/distribution/validate_fixture.py --package
python3 tools/distribution/validate_fixture.py --revision EXACT_PUBLIC_GIT_SHA --package
```

It explicitly selects local or pinned public Git dependencies, preserves the
xtask alias, generates icons/sidecars, invokes the same build/package command and
runs the staged runtime consumer. It rejects editor/import/tool dependencies in
the application's normal graph. A local-path run does not prove public Git works.

See [ADR 0023](decisions/0023-desktop-distribution.md) for ownership, security and
license obligations. Native release automation and public download acceptance
remain incomplete; do not infer them from a local package.
