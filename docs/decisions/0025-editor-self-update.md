# ADR 0025: Development-stage editor self-update and safe restart

- Status: MVP scope and inline implementation approved; native acceptance pending
- Date: 2026-10-21
- Depends on: [desktop distribution](0023-desktop-distribution.md),
  [document sessions](0012-document-sessions-and-selection.md)

## Intent and boundaries

Prepare editor self-update for Windows x64, Linux x64 and macOS Apple Silicon
without changing the active release work. Discovery, download, verification,
installation and restart are distinct operations. Download requires an explicit
click; installation/restart requires another explicit click. Neither discovery
nor completed download authorizes installation on exit or automatic restart.

The bounded mechanism and editor integration are implemented for review; native
acceptance is incomplete. Isolated macOS DMG staging/replacement/relaunch is covered
with ordinary third-party executables, not copied Apple platform binaries. Actual
released-artifact updates, Windows/Linux manual workflows and real UI input/frames
remain unverified. Current release behavior is described in [distribution](../distribution.md).
No cryptographic signing, publication or production-readiness claim follows.

### Accepted MVP scope

Primary installation kinds are Windows NSIS, Linux AppImage and macOS installed
app. Portable ZIP/TAR distributions and development/source runs have no self-update
in this iteration; hide the control rather than converting their installation kind.
Use the official early-access prerelease channel and check once during startup.
No periodic scheduler, channel settings or new update hosting service is needed.

macOS must install and restart automatically after explicit Update activation;
manual DMG installation is not an accepted macOS fallback. Windows/Linux may use
manual installation where a
maintained backend cannot meet the narrow safety contract without substantial
additional infrastructure. Download and verify the existing release package, show
that manual installation is required, and let Update reveal the package in the OS
file manager. Do not exit, execute the package silently or claim it was installed.
Keep a revealed package in the user's update cache across normal editor exit so
the user can close the editor before manual installation; never leave it only in
an exit-deleted tempfile. This does not restore Ready or resume download next time.
The popover remains status-only with no additional actions.

Exclude signatures, delta updates, download resumption, background installation,
install-on-exit, multi-editor coordination, privilege escalation and automatic
rollback after launching the new version. Another editor instance, insufficient
permissions or unsupported launch location causes refusal, not forced shutdown.
Handled replacement errors must preserve/restore the old complete installation;
power-loss recovery is not promised. If even handled-error safety is uncertain,
use manual fallback instead of adding a new recovery framework.

## Accepted presentation

Figma file `2EZlK40cfx2Mub7GmDyI70`, page `175:45356` contains the canonical
[update masters](https://www.figma.com/design/2EZlK40cfx2Mub7GmDyI70/UI-KIT?node-id=327-17756).
Indicator family `327:17796` and popover family `327:17900` define Available,
Downloading, Verifying, Ready and Error. Shared Toolbar `27:1016` already has an
update slot; editor header masters are right `222:51056` and macOS `222:51199`.

- No update: hide the control and reclaim its width.
- Available: 22 x 22 update/download icon; activation starts one download.
- Downloading/Verifying: process indicator; repeated activation is a no-op.
- Ready: Primary purple Update button, 64 x 22; activation requests safe install.
- Error: persistent indicator; activation retries download, never installs bytes
  from a failed verification. Installation errors must not silently trigger restart.
- Popover opens on hover or keyboard focus, not activation. It contains status,
  current/new versions, truthful progress/error detail and key changes, no buttons.
- Notes occupy at most seven rendered lines including wrapping, not seven input
  lines. Height hugs shorter content; no reserved empty space. Treat notes as plain
  untrusted text, not HTML, executable links or asset paths.
- Keep the shared 32px header, 22px Build group, aligned Save/Build, kit tokens,
  native macOS control reservation and Windows caption controls.
- The update control is a client hit-test region, never a drag region.
- Closing the popover has no effect on the operation. Unknown totals use an
  indeterminate indicator; verification is not an invented percentage.

Figma descriptions that still mention 26/28px controls do not override current
22px geometry or the explicit owner requirements.

## Existing integration constraints

`kedra_editor::chrome` already binds authored header slots and client regions.
Extend that composition rather than adding another toolbar/window implementation.
`kedra_ui::tooltip` is a plain hover-only hint and closes on click: it is not a
drop-in implementation of the required structured hover/focus popover. Reuse its
native geometry/picking approach without changing unrelated tooltip semantics.

`kedra_desktop` currently emits `AppExit` immediately when the active window closes.
Editor Save and dirty-document settlement are not implemented. Project-entry
IME/paste settlement exists but is not a session-wide Save protocol. Do not claim
it protects unsaved authoring documents or introduce a second authoring writer.

The update mechanism belongs to runtime-layer `kedra_update`, with no workspace
or Bevy dependency. It owns release validation, bounded transport, verified package
lifetime, native marker validation and explicit macOS staging/replacement. The
editor is its concrete consumer and supplies trusted repository/application/bundle/
executable identity and running version through validated configuration. Library
version is never a substitute for application version. Existing inventory/marker
formats and resource layout remain unchanged.

The editor owns workers outside the frame loop, UI projection, input/source
settlement, preferences and exit authorization. An owned restart action crosses
Bevy's consumed-World boundary; only the stopped host invokes its explicit commit.
No library drop, discovery or completed download authorizes installation. This
separation follows the contributor rule requiring coherent independent mechanisms
to become crates even with a single real consumer. Packaging remains owned by
`kedra_build` and cargo-packager, not by the update library.

## Proposed trust and release contract

The owner defers independent cryptographic update signing during active development.
Use HTTPS, the official repository's complete published release, explicit target/
installation-kind selection, expected size and SHA-256 from its release inventory.
Do not create keys, add signing secrets or require publisher-signing infrastructure
for this iteration. OS signing and notarization remain separately unimplemented.

This deliberately trusts GitHub release hosting and repository control. A digest
downloaded with the package detects corruption, not malicious replacement of both
package and inventory. Repository/account compromise can therefore authorize
malicious update code. HTTPS does not remove that limitation. Do not describe this
scheme as independently authenticated, tamper-proof or production-hardened.

Release metadata binds schema, application, channel, semantic version, source SHA,
supported OS/architecture, installation kind, minimum OS, artifact filename/URL/
size/hash and bounded plain-text summary. Read metadata with hard size/time bounds.
Reject unknown schemas, duplicate targets, unsupported installation kinds,
non-newer versions, wrong application/architecture/channel, size/hash mismatches,
oversized input, insecure URLs and downgrade redirects. Discovery uses only the
official repository; validate package URLs and HTTPS redirects against its expected
GitHub release/CDN origins. Do not fetch arbitrary user-provided endpoints or infer
authenticity from filenames. No embedded GitHub tokens or automatic use of ambient
developer credentials. Persisted version history can reject previously observed
downgrades, but is not cryptographic replay/freeze protection.

Before broader production distribution, reconsider a maintained signature verifier
and authenticated metadata binding version/channel/target to artifact size/hash.
That future change needs explicit key-custody, rotation/recovery and migration
decisions. Do not implement custom cryptography or a misleading signature bypass
switch in a library that requires signed updates. Reassess backend fit instead.

The existing publisher accepts exactly five packages plus inventory/checksums.
Any new manifest/helper or macOS updater archive changes that explicit
set and requires coordinated publisher tests, not a wildcard upload relaxation.
Publish update metadata only after the entire validated release is available.
Incomplete drafts and build-only runs are never discovery candidates.

The current publisher marks even `v0.1.0` a GitHub prerelease. Do not use GitHub's
stable-only latest-release endpoint and conclude there are no updates. Proposed
initial channel is explicitly early-access; stable/channel migration is out of scope.
Use existing GitHub release metadata, with bounded anonymous requests and rate-limit
backoff. An unavailable/quota-limited check does not block startup or create a retry
scheduler. No new service or credentials are authorized.

## Installation matrix

| Existing installation | Candidate payload | Required handling |
| --- | --- | --- |
| Windows NSIS | Verified NSIS installer | Native installer owns installed layout; wait for old processes to close, handle UAC/cancel/failure, confirm result before relaunch |
| Windows portable ZIP | None in MVP | Hide self-update; never silently convert to NSIS |
| Linux AppImage | Verified full AppImage | Replace the actual outer image, not the mounted inner ELF; stage on the same filesystem |
| Linux portable TAR.GZ | None in MVP | Hide self-update; never replace arbitrary parent directories |
| macOS installed app | Verified complete app bundle | Replace the complete bundle, not only Contents/MacOS; preserve modes and allowed internal links |

The current macOS release remains a DMG. Mount the verified image read-only with
native `hdiutil`, stage the entire `Kedra.app` using native `ditto` next to the
installed app, validate its bundle identity/version/resources, and detach the
image before authorizing exit. `self_update` 1.3.1's maintained `Move` primitive
owns same-filesystem replace/restore. Keep the backup on replacement/restore error;
do not let temporary-directory cleanup destroy recovery material. No new release
archive, shell-formatted scripts, administrator prompt or code-signing bypass is
introduced. A shared install-scoped file lease on every updater-capable process
becomes exclusive during staging/handoff; competing editor instances cause refusal.
Normal Bevy exit and World teardown precede replacement and relaunch with sanitized
native path arguments. On macOS the original host process performs this small
post-run tail before returning normally; it does not install a separate helper or
claim that the old OS process has already terminated. A cancelled/failed preparation
or exit check keeps the old app running.
No Intel editor updates are implied. Running from
DMG/App Translocation, read-only locations,
package-manager ownership and ambiguous development layouts must fail safely,
with actionable status, not guessed write destinations or global protection changes.

Bind installation kind/root to explicit package metadata and actual executable
identity. `APPIMAGE` alone is not proof of ownership. Validate filesystem identity,
symlink policy and the installation marker before writing. Portable project data
must not be stored in/replaced with the immutable distribution tree. Existing
unmarked archives require an explicit migration/manual reinstall policy.

Use an external maintained installer/update helper where suitable, rather than
building a generic installer. Windows automatic replacement needs an external
installer handoff for file locks and lifetime independent of the old editor;
otherwise use manual installation. An automatic installer must operate only
on a verified staged payload and a narrowly validated installation identity, not
arbitrary remote commands or shell-formatted paths. Workspace code remains safe
Rust; choosing an upstream native implementation does not authorize local unsafe.

## Lifecycle and failure invariants

One app-wide operation owns request generation, immutable release identity and
worker lifetime. Header entities/popovers are disposable projections. Stale worker
messages cannot overwrite a newer operation. Repeated clicks cannot launch a
second network request or helper. Use existing worker/channel patterns, not another
scheduler. Discovery runs once at startup, bounded and non-blocking;
offline/check failures must not block normal editor startup.

Stream downloads into private staging with hard byte/time limits. Total progress
comes from the release inventory's expected size, checked against received bytes. Partial
files never become Ready. A completed download transitions to Verifying, then
Ready only after official-release identity and size/hash checks, plus payload-layout
checks for automatic replacement. Manual packages remain unopened during validation. Never execute
the candidate as an initial authenticity probe. Revalidate durable staged content
before installation; a cache flag is not a trust receipt. MVP does not persist Ready
across editor launches or resume partial downloads.

For automatic archive installation, extract with maintained readers into a new
owned directory. Do not implement extraction for the manual DMG/NSIS path. Reject absolute
paths, parent traversal, escaping links, device/special entries, duplicate/colliding
paths and expanded-size/file-count abuse. macOS's required internal bundle links
need a bounded allow policy rather than blindly accepting or banning every link.

On Update activation:

1. Preflight payload, installation identity, available space and permissions without
   changing the current installation. Acquire an install-scoped interprocess lock.
2. Refuse if another editor instance is using the installation and tell the user
   to close it. Do not implement cross-editor settlement or termination. If a
   backend cannot establish exclusive installation access, use manual fallback.
3. Settle all pending native input/drafts and source sessions, including hidden
   documents and locked inspectors. Save only through authoring authority. Cancel,
   unknown dirty state, Save error/conflict or changed session generation aborts.
4. Freeze mutation for the final exit handoff; an earlier clean check is not a
   permanent approval. Persist preferences through the existing lifecycle. Finish
   native preparation before authorizing shutdown. Emit normal app exit and let
   Bevy release its World, never `process::exit` from a UI callback.
5. The macOS host's post-run tail commits only after explicit authorized handoff
   and successful Bevy exit. The old process returns after replacement/spawn; there
   is no separately packaged helper. Close/crash without that handoff does not mean
   install-on-exit permission. Windows/Linux manual paths never authorize exit.
6. Preserve the last complete installation until replacement commits. Stage next
   to the destination; do not assume temporary directories share its filesystem.
   Restore on handled replacement errors. Do not claim power-loss safety or add
   a persistent recovery journal in MVP.
7. Relaunch only after successful commit using the new installation identity and
   a typed, sanitized launch intent. Preserve project/state paths as native path
   values; do not replay asset subcommands, automation credentials or arbitrary
   old CLI/environment overrides. Project data is not migrated by the updater.

For today's read-only editor, the exit gate can approve only a provably clean
session with no unsettled entry/input/job state. It must fail closed when editing
capabilities appear without an installed settlement participant. Do not implement
the next authoring/Save milestone implicitly as part of self-update.

OS installer cancel/failure, helper failure, disk exhaustion and relaunch failure
must be reported distinctly when automatic installation is supported. Failed
relaunch does not prove corrupt installation; do not automatically oscillate versions.
Full crash recovery is out of scope; handled-error replacement safety is mandatory.

## Maintained alternatives and source findings

- [cargo-packager-updater source][packager-updater] at the existing packager commit
  is MIT/Apache licensed, version 0.2.3, and supports NSIS/WiX, AppImage and app
  bundles with minisign verification. It lacks portable ZIP/TAR directory installation.
  Download buffers the payload and verification makes another full data buffer.
  Windows installation spawns PowerShell and calls `process::exit`; that is not an
  acknowledged graceful-exit handoff. Linux renames the old image then writes the
  new image in place, leaving an interruption window. macOS stages in the system
  temporary directory, renames the old bundle there and has no restore on a failed
  replacement rename; its privileged AppleScript path removes the old bundle.
  These are source-level fit findings, not an executed exploit or native test.
  Do not adopt the installer blindly or fork/patch upstream merely to force a fit.
- [self_update][self-update] is MIT licensed; crates.io lists 1.3.1, Rust 1.88,
  not yanked. Its current documentation describes bundle/multi-file replacement,
  signature/checksum hooks, explicit permissions and same-filesystem constraints.
  Reviewed published 1.3.1 `Move::replace_using_temp` provides replacement with
  restore on a handled rename error. Only that primitive is used for the macOS
  whole-bundle commit; native tools own DMG mounting and bundle copying. Its full
  updater/discovery/extractor is not another authority. Enable only its existing
  ureq/Rustls/platform-cert path, with defaults/CLI progress/signatures disabled.
  This reuses existing graph packages and adds self_update/self-replace; pinned
  package admission still requires cargo-deny. Unlike the automatic temporary
  cleanup in its higher-level bundle path, retain backup material if restore fails.
  Windows loaded DLL locks, NSIS orchestration and crash-safe recovery still need
  separate evaluation rather than assumptions from README claims.
  GitHub discovery alone is insufficient for authenticated release identity.
- [Sparkle][sparkle] is a macOS-specific established updater with signed artifacts,
  optional signed feeds, native installer helpers and custom UI support. It adds
  framework/helper packaging and a platform integration boundary; evaluate its
  license notices, signing requirements and safe Rust integration before adoption.
  It is not a Windows/Linux solution or permission to replace the accepted UI.
- [Velopack][velopack] is a cross-platform packaging/update alternative to examine
  if existing backend guarantees are insufficient. Adoption changes package and
  bootstrap ownership and would require an explicit distribution architecture
  revision, not adding a second release orchestrator during active CI/CD work.

Prefer a maintained backend satisfying these invariants over custom replacement
infrastructure. Select automatic or manual capability per primary platform before
implementation; manual fallback is an accepted outcome, not a backend-selection
blocker. Do not resolve poor fit by weakening handled-error safety or claiming
that a downloaded package was automatically installed.
The extraction moves existing dependencies rather than admitting another backend.
Dependency graph/advisory/license checks remain required on the final revision.

## Verification and acceptance

Tests must use deterministic local artifacts and endpoints; routine checks must not
depend on live GitHub, personal keys, Figma, AI services or a public release.

- Unit/state tests: every legal/illegal transition, duplicate activation, stale
  completion, error retry, popover closure and absence of automatic install/exit.
- Feed/security tests: wrong repository/app/channel/version, downgrade, incompatible
  OS/target, incomplete release, oversized/malformed data, unexpected asset origins,
  redirect downgrade, timeout/rate limit, truncated/hash-mismatched downloads.
  Signature verification is deferred, not an implicitly passing test requirement.
- Filesystem tests for enabled automatic backends: Unicode/spaces/quotes, path
  traversal, symlinks, case collisions, read-only destination, cross-device staging,
  disk-full and handled replacement errors. Old installation remains recoverable.
- Lifecycle tests: active IME/paste, pending jobs, generation change, unknown/dirty
  state and competing instance refusal. Simulated settlement error/cancellation
  prevents exit; no new Save implementation is required.
- Native N to N+1 acceptance on the three primary installation kinds: real resources,
  process termination/locks, UAC cancellation, DMG/translocation refusal, AppImage
  outer-path replacement, permissions and notices. Automatic capability needs
  successful restart and handled-error recovery; manual capability needs package
  reveal with no exit/mutation and clear instructions. Portable/development runs
  must remain disabled. Power-loss recovery and post-launch rollback are not gates.
- Real input/rendered frames: hover and keyboard focus, no click-open popup, seven
  wrapped lines/short-height content, Ready width, narrow headers, DPI/window move,
  client hit testing and truthful progress. Headless tests are not native acceptance.
- Run `cargo xtask check`, feature/target checks, dependency/security/license policy
  and publisher regression checks on the final revision; report unavailable platforms
  and interrupted runs as unverified, never passed.

## Implementation boundary

The owner accepted the restricted MVP, portable exclusion and per-platform manual
fallback. Backend choices are implementer decisions within that boundary, not
authorization to expand it. Reconcile release metadata after active CI/CD work
finishes; report actual tests and native acceptance separately from implementation.

[packager-updater]: https://github.com/crabnebula-dev/cargo-packager/blob/5c1824ba8022401e9ef0de36ff1a12c5acf60237/crates/updater/src/lib.rs
[self-update]: https://github.com/jaemk/self_update
[sparkle]: https://sparkle-project.org/documentation/
[velopack]: https://docs.velopack.io/packaging/overview
