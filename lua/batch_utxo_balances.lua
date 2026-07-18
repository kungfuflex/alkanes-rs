-- Batch UTXO balance fetching for optimized alkanes execution
-- This script fetches UTXOs for an address and their alkane balances in a single evalscript call
-- Args: address, protocol_tag (default: 1 for alkanes), block_tag (optional)
--
-- IMPORTANT: Uses alkanes_protorunesbyaddress instead of protorunes_by_outpoint
-- because the per-outpoint lookup only works for outpoints that are already in
-- the alkane index, not for all esplora UTXOs.

local address = args[1]
local protocol_tag = args[2] or "1"
local block_tag = args[3]

-- Fetch all UTXOs for the address from esplora
local utxos = _RPC.esplora_addressutxo(address)
if not utxos then
    return { utxos = {}, error = "Failed to fetch UTXOs" }
end

-- Fetch alkane balances for the address using protorunesbyaddress
-- This returns all outpoints with their alkane balances
local protorunes = _RPC.alkanes_protorunesbyaddress({
    address = address,
    protocolTag = protocol_tag
}) or {}

-- Build a lookup map from alkane outpoints
-- Key format: "txid:vout" -> balances array
local alkane_balances_map = {}
if protorunes.outpoints then
    for _, outpoint_response in ipairs(protorunes.outpoints) do
        if outpoint_response.outpoint then
            local txid = outpoint_response.outpoint.txid
            local vout = outpoint_response.outpoint.vout
            local key = txid .. ":" .. tostring(vout)

            -- Extract balances from the response
            local balances = {}
            if outpoint_response.balances then
                for _, balance in ipairs(outpoint_response.balances) do
                    if balance.block ~= nil and balance.tx ~= nil and balance.amount ~= nil then
                        table.insert(balances, {
                            block = balance.block,
                            tx = balance.tx,
                            amount = tonumber(balance.amount) or 0
                        })
                    end
                end
            end

            -- Also check balance_sheet format (older response format)
            if outpoint_response.balance_sheet and outpoint_response.balance_sheet.cached then
                local cached_balances = outpoint_response.balance_sheet.cached.balances
                if cached_balances then
                    for _, balance in ipairs(cached_balances) do
                        if balance.block ~= nil and balance.tx ~= nil and balance.amount ~= nil then
                            table.insert(balances, {
                                block = balance.block,
                                tx = balance.tx,
                                amount = tonumber(balance.amount) or 0
                            })
                        end
                    end
                end
            end

            if #balances > 0 then
                alkane_balances_map[key] = balances
            end
        end
    end
end

-- Result table
local result = {
    utxos = {},
    count = 0
}

-- For each esplora UTXO, look up any alkane balances from the map
for i, utxo in ipairs(utxos) do
    local txid = utxo.txid
    local vout = utxo.vout
    local value = utxo.value
    local key = txid .. ":" .. tostring(vout)

    -- Build UTXO entry with balance info
    local utxo_entry = {
        txid = txid,
        vout = vout,
        value = value,
        status = utxo.status,
        balances = alkane_balances_map[key] or {}
    }

    table.insert(result.utxos, utxo_entry)
    result.count = result.count + 1
end

return result
