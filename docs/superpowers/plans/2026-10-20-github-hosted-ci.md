# GitHub Hosted CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use executing-plans or subagent-driven-development to
> implement this plan task-by-task. Checkboxes track completed deliverables, not intentions.

**Goal:** Publish the preserved Winit fork and cleaned Kedra source to GitHub with complete, cached,
managed PR/main CI.

**Architecture:** Keep product verification in the public Kedra repository and use disposable
GitHub-hosted runners. Restore dependency-only caches in PRs; save shared caches only after
successful main builds. Keep publishing and private infrastructure credentials out of compilation
jobs.

**Tech Stack:** GitHub CLI/API, GitHub Actions, pinned Rust, existing xtask, Python unittest,
maintained Rust cache/tool-install actions, actionlint and workflow security lint.

**Spec:** [Approved design](../specs/2026-10-20-github-hosted-ci-design.md).

## Global Constraints

- Destinations: public `kedra-engine/kedra` and `kedra-engine/winit`.
- Preserve Winit revision `948d27f0007602d2536a8974fd483c6fd33d19d3` and upstream provenance.
- Preserve cleaned Kedra product history; do not publish obsolete CI commits or private
  evidence.
- No new commits until the owner inspects and approves the diff; no paid services or releases.
- Native Linux/macOS jobs: four Cargo jobs; Windows diagnostic build: one Cargo job.
- Native and coverage timeout: 180 minutes; light jobs: 30 minutes.
- Coverage remains fresh and >=80%; no skipped tests, packages or failure suppression.
- Shared caches contain dependencies only, separated by
  OS/architecture/compiler/flags/lockfile/role.
- No whole-target archives, credentials, coverage profiles or cached test conclusions.
- Default cache allowance remains 10 GB; no billed increase without approval.
- Keep all authored files within 1000 lines / 64 KiB; no generic executor or parser.
- Private infrastructure and YouTrack/TeamCity stay separate; do not deploy Kubernetes.
- Native UI acceptance still requires real input and frames and is not claimed by headless CI.

## Review Focus

- A local Cargo Git cache may be incomplete: verify full fork ancestry and anonymously fetch the
  exact SHA after import.
- PR-controlled values must not reach privileged execution: regular pull_request, read-only
  contents, no secrets or persistent checkout token.
- Missing/skipped/cancelled matrix or dependent jobs must make the required gate fail, not
  disappear.
- Keep native artifacts separate from coverage and PR cache writes out of main scope;
  review the actual cache settings and validate them with standard workflow tools.
- New hosting must not alter dependency versions or accidentally trigger inherited fork release
  workflows.

## File responsibilities

- `.github/workflows/ci.yml`: actual PR/main/merge-queue checks, caching and required gate.
- `tools/ci/required.py`: small fail-closed result policy, not a job executor.
- `tools/ci/test_required.py`: real CLI success/error tests; no YAML-shape assertions.
- `.github/zizmor.yml`: narrow owner-approved floating-major policy for the six reviewed actions.
- `Cargo.toml`, `Cargo.lock`, `deny.toml`: public source URL migration without version changes.
- `xtask/tests/portable_contracts.rs`: provider-independent fork provenance assertions.
- `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`: current public hosting links
  and factual status.
- `docs/ci.md`, `docs/hosting.md`, `docs/testing.md`, `docs/third-party.md`:
  workflow/security/cache/runbook and provenance contracts.
- `docs/decisions/0001-foundation.md`, `docs/decisions/0020-macos-client-titlebar-input.md`: current
  locators, retaining original decisions.
- Main working directory: the owner's normal checkout, selected by the owner;
  no changes to other active worktrees.

---

### Task 1: Preserve and inspect publication inputs

**Files:** no product edits; ignored `target/github-migration/` for sanitized evidence.

**Interfaces:** consumes local main and the read-only Cargo Winit Git database; produces validated
exact publication SHAs and full local fork copy.

- [ ] Revalidate HEAD, all worktree changes and existing GitHub destination absence; preserve user
      edits.
- [ ] Work in the owner's main working directory, preserving unrelated edits;
      do not rebuild all old worktrees.
- [ ] Copy Winit into a separate local repository without modifying Cargo's shared cache; verify
      objects with `git fsck --full`.
- [ ] Verify upstream base is an ancestor of the pinned fork, inspect both fork commits and preserve
      licenses/notices.
- [ ] Secret-scan the exact Kedra and Winit publication histories with redacted output. Do not scan
      private backup archives as publication inputs.
- [ ] Checkpoint: display publication refs and scan outcomes; block on missing objects, unexpected
      provenance or findings.

### Task 2: Import the exact Winit fork

**Files:** no new Winit commits; external objects `kedra-engine/winit`, branch `kedra-v0.30.13`.

**Interfaces:** consumes Task 1 SHAs; produces anonymously fetchable
`https://github.com/kedra-engine/winit` with exact fork revision.

- [ ] Create the native GitHub fork from `rust-windowing/winit` into the approved organization;
      verify actual parent metadata and public visibility.
- [ ] Disable inherited Actions during import; do not register secrets or enable upstream
      release/deployment jobs.
- [ ] Push only the dedicated branch at the existing exact SHA, not a mirror of all refs. Read back
      the branch identity.
- [ ] Fetch that branch into a clean credential-free Git directory, verify commit SHA, upstream
      ancestry and source tree identity.
- [ ] Stop on a collision with an existing destination rather than overwriting it. Do not delete the
      former-host fork as part of this task.

### Task 3: Migrate public dependency and hosting locators

**Files:** `Cargo.toml`, `Cargo.lock`, `deny.toml`, `xtask/tests/portable_contracts.rs`, current
hosting/provenance documents listed above.

**Interfaces:** consumes the public fork from Task 2; produces an independently resolvable locked
graph using GitHub.

- [ ] Add `winit_fork_retains_exact_revision_and_reviewed_source` to portable contracts: assert
      GitHub patch URL, exact revision, allowed Git source and matching Cargo.lock identity via
      existing cargo-lock tooling.
- [ ] Run the focused test before implementation and confirm failure against the old hosting
      locator.
- [ ] Update the patch and source allowlist, regenerate Cargo.lock through Cargo, and compare
      resolved package versions/identities excluding the reviewed Winit URL substitution.
- [ ] Update current repository links to GitHub. Describe CI and protections as pending until their
      actual acceptance, not automatically verified.
- [ ] Run portable contracts and locked Cargo metadata; verify zero package-version changes and no
      remaining active former-host dependency.
- [ ] Stage only this task's reviewed files for owner inspection; do not commit yet.

### Task 4: Build the fail-closed required gate

**Files:** create `tools/ci/required.py`, `tools/ci/test_required.py`.

**Interfaces:** `require_success(results: object) -> None` accepts GitHub needs-shaped JSON and
raises ValueError unless exactly `policy`, `native`, `coverage`, `supply-chain`, `repository`,
`secrets`, `python-qa` exist with result success. `main() -> int` reads `NEEDS_JSON`, reports
bounded diagnostics and returns nonzero on malformed/missing/non-success input.

- [ ] Write unittest cases for all-success, each failure/cancel/skip result, absent/extra job, empty
      mapping, invalid JSON, invalid result shape and missing environment input.
- [ ] Exercise the actual `required.py` entry point in subprocess tests, not only an unused helper;
      confirm RED before adding it.
- [ ] Implement only JSON validation and exit-status policy with Python stdlib. Never execute
      repository-selected commands or interpolate untrusted values into shell.
- [ ] Run `python3 -m unittest discover -s tools/ci -p 'test*.py'`; real success and
      failure-path cases must pass. Do not add YAML configuration snapshot tests.

### Task 5: Define managed verification and caches

**Files:** create `.github/workflows/ci.yml`; extend `tools/ci/test_required.py`; update CI/testing
documentation.

**Interfaces:** produces seven stable job identities consumed by Task 4 and final job `CI required`.
Native job is a fail-fast-disabled OS matrix.

- [ ] Resolve the latest stable releases and inspect upstream code/licenses.
      Use owner-selected floating major tags: checkout/setup-python/upload-artifact v7,
      Rust-cache/install-action v2 and mise-action v5. Test the narrow audit policy
      without disabling audits.
- [ ] Use ubuntu-24.04, windows-2025 and macos-15 labels after checking availability and pinned Rust
      compatibility. Log OS/architecture/toolchain identity.
- [ ] Configure pull_request to main, main push and merge_group without path filtering; concurrency
      cancels superseded PR runs only.
- [ ] Give all jobs contents read and checkout persist-credentials false. No pull_request_target,
      write token or repository secrets; external actions use the approved major tags.
- [ ] Native matrix runs `cargo xtask check --non-interactive` and
      `cargo xtask features --non-interactive`. Windows first runs the existing wrapper regression
      test and diagnostic build and checks every native exit status.
- [ ] Policy runs existing file/architecture/fmt/portable checks. Coverage runs the documented full
      all-target/all-feature llvm-cov command with fresh profiles and >=80 threshold, under its own
      cache role.
- [ ] Supply-chain runs pinned cargo-deny with fresh advisories. Repository runs pinned rumdl and
      offline lychee. Secrets scans Git history and clean sources before compilation; Python QA runs
      existing tools/qa plus tools/ci tests.
- [ ] Restore dependency-only Rust caches; set cache-workspace-crates false and main-push-only save
      policy. Explicitly bind native/coverage roles, toolchain, architecture and build flags to
      keys; do not add a second full compiler cache by default.
- [ ] Set nonincremental line-table builds and the budgets from Global Constraints. Upload only
      logs/LCOV and bounded diagnostics with always conditions and seven-day retention; missing
      successful-run LCOV is an error.
- [ ] Wire `CI required` with `if: always()`, exactly seven needs and `NEEDS_JSON: toJSON(needs)`
      into `python3 tools/ci/required.py`.
- [ ] Validate workflow syntax/expressions with actionlint and security with zizmor;
      do not add homegrown tests of YAML values or a parser dependency.
- [ ] Run actionlint, workflow security lint, the full gate tests and tool/QA tests. Fix genuine
      diagnostics without blanket suppressions or permissions expansion.

### Task 6: Inspect, verify and publish Kedra

**Files:** all candidate source changes; public `kedra-engine/kedra` and reviewed main only.

**Interfaces:** consumes Tasks 1–5; produces actual hosted validation evidence tied to an approved
Git revision.

- [ ] Run `cargo xtask check`, feature/portable/gate/Python tests, cargo-deny, Markdown, links and
      exact-source secret scanning. Check disk space first; do not refill all discarded worktree
      caches.
- [ ] Present diff/stat, dependency graph comparison, action/tool pins, actual checks and remaining
      limitations to the owner; obtain explicit commit/publication approval before committing.
- [ ] Create reviewed commits without restoring obsolete CI history. Create the public Kedra
      repository without generated README/license or hidden history rewrites.
- [ ] Add the reviewed GitHub remote and push main only. Read back visibility, default branch and
      exact remote SHA.
- [ ] Observe the real first Actions run to completion; fix only its actual failures, retaining all
      checks. Do not call registration or a queued run success.
- [ ] Run one comparable warm validation and collect actual dependency-cache hit/size and timing
      evidence with fresh tests/coverage. No performance promise before measurement.

### Task 7: Protect main and close acceptance

**Files:** factual status/runbook updates in `docs/ci.md` and `docs/hosting.md`; native GitHub
rules.

**Interfaces:** consumes complete green CI identity and owner-approved policy; produces verified
main protection without requiring an unavailable second human reviewer.

- [ ] Configure PR-only changes, required `CI required` from GitHub Actions, current base, resolved
      conversations and force-push/deletion denial, without silent bypass actors.
- [ ] Read back native settings and verify failed/missing/cancelled/skipped gate behaviour plus
      legitimate green PR acceptance using bounded checks; do not merge a synthetic fixture into
      product main.
- [ ] Verify outside-PR execution cannot access secrets or write the repository; inspect configured
      token and cache policy. A redacted token is not proof it was inaccessible.
- [ ] Report actual cold/warm results, cross-OS results, fresh LCOV, cache size and limitations. Do
      not claim native UI, release readiness or private security channels are verified.
- [ ] Leave old YC resources unchanged pending a separate cleanup decision; do not add Kubernetes or
      TeamCity deployment work to this migration.

## Handoff

Recommended execution: native, task-by-task in this session, with one independent
review before publication. The tasks are sequential and share publication SHAs;
parallel implementers would add coordination without accelerating the hosted runs.
Owner review of this plan precedes implementation. Owner inspection of the actual
diff separately precedes new commits and final publication.
