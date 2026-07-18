-- Execute multiple RPC calls in a single batch (replacement for sandshrew_multicall)
-- Args: array of [method, params] tuples
-- Example: [["btc_getblockcount", []], ["btc_getblockhash", [100]]]

-- Each element in args should be a table with [method, params]
local results = {}

for i, call in ipairs(args) do
    -- Each call should be a 2-element array: [method, params]
    if type(call) ~= "table" or #call ~= 2 then
        return {
            error = "Each multicall entry must be a tuple of [method, params]",
            index = i
        }
    end
    
    local method = call[1]
    local params = call[2]
    
    if type(method) ~= "string" then
        return {
            error = "Method name must be a string",
            index = i
        }
    end
    
    if type(params) ~= "table" then
        return {
            error = "Method params must be an array",
            index = i
        }
    end
    
    -- Execute the RPC call
    local success, result = pcall(function()
        -- Use _RPC table to call the method dynamically
        local rpc_func = _RPC[method]
        if not rpc_func then
            error("Method not found: " .. method)
        end
        
        -- Call the RPC method with unpacked params
        if #params == 0 then
            return rpc_func()
        elseif #params == 1 then
            return rpc_func(params[1])
        elseif #params == 2 then
            return rpc_func(params[1], params[2])
        elseif #params == 3 then
            return rpc_func(params[1], params[2], params[3])
        elseif #params == 4 then
            return rpc_func(params[1], params[2], params[3], params[4])
        elseif #params == 5 then
            return rpc_func(params[1], params[2], params[3], params[4], params[5])
        else
            -- For more params, use table.unpack (Lua 5.2+) or unpack (Lua 5.1)
            local unpack_func = table.unpack or unpack
            return rpc_func(unpack_func(params))
        end
    end)
    
    if success then
        table.insert(results, { result = result })
    else
        table.insert(results, { error = { message = tostring(result) } })
    end
end

return results
