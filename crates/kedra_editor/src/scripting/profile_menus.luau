-- Diagnostic workload, not visual or physical-input acceptance.
-- The application runs the interaction; external RPC polling is not timed.
function run(ctx: EditorContext, args)
    local iterations = args.iterations or 20
    assert(type(iterations) == "number" and iterations % 1 == 0
        and iterations >= 1 and iterations <= 64, "Expected 1..64 menu iterations")
    assert(ctx.editor.workspace().bound == true, "Open an editor workspace before profiling")
    local app = ctx.app
    local window = app.window()
    local before = app.performance()
    local samples = {}
    for _ = 1, iterations do
        local started = runtime.elapsed_ms()
        ctx.profiler.operation("menu.cycle", function()
            app.click("workspace/panels")
            app.wait_node("dock-menu")
            app.input(window, {kind = "key", key = "escape"})
            wait.until_true("dismissed profiling menu", function()
                return app.node("dock-menu") == nil
            end, 12000)
        end)
        ctx.profiler.counter("completed_cycles", #samples + 1)
        table.insert(samples, runtime.elapsed_ms() - started)
    end
    return {
        workload = "editor.profile.menus",
        timing = "in-application sequence, not physical-input-to-photon",
        before = before,
        after = app.performance(),
        menu_roundtrip_ms = samples,
    }
end
