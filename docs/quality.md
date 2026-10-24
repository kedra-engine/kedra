# Quality and validation policy

## Required change validation

The workflow configures validation and a fail-closed `CI required` gate. Run the
local commands below and verify hosted results and branch protection on GitHub.
See [testing](site/en/contributing/contracts/testing.md),
[CI requirements](ci.md) and [hosting](hosting.md).

Product contracts stay with their production owners under
[ADR 0031](decisions/0031-contract-owned-testing.md). Shared fixtures are support,
not other test suites; repository policy and native acceptance remain separate
evidence categories. Thin utilities use real invocations, not upstream-tool self-tests.

- Rustfmt, warnings-as-errors Clippy, tests, doctests and rustdoc via `cargo xtask check`.
- Architecture policy and negative tests, workspace lint inheritance and the
  independent Clippy `-F unsafe_code` guard.
- Default/minimal workspace and isolated native-runtime builds via `cargo xtask features`.
  This includes minimal scene/asset/UI libraries and shipping diagnostic-absence
  tests in the unoptimized, debug-assertions-disabled `release-boundary` profile.
  Each Cargo process is checked separately; a later success cannot hide a failure.
- File growth: 1000 lines per authored UTF-8 file and 64 KiB per repository file,
  including comments/blank lines. Byte limits also catch compressed one-line code.
  Images have no path, extension or directory exemption from the byte limit.
- Dependency license/advisory/source policy via cargo-deny; Gitleaks secret scanning.
- Markdown lint through rumdl and local links through Lychee.
- Documentation-site frozen install/audit, EN/RU page pairs, VitePress build,
  bundled-license validation and generated links/fragments; see
  [documentation infrastructure](documentation-infrastructure.md).
- QA workflows are verified by real product scenarios, native input and GPU fences.
  Screenshot coverage is only kit buttons/inputs; other behavior checks publish no PNGs.
  There is no separate self-test suite for QA drivers/fixture hosts/reports.
  Product Rust unit, integration and regression checks remain mandatory.
- Fresh workspace Rust line coverage with a retained informational report. There
  is no minimum percentage or coverage-based merge threshold. Test and measurement
  failures still fail validation; critical persistence/identity/recovery need fault tests.
- Linux, Windows and macOS verification for platform claims, with actual
  target/toolchain recorded. Native UI acceptance requires real input and frames.

Missing, failed, cancelled or unexpectedly skipped required work is not success.
The local runner stops on failure, reports remaining stages as skipped and keeps
diagnostics. The hosted `CI required` job consumes `tools/ci/required.py`;
the `python-test` job (`Workflow and distribution tooling contracts`) runs its
malformed/missing-result and failure-path tests against that implementation.

## Security and review boundaries

Validation uses no private infrastructure or credentials. Untrusted contributions
must not execute with secrets or trusted worker/cache write access. Pin external
tool versions and use their upstream lockfiles. Local Lychee installation builds
an exact crates.io release; hosted CI uses the pinned Aqua tool through Mise with
checksum verification. Review future automation,
worker isolation and permissions before enabling it. Release credentials/signing
belong in a separately approved protected workflow.

Local checks alone do not protect the hosted default branch. An administrator
must verify destination review/protection settings and reporting channels.
Configured source workflows and the expected administrative contract do not prove
current live server settings or hosted results.

## Progressive guarantees

Checks verify the existing harness, not future engine behavior. New subsystems
extend tests, supported feature combinations and acceptance evidence. Neither
arbitrary complexity ceilings nor a 100% coverage badge replaces maintainable
modules and reviewed behavior.

## File growth enforcement

`cargo xtask files` uses Git's tracked and non-ignored untracked file inventory.
Tracked files remain checked even if added to `.gitignore`; NUL-separated paths
handle spaces and line breaks without a custom filesystem walker. Missing Git,
unreadable files and unsupported symlinks/submodules fail the gate.
Git-reported unstaged deletions are accepted; the new file in an unstaged rename
is still checked. Missing files not reported as deletions remain an error.

Only exact root paths `Cargo.lock`, `LICENSE-MIT` and `LICENSE-APACHE` are exempt:
generated resolution and canonical legal text must not be arbitrarily split.
Binary assets are subject to the byte cap until a separate reviewed asset policy
exists. Exceptions/threshold changes require explicit owner review; do not split
responsibilities into meaningless numbered fragments or minify code.

Changes to automation, policy tools, architecture rules and AI instructions are
security-sensitive and require explicit review. An AI reviewer is supplementary,
never the only required approver. New baselines and exceptions require rationale.
