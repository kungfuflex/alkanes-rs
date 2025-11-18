# Alkanes Backtest Testing

## Overview

The `alkanes backtest` command allows you to simulate a transaction's execution by creating a virtual block and calling `metashrew_preview` to get the execution trace.

## Test Scripts

### 1. `test-backtest.sh` - Manual cURL Test

This script manually constructs the `metashrew_preview` call using cURL to help debug and understand the exact parameters being sent.

**Usage:**
```bash
./scripts/test-backtest.sh
```

**What it does:**
1. Fetches the transaction hex using `esplora_tx::hex`
2. Gets the current block height
3. Gets the previous block hash
4. Constructs a simulated block with:
   - Block header (version, prev hash, merkle root, timestamp, bits, nonce)
   - Coinbase transaction (minimal, valid structure)
   - The target transaction to backtest
5. Encodes the trace outpoint as hex
6. Calls `metashrew_preview` with:
   - Block hex
   - "trace" (view function name)
   - Outpoint hex
   - Block tag (height before deployment)

### 2. CLI Command

The integrated CLI command that does the same thing:

```bash
./target/release/alkanes-cli \
  --metashrew-rpc-url http://localhost:18888 \
  --sandshrew-rpc-url http://localhost:18888 \
  alkanes backtest <TXID>
```

## Expected Response

### Success Case

When working correctly, `metashrew_preview` should return:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "trace": "0x<hex_encoded_protobuf_trace>"
  },
  "id": 1
}
```

The CLI will then:
1. Decode the protobuf trace
2. Display formatted transaction analysis
3. Display formatted execution trace (similar to `alkanes trace` command)

### Error Cases

#### Preview Function Not Implemented
```json
{
  "error": {
    "code": -32000,
    "message": "View function error: Preview function failed: ..."
  }
}
```

This means:
- The metashrew indexer doesn't have the preview functionality fully implemented
- OR the transaction format is not what the preview expects
- OR there's an error in the block construction

## Debugging

### Check Block Format

The simulated block must have:
1. **Valid block header (80 bytes)**:
   - Version: 4 bytes (little-endian)
   - Previous block hash: 32 bytes (reversed/little-endian)
   - Merkle root: 32 bytes
   - Timestamp: 4 bytes (little-endian)
   - Bits/difficulty: 4 bytes
   - Nonce: 4 bytes

2. **Transaction count (varint)**: Number of transactions

3. **Transactions** in raw format:
   - Must include a valid coinbase first
   - Then the target transaction

### Check Trace Outpoint

The outpoint must be hex-encoded as a string:
- Format: `<txid>:0`
- Example: `326439356635363839303833343966643030663838633266353830316535626637626163303834626335363163376330613661636331393430666330646535373a30`

This is the hex encoding of the UTF-8 string `"2d95f568908349fd00f88c2f5801e5bf7bac084bc561c7c0a6acc1940fc0de57:0"`

### Manual Testing with cURL

```bash
# 1. Get transaction hex
TX_HEX=$(curl -s -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"esplora_tx::hex","params":["<TXID>"],"id":1}' \
  | jq -r '.result')

# 2. Get current height
HEIGHT=$(curl -s -X POST http://localhost:18888 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}' \
  | jq -r '.result')

# 3. Build block and call preview
# (see test-backtest.sh for full implementation)
```

## Block Format Details

### Coinbase Transaction Structure

The minimal coinbase used in the test:

```
02000000                       # version (2)
00                             # marker (segwit)
01                             # flag (segwit)
01                             # input count
  0000...0000                  # previous outpoint (null - 32 bytes)
  ffffffff                     # previous vout (coinbase marker)
  00                           # script sig length
  ffffffff                     # sequence
01                             # output count
  00f2052a01000000           # value (50 BTC in satoshis)
  0012                         # script length
  0000...0000                  # script pubkey (32 bytes)
00                             # witness (empty)
00000000                       # locktime
```

## Metashrew Preview API

### Method: `metashrew_preview`

**Parameters:**
1. `block_hex` (string): Full block in hex format
2. `view_function` (string): Name of the view function (e.g., "trace")
3. `view_params` (string): Hex-encoded parameters for the view function
4. `block_tag` (string): Block height to query state from (as string)

**Returns:**
- Success: `{"result": {"trace": "0x..."}}` or similar view function response
- Error: `{"error": {"code": -32000, "message": "..."}}`

## Next Steps

1. **Verify metashrew implementation**: Check if the alkanes indexer has `metashrew_preview` implemented
2. **Check preview function**: Verify the "trace" view function works in preview mode
3. **Test with reveal transaction**: Try backtesting a reveal transaction with envelope data
4. **Compare with live trace**: Compare backtest results with actual `alkanes trace` output

## Example Usage

```bash
# Test with commit transaction
./target/release/alkanes-cli \
  --metashrew-rpc-url http://localhost:18888 \
  --sandshrew-rpc-url http://localhost:18888 \
  alkanes backtest 2d95f568908349fd00f88c2f5801e5bf7bac084bc561c7c0a6acc1940fc0de57

# Test with reveal transaction (when available)
./target/release/alkanes-cli \
  --metashrew-rpc-url http://localhost:18888 \
  --sandshrew-rpc-url http://localhost:18888 \
  alkanes backtest <REVEAL_TXID>
```

## Troubleshooting

### Error: "Preview function failed: Error executing _start in preview"

This error indicates that:
1. The metashrew WASM module is trying to execute `_start`
2. Something in the preview execution environment is not properly initialized
3. The block format might need adjustment

**Possible solutions:**
- Check metashrew logs for more detailed error information
- Verify the block header is correctly formatted
- Ensure the coinbase transaction is valid
- Check if preview mode requires special initialization

### Error: "View function error: Preview function failed: Failed to execute view function"

This suggests:
- The "trace" view function doesn't exist or isn't accessible in preview mode
- The parameters format is incorrect
- The state at the specified block_tag cannot be loaded

**Possible solutions:**
- Use `metashrew_view` with "trace" to verify the function exists
- Check the block_tag is valid and within range
- Verify the outpoint hex encoding is correct
