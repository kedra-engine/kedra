# Roadmap

Kedra is being built as a connected game-development environment on Bevy. The
next milestones establish a usable authoring and Play workflow before exposing
project-defined editor extensions. Bevy remains available for ordinary components,
systems, plugins and low-level work.

Milestones describe intended order and user-visible completion criteria, not
calendar promises. Dates and release versions are not assigned yet. Planned work
is not implemented behavior or automatic authorization to expand a change's scope.

## At a glance

| Stage | Status | User-visible result |
| --- | --- | --- |
| Foundation | Available; native acceptance incomplete | Open a project, use the native editor shell and shared infrastructure |
| 1. Scene and prefab authoring | Partially implemented; acceptance incomplete | Select an object, configure supported components, undo changes, save and reopen |
| 2. In-application Play | Planned; follows authoring | Run a scene, inspect diagnostics, stop and return to editing |
| 3. Project-defined editor extensions | Planned; follows the working authoring/Play loop | Supply custom field/type inspectors and editor windows from a project |

Reliability and platform acceptance run alongside these stages, not only after
new features are complete. Web and mobile runtime support remain later directions.

## Current foundation

The workspace provides native project startup, verified asset deliveries, shared
authored scenes/UI, source-editing APIs, application automation and retained native
docking. The editor exposes a
[Asset Browser](site/en/contributing/contracts/editor/asset-browser.md) and
[source document sessions](site/en/contributing/contracts/editor/source-documents.md),
with editable [Hierarchy](site/en/contributing/contracts/editor/hierarchy.md),
[Scene View](site/en/contributing/contracts/editor/scene-view.md) transforms and a
source-bound Inspector. Supported edits use Undo/Redo and explicit Save; creation
publishes a source before opening its clean tab. There is no stable game SDK or
in-editor project execution. See
[architecture](architecture.md) for ownership boundaries.

The implemented authoring subset does not complete stage 1. Remote observation,
injected input and renderer captures support
automation; they do not certify physical OS input or native UI stability.

## 1. Scene and prefab authoring

**Status: partially implemented; completion criteria remain acceptance gates.**
Document creation, supported Inspector/component and Hierarchy edits, single-node
transforms, Environment editing, history, Save and unsaved-exit choices are implemented.
Full prefab workflows, drag-placement feedback and platform/native acceptance remain incomplete.

### Scope

- Create and open scene and prefab documents.
- Show the document hierarchy, select an object and inspect its registered
  components and fields.
- Edit supported numbers, strings, booleans, enums, vectors, colors and asset
  references. Show unsupported fields explicitly as read-only or unsupported;
  do not silently omit them.
- Reflect accepted source changes in the viewport. Provide asset browsing and
  selection, with the previews needed to choose references for those fields.
- Place prefab instances and distinguish instance overrides from changes to the
  prefab source. Make the destination of an edit explicit.
- Support undo/redo, dirty-document indicators, source-owned Save, reopening and
  explicit handling of unsaved changes when closing a document.
- Report validation, write failures and external source conflicts without
  discarding the current draft or silently overwriting another revision.

The editing path is **inspector → validated authoring document → live projection**.
Changing only a live Bevy component is not a persistent authoring edit. The host
filesystem writer must honor the existing save protocol's revision, conflict and
durability contracts; runtime crates do not acquire an authoring-writer dependency.
See the [source-authoring contract](site/en/contributing/contracts/source-authoring.md).

### Completion criteria

A developer can assemble a small scene from prefab instances, select an object,
configure supported component fields without manually editing source files, see
the result, undo/redo the change, save and reopen it after restarting the editor.
Prefab-source edits and instance overrides remain distinguishable after reopening.
Environment assignment and reusable environment-asset editing follow
[ADR 0018](decisions/0018-scene-environment-and-preview-stages.md): distinguish
editing the shared asset, changing a scene reference and making a unique copy.
Asset editing requires explicit source ownership, history, conflict-safe Save and
reopening; it must not bypass the document-editing contract through live ECS writes.
Rejected edits and failed saves preserve the last valid sources and the unsaved
draft; a successful save does not acknowledge a newer, uncommitted edit.

The first inspector should use mechanisms that can later support project-defined
inspectors. This does not require freezing a public extension API before real
authoring tools have exercised those mechanisms.

[ADR 0009](decisions/0009-property-editing-and-inspector.md) proposes the common
field/edit-session contract and incremental delivery order. The first intended
consumer edits Kedra's own UI sources in an independent preview; it does not
modify the serving editor shell live or replace later 3D authoring acceptance.
[ADR 0012](decisions/0012-document-sessions-and-selection.md) proposes the shared
document/selection and opening contract, with separate ADRs for each dockable tool.

### Read-only Scene View and Inspector foundation

[ADR 0017](decisions/0017-read-only-scene-view.md) defines the rendering slice: render an
opened scene/prefab, navigate with orbit/pan/zoom and free flight, frame objects,
use grid/axes and standard views, and select objects in sync with Hierarchy.
Complete it through separately demonstrated render/resource, navigation/docking
and selection slices. The
[Inspector](site/en/contributing/contracts/editor/inspector.md) connects to
the same source selection without requiring renderer readiness. Camera and selection
state are not document edits. Source transforms are separately owned by
[ADR 0053](decisions/0053-scene-transform-manipulators.md); Play remains excluded.

The initial projection profile supports spatial/UI documents and PNG/static-GLB
inspection with an explicit matching verified delivery. Spatial camera navigation
and opaque-mesh selection are implemented; remaining native input acceptance is
the gate before full native acceptance of the read-only slice. Source
admission alone is insufficient for rendering. This work takes delivery priority
over the earlier UI-first preview/Inspector sequence without replacing the
independent passive UI-preview contract in ADR 0016. Own-editor source admission
and interactive UI testing are not implicitly enabled by the 3D Scene View.

### Shared environment and model-preview stages

[ADR 0018](decisions/0018-scene-environment-and-preview-stages.md) defines lighting
parity. The initial implementation supplies a native reusable environment asset
(solid background/ambient only), scene-root assignment, shared native hydration
and an ordinary authored editor lighting stage for models. Light entities remain
in scenes/prefabs; exposure/tonemapping are camera concerns. Editor lighting
replaces rather than adds to scene lighting without editing sources. Complete
skybox/IBL, camera-source settings, native isolation/parity acceptance and source
editing through the authoring milestone before claiming the full contract.

## 2. In-application Play

**Status: planned, after the first working authoring milestone.** Complete the
loop from editing a scene to checking it in a running game.

### Scope

- Start project execution from the selected scene through the environment.
- Interact with the running game and see execution errors and diagnostics.
- Stop execution and return to the authoring workspace.
- Launch from a revision-pinned snapshot of accepted working state, including
  unsaved edits but excluding unresolved drafts, as proposed by
  [ADR 0046](decisions/0046-scene-game-runtime-boundaries.md).

Select and document the project-code execution and isolation boundary before
implementation. In-application Play describes the user workflow, not a promise
that editor and game share one mutable world or one process. Opening a project
must not implicitly become permission to execute its code.
The proposed Scene/Game/Pause/Prefab workflow and implementation gates are in
the [Scene/Game runtime plan](plans/scene-game-runtime.md); the execution/render
backend is not selected by that product proposal.

### Completion criteria

A developer can **edit → run → check → stop → continue editing**. Stopping or
failing execution preserves authoring documents and unsaved work. Gameplay state
does not silently become saved scene data. Startup, failure and shutdown have
explicit lifecycle behavior and regression coverage.

## 3. Project-defined editor extensions

**Status: planned, after the authoring/Play loop is usable.** Let a project extend
the environment instead of requiring changes to Kedra itself.

### Scope

- Register custom inspectors for fields and component types.
- Register project-defined editor windows with the native workspace.
- Expose documented access to selection and authoring documents, with edits using
  the same validation, undo/redo and Save mechanisms as built-in tools.
- Define registration, ownership, loading, retirement, compatibility and error
  reporting for project editor code. Do not assume an unreviewed dynamic-loading
  or hot-reload mechanism.

Built-in tools should exercise the same extension mechanisms where applicable.
Public APIs follow demonstrated needs; they do not introduce a second reflection
system, ECS or general-purpose plugin framework alongside Bevy.

### Completion criteria

A project can provide a custom inspector and an editor window without modifying
Kedra's source. Their document edits participate in normal undo/redo and Save.
Invalid registrations, unsupported compatibility and extension failures produce
actionable diagnostics rather than silent fallback or document loss. The loading
and trust boundary for project code is explicit.

## Reliability and acceptance throughout

Native interaction and capture reliability have unresolved intermittent failures.
Local Rust checks do not exercise the full real-window
visual/interaction corpus. Headless CI and its required aggregate are configured;
current hosted results and live enforcement require external verification. Native
regression execution must become automated in a controlled graphical environment,
with retained failure evidence and no retries that turn an unexplained failure into
a passing result.

Every claimed milestone needs verification of its actual user workflow, including
failure paths. Compilation, model tests, remote receipts and passing headless CI
alone are not native UI acceptance.

- Preserve applicable [third-party licenses and notices](third-party.md) when
  distributing sources or binaries.
- Verify physical native input, picker/chrome/IME, monitor/DPI transitions and
  floating-host behavior on the desktop targets being claimed.
- Keep independent Figma references and reproducible native fixtures. Complete
  startup and control-state acceptance without promoting application pixels into
  expected design references.
- Establish actual CI results and public repository administration before claiming
  platform or release readiness. See [CI](ci.md) and [hosting setup](hosting.md).

## Later directions

These directions have no fixed delivery order or dates yet:

- Extend asset authoring with richer type-specific previews, import-setting
  editors, reference-aware renames and measured incremental processing where real
  workflows justify them. Stage 1 includes the asset selection needed for component
  editing, not a complete asset-authoring toolset.
- Establish supported and certified desktop runtime targets for Linux, Windows
  and macOS. Desktop acceptance work starts during the earlier stages.
- Extend the game runtime to web and mobile platforms (Android and iOS). That
  support is planned, not implemented; the editor remains desktop-only.
- Extend the initial shared Luau runtime into project-owned script delivery,
  generated API definitions and debugger tooling. The existing explicit runtime
  and native QA sequences do not implement automatic project-code execution,
  Play isolation or a stable scripting SDK.

## Scope and contribution boundaries

New public APIs, crates and dependencies require a concrete responsibility and
consumer. Use ordinary Rust crates and Bevy plugins; do not introduce a parallel
ECS, scheduler, layout engine or speculative compatibility layer.
