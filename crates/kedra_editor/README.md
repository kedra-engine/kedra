# kedra_editor

This editor-layer crate composes project startup and editor features. `domain` contains ordinary
Rust models, drafts and validated lifecycle transitions without ECS handles.
`model` is the thin Bevy resource/state adapter; `actions`, `jobs` and `screens`
own semantic input, effects and retained views respectively.

The desktop host opens only validated projects and does not run project code.
Asynchronous completions carry generations. Creation cannot be cancelled with a
false rollback promise. `kedra_wakeup` registers native readiness automatically
when a Task handle is created; owners only poll or await their results. Update
progress uses an automatically notifying bounded producer. Neither path needs
per-operation wake calls; results and authorization stay with these owners.
The native host opts into `ReactivePlugin`: 100 ms bounded idle safety polling,
1/60 s focused active cadence and subsystem-owned timed/active hints.
Tooltip/preference deadlines shorten idle waits; caret, scripts, render preparation
and editor lifecycle work retain active demand. Indefinite sleep is not supported;
third-party asset/animation paths retain the safety sweep. Visible serving Scene
Views request continuous updates even after their first frame is ready; the
current shared App cannot independently sleep its UI while servicing that preview.
This is not a project runtime or Play implementation. See
[ADR 0044](../../docs/decisions/0044-native-readiness-wakeups.md) and
[ADR 0045](../../docs/decisions/0045-bounded-reactive-editor.md).
The proposed Scene/Game workflow and unselected execution/render
backend are in [ADR 0046](../../docs/decisions/0046-scene-game-runtime-boundaries.md).

The Assets browser supports explicit native file import via `kedra_import::ingest`. The production
import dependency reuses source copying/metadata initialization instead of duplicating
filesystem policy in the UI host. Import does not replace the runtime delivery.
The browser provides a lazy
folder tree, Unicode search, virtualized grid/list and source document Open through
one project-owned authoring workspace. Hierarchy displays composed source nodes,
including nested prefabs, with search, expansion and shared document selection.
Content context menus own node creation, Rename/Delete and source Undo/Redo.
Node drops reparent within the active source while preserving spatial world pose;
unsupported cycles, singular transforms and shear refuse atomically.
Inspector follows shared logical asset/node selection with lock, search and shared
reflected field presentations. `context` and `drag` expose source-backed selection
and typed payloads to native consumers without granting authoring permission.
New scene/prefab selects an unused `.scene.ron`/`.prefab.ron` path inside the project's assets, then creates
the document through common named history and publishes it before opening a clean tab.
Subsequent edits require explicit Save, which publishes the complete document
group and refreshes Assets. Undo/Redo preserves identities and never writes files;
Undo after Save leaves a dirty removal for explicit Save. Source removal with sidecars
requires a separate managed asset lifecycle and is refused. Document tabs host the
Scene View for supported spatial/UI sources; PNG/static-GLB assets use separate
asset-preview views. Resource-free documents need no project delivery; native asset
references require an explicit matching verified project delivery.
Spatial camera navigation, grid/inspection controls and scoped opaque-mesh picking
are implemented. The viewport panel groups Move/Rotate/Scale and snapping icons;
single-node drags preview before one source-history publication. Local rotation and
axis/uniform scale preserve signed scale; Escape restores the baseline. Their native
input acceptance remains separate from compilation.
Supported Inspector fields and explicitly granted editor scripts share authoring
transactions/Undo/Redo. Existing-document group Save is separate from accepting
in-memory edits. General scene editing and gameplay project execution remain unavailable.

The registry crate supplies this library, not the editor executable or the external
`assets/editor` source/delivery. Assemble those through the source repository's
[startup workflow](../../docs/site/en/contributing/contracts/startup.md). Registry
consumers on macOS must also apply the consuming-root Winit patch documented by
[`kedra_desktop`](../kedra_desktop/README.md#registry-consumers-on-macos).

Debug `--scene` uses the same `EditorPlugin` native capabilities, but selects an
explicit scene lifecycle instead of project startup. Project operations are rejected
while that lifecycle owns startup. Authored components bind catalog, docking and
consumer-owned reflected-value diagnostics; scene paths do not select Rust plugins.
Native scene/prefab references use full `editor://` asset paths, resolved against
loaded native catalog members and the matching published source revision. Source
UUIDs, selection addresses, sidecars and provenance remain persistent identities.
Catalog text/calibration lives in its authored `editor.catalog` component; native
code keeps source-scoped measurement and capture. Named `dock.reset`, `dock.drafts`
and `dock.overflow` Luau programs own diagnostic sequence selection and ordering.
Their scoped native capabilities do not replace production gestures or expose a
general remote topology writer. F12 remains the explicit-scene exit shortcut.
The production default topology is the `ui.docking` Snapshot on
`editor://scenes/editor.scene.ron`. The common editor plugin registers this inert
schema in release and debug builds. Editor integration adapts template aliases to
the live main host and existing tool ViewIds by kind; extra descriptors and document
views are retained. Reset replaces topology nodes without recreating retained
content. Current saved layouts restore without template rearrangement.
The docking scene's `editor.dock_overflow` component supplies the overflow view,
target/index, descriptor, title, content path, scope and identity text. Native setup
validates source membership and placement and prepares content before committing
the existing docking command. Inert `ui.docking` data alone grants no controller or
diagnostic-script authority.
The Inspector library retains only Bevy/UI production dependencies; the ordinary
editor hosts its non-document value consumer through
`editor://fixtures/inspector.scene.ron`, not a separate example target or Cargo profile.

Scene mounting uses
`kedra_desktop::replace_mounted(world, kind, title, size, mount, rollback)`.
The callbacks receive `(world, candidate_surface)`; mount returns
`Result<(), String>` and rollback returns `()`. Desktop invokes rollback on a
returned mount error or a later native
root/camera/visibility failure, before retiring candidate entities. The editor
clears only candidate catalog/workspace/value bindings and restores motion
configuration; the prior primary surface and focus remain owned by desktop.
Native acknowledgment, rendered frames and interaction acceptance remain separate.

See [startup](../../docs/site/en/contributing/contracts/startup.md),
[docking](../../docs/site/en/contributing/contracts/ui/docking.md)
and [native tests](../../docs/site/en/contributing/contracts/ui/native-docking-tests.md).
See [Scene View](../../docs/site/en/contributing/contracts/editor/scene-view.md)
for ownership, supported data, limits, explicit asset binding and native acceptance commands.

## Contract tests

`cargo test -p kedra_editor --test contracts --locked` checks editor-owned authored
sources, CSS/design rules, icon density metadata and reference integrity. These
are source/CPU contracts, not screenshot or physical-input acceptance. Private
feature tests use local support fixtures rather than other suites; the
[testing guide](../../docs/site/en/contributing/contracts/testing.md)
maps owners, placement and native routes.
