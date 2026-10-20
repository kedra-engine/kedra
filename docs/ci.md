# Local validation and future CI requirements

## Current status

The repository is moving to <https://github.com/kedra-engine/kedra>.
No GitHub CI configuration, runner fleet, scheduled audit or server-side
required-check enforcement has been installed or verified. Local commands remain
the validation entry points; there is no hosted green-status claim for this tree.
Future self-hosted Yandex Cloud workers are a possibility, not created resources
or an approved deployment. Do not infer provisioning authority from this guide.

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

## Requirements for future automation

- Validate each reviewed change and the merged state on Linux, Windows and macOS.
  No path skips or ignored failures may weaken required checks. One broken
  platform must not suppress diagnostics from the others.
- Retain the complete validation map, fresh >=80% Rust line coverage, and refreshed
  dependency advisories. Coverage is a floor, not proof of correctness.
- An aggregate must reject empty/malformed/missing results as well as failures,
  cancellations and unexpected skips. Test the actual configured aggregate's
  negative paths when a provider adapter exists; no unused placeholder gate exists.
- Separate untrusted execution from secrets, publishing and trusted cache writes.
  Use least privilege, reviewed/pinned external tools and isolated disposable
  workers. Self-hosted execution needs an explicit threat model and cleanup proof.
- Caches are optional acceleration, never cached test/coverage/advisory conclusions.
  Separate platform/compiler/lockfile/tool/coverage roles; untrusted contributions
  cannot seed trusted caches. Verify cold builds and cache isolation before relying
  on them. Do not cache credentials or upload whole Cargo target directories.
- Define timeouts, storage/concurrency budgets and diagnostic retention only after
  selecting and measuring workers. Preserve logs even on failure where possible;
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
