-- Fetch spendable UTXOs for an address, filtering out immature coinbase outputs
-- Args: address
-- Returns: list of UTXOs that are safe to spend (excludes immature coinbase)

local address = args[1]

-- Coinbase maturity constant (100 blocks required before coinbase outputs are spendable)
local COINBASE_MATURITY = 100

-- Get current block height
local current_height = _RPC.btc_getblockcount() or 0

-- Get UTXOs for the address
local utxos = _RPC.esplora_addressutxo(address) or {}

-- Cache for transaction coinbase status to avoid redundant RPC calls
local tx_coinbase_cache = {}

-- Helper function to check if a transaction is a coinbase transaction
local function is_coinbase_tx(txid)
    -- Check cache first
    if tx_coinbase_cache[txid] ~= nil then
        return tx_coinbase_cache[txid]
    end

    -- Fetch transaction details from esplora
    local tx = _RPC.esplora_tx(txid)
    if not tx then
        tx_coinbase_cache[txid] = false
        return false
    end

    -- A coinbase transaction has vin[0].is_coinbase = true
    local is_coinbase = false
    if tx.vin and #tx.vin > 0 then
        local first_vin = tx.vin[1]
        if first_vin and first_vin.is_coinbase then
            is_coinbase = true
        end
    end

    tx_coinbase_cache[txid] = is_coinbase
    return is_coinbase
end

-- Process UTXOs and filter out immature coinbase
local spendable_utxos = {}
local immature_utxos = {}

for _, utxo in ipairs(utxos) do
    local txid = utxo.txid
    local vout = utxo.vout
    local value = utxo.value
    local key = txid .. ":" .. vout

    -- Get confirmations
    local height = nil
    local confirmations = 0
    if utxo.status and utxo.status.block_height then
        height = utxo.status.block_height
        confirmations = current_height - height + 1
    end

    -- Check if this is a coinbase transaction
    local is_coinbase = is_coinbase_tx(txid)

    -- Build UTXO entry
    local utxo_entry = {
        txid = txid,
        vout = vout,
        value = value,
        outpoint = key,
        height = height,
        confirmations = confirmations,
        is_coinbase = is_coinbase
    }

    -- Filter: skip unconfirmed UTXOs and immature coinbase
    if not utxo.status or not utxo.status.confirmed then
        -- Unconfirmed - skip for spending
    elseif is_coinbase and confirmations < COINBASE_MATURITY then
        -- Immature coinbase - track separately
        utxo_entry.maturity_blocks_remaining = COINBASE_MATURITY - confirmations
        table.insert(immature_utxos, utxo_entry)
    else
        -- Spendable
        table.insert(spendable_utxos, utxo_entry)
    end
end

return {
    spendable = spendable_utxos,
    immature = immature_utxos,
    currentHeight = current_height,
    address = address
}
