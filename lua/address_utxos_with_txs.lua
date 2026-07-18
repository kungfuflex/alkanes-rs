-- Batch fetch UTXOs for an address with full transaction details
-- This replaces multiple individual esplora_tx calls with a single script execution
-- Args: address

local address = args[1]

-- Fetch all UTXOs for the address
local utxos = _RPC.esplora_addressutxo(address)
if not utxos then
    return { 
        utxos = {},
        error = "Failed to fetch UTXOs for address"
    }
end

-- Result table
local result = {
    utxos = {},
    count = 0
}

-- For each UTXO, fetch its full transaction details
for i, utxo in ipairs(utxos) do
    local txid = utxo.txid
    local vout = utxo.vout
    
    -- Fetch full transaction data for this UTXO
    local tx_data = _RPC.esplora_tx(txid)
    
    -- Build UTXO entry with transaction data
    local utxo_entry = {
        txid = txid,
        vout = vout,
        value = utxo.value,
        status = utxo.status,
        tx = tx_data  -- Include full transaction data
    }
    
    table.insert(result.utxos, utxo_entry)
    result.count = result.count + 1
end

return result
