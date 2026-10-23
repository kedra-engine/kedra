--!nolint FunctionUnused
-- The host invokes run; diagnostic settlement/reset remains host-owned.
local helpers = require("./helpers")

function run(ctx: KedraDockContext, args)
    local app = ctx.app
    local dock = ctx.dock
    local function state()
        return helpers.state(ctx)
    end
    state()
    local function validate_dimension(dimension)
        assert(type(dimension) == "number" and dimension >= 64 and dimension <= 8192
            and dimension == math.floor(dimension), "Unsupported reset dimensions")
    end
    validate_dimension(args.width)
    validate_dimension(args.height)
    local window = app.window()
    app.input(window, {kind = "modifier", modifier = "shift", pressed = false})
    app.input(window, {kind = "button", pressed = false})
    app.input(window, {kind = "key", key = "escape"})
    app.resize(window, args.width, args.height, 1)
    local generation = state().generation
    dock.reset()
    wait.until_true("source reset", function() return state().generation > generation end, 12000)
    return {sequence = "dock.reset", generation = state().generation}
end
