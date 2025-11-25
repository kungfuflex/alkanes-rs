-- Batch UTXO balance fetching for optimized alkanes execution
-- This script fetches UTXOs for an address and their alkane balances in a single evalscript call
-- Args: address, protocol_tag (default: 1 for alkanes), block_tag (optional)

local address = args[1]
local protocol_tag = args[2] or 1
local block_tag = args[3]

-- Fetch all UTXOs for the address
local utxos = _RPC.esplora_addressutxo(address)
if not utxos then
    return { utxos = {}, error = "Failed to fetch UTXOs" }
end

-- Result table
local result = {
    utxos = {},
    count = 0
}

-- For each UTXO, fetch its alkane balances
for i, utxo in ipairs(utxos) do
    local txid = utxo.txid
    local vout = utxo.vout
    local value = utxo.value
    
    -- Query protorunes balance for this UTXO
    local balance_response = _RPC.protorunes_by_outpoint(
        txid,
        vout,
        block_tag,
        protocol_tag
    )
    
    -- Build UTXO entry with balance info
    local utxo_entry = {
        txid = txid,
        vout = vout,
        value = value,
        status = utxo.status,
        balances = {}
    }
    
    -- Extract alkane balances if available
    if balance_response and balance_response.balance_sheet and balance_response.balance_sheet.cached then
        local balances = balance_response.balance_sheet.cached.balances
        if balances then
            for alkane_id, amount in pairs(balances) do
                table.insert(utxo_entry.balances, {
                    block = alkane_id.block,
                    tx = alkane_id.tx,
                    amount = amount
                })
            end
        end
    end
    
    table.insert(result.utxos, utxo_entry)
    result.count = result.count + 1
end

return result
