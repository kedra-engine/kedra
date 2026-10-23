# Testing and local tools

## Start with the contract

Tests protect Kedra behavior, not the implementation of Cargo, Bevy, serde or other
dependencies. [ADR 0031](decisions/0031-contract-owned-testing.md) defines ownership
and placement. Product contracts, assembled acceptance and repository policy are
different responsibilities, even when one command runs several of them.

Before writing a test, identify the Kedra guarantee, its production owner, the
observable failure, the least expensive sufficient environment and why existing
checks do not detect that failure. Name the test by behavior and outcome. Extend
the relevant contract suite for a bug fix; do not create historical regression buckets.

### Find the owner

| Capability | Primary contracts | Editor integration / assembled acceptance |
| ---------- | ----------------- | ---------------------------------------- |
| Projects and source paths | `kedra_project` | Editor startup, forms and project opening |
| Documents and instances | `kedra_scene` | Editor documents, Hierarchy and Scene View |
| Working changes and save protocol | `kedra_authoring` | Editor editing/Save are not implemented |
| Runtime delivery/integrity | `kedra_assets` | Preview resource binding |
| Processing and publication | `kedra_import` | Browser/Open/preview workflows |
| Dock topology/candidates | `kedra_docking` | Editor gestures and host coordination |
| Retained UI/projection/input | `kedra_ui`, `kedra_desktop` | Native dock/host/focus workflows |
| Reflected values/presentation | `kedra_inspector` | Editor source adapter and Inspector scenario |
| Script lifecycle/modules/scopes, children/events/parameters/diagnostics | `kedra_script/tests/contracts/`, `kedra_app` | Editor Luau and independent runtime scenarios |
| Script-owned Transform/UI adapters | `kedra_script_bevy/tests/contracts/` | Canonical native switch Behavior/Sequence |
| Editor styles/icons/authored sources | `kedra_editor/tests/contracts/` | Component corpus and native editor geometry |
| Build/package/update | `kedra_build`, `kedra_update` | Installed distribution and update acceptance |
| Repository and release policy | `xtask`, `tools/ci`, `tools/security` | Real checks, scanners and distribution workflow |

For a public crate contract, use `cargo test -p PACKAGE --test contracts --locked`.
Add a behavioral filter such as `docking::` or `styles::` to select a suite. Private
module contracts run with `cargo test -p PACKAGE --lib --locked FILTER`.
For example:

```sh
cargo test -p kedra_docking --test contracts --locked
cargo test -p kedra_editor --test contracts --locked styles::
cargo test -p kedra_editor --lib --locked workspace::
```

### Placement and fixture dependencies

- Private invariants use the owning production module's child `tests` module,
  either inline or a sibling test file. Larger suites split by coherent behavior.
- Public API contracts use one `tests/contracts.rs` Cargo target with behavioral
  modules under `tests/contracts/`. Separate targets need a process/isolation reason,
  not one executable per small source file. Do not widen production visibility.
  Repository-policy smoke targets under `xtask/tests/` remain separate from these
  product contracts and exercise real command/process boundaries.
- Shared setup lives in the narrowest `test_support` or `tests/support` module.
  The dependency direction is `tests -> support -> production APIs`, never from
  one suite to helpers exported by another suite. Nested cases within one suite
  may share that suite's local setup; independently owned suites must not.
- Editor CPU hosts live in `kedra_editor/src/test_support.rs`, workspace host/layout
  inputs in `workspace/test_support/`, and primitive projections in
  `scene_view/test_support.rs`. These are different fixtures, not a universal App.
- Native acceptance lives in `tools/test/src/scenarios/editor` and
  `tools/test/src/scenarios/runtime.rs`. Component capture/comparison uses the same
  existing runner and `kedra_test`, not a second framework.
- Tiny invalid inputs stay beside their cases. Canonical native project sources
  remain in `tests/fixtures/test_project`; mutations use complete private copies.

### Choose evidence, not repetition

Pure values cover domain/error matrices. Headless Bevy checks our registration,
ownership and scheduling. Real I/O checks publication and process boundaries.
Native workflows check integration and input/frame delivery. Only an explicit
pixel assertion proves image content; a GPU fence does not. Synthetic layout is
not production layout, and injected Bevy input is not physical OS certification.

For docking, topology assertions belong to `kedra_docking`, retained attachment
assertions to `kedra_ui`, document/selection coordination to the editor, and real
gesture/lifecycle assertions to native acceptance. Do not replay the entire domain
error matrix through the UI. A higher-level check needs a different failure boundary.

Negative cases assert relevant unchanged sources, history, published state and
foreign resources, not merely an error return. Prefer explicit clocks/readiness
over sleeps. Potentially blocking cases use killable subprocesses and an external
deadline independent of the mechanism being tested.

Thin utility wrappers are verified by real success/failure invocations. Do not
construct synthetic Cargo workspaces to demonstrate Clippy/rustdoc or test upstream
serialization/layout in isolation. Keep focused tests of Kedra-specific policy,
source safety and public build/package/run results. Upstream compatibility probes
need an explicit Kedra contract and a reason ordinary product checks cannot cover it.
Do not add QA-driver/report mocks or a self-test framework. Existing comparator
guards protect substantive acceptance decisions, not reporter cosmetics.

## Shared commands

| Command                    | Checks                                                     |
| -------------------------- | ---------------------------------------------------------- |
| `cargo xtask check`        | File size, architecture, format, lint, tests, doctests, API docs |
| `cargo xtask files`        | Per-file line and byte limits, including tracked ignored files |
| `cargo xtask fmt`          | Formatting without modifying files                         |
| `cargo xtask lint`         | All targets/features, Clippy warnings as errors            |
| `cargo xtask test`         | All-target tests and a separate doctest run                |
| `cargo xtask doc`          | API documentation with warnings and broken links as errors |
| `cargo xtask architecture` | Declared workspace dependency layers and package licenses  |
| `cargo xtask features`     | Default/minimal workspace, isolated runtimes and shipping diagnostic boundary |
| `cargo xtask visual --config … --actual … --output …` | Configured offline screenshot regression and review artifacts |

Use `cargo fmt --all` to apply formatting. Unknown task names fail, not skip.

The `cargo xtask` alias builds its orchestrator in `target/xtask-bootstrap/`,
separate from workspace test binaries. This prevents Cargo from replacing the
running `xtask.exe` during all-target tests, which Windows forbids. Child checks
still use the normal Cargo target directory (or `CARGO_TARGET_DIR` when set);
the alias's explicit bootstrap directory applies only to the orchestrator.

Docking has pure-model and independent projection tests plus an opt-in native
editor driver and an isolated A/B/C/D scene in the ordinary editor executable. See
[docking acceptance](ui/docking-implementation.md) for commands and evidence limits.
The [isolated dock fixture](ui/docking-tests.md) verifies production gestures and
retention without editor startup/tools or screenshot goldens.
The implementation is shared across Linux, Windows and macOS. The source CI
matrix checks headless behavior on those systems; current hosted results and
native input/frame acceptance require separate evidence.

Screenshot policy, actual-evidence format, animation boundaries and independent
baseline review are documented in [visual acceptance](ui/visual-acceptance.md).
The visual task is explicit, not an extra passing claim in `check` without frames.

The [production UI corpus](ui/ui-corpus.md) covers buttons and text fields only.
`cargo xtask test -- components` captures all 22 scenarios in two native sessions
and compares them against independent offline Figma sources. Real frames and input
verify the QA workflow; there are no separate mock/unit self-tests of QA scripts,
fixture hosts or report generators. Product Rust regression tests remain required.
The dispatched Native UI corpus workflow uses this same command; a reviewed Linux
profile and actual hosted acceptance are still pending.
The component policy pins the explicitly approved dependency graph. Its lock check
rejects changed graphs without reviewed lock approval; adding runtime dependencies
does not automatically approve a new visual profile or grant native acceptance.

Shared process, transport, comparison and evidence APIs live in `kedra_test`.
Concrete editor scenarios live in `tools/test`; run `cargo xtask test -- features`
or select a comma-separated subset with `--only`. Each run requires a new output
directory and an idle native desktop. Setup failures and unrun cases are reported
explicitly; a partial run cannot establish complete native acceptance.
Selecting `--only hierarchy` runs both behavior and appearance phases once in one
session, with separate phase results and renderer frames, including failed phases.
There is no separate `hierarchy-appearance` command. `scene-navigation` opens only
the model and mixed scene needed for navigation, with its own setup fences;
`scene-view` separately retains PNG/model/UI/mixed preview and host-transfer checks.
Catalog and docking scenes launch the ordinary debug `kedra` executable through
the shared `--scene` options, without separate Cargo example targets or hidden
fixture modes. The independent consumer uses the same application options; its
concrete behavior checks live in the common runner, not a per-game test executable:

```sh
cargo xtask test -- native-switch \
  --manifest-path PATH_TO_PROJECT_COPY_CARGO_TOML \
  --assets EDITOR_DELIVERY --project-assets GAME_DELIVERY --output NEW_DIRECTORY
```

This explicitly named scenario requires the `kedra-test-project` ordinary package/bin,
registered `scene.switch` program, known switch state and renderer method; it does
not claim arbitrary manifest support. The manifest locates a materialized consumer
workspace copied completely from `tests/fixtures/test_project` for that scenario,
including its ordinary runtime-only game executable. There is no separate consumer
workspace or per-scenario Cargo target. Reusable Cargo/process, scene-program transport
and evidence mechanisms
remain in `kedra_test`; other applications supply their own ordinary Rust verifier.
Both commands require the same two independently verified deliveries; resources do not select native
capabilities or source admission. `native-projection` accepts the same arguments and
selects the independent docking composition in two processes. It validates
all action topologies, retained Unicode draft, live observation, renderer readbacks
and successful native-close exits. See the
[native game contract](../tests/fixtures/test_project/native.md). Its native scenes and Luau
programs live only in `tests/fixtures/test_project`; the cross-delivery host catalogue
lives outside imported assets and is embedded by the game. Unique headless Luau
scoped-API isolation checks belong to `kedra_script` product tests, not another
consumer project or duplicate runtime-regression stand.
The excluded game's three projection-readiness contracts in `src/docking/scripting.rs`
run automatically before either independent native scenario builds/launches its host.
A failure prevents native acceptance. Root workspace tests do not execute the
excluded game; after the
[complete-copy preparation](../tests/fixtures/test_project/native.md#run-explicitly),
the focused headless entrypoint is also available:

```sh
cargo test --locked --manifest-path target/native-switch/project/Cargo.toml \
  --bin kedra-test-project docking::scripting::tests
```

Report this check separately from native input/frame acceptance. Shared schema,
parser and startup-admission tests remain in their product-owner crates; fixture
CLI/catalogue convention selftests are not another QA-driver suite.
Without arguments after `--`, `cargo xtask test` runs the workspace Rust tests and
doctests. Explicit trailing commands select native scenarios or source export;
ordinary `check` does not launch native applications.
Project-based scenarios consume the one checked
[test project](../tests/fixtures/test_project/README.md), with a fresh complete copy
per scenario. They do not generate scene/project fixtures or initialize new metadata
at startup. Failure checks mutate only their copies, and docking restart reuses its
scenario's copy. Source/delivery IDs remain stable. The startup Create check is an
explicit product operation under test, not fixture preparation. Project-independent
component hosts retain their separate isolation contracts.
The shared native actions execute in-application Luau under the same external Rust
supervisor. The `luau` feature runs a complete editor-owned startup-form program;
the remaining Rust scenarios preserve their existing feature assertions. Shared
runtime/contract regression tests live in `crates/kedra_script/tests` and
`kedra_app::scripting::tests`, not in a duplicate QA-driver self-test framework.
These are behavior checks only. Temporary GPU readbacks are synchronization fences,
not published screenshots or pixel comparisons. Screenshot coverage is limited
to the 22 kit button/input cases; full-window scenarios have no screenshot gate.
See the [shared tooling contract](../crates/kedra_test/README.md).

## Interactive CLI and logs

Run `cargo xtask` without a task in a terminal to choose from a keyboard menu.
An explicit task, such as `cargo xtask check`, runs immediately with numbered
stages, a live elapsed-time spinner, streaming child stdout/stderr and a final
per-stage summary. A failure stops subsequent stages and reports skipped counts.

Every selected run records a separate plain-text log under `target/xtask/logs/`;
its path is printed at startup and in the summary. Logs preserve diagnostics even
when a child fails. They are local artifacts, not committed source. Review them
for sensitive paths/data before sharing. Logs are not automatically pruned yet.
There is no automatic log upload or provider-specific summary adapter. Stage
status/duration and raw child diagnostics remain in terminal output and logs.
An early compilation failure may have no xtask log. Legacy hosting summary
environment variables have no effect, including during intentional test failures.

CI, redirected streams and `--non-interactive` disable prompts and animation;
live output and logs remain available. A missing task in that mode is an error,
not a prompt or an implicit full build. Use `cargo xtask --help` for available tasks.
Subprocess output is piped for streaming; this is not a PTY or a full-screen TUI,
and arbitrary third-party interactive applications are not supported by this runner.
Forced termination can leave an incomplete log and no final summary.

## Scope and additional environments

Contract reorganization preserves existing failure paths, identity/ownership checks,
source provenance and execution routes. Delete superseded paths and update callers
directly, without compatibility forwarding modules, aliases or duplicate suites.
Removing redundant/upstream-only checks requires identifying the remaining guarantee;
it must not hide a failure or weaken security/reference/coverage policy.

Ordinary CI does not build optimized editor releases or run full application
build/package/launch acceptance as an additional test matrix. Shared-build tests
use unit/failure-path checks and small dependency-free debug fixtures. Do not add
tests merely to assert Cargo release settings. Actual optimized editor packages
and independent application build/run acceptance belong to the explicitly
dispatched Desktop distribution workflow, not every PR or coverage run.

`features` separately checks minimal `kedra_app`, `kedra_script`, `kedra_scene`,
`kedra_assets` and `kedra_ui` libraries, so editor feature unification cannot supply
their optional capabilities. It also runs `kedra_app` launch contracts in the
`release-boundary` Cargo profile: dev optimization with debug assertions disabled.
This executes diagnostic CLI/plugin absence assertions, not an optimized editor
release or native application acceptance. Every subprocess must succeed independently.

## Evidence

Report exact commands, target, revision (including dirty changes), exit status and
limitations. Skipped tests are not passes. Avoid wall-clock sleeps and uncontrolled
network access in deterministic tests. Flaky tests get a tracked cause, owner and
repair plan, not unbounded retries or silent removal.

## Coverage and deeper analysis

Coverage is a reference metric for workspace Rust code, not a merge threshold.
There is no minimum percentage. The configured GitHub coverage job runs fresh
headless workspace tests and retains LCOV for Rust-affecting changes. Test failures,
measurement errors and missing/empty reports still fail validation; a low percentage
does not. Current hosted results require external verification.

Run the same measurement locally with cargo-llvm-cov 0.7.1:

```sh
cargo llvm-cov --workspace --all-targets --all-features --locked \
  --lcov --output-path target/coverage.lcov
```

This measures only headless workspace tests. Native scenario code remains in the
workspace denominator, but its ordinary binaries are not executed by this command.
The percentage therefore does not represent native scenario execution or UI
acceptance. Real native behavior and the reviewed component screenshot corpus
retain their separate input/frame checks; they are not run merely to raise coverage.
No QA-driver/report self-tests, source exclusions or coverage-only application modes
are needed for this metric. Critical persistence, identity and recovery behavior
still require explicit negative/fault tests regardless of percentage.

Fuzzing, Miri and benchmark regression gates are not installed as dummy jobs.
Introduce fuzz targets with parsers/untrusted formats, Miri with applicable memory
or concurrency code, and performance thresholds after a repeatable baseline exists.
Future native/browser runtime acceptance needs its own fixtures and runners;
the current desktop harness does not certify an engine or web export.

## Additional repository checks

`cargo xtask check` is the Rust gate, not a wrapper that silently installs every
external tool. Run the additional repository checks locally:

Run the native screenshot workflow on an idle, unlocked desktop:

```sh
cargo xtask test -- components --output target/test/components
cargo xtask test -- features --output target/test/features
cargo xtask test -- features --only luau --output target/test/luau
```

Native QA uses Rust, without a Python or Node/npm dependency. Figma source refresh is
explicit authoring in the Figma host and never a CI network dependency. See the
[shared comparison workflow](ui/ui-corpus.md#shared-buttons-and-text-field-comparison).

The portable Rust policy contracts, editor-icon contracts and feature builds run
without Bash/jq or a provider workflow. The icon target checks browser, hierarchy
and Scene View density metadata, not rendered pixels. The feature task checks
each subprocess exit and stops on failure:

```sh
cargo test -p xtask --test portable_contracts --locked
cargo test -p kedra_editor --test contracts --locked icons::
cargo xtask features --non-interactive
```

The configured `python-test` CI job (`Workflow and distribution tooling contracts`)
uses Python 3.13 for required-gate and distribution failure paths. Its local commands
are separate from native QA:

```sh
python3 -m unittest discover -s tools/ci -p 'test*.py'
python3 -m unittest discover -s tools/distribution -p 'test*.py'
```

```sh
# Pinned Rust tools
cargo install rumdl --version 0.3.0 --locked
rumdl check .
# Apply Markdown formatting explicitly when needed: rumdl fmt .
cargo install cargo-deny --version 0.19.0 --locked
cargo deny --locked check
cargo install lychee --version 0.24.2 --locked
lychee --offline --include-fragments --no-progress './*.md' './docs/**/*.md'
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.7.1 --locked
# Run the fresh informational workspace coverage command documented above.

# Go 1.24+ tools; use the executables in `go env GOPATH`/bin
go install github.com/zricethezav/gitleaks/v8@v8.24.2
```

Run `gitleaks dir . --redact --exit-code 1`
after adding that bin directory to PATH. This local directory scan checks the
working tree, not Git history. Hosted CI separately configures a full-history
checkout (`fetch-depth: 0`) and `gitleaks git --redact --no-banner .`.
Run the directory scan in a clean checkout,
because it also examines ignored build artifacts, which can
contain upstream cryptography examples after compiling tools locally. If artifacts
are present, scan a clean Git-inventory source snapshot rather than weakening
detectors or adding secret allowlists. The owner-reviewed non-secret provenance
exception below is not permission to allow authentication credentials. Check local
Markdown links
with Lychee 0.24.2 built from source with `cargo install --locked`, in offline/fragment
mode. Remote link availability
does not block PRs because external outages are not contributor code failures.

Tool installation and advisory refresh require network access. Install Go for
Gitleaks separately. The basic Rust contributor
loop needs Rust, a linker and the public native development packages documented in
[contributing](../CONTRIBUTING.md). Tool pins do not make the entire environment
bit-for-bit hermetic; record OS/toolchain identity with results.

Upstream tool lockfiles are separate from Kedra's dependency audit. The current
Lychee lock emits a yanked `spin 0.9.8` warning and a future-compatibility warning
for `proc-macro-error2 2.0.1`; these do not prevent compilation. Do not remove
`--locked` to silence them. Review tool pins and their upstream dependency changes
explicitly; building from pinned source is not a claim that every tool is vulnerability-free.

The commands above remain standalone local entry points. The GitHub workflow uses
reviewed major-tagged installation and dependency-cache actions; see [GitHub CI](ci.md)
for permissions, cache boundaries, Windows diagnostics and the failure runbook.

### Reviewed non-secret Figma identities

The root `.gitleaks.toml` extends all pinned Gitleaks 8.24.2 default detectors.
Its only additions are owner-approved, non-secret provenance exceptions for the
22 button/input metadata files in `docs/ui/corpus/text/`: the shared Figma file
identity and each file's own canonical component identity. These values identify
design objects; they do not authenticate the caller or grant bearer access.
The exporter derives them from `figma.fileKey` and `component.key`; see
[source provenance](ui/ui-corpus.md#evidence-and-source-provenance).

Each rule-scoped table requires **both** an anchored, exact repository-relative
file path and an anchored `generic-api-key` match binding the exact field spelling
to its reviewed value. Alternative field/value pairs within one table are OR;
path and content criteria are explicitly AND. No global exemptions, detector
overrides, directory exemptions or arbitrary metadata-value patterns are allowed.
Matching the individual finding, not the whole line, preserves detection of other
credentials even on the same line. Component values are not interchangeable
between files or with the file-identity field.

Gitleaks matches text, not JSON ancestry: it cannot enforce `$.file_key` versus
`$.instances[0].key` or authenticate a live library mapping. A repeated approved
field/value in the same approved file also matches. Existing source-schema and
manifest/fingerprint validation remain independent requirements. Changing or
adding identities requires fresh provenance review and explicit owner approval;
never auto-refresh this configuration from capture output or change source bytes,
manifests, licenses or reviewed baselines just to satisfy secret scanning.
Authentication credentials must never receive these exceptions.

For a clean source snapshot, run from its root using `gitleaks dir .` and its copied
configuration (or an explicit `--config` path); paths are deliberately not suffix
matches for arbitrary absolute or nested paths. History scans retain default
detectors and use the same exact repository-relative source paths.

The required CI `secrets` job runs the offline detector-policy regression after
provisioning Gitleaks 8.24.2 and before the history scan. Missing tools, unsafe
configuration or unexpected detector results fail the job, without a skip or
continue-on-error path. Run the same check locally with Python 3.11+ and the
installed pinned scanner on PATH:

```sh
python3 tools/security/check_gitleaks.py --gitleaks gitleaks
```

The command fails for absent/wrong scanner versions, validates configuration
structure and manifest consistency, and runs the real scanner on reviewed IDs,
synthetic credential additions/replacements, crossed fields/files, same-line
credentials, unrelated/prefix/suffix paths and a provider-specific default rule.
It retains redacted reports under a new `target/gitleaks-policy-*` directory.
It requires no account, service or network access. `--gitleaks /path/to/gitleaks`
selects an explicit installed executable; the check is not part of ordinary
Rust/Python test discovery and never silently skips when the scanner is absent.
