# Testing and local tools

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
| `cargo xtask features`     | Separate default/minimal workspace and isolated runtime builds |
| `cargo xtask visual --config … --actual … --output …` | Configured offline screenshot regression and review artifacts |

Use `cargo fmt --all` to apply formatting. Unknown task names fail, not skip.

The `cargo xtask` alias builds its orchestrator in `target/xtask-bootstrap/`,
separate from workspace test binaries. This prevents Cargo from replacing the
running `xtask.exe` during all-target tests, which Windows forbids. Child checks
still use the normal Cargo target directory (or `CARGO_TARGET_DIR` when set);
the alias's explicit bootstrap directory applies only to the orchestrator.

Docking has pure-model and independent projection tests plus an opt-in native
editor driver and a separate native A/B/C/D host. See
[docking acceptance](ui/docking-implementation.md) for commands and evidence limits.
The [isolated dock fixture](ui/docking-tests.md) compares the production dock
mechanism against pinned independent Figma exports, without editor startup/tools.
The implementation is shared across Linux, Windows and macOS; destination CI
is not configured or certified yet.

Screenshot policy, actual-evidence format, animation boundaries and independent
baseline review are documented in [visual acceptance](ui/visual-acceptance.md).
The visual task is explicit, not an extra passing claim in `check` without frames.

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

## Test design

- Unit tests cover invariants and error paths, not private implementation trivia.
- Contract tests cover plugin registration, references, formats and dependency boundaries.
- Integration tests use independent consumer workspaces and installed artifacts.
- Property tests cover graph operations, references and encode/decode invariants.
- Storage tests inject failures around writes, rename, sync, receipts and recovery.
- Lifecycle tests include repeated Play/Stop, stale completions and partial startup failure.
- UI tests require native input, screenshots and focus/docking/draft assertions.
- Performance tests record representative fixtures, environment and raw samples.

Architecture-policy and CLI integration tests exist today. The other categories become required
with their corresponding implementation, not empty placeholder jobs.

## Evidence

Report exact commands, target, revision (including dirty changes), exit status and
limitations. Skipped tests are not passes. Avoid wall-clock sleeps and uncontrolled
network access in deterministic tests. Flaky tests get a tracked cause, owner and
repair plan, not unbounded retries or silent removal.

## Coverage and deeper analysis

The required foundation baseline is 80% line coverage for workspace Rust code,
measured by the local LLVM command below. No GitHub coverage job is installed
or asserted to pass. This is a coarse floor, not proof of
correctness. Do not lower it to merge code. Critical persistence, identity and
recovery behavior needs explicit negative/fault tests regardless of percentage.

Fuzzing, Miri and benchmark regression gates are not installed as dummy jobs.
Introduce fuzz targets with parsers/untrusted formats, Miri with applicable memory
or concurrency code, and performance thresholds after a repeatable baseline exists.
Future native/browser runtime acceptance needs its own fixtures and runners;
the current desktop harness does not certify an engine or web export.

## Additional repository checks

`cargo xtask check` is the Rust gate, not a wrapper that silently installs every
external tool. Run the additional repository checks locally:

Run standard-library Python native-QA regressions without launching a native
application or requiring a GPU:

```sh
python3 -m unittest discover -s tools/qa -p 'test*.py'
```

Python is an additional repository-check prerequisite, not a requirement for
the basic Rust contributor loop. These tests verify orchestration failures and
evidence rules, not native rendering or physical input.

The portable Rust contracts and feature builds run without Bash/jq or a provider
workflow. The feature task checks each subprocess exit and stops on failure:

```sh
cargo test -p xtask --test portable_contracts --locked
cargo xtask features --non-interactive
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
cargo llvm-cov --workspace --all-targets --all-features --locked --fail-under-lines 80 --lcov --output-path target/coverage.lcov

# Go 1.24+ tools; use the executables in `go env GOPATH`/bin
go install github.com/zricethezav/gitleaks/v8@v8.24.2
```

Run `gitleaks dir . --redact --exit-code 1`
after adding that bin directory to PATH. Secret scanning currently checks the
working tree; it is not a history audit. Run the directory scan in a clean checkout,
because it also examines ignored build artifacts, which can
contain upstream cryptography examples after compiling tools locally. If artifacts
are present, scan a clean Git-inventory source snapshot rather than weakening
detectors or adding secret allowlists. Check local Markdown links
with Lychee 0.24.2 built from source with `cargo install --locked`, in offline/fragment
mode. Remote link availability
does not block PRs because external outages are not contributor code failures.

Tool installation and advisory refresh require network access. Install Go for
Gitleaks and Python 3 for headless QA tests separately. The basic Rust contributor
loop needs Rust, a linker and the public native development packages documented in
[contributing](../CONTRIBUTING.md). Tool pins do not make the entire environment
bit-for-bit hermetic; record OS/toolchain identity with results.

Upstream tool lockfiles are separate from Kedra's dependency audit. The current
Lychee lock emits a yanked `spin 0.9.8` warning and a future-compatibility warning
for `proc-macro-error2 2.0.1`; these do not prevent compilation. Do not remove
`--locked` to silence them. Review tool pins and their upstream dependency changes
explicitly; building from pinned source is not a claim that every tool is vulnerability-free.

There are no hosted tool-install/cache actions. The commands above are standalone
local entry points. See [CI requirements](ci.md) for Windows diagnostics, future
worker isolation and cache requirements, and the failure runbook.
