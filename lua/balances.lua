local address = args[1]
if not address then
    return { error = "Address required" }
end

local protocol_tag = args[2] or "1" -- Default protocol tag for alkanes?

-- Helper to check if a table is empty (for dictionary-like tables)
local function is_empty(t)
    if t == nil then return true end
    return next(t) == nil
end

-- 1. Fetch Data
local utxos = _RPC.esplora_addressutxo(address)
if not utxos then return { error = "Failed to fetch UTXOs" } end

-- We construct the protorunes param object carefully
local protorunes_param = {}
protorunes_param.address = address
protorunes_param.protocolTag = protocol_tag

local protorunes_result = _RPC.alkanes_protorunesbyaddress(protorunes_param)
local ord_outputs = _RPC.ord_outputs(address)
local ord_height = _RPC.ord_blockheight() or 0
local metashrew_height = _RPC.metashrew_height() or 0

-- 2. Build Maps
local runes_map = {}
if protorunes_result and protorunes_result.outpoints then
    for _, item in ipairs(protorunes_result.outpoints) do
        runes_map[item.outpoint] = item.runes
    end
end

local ord_map = {}
if ord_outputs then
    for _, output in ipairs(ord_outputs) do
        if output.outpoint then
            ord_map[output.outpoint] = {
                inscriptions = output.inscriptions,
                runes = output.runes
            }
        end
    end
end

local max_indexed_height = math.max(ord_height, metashrew_height)

-- 3. Classify UTXOs
local spendable = {}
local assets = {}
local pending = {}

for _, utxo in ipairs(utxos) do
    -- esplora utxo format: { txid: "...", vout: 0, value: 100, status: { confirmed: true, block_height: ... } }
    local txid = utxo.txid
    local vout = utxo.vout
    local value = utxo.value
    
    local height = nil
    if utxo.status and utxo.status.block_height then
        height = utxo.status.block_height
    end

    local outpoint = string.format("%s:%d", txid, vout)
    
    -- Check for attached assets
    local my_runes = runes_map[outpoint]
    local ord_data = ord_map[outpoint]
    local my_inscriptions = ord_data and ord_data.inscriptions
    local my_ord_runes = ord_data and ord_data.runes

    local has_runes = my_runes and #my_runes > 0
    local has_inscriptions = my_inscriptions and #my_inscriptions > 0
    local has_ord_runes = my_ord_runes and not is_empty(my_ord_runes)

    local entry = {
        outpoint = outpoint,
        value = value,
        height = height,
        runes = my_runes,
        inscriptions = my_inscriptions,
        ord_runes = my_ord_runes
    }

    -- Determine status
    local is_pending = false
    if not height then
        is_pending = true -- Mempool
    elseif height > max_indexed_height then
        is_pending = true -- Confirmed but not yet indexed
    end

    if is_pending then
        table.insert(pending, entry)
    elseif has_runes or has_inscriptions or has_ord_runes then
        table.insert(assets, entry)
    else
        table.insert(spendable, entry)
    end
end

-- 4. Sort
local function compare_utxos(a, b)
    local ha = a.height
    local hb = b.height

    if not ha and not hb then return false end -- Both mempool? Equal.
    if not ha then return false end -- a is mempool (pending/new), b is confirmed. a > b? Rust: None > Some.
    -- Wait, Rust sort logic was:
    -- (None, None) => Equal
    -- (None, Some) => Greater (Mempool > Confirmed)
    -- (Some, None) => Less
    -- (Some, Some) => a.cmp(b) (Oldest first)
    
    -- Lua sort is "less than".
    -- If we want [Oldest ... Newest ... Mempool]
    -- Oldest < Newest.
    -- Confirmed < Mempool.
    
    if not hb then return true end -- b is mempool, a is confirmed. a < b.
    
    return ha < hb
end

table.sort(spendable, compare_utxos)
table.sort(assets, compare_utxos)
table.sort(pending, compare_utxos)

-- 5. Return
return {
    spendable = spendable,
    assets = assets,
    pending = pending,
    ordHeight = ord_height,
    metashrewHeight = metashrew_height
}
