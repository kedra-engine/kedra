# Desktop application distribution

The public `v0.1.0` prerelease has a known macOS bundle-signature defect; local
re-signing is required for that asset. New macOS packages default to complete
ad-hoc bundle signing, not a linker-only executable signature. They are still not
Developer ID-signed or notarized; Windows packages remain unsigned. Signature
integrity checks do not certify Gatekeeper trust, physical input, OS integration
or rendered UI. Native interaction and frame acceptance remain separate requirements.

## Shared application tooling

`kedra_build` is tooling-only, independent of editor/xtask. A project-local
launcher forwards `std::env::args_os()` unchanged to `kedra_build::run_cli`.
The Cargo alias isolates the running executable in `target/kedra-build-bootstrap`,
so workspace builds cannot overwrite it on Windows. `cargo xtask` remains
repository validation; application tooling does not replace that alias.

```sh
cargo kedra build --release --package kedra --output target/editor-build
cargo kedra run --package kedra --output target/editor-run -- --help
cargo kedra package --package kedra --target aarch64-apple-darwin --output target/editor-package
```

Outputs must be new directories. Failure retains partial evidence and never
reports an inventory as a successful distribution. Without `--output`, `build`
reserves a fresh directory under Cargo's target directory. Neither default nor
explicit output may be inside source assets. Packaging always builds release.
Cargo executes project build scripts: use these commands only on trusted projects.
Only explicit `run` launches the selected application after a successful build
and verification. `build` and `package` do not execute application CLI/gameplay.

`build` produces an ordinary signed `.app` on macOS without requiring a DMG;
Windows/Linux produce an executable with adjacent assets and notices. A version-1
`build-result.json` records application/version/target and output-relative
`application_root`, `executable` and `resources` locations. It is written only
after validation; it is a result handoff, not project configuration.

`run` uses that same invocation's verified result, never an older output. Arguments
after `--` retain their exact boundaries and Unicode. Launch does not use source
assets or the source working directory, and does not inject `KEDRA_ASSETS`.
Windows/Linux direct children have closed stdin and persistent
`application-stdout.log`/`application-stderr.log` in the output. macOS uses Launch
Services to open the actual bundle. Closing the launcher does not intentionally
terminate the app. Launch acceptance is distinct from runtime exit, GUI readiness
or physical input; dispatch failure leaves the verified application on disk.

Editor packages use Cargo's release profile with `opt-level = 3`,
`codegen-units = 1` and `lto = "thin"`, following Bevy's optional
[release-mode recommendations](https://bevy.org/learn/quick-start/getting-started/setup/#compile-with-performance-optimizations).
This deliberately costs compilation time for runtime optimization; it does not
enable development-only dynamic linking or change debug/test profiles. The shared
builder invokes release packaging but leaves independent applications' profile
policy in their own workspace Cargo.toml rather than overriding it globally.

Supported native hosts/targets are Windows x64 MSVC, Linux x64 GNU, macOS ARM64
and macOS Intel. Cross-platform SDK installation is not provided. Native builds
need the normal [platform prerequisites](../CONTRIBUTING.md). Windows package
inspection additionally requires maintained 7-Zip (`7z` on PATH). NSIS contents
are extracted without executing the installer: a temporary install destination
does not isolate registry, shortcuts or application-termination side effects.
macOS DMG creation
uses system disk-image tools; Windows installers use upstream NSIS helpers;
Linux AppImage uses upstream linuxdeploy/AppRun tools. The reviewed packager
downloads helpers to its OS cache. Some helper downloads are not cryptographically
pinned upstream; this is not a hermetic build claim.

The shared builder retains macOS Intel support for other applications. Kedra's
editor CI/CD selects only Windows x64, Linux x64 and macOS Apple Silicon; it does
not build or publish an Intel macOS editor package.

Linux packaging additionally requires Debian/Ubuntu `dpkg-query`, `pax-utils`
(`lddtree`), `tar`, and the distro copyright/common-license directories. The
currently accepted GNU Linux distribution host is Ubuntu 24.04. Missing native
ownership, notices, source downloads or unreviewed bundled libraries fail packaging;
other distributions need an explicit provenance adapter rather than guessed notices.
Docker on native x86-64 Linux is required for the separate recipient-relink test,
not for the ordinary packaging command. CI runs that test explicitly.

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
Nonempty `packager.resources` is currently rejected during preflight, rather than
silently omitted. Put supported delivery inputs in `distribution.assets` and
license inputs in `distribution.notices`; arbitrary additional native/portable
resource mappings are not implemented.
Additional AppImage `libs`, `bins`, `files` and linuxdeploy plugins are also
rejected before compilation: their native provenance/source handling is not
implemented. Library exclusions remain packager configuration.

`[package.metadata.kedra.distribution]` contains `assets`, `profile`, `namespace`,
`icon` and `notices`. Paths resolve from the application's package directory
and must stay inside its Cargo workspace. `assets` is an existing source directory;
`icon` is a self-contained SVG; `notices` lists existing license files. Preparation
reuses the editor's importer and runtime verifier. Build/package never implicitly
initialize or replace source identities.

Applications can also configure `license-config`, a package-relative cargo-about
configuration. Build and packaging then require cargo-about 0.9.2, collect native normal
dependency licenses with failure on unresolved licenses, render full copyright/
license notices, and bundle checksum-verified original `.crate` source archives
for MPL-covered crates.io dependencies. Unknown non-registry covered-source
provenance fails rather than silently omitting sources. Local build paths are not
published in the notice HTML. This is additional compliance evidence, not a
replacement for cargo-deny's dependency-admission policy or legal review.

Rust notices alone do not cover native ELF libraries, AppRun or the outer
AppImage runtime. Shared tooling uses `lddtree` and `dpkg-query` to preserve full
distro copyright records, package/source versions and referenced common licenses
under `licenses/native-libraries`. It inspects the actual generated AppImage and
rejects bundled ELF libraries without a unique recorded owner. Identity binds
loaded code/data/notes, resolved dynamic symbols and needed-library names to the
original distro input, accounting for normal patchelf relocation metadata through
the maintained ELF parser. Inspection covers the entire AppDir; only the
individually verified application, AppRun and source-kit runtime are exempt from
distro-library matching. Installed source/notice files must match verified staging,
not merely exist. Native library admission is restricted to reviewed permissive
Brotli, bzip2, libffi, libpng and
Wayland copyright records: new source families or changed notice hashes need review,
not an automatic license exception. Collecting a notice does not automatically
authorize redistributing a library with additional source obligations.

The runtime is selected through linuxdeploy's supported `LDAI_RUNTIME_FILE`.
Its actual image prefix (except appimagetool's standard 16-byte `.digest_md5`
field) and the bundled AppRun are checked against the reviewed
inputs. `licenses/appimage-runtime` includes checksum-pinned runtime/libfuse/
squashfuse sources, the upstream libfuse patch, full native license notices,
Alpine static objects/headers and recipient rebuild instructions. These materials
are embedded in both Linux packages, not just linked from a changing upstream
branch. Moving upstream downloads that no longer match fail closed. The supplied
standard Docker/Meson/Make relink path is tested with modified LGPL library code;
it does not claim hermetic or bit-identical compilation. This extension remains
under native acceptance until the actual hosted relink and packaged-input checks
have passed; no release/license-completeness claim follows from its source alone.
GCC startup/runtime license and exception notices are also included. Standard
compiler startup objects are acquired through the recipient's normal toolchain,
not falsely represented as supplied by the static-library APKs. Native relink
checks retain an actual linker input map and toolchain/package inventory.

The released collector's frozen TLS graph has current advisories; its library's
krates dependency also pins indexmap/petgraph versions incompatible with this
workspace. Do not downgrade project dependencies or patch upstream to force it in.
The explicit shared installer uses checksum-pinned upstream source, refreshes and audits
an isolated tool lockfile, then installs that exact graph with `--locked`:

```sh
cargo kedra tools install-about --output /OUTSIDE_CARGO_WORKSPACE/about-tool
```

Put the resulting `bin` directory on PATH before packaging. CI uses `RUNNER_TEMP`
and retains the resolved tool lock as evidence. Source/lock/build stay outside the
application graph; they are not another tracked lockfile or wrapper crate. The
tool's security updates are resolved at installation time, so its graph is not
claimed identical across installation dates. Each installation is audited before
compiling, and existing tool directories are never overwritten.

The installer is part of `kedra_build`, not a Python repository script. It requires
Cargo and cargo-deny for the advisory audit. Setup is explicit: builds never
download/install a missing collector or system SDK automatically. Native tools
and configured collectors are checked before expensive application compilation.
Help/version do not require a project or packaging prerequisites.

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
`kedra assets verify` without a path verifies that same default delivery without
opening a window; an explicit verification path remains supported. Native package
inspection invokes this command without overrides from an unrelated directory.
Linux inspection runs extracted `AppRun`, not just the inner ELF executable.

The shared builder validates executable format/architecture, delivery checksums,
configured notices and macOS bundle identity/signature. Packaging inspects actual
DMG/NSIS/ZIP/tar/AppImage contents before writing its successful inventory. Native
Linux library/runtime/source checks remain part of package creation. These checks
use each application's metadata and never require editor-specific CLI commands.
The separate `editor_package_smoke.py` is an editor acceptance test: it extracts
packages again to exercise the editor's installed CLI/resource lookup, not to
finish or repair a build. Other applications do not need that test driver.
Its Windows install/uninstall acceptance requires the explicit
`--disposable-windows-host` option and is intended only for disposable CI runners,
not a developer machine with an existing application installation.

## Outputs and identity

- Windows x64: NSIS installer `.exe` and portable `.zip`.
- Linux x64: `.AppImage` and portable `.tar.gz`.
- macOS editor: ARM64 `.dmg` for Apple Silicon, containing an ordinary `.app`.
  The shared builder can also produce Intel DMGs for other applications.

The maintained packager signs macOS nested code and the completed bundle
inside-out, using an explicitly configured identity or ad-hoc `-` by default.
All resources and `Info.plist` must be sealed before creating the DMG. Inspection
copies the app from the actual disk image and runs
`codesign --verify --deep --strict --verbose=2` before executing its CLI. A valid
ad-hoc signature is not an Apple-trusted identity or notarization; users may still
need the normal macOS approval flow. No global Gatekeeper settings are changed.

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
python3 -X utf8 tools/distribution/validate_fixture.py --package
python3 -X utf8 tools/distribution/validate_fixture.py --revision EXACT_PUBLIC_GIT_SHA --package
```

It explicitly selects local or pinned public Git dependencies, preserves the
xtask alias, generates icons/sidecars, invokes the same build/package command and
runs the staged runtime consumer. It rejects editor/import/tool dependencies in
the application's normal graph. A local-path run does not prove public Git works.
It also exercises `cargo kedra run` and waits for an application-owned launch
marker after the independently launched runtime verifies its delivery.

## GitHub release boundary

[Desktop distribution](../.github/workflows/release.yml) is manually dispatched
from protected `main` with an exact source SHA and a new Cargo-matching tag.
Preflight requires protected-main ancestry, genuine successful repository push CI
at that exact SHA, and the current GitHub Actions `CI required` result. Existing
tags/releases are refused rather than overwritten.
The source's `.github/workflows` tree must also match the current default branch:
GitHub's release API can require workflow-write permission for other targets,
which the ordinary Actions token cannot acquire. This is checked before building
and again at publication, without introducing a personal token or widening access.

Three standard native runners use the same command and independent pinned Git
consumer, with dependency-only caches separated from validation/coverage roles.
The target-independent application dependency audit runs once on Linux, not once
per package OS. All runners still install cargo-deny because the isolated license
collector has its own freshly resolved tool graph to audit before compilation.
Generic architecture, signature and packaging regressions live with `kedra_build`
and run in native Rust CI. Python retains editor-specific smoke regressions. Source-kit
preparation and modified-LGPL-library relink regressions run in native Linux CI
when packaging inputs change, rather than repeating them during every release.
Actual package inspection and the independent public Git consumer still run on
each native target: these verify different artifacts and installed behavior.
Native inspection mounts/unpacks actual distributions and exercises installed
CLI/resources/notices. Physical-input, OS-trust and GUI acceptance are separate.
Cargo dependency caches are restricted to `target/distribution-cargo` and the
isolated launcher target. Final packages and independent consumer workspaces live
under the runner's temporary directory, outside those caches. Old package outputs
are never deleted/reused to make a warm build pass; existing outputs remain errors.
Build/inspection jobs have read-only repository permissions. Package artifacts
are retained one day, bounded diagnostic/tool-lock evidence three days.
Python tooling runs in the standard UTF-8 mode, including on Windows; Unicode
paths are not removed or replaced to accommodate legacy console code pages.
macOS architecture checks use the native `lipo` input-first command form and test
both a matching and a mismatching architecture against an actual compiled binary.

`publish` defaults to false, permitting actual build-only acceptance first. Only
the final publisher receives `contents: write`. It does not checkout or build
application sources: it loads only narrow policy files from the immutable main
workflow SHA, requires the scheduler's successful native matrix result, consumes
this run's artifacts and validates all five package sizes/hashes/source identities. It creates
a new draft, checks the complete uploaded set and GitHub digests, binds the tag
to the reviewed source, and only then publishes an explicitly unsigned prerelease.
An upload/policy failure leaves an unpublished draft; reruns never replace it.
Draft creation uses the REST response's release ID for subsequent inspection and
publication. GitHub's by-tag release endpoint is used only after publication;
it is documented for published releases, not draft discovery. Asset transport
remains the maintained GitHub CLI, with verified server sizes/digests before the
ID-based update clears `draft`.

The unsigned `v0.1.0` prerelease is publicly available. Its native package matrix
passed, and publication was completed through an authorized manual recovery.
The corrected automatic draft-to-published path still needs hosted acceptance;
manual recovery is not evidence that the automated publisher succeeded.
See [ADR 0023](decisions/0023-desktop-distribution.md) for ownership/security and
license obligations.
