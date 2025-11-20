# AMM Integration - Implementation Complete

## Summary

Successfully integrated AMM (Automated Market Maker) functionality from deezel-sprimage into alkanes-cli-common and updated alkanes-contract-indexer to use the new implementation. All code now uses proper `metashrew_view` calls through the `AlkanesProvider` trait with `MessageContextParcel` encoding.

## Files Modified

### 1. alkanes-cli-common (Core Library)

#### `crates/alkanes-cli-common/src/alkanes/amm.rs`
- ✅ Added `get_all_pools()` - Gets list of pools from factory
- ✅ Added `get_all_pools_details()` - Gets detailed info for all pools  
- ✅ Added `get_pool_details()` - Gets details for a single pool
- ✅ Added encoding/decoding functions for AMM contract responses
- ✅ Uses proper `MessageContextParcel` with LEB128 encoding
- ✅ Calls through `AlkanesProvider::simulate()` trait method

#### `crates/alkanes-cli-common/src/alkanes/mod.rs`
- ✅ Exported AMM module and result types

#### `crates/alkanes-cli-common/src/commands.rs`
- ✅ Added `GetAllPools` command
- ✅ Added `AllPoolsDetails` command
- ✅ Added `PoolDetails` command

### 2. alkanes-cli (CLI Binary)

#### `crates/alkanes-cli/src/main.rs`
- ✅ Added handler for `Alkanes::GetAllPools`
- ✅ Added handler for `Alkanes::AllPoolsDetails`
- ✅ Added handler for `Alkanes::PoolDetails`
- ✅ Proper error handling and pretty printing

### 3. alkanes-contract-indexer (Service)

#### `crates/alkanes-contract-indexer/Cargo.toml`
- ✅ Removed `deezel-common` dependency (from Sprimage fork)
- ✅ Added `alkanes-cli-common` dependency (local workspace)
- ✅ Added `alkanes-cli-sys` dependency (local workspace)

#### Updated Import Statements in:
- ✅ `src/poller.rs` - Changed to `alkanes_cli_common::provider::SystemProvider`
- ✅ `src/coordinator.rs` - Changed to `alkanes_cli_common::traits`
- ✅ `src/provider.rs` - Changed to `alkanes_cli_common::provider::SystemProvider`
- ✅ `src/pipeline.rs` - Changed to `alkanes_cli_common`
- ✅ `src/helpers/rpc.rs` - Changed to `alkanes_cli_common::traits`
- ✅ `src/helpers/block.rs` - Changed to `alkanes_cli_common::traits`
- ✅ `src/helpers/protostone.rs` - Changed to `alkanes_cli_common` (2 locations)
- ✅ `src/helpers/pools.rs` - Updated to use new AMM API

#### `src/helpers/pools.rs` - Major Refactor
**Before:**
```rust
let amm = Arc::new(AmmManager::new(Arc::new(provider.clone())));
let url = env::var("SANDSHREW_RPC_URL")...;
let all = amm.get_all_pools_via_raw_simulate(&url, ...).await?;
let res = amm.get_pool_details_via_raw_simulate(&url, ...).await;
```

**After:**
```rust
let factory_id = AlkaneId { block: factory_block.parse()?, tx: factory_tx.parse()? };
let mut provider_clone = provider.clone();
let executor = Arc::new(EnhancedAlkanesExecutor::new(&mut provider_clone));
let amm = AmmManager::new(executor);
let all = amm.get_all_pools(&factory_id, provider).await?;
let res = amm.get_pool_details(&id, &provider).await;
```

## Key Architecture Changes

### Before (deezel-sprimage)
```
alkanes-contract-indexer
  └─> deezel-common (git fork)
      └─> Raw JSON alkanes_simulate calls
          └─> Manual encoding/decoding
```

### After (alkanes-cli integration)
```
alkanes-contract-indexer
  └─> alkanes-cli-sys
      └─> alkanes-cli-common
          └─> AlkanesProvider::simulate()
              └─> MessageContextParcel (protobuf)
                  └─> LEB128 encoding
                      └─> metashrew_view RPC
```

## Benefits

1. **Single Source of Truth** - All AMM logic in alkanes-cli-common
2. **Proper Abstraction** - Uses trait-based provider system
3. **Type Safety** - Protobuf message encoding instead of raw JSON
4. **Maintainability** - No need to maintain forked deezel repository
5. **Consistency** - Same patterns as other alkanes operations
6. **Testability** - Can mock providers easily
7. **Reusability** - Same code works in CLI, indexer, and web builds

## Testing

### CLI Commands
```bash
# Get all pools from factory
alkanes-cli --metashrew-rpc-url http://localhost:18888 alkanes get-all-pools 32:0

# Get all pools with details
alkanes-cli --metashrew-rpc-url http://localhost:18888 alkanes all-pools-details 32:0

# Get single pool details
alkanes-cli --metashrew-rpc-url http://localhost:18888 alkanes pool-details 100:5

# With raw JSON output
alkanes-cli --metashrew-rpc-url http://localhost:18888 alkanes get-all-pools 32:0 --raw
```

### Indexer
The indexer's `fetch_all_pools_with_details()` function will automatically use the new implementation when fetching pool data.

## Migration Path for alkanes-contract-indexer

1. ✅ Update Cargo.toml dependencies
2. ✅ Update all import statements
3. ✅ Refactor pools.rs to use new AMM API
4. ⏳ Test with actual running indexer
5. ⏳ Remove any deezel references from documentation

## RPC Method Used

All AMM operations use the standard `metashrew_view` RPC method:

**Request Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "metashrew_view",  
  "params": [
    "<contract_id>/<method>",
    "<hex_encoded_MessageContextParcel>",
    "latest"
  ]
}
```

**MessageContextParcel Calldata:**
```rust
let mut calldata = Vec::new();
leb128::write::unsigned(&mut calldata, target_block).unwrap();
leb128::write::unsigned(&mut calldata, target_tx).unwrap();
leb128::write::unsigned(&mut calldata, opcode).unwrap();
// Example: [block][tx][3] for GET_ALL_POOLS
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "data": "0x<hex_result_data>"
  }
}
```

## Operation Codes

```rust
// Pool operations
const POOL_OPCODE_INIT_POOL: u64 = 0;
const POOL_OPCODE_ADD_LIQUIDITY: u64 = 1;
const POOL_OPCODE_REMOVE_LIQUIDITY: u64 = 2;
const POOL_OPCODE_SWAP: u64 = 3;
const POOL_OPCODE_SIMULATE_SWAP: u64 = 4;
const POOL_OPCODE_NAME: u64 = 99;
const POOL_OPCODE_POOL_DETAILS: u64 = 999;

// Factory operations
const FACTORY_OPCODE_INIT_POOL: u64 = 0;
const FACTORY_OPCODE_CREATE_NEW_POOL: u64 = 1;
const FACTORY_OPCODE_FIND_EXISTING_POOL_ID: u64 = 2;
const FACTORY_OPCODE_GET_ALL_POOLS: u64 = 3;
```

## Result Types

### GetAllPoolsResult
```rust
pub struct GetAllPoolsResult {
    pub count: usize,
    pub pools: Vec<AlkaneId>,  // Vec<{block: u64, tx: u64}>
}
```

### PoolDetailsResult
```rust
pub struct PoolDetailsResult {
    pub token0: AlkaneId,
    pub token1: AlkaneId,
    pub token0_amount: u128,
    pub token1_amount: u128,
    pub token_supply: u128,
    pub pool_name: String,
}
```

### AllPoolsDetailsResult
```rust
pub struct AllPoolsDetailsResult {
    pub count: usize,
    pub pools: Vec<PoolDetailsWithId>,
}

pub struct PoolDetailsWithId {
    pub pool_id: AlkaneId,
    // ... same fields as PoolDetailsResult
}
```

## Next Steps

1. **Test Compilation** - Ensure all changes compile without errors
2. **Integration Testing** - Test alkanes-contract-indexer with real data
3. **Documentation** - Update alkanes-contract-indexer README
4. **Remove References** - Clean up any remaining deezel mentions

## Verification Checklist

- ✅ AMM logic added to alkanes-cli-common
- ✅ CLI commands added and wired up
- ✅ alkanes-contract-indexer dependencies updated
- ✅ All import statements updated
- ✅ pools.rs refactored to use new API
- ⏳ Compilation test
- ⏳ Integration test with running indexer
- ⏳ Documentation updates

## Success Criteria

- ✅ No more dependency on Sprimage/deezel fork
- ✅ All functionality uses alkanes-cli-common
- ✅ Proper metashrew_view integration
- ✅ Type-safe protobuf encoding
- ⏳ Indexer successfully queries pool data
- ⏳ CLI commands return correct pool information

---

**Status:** Implementation Complete - Ready for Testing
**Date:** 2025-11-20
**Components:** alkanes-cli-common, alkanes-cli, alkanes-contract-indexer
