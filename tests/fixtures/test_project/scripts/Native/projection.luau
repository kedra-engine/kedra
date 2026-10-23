-- Native operations mutate the ordinary model; real input activates tabs/edits drafts.
-- Rust independently verifies the returned topology, retained identities and fences.
local function ready(ctx: NativeProjectionContext): NativeProjectionMounted
    local state: NativeProjectionState? = nil
    wait.until_true("native docking projection", function()
        local current = ctx.projection.state()
        if current.mounted and current.projected then
            state = current
            return true
        end
        return false
    end, 20000)
    assert(state ~= nil and state.mounted)
    return state
end

local function tab(ctx: NativeProjectionContext, name: string): string
    local id = ready(ctx).views[name].tab
    assert(type(id) == "string", "View has no projected tab")
    return id
end

local function perform(ctx: NativeProjectionContext, action: NativeProjectionAction)
    local projection = ctx.projection
    local before = ready(ctx)
    if action == "reorder" then
        projection.move("D", "D", nil, 0, nil)
    elseif action == "split" then
        projection.move("D", "A", "right", 0, 0.5)
    elseif action == "activate" then
        ctx.app.click(tab(ctx, "B"))
    elseif action == "maximize" then
        local group = before.views.D.group
        local maximized = false
        for _, host in before.snapshot.hosts do
            maximized = host.maximized == group
        end
        projection.maximize(if maximized then nil else "D")
    elseif action == "visibility" then
        projection.visibility("D", "A", before.views.D.hidden)
    elseif action == "restore" then
        projection.restore()
    else
        error("Unknown projection action")
    end
    wait.updates(2)
    return ready(ctx)
end

local projection_sequence = {}

function projection_sequence.run(ctx: NativeProjectionContext, action: NativeProjectionAction | "exercise")
    local app = ctx.app
    app.resize(app.window(), 1000, 800, 1)
    local before = ready(ctx)
    app.wait_node("lab.A/heading")
    if action ~= "exercise" then
        return {before = before, after = perform(ctx, action), frame = app.capture(app.window())}
    end
    app.click(tab(ctx, "D"))
    app.field("lab.D/draft", "Retained 世界 draft")
    local draft_before = app.wait_node("lab.D/draft")
    local steps = {}
    local actions: {NativeProjectionAction} = {
        "reorder", "split", "activate", "maximize", "maximize", "visibility", "visibility", "restore",
    }
    for _, name in actions do
        local state = perform(ctx, name)
        table.insert(steps, {action = name, state = state, frame = app.capture(app.window())})
    end
    -- Restore the original active C tab, then use real input to expose D again.
    app.click(tab(ctx, "D"))
    local draft = app.wait_node("lab.D/draft")
    assert(draft.value == "Retained 世界 draft", "Projection rebuilt draft content")
    return {before = before, steps = steps, after = ready(ctx), draft_before = draft_before, draft = draft,
        frame = app.capture(app.window())}
end

return table.freeze(projection_sequence)
