# Trace Transform - Next Steps

## Current Status

### ✅ What Works
1. Trace events captured correctly (invoke, receive_intent, value_transfer)
2. TraceTrade table structure correct with NUMERIC types
3. Data API queries TraceTrade successfully
4. CLI displays swap history
5. SQL type casting fixed (::numeric)
6. All comprehensive tests passing

### ❌ What's Wrong
1. **Pool ID Detection**: Capturing factory 4:65522 instead of pool 2:3
2. **get-alkanes**: Returns empty (separate issue from trace transforms)
3. **No pool registry**: Not tracking which addresses are pools vs factory/router

## Root Cause Analysis

### The Swap Flow
```
User → Factory (4:65522) → Pool (2:3) → Tokens
```

### Current Detection Logic
```rust
// Finds FIRST invoke with type="call" + incomingAlkanes
// This matches Factory (4:65522) receiving the swap request
let invoke = vout_traces.iter().find(|t| {
    t.event_type == "invoke" && 
    t.data.get("type") == Some("call") &&
    has_incoming_alkanes(t)
});
```

### Why It's Wrong
The factory receives tokens first to route them, so it appears as the first invoke with incomingAlkanes. The actual pool (2:3) also has an invoke with incomingAlkanes but appears later in the trace sequence.

## Solution: Pool Registry

### Architecture
```rust
struct TraceTransformService {
    pool: PgPool,
    balance_processor: OptimizedBalanceProcessor,
    amm_tracker: OptimizedAmmTracker,
    known_pools: HashSet<AlkaneId>,  // NEW: Registry of pool addresses
    factory_id: AlkaneId,             // NEW: Factory to monitor (4:65522)
}
```

### Implementation Steps

#### 1. Query Existing Pools at Startup
```rust
impl TraceTransformService {
    pub async fn new(pool: PgPool) -> Result<Self> {
        // Query Pool table for all known pools
        let known_pools = sqlx::query!(
            r#"SELECT DISTINCT "poolBlockId"::int, "poolTxId"::bigint FROM "Pool""#
        )
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|r| AlkaneId::new(r.poolBlockId, r.poolTxId as i64))
        .collect::<HashSet<_>>();
        
        Ok(Self {
            pool: pool.clone(),
            balance_processor: OptimizedBalanceProcessor::new(pool.clone()),
            amm_tracker: OptimizedAmmTracker::new(pool),
            known_pools,
            factory_id: AlkaneId::new(4, 65522),
        })
    }
}
```

#### 2. Update Pool ID Detection
```rust
// In extract_trades_from_traces
let pool_invoke = vout_traces.iter().find(|t| {
    if t.event_type != "invoke" { return false; }
    if t.data.get("type") != Some("call") { return false; }
    
    // Check if this invoke is on a KNOWN POOL
    let addr = AlkaneId::new(
        t.alkane_address_block.parse().unwrap_or(0),
        t.alkane_address_tx.parse().unwrap_or(0)
    );
    
    known_pools.contains(&addr)
});
```

#### 3. Track New Pool Creations (Future)
```rust
// Monitor factory for create events
if trace.event_type == "create" && parent_is_factory(trace) {
    // Extract new pool ID and register it
    let pool_id = extract_pool_from_create(trace);
    known_pools.insert(pool_id);
    
    // Also insert into Pool table
    insert_pool_to_db(pool_id, tokens).await?;
}
```

### Testing Strategy

#### Unit Test with Mock Pool Registry
```rust
#[test]
fn test_pool_detection_with_registry() {
    let mut known_pools = HashSet::new();
    known_pools.insert(AlkaneId::new(2, 3)); // DIESEL/frBTC pool
    
    let traces = create_block_480_traces(); // Real data
    let trades = extract_trades(traces, &known_pools);
    
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].pool_id, AlkaneId::new(2, 3)); // Not 4:65522!
}
```

#### Integration Test
```rust
#[tokio::test]
async fn test_with_real_pool_table() {
    let pool = setup_test_db().await;
    
    // Insert test pool
    sqlx::query!(
        r#"INSERT INTO "Pool" ("poolBlockId", "poolTxId", ...) VALUES ($1, $2, ...)"#,
        "2", "3", ...
    ).execute(&pool).await?;
    
    let service = TraceTransformService::new(pool).await?;
    assert!(service.known_pools.contains(&AlkaneId::new(2, 3)));
}
```

## CLI Flag Fix

### Remove Default Pool
```rust
// In commands.rs
GetSwapHistory {
    #[arg(long)]  // NO default_value
    factory_id: Option<String>,  // Make optional
    ...
}
```

### Require Pool ID or Factory ID
```rust
// In main.rs
DataApiCommand::GetSwapHistory { factory_id, ... } => {
    let pool_id = factory_id.ok_or_else(|| 
        anyhow!("--factory-id is required")
    )?;
    ...
}
```

## get-alkanes Issue (Separate)

The AlkaneBalance table exists but returns empty. This is unrelated to trace transforms - it's a data API issue querying the wrong table or with wrong filters.

## Priority Order

1. **HIGH**: Fix pool ID detection with registry (affects trade data accuracy)
2. **MEDIUM**: Add pool creation tracking (enables dynamic pool discovery)
3. **LOW**: Fix get-alkanes (separate API issue, not blocking swaps)

## Files to Modify

1. `crates/alkanes-contract-indexer/src/transform_integration.rs`
   - Add known_pools registry
   - Update extract_trades_from_traces signature
   - Fix pool detection logic

2. `crates/alkanes-contract-indexer/tests/transform_service_integration_test.rs`
   - Add test with pool registry
   - Verify pool 2:3 is detected (not 4:65522)

3. `crates/alkanes-cli/src/commands.rs`
   - Remove default_value from factory_id
   - Make it required (no default pool)

## Success Criteria

- ✅ Trades show pool 2:3 (not 4:65522)
- ✅ Tests pass with pool registry
- ✅ CLI requires explicit --factory-id
- ✅ New pools can be registered dynamically
