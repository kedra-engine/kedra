# ADR 0025: Verified standalone application builds and launch

- Status: implemented CLI contract; native cross-platform acceptance pending
- Date: 2026-10-21
- Extends: [ADR 0023](0023-desktop-distribution.md)

## Context

Applications must be buildable through their own pinned `kedra_build` launcher
without an installed Kedra editor or a copy of the editor repository's scripts.
The editor, terminal and CI must not have different recipes for producing a
correct application. Shared `kedra_build` owns runnable assembly, generic
installed-package verification and the explicit audited license-collector installer.

The first supported workflow is build and launch of a standalone desktop
application. It is not in-editor Play, a game runtime or source export. Mobile
APK/IPA outputs are future work and do not authorize mobile exporters now.

## Decision

### Ownership and commands

`kedra_build` owns preparation, Cargo compilation, runnable application assembly,
configured licensing, signing, generic validity checks and explicit launch.
Cargo, the existing asset pipeline and maintained cargo-packager retain their
current responsibilities; do not introduce another build language or framework.

- `cargo kedra build` produces a verified runnable desktop application.
- `cargo kedra run` performs that same build and verification, then requests
  launch of exactly that result. It accepts application arguments after `--`.
- `cargo kedra package` builds release artifacts and validates their actual
  distributed contents before writing a successful artifact inventory.
- Publishing remains separate, with repository policy and GitHub credentials
  outside ordinary application builds.

Existing selection, native-target, release and fresh-output options remain.
Builds do not overwrite previous outputs or initialize source identities.
Failure preserves partial evidence, does not publish a successful result, and
never falls back to launching an older application.

### Runnable result and launch

Windows and Linux build results contain an executable with adjacent verified
assets and configured notices. macOS produces an ordinary `.app`, including
Info.plist, resources, icons and complete inside-out bundle signing. Creating
a runnable `.app` must not require creating or mounting a DMG.

The build result records application/version, target and output-relative launch
and resource locations in a versioned `build-result.json`. A result is written
only after verification. It is a client handoff, not a second project settings
format; Cargo metadata remains the source of settings.

Launch is explicit, separate from validation and independent of the editor's
lifetime. It does not modify source documents or implicitly save anything.
Use platform launch facilities on macOS and safe standard process APIs on
Windows/Linux. Preserve argument boundaries and Unicode; never construct a shell
command from application arguments. Do not inject the editor's asset override.
Use an output-directory working directory for direct children, not the source
checkout, and give them persistent stdout/stderr logs and closed stdin. macOS
Launch Services owns the launched bundle's working directory and diagnostics;
application resource lookup must remain executable-relative on every platform.

Report launch-request acceptance separately from build success. Acceptance is
not evidence of rendered frames, physical interaction, eventual exit success or
OS trust. Spawn/dispatch failure returns failure without discarding the verified
build. This slice is not a debugger, process supervisor or crash monitor.

### Verification boundary

Generic verification belongs to `kedra_build` and uses the selected application's
identity, target, binary and resource metadata, never `Kedra.app`, a fixed bundle
identifier or mandatory application CLI commands.

Verify executable architecture/format, delivery integrity, configured notices,
resource placement and macOS bundle identity/signature. Verify actual package
contents, not only input staging. Retain Linux native provenance, AppImage runtime
and source-kit checks. Use maintained archive/object libraries and system platform
tools, not new parsers or filesystem traversal frameworks.

Windows package verification uses maintained 7-Zip to extract NSIS contents;
it never executes the installer or uninstaller. A temporary `/D=` does not isolate
registry, shortcuts, upgrade handling or process termination. Actual Windows
installation acceptance belongs to explicit tests on disposable native runners,
not an ordinary developer-machine build. Reject DLLs/dylibs as applications;
Linux PIE binaries may use the ELF dynamic type with a real entry point.

Application-specific smoke checks, such as the editor's `assets verify` command,
font expectations, rendered-frame tests and interaction tests, remain consumer
tests. CI invokes the same build/package commands and additionally performs those
checks, coverage, supply-chain admission and release policy. It does not repair
or supply missing generic package validation after a successful build.

### Prerequisites and licensing

Use the native host's installed Rust/toolchain and platform prerequisites; do not
install system SDKs, silently elevate privileges or broaden permissions. Check
required tools before expensive compilation and provide actionable diagnostics.
Only check tools needed by the selected operation: a macOS build needs signing
tools, not disk-image tools; help/version need neither.

Expose the existing isolated, checksum-pinned, advisory-audited cargo-about setup
through an explicit `cargo kedra tools install-about --output PATH` command.
Reuse maintained download/archive tools and normal Cargo commands. No implicit
network installation during builds, global installation or audit bypass is added.
Its source, lock and build stay outside application workspaces. A foreign project
can perform this setup without Python or editor-repository scripts. Repository CI
may retain a thin adapter for its PATH handoff, but no duplicate installation logic.

Configured license collection applies to the runnable result as well as packages.
Failure to collect required notices is failure, not a warning. Native Linux
redistribution obligations remain format-specific and are not silently weakened
for portable packages.

## Alternatives and limits

Keeping generic checks in CI makes local success weaker than CI success and
forces other applications to copy repository scripts. Embedding the builder in
the editor duplicates process ownership and introduces heavy tooling dependencies.
Instead, future editor integration invokes the project-local CLI as a child.
No editor UI is added by this slice.

No new public Rust API, crate, generic hook/plugin system or runtime dependency is
needed. Reuse the locked object/archive/network dependencies; any feature changes
must be justified and audited. Ad-hoc signing preserves integrity, not Developer
ID trust or notarization. Native Intel execution and unsupported host acceptance
must not be inferred from compilation on another machine.

## Acceptance

An isolated application with its own identity, assets, launcher and no editor
dependency can build, verify and launch from an unrelated directory using only
the shared CLI and documented prerequisites. Its ordinary runtime graph contains
no editor/import/build tooling. It need not implement any Kedra CLI commands.

Regression coverage includes malformed/tampered resources, wrong architecture or
identity, missing prerequisites, failed compilation, failed verification, launch
failure, existing outputs, argument forwarding and Unicode paths. Failed build or
verification must not produce a launch side effect. Actual installed packages
are checked on native Windows, Linux and macOS runners; physical GUI interaction
remains a separate acceptance requirement.

Keep ordinary CI economical: unit/failure-path tests and dependency-free debug
fixtures do not build optimized editor releases. Full native build/package/launch
acceptance runs during explicit release validation. Cargo profile values are
configuration, not a reason to add another release compilation to the test suite.
