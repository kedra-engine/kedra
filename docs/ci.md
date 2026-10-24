# Local validation and GitHub CI

## Current status

Repository: <https://github.com/kedra-engine/kedra>. The
[workflow](../.github/workflows/ci.yml) configures PR/main validation on managed
GitHub-hosted runners. Its configuration does not establish current hosted results,
cache performance or live branch-protection enforcement. These require external
verification for the source being accepted; native UI and distribution acceptance
remain separate.
Local commands remain independent entry points.
Releases, Kubernetes, private infrastructure and self-hosted runners are outside
this workflow. The exact Winit fork revision is public on GitHub; see
[source provenance](third-party.md#source-provenance-and-publication).

## Workflow and required result

PRs targeting main, main pushes and merge queue events always create `CI required`;
workflow-level path filters do not leave branch protection waiting for a missing
check. A lightweight `changes` job uses maintained `dorny/paths-filter@v4` and
[path groups](../.github/ci-paths.yml) to select work. Manual CI dispatch selects
every group. Detection uses the action's Git mode, not the file-count-limited PR
REST endpoint. PRs compare the checked-out merge commit with the event's base SHA;
main pushes compare against their before SHA, and merge groups against their base SHA.

| Change | Selected work, in addition to file policy and history secret scan |
| --- | --- |
| Root/docs Markdown and known documentation-site inputs | Site build/audit, Markdown and links; no Rust matrix, coverage or native cache transfer |
| Python tools only | Python suites; no Rust matrix, coverage or native cache transfer |
| `.github/` only | Workflow syntax/security and Python gate/tool tests |
| Rust, manifests, locks, assets, native scripts or unknown inputs | Native matrix, fresh coverage and dependency audit |
| Mixed changes | Union of the corresponding groups |

JSON/PNG changes to QA policies and reference images select repository Rust checks
and informational coverage, not native screenshot acceptance. Run the relevant
component scenario explicitly on an idle desktop or dispatch the Native UI corpus
workflow. Ordinary headless CI cannot replace its real input/frame evidence or
approve a changed profile.

Markdown embedded under crates is a Rust input, not a documentation-only exception.
Unknown paths conservatively select Rust checks. Native validation runs on
ubuntu-24.04, windows-2025 and macos-15 with fail-fast disabled. Clippy, workspace
tests/doctests and default/minimal/isolated-runtime features run on each selected
OS. The feature route also checks isolated minimal scene/asset/UI libraries and
executes shipping diagnostic-absence contracts with debug assertions disabled,
without an optimized editor release build. Linux workspace unit/integration tests
run once in the required coverage job,
not again in native Linux; doctests remain in native Linux because coverage does
not run them. Coverage has no minimum percentage; tests and report generation
must still succeed. Native behavior and screenshot acceptance require separate
real-input/frame runs. API documentation runs once on Linux.
Rust formatting/architecture run once in policy, and portable-contract tests are
included in workspace tests.
No compilation-only prebuild replaces or precedes the Windows test execution.
The xtask command-stage regression uses a tiny real Cargo workspace, rather than
running full-workspace lint/docs recursively into a second `target/xtask-cli`.
CLI missing-tool failures and the real unsafe-code rejection fixture remain
covered; native CI still executes the actual full-workspace xtask commands.
The OS labels are managed images, not a bit-for-bit reproducible OS pin; Rust is
selected by `rust-toolchain.toml`.

Native and coverage jobs wait for the lightweight policy/workflow checks, avoiding
expensive compilation for a known policy failure. The matrix platforms remain
independent of each other's failures.

`CI required` invokes `python3 -I tools/ci/required.py` and evaluates eight dependency
results: `changes` plus seven validation groups (`policy`, `native`, `coverage`,
`supply-chain`, `repository`, `secrets` and `python-test`). The `python-test` job is
named `Workflow and distribution tooling contracts`. Selected
jobs must succeed; unselected jobs must be `skipped`. Failed change detection,
missing/extra results, malformed selection, failures, cancellations and unexpected
skips fail closed. The `python-test` job tests all selection combinations and
their failure paths against the configured gate entry point:

```sh
python3 -m unittest discover -s tools/ci -p 'test*.py'
```

Path discovery and matching use the maintained action, not a custom diff walker
or glob matcher. The Python gate enforces Kedra's expected results; GitHub owns scheduling and
execution. actionlint and zizmor validate workflow syntax and security directly.

Native and coverage jobs allow 180 minutes; policy, supply-chain, repository,
secrets and Python jobs allow 30 minutes. Change selection and the final gate
allow five minutes. GitHub documents a six-hour hosted-job execution maximum.
These budgets do not imply measured runtimes. Superseded PR runs are cancelled;
accepted main runs are not cancelled just because a newer main exists. Compilation
uses nonincremental, line-table debug profiles and four jobs on Linux/macOS, two
on Windows to bound simultaneous MSVC linking. Local developer defaults do not change.
Hosted timing and memory acceptance remain unverified. Release compilation also
uses two Windows jobs without changing its optimization, codegen-unit or Thin LTO settings.

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

Coverage uses cargo-llvm-cov 0.7.1 to run fresh headless workspace tests and retain
LCOV under `target/coverage.lcov`. Its percentage is a reference metric, not a merge
threshold. Test failures, measurement errors and missing/empty LCOV still fail the
job; a low percentage does not. No source exclusions or ignored failures are added.
Native scenario code stays in the workspace denominator, but this headless run
does not execute those binaries. See
[measurement scope](site/en/contributing/contracts/testing.md#coverage-and-future-analysis).

Only main-push jobs may save shared CI caches; PR jobs restore without saving.
The policy cache includes only `target/xtask-bootstrap`, not a separate workspace
test target. Policy uses the `policy-bootstrap-v2` key; native uses `native-lean-v2`
for `target` and `target/xtask-bootstrap`. Native/coverage/distribution caches remain
separate because their compiler profiles and consumers differ; docs/Python-only
runs never restore or save them.
Native jobs enable the action's supported `cache-on-failure` option, retaining
compiled dependencies even when later validation fails. Policy and coverage jobs
still save only on success. Main-ref distribution jobs also retain dependency-only
caches after failure, with packages and independent consumers outside cache paths.
The action filters and saves in its job post-step, not immediately after compilation;
cancellation or runner termination can still prevent saving. Failed checks remain
failed, and restoring dependencies never reuses their previous test outcomes.
GitHub's branch scope keeps PR-created caches out of main. Selected tests, coverage
profiles and advisory checks run anew, as does the always-on history secret scan.
Native caching does not certify artifact correctness. Misses must build normally. Cache restores are
optional acceleration, never permission to skip a check.

Cache use is subject to GitHub's default 10 GB per-repository allowance;
eviction can reduce reuse and must be observed before performance claims. No paid
increase is implicit. Logs/LCOV, public coverage-scenario reports, software-renderer
diagnostics and bounded Windows diagnostics are retained for seven days, including
after failure when available. Coverage uploads omit private sessions/credentials.
Successful coverage requires a nonempty LCOV; absence after an earlier failure does
not turn the job green.
Changing `CARGO_BUILD_JOBS` or the managed image can invalidate cache reuse.

Jobs have `contents: read`, checkout does not persist its token, and no
corporate secrets or publishing/OIDC permissions are supplied. External actions
use owner-selected floating stable major tags: checkout/setup-python/upload-artifact
v7, rust-cache/install-action v2, mise-action v5 and paths-filter v4. These tags can
move; they are not immutable source identities. Release download-artifact uses
reviewed stable v8.
The narrow zizmor ref policy covers only these reviewed
actions; all other security audits remain enabled. Tool installs use pinned
versions and checksum verification without an automatic source-build fallback.
Mise 2026.10.15 uses explicit Aqua backends for actionlint, zizmor, rumdl, lychee
and gitleaks, plus pinned Node 24.22.1 and Bun 1.3.13 for documentation;
its signed binary checksums and upstream tool checksum definitions
are checked by maintained installers. Rust Cargo tools use install-action.
Path selection uses Git without pull-request API permissions. The history secret
scan covers the clean checkout's tracked source as well, without repeating a
directory scan.
Mise tool caches save only on main pushes and do not persist GitHub credentials.
Untrusted PR execution uses `pull_request`, never a privileged `pull_request_target`
checkout. Compilation and
release/publishing remain separate. Do not put confidential data into public caches
or logs. Retain and review tool/license/security pins when updating actions.

## Local validation map

Crates.io library publication is a separate, manually dispatched
[workflow](../.github/workflows/crates.yml), with credentialless archive verification
on all three desktop runners and a main-only upload job. It defaults to dry-run;
see [publication admission, credentials and recovery](crates-publication.md).

Install the public native prerequisites in [contributing](../CONTRIBUTING.md)
and the pinned repository tools in [testing](site/en/contributing/contracts/testing.md).
Run each command as a separate checked process; stop on its nonzero status. In PowerShell, check
`$LASTEXITCODE` after every native command rather than returning only the last one.

| Check | Local command |
| --- | --- |
| File growth, architecture, fmt, lint, tests/doctests, API docs | `cargo xtask check --non-interactive` |
| Default/minimal features and independent runtime | `cargo xtask features --non-interactive` |
| Bootstrap/native dependency contracts | `cargo test -p xtask --test portable_contracts --locked` |
| Native buttons/fields screenshots (idle desktop, reviewed profile) | `cargo xtask test -- components` |
| Licenses/advisories/sources | `cargo deny --locked check` |
| Markdown | `rumdl check .` |
| Local links | `lychee --offline --include-fragments --no-progress --root-dir "$PWD/docs/site" './*.md' docs` |
| Documentation site, EN/RU pairs, bundled licenses and generated links | [Documentation infrastructure](documentation-infrastructure.md#local-validation) |
| Secrets | `gitleaks dir . --redact --exit-code 1` on a clean source snapshot |
| Coverage | [Fresh informational workspace measurement](site/en/contributing/contracts/testing.md#coverage-and-future-analysis), without a percentage threshold |

The `repository` job also audits the frozen documentation dependency graph, builds
the bilingual site and checks generated HTML links/fragments. It retains a
seven-day preview artifact, without deployment or publishing permissions. Known
site inputs select this job through the Markdown output; the required gate uses
the job identities and skip policy described above. See
[documentation infrastructure](documentation-infrastructure.md).

The same repository job validates the separate landing, its frozen dependency graph,
build contracts and generated links. After `CI required` succeeds, official main CI
runs the [site publication jobs](../.github/workflows/ci.yml). The build job has
no cloud credentials; only the environment-scoped upload step receives the two Object
Storage secrets. Fork/PR checks and the required gate do not depend on those secrets.
See [hosting](../tools/hosting/README.md) for source freshness, domain and permission rules.

`features` runs eight separate Cargo invocations with streamed diagnostics, logs and
fail-closed stage aggregation: workspace default and no-default-feature all-target checks;
isolated no-default-feature library checks for `kedra_app`, `kedra_script`, `kedra_scene`,
`kedra_assets` and `kedra_ui`; and `kedra_app`'s `launch::tests` under the
debug-assertions-disabled `release-boundary` profile. The isolated checks detect capabilities
accidentally supplied by workspace feature unification, including production Linux Winit
backends. No skipped stage is reported as passed. All-features testing alone does not
replace this route.

## Windows build diagnostics

`tools/ci/windows-build.ps1` is an optional manual diagnostic command with a
success/failure/sampling regression suite. Normal CI runs the regression suite
and actual tests without a separate compilation-only prebuild:

```powershell
$env:CARGO_BUILD_JOBS = '2'
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
can prevent a final sample. Two build jobs bound competing MSVC links; this does
not serialize tests. Line-table debug profiles retain source locations, not full
interactive variable/type inspection; use separate full-debug builds for that.
These are optional local resource settings, not changed workspace defaults or
measured completion guarantees.

## Acceptance requirements

- Validate Rust-affecting changes and their merged state on Linux, Windows and
  macOS. Only the declared changed-path plan may skip work; unexpected skips and
  ignored failures remain errors. One broken platform must not suppress the others.
- Retain the complete validation map, a fresh informational Rust line coverage
  report and refreshed dependency advisories for Rust-affecting changes. No previous
  test/coverage outcomes are cached. Coverage has no minimum and is not proof of correctness.
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
  measuring workers. The configured budgets above remain subject to acceptance.
  Preserve logs even on failure where possible;
  scrub sensitive data before sharing. Configured budgets are not performance
  guarantees.
- Native UI requires real input and rendered frames from controlled graphical
  environments. Rust/headless tooling checks do not certify that workflow or web/mobile.
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
