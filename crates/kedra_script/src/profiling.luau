-- Logical duration may include suspension. Native CPU scopes cannot cross yields.
return function(beginOperation, finishOperation)
    return function(name, callback, ...)
        local token = beginOperation(name)
        local result = table.pack(pcall(callback, ...))
        finishOperation(token, result[1])
        if not result[1] then error(result[2], 0) end
        return table.unpack(result, 2, result.n)
    end
end
