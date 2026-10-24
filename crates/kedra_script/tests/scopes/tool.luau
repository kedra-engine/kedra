--!nolint FunctionUnused
--!nolint BuiltinGlobalWrite
-- Rust invokes this global entrypoint after the chunk has loaded.
-- The declared marker is intentionally written to probe per-VM global isolation.
function run(ctx: TestToolContext)
    assert(game_scope_only == nil, "Game globals must not leak into tool scopes")
    assert(ctx.editor.selection() == "Main Camera")
    assert(ctx.game.count() == 1)
    assert(not pcall(function() ctx.editor.selection = function() return "Other" end end))
    tool_scope_only = true
end
