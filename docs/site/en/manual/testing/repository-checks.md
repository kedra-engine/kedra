# Repository checks

Use focused tests while developing, then the repository gate and relevant extra environments.
These commands validate repository Rust behavior; they do not launch native acceptance.
In particular, `cargo xtask features` and `cargo xtask test -- features` are different tasks.

## Prepare the host

Use the [repository CLI reference](../cli/repository-and-tests.md) for dispatch syntax and
the [CLI task map](../cli/index.md) when choosing between build and test commands.

Install Git, rustup and the native linker/development packages described in
[Getting started](../getting-started/index.md). The repository selects `rust-toolchain.toml`.
Initial dependency fetching needs network access. Normal Rust checks use the checked lockfile.
Do not remove `--locked` to work around dependency drift.

The basic Rust loop needs no private service, credentials, Python or AI subscription.
Headless checks need no GPU, although compiling native dependencies still needs platform packages.
External Markdown, links, security and coverage tools have separate installation requirements.

## Run an owner contract first

Run a focused suite for the code you are changing, for example:

```sh
cargo test -p kedra_docking --test contracts --locked
cargo test -p kedra_editor --test contracts --locked styles::
cargo test -p kedra_editor --lib --locked workspace::
```

The first checks public topology, identity, candidate and snapshot behavior without a native host.
The second selects editor-owned style contracts; it does not compare rendered text.
The third selects private workspace tests; supplied layout geometry is not native layout evidence.
Cargo filters are substring selections. Inspect the reported tests, including zero-test results.
A green command that selected nothing does not verify the intended contract.

## Run the Rust gate

```sh
cargo xtask check
```

The task runs eight ordered stages and stops after the first failure:

| Stage | What is checked |
| --- | --- |
| File growth policy | Git inventory, including tracked ignored files; 1000 lines / 64 KiB |
| Dependency architecture | Workspace layer edges, package licenses and lint inheritance |
| Dependency exception policy | Reviewed advisory identities, expiry, restricted callers and exact duplicate waivers |
| Rust formatting | `cargo fmt --all -- --check`; no formatting writes |
| Clippy | Workspace, all targets/features, locked; unsafe code forbidden and warnings denied |
| Unit and integration tests | Workspace, all targets/features, locked |
| Documentation tests | Separate workspace doctest run with all features, locked |
| API documentation | Workspace rustdoc without dependencies, all features; warnings denied |

All-target tests do not execute ordinary native scenario binaries as applications.
`check` does not include the feature matrix, image comparison, external scanners or site build.
It is neither an optimized editor release nor build/package/install acceptance.

For a focused repeat, use `cargo xtask files`, `architecture`, `dependency-policy`, `fmt`,
`lint`, `test` or `doc`.
Without trailing arguments, `cargo xtask test` runs Rust tests and then doctests.
Arguments after `--` instead dispatch the native-test tool; they are not Rust test filters.

## Check features independently

```sh
cargo xtask features --non-interactive
```

This runs default and no-default-feature workspace checks, followed by separate minimal library
checks for `kedra_app`, `kedra_script`, `kedra_scene`, `kedra_assets`, `kedra_ui`
and `kedra_wakeup`,
plus the profiling-disabled `kedra_profiling` tests.
Separate invocations prevent editor feature unification from supplying optional capabilities.
An all-features build alone cannot establish a minimal consumer boundary.

The final stage runs `kedra_app` launch tests under the `release-boundary` profile with
debug assertions disabled. They assert that shipping CLI/transport diagnostics are absent.
This profile uses development optimization; it is not an optimized editor release build.
Each subprocess must succeed independently, and failures stop the task.

## Read progress and recover

Explicit tasks print numbered stages, stream child diagnostics and finish with a per-stage summary.
Rust task logs live under `target/xtask/logs/`; the path is printed for the current run.
Skipped later stages are not passes. Early bootstrap compilation failure can precede log creation.
Forced termination can leave an incomplete log and no summary.

CI, redirected streams and `--non-interactive` disable menus and animation, not diagnostics.
A missing task outside an interactive terminal is an error. Use `cargo xtask --help` to choose.
The orchestrator is bootstrapped under `target/xtask-bootstrap/`; child checks use the ordinary
Cargo target directory, including `CARGO_TARGET_DIR` when set.

Fix the reported failure in the responsible module, then repeat the focused stage and required gate.
Apply Rust formatting deliberately with `cargo fmt --all`, without overwriting unrelated work.
For architecture errors, fix the dependency responsibility rather than adding a forwarding crate.
For growth errors, split coherent responsibilities rather than adding exclusions
or compressing text. Never disable a test, relax lint/security policy or mutate a reviewed
reference to obtain green output.

## Additional evidence is separate

Markdown lint, offline links, dependency/security checks and the documentation site's
paired-page validation are separate repository checks, not hidden stages of the Rust gate.
Their success must be recorded separately when relevant to the change.

Coverage is informational, with no minimum percentage. Fresh measurement must still succeed and
produce a nonempty report. With cargo-llvm-cov 0.7.1 and `llvm-tools-preview` installed:

```sh
cargo llvm-cov --workspace --all-targets --all-features --locked \
  --lcov --output-path target/coverage.lcov
```

This executes headless workspace tests, not native acceptance scenarios.
Native driver source in the denominator does not mean its workflows ran.
Do not add artificial tests, exclusions or native runs solely to improve the percentage.
Use [Native scenarios](native-scenarios.md) for integration evidence and
[Visual verification](visual-verification.md) for the reviewed screenshot scope.

## External tools and security

These standalone commands run from the repository root. Installation and advisory refresh
require network access; nothing below is an implicit part of the Rust gate. The
[testing contract](../../contributing/contracts/testing.md#engineering-tools-and-pins)
lists required versions, check scope and exceptions.
Inspect commands before authorizing installation.

```sh
# Install pinned tools explicitly; these commands fetch/build external source.
cargo install rumdl --version 0.3.0 --locked
cargo install cargo-deny --version 0.19.0 --locked
cargo install lychee --version 0.24.2 --locked
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.7.1 --locked
# With Go 1.24+ installed separately:
go install github.com/zricethezav/gitleaks/v8@v8.24.2
```

Add the executables in `go env GOPATH`/bin to PATH before scanning. Apply formatting
only deliberately: `rumdl fmt .` writes Markdown; `rumdl check .` does not.
The offline link command validates local fragments without contacting remote sites:

```sh
rumdl check .
cargo deny --locked check
lychee --offline --include-fragments --no-progress --root-dir "$PWD/docs/site" './*.md' docs
cargo test -p xtask --test portable_contracts --locked
cargo test -p kedra_editor --test contracts --locked icons::
python3 -m unittest discover -s tools/ci -p 'test*.py'
python3 -m unittest discover -s tools/distribution -p 'test*.py'
```

The two Python discovery routes are CI tooling/distribution checks (CI uses Python 3.13),
not dependencies of native QA. Portable/icon checks assert policy/metadata, not pixels.
Measure coverage with the separate command above. Site validation has its own
[documentation workflow](../../contributing/documentation.md).

Run this working-tree scan from a **clean checkout or clean Git-inventory source snapshot**,
including the copied `.gitleaks.toml`, at its root:

```sh
gitleaks dir . --redact --exit-code 1
```

It is not a history scan. Ignored build artifacts are examined too and can contain upstream
crypto examples; use a clean source snapshot rather than weakening detectors. CI separately
uses full history (`fetch-depth: 0`) and `gitleaks git --redact --no-banner .`.
The [reviewed non-secret identity policy][non-secret-policy]
never permits credential exemptions.

With Python 3.11+ and pinned Gitleaks on PATH, explicitly run the offline policy check:

```sh
python3 tools/security/check_gitleaks.py --gitleaks gitleaks
```

`--gitleaks /path/to/gitleaks` selects an installed scanner. Missing/wrong tools fail rather
than skip. The real scanner checks configuration/provenance and synthetic negative cases;
redacted evidence goes to a new `target/gitleaks-policy-*`. It needs no network or account
and is not part of ordinary Python/Rust discovery. Review artifacts before sharing.

[non-secret-policy]: ../../contributing/contracts/testing.md#reviewed-non-secret-figma-identities
