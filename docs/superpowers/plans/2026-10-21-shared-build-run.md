# Shared verified build and run implementation plan

> **For agentic workers:** Use `executing-plans` for inline implementation, or
> `subagent-driven-development` if the owner chooses delegated execution. Track
> each step with the checkboxes below. No commits without owner inspection and approval.

**Goal:** Deliver the editor-independent verified desktop build/run contract in 0.1.2.

**Architecture:** `kedra_build` owns assembly, generic validation and explicit launch.
Cargo, the existing importer and cargo-packager retain compilation, assets and
native formats. CLI, future editor integration and CI consume the same result.

**Tech Stack:** Rust, clap, cargo_metadata, cargo-packager, existing object/archive/
download libraries, system native validation tools and the project-local launcher.

**Spec:** [ADR 0025](../../decisions/0025-verified-build-and-run.md).

## Global constraints

- Work in the current folder on `feat/shared-build-run-0.1.2`; no new worktree.
- Do not change the 0.1.1 release source `4187f86eee86fee7736f05f10bcb4175cc7659fa`.
- Preserve native Windows x64, Linux x64, macOS ARM64 and macOS Intel selection.
- Preserve release `opt-level = 3`, `codegen-units = 1`, `lto = "thin"`.
- No unsafe code, extra crate/framework, runtime-to-tooling edges or implicit installs.
- Fresh outputs only; keep failure evidence and source identities unchanged.
- No APK/IPA implementation, editor button, in-editor Play or JSON/PNG QA routing.
- No automatic commit, push or 0.1.2 publication under this plan.
- Ordinary CI uses unit/failure-path checks and tiny debug fixtures, not new full
  build/run tests or release-profile assertions. Full acceptance belongs to CD.

## Review focus

- Unicode/spaces/option-like arguments must remain distinct native arguments.
- Asset overrides and source working directories must not hide broken output layouts.
- A failed build/verification must never launch an older or partial result.
- Consumer-specific identities/CLI assumptions must not leak into generic checks.
- Missing tools and corrupt installed package contents must fail before success.

## File responsibilities

- `crates/kedra_build/src/pipeline.rs`: shared ordered orchestration and result publication.
- New `application.rs`: runnable output assembly and verified result schema.
- New `verify.rs`: consumer-neutral runnable and installed-content validation.
- New `launch.rs`: platform launch request, argument/environment/log handling.
- New `tools.rs`: prerequisite checks and explicit audited collector setup.
- Existing `package.rs`/`worker.rs`: reuse packager configuration and native workers.
- Existing `cli.rs`/`lib.rs`: commands, diagnostics and documented lifecycle contract.
- Tests beside these modules: focused native and failure-path regression fixtures.
- Existing independent distribution fixture/helper: real cross-project CLI acceptance.
- Python distribution tools: retain editor smoke tests only; remove migrated scripts.
- `docs/distribution.md`/`docs/architecture.md`: current contracts after implementation.

## Task 1: Independent prerequisite and license setup

**Files:** new `src/tools.rs` and `src/tools_tests.rs` under `crates/kedra_build`;
modify `cli.rs`, `lib.rs`, `pipeline.rs`, and remove the Python installer and its
tests after migration. Read `licenses.rs` and installer tests before editing.

**Interfaces:** `tools::preflight(app: &Application, package: bool) -> Result<(), String>`;
`tools::install_about(output: &Path, archive: Option<&Path>) -> Result<(), String>`.
Expose `tools install-about --output PATH [--archive PATH]` through clap.

- [ ] Write failing tests for checksum mismatch before extraction/execution,
  existing output refusal, workspace isolation, audit failure before compilation,
  missing collector/native tools and build-vs-package tool selection.
- [ ] Run `cargo test -p kedra_build tools` and the CLI installer tests;
  record the expected failures under `target/`.
- [ ] Port the existing pinned source/checksum and Cargo update/deny/install flow,
  reusing ureq/tar/flate2/tempfile. Stream diagnostics; retain source/lock/build.
  Invoke this CLI directly in CI; PATH handoff belongs to CI, not a Python wrapper.
- [ ] Call preflight before output preparation/expensive application compilation.
  Apply configured dependency notices to both runnable and packaged results.
- [ ] Rerun the focused tests, including negative paths. Do not suppress advisory
  or license failures to obtain green results.

## Task 2: Verified runnable application and result handoff

**Files:** new `application.rs`, `verify.rs` and their focused test modules;
modify `pipeline.rs`, `package.rs`, `worker.rs`, `lib.rs`, and `Cargo.toml` only
if existing object feature flags need PE/Mach-O support.

**Interfaces:** define internal serializable `BuildResult` with schema version 1,
application/version/target and output-relative application, executable and resource
paths. `application::assemble(app: &Application, stage: &Path, output: &Path,
icons: Vec<PathBuf>) -> Result<BuildResult, String>`;
`verify::application(app: &Application, output: &Path, result: &BuildResult)
-> Result<(), String>`. Publish `build-result.json` only after successful validation.

- [ ] Add failing fixtures proving wrong executable architecture, missing/tampered
  delivery, missing notices and incorrect macOS bundle identity/signature fail.
  Include non-editor product/binary names and Unicode output paths.
- [ ] Run `cargo test -p kedra_build application` and `cargo test -p kedra_build verify`;
  inspect actual failure reasons rather than accepting unrelated compile errors.
- [ ] Reuse shared packager configuration for macOS `PackageFormat::App` without
  DMG creation; preserve Windows/Linux adjacent-resource staging. Keep worker
  self-reexecution rather than mutating process-global environment/directory.
- [ ] Use the existing object reader for machine/format checks and system codesign
  for strict macOS verification. Validate delivery with the existing runtime API.
  Refactor pipeline to return the verified result; do not expose new public APIs.
- [ ] Test that validation failure publishes neither build-result nor success;
  existing-output rejection and source bytes/sidecars remain unchanged.
- [ ] Rerun focused tests, including the real native signing/tamper regression.

## Task 3: Explicit build and launch

**Files:** new `launch.rs`/`launch_tests.rs`; modify `cli.rs`, `lib.rs`, pipeline
return handling, `tests/cli.rs`, and the independent fixture's argument handling.

**Interfaces:** `launch::request(app: &Application, output: &Path,
result: &BuildResult, arguments: &[OsString]) -> Result<(), String>`.
Add `run` with existing build selection/release/output options and trailing args.

- [ ] Write focused tests for `run --help`, native argument construction,
  Unicode/spaces, missing-result/dispatch failure and validation refusal.
- [ ] Run `cargo test -p kedra_build launch` and `cargo test -p kedra_build --test cli`.
- [ ] Make `run` call the same build pipeline, then launch its verified result.
  On macOS use `open -n` with the explicit bundle path and `--args`; on Windows/
  Linux use standard Command APIs with an independent process group where
  available. Never invoke a shell or wait for application exit as build acceptance.
- [ ] Clear child `KEDRA_ASSETS`, set output working directory, close direct-child
  stdin and redirect direct-child stdout/stderr to new persistent output logs.
  Report launch-request acceptance, not GUI readiness or eventual exit status.
- [ ] During explicit release acceptance, verify the short-lived independent
  fixture launch and executable-relative resources. Keep this out of ordinary CI.
  Failed launch must retain the valid result and return command failure.

## Task 4: Validate actual packages inside shared tooling

**Files:** new `package_verify.rs`/`package_verify_tests.rs`; modify `package.rs`,
`pipeline.rs`, `lib.rs`, Python inspector/tests and distribution fixture helper.

**Interfaces:** `package_verify::verify(app: &Application, stage: &Path,
artifacts: &[(PathBuf, &'static str)]) -> Result<(), String>` before inventory write.
Reuse `verify::application` for extracted installed layouts with explicit mapping.

- [ ] Write failing native tests for corrupt/missing packaged assets/notices,
  identity/architecture mismatch and malformed macOS signatures. Test an
  independent application that implements none of the editor's CLI commands.
- [ ] Run `cargo test -p kedra_build package_verify` on the native host.
- [ ] Inspect real formats using maintained zip/tar readers and platform tools:
  read-only DMG mount/copy with guaranteed detach attempt, nonexecuting 7-Zip NSIS extraction,
  AppImage extraction and portable archive extraction. Reject archive path escape.
  Compare delivered resources/notices against verified staging; retain existing
  Linux source/provenance/runtime checks, without duplicating their algorithms.
  Installation acceptance is explicit and runs only on disposable Windows hosts;
  a temporary `/D=` is not sufficient isolation from an existing installation.
- [ ] Keep editor-specific CLI/font/interaction smoke checks in Python. Remove
  only duplicated generic ownership after tests prove shared validation runs.
- [ ] Verify failure prevents artifact inventory/success and does not launch a
  consumer executable as an implicit generic smoke test.

## Task 5: Independent acceptance, documentation and 0.1.2 version

**Files:** `tools/distribution/validate_fixture.py`, independent fixture, existing
CI selectors/workflows where needed, `Cargo.toml`, `Cargo.lock`, fixture pins,
`docs/distribution.md`, `docs/architecture.md`, and ADR 0025 status/limits.

- [ ] Extend independent-consumer acceptance to build and `run`, validating an
  actual child marker/arguments from an unrelated directory with no asset override.
  Preserve the normal-graph exclusion for editor/import/build tooling.
- [ ] Run local isolated-consumer acceptance and the relevant Python suites;
  retain commands/results under `target/`, not repository narrative documents.
- [ ] Update contracts and project-local setup instructions. State native GUI,
  Developer ID/notarization, mobile and untested Intel execution limits explicitly.
- [ ] Bump workspace/internal/fixture versions to 0.1.2 and update only corresponding
  lock entries, without unrelated third-party dependency refreshes.
- [ ] Run `cargo xtask check`, focused Rust/Python regressions, Markdown/link and
  workflow checks. Exercise real native builds/packages on all three release OSes
  through the normal reviewed PR when publication authorization is provided.
- [ ] Self-review against ADR 0025, report actual results and missing native
  evidence, and present the diff for owner inspection. Do not commit or publish
  0.1.2 automatically; the 0.1.1 release continues independently.
