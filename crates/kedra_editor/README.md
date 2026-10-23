# kedra_editor

Editor-layer project startup feature and composition. `domain` contains ordinary
Rust models, drafts and validated lifecycle transitions without ECS handles.
`model` is the thin Bevy resource/state adapter; `actions`, `jobs` and `screens`
own semantic input, effects and retained views respectively.

The desktop host opens only validated projects and does not run project code.
Asynchronous completions carry generations. Creation cannot be cancelled with a
false rollback promise. The workspace has a read-only Assets browser with a lazy
folder tree, Unicode search, virtualized grid/list and source document Open through
one project-owned authoring workspace. Hierarchy displays composed source nodes,
including nested prefabs, with search, expansion and shared document selection.
The read-only Inspector follows source selection with lock, search and shared
reflected field presentations. Document tabs host the initial read-only
Scene View for supported spatial/UI sources; PNG/static-GLB assets use separate
asset-preview views. An explicit matching verified project delivery is required.
Spatial camera navigation, grid/inspection controls and scoped opaque-mesh picking
are implemented. Their native input acceptance remains separate from compilation.
Scene editing, project execution and document Save are not implemented.

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

See [startup](../../docs/startup.md), [docking](../../docs/ui/docking-implementation.md)
and [native tests](../../docs/ui/docking-tests.md).
See [Scene View](../../docs/ui/scene-view.md) for ownership, supported data,
limits, explicit asset binding and native acceptance commands.

## Contract tests

`cargo test -p kedra_editor --test contracts --locked` checks editor-owned authored
sources, CSS/design rules, icon density metadata and reference integrity. These
are source/CPU contracts, not screenshot or physical-input acceptance. Private
feature tests use local support fixtures rather than other suites; the
[testing guide](../../docs/testing.md) maps owners, placement and native routes.
