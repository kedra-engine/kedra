# Architecture contract

## Current scope

The workspace contains `xtask`, native startup/editor modules and shared asset
runtime/import modules, with the editor and isolated native fixtures as consumers.
Bevy 0.20.0 is pinned for this slice. Shared opt-in Luau execution uses `kedra_script`;
Play isolation and project script loading remain unselected/unimplemented.
The game-style asset consumer is an isolated test host, not a running game SDK.
See [ADR 0002](decisions/0002-native-startup.md), the
[windowless startup revision](decisions/0021-windowless-startup.md) and [startup](startup.md).

Shared application automation lives in runtime-layer `kedra_app`, not the
editor. `kedra_ui` owns stable retained-UI identities and renderer-derived geometry.
The editor is one consumer; an independent native fixture and mailbox tests avoid
runtime-to-editor edges. This is an application service slice, not a running game
SDK. See [ADR 0003](decisions/0003-application-automation.md) and
[automation](automation.md) for authority, lifecycle and verification limits.

Shared `kedra_app::launch` arguments are available to independent native games,
not supplied by the editor or test tool. Debug assertions gate the scene override,
remote CLI fields and diagnostic service modules. Release keeps configured scene
startup and game-facing scripting, but has no application transport or injected
input/capture API. The editor uses the same remote configuration and build boundary.

Runtime-layer `kedra_script` owns BMS/Luau execution, persistent behavior callbacks,
finite sequences, standard admitted-source modules, scoped ctx bindings, Bevy-clock
waits, budgets and revocable owner lifetime. The editor and independent applications
consume the same mechanism. Host-validated immutable parameters, bounded subsystem
subscriptions, owner-bound child sequences and structured diagnostics share that
lifetime. `kedra_script_bevy` separately owns opt-in Transform/UI bindings and native
Activate forwarding; it does not expose generic World access. `kedra_app::scripting`
adapts existing native feature
systems to finite in-application programs; the editor supplies its own read-only
state/workspace API. External Rust QA retains process/evidence authority. No source
eval is exposed remotely and the runtime has no QA/editor dependency. See
[ADR 0029](decisions/0029-luau-runtime-evaluation.md), the
[runtime contract](../crates/kedra_script/README.md) and
[ADR 0032](decisions/0032-script-entrypoints-and-modules.md).

Runtime-layer `kedra_scene::startup` selects one explicit Scene path from the host's
native source catalog. Hosts choose native hydration/plugins and authorize scripts
separately; loading a scene grants neither editor authority nor script execution.
See [ADR 0030](decisions/0030-explicit-startup-scenes.md) and
[scene lifecycle](scenes.md#explicit-startup-scene). Catalog and docking adapters
consume that lifecycle. Complete native acceptance of the current composition
remains pending.

Docking topology lives in runtime-layer `kedra_docking` as ordinary Rust
values with no Bevy dependency. `kedra_ui::docking` owns retained authored-chrome
projection; the editor owns gesture sessions, feature bindings and preferences;
desktop owns acknowledged native surfaces. See
[ADR 0008](decisions/0008-docking-ownership.md) and its implementation status.

Window headers use shared `kedra_ui::window_chrome` slot/region contracts and the
`kedra_desktop::chrome` native adapter. Editor and docking supply compositions,
not competing window-move implementations. See
[ADR 0019](decisions/0019-composable-window-chrome.md) and
[header integration](ui/window-chrome.md) for lifecycle and acceptance limits.

## Package layers

Every workspace package declares `[package.metadata.kedra] layer = "..."`.
`cargo xtask architecture` checks all declared workspace production/build edges,
including optional and target-specific dependencies. Renames do not bypass it.

| Layer       | Allowed workspace dependencies           |
| ----------- | ---------------------------------------- |
| `core`      | core                                     |
| `runtime`   | core, runtime                            |
| `authoring` | core, runtime, authoring                 |
| `import`    | core, runtime, authoring, import         |
| `editor`    | core, runtime, authoring, import, editor |
| `tooling`   | all layers                               |

Dev dependencies are excluded: they cannot ship in a player. Package cycles are
also rejected by Cargo. Tests prove that allowed edges remain allowed through
transitive composition. Every workspace package must use MIT OR Apache-2.0.
Every member must also declare `[lints] workspace = true`; the architecture gate
parses each manifest with `toml` and rejects missing inheritance. Clippy additionally
receives `-F unsafe_code`, so omitting inheritance cannot silently enable unsafe code.

**Limit:** this checks workspace package edges, not capabilities or arbitrary
third-party dependency graphs. A mixed runtime/editor package could still defeat
the intended semantics. Review crate responsibilities; add resolved player-graph
checks and explicit feature/target fixtures when actual player packages exist.
Do not interpret an empty engine graph as proven player isolation.

## Module rules

`kedra_update` owns blocking release validation, verified download/package lifetime,
native marker checks and explicit macOS staging/replacement. It has no workspace
or Bevy dependencies. The editor supplies trusted application configuration and
owns workers, UI, source/input settlement, preferences and exit authorization.
An installation writer is not an authoring writer and never saves project sources.
See [ADR 0026](decisions/0026-editor-self-update.md) and the
[crate contract](../crates/kedra_update/README.md); native update acceptance is pending.

[ADR 0009](decisions/0009-property-editing-and-inspector.md) proposes the full
authoring slice's field interactions and isolated own-source preview. The
[read-only Inspector profile](ui/inspector.md) implements shared `kedra_ui::fields`
controls, consumer-neutral `kedra_inspector` reflection/presentation and an editor
source adapter. The architecture gate additionally restricts Inspector production
workspace dependencies to core/runtime layers. Editing and Save remain unimplemented.

[ADR 0012](decisions/0012-document-sessions-and-selection.md) proposes shared
document/selection authority. Asset Browser, Hierarchy, Inspector and Document
Preview each have a separate linked ADR and independent existing-docking view.
The [current read-only source profile](ui/source-documents.md) uses one
`kedra_authoring` workspace per project; it does not implement editable-document
settlement or Save. Rendering is a separate read-only presentation capability,
requiring an explicit matching verified delivery rather than source validity alone.
Read-only [Hierarchy](ui/hierarchy.md) projects the existing composed graph and
routes scoped node selection through that same document authority.
[ADR 0017](decisions/0017-read-only-scene-view.md) defines the read-only Scene
View slice before Inspector: isolated source projection, verified resource binding,
editor camera navigation and scoped viewport selection. Its initial
[projection profile](ui/scene-view.md) supports spatial/UI rendering and raw-asset
inspection, spatial navigation and scoped opaque-mesh selection. No projection mutates sources
or authorizes project execution.

[ADR 0018](decisions/0018-scene-environment-and-preview-stages.md) defines the
shared rendering direction: reusable environment assets, ordinary scene light
entities, camera-owned display settings and an authored model-preview stage.
`kedra_scene::environment` supplies the version-1 asset/loader and scene-root
reference. Its opt-in `render` feature supplies native primitive, material, sun
and camera-environment hydration consumed by Scene View. No rendering feature is
required to load the data. Environment source editing remains authoring-owned;
asset-editing/Save, skybox/IBL and full game/editor frame parity are not implemented.

- Prefer maintained libraries and standard ecosystem tools over bespoke infrastructure.
  Compare fit, license, platform support, maintenance and dependency cost first.
  Keep custom code focused on Kedra-specific semantics and policy; record a reason
  when an existing solution cannot serve the requirement.
- Each crate owns a coherent responsibility and exposes the smallest useful API.
- Standard Bevy ECS, reflection, schedules, assets and plugins own their domains.
- Editor UI, import tools and filesystem writers are not runtime dependencies.
- Persistent document/asset identity is distinct from runtime entity/asset handles.
- Authoring operations validate a candidate before publishing changes and undo history.
- Filesystem commit, authoring transaction and deferred ECS mutation are different guarantees.
- Lifecycle APIs define owners, cancellation and rejection of stale async results.
- Save writes sources, never the live simulation World.
- Editor domain models and drafts are ordinary Rust values; ECS resources own
  the model as a value. Rendered entities and Bevy scheduling state are projections,
  not document or feature authority. Entity handles stay in presentation adapters.

Feature combinations must be explicit and tested. `--all-features` alone does
not prove default, minimal or target-specific builds work. An ADR and CI matrix
change accompany any mutually exclusive features or platform commitments.

## Documentation contract

Each future engine crate documents purpose, dependencies, supported features,
examples and failure behavior. Significant cross-cutting decisions get an ADR
with alternatives, consequences and verification criteria. Research proposals
must not be presented as accepted decisions or working features.

Documentation records current facts, invariants, limitations, plans and strategy.
It does not retain session narratives, intermediate design/review transcripts or
local run histories. Generated evidence belongs under `target/`; source provenance,
licensing obligations and unresolved publication blockers remain documented.
Planned functionality is separate from implemented behavior in the [roadmap](roadmap.md).

## Repository tooling dependencies

Testing follows [ADR 0031](decisions/0031-contract-owned-testing.md): contracts stay
with their production owners; fixtures never depend on test suites. Product
contracts, native acceptance and repository policy have distinct evidence boundaries.
The [testing guide](testing.md) owns placement and execution navigation.

Application builds use tooling-layer `kedra_build` through a thin project-local
`cargo kedra` launcher. Cargo owns compilation/artifact discovery, existing import
and delivery APIs own assets, and maintained cargo-packager owns native formats.
No runtime/editor dependency on this tool is introduced. See
[distribution contracts](distribution.md) and [ADR 0023](decisions/0023-desktop-distribution.md).

The shared builder owns runnable application assembly, generic installed-package
verification and explicit standalone launch. `build-result.json` is a verified
client handoff; Cargo metadata owns settings. Future editor integration invokes
the same project-local CLI, without importing tooling into runtime/editor crates.
CI supplies machines/credentials and adds application-specific tests/publication,
not missing build steps. See [ADR 0025](decisions/0025-verified-build-and-run.md).

Shared native QA lives in tooling-layer `kedra_test`; see
[ADR 0028](decisions/0028-shared-native-test.md) and [testing](testing.md).
The comparison engine, native sessions, evidence/reports and project-target CLI
routes are implemented. The component policy pins the explicitly approved dependency
graph; its lock check rejects changed graphs without reviewed lock approval.
Component and editor-feature acceptance require their separate native runs. Generic process,
transport, evidence and comparison belong to the tool;
project-owned Rust scenarios retain application setup, readiness and lifecycle
authorization. Runtime crates must not depend on this tooling crate or the editor.

`xtask` is a thin command orchestrator. `cargo_metadata` owns Cargo's metadata
schema and invocation; Git owns the repository file inventory. The small custom
checks enforce Kedra's layer DAG, license declaration and per-file growth policy,
which standard Cargo checks do not express. Unit/CLI tests cover those rules.
Formatting, Markdown lint, links, coverage, advisories and secrets
use existing ecosystem tools rather than parallel in-house implementations.
CLI argument parsing uses `clap`, terminal selection uses `dialoguer`, progress
uses `indicatif`, subprocess streaming uses `duct`, and unique log creation uses
`tempfile`. `console` handles ANSI stripping in persisted logs. These dependencies
avoid custom terminal rendering, command parsing, pipe plumbing and temp-name schemes.
Offline visual regression lives in `kedra_test`, with a thin `xtask visual` adapter:
image/PNG and dify own decoding
and perceptual comparison; Kedra owns only configured budgets, exclusions and
evidence policy. See [ADR 0004](decisions/0004-visual-regression.md).
Manifest lint-inheritance and portable bootstrap/runtime contracts use the standard
`toml` parser. Provider-specific workflow adapters are not part of the local gate.
Dependency-exception tests use RustSec's MIT/Apache `cargo-lock` 11.1.0 to read
exact package/caller identities without a package cache or Cargo invocation.
It supports the current lockfile format and pinned Rust toolchain; its optional
CLI/graph features are disabled. The parser reuses serde/TOML/semver and adds the
URL identity stack rather than a custom Cargo lockfile grammar.

Native dependencies additionally require permissive BSD/ISC/Zlib/MIT-0 and
Unicode-3.0 licenses. MPL-2.0 remains restricted to exact reviewed package versions.
`deny.toml` retains default duplicate and wildcard denial, with exact upstream
version exceptions and time-bounded unmaintained parser/macro exceptions. See
[ADR 0007](decisions/0007-dependency-policy.md) and [third-party obligations](third-party.md).
