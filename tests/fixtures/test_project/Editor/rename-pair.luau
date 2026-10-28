local function named(edit, path, name)
    for _, node in edit:nodes(path) do
        if node.name == name then return node end
    end
    error("Expected fixture source node is missing")
end

function run(ctx)
    local edit = ctx.edit.begin("Rename fixture pair")
    local hud = named(edit, "game://Hud.prefab.ron", "Passive UI fixture")
    local cuboid = named(edit, "game://Cuboid.prefab.ron", "Textured cuboid")
    wait.updates(1)
    edit:rename(hud, "Script HUD 入口")
    wait.updates(1)
    edit:rename(cuboid, "Script cuboid")
    wait.updates(1)
    local operation = edit:commit()
    assert(operation ~= nil, "Expected one accepted operation")
    return { operation = operation }
end
