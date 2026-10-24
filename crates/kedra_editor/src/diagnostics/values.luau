--!nolint FunctionUnused
-- The host invokes run; native controllers own refresh and retained presentation.
function run(ctx: AppContext, args)
    local app = ctx.app
    for _, id in ipairs({args.boolean, args.gain}) do
        assert(type(id) == "string" and string.sub(id, 1, 17) == "fixture/settings/",
            "Refusing a field outside the reflected-value composition")
    end
    assert(app.wait_node(args.boolean).value == "true", "Expected initial host value")
    assert(app.wait_node(args.gain).value == "25%", "Expected specialized field")
    app.click("fixture/refresh")
    wait.until_true("host-owned refresh", function()
        return app.wait_node(args.boolean).value == "false"
    end, 12000)
    app.click("fixture/toggle")
    wait.until_true("explicit default presentation", function()
        return app.wait_node(args.gain).value == "0.25"
    end, 12000)
    return {language = "Luau", sequence = "refresh-default"}
end
