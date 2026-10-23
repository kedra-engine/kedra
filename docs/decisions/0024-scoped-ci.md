# ADR 0024: Changed-path CI selection and bounded duplication

## Decision

Keep PR/main/merge-group CI entry points and the stable `CI required` check, but
select expensive job groups by changed inputs. Reuse maintained MIT-licensed
`dorny/paths-filter@v4`; do not implement a diff walker, glob matcher or CI engine.
Use its Git discovery mode without pull-request API access or the REST endpoint's
3,000-file limit. No required check is omitted at workflow-trigger level.

Known documentation/Python/workflow-only changes do not need Rust compilation or
native/coverage cache transfers. Unknown inputs select Rust work conservatively.
Embedded crate documentation and non-Markdown assets remain Rust inputs. Manual
dispatch runs all groups. A failed selector cannot authorize a skip: the aggregate
requires valid explicit selections and accepts skips only for unselected groups.

Run formatting/architecture once, native lint/features per OS, and API docs once
on Linux. The required Linux coverage job already executes all workspace targets
with all features, so it also owns Linux unit/integration test execution. Native
Linux runs doctests separately; Windows/macOS retain their native tests/doctests.
Coverage is informational, without a minimum percentage. Workspace tests and
report generation must still succeed. Native behavior acceptance remains separate;
headless coverage does not measure execution of the native scenario binaries.
Windows runs those tests without a diagnostic prebuild; the diagnostic tool is
available for manual use. Portable contract tests remain in the
native suite, rather than also executing in policy. Exercise xtask lint/doc stages
on a small real Cargo fixture using the same configured arguments; do not compile
the entire repository recursively during its own tests. Actual native CI commands
still validate full-workspace CLI execution. The policy cache needs only
the xtask bootstrap target; native caches exclude `target/xtask-cli`.
Use role-specific narrow cache keys. Keep separate dependency-only native/coverage/release
caches and fresh instrumented coverage; do not cache successful check outcomes.

CD audits the all-target application dependency graph once. Retain cargo-deny on
every runner for the isolated collector's separate fresh tool-graph audit before
its compilation. CD then inspects each
actual native package and independent public Git consumer. Portable Python and
Linux source-kit/relink regressions belong to relevant source CI, not every
release rebuild. Retain the native macOS inspector regression in CD because Linux
cannot execute it. Publisher source, artifact completeness/hash and upload checks
are different trust boundaries, not duplicate compiler/test passes.

## Consequences and verification

Path groups are part of the checked workflow configuration and must be updated
when new external inputs are introduced. Test the actual required-check entry
point for selected/unselected groups, malformed selection, failure, cancellation
and accidental skips. Use actionlint/zizmor for workflow correctness and security.
Local validation does not establish hosted event behavior or measured cache/time
savings; PR, push and merge-group acceptance remain necessary before claiming them.
Do not delete remote caches or change storage allowance implicitly. Cache expiry
uses normal retention; manual removal requires separate operational scope.
