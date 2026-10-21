# Editor self-update MVP implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the accepted update indicator and verified release download for three
primary desktop installations, using manual installation wherever safe automatic
installation would expand the MVP.

**Architecture:** Editor-owned ordinary Rust state, projected through the existing
authored toolbar. A bounded worker handles public GitHub discovery and downloads;
an explicit platform capability selects automatic handoff or manual package reveal.
The mechanism is extracted into `kedra_update` under the
[approved extraction plan](2026-10-21-update-crate.md). No new scheduler, signing
infrastructure or package orchestrator.

**Tech Stack:** Rust 1.98, Bevy 0.20, existing authored UI/bevy_flair, serde_json,
tempfile, SHA-256 and a maintained HTTPS client. Prefer the already-resolved ureq
3.4.2 TLS/platform-verifier configuration used by `kedra_build`, without depending
on that tooling crate. Admit any direct dependencies through existing policy.

**Spec:** [ADR 0026](../../decisions/0026-editor-self-update.md).

## Global constraints

- Scope and inline execution approved; current main and local checks are authorized.
- No commit, push or publication without separate explicit owner authorization.
- Test commands below are instructions, not passing evidence until actually run.
- No commits without explicit owner inspection/approval, including task boundaries.
- Primary kinds: Windows NSIS, Linux AppImage, macOS installed app, ARM64 only.
- Portable ZIP/TAR, source/development and ambiguous installs have no self-update.
- Official early-access releases only; one startup check; no signing or new service.
- Download and install/reveal each require a separate explicit activation.
- Preserve the accepted 32px toolbar, 22px icon, 64 x 22 Primary Update and hover/
  focus-only status popover with at most seven wrapped note lines.
- No new Save workflow, elevation, multi-instance coordination or crash journal.
- macOS must install/restart automatically from the existing verified DMG using
  hdiutil/ditto staging and maintained self_update whole-bundle replace/restore.
- Windows/Linux manual fallback reveals a verified package without exiting or claiming installation.
- Full `cargo xtask check` and native platform acceptance follow only after the hold.

## Review focus

- All current releases are prereleases: discovery must not use stable-only latest.
- Unmarked portable/source layouts: never mistake their executable for an install.
- HTTP body totals can lie: cap received bytes and hash actual stored bytes.
- Focus/popup lifetimes can change during download: operation survives UI retirement.
- Input/session state can change during handoff: stale approval never authorizes exit.

## File structure

- New `crates/kedra_editor/src/updates/{mod,model,release,download,installation,view}.rs`:
  ECS adapter, pure state, metadata policy, bounded transfer, platform capability and
  header/popover projection respectively. Add sibling responsibility-named test files.
- Modify `crates/kedra_editor/src/{lib,chrome}.rs`: install adapter and bind slot.
- Modify `crates/kedra_editor/src/documents/mod.rs` and `actions/native.rs` only for
  narrow read-only queries needed by an enabled automatic exit gate.
- Modify `assets/editor/prefabs/editor-titlebar.prefab.ron`, existing styles and exact
  Figma-exported icons/sidecars. No screenshot backgrounds or fabricated symbols.
- Modify `crates/kedra_editor/Cargo.toml`/`Cargo.lock` only for admitted dependencies.
- Modify `docs/decisions/0026-editor-self-update.md` and `docs/ui/figma.md` for contracts
  and provenance. Any package-marker change belongs to `kedra_build` packaging and
  its tests, never to a runtime filesystem writer.
- No workflow/publisher edits until finished CI/CD contracts have been reconciled.

## Task 1: Close platform capability selection without expanding packaging

**Files:** ADR 0026; read existing `kedra_build/src/{package,inventory}.rs`,
`tools/ci/release*.py`, actual upstream installer sources and installation metadata.

**Interfaces:** Produce one explicit `Automatic` or `Manual` choice for each primary
kind, with its maintained backend and refusal rules recorded in ADR 0026. This
choice is consumed by `installation::detect()` in Task 2.

- [ ] Inspect the final CI/CD inventory and exact installation layouts after its
  owner signals readiness; do not rebase or import their unfinished changes blindly.
- [ ] Perform one focused source review of maintained automatic paths. Reject a
  candidate requiring signature bypass, framework/package migration, custom helper
  infrastructure, forced exit or loss of the old installation on handled failure.
- [ ] Record manual fallback for rejected Windows/Linux paths. macOS uses its
  existing DMG automatically; do not add a new app archive or signing framework.
- [ ] Define positive installed-kind evidence from current packaging. If absent,
  plan a small explicit packaged marker plus verification in the existing builder;
  unmarked installations remain disabled until reinstalled, not heuristically adopted.
- [ ] Read the capability table against the accepted scope. No unsupported automatic
  capability is a failure of MVP: manual is explicitly acceptable.

## Task 2: Implement release policy and single-operation state

**Files:** `updates/{model,release,installation}.rs`, sibling tests, editor manifest.

**Interfaces:** All APIs are crate-private. `InstallationKind` has `Nsis`,
`AppImage`, `MacApp`; `Capability` has `Disabled`, `Manual`, `Automatic`.
`Installation { kind: InstallationKind, root: PathBuf, capability: Capability }` is
constructed only by `installation::detect() -> Result<Option<Installation>, String>`.
`Release { version: semver::Version, source_revision: String, notes: String,
asset: Artifact }` and `Artifact { name: String, url: url::Url, bytes: u64,
sha256: [u8; 32] }` are validated policy output, not raw API DTOs.
`release::select(releases: &[u8], inventory: &[u8], current: &semver::Version,
kind: InstallationKind) -> Result<Option<Release>, String>` validates fixture inputs.
The adapter fetches the selected candidate inventory before calling this policy.
`Model::activate(&mut self) -> Option<Effect>` emits `Download` or `Apply` only;
generation-tagged worker messages are accepted by `Model::receive` only when current.

- [ ] Write failing tests named `prerelease_is_discovered`, `draft_is_ignored`,
  `wrong_target_or_inventory_is_rejected`, `portable_and_source_are_disabled`,
  `duplicate_activate_emits_once`, `stale_completion_is_ignored` and
  `download_completion_never_emits_apply`. Assert the exact policy/state result.
- [ ] After the hold, run `cargo test -p kedra_editor updates --locked`; confirm the
  new tests fail for missing behavior, not fixture setup, before implementation.
- [ ] Implement strict official-repository asset selection and semantic newer-version
  comparison. Verify the current inventory schema rather than inventing a second one.
  States are Hidden, Available, Downloading, Verifying, Ready and Error; retain the
  release identity on error so retry means a fresh download of that candidate.
- [ ] Bound release-list JSON to 256 KiB, inventory to 1 MiB, summary to 16 KiB and
  advertised package size to 2 GiB. Reject malformed/duplicate/negative/overflowing
  sizes, missing complete inventory and conflicting version/source/target identities.
- [ ] Run the focused tests again and confirm all added cases pass. Review direct
  dependency licenses/features and document their reuse; no blanket deny exceptions.

## Task 3: Wire bounded discovery/download outside the frame loop

**Files:** `updates/{mod,download}.rs`, sibling tests; `lib.rs` module/plugin wiring.

**Interfaces:** `download::fetch(release: &Release, staging: &Path,
events: &std::sync::mpsc::SyncSender<WorkerEvent>) -> Result<VerifiedPackage, String>`.
`VerifiedPackage` has private fields and owns the temporary package directory;
only successful verification constructs it. Worker messages carry model generation.
`updates::install(app: &mut App)` and `updates::bind(world: &mut World,
view: &kedra_ui::authored::view::UiView) -> Result<(), String>` are adapter entrypoints.

- [ ] Write failing local-fixture tests `truncated_and_oversized_bodies_fail`,
  `checksum_failure_never_becomes_ready`, `redirect_downgrade_is_rejected`,
  `unexpected_origin_is_rejected`, `timeout_does_not_block_startup` and
  `popover_retirement_does_not_cancel_download`. Inject local transports for tests;
  production URLs remain fixed to the official repository and vetted HTTPS CDN.
- [ ] After the hold, run focused tests and confirm failure for the intended reason.
- [ ] Use the existing IoTaskPool/polling pattern and a bounded event channel, without
  sharing project `PendingJob` ownership. Startup discovery has a 30-second limit;
  explicit downloads have a 10-minute limit. No periodic retries or credentials.
- [ ] Stream to a private tempfile; count/hash actual bytes, fail above inventory size
  or hard cap. Never mark Ready on EOF alone; close/verify stored content before
  publishing the verified object. Progress events cannot accumulate without bounds.
- [ ] Use reported/expected totals honestly; Verifying is a distinct state. A failed
  startup check logs an actionable warning but does not block editor startup or
  masquerade as an available update. CLI asset subcommands never check updates.
- [ ] Run focused tests and confirm passing results; document actual CDN policy and
  quota behavior, without claiming unsigned metadata is publisher authentication.

## Task 4: Implement explicit install/reveal capability

**Files:** `updates/installation.rs`, sibling tests; narrow lifecycle read queries
only if Task 1 selected an automatic backend. Update ADR capability table.

**Interfaces:** `installation::apply(installation: &Installation,
package: &VerifiedPackage, intent: ApplyIntent) -> Result<ApplyOutcome, String>`;
`ApplyOutcome` distinguishes `Revealed` from acknowledged automatic handoff.
`ApplyIntent` carries sanitized native project/state paths, not raw old CLI arguments.
`exit_ready(world: &mut World) -> Result<(), String>` exists only for automatic paths.

- [ ] Write failing tests `manual_reveal_never_exits_or_executes_package`,
  `failed_reveal_retains_ready_package`, `unknown_or_readonly_install_refuses`,
  and `modified_staging_is_rejected_before_apply`.
- [ ] For each automatic capability add `pending_input_or_job_blocks_exit`,
  `dirty_or_unknown_session_blocks_exit`, `cancel_or_save_error_blocks_exit`,
  `another_instance_refuses`, `stale_exit_approval_is_rejected` and
  `replacement_failure_preserves_old_installation` before its implementation.
- [ ] After the hold, run focused tests and confirm intended failures.
- [ ] Manual path uses a maintained native file-manager reveal mechanism for the
  verified local package, preserving native path values; no shell interpolation,
  arbitrary remote execution, editor exit or fake installed receipt. Retain Ready
  and ownership of the staged package; revealing must not drop/delete it. Show
  `Manual installation required` plus platform-specific plain instructions. Keep
  successful revealed packages in the user update-cache directory across clean
  exit so the user can close the editor before installing. Partial downloads remain
  temporary; persistent packages are reverified on use, never restored as Ready.
- [ ] Automatic paths use only the Task 1 admitted mechanism, revalidate staging,
  require exclusive installation access and a fresh clean-session check, then normal
  lifecycle exit. If this requires bespoke orchestration, select Manual instead.
  Do not redesign ordinary window-close behavior as part of this MVP.
- [ ] Run focused tests and confirm passing results. No successful native install
  claim without the corresponding actual OS acceptance in Task 6.

## Task 5: Project accepted states into the existing toolbar

**Files:** `updates/view.rs`, sibling tests, `chrome.rs`, editor titlebar prefab,
existing controls/workbench styles, exact update icon exports/sidecars, Figma map.

**Interfaces:** Task 3's `updates::bind` binds stable `editor.update` slot under
`window.tools`; view consumes Model/Capability and activates Model, never network
or filesystem operations directly. Hover/focus popover is a disposable projection.

- [ ] Write failing tests `hidden_reclaims_width`, `ready_is_primary_64_by_22`,
  `control_is_client_not_drag`, `activation_does_not_open_popover`,
  `keyboard_focus_opens_status`, `popover_contains_no_actions` and
  `short_notes_hug_and_wrapped_notes_stop_at_seven_lines`.
- [ ] After the hold, run focused tests and confirm intended failures.
- [ ] Reuse authored kit buttons, existing spinner/warning assets and exact Figma
  update SVG. Obtain fresh high-fidelity design context before visual implementation;
  do not use expired URLs or recreate accepted artwork.
- [ ] Implement all states, hover/focus-only noninteractive popover and rendered-line
  note clipping with 16px line height and content-hugging layout. Leave unrelated
  Tooltip semantics unchanged. Preserve focus through indicator/button replacement.
- [ ] Run focused tests; verify native geometry/input in Task 6 rather than asserting
  that ECS construction alone proves wrapping, alignment or hit testing.

## Task 6: Verify bounded MVP after CI/CD completion

**Files:** existing native QA tooling/fixtures, ADR capability/limitations and
`docs/ui/figma.md`; generated logs/captures belong under `target/` only.

- [ ] Confirm the owner has lifted the build/test hold before running any command.
- [ ] Run `cargo xtask check`, `cargo xtask features --non-interactive` and
  `cargo deny --locked check` on the actual final revision; retain real failures.
- [ ] Exercise each primary OS with two known versions and both success/failure
  paths. Manual paths must reveal the verified package without editor exit or file
  replacement. Automatic paths additionally require real process/permission/lock,
  handled-error rollback and relaunch evidence. Unavailable hardware stays unverified.
- [ ] Use real input/frames for 32px header, 22px target, Ready 64 x 22, focus/hover,
  seven wrapped lines, short popup, DPI/resize, Build/Save alignment and non-dragging.
- [ ] Record actual supported automatic/manual/disabled capabilities and unresolved
  acceptance limits. Do not publish a release or push the work without authorization.
- [ ] Stop for owner inspection; commit only after explicit approval.
