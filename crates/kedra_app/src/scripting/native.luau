-- Host binding factory keeps the private native adapter out of the public context.
-- This code executes in the application, not in the external QA process.
return function(native: NativeApp)
local api = {window = native.window, node = native.node}

local function await_receipt(receipt: Receipt)
    local current = receipt
    wait.until_true("operation " .. receipt.id, function()
        current = native.receipt({id = receipt.id})
        assert(current.state == "pending" or current.state == "succeeded" or current.state == "failed")
        return current.state ~= "pending"
    end, 12000)
    assert(current.state == "succeeded", "Native operation failed: " .. tostring(current.detail))
    return current.detail
end

function api.input(window: string, input: Input): Delivered
    return await_receipt(native.submit("app.input", {window = window, input = input})) :: Delivered
end
function api.resize(window: string, width: number, height: number, dpr)
    return await_receipt(native.submit("app.resize", {window = window, width = width, height = height, dpr = dpr})) :: Delivered
end
function api.capture(window: string)
    return await_receipt(native.submit("app.capture", {window = window})) :: Frame
end
function api.wait_node(id: string)
    local node
    wait.until_true(id, function()
        node = native.node(id)
        return node ~= nil
    end, 40000)
    return node
end
function api.click(id: string)
    local node = api.wait_node(id)
    assert(node.enabled, "Target is disabled: " .. id)
    return api.input(node.window, {kind = "click", element = id})
end
function api.field(id: string, value: string)
    local node = api.wait_node(id)
    assert(node.enabled, "Target is disabled: " .. id)
    api.input(node.window, {kind = "click", element = id})
    wait.until_true("pointer-acquired focus for " .. id, function()
        local current = native.node(id)
        return current ~= nil and current.focused
    end, 40000)
    api.input(node.window, {kind = "key", key = "select_all"})
    api.input(node.window, {kind = "text", text = value})
    wait.until_true("text delivery to " .. id, function()
        local current = native.node(id)
        return current ~= nil and current.value == value
    end, 40000)
end
return table.freeze(api)
end
