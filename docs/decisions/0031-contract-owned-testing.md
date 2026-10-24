# ADR 0031: Contract-owned tests and explicit acceptance boundaries

Status: accepted. Applies to product tests, native acceptance and repository checks.

## Decision

Organize verification by the Kedra contract and its production owner, not by the
history of a fix, the upstream library used, or whichever fixture is convenient.
The execution environment is a separate choice: use the least expensive environment
that can detect the relevant failure. Cargo remains the Rust test runner; ordinary
Rust functions and `kedra_test` remain the native acceptance mechanism. Do not add
a test registry, scenario language, forwarding crate or universal fixture framework.

Three responsibilities remain distinct:

- **Product contracts:** sources, identities, transactions, runtime ownership,
  rendering, editor coordination and the public build/package/update workflow.
- **Acceptance:** assembled application workflows, actual input/frame delivery and
  the explicitly reviewed component screenshot corpus.
- **Repository policy:** dependency boundaries, file growth, security, CI admission
  and release authorization. These checks do not count as editor feature acceptance.

Each contract has a primary owner. Lower levels exercise domain/error matrices;
higher levels verify a different integration boundary or the assembled result,
not the same matrix again. For docking, `kedra_docking` owns topology, `kedra_ui`
owns retained projection, the editor owns document/focus coordination, and native
scenarios own gesture delivery and application lifecycle.

## Placement and dependencies

- Private invariants belong in a module's child `tests` module. Split larger suites
  by behavior, not numbered fragments or historical `regression` buckets.
- Public crate contracts use a `tests/contracts.rs` Cargo target with behavioral
  modules under `tests/contracts/`. Separate targets require an isolation reason.
- Reusable fixtures belong in the narrowest `test_support` or `tests/support`
  module. Tests may depend on support and production APIs, never another test suite.
  Do not make production APIs public only to accommodate test placement.
- Editor styles, icons and authored compositions belong to editor contracts.
  Importer tests own generic admission, processing and publication, not editor design.
- Native editor, runtime-consumer and component scenarios have separate source
  modules in the existing runner. The canonical project retains stable identities;
  each mutating scenario operates on its own complete copy.

Test names describe behavior and expected outcome. A bug fix extends the relevant
contract suite; a regression is not a new architectural category. A short module
description states the owner, environment and guarantee. No per-test registry or
duplicated catalogue of every function is required.

## Scope of assertions

Test Kedra's observable result, rejection and ownership, not Bevy/Cargo/serde's
documented behavior in isolation. A narrow upstream compatibility probe requires
an explicit Kedra dependency contract and a reason ordinary product tests cannot
protect it. Thin command wrappers are verified by real success/failure invocations;
do not construct separate workspaces merely to prove Clippy or rustdoc works.
Build/package/run is a product capability, not disposable wrapper infrastructure.
Preserve its real independent-consumer and source-safety checks.

Repository-specific policy still needs negative cases: missing CI jobs, forbidden
dependency edges and broadened security exceptions must fail closed. Keep policy
implementation separate from fixtures used to exercise it. Do not create a parallel
QA-driver/report self-test framework. The existing comparator's acceptance guards
remain substantive checks, as required by ADRs 0027 and 0028.

Synthetic geometry proves response to geometry, not production layout. RPC state
proves state, not pixels. GPU readback proves delivery, not correct scene content.
Injected Bevy input does not certify physical OS input. Keep these boundaries in
scenario reports. Changes to screenshot scope, references or accepted differences
still need explicit review; this decision does not expand visual acceptance.

Negative tests verify the relevant unchanged sources, history, published state and
foreign resources, not only `is_err()`. Use controlled clocks/readiness predicates.
Potentially blocking probes need a killable child and an external deadline that
does not rely on the mechanism under test. Fixtures must not replace the operation
being tested, read personal preferences or write canonical project sources.

## Maintenance and verification

The [testing guide](../site/en/contributing/contracts/testing.md)
is the single navigation entry point: ownership,
placement, execution routes and evidence limits. It maps capabilities to suites,
not every individual test. AI contributors follow the same contract in `AGENTS.md`.

Reorganization preserves safety/failure-path assertions and execution routes.
Delete superseded paths and update consumers directly; do not retain compatibility
modules, aliases or duplicate suites. Remove upstream-only and redundant checks
because their responsibility is absent or already exercised, never to hide failure.
Do not alter coverage policy, security exceptions or baselines to make refactoring pass.

Run `cargo xtask check` and relevant feature/consumer routes on the changed sources.
Native changes require separate real input/frame execution; unavailable native
acceptance remains unverified. Compilation and test counts are not acceptance.

## Alternatives

- Centralizing every test in the repository root loses private boundaries and owners.
- Keeping fix-oriented buckets makes fixture dependencies and missing coverage obscure.
- A universal test framework creates another product to maintain without improving
  the contracts. Standard Cargo modules and the existing native runner suffice.
- Retesting each dependency adds cost without establishing Kedra behavior.
