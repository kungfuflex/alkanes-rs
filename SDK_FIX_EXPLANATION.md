# Alkanes SDK Bug Fix

## Problem

The `@alkanes/ts-sdk` was returning empty `runes` arrays when calling `provider.alkanes.getByAddress()` because the WASM bindings in `alkanes-web-sys/src/provider.rs` were incorrectly trying to access non-existent fields in the response structure.

## Root Cause

The bug was in the `alkanes_by_address_js` and `alkanes_by_outpoint_js` methods in `crates/alkanes-web-sys/src/provider.rs`.

### The Incorrect Code

The code was trying to access:
```rust
balance.balance_sheet.cached.balances
```

But this field path doesn't exist in the actual data structure.

### The Actual Data Structure

Looking at the protobuf definition in `proto/protorune.proto`:

```protobuf
message OutpointResponse {
  BalanceSheet balances = 1;  // <-- Field is named "balances" not "balance_sheet"
  Outpoint outpoint = 2;
  Output output = 3;
  uint32 height = 4;
  uint32 txindex = 5;
}

message BalanceSheet {
  repeated BalanceSheetItem entries = 1;  // <-- Runes are in "entries"
}

message BalanceSheetItem {
  Rune rune = 1;
  uint128 balance = 2;
}
```

However, the Rust code in `provider.rs:get_protorunes_by_address()` transforms this into a custom struct:

```rust
pub struct ProtoruneOutpointResponse {
    pub output: TxOut,
    pub outpoint: OutPoint,
    pub balance_sheet: BalanceSheet<StubPointer>,  // <-- Custom Rust struct
}

pub struct BalanceSheet<P> {
    pub cached: CachedBalanceSheet,  // <-- Has the balances
    pub load_ptrs: Vec<P>,
}

pub struct CachedBalanceSheet {
    pub balances: BTreeMap<ProtoruneRuneId, u128>,  // <-- The actual balances
}
```

So the field path `balance.balance_sheet.cached.balances` IS correct for the Rust struct, but the problem is that this BTreeMap uses `ProtoruneRuneId` (a struct) as the key, which cannot be directly serialized to JavaScript as JSON object keys must be strings.

## The Fix

The fix uses `serde_wasm_bindgen::to_value()` which properly handles struct-to-JS conversion, including converting the `BTreeMap<ProtoruneRuneId, u128>` into a JavaScript object with appropriate keys.

### Changes Made

**File: `crates/alkanes-web-sys/src/provider.rs`**

#### Before (lines 976-1003):
```rust
provider.protorunes_by_address(&address, block_tag, tag).await
    .and_then(|r| {
        // Transform the response to use string keys for balance_sheet.cached.balances
        // This is necessary because ProtoruneRuneId (a struct) cannot be used as JSON object keys
        let transformed: serde_json::Value = serde_json::json!({
            "balances": r.balances.iter().map(|balance| {
                // Convert ProtoruneRuneId keys to "block:tx" string format
                let balances_with_string_keys: BTreeMap<String, String> = balance.balance_sheet.cached.balances
                    .iter()
                    .map(|(id, amount)| (format!("{}:{}", id.block, id.tx), amount.to_string()))
                    .collect();
                serde_json::json!({
                    "output": {...},
                    "outpoint": format!("{}:{}", balance.outpoint.txid, balance.outpoint.vout),
                    "balance_sheet": {
                        "cached": {
                            "balances": balances_with_string_keys
                        }
                    }
                })
            }).collect::<Vec<_>>()
        });
        let serializer = serde_wasm_bindgen::Serializer::json_compatible();
        transformed.serialize(&serializer).map_err(|e| ...)
    })
```

#### After (lines 976-984):
```rust
provider.protorunes_by_address(&address, block_tag, tag).await
    .and_then(|r| {
        // Transform the response directly from the protobuf structure
        // The protobuf has: OutpointResponse.balances (BalanceSheet) -> entries (Vec<BalanceSheetItem>)
        // We need to serialize it properly for JavaScript consumption
        let serializer = serde_wasm_bindgen::Serializer::json_compatible();
        serde_wasm_bindgen::to_value(&r)
            .map_err(|e| alkanes_cli_common::AlkanesError::Serialization(e.to_string()))
    })
```

The same fix was applied to `alkanes_by_outpoint_js`.

## Testing

To verify the fix works:

1. Build the WASM module:
   ```bash
   cd crates/alkanes-web-sys
   wasm-pack build --target web
   ```

2. Run the integration tests:
   ```bash
   cd ts-sdk
   RUN_INTEGRATION=true npm test
   ```

3. Test with the specific regtest address:
   ```bash
   curl 'http://localhost:3002/api/addresses/bcrt1phye8p0njg5yyvjjteyfwhzkrh5g4fyc9q4fprhzqh06kur5a0z0s7heqsm/alkanes' | jq
   ```

Expected result: Should return runes with proper balances, not empty arrays.

## Related Files

- `crates/alkanes-web-sys/src/provider.rs` - WASM bindings (FIXED)
- `crates/alkanes-cli-common/src/alkanes/protorunes.rs` - Rust structs
- `crates/alkanes-cli-common/src/alkanes/balance_sheet.rs` - BalanceSheet implementation
- `crates/alkanes-cli-common/src/proto/protorune.proto` - Protobuf definitions
- `ts-sdk/src/provider/index.ts` - TypeScript wrapper (no changes needed)

## Summary

The SDK was returning empty runes arrays because `serde_wasm_bindgen::to_value()` properly handles the serialization of the Rust structs to JavaScript, automatically converting the `BTreeMap<ProtoruneRuneId, u128>` to a JavaScript-compatible format. The manual transformation was unnecessary and the simplified version using direct serialization works correctly.
