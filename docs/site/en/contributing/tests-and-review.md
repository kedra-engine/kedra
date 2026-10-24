# Tests and review

Start with the guarantee you are changing, not a coverage target or a favorite fixture.
Use the [testing guide][testing] to find who owns each test and how to run it.
[ADR 0031][adr] explains test placement and what each kind of check can establish.

## State the test contract first

Before adding a test, write down the production owner, observable failure, least expensive
sufficient environment and why an existing check does not catch it. Name the behavior and
expected result, not the issue number or a generic regression bucket.

For a stale docking candidate, the model owner can compare the published snapshot before
and after rejection. That needs ordinary values, not a window. For a retained projection
that disappears after host transfer, headless ownership assertions may help, but the actual
gesture/window lifecycle needs a separate native scenario. These protect different failures.

| Failure boundary | Sufficient starting environment | What it does not prove |
| --- | --- | --- |
| Invalid candidate changes published model | Owning module's test using ordinary values | Native dragging |
| Plugin registration/retirement breaks ownership | Headless Bevy owner test | Production layout or physical input |
| Failed publication damages sources/output | Owning module's test with real I/O | Editor interaction quality |
| Assembled gesture fails to reach the feature | Existing native scenario | Physical OS certification |
| Reviewed kit control differs in pixels | Explicit component image comparison | Every editor window's appearance |

Do not replay the entire lower-level error matrix through native UI. A higher-level test
needs a distinct integration boundary, not simply more expensive execution.

## Put the regression beside its owner

Private invariants belong in a child `tests` module of the production module. Public
crate contracts use `tests/contracts.rs` and behavioral modules in `tests/contracts/`.
Separate Cargo targets need an isolation/process reason. Do not make private production
APIs public solely to reach them from an integration test.

Shared setup belongs in the narrowest `test_support` or `tests/support` module. Dependencies
point from tests to support to production APIs, never into another suite's `tests` or
`*_tests` module. Editor authored styles/icons belong to editor contracts; importer
contracts own generic processing/publication rather than editor design.

These are existing focused invocation forms, run from the repository root:

```sh
cargo test -p kedra_docking --test contracts --locked
cargo test -p kedra_editor --test contracts --locked styles::
cargo test -p kedra_editor --lib --locked workspace::
```

Choose the package and filter for your change; unrelated editor suites are not required
for every contribution. Report the results of the commands you actually run.

## Exercise rejection and unchanged state

A bug fix needs a test that fails before the fix and passes afterward, including its
failure path. Assert relevant unchanged sources, history, published state and foreign
resources, not just `is_err()`. A rejected operation can still have corrupted its owner.

Use controlled clocks and readiness predicates. Potentially blocking probes need a
killable child and an external deadline independent of the mechanism being tested.
Native fixture changes use complete private copies of the canonical project, preserving
identities; do not change checked fixture sources or personal preferences during a test.

Thin wrappers use real success/failure invocations. Do not construct artificial Cargo
projects merely to prove Clippy/rustdoc, re-test upstream serialization without a Kedra
contract, or add QA-driver/report self-test infrastructure. Keep substantive negative
checks for repository policy and public build/package/run behavior.

## Run the review checks in the right category

The Rust review route is:

```sh
cargo xtask check
cargo xtask features --non-interactive
```

Run commands separately, stop on failure and include relevant feature/target/consumer
checks. `check` runs file growth, architecture, formatting, Clippy, all-target/all-feature
tests, a separate doctest run and API documentation. `features` separately checks default,
minimal and isolated runtime builds plus the shipping diagnostic boundary. It is not
already included in `check`.

The runner streams diagnostics, prints stage summaries and writes `target/xtask/logs/`.
Later skipped stages are not passes. Bootstrap failure or forced termination may leave no
complete log/summary. CI and redirected streams do not prompt; an explicit task avoids
an interactive menu. Repair the owner failure rather than suppressing checks.

Markdown/links, documentation-site validation, dependency/advisory/license checks and
secret scanning are separate. Use the pinned tools and exact routes in the
[testing guide][testing] and [CI guide][ci], including their install/network prerequisites.
Do not widen exceptions, change approved image baselines or weaken policy to get green.

Coverage is informational with no minimum percentage, but fresh tests/measurement and a
nonempty report must succeed. Native scenario source in coverage does not mean its native
workflows executed. Do not add tests solely to raise the percentage.

## Record native evidence honestly

Native UI changes require actual input and frames on the claimed setup. The existing
[native scenarios](../manual/testing/native-scenarios.md) need an idle desktop and explicit
preparation; read their prerequisites before launching them. An unavailable desktop or
unrun scenario means native acceptance is unverified.

Synthetic geometry is not production layout. RPC observes state; renderer readback proves
delivery, not correct image content unless a pixel assertion checks it. Injected Bevy
input does not certify physical OS events, IME or mixed-DPI monitors. Screenshot coverage
is limited to the reviewed 22 button/input kit cases, not full-window feature acceptance.

## Hand off reviewable evidence

Report revision including dirty changes, exact commands, environment/target, exit status,
artifact locations, skipped/interrupted checks and remaining risks. Keep generated logs
under `target/`, redact sensitive data before sharing and avoid permanent run diaries in docs.

The [quality policy][quality] and [contribution policy][contributing] own merge requirements.
The configured `CI required` gate is fail-closed for selected work; source configuration
does not prove hosted success or live protections. Local checks, native acceptance and
human design review remain separate. Follow the [contribution handoff](index.md) for approval.

[testing]: contracts/testing.md
[adr]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0031-contract-owned-testing.md
[ci]: https://github.com/kedra-engine/kedra/blob/main/docs/ci.md
[quality]: https://github.com/kedra-engine/kedra/blob/main/docs/quality.md
[contributing]: https://github.com/kedra-engine/kedra/blob/main/CONTRIBUTING.md
