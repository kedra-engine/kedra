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
Inspector is an empty dockable slot. Document tabs host the initial read-only
Scene View for supported spatial/UI sources; PNG/static-GLB assets use separate
asset-preview views. An explicit matching verified project delivery is required.
Spatial camera navigation, grid/inspection controls and scoped opaque-mesh picking
are implemented. Their native input acceptance remains separate from compilation.
Scene editing, project execution and document Save are not implemented.

See [startup](../../docs/startup.md), [docking](../../docs/ui/docking-implementation.md)
and [native tests](../../docs/ui/docking-tests.md).
See [Scene View](../../docs/ui/scene-view.md) for ownership, supported data,
limits, explicit asset binding and native acceptance commands.
