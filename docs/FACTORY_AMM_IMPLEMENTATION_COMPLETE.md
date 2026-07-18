# Factory-Aware AMM Tracking - Implementation Complete

## Summary

Successfully implemented factory-aware pool tracking that:
1. Hardcodes factory ID (4:65522)
2. Tracks pool creations from factory
3. Only captures swaps on registered pools (not on factory/router)
4. Requires explicit --pool-id in CLI (no defaults)

## Architecture

### Constants
```rust
const FACTORY_BLOCK: i32 = 4;
const FACTORY_TX: i64 = 65522;
```

### TraceTransformService Structure
```rust
pub struct TraceTransformService {
    pool: PgPool,
    balance_processor: OptimizedBalanceProcessor,
    amm_tracker: OptimizedAmmTracker,
    pub known_pools: HashSet<types::AlkaneId>,  // Pool registry
}
```

## Flow

### 1. Initialization (Startup)
```
TraceTransformService::new()
 ↓
load_existing_pools()  // Query Pool table
 ↓
known_pools = {2:3, ...}  // Populate registry
```

### 2. Pool Creation Detection (Per Transaction)
```
For each trace event:
  if event_type == "create":
    if factory (4:65522) invoked in same vout:
      Extract new pool address
      known_pools.insert(new_pool)
      Log: "Discovered new pool"
```

### 3. Swap Detection (Per Transaction)
```
For each vout with receive_intent + value_transfer:
  Find invoke events where:
    - type == "call"
    - alkane_address IN known_pools  ← KEY CHANGE!
  
  If pool_invoke found:
    pool_id = invoke.alkane_address
    Extract trade details
    Write to TraceTrade table
```

## Key Changes

### Before (WRONG)
```rust
// Found FIRST invoke with incomingAlkanes
// This captured factory (4:65522) routing the swap
let invoke = vout_traces.iter().find(|t| {
    t.event_type == "invoke" &&
    t.data.get("type") == Some("call") &&
    has_incoming_alkanes(t)
});
```

### After (CORRECT)
```rust
// Find invoke on KNOWN POOL (created by factory)
let pool_invoke = vout_traces.iter().find(|t| {
    t.event_type == "invoke" &&
    t.data.get("type") == Some("call") &&
    known_pools.contains(&alkane_address)
});
```

## Files Modified

### 1. `transform_integration.rs`
- Added `known_pools: HashSet<AlkaneId>`
- Added `FACTORY_BLOCK` and `FACTORY_TX` constants
- Added `load_existing_pools()` method
- Updated `process_transaction()` to track pool creations
- Updated `extract_trades_from_traces()` to check pool registry
- Changed pool detection from "first invoke with incomingAlkanes" to "invoke on known pool"

### 2. `pipeline.rs`
- Call `load_existing_pools()` at start of each block processing

### 3. `main.rs`
- Call `load_existing_pools()` at startup
- Log how many pools were loaded

### 4. `commands.rs` (CLI)
- Removed `default_value` from `--pool-id` flag
- Made pool_id `Option<String>` (was `String`)

### 5. `main.rs` (CLI)
- Require `--pool-id` with helpful error message
- No default pool allowed

### 6. `provider.rs`
- Added `data_api_url: None` to RpcConfig initialization

## Testing Strategy

### Current State
Tests exist in `transform_service_integration_test.rs` but need updating to:
1. Pass `known_pools` registry to `extract_trades_from_traces()`
2. Pre-populate registry with pool 2:3
3. Verify pool 2:3 is detected (not 4:65522)

### Example Test Update Needed
```rust
#[test]
fn test_pool_detection_with_registry() {
    let mut known_pools = HashSet::new();
    known_pools.insert(AlkaneId::new(2, 3)); // Register DIESEL/frBTC pool
    
    let traces = create_block_480_traces(); // Real production data
    let trades = extract_trades_from_traces(&context, &traces, &known_pools);
    
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].pool_id, AlkaneId::new(2, 3)); // ✓ Correct!
    assert_ne!(trades[0].pool_id, AlkaneId::new(4, 65522)); // ✗ Factory
}
```

## Usage

### Query Specific Pool
```bash
# DIESEL/frBTC pool
alkanes-cli dataapi get-swap-history --pool-id 2:3

# Error if --pool-id not provided
alkanes-cli dataapi get-swap-history
# Error: --pool-id is required. Specify a pool address like 2:3
```

### Production Flow
```
1. Indexer starts → Loads pool 2:3 from Pool table
2. Factory creates new pool → Detected and registered
3. User swaps on pool → Pool invoke found in registry
4. Trade captured with CORRECT pool ID (not factory)
5. Data API queries work with actual pool addresses
```

## Benefits

1. **Accurate Pool Attribution**: Trades show actual pool (2:3), not factory (4:65522)
2. **Dynamic Pool Discovery**: New pools auto-registered when factory creates them
3. **Scalable**: Handles unlimited pools created by factory
4. **No Hardcoding**: Pool list maintained in database, not code
5. **Factory-Agnostic**: Could track multiple factories by expanding constants

## Next Steps

1. Update integration tests with pool registry
2. Deploy and verify block 480 shows pool 2:3 (not 4:65522)
3. Test pool creation detection with actual factory transaction
4. Consider tracking multiple factories in future

## Database Schema

### Pool Table (Already Exists)
```sql
CREATE TABLE "Pool" (
    "poolBlockId" TEXT,
    "poolTxId" TEXT,
    "token0BlockId" TEXT,
    "token0TxId" TEXT,
    "token1BlockId" TEXT,
    "token1TxId" TEXT,
    ...
);
```

### TraceTrade Table (Using Correct Pool IDs)
```sql
CREATE TABLE "TraceTrade" (
    id UUID PRIMARY KEY,
    txid TEXT,
    vout INTEGER,
    pool_block INTEGER,  -- NOW CORRECT! (2, not 4)
    pool_tx BIGINT,      -- NOW CORRECT! (3, not 65522)
    token0_block INTEGER,
    token0_tx BIGINT,
    token1_block INTEGER,
    token1_tx BIGINT,
    amount0_in NUMERIC,
    amount1_in NUMERIC,
    amount0_out NUMERIC,
    amount1_out NUMERIC,
    ...
);
```

## Success Metrics

- ✅ Factory (4:65522) hardcoded
- ✅ Pool registry loads from database
- ✅ Pool creations tracked dynamically
- ✅ Swaps only captured on registered pools
- ✅ CLI requires explicit pool ID
- ✅ Builds successfully
- ⏳ Integration tests need updating
- ⏳ Production deployment pending

## Conclusion

The trace transform system now correctly distinguishes between:
- **Factory/Router** (4:65522) - Routes swaps, creates pools
- **Pools** (2:3, etc.) - Execute actual swaps

This enables accurate tracking of which pool handled each trade, essential for per-pool analytics like volume, liquidity, and price history.
