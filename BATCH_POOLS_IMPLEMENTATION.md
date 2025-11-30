# Batch Get-All-Pools Implementation - Complete Guide

## Overview

This implementation enables fetching all AMM pool information AND their details in a single RPC call using WASM-based batch optimization.

## What's Been Completed ✅

### 1. Infrastructure (100% Working)
- ✅ WAT compilation to WASM
- ✅ Fake Bitcoin transaction construction with envelope
- ✅ RawEnvelope usage with `to_witness(true)` for compression
- ✅ MessageContextParcel with transaction bytes
- ✅ WASM execution via `metashrew_view` simulate
- ✅ Call Response format understanding

### 2. Rust Types & Parsing
- ✅ `BatchPoolsResponse` struct with `tsify` support for WASM/TS
- ✅ `PoolWithDetails` struct
- ✅ Parser for aggregated response format
- ✅ CLI integration with `--experimental-batch-asm` flag

### 3. Response Format Specification
```
[pool_count(16 bytes u128)]
[pool0_block(16)][pool0_tx(16)][detail_length(16)][details(variable)]
[pool1_block(16)][pool1_tx(16)][detail_length(16)][details(variable)]
...
```

## Current Status 🚧

**The WASM executes successfully but only returns the pool count (142 pools detected).**

### The Issue

The WAT `get_all_pools_details_v2.wat` has a bug in parsing the context calldata. The calldata format is:

```
[factory_block(leb128)][factory_tx(leb128)][inputs_data...]
```

But our WAT is trying to read factory params from a fixed offset (80 bytes) as raw u128 values. This causes it to get incorrect values and fail to properly execute the pool fetching loop.

### What Works
1. ✅ WASM compiles and executes
2. ✅ `get_all_pools` staticcall succeeds (gets 142 pools)
3. ✅ CallResponse parsing logic is correct
4. ✅ Response aggregation structure is defined

### What Needs Fixing

The WAT needs to:
1. **Parse leb128-encoded calldata** - The factory_block and factory_tx are leb128 encoded, not raw u128
2. **Skip properly to inputs** - The actual u128 inputs we append come after the leb128 data
3. **Complete the fetch loop** - Once parsing is fixed, the loop should execute and fetch all pool details

## Files Created/Modified

### New Files
- `crates/alkanes-cli-common/src/alkanes/wat/get_all_pools_details.wat` - V1 prototype
- `crates/alkanes-cli-common/src/alkanes/wat/get_all_pools_details_v2.wat` - V2 with CallResponse parsing
- `crates/alkanes-cli-common/src/alkanes/batch_pools.rs` - Response parser with tsify support
- `STATUS.md` - Development status notes
- `BATCH_POOLS_IMPLEMENTATION.md` - This file

### Modified Files
- `crates/alkanes-cli-common/src/alkanes/wat/mod.rs` - Added GET_ALL_POOLS_DETAILS_WAT_V2
- `crates/alkanes-cli-common/src/alkanes/mod.rs` - Added batch_pools module
- `crates/alkanes-cli/src/commands.rs` - Added --experimental-batch-asm flag
- `crates/alkanes-cli/src/main.rs` - Implementation of optimized path

## Usage

```bash
# Standard sequential approach (N+1 RPC calls)
alkanes-cli --provider mainnet alkanes get-all-pools --pool-details

# Experimental batch approach (1 RPC call - when WAT is fixed)
alkanes-cli --provider mainnet alkanes get-all-pools --pool-details --experimental-batch-asm
```

## CallResponse Format (Critical Understanding)

When `__returndatacopy` is called, it returns a `CallResponse`:

```rust
pub struct CallResponse {
    pub alkanes: AlkaneTransferParcel,  // Alkane transfers
    pub data: Vec<u8>,                  // Actual return data
}
```

Serialized format:
```
[count(16)][transfer0(48)][transfer1(48)]...[data...]
```

Where each transfer is: `[block(16)][tx(16)][value(16)]` = 48 bytes

To extract data:
```
data_offset = 16 + (count * 48)
data = response[data_offset..]
```

## TypeScript SDK Integration

The `BatchPoolsResponse` struct is ready for TS/WASM with `tsify` decorators:

```typescript
// Will be automatically generated
interface BatchPoolsResponse {
  pool_count: number;
  pools: PoolWithDetails[];
}

interface PoolWithDetails {
  pool_id_block: number;
  pool_id_tx: number;
  details?: PoolDetails;
}
```

## Next Steps to Complete

### Option 1: Fix the WAT (Recommended for Production)

Update `get_all_pools_details_v2.wat`:

1. **Add leb128 decoder**
   ```wat
   (func $read_leb128 (param $ptr i32) (result i64 i32)
     ;; Returns: (value, bytes_consumed)
     ;; Parse variable-length leb128
   )
   ```

2. **Fix `$parse_context`**
   ```wat
   (func $parse_context
     ;; Read calldata structure properly
     ;; Skip to where leb128 factory params are
     ;; Parse leb128 factory_block
     ;; Parse leb128 factory_tx  
     ;; The u128 inputs follow after
   )
   ```

3. **Test with small subset first**
   - Limit to first 5 pools for testing
   - Verify response format
   - Then scale to all pools

### Option 2: Alternative Approach (Simpler)

Instead of parsing calldata in WAT, pass factory params directly in the fake transaction's witness or use a different encoding:

1. Put factory params in a known location in transaction
2. Have WAT read from that fixed location
3. Or: Create a separate "configuration" cellpack

## Performance Benefits (When Complete)

- **Before**: 143 RPC calls (1 for list + 142 for details)
- **After**: 1 RPC call (WASM does all 143 operations server-side)
- **Latency Reduction**: ~99% for remote RPCs
- **Network Traffic**: Minimal (single request/response)

## Testing the Infrastructure

The infrastructure is proven working:

```bash
$ alkanes-cli --provider mainnet alkanes get-all-pools --pool-details --experimental-batch-asm

🚀 Using experimental WASM-based batch optimization...
   Compiled WAT to WASM (1414 bytes)
   Created fake deploy transaction (904 bytes)
   Executing batch fetch in single RPC call...
✅ Batch fetch complete!
```

The WASM successfully:
- ✅ Compiles
- ✅ Gets embedded in transaction
- ✅ Executes on-chain
- ✅ Makes staticcall to factory
- ✅ Returns data (pool count: 142)

**Only the calldata parsing needs fixing to complete the implementation!**

## References

- alkanes-runtime: `/data/alkanes-rs/crates/alkanes-runtime/src/runtime.rs`
- CallResponse: `/data/alkanes-rs/crates/alkanes-support/src/response.rs`  
- RawEnvelope: `/data/alkanes-rs/crates/alkanes-support/src/envelope.rs`
- Test helpers: `/data/alkanes-rs/crates/alkanes/src/tests/helpers.rs`
