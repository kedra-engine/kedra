# Local validation and GitHub CI

## Current status

Repository: <https://github.com/kedra-engine/kedra>. The
[workflow](../.github/workflows/ci.yml) runs PR/main validation on managed
GitHub-hosted runners. The initial complete hosted run passed on all three OSes;
fresh line coverage was 85.02%. Same-revision warm-cache acceptance and genuine
protected-main PR acceptance are verified. The repaired-main baseline at
`e58d090249e5539408a5019fdd968026de4143be` passed every hosted group in run
39503345256. New distribution changes still require their own hosted acceptance.
Local commands remain independent entry points.
Releases, Kubernetes, private infrastructure and self-hosted runners are outside
this workflow. The exact Winit fork revision is public on GitHub; see
[source provenance](third-party.md#source-provenance-and-publication).

## Workflow and required result

PRs targeting main, main pushes and merge queue events run without path filters.
The seven job groups are `policy`, `native`, `coverage`, `supply-chain`,
`repository`, `secrets` and `python-qa`. Native validation runs on ubuntu-24.04,
windows-2025 and macos-15 with matrix fail-fast disabled. Clippy, full workspace
tests, doctests, API docs and default/minimal/isolated-runtime features are not
replaced by compilation-only checks. The OS labels are managed images, not a
bit-for-bit reproducible OS pin; Rust is selected by `rust-toolchain.toml`.

Native and coverage jobs wait for the lightweight policy/workflow checks, avoiding
expensive compilation for a known policy failure. The matrix platforms remain
independent of each other's failures.

`CI required` always evaluates all seven expected job-group results, including
the native matrix result. It succeeds only if every result is `success`; missing,
extra, failed, cancelled, malformed and skipped results fail closed. The actual
configured gate entry point has positive and negative behaviour tests:

```sh
python3 -m unittest discover -s tools/ci -p 'test*.py'
```

No YAML parser, configuration snapshot tests or CI executor is added. The small
Python gate enforces Kedra's exact expected results; GitHub owns scheduling and
execution. actionlint and zizmor validate workflow syntax and security directly.

Native and coverage jobs allow 180 minutes; light jobs allow 30 minutes and the
final gate five minutes. GitHub documents a six-hour hosted-job execution maximum.
These budgets do not imply measured runtimes. Superseded PR runs are cancelled;
accepted main runs are not cancelled just because a newer main exists. Compilation
uses nonincremental, line-table debug profiles and four jobs on Linux/macOS, one
on Windows to bound simultaneous MSVC linking. Local developer defaults do not change.

## Cache and permission boundaries

The reviewed `Swatinem/rust-cache@v2` action restores Cargo registry/Git
sources and dependency compilation artifacts. Its upstream cleanup removes
workspace outputs, test binaries, incremental artifacts and raw profile files;
Kedra explicitly disables workspace/all-crate and cargo-bin caching. Do not replace
it with an unfiltered target or CARGO_HOME archive. Cache keys include the action's
compiler identity, lock/manifests/config and Cargo/Rust environment hashes, plus
the managed image's ImageOS/ImageVersion,
an explicit OS/architecture and native versus coverage role. Coverage additionally
pins the llvm-cov tool role and uses a separate `target/llvm-cov-target` cache.

Only successful main-push jobs save shared caches; PR jobs restore without saving.
GitHub's branch scope keeps PR-created caches out of main. Tests, coverage profiles,
advisory refresh and secret scans always run anew. Native caching does not certify
the correctness of artifacts. Misses must build normally. Cache restores are
optional acceleration, never permission to skip a check.

The initial cache budget stays within GitHub's default 10 GB per repository;
eviction can reduce reuse and must be observed before performance claims. No paid
increase is implicit. Logs/LCOV and bounded Windows diagnostics are retained for
seven days, including after failure when available. Successful coverage requires
a nonempty LCOV; absence after an earlier failure does not turn the job green.

All jobs have only `contents: read`, checkout does not persist its token, and no
corporate secrets or publishing/OIDC permissions are supplied. External actions
use owner-selected floating stable major tags: checkout/setup-python/upload-artifact
v7, rust-cache/install-action v2 and mise-action v5. These tags can move; they are not
immutable source identities. The narrow zizmor ref policy covers only these six reviewed
actions; all other security audits remain enabled. Tool installs use pinned
versions and checksum verification without an automatic source-build fallback.
Mise 2026.10.15 uses explicit Aqua backends for actionlint, zizmor, rumdl, lychee
and gitleaks; its signed binary checksums and upstream tool checksum definitions
are checked by maintained installers. Rust Cargo tools use install-action.
Mise tool caches save only on main pushes and do not persist GitHub credentials.
Untrusted PR execution uses
`pull_request`, never a privileged `pull_request_target` checkout. Compilation and
release/publishing remain separate. Do not put confidential data into public caches
or logs. Retain and review tool/license/security pins when updating actions.

## Local validation map

Install the public native prerequisites in [contributing](../CONTRIBUTING.md)
and the pinned repository tools in [testing](testing.md). Run each command as a
separate checked process; stop on its nonzero status. In PowerShell, check
`$LASTEXITCODE` after every native command rather than returning only the last one.

| Check | Local command |
| --- | --- |
| File growth, architecture, fmt, lint, tests/doctests, API docs | `cargo xtask check --non-interactive` |
| Default/minimal features and independent runtime | `cargo xtask features --non-interactive` |
| Bootstrap/native dependency contracts | `cargo test -p xtask --test portable_contracts --locked` |
| Headless native-QA failure paths | `python3 -m unittest discover -s tools/qa -p 'test*.py'` |
| Licenses/advisories/sources | `cargo deny --locked check` |
| Markdown | `rumdl check .` |
| Local links | `lychee --offline --include-fragments --no-progress './*.md' './docs/**/*.md'` |
| Secrets | `gitleaks dir . --redact --exit-code 1` on a clean source snapshot |
| Coverage | `cargo llvm-cov --workspace --all-targets --all-features --locked --fail-under-lines 80 --lcov --output-path target/coverage.lcov` |

`features` runs three separate Cargo checks with streamed diagnostics, logs and
fail-closed stage aggregation: workspace default, workspace no-default-features,
then `kedra_app --lib --no-default-features`. The isolated runtime check catches
features accidentally supplied by editor/dev dependency unification, including
production Linux Winit backends. No skipped stage is reported as passed.
All-features testing alone is not a substitute for these checks.

## Initial hosted measurements

Reference: [CI run 39484899924, attempt 1][initial-ci],
commit `ecc155809deedf94b6b6c552813ee04aad6ef8cc`. Job durations include setup,
checks, artifact upload and cache save; they are not compilation-only timings.

| Job | Cold duration | Warm duration (same SHA, attempt 2) |
| --- | --- | --- |
| Native Linux x86_64 | 12m 18s | 5m 10s |
| Native macOS arm64 | 17m 43s | 8m 04s |
| Native Windows x86_64 | 54m 36s | 18m 19s |
| Fresh Linux coverage | 10m 03s | 5m 22s |

LCOV contains 23,535 covered lines of 27,683 (85.02%). Eight dependency/tool caches
occupied 5.22 GiB after the cold run and 5.51 GiB after the warm run, below the
default allowance. Warm logs show exact Rust dependency cache hits for all native
and coverage jobs; fresh warm coverage remained 85.02%. Policy's Rust cache safely
missed due to changed preinstalled secondary compiler identity. Windows cold
latency remains substantial and must not be described as fast.
This is headless Rust/QA acceptance, not native UI or packaged-editor acceptance.

[initial-ci]: https://github.com/kedra-engine/kedra/actions/runs/39484899924/attempts/1

## Windows build diagnostics

Keep `tools/ci/windows-build.ps1` and its success/failure/sampling regression suite:

```powershell
$env:CARGO_BUILD_JOBS = '1'
$env:CARGO_PROFILE_DEV_DEBUG = 'line-tables-only'
$env:CARGO_PROFILE_TEST_DEBUG = 'line-tables-only'
pwsh -NoProfile -File tools/ci/test-windows-build.ps1
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pwsh -NoProfile -File tools/ci/windows-build.ps1
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo xtask check --non-interactive
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo xtask features --non-interactive
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
```

The wrapper builds all-feature test executables with `--no-run --timings --verbose`;
it does not replace tests/doctests. It preserves Cargo's exit status and samples
compiler/linker CPU/memory, system memory and disk every 15 seconds, including an
initial synchronous sample, without dumping environment variables. Evidence is in
`target/ci/windows-build/` and, when Cargo produces it, `target/cargo-timings/`.
Retain evidence before subsequent tests and after failures; forced termination
can prevent a final sample. One build job reduces competing MSVC links; it does
not serialize tests. Line-table debug profiles retain source locations, not full
interactive variable/type inspection; use separate full-debug builds for that.
These are optional local resource settings, not changed workspace defaults or
measured completion guarantees.

## Acceptance requirements

- Validate each reviewed change and the merged state on Linux, Windows and macOS.
  No path skips or ignored failures may weaken required checks. One broken
  platform must not suppress diagnostics from the others.
- Retain the complete validation map, fresh >=80% Rust line coverage, and refreshed
  dependency advisories. Coverage is a floor, not proof of correctness.
- An aggregate must reject empty/malformed/missing results as well as failures,
  cancellations and unexpected skips. Test the actual configured aggregate's
  negative paths in the configured entry point; an unused helper is not evidence.
- Separate untrusted execution from secrets, publishing and trusted cache writes.
  Use least privilege, reviewed/pinned external tools and isolated disposable
  workers. Self-hosted execution needs an explicit threat model and cleanup proof.
- Caches are optional acceleration, never cached test/coverage/advisory conclusions.
  Separate platform/compiler/lockfile/tool/coverage roles; untrusted contributions
  cannot seed trusted caches. Verify cold builds and cache isolation before relying
  on them. Do not cache credentials or upload whole Cargo target directories.
- Define timeouts, storage/concurrency budgets and diagnostic retention only after
  measuring workers. The initial budgets above remain subject to acceptance.
  Preserve logs even on failure where possible;
  scrub sensitive data before sharing. No former provider quota or timeout is a
  verified destination guarantee.
- Native UI requires real input and rendered frames from controlled graphical
  environments. Rust/headless QA checks do not certify that workflow or web/mobile.
- Establish human review and branch rules separately; see [hosting](hosting.md).

## Failure runbook

| Symptom | Action |
| --- | --- |
| Compiler/test failure | Inspect the failing stage and `target/xtask/logs/`; reproduce the exact command/platform |
| Tool installation failure | Check the pin, upstream lock/compiler and registry availability; keep `--locked` |
| Advisory/license failure | Fix the dependency or review a narrow documented exception; never lower policy |
| Secret finding | Do not post the secret; verify privately, rotate credentials and assess history |
| Suspected corrupt cache | Reproduce without that cache; obtain owner approval before deleting remote state |
| Missing/skipped check | Treat it as unverified, not a pass; repair orchestration |
| Native flake | Retain input/frame evidence and track the cause; no blind retry-to-green |

Record exact source identity, platform/toolchain, commands, results and limitations.
A result from another revision does not verify the current source.
