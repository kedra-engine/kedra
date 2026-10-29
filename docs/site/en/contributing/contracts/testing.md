# Testing contract

Place tests with the code that owns the guarantee and choose checks that can expose its failure.
[Tests and review](../tests-and-review.md) supplies the contribution workflow;
the [Testing Manual](../../manual/testing/index.md) supplies command recipes.
[ADR 0031][adr-testing] defines the policy and its rationale.

## Contract ownership

Before adding a test, identify the Kedra guarantee, production owner, observable failure,
least expensive sufficient environment and gap in existing checks. Name behavior/outcome,
not issue history. A bug fix extends its owner's suite, including failure paths.
Product contracts, assembled acceptance and repository policy are separate responsibilities.

| Capability | Primary contract owner | Assembled editor / native boundary |
| --- | --- | --- |
| Projects/source paths | `kedra_project` | Startup/forms/Open |
| Logical assets/shared selection/typed drag | `kedra_editor_context/tests/contracts` | Documents admission/activation, Inspector following/locking and native producer/drop lifecycle |
| Documents/instances | `kedra_scene` | Documents, Hierarchy, Scene View |
| Working changes/property candidates/save protocol | `kedra_authoring` | Transform/UiCaption source edits and existing-document Save |
| Recoverable existing-source groups | `kedra_authoring_fs` | Documents group receipts/newer edits; native Save shortcut/menu; real recovery CLI in `apps/kedra/tests/save_recovery.rs` |
| Runtime delivery/integrity | `kedra_assets` | Preview resource binding |
| Native handle reconstruction | `kedra_assets/tests/contracts/references.rs` | Inspector picking/drops |
| Source-matched resource admission | `kedra_authoring_assets` | Documents resource assignment and Scene View |
| Passive static model/bounds/subassets | `kedra_preview` | Scene View camera/frame/refresh; Inspector resource readiness |
| Processing/publication | `kedra_import` | Browser/Open/preview |
| Dock topology/candidates | `kedra_docking` | Gestures/host coordination |
| Retained UI/projection/input and field acknowledgment | `kedra_ui`, `kedra_desktop` | Dock/host/focus; independent diagnostic text field |
| Retained docking hierarchy invalidation | `kedra_ui/tests/contracts/docking.rs` and `docking/headers.rs` | Editor and independent native projection; unchanged edge/order ticks, changed topology, retained input/content and rejection safety |
| Shared live CSS / matching and invalidation patches | `kedra_ui/tests/contracts/styles.rs`; private selector/invalidation tests in the pinned Flair fork | Class/focus/hierarchy/nested-sheet behavior; inline geometry, variables, explicit/default inheritance and transitions; explicit engine suite per ADR 0043; editor/game input and frames remain separate |
| Contained-clip render elision | `kedra_ui/src/render_clipping/tests.rs` | Editor and independent native game with `kedra_ui/render`; matching viewport/DPR frames; main-world clipping and input remain Bevy-owned |
| Native task/producer readiness notifications | `kedra_wakeup/tests/contracts.rs`: automatic initial registration, ready/pending/cancel/await ownership and producer backpressure/failure | `native-wakeup` supervises the real example: sleep and pipe-released worker before any owner poll, GPU frames without RPC/Luau frame pumping; editor owners retain result/failure checks |
| Bounded reactive waits and subsystem demand | `kedra_wakeup/src/reactive/tests.rs`; tooltip/caret/readback tests in `kedra_ui`; clock/terminal tests in `kedra_script`; preferences/motion tests in editor | `native-wakeup` deadline advances real GPU frames without external release/input; editor behavior and independent game regressions remain separate |
| Continuous serving previews | `kedra_editor/src/scene_view/control_tests.rs` and `kedra_wakeup/src/reactive/tests.rs` | Ready/unchanged views must not idle; hidden/failed/retired owners release demand; native Scene navigation and passive update observations are separate from headless policy checks |
| Ordered native text/local history | `kedra_text_input` | Inspector search; Transform/UiCaption fields |
| Reflected presentation | `kedra_inspector` | Editor source adapter/Inspector |
| Scoped Luau drawer composition | `kedra_inspector_luau` | Embedded UiCaption; UI-only/mixed caption source edits |
| Scoped edit candidates/named history | `kedra_authoring/tests/contracts/edit_transactions.rs` | Real Luau/Documents transactions in `documents/transactions/tests.rs` |
| History retention/content checkpoints | `kedra_authoring/tests/contracts/history_retention.rs` | Value-free collection attachments in Inspector contracts |
| Explicit project editor scripts | `kedra_editor/src/edit_scripts/source/tests.rs` | `editor-scripts` native scenario: two-document commit, common Undo/Redo, Save input and disk bytes |
| Script lifecycle/modules/scopes/children/events/parameters/diagnostics | `kedra_script/tests/contracts/`, `kedra_app` | Editor Luau/independent host |
| Update intervals/UI mutation diagnostics | `kedra_app::performance` | Opt-in RPC/Luau performance observation, not physical-input latency |
| Finite CPU recordings/markers/counters | `kedra_profiling/tests/contracts` | Editor self-recording and independent Bevy consumer; ready-made Perfetto analysis |
| Scoped Luau profiling adapter | `kedra_script/tests/contracts/profiling` | Editor workload instrumentation; no recording authority |
| Script Transform/UI adapters | `kedra_script_bevy/tests/contracts/` | Native switch Behavior/Sequence |
| Editor styles/icons/authored sources | `kedra_editor/tests/contracts/` | Component corpus/editor geometry |
| Build/package/update | `kedra_build`, `kedra_update` | Installed distribution/update |
| Repository/release policy | `xtask`, `tools/ci`, `tools/security` | Real scanners/distribution workflow |

### Placement and fixtures

Private invariants belong in the production module's child `tests`, inline or sibling files.
Public APIs use `tests/contracts.rs` with behavioral modules in `tests/contracts/`.
Separate Cargo targets need a process/isolation reason, not one executable per small file.
Repository smoke targets in `xtask/tests/` remain separate real command/process boundaries.
Never widen production visibility to accommodate tests.

Shared setup belongs in the narrowest `test_support` or `tests/support`:
`tests -> support -> production APIs`, never helpers exported from another suite's `tests`
or `*_tests`. Local nested cases may share their own suite setup, not independent suites.
Editor CPU hosts use `kedra_editor/src/test_support.rs`; workspace layout/host inputs use
`workspace/test_support/`; primitive projections use `scene_view/test_support.rs`.
These are distinct fixtures, not a universal App. Tiny invalid inputs stay beside their cases.

Native editor acceptance belongs to `tools/test/src/scenarios/editor`; independent runtime
acceptance to `tools/test/src/scenarios/runtime.rs`. Component comparison uses the existing
runner and `kedra_test`, not another framework. Canonical native content stays in
`tests/fixtures/test_project`; mutating scenarios use complete private copies with stable IDs.
The [canonical fixture contract][test-project] retains source inventory and copy ownership.

### Assertion and refactoring policy

Pure values cover domain/error matrices; headless Bevy covers Kedra registration, ownership
and scheduling; real I/O covers publication/process boundaries. Native workflows cover
assembled input/frame delivery. Higher levels need a different failure boundary, not the
same matrix repeated through UI. Dock topology belongs to `kedra_docking`, retained
attachment to `kedra_ui`, document/selection coordination to the editor and gestures/lifecycle
to native acceptance. Editor design assertions do not belong to generic importer tests.

Negative cases assert unchanged sources, history, published state and foreign resources,
not only errors. Use controlled clocks/readiness, not wall-clock sleeps. Potentially blocking
probes require killable children and independent external deadlines. Avoid uncontrolled
network access in deterministic tests.

Test thin utilities through real successful and failing invocations. Do not create artificial
Cargo projects merely to prove Clippy/rustdoc, test upstream serialization/layout without a Kedra
guarantee, or add QA-driver/report mocks or a new self-test framework. Compatibility probes need
an explicit contract and an explanation why ordinary product tests cannot protect it.
Existing comparator guards remain substantive acceptance checks. Build/package/run and
source-safety failures remain real product contracts, not disposable wrappers.

Refactoring removes superseded paths and updates callers directly, without forwarding
modules, aliases or duplicate suites. Removing redundancy must identify the remaining
guarantee and preserve failure paths, identities, ownership, provenance and execution routes.
Never weaken security, reference or coverage policy to make a change pass.

Do not add tests that merely repeat dependency names, versions, Git revisions or
source lists from Cargo manifests/lockfiles. Cargo and cargo-deny own dependency
resolution and admission. Test actual integration behavior instead. Substantive
security policies, such as bounded advisory exceptions with expiry and caller
restrictions not expressible in cargo-deny, retain their owning policy tests.

## Repository and platform scopes

`cargo xtask check` runs file growth, architecture, formatting, Clippy, all-target tests,
separate doctests and API docs. It neither launches native apps nor includes feature builds,
external tools, site builds or pixel comparison. Unknown tasks fail. The alias bootstraps
only the orchestrator in `target/xtask-bootstrap/`, avoiding Windows replacement of a running
`xtask.exe`; children use the ordinary Cargo target directory or `CARGO_TARGET_DIR`.

`cargo xtask features` separately checks default/minimal workspace configurations and
minimal `kedra_app`, `kedra_script`, `kedra_scene`, `kedra_assets`, `kedra_ui`, `kedra_wakeup`
libraries, plus the profiling-disabled application contract.
This prevents editor feature unification from supplying optional runtime capabilities.
The `release-boundary` profile runs `kedra_app` launch contracts with debug assertions off
and development optimization: diagnostic parser/plugin absence, not optimized editor/native
acceptance. Every subprocess must succeed independently.

Ordinary CI does not add optimized editor release/build/package/launch acceptance to every
PR or coverage run. Shared-build tests use unit/failure checks and small dependency-free
debug fixtures. Actual optimized packages and independent application build/run belong to
the explicitly dispatched Desktop distribution workflow. Do not test Cargo release settings
for their own sake. Platform-specific ignored package/update probes require their real
fixtures and explicit environment; ordinary green tests do not imply they ran. Current
source includes Linux distro/library/AppImage/network probes in `kedra_build` and an
isolated DMG replacement/relaunch probe in `kedra_update/src/macos.rs`.

Docking implementation is shared across Linux, Windows and macOS; the source CI matrix
checks headless behavior. Hosted success and native platform acceptance require separate
evidence. See [docking acceptance][docking] and the
[isolated fixture][dock-fixture]. No current desktop harness certifies
an engine, web export, physical IME/pickers or mixed-monitor input.

### Runner output and local privacy

Explicit xtask commands stream child stdout/stderr, numbered stages and elapsed progress,
then a per-stage summary. First failure stops later stages and reports skipped counts.
Every selected Rust run creates a plain log in `target/xtask/logs/`, printed at startup and
summary; early bootstrap failure may precede log creation. Logs are not automatically pruned
or uploaded. Review sensitive paths/data before sharing. Legacy hosting summary environment
variables have no effect; there is no provider-specific summary adapter.

CI, redirected streams and `--non-interactive` disable prompts/animation, not live diagnostics.
A missing task there is an error; an interactive terminal can choose a keyboard menu.
Subprocess pipes are not a PTY/full-screen TUI and do not support arbitrary interactive tools.
Forced termination can leave incomplete logs and no summary.

## Native ownership and evidence

Each native run requires a fresh output directory and idle desktop. Setup failures, unrun
cases and partial selections remain explicit; none establish complete acceptance.
`--only hierarchy` runs behavior/usability phases once in one session, with separate
results and renderer frames including failures. There is no `hierarchy-appearance` task.
`scene-navigation` covers model/mixed navigation with its own readiness fences; `scene-view`
separately covers PNG/model/UI/mixed preview and host transfer. Catalog/docking scenes use
the ordinary debug editor and shared `--scene`, not hidden fixture modes/examples.

Project scenarios copy the complete canonical project, never generate fixtures/metadata at
startup. Failure mutations affect only copies; docking restart reuses its scenario copy.
Startup Create is an explicit product operation under test. Component hosts keep independent
isolation. Shared actions run finite Luau inside the app under external Rust supervision;
`luau` is an editor-owned startup-form program, while Rust retains feature assertions.
Runtime/script regressions stay in `kedra_script/tests` and `kedra_app::scripting::tests`.

Concrete `native-switch`/`native-projection` require the `kedra-test-project` package/bin,
registered programs/state/renderer method, complete materialized project and two independently
verified deliveries. They are not arbitrary-manifest runners. The fixed host catalogue is
embedded outside imported assets; resource arguments cannot select capabilities/admission.
Both run excluded-game projection-readiness tests before building/launching the host; failure
blocks native acceptance. Root workspace tests do not run the excluded game. Its three
headless readiness contracts are separately reportable, not native acceptance. Unique scoped
Luau isolation checks belong to `kedra_script`, not a duplicate consumer workspace.
Two-process verifiers independently check state, topology, retained Unicode draft/identities,
observations, renderer/readbacks and successful native-close exits. Other apps supply their
own ordinary Rust verifier using `kedra_test`. See
[automation](automation.md#concrete-native-host-and-evidence-limits) and the
[native game contract][native-game].

### Evidence categories and screenshot boundary

Native feature scenarios verify user workflows, availability, retained state, source safety
and usable layout, not Figma coordinates, exact padding or palette values. Requested viewport
sizes exercise compact/large and DPR configurations; window managers can round native sizes.
Resize delivery is not a pixel-equality readiness gate. Native compass/order/refusal/cancellation
matrices belong to `dock-fixture`; editor docking scenarios own real-panel and menu integration.
Readback fences still require exact agreement with independently observed viewport dimensions.
Precise drop-preview/commit agreement remains a docking behavior contract, not design matching.

Distinguish source inspection, pure/headless execution, synthetic layout, RPC/injected input,
renderer readback, explicit pixel assertions and physical input/human review. Source inspection
proves no execution. Synthetic layout is not production layout; RPC is not pixels; a GPU
fence is not scene-content verification; injected Bevy input is not OS certification.

Feature readbacks are validated transient fences, then retired, not galleries or goldens.
Only 22 kit button/input cases have screenshot coverage. Components run the selected sequence
in exactly two sessions against independent offline Figma sources. The reviewed dependency
lock/profile is explicit: graph changes need lock approval, not automatic visual acceptance.
Linux policy/hosted acceptance remains pending; a dispatched workflow is not a passed run.
Figma refresh is explicit host-side authoring, never a CI network dependency. Native QA uses
Rust without Python/Node/npm. See [visual policy][visual-policy],
[UI corpus][corpus] and
[Visual verification](../../manual/testing/visual-verification.md).

Record exact commands, revision including dirty changes, environment/target, selected cases,
exit status, artifacts and limits. Skipped/interrupted checks are not passes. Flakiness needs
a tracked cause, owner and repair plan, not unlimited retries/silent removal. Public reports
contain portable evidence only; keep credentials/private logs/raw failure observations local.

## Coverage and future analysis

Coverage is informational workspace Rust measurement with no minimum percentage. The
configured GitHub job runs fresh headless tests for Rust-affecting changes and retains LCOV;
test/measurement errors or missing/empty reports fail, low percentages do not. Hosted results
need external verification. Native runner source remains in the denominator, but its ordinary
binaries are not launched: this is not native/UI acceptance. Do not add coverage-only modes,
QA/report self-tests, exclusions or native runs merely to raise percentages. Critical
persistence, identity and recovery still require explicit negative/fault tests.

Fuzzing, Miri and benchmark gates are not dummy jobs. Add fuzzing with parsers/untrusted
formats, Miri with applicable memory/concurrency work, and performance thresholds only after
a repeatable baseline. Future native/browser runtime acceptance needs its own fixtures/runners.

## Engineering tools and pins

The Rust gate does not silently install external tools. Current local pins are rumdl 0.3.0,
cargo-deny 0.19.0, Lychee 0.24.2 built from source with `--locked`, cargo-llvm-cov 0.7.1
with `llvm-tools-preview`, and Gitleaks 8.24.2 (Go 1.24+ when building via Go).
Python CI tooling/distribution contracts use Python 3.13; the explicit detector-policy
command requires Python 3.11+. Portable Rust policy/icon/feature checks need no Bash/jq or
provider workflow; icon metadata checks density for browser/Hierarchy/Scene View, not pixels.
See [standalone commands](../../manual/testing/repository-checks.md#external-tools-and-security).

Installation/advisory refresh needs network access; install Go separately. Normal Rust work
needs Rust, a linker and [native development packages][contributing].
Pins do not make the whole OS/toolchain hermetic: record their identity. Upstream tool
lockfiles are separate from Kedra dependency audit; never drop `--locked` to silence warnings.
The documented Lychee `spin 0.9.8` yanked and `proc-macro-error2 2.0.1` future-compatibility
warnings are upstream review concerns, not permission to relax locking or a claim of tool
security. Their occurrence in a fresh build requires actual build diagnostics, not this page.

Local link checking is offline with fragments: external outages do not block PRs. Local
standalone tool entrypoints are distinct from CI installation/cache actions. See
[GitHub CI][ci] for permissions, cache boundaries, Windows diagnostics and
failure handling; source workflow configuration does not prove hosted acceptance.

## Reviewed non-secret Figma identities

The root [.gitleaks.toml][gitleaks-config] extends all pinned default
detectors. Its only additions are owner-approved provenance exceptions for the 22 metadata
files in `docs/ui/corpus/text/`: shared Figma file identity and each file's own component
identity. `figma.fileKey`/`component.key` are exported design-object identities, not bearer
credentials. Exact reviewed values remain in that configuration and their source files;
see [source provenance][source-provenance].

Each rule-scoped table ANDs an anchored exact repository-relative path with an anchored
`generic-api-key` individual match binding the exact field spelling to its reviewed value.
Alternative field/value pairs within the table are OR. No global/directory exemption,
detector override or arbitrary metadata-value pattern is allowed. Match targeting preserves
detection of other credentials even on the same line. Component IDs cannot cross files or
swap with the file-ID field.

Gitleaks matches text, not JSON ancestry: it cannot distinguish `$.file_key` from
`$.instances[0].key` or authenticate live mappings; repeated approved pairs in the approved
file also match. Source schema/manifest/fingerprint validation remains independent.
Changes need fresh provenance review and explicit owner approval. Never auto-refresh this
configuration from captures or alter source bytes/manifests/licenses/baselines to appease
scanning. Authentication credentials must never receive exceptions.

Directory scanning uses a clean checkout or clean Git-inventory source snapshot, including
its copied configuration, from that snapshot root (or an explicit `--config`). Ignored build
artifacts may contain upstream crypto examples; do not weaken detectors to scan them.
Exact relative paths are not suffix matches for arbitrary absolute/nested paths. History
scanning retains defaults and the same exact paths, with a full-history checkout.

The required CI `secrets` job runs `tools/security/check_gitleaks.py` with pinned Gitleaks
before history scanning. Absent/wrong versions, unsafe config or unexpected detector results
fail, never skip/continue-on-error. It validates config/manifest, then real scanner results
for reviewed IDs, synthetic replacements/additions, crossed fields/files, same-line secrets,
unrelated/prefix/suffix paths and a provider-specific default rule. Redacted artifacts go to
a new `target/gitleaks-policy-*`; no account/service/network is required. It is an explicit
policy check, not ordinary Rust/Python discovery. `--gitleaks /path/to/gitleaks` selects an
installed executable; see the Manual recipe rather than installing during tests.

[adr-testing]: https://github.com/kedra-engine/kedra/blob/main/docs/decisions/0031-contract-owned-testing.md
[docking]: ui/docking.md
[dock-fixture]: ui/native-docking-tests.md
[native-game]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/native.md
[visual-policy]: ui/visual-verification.md
[corpus]: ui/ui-corpus.md
[contributing]: https://github.com/kedra-engine/kedra/blob/main/CONTRIBUTING.md
[ci]: https://github.com/kedra-engine/kedra/blob/main/docs/ci.md
[gitleaks-config]: https://github.com/kedra-engine/kedra/blob/main/.gitleaks.toml
[source-provenance]: ui/ui-corpus.md#evidence-and-source-package
[test-project]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
