--!nolint FunctionUnused
--!nolint BuiltinGlobalWrite
-- Rust invokes this global entrypoint after the chunk has loaded.
-- The declared marker is intentionally written to probe per-VM global isolation.
function run(ctx: TestGameContext)
    assert(game == nil and editor == nil, "Host capabilities belong only to ctx")
    assert(ctx.editor == nil, "Game scripts must not receive the tool API")
    assert(world == nil, "Generic reflected World access must not be exposed")
    assert(entity == nil and register_callback == nil)
    assert(io == nil and package == nil and type(require) == "function")
    assert(tool_scope_only == nil, "Tool globals must not leak into game scopes")
    assert(not pcall(function() ctx.game.increment = function() end end))
    assert(not pcall(function()
        ctx.game = {count = function() return 0 end, increment = function() end}
    end))
    local before = ctx.game.count()
    ctx.game.increment()
    assert(ctx.game.count() == before + 1)
    game_scope_only = true
end
