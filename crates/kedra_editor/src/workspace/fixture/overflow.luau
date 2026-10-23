--!nolint FunctionUnused
-- The host invokes the global run entrypoint.
local helpers = require("./helpers")

function run(ctx: KedraDockContext, _args)
    helpers.state(ctx)
    ctx.dock.overflow()
    wait.updates(1)
    return {sequence = "dock.overflow"}
end
