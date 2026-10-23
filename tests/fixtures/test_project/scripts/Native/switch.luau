--!nolint FunctionUnused
-- Native input drives Bevy Activate and the game-owned switch Behavior/Sequence.
function run(ctx: KedraAppContext, args)
    local app = ctx.app
    local window = app.window()
    app.resize(window, 320, 320, 1)
    wait.until_true("scene switch geometry", function()
        local node = app.node("game.switch")
        return node ~= nil and node.rect[1] == 40 and node.rect[2] == 50
            and node.rect[3] == 160 and node.rect[4] == 114
    end, 20000)
    local before = app.wait_node("game.switch")
    local before_frame = app.capture(window)
    app.click("game.switch")
    wait.until_true("native scene activation", function()
        local node = app.node("game.switch")
        return node ~= nil and node.background[1] == 30 and node.background[2] == 100
            and node.background[3] == 190 and node.background[4] == 255
    end, 20000)
    local after = app.wait_node("game.switch")
    return {before = before, after = after, before_frame = before_frame, after_frame = app.capture(window)}
end
