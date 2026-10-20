# Quality and validation policy

## Required change validation

GitHub CI is not configured. These are required validation policies with local
commands, not claims of installed server enforcement or passed coverage thresholds.
See [testing](testing.md), [CI requirements](ci.md) and [hosting](hosting.md).

- Rustfmt, warnings-as-errors Clippy, tests, doctests and rustdoc via `cargo xtask check`.
- Architecture policy and negative tests, workspace lint inheritance and the
  independent Clippy `-F unsafe_code` guard.
- Default/minimal workspace and isolated native-runtime builds via `cargo xtask features`.
  Each Cargo process is checked separately; a later success cannot hide a failure.
- File growth: 1000 lines per authored UTF-8 file and 64 KiB per repository file,
  including comments/blank lines. Byte limits also catch compressed one-line code.
- Dependency license/advisory/source policy via cargo-deny; Gitleaks secret scanning.
- Markdown lint through rumdl and local links through Lychee.
- Headless Python QA regressions for orchestration and failure reporting.
- Fresh workspace Rust line coverage >=80% with a retained report, not a lower
  threshold to merge. Critical persistence/identity/recovery still need fault tests.
- Linux, Windows and macOS verification for platform claims, with actual
  target/toolchain recorded. Native UI acceptance requires real input and frames.

Missing, failed, cancelled or unexpectedly skipped required work is not success.
The local runner stops on failure, reports remaining stages as skipped and keeps
diagnostics. Future hosted aggregation must preserve fail-closed semantics and
exercise malformed/missing-result cases against its actual implementation.
No provider-only aggregate script is retained without a consumer.

## Security and review boundaries

Validation uses no private infrastructure or credentials. Untrusted contributions
must not execute with secrets or trusted worker/cache write access. Pin external
tool versions and use their upstream lockfiles. Lychee is built from an exact
crates.io release, not a mutable release executable. Review future automation,
worker isolation and permissions before enabling it. Release credentials/signing
belong in a separately approved protected workflow.

Local checks alone do not protect the hosted default branch. An administrator
must verify destination review/protection settings and reporting channels.
No current setting or future worker deployment is asserted by repository docs.

## Progressive guarantees

Checks establish harness properties, not future engine behavior. New subsystems
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
