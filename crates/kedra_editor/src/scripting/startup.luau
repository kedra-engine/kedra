--!nolint FunctionUnused
-- The Rust host calls the global run entrypoint; there is no Luau caller in this file.
-- An ordinary Luau program using the editor's real state API and normal Bevy input.
function run(ctx: EditorContext, args)
    local app = ctx.app
    local editor = ctx.editor
    local frames = {}
    local function phase()
        return editor.state().phase
    end
    local function entry_open()
        return editor.state().entry ~= json.null
    end
    local function entry(): Entry
        local current = editor.state().entry
        assert(type(current) == "table", "Startup form is not open")
        return current
    end
    local function key(name)
        app.input(app.window(), {kind = "key", key = name})
    end
    local function frame(label, dpr)
        -- Measure the full-window panel independently of the readback receipt.
        -- Native window managers can round a requested resize to their pixel grid.
        local panel = app.wait_node("projects.panel")
        local size = {math.floor(panel.rect[3] * dpr + 0.5), math.floor(panel.rect[4] * dpr + 0.5)}
        assert(size[1] > 0 and size[2] > 0, "Project manager has no usable viewport")
        table.insert(frames, {label = label, dpr = dpr, size = size, receipt = app.capture(app.window())})
    end
    wait.until_true("project manager", function() return phase() == "projects" end, 40000)
    assert(editor.state().preferences == args.preferences, "Refusing personal editor preferences")
    assert(not entry_open(), "Startup form is already open")
    assert(editor.workspace().bound == false, "Startup invented a bound workspace")
    for _, dpr in ipairs({1, 2}) do
        app.resize(app.window(), 1120, 720, dpr)
        frame("manager", dpr)
        app.click("projects.new")
        wait.until_true("create form", entry_open, 40000)
        wait.until_true("modal autofocus", function() return app.wait_node("entry.name").focused end, 40000)
        key("tab")
        assert(app.wait_node("entry.directory").focused, "Tab escaped modal")
        key("shift_tab")
        assert(app.wait_node("entry.name").focused, "Reverse Tab escaped modal")
        frame("create", dpr)
        app.field("entry.name", "QA Кедра")
        assert(entry().name == "QA Кедра", "Unicode native entry lost")
        app.field("entry.name", "invalid/name")
        key("enter")
        wait.until_true("validation failure", function() return entry().error ~= "" end, 40000)
        assert(entry().name == "invalid/name", "Validation discarded invalid draft")
        frame("invalid create", dpr)
        key("escape")
        wait.until_true("form cancellation", function() return not entry_open() end, 40000)
        app.click("projects.open")
        wait.until_true("open form", entry_open, 40000)
        frame("open", dpr)
        app.click("entry.cancel")
        wait.until_true("open form cancellation", function() return not entry_open() end, 40000)
    end
    return {verification = "behavior_only", language = "Luau", frames = frames}
end
