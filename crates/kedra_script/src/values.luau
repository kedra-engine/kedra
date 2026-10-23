-- Validate JSON-domain values before the maintained serde decoder traverses them.
local null = json.null
local assert, type, next = assert, type, next
local abs, huge = math.abs, math.huge
local function validate(value, depth, seen)
    assert(depth <= 32, "Script value exceeds depth 32")
    local kind = type(value)
    if kind == "number" then
        assert(value == value and abs(value) ~= huge, "Nonfinite script number")
    elseif kind == "table" then
        assert(not seen[value], "Recursive script table")
        seen[value] = true
        for key, child in next, value do
            validate(key, depth + 1, seen)
            validate(child, depth + 1, seen)
        end
        seen[value] = nil
    else
        assert(value == null or kind == "nil" or kind == "boolean" or kind == "string",
            "Unsupported JSON-domain script value: " .. kind)
    end
end
return function(value) validate(value, 0, {}) end
