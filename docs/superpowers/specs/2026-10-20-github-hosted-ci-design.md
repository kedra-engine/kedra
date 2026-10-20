# GitHub hosting and managed CI

Status: the owner approved the design and both public destinations. No repository
has been created or published under this design. New commits and the candidate
diff still require the owner's inspection and approval before publication.

## Goal and scope

Move the Kedra Winit fork to GitHub, publish the cleaned Kedra history there and
run complete PR/main validation on managed GitHub Actions runners with reusable
dependency compilation caches. Do not deploy Kubernetes, use the home production
server, restore obsolete CI commits or require private infrastructure for builds.
Managed runners replace the earlier private-infrastructure CI proposal.

The approved public destinations are `kedra-engine/kedra` and
`kedra-engine/winit`.
No paid runners, extra cache storage or paid services are enabled implicitly.
Existing YC cache/registry resources are not deleted by this migration.

## Source preservation and publication

The accepted local product baseline is commit
`5ed0552ce846049e0a21be6778c564018c7829ec`. Revalidate HEAD and the working tree before
editing: other local work must not be overwritten. Publish only reviewed branch
refs, never `--mirror`, reflogs, private backup archives, old CI test branches,
worktree state or generated evidence. Run secret scanning on the exact source
history being published before the first push. Findings block publication;
masking logs is not remediation.

Create Winit as a GitHub fork of `rust-windowing/winit`, preserving upstream
provenance, notices and history. Import our exact modified revision
`948d27f0007602d2536a8974fd483c6fd33d19d3`, whose upstream base is
`e9809ef54b18499bb4f2cac945719ecc2a61061b` (`v0.30.13`). Use a dedicated
`kedra-v0.30.13` branch. Do not upgrade Winit, rewrite its commits, publish upstream
release tags indiscriminately or modify the shared Cargo Git cache in place.
Review fork workflows; keep inherited workflow execution disabled during import
so upstream release/deployment automation is not accidentally activated.

Verify anonymous fetching of the exact imported revision before switching Kedra's
Cargo patch URL. Update Cargo.lock through Cargo and review that only the source
locator changes, not the selected versions or fork revision. Update current
hosting, dependency-policy/provenance and contributor documentation to GitHub;
preserve historical architecture facts without a broad unrelated rewrite.

## Managed runner model

Use standard GitHub-hosted Linux, Windows and macOS runners, with explicit OS
labels rather than an uncontrolled latest label where practical. Record labels
and resolved image/toolchain versions in logs. Exact images and architecture must
be compatible with the pinned Rust toolchain and available dependencies.

Start native validation and coverage with a 180-minute job timeout; light checks
have a 30-minute timeout. GitHub documents a six-hour hosted job execution limit,
not a 20-minute step limit. These are budgets, not measured duration guarantees.
Do not pay for larger runners to make initial acceptance pass without approval.

Use four Cargo build jobs on Linux/macOS. Preserve the documented Windows
single-job diagnostic build to avoid competing MSVC links; do not claim that
compiling test binaries alone runs the tests. Disable incremental compilation in
CI and use line-table debug information to bound disk/linking cost while retaining
source locations. Do not change developer profile defaults in Cargo manifests.

## Required validation

Preserve the complete map in `docs/ci.md` and `docs/testing.md`:

- File growth, dependency architecture, Rust formatting and tooling contracts.
- Clippy, all workspace/all-target/all-feature tests, doctests and API docs.
- Default/minimal workspace and independently built runtime feature checks.
- Fresh Linux Rust line coverage >=80%, with retained LCOV even on threshold failure.
- Current dependency advisories, sources and licenses via cargo-deny.
- Markdown, offline local links, exact publication history secret scanning.
- Headless Python QA tests, including their existing failure-path tests.

Native Rust validation runs on Linux, Windows and macOS with matrix fail-fast
disabled. Run the Windows wrapper's regression tests before using it. Lightweight
cross-platform-independent checks and coverage run on Linux once. Use existing
xtask and maintained tools; do not build a new executor or restore the immutable
provider-specific Python adapter. Native steps must propagate nonzero exit status.

PRs, main pushes and merge queue events run required CI without path skips. Cancel
superseded PR runs to reduce wasted work, but cancellation is not success. Do not
cancel accepted main validation merely because a newer main push appears.
Run names must remain stable so branch rules can require the actual result.

A final `CI required` gate runs even after dependency failures and succeeds only
when every expected job result is exactly `success`. Reject missing, failed,
cancelled and skipped results. Test the actual gate's negative paths rather than
only adding a test of unused helper code. Full native UI acceptance is explicitly
outside headless CI: do not present software-rendered frames as physical input,
mixed-DPI or IME certification.

## Caching and performance

Reuse a maintained Rust cache action with dependency-only artifact caching and
Cargo registry/git source caches. Evaluate its exact version, licensing,
maintenance, save policy and trimming behaviour before pinning it. Do not archive
the entire unfiltered target tree, whole CARGO_HOME, credentials, build reports,
test binaries or profiles from previous coverage runs.

Cache identity separates runner OS/architecture, compiler/toolchain, Cargo.lock,
relevant build flags, dependency manifests and the native versus coverage role.
Compatibility fallback must not cross OS/compiler/coverage boundaries. Only
successful main-push jobs may save shared main caches. PRs restore acceleration;
they do not become trusted cache writers. GitHub cache branch scoping is part of
the threat model, not a custom public S3 writer credential.

Fresh tests, coverage profiles, secret scans and advisory checks run regardless of
cache hits. Cache misses must build from source. Corrupt cache recovery must fail
visibly and not become an unconditional retry-to-green.

Initially stay within the default 10 GB repository cache allowance and short
artifact retention. Avoid duplicating large caches for every workspace crate or
every minor check. Publish only useful logs, LCOV and bounded diagnostics, not
build directories. Measure real cache sizes/hits and cold versus warm duration
before calling CI fast; tune only with evidence and without weakening checks.

## Permissions and repository policy

PR/main jobs use `contents: read`, checkout with persisted credentials disabled
and no corporate secrets. Use `pull_request`, not `pull_request_target` for
untrusted execution. Do not give compilation jobs id-token, package publication,
repository write, administration or infrastructure credentials. Pin external
actions to owner-selected floating stable major tags. These tags are mutable;
scope the security audit's ref policy to the six reviewed actions without
disabling other audits. Pin tool versions and verify downloads.

No release workflow or signing/publishing credentials are introduced. Future
release jobs must be separate from untrusted CI and its caches. Infrastructure
administration remains private and, if later needed, follows YouTrack/TeamCity;
Kedra's public GitHub CI does not join that trusted infrastructure pipeline.

After actual green CI, configure main rules: PR-only changes, exact required gate,
resolved conversations, current base requirements, force-push/deletion protection
and no silent administrator/service-account bypass. Preserve the solo-owner model:
do not demand an impossible second human approval or create an unreviewed merge
bot. Read back actual rules; exercise acceptance and negative merge paths without
using a failing fixture as product coverage evidence.

## Delivery and acceptance

1. Owner approves the destinations, this design and then its implementation plan.
2. Prepare the fork import and workflow/documentation changes locally in an
   owner's normal working directory, as requested by the owner;
   preserve current user work and inspect exact publication refs.
3. Run local focused tests, workflow lint/security checks and `cargo xtask check`;
   record actual limitations, including unavailable target OS/hardware.
4. Show the owner the diff before any new commit, then obtain approval for the
   reviewed commits and publication. The user's migration request authorizes the
   intended destinations, not unrelated changes, releases or infrastructure cleanup.
5. Import and anonymously verify Winit, publish reviewed Kedra main and observe the
   first real Actions run. Fix real failures, not speculative auxiliary workflows.
6. Obtain a full cold successful run and a comparable warm run demonstrating actual
   cache reuse; preserve source/image identities, timings and fresh LCOV >=80%.
7. Activate and verify the accepted main rules. Report changes, actual checks,
   cache/timing evidence, limitations and remaining risks. Do not claim cross-OS or
   hosted acceptance from local compilation or worker registration alone.
