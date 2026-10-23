--!nolint FunctionUnused
-- Explicitly invoked through scene.remote, never by scene loading.
function run(ctx: KedraAppContext, args)
    local app = ctx.app
    local panel = app.wait_node("game.panel")
    local target = app.wait_node("game.target")
    assert(panel.window == target.window, "Scene UI lost its native window")
    assert(target.rect[3] - target.rect[1] == 64, "Scene target height changed")
    assert(target.rect[2] - target.rect[0] == 120, "Scene target width changed")
    return {frame = app.capture(target.window), source_ready = true}
end
