--!nolint BuiltinGlobalWrite
-- Host bootstrap intentionally publishes the declared wait capability table.
-- Shared coroutine vocabulary. Bevy schedules and clocks own resumption.
local function suspend(kind: string, value: number)
    assert(type(value) == "number" and value > 0 and value <= 1000000, "Invalid wait duration")
    if kind == "updates" or kind == "fixed" then
        assert(value % 1 == 0, "Tick counts must be integers")
    end
    return coroutine.yield({kind = kind, value = value})
end

wait = table.freeze({
    updates = function(n: number) return suspend("updates", n) end,
    fixed = function(n: number) return suspend("fixed", n) end,
    game_seconds = function(n: number) return suspend("game", n) end,
    real_seconds = function(n: number) return suspend("real", n) end,
    until_true = function(label: string, predicate, timeout_ms: number)
        assert(type(label) == "string" and type(predicate) == "function")
        assert(type(timeout_ms) == "number" and timeout_ms > 0 and timeout_ms <= 600000)
        local deadline = runtime.elapsed_ms() + timeout_ms
        while true do
            if predicate() then return end
            assert(runtime.elapsed_ms() < deadline, "Timed out waiting for " .. label)
            suspend("updates", 1)
        end
    end,
})
