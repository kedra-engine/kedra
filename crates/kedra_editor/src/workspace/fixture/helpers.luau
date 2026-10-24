--!strict
local helpers = {}

function helpers.state(ctx: DockContext): {ready: true, generation: number}
    local current = ctx.dock.state()
    assert(current.ready, "Dock diagnostic scene is not bound")
    return current
end

return table.freeze(helpers)
