# Alkanes Backtest Testing

## Overview

This document describes the test infrastructure for the alkanes backtest functionality, which allows simulating transaction execution via `metashrew_preview`.

## Test Files

### `src/tests/alkanes_backtest_test.rs`

Three comprehensive tests demonstrating how `metashrew_preview` should work:

#### 1. `test_alkanes_backtest_with_trace`

**Purpose**: Demonstrates end-to-end preview functionality with a simulated block containing transactions.

**What it does**:
1. Creates an InMemoryRuntime with the minimal test WASM
2. Indexes genesis block and block 1
3. Creates a realistic Bitcoin transaction with inputs and outputs
4. Creates a coinbase transaction
5. Builds a simulated block with both transactions
6. Calls `execute_preview` with the block and "blocktracker" view function
7. Verifies the preview result shows 3 blocks processed
8. Confirms the original database state remains unchanged (still shows 2 blocks)

**Key Insight**: This proves the preview mechanism works - it processes a block and runs a view function without modifying the actual database.

#### 2. `test_trace_input_encoding`

**Purpose**: Demonstrates the correct format for trace view function input data.

**What it does**:
1. Takes a transaction ID and vout
2. Converts the hex txid to 32 bytes
3. Creates input_data by concatenating: `txid_bytes (32) + vout_le_bytes (4) = 36 bytes`
4. Shows that input should be binary protobuf, NOT hex-encoded string

**Key Insight**: The alkanes indexer's `trace` view function expects:
1. Height (u32) - 4 bytes little-endian
2. Protobuf-encoded Outpoint message

**Correct Format**:
```rust
// Correct implementation:
use protorune_support::proto::protorune::Outpoint;
use prost::Message;

let txid_bytes = hex::decode(txid)?;
let outpoint = Outpoint {
    txid: txid_bytes,
    vout: 0,
};
let outpoint_bytes = outpoint.encode_to_vec();

// Input data = height (4 bytes LE) + protobuf outpoint
let mut input_data = Vec::new();
input_data.extend_from_slice(&height.to_le_bytes());
input_data.extend_from_slice(&outpoint_bytes);
```

#### 3. `test_preview_with_actual_bitcoin_block`

**Purpose**: Demonstrates creating realistic Bitcoin block structures.

**What it does**:
1. Creates a proper Bitcoin block with:
   - Valid header (version, prev_hash, merkle_root, time, bits, nonce)
   - Coinbase transaction (with null outpoint)
   - Regular transaction
2. Serializes the block using consensus encoding
3. Calls preview and verifies success

**Key Insight**: Shows the exact block format that `metashrew_preview` expects.

## Running the Tests

```bash
# Run all alkanes backtest tests
cargo test --package rockshrew-mono alkanes_backtest --lib

# Run individual tests
cargo test --package rockshrew-mono test_alkanes_backtest_with_trace --lib
cargo test --package rockshrew-mono test_trace_input_encoding --lib
cargo test --package rockshrew-mono test_preview_with_actual_bitcoin_block --lib
```

## Why The CLI Backtest Command Fails

Based on our tests, the `alkanes backtest` CLI command fails because:

### Issue 1: Wrong View Function Name

**Problem**: Calling "trace" on the minimal test WASM
- The minimal WASM only exports "blocktracker"
- The actual alkanes indexer exports "trace"

**Solution**: This is expected - the CLI needs the actual alkanes indexer WASM, not the minimal test WASM.

### Issue 2: Incorrect Input Data Format

**Problem**: Hex-encoding the string "txid:vout" as UTF-8
```rust
// Current implementation (WRONG):
let trace_outpoint = format!("{}:0", txid);
let trace_outpoint_hex = hex::encode(trace_outpoint.as_bytes());
```

**Solution**: Send binary protobuf bytes
```rust
// Correct implementation:
let txid_bytes = hex::decode(txid)?;
let mut input_data = Vec::new();
input_data.extend_from_slice(&txid_bytes);  // 32 bytes
input_data.extend_from_slice(&0u32.to_le_bytes());  // 4 bytes
```

### Issue 3: "_start" Error

The error "Error executing _start in preview" suggests:
1. The WASM module's initialization is failing
2. OR the view function doesn't exist
3. OR the input format is incorrect causing a panic in the WASM

## Fixes Needed for CLI

### Fix 1: Change Input Data Format

In `/data/alkanes-rs/crates/alkanes-cli/src/main.rs`, function `backtest_transaction`:

**Current**:
```rust
let trace_outpoint = format!("{}:0", txid);
// ...
let params = serde_json::json!([
    block_hex,
    "trace",
    hex::encode(trace_outpoint.as_bytes()),  // WRONG
    block_tag_before
]);
```

**Should be**:
```rust
// Parse the txid as bytes
let txid_bytes = hex::decode(txid)?;

// Create input_data: txid (32 bytes) + vout (4 bytes LE)
let mut input_data = Vec::new();
input_data.extend_from_slice(&txid_bytes);
input_data.extend_from_slice(&0u32.to_le_bytes());

// Hex encode the binary data for JSON-RPC
let input_data_hex = hex::encode(&input_data);

let params = serde_json::json!([
    block_hex,
    "trace",
    input_data_hex,  // NOW CORRECT
    block_tag_before
]);
```

### Fix 2: Use Proper Protobuf Encoding

For the actual alkanes indexer, the trace view function likely expects a proper protobuf message. Check the alkanes proto definitions:

```protobuf
// Likely format:
message TraceRequest {
  bytes txid = 1;
  uint32 vout = 2;
}
```

Use `prost` to encode:
```rust
use alkanes_cli_common::proto::alkanes::TraceRequest;  // or similar

let request = TraceRequest {
    txid: txid_bytes,
    vout: 0,
};

let input_data = prost::Message::encode_to_vec(&request);
let input_data_hex = hex::encode(&input_data);
```

## Test Results

✅ **All tests pass**
- `test_alkanes_backtest_with_trace`: Proves preview mechanism works
- `test_trace_input_encoding`: Shows correct input format
- `test_preview_with_actual_bitcoin_block`: Validates block structure

## Next Steps

1. **Update CLI command** to use binary input_data instead of hex-encoded string
2. **Check alkanes proto definitions** for the exact TraceRequest format
3. **Test with actual alkanes indexer WASM** instead of minimal test WASM
4. **Add error handling** for better diagnostics
5. **Consider adding** a `--view-function` flag to allow testing different view functions

## Related Files

- `crates/alkanes-cli/src/main.rs` - CLI backtest command implementation
- `crates/rockshrew-mono/src/adapters.rs` - `execute_preview` implementation
- `crates/metashrew-runtime/src/lib.rs` - `preview_async` method
- `scripts/test-backtest.sh` - Manual cURL test script
- `BACKTEST_IMPLEMENTATION.md` - Full CLI implementation documentation
