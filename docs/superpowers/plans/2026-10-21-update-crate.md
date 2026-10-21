# Update crate extraction implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for
> inline execution. Track the checkboxes below. Commit only with owner authorization.

**Goal:** Extract the existing update mechanism into `kedra_update`, consumed by
the editor without changing accepted update behavior.

**Architecture:** A runtime-layer ordinary Rust crate owns release validation,
bounded HTTPS download/integrity verification, native installation identity and
macOS staging/replacement. It has no Bevy or workspace dependencies. The editor owns
presentation, task scheduling, input/document settlement, preference persistence
and explicit exit authorization. The crate never decides when the application may exit.

**Tech Stack:** Existing Rust 1.98 dependencies and maintained `self_update::Move`;
no new third-party dependencies.

**Spec:** [ADR 0026](../../decisions/0026-editor-self-update.md), plus the approved
extraction boundary above.

## Global constraints

- Commit, push and PR publication require explicit owner authorization. Do not
  modify the owner's installed application.
- Preserve automatic macOS installation after explicit activation and Windows/Linux manual reveal.
- Portable/source/unmarked installations remain disabled. Preserve the existing
  release inventory and package-marker formats.
- Repository, application/bundle/executable identity and running application
  version come from validated consumer configuration, not the update crate's
  version or editor constants.
- Keep public APIs small and document invariants, errors, ownership, cleanup and
  explicit commit/relaunch semantics.
- Do not treat this refactor or isolated native fixtures as native UI acceptance.

## Review focus

- A consumer version different from the library version must control release
  comparison and marker matching.
- Alternate configured identity must be honored; foreign repository/application/
  bundle identities must still be rejected.
- Invalid configured path segments must not produce URLs or installation paths
  outside the intended identity.
- Dropping prepared/verified objects must retain current cleanup semantics; only
  explicit commit may replace the installation.
- Editor lifecycle handoff must survive Bevy consuming its World without moving
  settlement authority into the library.

## Task 1: Extract the mechanism and its independent contract

**Files:** Create `crates/kedra_update/{Cargo.toml,README.md,src/lib.rs}`; move
mechanism modules and their tests from `crates/kedra_editor/src/updates/`; update
workspace/editor manifests and lockfile. Keep `mod.rs`, `view.rs` and `exit.rs` in
the editor. Keep editor presentation-state policy there unless required by the
independent mechanism API.

**Interfaces:** Export validated `Config`, `Kind`, `Release`, `VerifiedPackage`
and native preparation ownership through a small public facade. `Config` supplies
repository, application, bundle identifier, executable name and current semantic
version. Discovery and marker validation accept `&Config`. Download accepts
progress/verifying callbacks rather than Bevy types or a public HTTP-client
response. Installation detection accepts explicit executable/resource/AppImage
paths; executable-relative resource resolution stays in the existing editor
adapter. macOS preparation returns an owned `Prepared`; its explicit commit takes
native argument values and returns the spawned child. Do not add a plugin,
scheduler, provider framework or forwarding modules in the editor.

- [x] Add an external-consumer integration test for the public configuration/
  download boundary; observe failure before extraction.
- [x] Add regression tests for consumer/library version independence, alternate
  identity acceptance, foreign identity rejection and invalid configuration paths.
- [x] Move existing mechanism tests alongside production modules; keep
  editor-dependent handoff tests in the editor.
- [x] Implement the minimal facade and configuration, preserving existing byte/time
  bounds, URL policy, marker format and maintained replacement primitive.
- [x] Run `cargo test -p kedra_update --locked`; verify moved and new tests pass.
  Inspect `cargo tree -p kedra_update --locked` for absence of Bevy/editor/
  workspace edges.

## Task 2: Reconnect the editor and document the actual boundary

**Files:** `crates/kedra_editor/src/updates/{mod,exit,view}.rs`, editor lifecycle
tests, `docs/architecture.md`, `docs/decisions/0026-editor-self-update.md`, original
update plan.

**Interfaces:** The editor constructs configuration for the official Kedra
repository and running editor version. It schedules library calls through its
existing workers, projects progress and retains the external authorized-exit
handoff. Source/input checks and preference persistence remain editor-only.
Tests exercise the public library rather than a duplicate mechanism implementation.

- [x] Run the existing handoff/ordinary-close/pending-input/save-failure regressions
  before changing the adapter and record their actual baseline.
- [x] Replace local mechanism references with direct `kedra_update` API calls;
  preserve UI states and sanitized native launch arguments.
- [x] Verify the editor lifecycle regressions after extraction, including no exit
  on rejected settlement and no implicit install-on-exit.
- [x] Document responsibility, dependency justification, supported features and
  unresolved native acceptance; remove stale claims of an editor-local mechanism.
- [x] Run `cargo xtask check --non-interactive`,
  `cargo xtask features --non-interactive` and `cargo deny --locked check`;
  report failures honestly.
- [x] Run the isolated native DMG test separately; do not confuse fixture success
  with released-artifact or real UI acceptance.
- [x] Inspect the complete diff including new files and stop for owner review
  before committing or publishing without authorization.
