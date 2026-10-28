# Scoped Luau authoring

This editor-layer adapter exposes native authoring edit leases to explicitly
authorized `kedra_script` programs. It installs no runtime, discovers no project
files and owns no source registry, Undo stack or filesystem writer. The editor
provides its real Documents backend and source grants.

```lua
function run(ctx, args)
    local edit = ctx.edit.begin("Arrange prefab objects")
    local first = edit:nodes("game://Prefabs/First.prefab.ron")[1]
    local second = edit:nodes("game://Prefabs/Second.prefab.ron")[1]
    edit:rename(first, "Entrance")
    edit:set_property(second, "ui.caption", "text", "Exit")
    return {operation = edit:commit()}
end
```

Node userdata is tied to the edit that enumerated it. Every call re-enters the
existing runtime Scope. Retired/foreign scopes and mixed-transaction nodes refuse.
An exception or dropped open lease never implicitly commits. Successful commit
closes the lease and returns an opaque operation ID string, or nil for a no-op.
Cancel closes it without publication. Commit is not Save or Git commit; source
Undo/Redo restores recorded state without retaining or re-running the program.
Structural methods `remove_node`, `reparent` and `remove_component` reuse native scene
operations and full host compilation; roots, cycles and cross-document moves refuse.

The host must validate exact source scope, revisions, native field types and complete
catalog before publication. JSON transports values through the existing serde/Bevy
reflection path; it is not a source document language. Handles/entity references
require separate host admission and are not manufactured by this scalar edit API.
General script discovery, automatic project execution and arbitrary filesystem
effects are not provided. See ADR 0038 for the system boundary and remaining work.
