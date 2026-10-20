# Desktop Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use executing-plans to implement
> this plan task-by-task. Checkboxes track evidence-backed deliverables.

**Goal:** Finish CI acceptance, then publish native editor packages built by the
same `kedra_build` tooling and `cargo kedra` command an independent app can use.

**Architecture:** A tooling-layer library owns Cargo/import/packager orchestration;
a tiny project-local launcher provides a namespaced Cargo alias. Existing runtime
asset code owns delivery reading and appropriate platform resource lookup. Native
bundling uses cargo-packager; GitHub alone owns release scheduling/publication.

**Tech Stack:** Rust 1.98, Cargo, clap, cargo_metadata, duct, existing Kedra import
and asset crates, cargo-packager 0.11.8 CLI, winresource 0.1.34, GitHub Actions/CLI.

**Spec:** [Approved design](../specs/2026-10-20-desktop-distribution-design.md).

## Global Constraints

- Finish existing CI cold/warm acceptance and main protection before CD code.
- Work in the owner's existing checkout, not a new worktree. Preserve other work.
- Owner authorized autonomous implementation, commits and pushes after approving
  the separate build crate/command. No further routine review interruptions.
- New crate: `kedra_build`, layer `tooling`; no editor/xtask dependencies, no runtime
  edges to tooling/import/editor, no proprietary build framework or unsafe code.
- `cargo xtask` stays repository validation; `cargo kedra` is application tooling.
- Targets: x86_64-pc-windows-msvc, x86_64-unknown-linux-gnu,
  aarch64-apple-darwin, x86_64-apple-darwin, on standard native hosted runners.
- Downloads: Windows NSIS EXE and ZIP; Linux AppImage and TAR.GZ; macOS DMG per arch.
- No trusted signing certificates, notarization, paid services or protection bypass.
- Initial proposed release: v0.1.0, GitHub prerelease, exact successful CI revision.
- Package tests/metadata checks do not certify real input, rendered UI or OS trust.
- No game runtime, generated game feature or SDK/crates.io publication in this slice.
- Reuse approved artwork, importer and maintained native packaging/resource tools.
- No YAML-shape tests, check suppression, coverage reduction or whole-target caches.
- Files stay within 1000 lines / 64 KiB; evidence belongs under target/.

## Review Focus

- Tool launcher/compiler artifacts must not overwrite the executing launcher or
  retain an old checkout root; independent-app test exercises relocated sources.
- App bundle resource paths, Unicode/spaces and read-only installs must not fall
  back to current directory; task 3 covers override precedence and layout failures.
- Windows GUI subsystem changes must not hide CLI failures or spawn an unwanted
  console; task 5 checks actual CLI exit/output and native GUI appearance separately.
- Linux AppImage must not conceal unsupported glibc/FUSE/GPU requirements; task 6
  checks external dependencies and documents actual startup evidence.
- Wrong SHA/tag, incomplete artifacts and reruns must not publish a partial release;
  task 8 covers actual preflight failures without synthetic product releases.

## File responsibilities

- `crates/kedra_build/{Cargo.toml,src/lib.rs,src/cli.rs}`: tooling package and
  documented `pub fn run_cli(args: impl IntoIterator<Item = OsString>) -> ExitCode`.
- `crates/kedra_build/src/{config.rs,cargo.rs,pipeline.rs}`: typed Cargo/packager
  metadata, artifact discovery and ordered build/asset/staging subprocess lifecycle.
- `crates/kedra_build/src/{icons.rs,package.rs,inventory.rs}`: maintained icon
  conversion, packager invocation and exact artifact identities/hashes.
- `crates/kedra_build/tests/`: real command/failure-path and independent-consumer tests.
- `tools/kedra/{Cargo.toml,src/main.rs}`, `.cargo/config.toml`: thin local launcher
  and alias with isolated `target/kedra-build-bootstrap` outputs.
- `apps/kedra/{Cargo.toml,build.rs}`, `apps/kedra/src/main.rs`: editor package
  metadata, Windows resource compilation and standard bundle resource resolution.
- `crates/kedra_assets/src/location.rs`: runtime-only resource location helper and
  tests, consumed by editor and independent fixture; no importer/tool dependency.
- `tests/fixtures/distribution/`: minimal independent ordinary Cargo app, icon,
  asset source, notices and thin tool launcher; not a game runtime/template product.
- `.github/workflows/release.yml`: native package matrix, checks and publisher.
- `tools/ci/release.py`, `tools/ci/test_release.py`: only narrow GitHub release
  identity/result policy if native gh expressions/APIs cannot express it cleanly.
- `docs/decisions/0023-desktop-distribution.md`, `docs/distribution.md`: approved
  ownership/tool decision, installation/tool prerequisites and factual limits.

Public APIs below other than run_cli remain private until a real consumer needs
them. Internal types may evolve together; do not expose a speculative build SDK.

## Task 1: Close the existing CI migration

**Consumes:** hosted run 39484899924 for ecc155809deedf94b6b6c552813ee04aad6ef8cc.
**Produces:** actual full green native/coverage CI, warm measurements, main rules.

- [ ] Await the existing background watch, download completed job logs and diagnose
  real failures by stage. Preserve successful checks and failed evidence.
- [ ] For each code failure, reproduce it with a regression test before fixing.
  For workflow configuration, use actual tool/process reproduction and standard
  validators rather than homegrown YAML assertions.
- [ ] Run relevant suites and cargo xtask check on the correction revision; commit
  and push focused fixes under the owner's authorization. Observe all job results.
- [ ] After a full green run, rerun the same revision for comparable warm cache
  hits, sizes and timings. Record evidence under target/github-migration/.
- [ ] Require actual GitHub Actions CI required on main with PR-only changes,
  current base, resolved conversations and no force pushes/deletion or bypass.
  Do not require a second human approval in this single-owner project.
- [ ] Read rules back and use bounded non-product-merge probes of missing/failed
  results. Verify a legitimate green PR can meet requirements.
- [ ] Update current CI/hosting facts and limits. Expected: entire matrix and fresh
  >=80% coverage green; warm evidence and active rule identities read back.

## Task 2: Validate packaging tools and record the ownership decision

**Files:** tooling evidence under target/distribution/, ADR and distribution docs.
**Consumes:** CI acceptance; approved spec and existing artwork/import pipeline.
**Produces:** verified tool install/prerequisite contract for the four targets.

- [ ] Pin released cargo-packager 0.11.8 and inspect its released config schema,
  license, checksummed installation choices, transitive advisories and required
  native tools. Do not infer CLI support from npm tags or current upstream main.
- [ ] Validate winresource 0.1.34 as a Windows-only build dependency: license,
  resource compiler behavior, icon/version support and Cargo compatibility.
- [ ] Verify tiny actual native packager outputs before adopting configuration:
  NSIS/portable, AppImage/archive, app/DMG for both macOS targets. Generated evidence
  stays under target; no product dependency is installed through build.rs.
- [ ] Record required tool versions, NSIS/AppImage prerequisites, available minimum
  OS evidence and unsigned/ad-hoc-only behavior. Report unsupported tool behavior
  as a design compatibility issue, not permission to write an installer.
- [ ] Write ADR 0023: separate tooling crate and pinned local alias; existing
  packager owns native formats; no runtime/import layering change.
- [ ] Expected: actual package inspection succeeds and tool constraints are known.
  Commit the approved decision/docs, never transient tool binaries/evidence.

## Task 3: Standard runtime resource lookup

**Files:** kedra_assets location module/tests, editor CLI and its existing tests.
**Produces:** `bundled_asset_root(executable: &Path, platform: BundlePlatform)`
`-> Result<PathBuf, LocationError>`; explicit platform enum keeps tests host-independent.

- [ ] Add failing tests for executable-adjacent assets on Windows/Linux and
  Contents/Resources/assets for a standard macOS bundle; test a plain macOS dev
  executable, invalid executable path, spaces and non-ASCII directory names.
- [ ] Keep CLI/environment override precedence in editor tests: explicit CLI wins,
  then KEDRA_ASSETS, then platform default; missing assets fail verification rather
  than searching current directory or a developer checkout.
- [ ] Run the new tests and observe failure before adding the minimal path helper.
  Use Path/OsStr, not string separator replacement or lossy path serialization.
- [ ] Wire the editor default to the helper without changing override semantics.
  Do not create an alternative manifest/delivery reader.
- [ ] Run cargo test -p kedra_assets --locked and relevant app CLI tests; expected:
  new cases green, existing invalid delivery/remote credential tests still green.
- [ ] Commit the focused runtime resource fix and tests.

## Task 4: Shared build crate, launcher and an independent consumer

**Files:** kedra_build config/CLI/Cargo modules and tests, tools/kedra launcher,
root workspace config, independent fixture, manifest/architecture contracts.
**Consumes:** tool contract and runtime location helper.
**Produces:** `cargo kedra build --release --package NAME --target TRIPLE` and
`cargo kedra package --package NAME --target TRIPLE --output NEW_DIRECTORY`.

- [ ] Define typed internal BuildRequest, ApplicationMetadata and BuildError.
  Read standard package.metadata.packager for native identity/icon/resource data;
  use a narrowly documented package.metadata.kedra.distribution table only for
  Kedra asset source/profile/namespace and approved vector source. Cargo owns
  application name/version; do not duplicate them in a new project language.
- [ ] Add failing parser/preflight tests for malformed/unsupported target, missing
  package/tool/metadata, incompatible version and path handling. Add tests proving
  no editor/xtask/runtime-to-tool dependency is introduced and xtask alias survives.
- [ ] Run targeted tests and observe RED before implementing run_cli and thin
  launcher. Add explicit workspace member tools/kedra; isolate its target directory
  as target/kedra-build-bootstrap to avoid executing overwritten tool binaries.
- [ ] Reuse cargo_metadata for workspace/package selection and Cargo JSON artifact
  messages. Verify the selected executable belongs to the requested target/package;
  do not infer output by appending a platform extension to a guessed path.
- [ ] Copy the independent fixture to a fresh temporary workspace. It has its own
  application metadata/lockfile/alias and uses the build library as tooling only.
  Tests can locate current tool sources explicitly; application logic may not know
  editor source directories. After a public commit, verify a pinned public Git
  dependency variant too, rather than claiming a path-only fixture proves bootstrap.
- [ ] Expected: both editor and independent fixture run cargo kedra build; the
  fixture's normal application dependency graph contains no editor/build/importer.
  Run crate tests, existing portable contracts, architecture and full Rust check.
- [ ] Commit crate, minimal launcher, configuration and tested independent fixture.

## Task 5: Asset staging, icons and Windows executable integration

**Files:** kedra_build pipeline/icons modules and tests; app package metadata,
Windows build.rs/resource inputs; fixture resource inputs and notices.
**Consumes:** BuildRequest/ApplicationMetadata, compiled Cargo artifact, tool pins.
**Produces:** verified fresh staging tree for the native packager, no published file.

- [ ] Add failing tests for existing/symlink/partial output, missing or corrupt
  resources, failed compiler/import subprocess and immutable source inputs.
  Test Unicode/space paths and verify no package is reported after any failure.
- [ ] Reuse kedra_import::build(source, output, profile, namespace, progress) and
  kedra_assets::delivery::read_directory for delivery preparation/verification.
  Use maintained file inventory/copy and archive libraries where needed; do not
  add a custom walker/parser or an alternate asset compiler.
- [ ] Generate sized images from the approved SVG with existing maintained SVG
  processing, then use maintained icon encoders. Store generated icons under target.
  Validate actual dimensions/alpha and required ICO/ICNS/PNG entries, not filenames.
- [ ] Wire Windows icon and version resources using winresource behind Windows
  build-dependencies only. Keep build.rs local and deterministic with rerun inputs;
  no cargo invocation, bundling, SDK install or network access from the hook.
- [ ] Test actual Windows PE resources and application CLI failures/exit codes.
  Preserve redirected diagnostics; separately verify that normal GUI launch does
  not create an unwanted console. No unsafe local console attachment workaround.
- [ ] Verify source data and license notices remain unchanged; run targeted crate,
  asset and app tests, then full Rust check and source/security policy checks.
- [ ] Expected: valid staging trees and icon/version resources; all failure cases
  fail without clobbering previous outputs. Commit focused code/tests/metadata.

## Task 6: Native packaging and artifact inventory

**Files:** kedra_build package/inventory modules and tests; distribution metadata.
**Consumes:** staging tree and checked packager executable from tasks 2/5.
**Produces:** the agreed files plus artifact-inventory.json and SHA256SUMS.

- [ ] Add failing tests for missing format/architecture/version, failed packager,
  unexpected/duplicate artifact and inventory/hash mismatch. Do not snapshot
  generated packager configuration; inspect resulting files and behavior.
- [ ] Invoke pinned cargo-packager directly with argument arrays and native config.
  It owns app/DMG, NSIS and AppImage internals. Use maintained archive tooling for
  portable ZIP/TAR.GZ. No competing shell/YAML bundler implementation.
- [ ] Generate a typed inventory binding source SHA, app/version, target, basename,
  format, byte size and SHA-256 for every file. Reject ambiguous/unexpected outputs.
  Hashes provide integrity, not trusted publisher authentication.
- [ ] Inspect ELF/PE/Mach-O architecture, native metadata, icons, resources and
  notices. Determine glibc/external library requirements; do not bundle GPU drivers
  or advertise AppImage as universally compatible Linux execution.
- [ ] Unpack/install into a fresh location and execute actual packaged entrypoints
  with no development asset overrides, source checkout or Rust on the runtime PATH.
  Test Windows uninstall and the copied macOS app, not only staging executables.
- [ ] Use available real window/input/frame fixtures and physical macOS evidence;
  distinguish package/process checks from unverified physical/native UI targets.
  A known broken startup blocks publication; unavailable hardware stays unverified.
- [ ] Expected: Windows two formats, Linux two formats and one DMG per macOS arch,
  all bound to their exact inventory. Run complete relevant suites and commit.

## Task 7: Native CD build matrix without publishing

**Files:** .github/workflows/release.yml; tool install/version docs; CI policy.
**Consumes:** cargo kedra package and native inspection/smoke contract.
**Produces:** validated four-target package artifacts from one explicit revision.

- [ ] Add manual workflow_dispatch with validated version/tag and reviewed source
  identity. Verify successful CI required from this repository's Actions workflow
  at that exact commit; reject wrong tag/version, unrelated SHA and missing green.
- [ ] Use read-only permissions for preflight/build/inspect; actions:read only where
  needed to inspect CI. Checkout never persists credentials. No privileged PR path.
- [ ] Run native targets on windows-2025, ubuntu-24.04, macos-15 and macos-15-intel;
  assert toolchain/target architecture and call the same cargo kedra command.
  Keep target matrix independent/fail-fast false and timeouts/resource limits visible.
- [ ] Cache only dependencies and verified build-time tools; separate release-role
  keys from test/coverage and bind compiler/flags/lock/OS-image/arch. Successful
  trusted-source jobs only save, no whole staging/bundle/profile cache.
- [ ] Retain inventories/packages briefly for publisher handoff and bounded failure
  logs separately. Measure actual sizes within free storage instead of silently
  increasing allowances or enabling paid runners.
- [ ] Validate with actionlint/zizmor and actual manual package jobs. Deliberately
  reject a bad version/SHA in read-only preflight; missing/failed matrix entries
  cannot reach publisher. No YAML-value tests or fake successful result injection.
- [ ] Expected: all four actual native package jobs pass on a reviewed revision.
  Commit workflow and factual tested prerequisites; publish nothing yet.

## Task 8: Narrow release publication and first prerelease

**Files:** publisher job, narrow release-policy tests if needed, distribution docs.
**Consumes:** current-run validated artifacts, complete matrix, exact green identity.
**Produces:** v0.1.0 GitHub prerelease, anonymously downloadable verified packages.

- [ ] Prefer native gh/API behavior; if custom policy is required, isolate only
  exact identity/inventory/result checks in tools/ci/release.py. Test real entrypoint
  failures for wrong SHA/version, failed/missing group, conflicting tag/release,
  unexpected file and hash mismatch before implementation; never add a CI executor.
- [ ] Give contents:write only to the final publisher, with no project-code checkout
  or arbitrary build-hook execution. Consume artifacts from this run, not a mutable
  external link, branch name or unrelated workflow's upload.
- [ ] Create a draft, upload the exact expected package set, checksums and notices,
  verify identities/complete uploads, then publish as prerelease. Failed uploads
  leave an explicit incomplete draft, never overwrite an unrelated existing release.
- [ ] Test preflight/manifest failure paths without merging fixtures or publishing
  dummy product releases. Rerunning against a conflicting completed release fails
  visibly instead of replacing its binaries.
- [ ] Run full validation on the final reviewed revision and observe its hosted CI.
  Publish the approved first prerelease only after exact-SHA CI and packaging pass.
- [ ] Download every asset without credentials and compare hashes/architecture;
  read back release visibility, prerelease flag, tag SHA and all asset names/sizes.
- [ ] Release notes disclose unsigned Gatekeeper/SmartScreen warnings, platform
  floors, startup/native QA limits and lack of working game runtime/Play. Never
  advise global OS-protection bypass. Link install and checksum instructions.
- [ ] Expected: actual downloadable packages match this run's inventory. Commit
  factual docs and leave a clean checkout; report real checks, remaining risks and
  unverified hardware rather than claiming complete native certification.

## Self-review and execution

Tasks 3/4/5 share exact asset/build request interfaces; task 6 consumes the verified
staging tree; tasks 7/8 consume the same inventory/source identity. No second build
path is introduced. Native packaging remains tooling-only; Windows resource hooks
do not acquire packaging or network responsibilities. The standalone consumer
tests the extraction boundary before future gameplay/scaffolding work exists.

Owner selected autonomous native execution with commits/pushes authorized. Keep
one whole-change independent review after implementation; do not re-dispatch a
review per task. CI completion remains the gate before starting task 2 product work.
