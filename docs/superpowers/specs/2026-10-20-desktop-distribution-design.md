# Desktop distribution and shared application build design

Status: owner-approved direction, including separate build tooling and the
`cargo kedra` command; implementation plan and actual commit diffs require review.

## Intent and scope

Finish hosted CI acceptance first, then deliver downloadable Kedra editor packages
through GitHub Releases. Establish a build/packaging contract reusable by future
Kedra games instead of writing a second release-only build system.

The owner wants generated projects to inherit compatible engine plugins and
platform settings, with a simple Cargo/xtask-style release command. The owner has
no signing certificates: the first delivery has no trusted Windows/macOS signing
or Apple notarization. These limitations must be visible to downloaders.

This slice delivers editor distribution. It does not implement a game runtime,
Play, game-project scaffolding, plugin discovery, SDK publication or project-code
execution. Those remain future product work with an actual player consumer.

## Current facts

- `apps/kedra` composes the editor and existing import/runtime asset crates.
- Creation currently writes `kedra.toml` and an empty asset directory, not a Rust
  game. Opening a project must remain read-only and must not execute build hooks.
- `cargo xtask` orchestrates checks; it has no distribution command today.
- The existing asset importer builds verified deliveries and retains notices.
- Editor startup uses explicit `--assets`, then `KEDRA_ASSETS`, then `assets` beside
  the executable. It does not currently understand a macOS Resources directory.
- The approved compact Kedra mark exists at
  `assets/editor/startup/project-mark-compact.svg`; its provenance is documented in
  [assets/README.md](../../../assets/README.md). Reuse it without redesign.
- The Winit override lives in the workspace root. Cargo patches do not propagate
  from a dependency into an unrelated consumer workspace.
- Hosted CI and native UI certification are separate acceptance claims.

## Proposed deliverables and targets

The owner approved this direction, amended to use `kedra_build` and `cargo kedra`.

| Platform | Initial architecture | Primary download | Additional download |
| --- | --- | --- | --- |
| Windows | x86_64 MSVC | NSIS installer `.exe` | Portable `.zip` |
| Linux | x86_64 GNU | `.AppImage` | Portable `.tar.gz` |
| macOS | Apple Silicon arm64 | `.dmg` containing `Kedra.app` | None initially |
| macOS | Intel x86_64 | Separate `.dmg` containing `Kedra.app` | None initially |

Use native standard hosted runners: Windows 2025, Ubuntu 24.04, macOS 15 arm64
and macOS 15 Intel. No paid runners or cross-compilation SDK workaround is required.
The builder must validate actual runner/target architecture rather than infer it
from a generic OS label. Universal macOS binaries and Windows/Linux arm64 are
deferred, not advertised as supported.

The initial Linux compatibility floor is Ubuntu 24.04 or a verified compatible
distribution. Record the actual glibc/symbol requirements and external library
requirements; AppImage is not a solution to arbitrary glibc/GPU incompatibility.
Set and document the macOS deployment target after checking Bevy/Winit/dependency
requirements on both architectures. Do not promise older systems without evidence.

A portable package includes all application-owned runtime resources. It does not
promise independence from OS services, graphics drivers or documented system
libraries. Users do not need Rust, Cargo, Python, the source checkout or an SDK.

## One local and hosted build path

Introduce `kedra_build` as a tooling-layer crate, consumed by a thin project-local
launcher. A Cargo alias exposes `cargo kedra build --release` and
`cargo kedra package --target <rust-target>`, with explicit package/output selection.
The editor and an independent application fixture are immediate consumers: this
is not speculative scaffolding. Existing `cargo xtask` remains repository checks.

The launcher pins its build-tool dependency and lockfile. It does not require a
global cargo subcommand installation or inherit a mutable system tool version.
An ordinary dependency does not register Cargo commands; project templates must
explicitly supply the tiny launcher and alias without overwriting existing aliases.
Do not copy packaging implementation into the launcher or use the monorepo xtask
as a library. Build uses Cargo plus asset preparation; package adds native bundling
and artifact verification. The common package operation has these semantics:

1. Validate the requested application, target, version and required native tools.
2. Compile the chosen application using Cargo, the pinned toolchain and lockfile.
3. Process source assets through the existing importer into a fresh delivery.
4. Generate platform icon variants from the approved vector source using maintained
   image tooling, retaining identity, transparency and required resolutions.
5. Stage binaries, resources, metadata, icons and license notices in a fresh output.
6. Ask a maintained packager to produce the selected native formats.
7. Inspect the resulting packages, run appropriate smoke checks and produce a
   machine-readable artifact inventory with SHA-256 hashes and source identity.

CI/CD calls this same command. Shell/YAML must not contain a competing asset-copy,
icon-generation or bundle-layout implementation. Standard tools own archives,
resource formats, directory traversal and installer generation. Kedra-specific
code owns input validation, asset-delivery policy and process orchestration.

Compilation uses Cargo's JSON artifact output through existing cargo_metadata
support, not guesses about executable paths or parsing human compiler output.
Unsupported host/target combinations fail early with the supported alternatives.
Show live subprocess output, progress and a clear summary; no prompts in CI.

An existing output is rejected by default. Do not recursively remove a caller's
directory, reuse a partial delivery as success, or overwrite previous release
assets. A failed build retains diagnostic evidence and does not publish a package.

## Packaging tool choice

Prefer pinned `cargo-packager` as an external build-time CLI. Its supported formats
include app/DMG, NSIS/MSI, AppImage and deb, and its configuration is reusable outside
this monorepo. It supports explicit resource/icon/binary metadata without adopting
a web runtime. Its upstream is active and permissively licensed.

Before adoption, verify the actual released Rust CLI version, license, toolchain
compatibility, transitive security posture, native-tool requirements, download
verification and generated package behavior. An npm release tag is not evidence
of the matching Rust CLI release. Do not add its large library graph to runtime
crates or silently bypass supply-chain checks to install it.

`cargo-dist` is a maintained alternative with strong release orchestration and
archive/installer support. For this slice, standard native GUI bundling and shared
project metadata favor cargo-packager; adding both would duplicate ownership.
Use existing GitHub release APIs/CLI for the small publishing step. If the selected
released packager fails a required format, report the compatibility finding and
revise this choice before inventing an installer or adopting a second framework.

## Platform contracts

### Windows

- The application executable has embedded icon and version/product metadata.
- Use an established Windows resource build dependency; `build.rs` compiles only
  local resources for Windows and declares accurate rerun conditions.
- No resource compiler, SDK or internet download is hidden in build.rs. Missing
  native prerequisites produce actionable errors from the build tooling.
- Launching the GUI application normally does not create an unwanted console.
  Preserve useful CLI error/exit behavior; test both entry paths before choosing
  the Windows subsystem/console handling mechanism.
- The installer installs the real executable, verified delivery, notices and
  shortcuts, supports uninstall and does not require administrator privileges
  unnecessarily. Installer and portable launch must not depend on working directory.

### macOS

- Use a standard `Kedra.app` with executable under `Contents/MacOS`, icons and
  delivered resources under `Contents/Resources`, and a valid `Info.plist`.
- Include bundle identifier, display name, version/build identity, icon reference
  and an accurate minimum OS declaration. Use `dev.kedra.editor` provisionally.
- Resource lookup must understand the standard bundle layout without a launcher
  script, source checkout, environment-variable workaround or duplicate asset copy.
- The DMG contains an application the user can copy to Applications. Verify the
  copied app, not only the staging executable or mounted image.
- No Developer ID identity or notarization is claimed. Toolchain-required local
  ad-hoc signing is allowed without a certificate and is not publisher verification.

### Linux

- Supply an executable, icon, desktop entry, resources and notices using the
  packaging tool's standard AppDir/layout mechanisms.
- Inspect dynamic dependencies and bundle redistributable application dependencies
  where appropriate. Do not copy a runner's entire library tree or bundle GPU drivers.
- Document FUSE requirements and the standard extract-and-run alternative if needed.
  Test from a separate directory without development environment overrides.
- Validate desktop/icon identifiers, executable permissions and filename quoting,
  including paths with spaces and Unicode.

### Shared resource and identity rules

Keep explicit CLI/environment overrides ahead of platform defaults. Never resolve
bundled resources relative to the caller's working directory. Extend an appropriate
existing runtime/app component only when there is a real consumer; keep packaging,
filesystem authoring and importer dependencies out of runtime.

The installed editor can create projects outside its application bundle. Logs,
preferences and user data belong in native user-data locations, not next to an
installed read-only executable. Opening a project remains non-executing.

Generate `.ico`, `.icns` and sized PNGs from the approved mark; do not redraw the
brand. Check icon content and visual appearance in actual platform shells when
hardware is available. Valid metadata alone does not certify icon presentation.

## Future engine/project contract

The eventual engine facade supplies coherent Bevy plugin composition and Cargo
features, not a parallel plugin runtime. A useful default profile enables the
normal supported game stack; optional heavy/platform-specific features remain
explicit and avoid pulling editor/import tooling into the player graph.

A generated game will be an ordinary Cargo workspace with a pinned engine
dependency, entry point, assets and application/package metadata. Required Cargo
patches and platform configuration must be generated explicitly and tested from a
clean independent workspace; dependency-local workspace settings are not inherited.

The package format/configuration and asset contract established here must work
without the editor repository's directory names. Keep editor identity/source paths
as configuration, not constants inside a supposedly generic package algorithm.
Use the packager's maintained metadata rather than inventing a project DSL.

Implement the approved `kedra_build` tooling crate now with the editor and a small
independent Cargo application as real consumers. It must not depend on editor or
xtask, and runtime must not depend on it. Test the second consumer outside the Kedra
workspace, with its own metadata, icon, assets, lockfile and build launcher. This
is packaging acceptance, not a generated game or implementation of gameplay.

The future game launcher pins the shared build-tool version/revision and exposes
the same `cargo kedra` alias. Keep tooling out of normal game runtime dependencies
and avoid pulling a full bundler/importer into build.rs. A lightweight Windows
resource helper can be a build dependency where required. A globally installed
`cargo-kedra` binary is optional future convenience, not the required bootstrap.
The editor and game then consume the same implementation.
UI Build/Export is a later frontend to that operation, not a third implementation.
SDK distribution, tool bootstrap/update and reproducibility for those independent
projects need their own consumer-backed acceptance before being advertised.

Record the approved ownership/layout/tooling decision in `docs/decisions/` during
implementation. This proposal does not itself authorize the future engine work.

## CD workflow and publication boundary

Complete current CI first: all required jobs green, fresh coverage at least 80%,
cold/warm cache evidence and enforced main protection. Do not lower those checks
to unblock publishing. Keep current actual limitations visible in CI documentation.

Use a separate manual release workflow for the initial rollout. Validate an
explicit version/tag and exact reviewed main commit. Require successful CI for
that exact commit, not the latest green run from some other revision. Reject a
tag/version mismatch, an unrelated commit and an existing conflicting release.

All compile/package/test jobs have read-only permissions. Build all four targets
independently and require the entire artifact matrix before publication. Keep
release compilation caches separate from native-test/coverage roles; cache only
dependencies and never reuse cached test results or source-specific bundles.
Measure cache size within the existing free allowance before claiming reuse.

Only the final publisher gets contents-write permission. It downloads this run's
validated artifacts and inventory without checking out/executing project build
code. It creates a draft release, uploads exactly the expected files and hashes,
then publishes only after upload/identity verification. Partial/mismatched output
does not become a public completed release. No `pull_request_target`, privileged
workflow_run checkout, signing secrets, paid services or automatic main releases.

Proposed first release: workspace version `0.1.0`, tag `v0.1.0`, explicitly marked
GitHub prerelease. This does not require claiming stable engine APIs or changing
the version of every workspace dependency. Owner approval of this design must also
approve creating that initial tag/prerelease; later releases remain deliberate.

Release notes list each OS/architecture, install instructions, actual system
requirements, source commit, checksum verification and known limitations.
Unsigned Windows/macOS warnings are disclosed. Never recommend disabling global
OS protections, disabling antivirus or claiming SHA-256 authenticates the publisher.
Include project and third-party notices, including font licenses and Winit changes.

## Verification and acceptance

Regression tests cover failures, not snapshots of YAML values. Use maintained
validators for native metadata/package formats and actionlint/zizmor for workflows.

- Unit/integration cases: invalid target/version, missing binary/resource/tool,
  failed subprocess, stale/nonempty output, spaces/Unicode paths and asset
  verification failures; no partial result reported as success.
- Build and inspect each actual native package. Check architecture, identifiers,
  version, icons, resource inventory, notices and absence of development/private data.
- Unpack/install in a fresh location; verify delivered assets and CLI diagnostics
  with no source checkout, overridden asset variable or Rust on the runtime PATH.
- Exercise installer/uninstaller and copied macOS app, not just `cargo run`.
- Native UI evidence requires a real window, input and captured rendered frame.
  Reuse existing native fixtures when the host supports them; headless verification
  and RPC alone are not native interaction acceptance. Record unavailable hardware
  as unverified, never as passed. A preview may explicitly retain those limits;
  a package that demonstrably fails startup must not be published as working.
- Check OS integration/icon appearance on available physical hardware. Hosted
  process smoke checks do not certify Gatekeeper, SmartScreen, DPI or GPU behavior.
- Exercise failed/missing package jobs and publication preflight: no completed
  release on failed CI, wrong SHA, wrong version, incomplete matrix or hash mismatch.
- Download the published files anonymously and compare hashes against this run's
  validated inventory; inspect release visibility, tag SHA and prerelease status.

Run `cargo xtask check`, relevant feature/native/package tests, dependency/security
checks, Markdown/links and actual hosted workflows on the final reviewed revision.
Every product-code bug fix receives a regression test, including its failure path.

## Implementation order after approval

1. Finish current CI acceptance and main protection; record measured limitations.
2. Validate pinned packaging/resource tooling against small real native outputs.
3. Implement `kedra_build`, the project-local `cargo kedra` launcher, resources/icons
   and platform metadata with regression tests, reusing existing asset processing.
4. Verify the editor and an independent fixture on Windows, Linux and both macOS
   architectures using the same `cargo kedra` commands.
5. Add read-only build matrix and narrowly privileged publication workflow.
6. Obtain required owner diff approval before commits; publish the approved first
   prerelease, download/verify it and update factual distribution documentation.

## Research references

- [cargo-packager](https://github.com/crabnebula-dev/cargo-packager): native GUI
  package formats, CLI/library separation, standard configuration and licenses.
- [cargo-dist](https://github.com/axodotdev/cargo-dist): maintained alternative for
  artifact/release orchestration; not adopted alongside another orchestrator.
- [GitHub hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners):
  standard arm64/Intel macOS and native Windows/Linux runner labels/resources.
