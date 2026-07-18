# Alkane Registry & Balance Tracking Implementation

## What We Built

### 1. Database Schema (✅ Complete)

**TraceAlkane** - Registry of all created alkanes
```sql
CREATE TABLE "TraceAlkane" (
    id UUID PRIMARY KEY,
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    created_at_block INTEGER NOT NULL,
    created_at_tx TEXT NOT NULL,
    created_at_height INTEGER,
    created_at_timestamp TIMESTAMPTZ,
    UNIQUE(alkane_block, alkane_tx)
);
```

**TraceAlkaneBalance** - Address → Alkane balances
```sql
CREATE TABLE "TraceAlkaneBalance" (
    id UUID PRIMARY KEY,
    address TEXT NOT NULL,
    alkane_block INTEGER NOT NULL,
    alkane_tx BIGINT NOT NULL,
    balance NUMERIC NOT NULL DEFAULT 0,
    last_updated_block INTEGER NOT NULL,
    last_updated_tx TEXT NOT NULL,
    last_updated_timestamp TIMESTAMPTZ,
    UNIQUE(address, alkane_block, alkane_tx)
);
```

### 2. Transform Logic (✅ Complete)

**Track ALL create events:**
```rust
for trace in &traces {
    if trace.event_type == "create" {
        // Insert into TraceAlkane registry
        insert_alkane_registry(alkane_id, context).await?;
        
        // If from factory → also add to known_pools
        if factory_invoke {
            known_pools.insert(alkane_id);
        }
    }
}
```

**Track balance changes:**
```rust
for trace in &traces {
    match trace.event_type {
        "receive_intent" => {
            // Extract recipient + amount
            // Upsert into TraceAlkaneBalance
        }
        "value_transfer" => {
            // Extract to/from + amount
            // Update balances for both addresses
        }
    }
}
```

## Next Steps to Complete

### Phase 1: Verify Data Collection (Testing)

1. **Force reindex from block 0** to populate tables
   ```bash
   docker-compose down
   docker volume rm alkanes-rs_postgres-data
   docker-compose up -d
   ```

2. **Verify TraceAlkane has entries:**
   ```sql
   SELECT COUNT(*) FROM "TraceAlkane";
   SELECT * FROM "TraceAlkane" ORDER BY created_at_height LIMIT 10;
   ```

3. **Verify TraceAlkaneBalance has data:**
   ```sql
   SELECT COUNT(*) FROM "TraceAlkaneBalance";
   SELECT address, alkane_block, alkane_tx, balance 
   FROM "TraceAlkaneBalance" 
   WHERE balance > 0 
   LIMIT 10;
   ```

### Phase 2: Data API Routes (TODO)

Need to implement 4 broken routes using the new tables:

#### 1. **get-alkanes** - List all alkanes
```rust
pub async fn get_alkanes(pool: &PgPool) -> Vec<AlkaneInfo> {
    // Query TraceAlkane for all alkane IDs
    let alkanes = sqlx::query!("SELECT DISTINCT alkane_block, alkane_tx FROM TraceAlkane")
        .fetch_all(pool).await?;
    
    // For each alkane, call reflect-alkane to get metadata
    let mut results = vec![];
    for alkane in alkanes {
        if let Ok(metadata) = call_reflect_alkane(alkane_block, alkane_tx).await {
            results.push(AlkaneInfo {
                id: format!("{}:{}", alkane_block, alkane_tx),
                name: metadata.name,
                symbol: metadata.symbol,
                supply: metadata.supply,
            });
        }
    }
    
    results
}
```

#### 2. **get-alkane-details** - Single alkane details
```rust
pub async fn get_alkane_details(
    pool: &PgPool,
    alkane_id: AlkaneId
) -> AlkaneDetails {
    // 1. Call reflect-alkane for metadata
    let metadata = call_reflect_alkane(alkane_id).await?;
    
    // 2. Query TraceTrade for trading volume/price
    let trades = query_trades_for_token(pool, alkane_id).await?;
    let volume_24h = calculate_24h_volume(&trades);
    let price = calculate_current_price(&trades);
    
    // 3. Query TraceAlkaneBalance for holder count
    let holder_count = sqlx::query!(
        "SELECT COUNT(DISTINCT address) FROM TraceAlkaneBalance 
         WHERE alkane_block = $1 AND alkane_tx = $2 AND balance > 0",
        alkane_id.block, alkane_id.tx
    ).fetch_one(pool).await?;
    
    AlkaneDetails {
        id: alkane_id,
        name: metadata.name,
        symbol: metadata.symbol,
        supply: metadata.supply,
        holders: holder_count,
        volume_24h,
        price,
    }
}
```

#### 3. **get-alkanes-by-address** - Alkanes owned by address
```rust
pub async fn get_alkanes_by_address(
    pool: &PgPool,
    address: String
) -> Vec<AlkaneBalance> {
    // Query TraceAlkaneBalance for this address
    let balances = sqlx::query!(
        r#"SELECT alkane_block, alkane_tx, balance 
           FROM TraceAlkaneBalance 
           WHERE address = $1 AND balance > 0
           ORDER BY balance DESC"#,
        address
    ).fetch_all(pool).await?;
    
    // Optionally enrich with metadata from reflect-alkane
    let mut results = vec![];
    for balance in balances {
        let metadata = call_reflect_alkane(balance.alkane_block, balance.alkane_tx)
            .await.ok();
        
        results.push(AlkaneBalance {
            alkane_id: format!("{}:{}", balance.alkane_block, balance.alkane_tx),
            balance: balance.balance,
            name: metadata.as_ref().map(|m| m.name.clone()),
            symbol: metadata.as_ref().map(|m| m.symbol.clone()),
        });
    }
    
    results
}
```

#### 4. **get-pool-by-id** - Pool details
```rust
pub async fn get_pool_by_id(
    pool: &PgPool,
    pool_id: AlkaneId
) -> PoolDetails {
    // 1. Query Pool table for basic info
    let pool_info = sqlx::query!(
        r#"SELECT * FROM "Pool" 
           WHERE "poolBlockId" = $1 AND "poolTxId" = $2"#,
        pool_id.block.to_string(),
        pool_id.tx.to_string()
    ).fetch_one(pool).await?;
    
    // 2. Query TraceTrade for swap statistics
    let stats = sqlx::query!(
        r#"SELECT 
            COUNT(*) as swap_count,
            SUM(amount0_in + amount0_out) as volume0_total,
            SUM(amount1_in + amount1_out) as volume1_total,
            MAX(timestamp) as last_swap
           FROM "TraceTrade"
           WHERE pool_block = $1 AND pool_tx = $2"#,
        pool_id.block, pool_id.tx
    ).fetch_one(pool).await?;
    
    // 3. Get latest reserves from most recent trade
    let latest = sqlx::query!(
        r#"SELECT reserve0_after, reserve1_after 
           FROM "TraceTrade"
           WHERE pool_block = $1 AND pool_tx = $2
           ORDER BY timestamp DESC LIMIT 1"#,
        pool_id.block, pool_id.tx
    ).fetch_optional(pool).await?;
    
    PoolDetails {
        id: pool_id,
        token0: pool_info.token0,
        token1: pool_info.token1,
        reserve0: latest.map(|l| l.reserve0_after).unwrap_or_default(),
        reserve1: latest.map(|l| l.reserve1_after).unwrap_or_default(),
        total_swaps: stats.swap_count,
        total_volume0: stats.volume0_total,
        total_volume1: stats.volume1_total,
        last_swap_time: stats.last_swap,
    }
}
```

### Phase 3: CLI Integration (TODO)

Update CLI commands to use the new API routes with correct request formats.

### Phase 4: Testing (TODO)

Create integration tests:
```rust
#[tokio::test]
async fn test_alkane_registry() {
    let pool = setup_test_db().await;
    
    // Insert test data
    insert_test_alkane(&pool, 2, 0).await;
    insert_test_alkane(&pool, 32, 0).await;
    
    // Query
    let alkanes = get_alkanes(&pool).await.unwrap();
    assert_eq!(alkanes.len(), 2);
}

#[tokio::test]
async fn test_balance_tracking() {
    let pool = setup_test_db().await;
    
    // Simulate receive_intent events
    process_receive_intent(&pool, "addr1", 2, 0, 1000).await;
    process_receive_intent(&pool, "addr1", 2, 0, 500).await;
    
    // Check balance
    let balance = get_balance(&pool, "addr1", 2, 0).await.unwrap();
    assert_eq!(balance, 1500);
}
```

## Status Summary

✅ **Complete:**
- Database schema (TraceAlkane, TraceAlkaneBalance)
- Transform logic (track creates, track balances)
- Builds successfully
- Tables created in database

⏳ **Pending:**
- Reindex from block 0 to populate tables
- Implement 4 data API routes
- Update CLI request formats
- Add integration tests
- Deploy and verify

## Key Design Decisions

1. **Track ALL creates** - Not just factory pools, but every alkane created (2:n and 4:n)
2. **Call reflect-alkane in API** - Don't store metadata in DB, fetch live from RPC when serving
3. **Balance tracking from events** - Use receive_intent and value_transfer traces, not simulate calls
4. **Hybrid approach** - Combine traced data (fast, cached) with RPC calls (fresh metadata)

## Files Modified

1. `crates/alkanes-trace-transform/src/schema.rs`
   - Added ALKANE_REGISTRY_SCHEMA constant
   - Updated apply_schema() to create new tables

2. `crates/alkanes-contract-indexer/src/transform_integration.rs`
   - Added insert_alkane_registry() method
   - Added process_balance_change() method  
   - Track all create events
   - Track receive_intent and value_transfer events

## Next Command

To populate tables with data:
```bash
cd /data/alkanes-rs
docker-compose down
docker volume rm alkanes-rs_postgres-data
docker-compose up -d
# Wait for indexing to complete, then query tables
```
