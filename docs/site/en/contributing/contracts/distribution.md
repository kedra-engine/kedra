# Application distribution contract

`kedra_build` assembles applications, inspects native packages and supports release orchestration
without depending on a particular application. For commands, prerequisites and configuration,
use [Build and run](../../manual/build-and-distribution/build-and-run.md),
[Packaging and licenses](../../manual/build-and-distribution/packaging-and-licenses.md),
[Application configuration](../../manual/build-and-distribution/configuration.md) and the
[builder reference](../../manual/cli/build-tool.md). These mechanisms do not implement an installed
game SDK, application template, project execution in the editor or Play.

## Owners and trust boundary

| Owner | Contract |
| --- | --- |
| Project-local launcher | Forward `std::env::args_os()` unchanged to `kedra_build::run_cli`; isolate the running tool in `target/kedra-build-bootstrap` so Windows builds cannot overwrite it. |
| Cargo / application | Own package name, version, one binary target, native/distribution metadata and reviewed `Cargo.lock`. Independent applications retain their own profile policy. |
| `kedra_build` | Preflight, fresh staging, compilation orchestration, runnable assembly, generic verification, explicit launch and package inventory. |
| `kedra_import` / `kedra_assets` | Prepare sources and verify deliveries using the same mechanisms as the editor; no implicit identity initialization. |
| Maintained cargo-packager | Native bundles/formats and platform helpers; not a custom installer implementation. |
| Editor acceptance / release CI | Installed editor behavior, native machines, credentials and publication; never missing assembly steps. |

The tooling crate is independent of editor/xtask; runtime must not depend on build/import tooling.
`cargo xtask` remains repository validation, not an application builder. Cargo build scripts and
downloaded native helpers execute code: a selected project must be trusted. `build` and `package`
do not execute application CLI/gameplay. Only explicit `run` dispatches the application after
verification. Package inspection may execute AppImage extraction, not application gameplay.

The locked packager is upstream commit `5c1824ba8022401e9ef0de36ff1a12c5acf60237`
(package version `0.11.8`), with CLI features disabled. Its OS-cache helper downloads are not all
cryptographically pinned upstream; the build is not hermetic. See
[ADR 0023 (English)][adr23] for dependency rationale and security ownership.

## Metadata and preparation invariants

Cargo metadata selects exactly one application. `productName` and `identifier` are required;
native version matches Cargo or is omitted. Distribution inputs are package-relative and resolve
inside its Cargo workspace, including through filesystem links. `assets`, `profile`, `namespace`,
`icon` and `notices` are required; `license-config` is optional. The Manual owns field syntax.
The shared builder owns outputs, formats, generated icons and verified resource mappings.
Nonempty `packager.resources` and additional AppImage `libs`, `bins`, `files` or linuxdeploy
plugins fail before compilation: arbitrary resource/provenance handling is not implemented.
Library exclusions remain native packager configuration.

Build/package never initialize or replace source identities. Explicit `assets init` only adds
missing sidecars. Native prefab/scene references use full source-qualified logical paths, such
as `editor://prefabs/button.prefab.ron`, resolved through the loaded source catalog with membership
and published-revision checks; never hardcoded UUIDs or a parallel registry. Persisted UUIDs,
sidecars and provenance retain their identity; logical asset paths are not filesystem locators.

Icons come from a self-contained SVG, producing PNG sizes 16–1024, ICO and an ICNS family with
2x densities. Ordinary Windows Cargo builds use the checked-in app icon; packaging supplies
generated inputs through the child build environment. Build hooks compile local resources only.

This workspace's release profile uses `opt-level = 3`, `codegen-units = 1` and `lto = "thin"`,
following [Bevy's release recommendations][bevy-release]. This trades compilation time for runtime
optimization without enabling development dynamic linking or changing debug/test profiles.
The shared builder requests release for packaging, not profile overrides in independent projects.

## Fresh assembly and result ownership

Preflight checks native tools and any configured collector before expensive compilation; it
does not install SDKs or missing collectors. Help/version need neither a project nor these tools.
Build/run default output reserves a fresh directory under Cargo's target directory; explicit
outputs must also be new. Neither may be inside source assets. No previous output is deleted
or reused. Failures retain partial evidence and must not be treated as successful distributions.

Assembly generates icons, builds/verifies assets, stages notices and optional dependency sources,
then compiles the selected binary with `cargo build --locked`. Cargo artifact messages identify
the executable; paths are not guessed. macOS gets an ordinary complete signed `.app` without
requiring a DMG. Windows/Linux get an executable with adjacent assets/notices.

Version-1 `build-result.json` records `application`, `version`, `target`, `application_root`,
`executable` and `resources`; the three locations are output-relative. Canonical membership
checks reject traversal and links escaping the reserved output. The result is a verified handoff,
not configuration or an input for reusing an old build. It is written after validation; package
invocations also finish package inspection before publishing that handoff.

Verification checks runnable format/architecture, executable permission where applicable,
delivery checksums and installed notices against staging. Linux checks unresolved dynamic
libraries; macOS checks bundle identity and signature. Generic checks use application metadata,
not editor CLI commands. `artifact-inventory.json` and `SHA256SUMS` follow actual package
inspection, binding application/version/target, basenames, sizes and SHA-256 hashes. A clean Git
root matching the Cargo workspace supplies its exact revision; dirty, nested-workspace and
non-Git builds record `source_revision: null` and cannot be published. Hashes prove integrity,
not publisher authentication. File presence alone is not success.

## Explicit launch lifecycle

`run` consumes this invocation's verified result only. Arguments after `--` preserve exact
boundaries and Unicode. The launch directory is the reserved output, not the source checkout;
the launcher removes inherited `KEDRA_ASSETS` and injects no source override.
Windows/Linux direct children have closed stdin and persistent `application-stdout.log` /
`application-stderr.log` in that output. macOS Launch Services opens the actual bundle.
Closing the launcher does not intentionally terminate the application. Dispatch failure leaves
the verified build on disk. Dispatch acceptance proves neither runtime exit, GUI readiness,
rendered frames nor physical input; readiness belongs to the application.

## Native packaging and installed resources

Supported native hosts/targets are Windows x64 MSVC, Linux x64 GNU and macOS ARM64/Intel;
there is no cross-platform SDK bootstrap. The editor release matrix selects only Windows x64,
Linux x64 and Apple Silicon, not Intel editor packages. Formats are NSIS `.exe` plus portable
`.zip`, AppImage plus portable `.tar.gz`, and `.dmg` containing `.app`, respectively.

Native packaging self-reexecutes the launcher from the application's manifest directory.
Relative native settings therefore belong to that application. Only the child gets `CI=true`
to avoid Finder AppleScript during DMG creation; caller environment/CWD remain unchanged.
Backend failure, missing reports or malformed reports fail the command.

DMGs use system disk-image tools; NSIS and Linux use upstream NSIS/linuxdeploy/AppRun helpers.
Windows inspection needs maintained `7z` on PATH and extracts NSIS without executing it.
A temporary install destination does not isolate registry, shortcut or process-termination
effects. Actual install/uninstall acceptance is editor-owned and requires
`--disposable-windows-host` on a disposable runner, never an existing developer installation.

Installed assets live in macOS `Contents/Resources/assets`, Linux
`usr/lib/<binary>/assets` for `usr/bin` layouts, or beside portable executables.
Lookup must not search launch CWD or the checkout. The editor prioritizes `--assets`, then
`KEDRA_ASSETS`, then its executable-relative default. `kedra assets verify` without a path
checks that default without opening a window; explicit verification paths remain supported.
The separate `editor_package_smoke.py` re-extracts real packages and invokes installed CLI
without overrides from an unrelated directory. Linux uses extracted `AppRun`, not only the
inner ELF. This tests editor integration, not a missing build step; other apps need no such
editor-specific driver. Generic creation inspects DMG/NSIS/ZIP/tar/AppImage contents first.

## Native provenance and recipient materials

The accepted GNU Linux distribution host is Ubuntu 24.04. Packaging needs `dpkg-query`,
`pax-utils` (`lddtree`), `tar` and distro copyright/common-license directories. Other distros
need a reviewed provenance adapter. Missing ownership/notices/sources or unreviewed bundled
libraries fail closed. Docker on native x86-64 Linux is for the separate recipient-relink
check, not ordinary packaging.

`licenses/native-libraries` preserves full distro copyright records, package/source versions
and referenced common licenses. `lddtree`/`dpkg-query` own discovery. The actual generated
AppImage is extracted and its entire AppDir checked for ELF inputs, not just expected library
folders. Each bundled library needs a unique recorded owner and verified identity: loaded
code/data/notes, resolved dynamic symbols and needed-library names match the distro original.
The maintained `object` parser accounts for normal patchelf relocation metadata; a matching
filename is insufficient. Only individually verified application/AppRun inputs are exempt
from distro matching; the source-kit runtime is a separate verified prefix, not another
bundled ELF license resource. Installed source/notice bytes must equal verified staging.
Admission is restricted to reviewed Brotli, bzip2, libffi, libpng and Wayland notice hashes.
Changed terms or source families need review; collecting notices is not redistribution approval.

The runtime is supplied through linuxdeploy's `LDAI_RUNTIME_FILE`. Its real image prefix must
match the reviewed input except appimagetool's standard 16-byte `.digest_md5` field; AppRun
also has an exact reviewed checksum. Moving upstream downloads that no longer match fail.
Both Linux formats embed `licenses/appimage-runtime`: pinned runtime/libfuse/squashfuse sources,
the upstream libfuse patch, full notices, Alpine static objects/headers and rebuild instructions.
These are recipient materials, not links to changing upstream branches.

The Docker/Meson/Make recipient check modifies LGPL implementation code, relinks and executes
the replacement runtime. It retains the actual linker input map and toolchain/package inventory;
it claims neither hermetic nor bit-identical compilation. GCC startup/runtime license and
exception notices accompany the kit. Standard compiler startup objects come from the recipient's
normal toolchain, not the static-library APKs. Native hosted relink and installed-material checks
remain release prerequisites; source availability alone proves no license-completeness acceptance.

[Third-party obligations (English)][third-party] and the applicable license texts remain the
legal authorities, including LGPL modification/debugging rights, exact source provenance,
MPL, GCC exceptions, modified Winit, fonts and artwork. This contract describes evidence and
failure ownership, not a second license policy or legal opinion.

## License collector isolation

Configured `license-config` requires cargo-about 0.9.2. Native normal-dependency collection
fails on unresolved licenses, renders full copyright/license notices without local source paths,
and bundles original MPL-covered crates.io `.crate` archives verified against `Cargo.lock`.
Unknown non-registry covered provenance fails rather than substituting registry sources.
This supplements cargo-deny's dependency-admission policy and legal review, not replaces them.

The collector's frozen TLS graph has advisories; its library's krates dependency pins
indexmap/petgraph incompatibly with this workspace. Do not downgrade the application graph,
patch upstream or add a wrapper crate to force integration. The explicit shared installer uses
checksum-pinned upstream source, refreshes an isolated lock, audits it with Cargo/cargo-deny
before compilation and installs that exact graph with `--locked`. Source/lock/build stay
outside any Cargo workspace; the lock is evidence, not another tracked application lockfile.
Existing tool directories are never overwritten. Graphs may differ across installation dates;
every install must be audited. CI uses `RUNNER_TEMP` and retains the resolved tool lock.
No Python installer or implicit build-time setup is involved; the Manual owns setup commands.

## Independent consumer acceptance

`tests/fixtures/distribution` is a separate Cargo app, not a game runtime or published template.
The helper copies it to an isolated workspace with its own lockfile and local launcher:

```sh
python3 -X utf8 tools/distribution/validate_fixture.py --package
python3 -X utf8 tools/distribution/validate_fixture.py --revision EXACT_PUBLIC_GIT_SHA --package
```

These are native execution checks, not documentation-check commands. Local and pinned public
Git sources are explicitly selected; a local-path run does not prove public Git consumption.
The helper preserves the xtask alias, generates icons/sidecars, uses the shared build/package
route and runs the staged runtime consumer. Its normal graph rejects editor/import/build-tool
dependencies. It also exercises `cargo kedra run`, waiting for an application-owned marker
after the independently launched runtime verifies its delivery.

## Release orchestration boundary

The manually dispatched [Desktop distribution workflow][release-workflow] is restricted to
protected `main`, an exact source SHA and a new Cargo-matching tag. Preflight requires
protected-main ancestry, genuine successful repository push CI at that SHA and the current
GitHub Actions `CI required` result. Existing tags/releases are errors, never overwritten.
The source `.github/workflows` tree must match the current default branch before building and
again at publication: other trees may require workflow-write permission unavailable to the
ordinary Actions token. No personal token or permission widening works around this boundary.

Three native runners use the shared command and independent pinned public Git consumer.
The target-independent application dependency audit runs once on Linux; all runners install
cargo-deny to audit their isolated collector graph. Generic architecture/signature/packaging
regressions belong to `kedra_build` in native Rust CI; Python owns editor-specific smoke checks.
Linux CI explicitly runs source-kit/modified-LGPL relink regressions when packaging inputs
change, not on every release. Each release target still inspects its real packages and
public Git consumer: these exercise different artifacts and installed behavior.

Dependency-only caches are separate from validation/coverage roles and restricted to
`target/distribution-cargo` and the isolated launcher target. Final packages and independent
consumer workspaces live in runner temporary storage outside caches. Existing outputs remain
errors even on warm builds. Build/inspection jobs have read-only repository permissions.
Packages are retained one day; bounded diagnostic/tool-lock evidence three days. Python uses
standard UTF-8 mode on Windows too; Unicode paths must not be removed for legacy console pages.
Architecture checks parse the actual compiled binary with `object`, including matching and
mismatching target cases; they do not depend on a guessed filename or CLI success.

`publish` defaults to false for build-only acceptance. Only the final publisher gets
`contents: write`. It checks out/builds no application code, loading only narrow policy files
from the immutable main workflow SHA. It requires the scheduler's successful current-run native
matrix, consumes only that run's artifacts and validates all five payload sizes/hashes/source
identities. It creates a new draft, checks the complete upload set and server digests, binds
the tag to the reviewed source, then publishes an explicitly unsigned prerelease.
Failures leave an unpublished draft; reruns never replace it. REST draft creation supplies
the numeric release ID for inspection/publication; by-tag lookup is only for published releases,
not draft discovery. Maintained GitHub CLI transports assets; ID-based update clears `draft`
only after verified sizes/digests. [Release policy][release-policy] owns this protocol.

## Signing, updates and acceptance limits

The `v0.1.0` prerelease macOS asset has a known bundle-signature defect and requires local
re-signing. New bundles default to inside-out complete ad-hoc signing (`-`), preserving an
explicit identity when configured. All resources and `Info.plist` are sealed before DMG
creation. Inspection copies the actual image's app and runs
`codesign --verify --deep --strict --verbose=2` before separate editor CLI acceptance.
This is not Developer ID signing or notarization; Windows packages remain unsigned.
Integrity checks do not certify Gatekeeper/SmartScreen trust. Never disable OS protections
globally; ordinary macOS approval may still be required.

Native editor packages carry the installation identity consumed by `kedra_update`; portable
builds and other applications must not acquire it accidentally. The host owns update approval,
input/document settlement and shutdown; installation writes never save project sources.
See the [update crate contract (English)][update] for explicit macOS replacement and manual-only
NSIS/AppImage handling. Native update acceptance remains separate and pending.

macOS 15 is the editor's configured minimum. Linux glibc/GPU dependencies, the Windows OS floor,
install/uninstall and each architecture still need native acceptance. AppImage does not make
glibc, graphics drivers or FUSE universally compatible. Signature checks, installed CLI/resource
checks, rendered frames, physical input and OS integration are distinct evidence.
Current public availability of `v0.1.0` needs external verification. Automatic draft-to-published
operation, current native packages/installed resources and recipient relinking remain unverified;
workflow source is not hosted acceptance, successful publication or license-completeness proof.

[adr23]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0023-desktop-distribution.md
[bevy-release]: https://bevy.org/learn/quick-start/getting-started/setup/#compile-with-performance-optimizations
[third-party]: https://github.com/kedra-engine/kedra/blob/main/docs/third-party.md
[release-workflow]: https://github.com/kedra-engine/kedra/blob/main/.github/workflows/release.yml
[release-policy]: https://github.com/kedra-engine/kedra/blob/main/tools/ci/release.py
[update]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_update/README.md
