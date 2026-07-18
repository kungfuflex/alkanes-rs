# Factory-Aware AMM Tracking Design

## Problem
Currently capturing factory/router address (4:65522) instead of actual pool addresses (e.g., 2:3) in trades.

## Root Cause
Looking for first `invoke` event with `type=call` and `incomingAlkanes`, which captures the factory routing the swap, not the pool executing it.

## Solution Architecture

### 1. Pool Registry
- Hardcode factory: `4:65522`
- Track pool creations from factory `create` events
- Maintain registry of: `pool_id -> (token0_id, token1_id)`

### 2. Pool Creation Detection
```
Event pattern:
- eventType: "create"
- Parent context: factory 4:65522
- Extract: new pool alkane ID + token pair from creation data
```

### 3. Swap Detection
```
For each vout with receive_intent + value_transfer:
1. Find ALL invoke events with type="call" and incomingAlkanes
2. Check if alkane address is in our pool registry
3. If match found: this is the pool executing the swap
4. Extract swap details using existing parse_trade_from_intent logic
```

## Implementation Options

### Option A: Fix transform_integration.rs (Quick Fix)
- Update pool ID detection to check against known pool registry
- Hardcode pool 2:3 for now as it's the only pool
- Query Pool table at startup to get all known pools

### Option B: Proper Factory Tracker (Complete Solution)
- Implement FactoryAmmExtractor in alkanes-trace-transform
- Track pool creations dynamically
- Use proper pipeline with tests
- More work but cleaner architecture

## Recommendation
Start with Option A to unblock, then migrate to Option B for proper architecture.

## Option A Implementation

### Step 1: Query known pools at startup
```rust
// In TraceTransformService::new()
let known_pools = query_pools_from_db(&pool).await?;
```

### Step 2: Pass known pools to extract_trades
```rust
fn extract_trades_from_traces(
    context: &TransactionContext,
    traces: &[TraceEvent],
    known_pools: &HashMap<AlkaneId, (AlkaneId, AlkaneId)>,
) -> Vec<TradeEvent>
```

### Step 3: Find pool in known_pools map
```rust
// Instead of finding first invoke
// Find invoke where alkane_address is in known_pools
let pool_invoke = vout_traces.iter().find(|t| {
    if t.event_type != "invoke" { return false; }
    if t.data.get("type") != Some("call") { return false; }
    
    let addr = AlkaneId::new(
        t.alkane_address_block.parse().unwrap_or(0),
        t.alkane_address_tx.parse().unwrap_or(0)
    );
    
    known_pools.contains_key(&addr)
});
```

This way we correctly identify pool 2:3 instead of factory 4:65522.
