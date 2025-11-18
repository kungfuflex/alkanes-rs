-- Example Lua script demonstrating the flat _RPC table

-- Example 1: Get current block height from Bitcoin Core
local height = _RPC.btc_getblockcount()
print("Current block height:", height)

-- Example 2: Get address UTXOs from Esplora
local address = args[1] or "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
local utxos = _RPC.esplora_addressutxo(address)
print("UTXOs for address:", #utxos)

-- Example 3: Get ord block height
local ord_height = _RPC.ord_blockheight()
print("Ord height:", ord_height)

-- Example 4: Get metashrew height
local metashrew_height = _RPC.metashrew_height()
print("Metashrew height:", metashrew_height)

-- Example 5: Complex calculation
local total_value = 0
for i, utxo in ipairs(utxos) do
  total_value = total_value + utxo.value
end

-- Return a result object
return {
  address = address,
  btc_height = height,
  ord_height = ord_height,
  metashrew_height = metashrew_height,
  utxo_count = #utxos,
  total_sats = total_value,
  total_btc = total_value / 100000000
}
