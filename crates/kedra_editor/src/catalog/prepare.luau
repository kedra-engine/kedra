--!nolint FunctionUnused
-- The host invokes run; fixture mounting remains a host-owned precondition.
function run(ctx: KedraAppContext, args)
    local app = ctx.app
    local window = args.window
    app.resize(window, 640, 360, 1)
    wait.until_true("measured specimen", function()
        local target = app.node("specimen.target")
        return target ~= nil and target.rect[3] > target.rect[1] + 1
    end, 12000)
    app.input(window, {kind = "move", position = {1, 1}})
    if args.state == "hover" or args.state == "pressed" then
        local target = app.wait_node("specimen.target")
        local r = target.rect
        app.input(window, {kind = "move", position = {(r[1] + r[3]) / 2, (r[2] + r[4]) / 2}})
        if args.state == "pressed" then app.input(window, {kind = "button", pressed = true}) end
    elseif args.state == "focus" then
        app.input(window, {kind = "key", key = "tab"})
        wait.until_true("native keyboard focus", function()
            local node = app.node(if args.field then "specimen.input" else "specimen.target")
            return node ~= nil and node.focused
        end, 12000)
        if args.field then app.input(window, {kind = "key", key = "end"}) end
    elseif args.state == "disabled" then
        assert(not app.wait_node("specimen.target").enabled, "Disabled fixture is still enabled")
    else
        assert(args.state == "default", "Unsupported component state")
    end
    return {window = window, delivered = true}
end
