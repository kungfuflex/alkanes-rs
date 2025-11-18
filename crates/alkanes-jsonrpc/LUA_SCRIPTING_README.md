# Lua Scripting for Sandshrew JSONRPC

## Overview

The alkanes-jsonrpc server now supports executing Lua scripts with full access to all RPC methods through three new JSONRPC methods:

1. **`sandshrew_evalscript`** - Execute Lua scripts directly
2. **`sandshrew_savescript`** - Save a script and get its SHA256 hash
3. **`sandshrew_evalsaved`** - Execute a previously saved script by hash

## Features

- **Full RPC Access**: Lua scripts can call any RPC method through the `_RPC` global table
- **Async Support**: RPC calls are handled asynchronously under the hood
- **Metrics**: Track number of RPC calls made and execution time
- **In-Memory Storage**: Scripts can be saved and reused
- **Error Handling**: Comprehensive error reporting with codes and messages

## Available RPC Methods in Lua

All RPC methods are available through a **single flat `_RPC` global table**.  
Methods follow the pattern: `namespace_methodname` (e.g., `esplora_addresstxs`, `ord_blockcount`)

### Esplora Methods
```lua
_RPC.esplora_addressutxo(address)
_RPC.esplora_addresstxs(address)
_RPC.esplora_addresstxschain(address)
_RPC.esplora_addresstxsmempool(address)
_RPC.esplora_address(address)
_RPC.esplora_tx(txid)
_RPC.esplora_txstatus(txid)
_RPC.esplora_txhex(txid)
_RPC.esplora_txraw(txid)
_RPC.esplora_txoutspends(txid)
_RPC.esplora_block(hash)
_RPC.esplora_blockstatus(hash)
_RPC.esplora_blocktxs(hash)
_RPC.esplora_blocktxids(hash)
_RPC.esplora_blockheight(height)
_RPC.esplora_mempool()
_RPC.esplora_mempooltxids()
_RPC.esplora_mempoolrecent()
_RPC.esplora_feeestimates()
```

### Ord Methods
```lua
_RPC.ord_content(inscription_id)
_RPC.ord_blockheight()
_RPC.ord_blockcount()
_RPC.ord_blockhash()
_RPC.ord_blocktime()
_RPC.ord_blocks()
_RPC.ord_outputs(address)
_RPC.ord_inscription(id)
_RPC.ord_inscriptions()
_RPC.ord_block(height_or_hash)
_RPC.ord_output(outpoint)
_RPC.ord_rune(rune)
_RPC.ord_runes()
_RPC.ord_sat(sat)
_RPC.ord_children(id)
_RPC.ord_decode(txid)
```

### Bitcoin Core RPC Methods
```lua
_RPC.btc_getblockcount()
_RPC.btc_getblockhash(height)
_RPC.btc_getblock(hash, verbosity)
_RPC.btc_getblockheader(hash, verbose)
_RPC.btc_getbestblockhash()
_RPC.btc_getblockchaininfo()
_RPC.btc_getrawtransaction(txid, verbose, blockhash)
_RPC.btc_sendrawtransaction(hex, maxfeerate)
_RPC.btc_getmempoolinfo()
_RPC.btc_getrawmempool(verbose)
_RPC.btc_getmempoolentry(txid)
_RPC.btc_getnetworkinfo()
_RPC.btc_gettxout(txid, n, include_mempool)
_RPC.btc_decoderawtransaction(hex, iswitness)
```

### Alkanes Methods
```lua
_RPC.alkanes_getbytecode({block = "2", tx = "0"}, block_tag)
_RPC.alkanes_protorunesbyaddress({address = "bc1...", protocolTag = "1"}, block_tag)
```

### Metashrew Methods
```lua
_RPC.metashrew_view(method, input, block_tag)
_RPC.metashrew_height()
```

### Sandshrew Methods
```lua
_RPC.sandshrew_multicall(calls)
_RPC.sandshrew_balances(request)
```

## Usage Examples

### 1. sandshrew_evalscript

Execute a Lua script directly with optional arguments:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "sandshrew_evalscript",
  "params": [
    "local height = _RPC.btc_getblockcount()\nreturn {height = height, doubled = height * 2}",
    "arg1",
    "arg2"
  ],
  "id": 1
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "calls": 1,
    "returns": {
      "height": 850000,
      "doubled": 1700000
    },
    "error": null,
    "runtime": 45
  },
  "id": 1
}
```

### 2. sandshrew_savescript

Save a script for later reuse:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "sandshrew_savescript",
  "params": [
    "local addr = args[1]\nlocal utxos = _RPC.esplora_addressutxo(addr)\nreturn #utxos"
  ],
  "id": 2
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "hash": "a1b2c3d4e5f6..."
  },
  "id": 2
}
```

### 3. sandshrew_evalsaved

Execute a saved script by its hash:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "sandshrew_evalsaved",
  "params": [
    "a1b2c3d4e5f6...",
    "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
  ],
  "id": 3
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "calls": 1,
    "returns": 5,
    "error": null,
    "runtime": 32
  },
  "id": 3
}
```

## Response Format

All three methods return a response with the following structure:

```json
{
  "calls": <number>,      // Number of RPC calls made during execution
  "returns": <any>,       // Return value from the Lua script (converted to JSON)
  "error": <object|null>, // Error information if execution failed
  "runtime": <number>     // Execution time in milliseconds
}
```

### Error Format

When an error occurs:

```json
{
  "calls": 2,
  "returns": null,
  "error": {
    "code": -1,
    "message": "RPC error -32601: Method not found"
  },
  "runtime": 15
}
```

## Accessing Arguments

Scripts receive arguments through the global `args` table (1-indexed):

```lua
local first_arg = args[1]
local second_arg = args[2]

-- Iterate over all args
for i, arg in ipairs(args) do
  print(i, arg)
end
```

## Complex Example

Get address balance and UTXOs, then calculate total value:

```lua
local address = args[1]

-- Get UTXOs for address
local utxos = _RPC.esplora_addressutxo(address)

-- Calculate total balance
local total = 0
local count = 0

for i, utxo in ipairs(utxos) do
  total = total + utxo.value
  count = count + 1
end

-- Get current block height
local height = _RPC.btc_getblockcount()

return {
  address = address,
  utxo_count = count,
  total_sats = total,
  total_btc = total / 100000000,
  current_height = height
}
```

## Performance Considerations

1. **RPC Call Tracking**: Each RPC call increments the `calls` counter
2. **Runtime Metrics**: The `runtime` field shows execution time in milliseconds
3. **Script Storage**: Saved scripts are stored in-memory (not persisted across restarts)
4. **Async Execution**: RPC calls block the Lua script but don't block other requests

## Technical Details

- **Lua Version**: 5.4 (vendored, no external dependencies required)
- **Async Handling**: Uses tokio runtime to handle async RPC calls within sync Lua context
- **JSON Conversion**: Automatic bidirectional conversion between Lua tables and JSON objects/arrays
- **Script Hashing**: SHA256 hash of script content

## Limitations

1. Scripts cannot access the file system or network directly (only through RPC)
2. Saved scripts are in-memory only (lost on server restart)
3. No standard Lua libraries loaded (only basic functionality + RPC)
4. Execution timeouts are not currently implemented (TODO)

## Future Enhancements

- [ ] Add execution timeouts
- [ ] Persist saved scripts to disk/database
- [ ] Add more Lua standard libraries (math, string, table)
- [ ] Add script execution sandboxing/limits
- [ ] Support for streaming results
- [ ] Script versioning and management API
