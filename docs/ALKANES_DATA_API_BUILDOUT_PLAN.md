# Alkanes Data API Complete Buildout Plan

## Executive Summary

This document provides a comprehensive, step-by-step plan to build out the complete alkanes-data-api using the infrastructure already in place in alkanes-contract-indexer. With our newly implemented unified trace events, we have all the raw data needed—now we need to process it and expose it via API endpoints.

---

## Current State Assessment

### What We Have ✅

**alkanes-contract-indexer**:
- ✅ Block processing pipeline
- ✅ Trace event indexing (with ReceiveIntent, ValueTransfer, ReturnContext)
- ✅ Pool tracking (creation, swaps, mints, burns)
- ✅ Transaction indexing
- ✅ Decoded protostone storage
- ✅ Postgres database with indexes

**alkanes-data-api**:
- ✅ Basic API server framework (Axum/Actix)
- ✅ Database connection pooling
- ✅ Some pool-related endpoints
- ✅ Configuration management

**Unified Traces**:
- ✅ ReceiveIntent events (incoming balances)
- ✅ ValueTransfer events (outgoing balances)
- ✅ ReturnContext with storage changes
- ✅ All events indexed in TraceEvent table

### What We Need to Build 🔨

1. **Balance Tracking System** (Phase 1)
2. **Storage Indexing System** (Phase 2)
3. **Enhanced AMM Data** (Phase 3)
4. **API Endpoints** (All phases)
5. **Testing & Documentation**

---

## Phase 1: Balance Tracking System (Week 1-2)

### Objective
Enable complete balance queries for addresses, UTXOs, and holder enumeration.

### Step 1.1: Database Schema Migration

**Location**: `crates/alkanes-contract-indexer/src/schema.rs`

**Tasks**:
```rust
// Add to schema.rs

pub const SCHEMA_BALANCE_TABLES: &str = r#"
-- Aggregate balances per address per alkane
create table if not exists "AlkaneBalance" (
  "id" uuid primary key default gen_random_uuid(),
  "address" text not null,
  "alkaneIdBlock" integer not null,
  "alkaneIdTx" bigint not null,
  "amount" text not null,
  "updatedAt" timestamptz not null default now(),
  "createdAt" timestamptz not null default now(),
  unique("address", "alkaneIdBlock", "alkaneIdTx")
);

create index if not exists "idx_AlkaneBalance_address" on "AlkaneBalance"("address");
create index if not exists "idx_AlkaneBalance_alkane" on "AlkaneBalance"("alkaneIdBlock", "alkaneIdTx");
create index if not exists "idx_AlkaneBalance_amount" on "AlkaneBalance"("alkaneIdBlock", "alkaneIdTx", "amount" desc);

-- UTXO-level balances
create table if not exists "AlkaneBalanceUtxo" (
  "id" uuid primary key default gen_random_uuid(),
  "address" text not null,
  "outpointTxid" text not null,
  "outpointVout" integer not null,
  "alkaneIdBlock" integer not null,
  "alkaneIdTx" bigint not null,
  "amount" text not null,
  "blockHeight" integer not null,
  "spent" boolean not null default false,
  "spentTxid" text,
  "spentBlockHeight" integer,
  "createdAt" timestamptz not null default now(),
  "updatedAt" timestamptz not null default now(),
  unique("outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx")
);

create index if not exists "idx_AlkaneBalanceUtxo_address" on "AlkaneBalanceUtxo"("address");
create index if not exists "idx_AlkaneBalanceUtxo_outpoint" on "AlkaneBalanceUtxo"("outpointTxid", "outpointVout");
create index if not exists "idx_AlkaneBalanceUtxo_alkane" on "AlkaneBalanceUtxo"("alkaneIdBlock", "alkaneIdTx");
create index if not exists "idx_AlkaneBalanceUtxo_block" on "AlkaneBalanceUtxo"("blockHeight");
create index if not exists "idx_AlkaneBalanceUtxo_spent" on "AlkaneBalanceUtxo"("spent") where not "spent";

-- Holder enumeration (materialized view alternative)
create table if not exists "AlkaneHolder" (
  "alkaneIdBlock" integer not null,
  "alkaneIdTx" bigint not null,
  "address" text not null,
  "totalAmount" text not null,
  "lastUpdated" timestamptz not null default now(),
  primary key("alkaneIdBlock", "alkaneIdTx", "address")
);

create index if not exists "idx_AlkaneHolder_alkane" on "AlkaneHolder"("alkaneIdBlock", "alkaneIdTx");
create index if not exists "idx_AlkaneHolder_amount" on "AlkaneHolder"("alkaneIdBlock", "alkaneIdTx", "totalAmount" desc);

-- Holder count cache
create table if not exists "AlkaneHolderCount" (
  "alkaneIdBlock" integer not null,
  "alkaneIdTx" bigint not null,
  "count" bigint not null default 0,
  "lastUpdated" timestamptz not null default now(),
  primary key("alkaneIdBlock", "alkaneIdTx")
);
"#;
```

**Files to Create/Modify**:
- `crates/alkanes-contract-indexer/src/schema.rs` - Add schema
- `crates/alkanes-contract-indexer/migrations/` - Add migration script

### Step 1.2: Balance Extractor Module

**Location**: `crates/alkanes-contract-indexer/src/helpers/balance_tracker.rs` (NEW)

**Tasks**:
```rust
// NEW FILE: src/helpers/balance_tracker.rs

use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub alkane_id_block: i32,
    pub alkane_id_tx: i64,
    pub amount: String,
}

#[derive(Debug, Clone)]
pub struct OutpointBalance {
    pub outpoint_txid: String,
    pub outpoint_vout: i32,
    pub address: String,
    pub changes: Vec<BalanceChange>,
}

/// Extract balance changes from a transaction's trace events
pub fn extract_balance_changes(
    tx: &JsonValue,
    trace_events: &[JsonValue],
) -> Result<Vec<OutpointBalance>> {
    let mut outpoint_balances: HashMap<(String, i32), OutpointBalance> = HashMap::new();
    
    // Get transaction outputs for address resolution
    let outputs = tx.get("vout")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing vout"))?;
    
    // Process each trace event
    for event in trace_events {
        let event_type = event.get("eventType")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let vout = event.get("vout")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let data = event.get("data").unwrap_or(&JsonValue::Null);
        
        match event_type {
            "value_transfer" => {
                let transfers = data.get("transfers")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&vec![]);
                let redirect_to = data.get("redirect_to")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                
                // Get address for target vout
                if let Some(address) = get_address_from_output(outputs, redirect_to) {
                    let txid = tx.get("txid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    let key = (txid.clone(), redirect_to);
                    let entry = outpoint_balances.entry(key).or_insert_with(|| {
                        OutpointBalance {
                            outpoint_txid: txid,
                            outpoint_vout: redirect_to,
                            address,
                            changes: Vec::new(),
                        }
                    });
                    
                    for transfer in transfers {
                        let alkane_id = transfer.get("id").unwrap_or(&JsonValue::Null);
                        let block = alkane_id.get("block")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0) as i32;
                        let tx_num = alkane_id.get("tx")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        let amount = transfer.get("amount")
                            .and_then(|v| v.as_str())
                            .or_else(|| transfer.get("value").and_then(|v| v.as_u64()).map(|n| n.to_string().leak() as &str))
                            .unwrap_or("0")
                            .to_string();
                        
                        entry.changes.push(BalanceChange {
                            alkane_id_block: block,
                            alkane_id_tx: tx_num,
                            amount,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    
    Ok(outpoint_balances.into_values().collect())
}

fn get_address_from_output(outputs: &[JsonValue], vout: i32) -> Option<String> {
    outputs.get(vout as usize)
        .and_then(|out| out.get("scriptPubKey"))
        .and_then(|spk| spk.get("address"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
}

/// Upsert UTXO balances into database
pub async fn upsert_utxo_balances(
    pool: &PgPool,
    block_height: i32,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    let mut tx = pool.begin().await?;
    
    for ob in outpoint_balances {
        for change in &ob.changes {
            sqlx::query(
                r#"
                insert into "AlkaneBalanceUtxo" 
                ("address", "outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx", "amount", "blockHeight")
                values ($1, $2, $3, $4, $5, $6, $7)
                on conflict ("outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx")
                do update set 
                    "amount" = excluded."amount",
                    "updatedAt" = now()
                "#
            )
            .bind(&ob.address)
            .bind(&ob.outpoint_txid)
            .bind(ob.outpoint_vout)
            .bind(change.alkane_id_block)
            .bind(change.alkane_id_tx)
            .bind(&change.amount)
            .bind(block_height)
            .execute(&mut *tx)
            .await?;
        }
    }
    
    tx.commit().await?;
    Ok(())
}

/// Update aggregate address balances
pub async fn update_address_balances(
    pool: &PgPool,
    outpoint_balances: &[OutpointBalance],
) -> Result<()> {
    // Group by address and alkane
    let mut aggregates: HashMap<(String, i32, i64), i128> = HashMap::new();
    
    for ob in outpoint_balances {
        for change in &ob.changes {
            let key = (ob.address.clone(), change.alkane_id_block, change.alkane_id_tx);
            let amount: i128 = change.amount.parse().unwrap_or(0);
            *aggregates.entry(key).or_insert(0) += amount;
        }
    }
    
    let mut tx = pool.begin().await?;
    
    for ((address, block, tx_num), amount) in aggregates {
        if amount != 0 {
            sqlx::query(
                r#"
                insert into "AlkaneBalance"
                ("address", "alkaneIdBlock", "alkaneIdTx", "amount")
                values ($1, $2, $3, $4)
                on conflict ("address", "alkaneIdBlock", "alkaneIdTx")
                do update set
                    "amount" = (
                        (coalesce("AlkaneBalance"."amount", '0')::numeric + $4::numeric)::text
                    ),
                    "updatedAt" = now()
                "#
            )
            .bind(&address)
            .bind(block)
            .bind(tx_num)
            .bind(amount.to_string())
            .execute(&mut *tx)
            .await?;
        }
    }
    
    tx.commit().await?;
    Ok(())
}

/// Refresh holder materialized data for an alkane
pub async fn refresh_holders(
    pool: &PgPool,
    alkane_id_block: i32,
    alkane_id_tx: i64,
) -> Result<()> {
    // Delete old entries
    sqlx::query(
        r#"delete from "AlkaneHolder" where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2"#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    // Insert fresh data
    sqlx::query(
        r#"
        insert into "AlkaneHolder" ("alkaneIdBlock", "alkaneIdTx", "address", "totalAmount")
        select "alkaneIdBlock", "alkaneIdTx", "address", "amount"
        from "AlkaneBalance"
        where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
          and "amount"::numeric > 0
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    // Update count
    sqlx::query(
        r#"
        insert into "AlkaneHolderCount" ("alkaneIdBlock", "alkaneIdTx", "count")
        select $1, $2, count(*)
        from "AlkaneHolder"
        where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
        on conflict ("alkaneIdBlock", "alkaneIdTx")
        do update set "count" = excluded."count", "lastUpdated" = now()
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .execute(pool)
    .await?;
    
    Ok(())
}
```

**Files to Create**:
- `crates/alkanes-contract-indexer/src/helpers/balance_tracker.rs` (NEW)
- `crates/alkanes-contract-indexer/src/helpers/mod.rs` - Add `pub mod balance_tracker;`

### Step 1.3: Integrate into Pipeline

**Location**: `crates/alkanes-contract-indexer/src/pipeline.rs`

**Tasks**:
```rust
// In pipeline.rs, after trace event indexing:

use crate::helpers::balance_tracker::{
    extract_balance_changes,
    upsert_utxo_balances,
    update_address_balances,
};

// In process_block, after replace_trace_events:

// Extract and index balances
let mut all_outpoint_balances = Vec::new();
for r in &results {
    if !r.trace_events.is_empty() {
        match extract_balance_changes(&r.transaction_json, &r.trace_events) {
            Ok(balances) => all_outpoint_balances.extend(balances),
            Err(e) => warn!(txid = %r.transaction_id, error = ?e, "balance extraction failed"),
        }
    }
}

if !all_outpoint_balances.is_empty() {
    upsert_utxo_balances(&self.pool, ctx.height as i32, &all_outpoint_balances).await?;
    update_address_balances(&self.pool, &all_outpoint_balances).await?;
}

// Refresh holder data for modified alkanes (can be done async or periodically)
// TODO: Track which alkanes were updated and refresh only those
```

**Files to Modify**:
- `crates/alkanes-contract-indexer/src/pipeline.rs`

### Step 1.4: API Endpoints (alkanes-data-api)

**Location**: `crates/alkanes-data-api/src/handlers/` (NEW)

**Tasks**:

Create `crates/alkanes-data-api/src/handlers/balance.rs`:
```rust
use axum::{extract::{Query, State}, Json};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct AddressBalancesQuery {
    address: String,
    #[serde(default)]
    include_outpoints: bool,
}

#[derive(Serialize)]
pub struct AddressBalancesResponse {
    ok: bool,
    address: String,
    balances: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outpoints: Option<Vec<OutpointInfo>>,
}

#[derive(Serialize)]
pub struct OutpointInfo {
    outpoint: String,
    entries: Vec<BalanceEntry>,
}

#[derive(Serialize)]
pub struct BalanceEntry {
    alkane: String,
    amount: String,
}

pub async fn get_address_balances(
    State(pool): State<Arc<PgPool>>,
    Query(params): Query<AddressBalancesQuery>,
) -> Json<AddressBalancesResponse> {
    // Query aggregate balances
    let balances_rows = sqlx::query!(
        r#"
        select "alkaneIdBlock", "alkaneIdTx", "amount"
        from "AlkaneBalance"
        where "address" = $1
        "#,
        params.address
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap_or_default();
    
    let mut balances = serde_json::Map::new();
    for row in balances_rows {
        balances.insert(
            format!("{}:{}", row.alkaneIdBlock, row.alkaneIdTx),
            serde_json::Value::String(row.amount),
        );
    }
    
    let outpoints = if params.include_outpoints {
        // Query UTXO balances
        let utxo_rows = sqlx::query!(
            r#"
            select "outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx", "amount"
            from "AlkaneBalanceUtxo"
            where "address" = $1 and not "spent"
            order by "outpointTxid", "outpointVout"
            "#,
            params.address
        )
        .fetch_all(pool.as_ref())
        .await
        .unwrap_or_default();
        
        let mut outpoint_map: std::collections::HashMap<String, Vec<BalanceEntry>> = std::collections::HashMap::new();
        for row in utxo_rows {
            let outpoint = format!("{}:{}", row.outpointTxid, row.outpointVout);
            outpoint_map.entry(outpoint).or_default().push(BalanceEntry {
                alkane: format!("{}:{}", row.alkaneIdBlock, row.alkaneIdTx),
                amount: row.amount,
            });
        }
        
        Some(outpoint_map.into_iter().map(|(outpoint, entries)| {
            OutpointInfo { outpoint, entries }
        }).collect())
    } else {
        None
    };
    
    Json(AddressBalancesResponse {
        ok: true,
        address: params.address,
        balances,
        outpoints,
    })
}

// Similar handlers for:
// - get_outpoint_balances
// - get_holders
// - get_holders_count
// - get_address_outpoints
```

**Files to Create**:
- `crates/alkanes-data-api/src/handlers/balance.rs` (NEW)
- `crates/alkanes-data-api/src/handlers/mod.rs` - Add module

**Update main.rs to register routes**:
```rust
// In main.rs
let app = Router::new()
    .route("/balance/address", get(handlers::balance::get_address_balances))
    .route("/balance/outpoint", get(handlers::balance::get_outpoint_balances))
    .route("/balance/holders", get(handlers::balance::get_holders))
    .route("/balance/holders/count", get(handlers::balance::get_holders_count))
    .route("/balance/address/outpoints", get(handlers::balance::get_address_outpoints))
    .with_state(pool);
```

---

## Phase 2: Storage Indexing System (Week 3)

### Objective
Enable contract storage queries via `get_keys` endpoint.

### Step 2.1: Storage Schema

**Location**: `crates/alkanes-contract-indexer/src/schema.rs`

```rust
pub const SCHEMA_STORAGE_TABLES: &str = r#"
create table if not exists "AlkaneStorage" (
  "id" uuid primary key default gen_random_uuid(),
  "alkaneIdBlock" integer not null,
  "alkaneIdTx" bigint not null,
  "key" bytea not null,
  "value" bytea not null,
  "lastTxid" text not null,
  "blockHeight" integer not null,
  "updatedAt" timestamptz not null default now(),
  "createdAt" timestamptz not null default now(),
  unique("alkaneIdBlock", "alkaneIdTx", "key")
);

create index if not exists "idx_AlkaneStorage_alkane" on "AlkaneStorage"("alkaneIdBlock", "alkaneIdTx");
create index if not exists "idx_AlkaneStorage_key" on "AlkaneStorage"("alkaneIdBlock", "alkaneIdTx", "key");
create index if not exists "idx_AlkaneStorage_block" on "AlkaneStorage"("blockHeight");
"#;
```

### Step 2.2: Storage Extractor

**Location**: `crates/alkanes-contract-indexer/src/helpers/storage_tracker.rs` (NEW)

```rust
use anyhow::Result;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use alkanes_support::proto::alkanes::ExtendedCallResponse;
use prost::Message;

pub async fn extract_and_index_storage(
    pool: &PgPool,
    txid: &str,
    block_height: i32,
    trace_events: &[JsonValue],
) -> Result<()> {
    for event in trace_events {
        let event_type = event.get("eventType").and_then(|v| v.as_str()).unwrap_or("");
        
        if event_type == "return" {
            let alkane_block = event.get("alkaneAddressBlock")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            let alkane_tx = event.get("alkaneAddressTx")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            
            if alkane_block == 0 || alkane_tx == 0 {
                continue;
            }
            
            // Extract storage from data field
            let data = event.get("data").unwrap_or(&JsonValue::Null);
            if let Some(storage_array) = data.get("storage").and_then(|v| v.as_array()) {
                for kv in storage_array {
                    if let (Some(key), Some(value)) = (
                        kv.get("key").and_then(decode_hex_or_bytes),
                        kv.get("value").and_then(decode_hex_or_bytes),
                    ) {
                        upsert_storage_entry(
                            pool,
                            alkane_block,
                            alkane_tx,
                            &key,
                            &value,
                            txid,
                            block_height,
                        ).await?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn decode_hex_or_bytes(v: &JsonValue) -> Option<Vec<u8>> {
    if let Some(s) = v.as_str() {
        if let Some(hex) = s.strip_prefix("0x") {
            return hex::decode(hex).ok();
        }
        return Some(s.as_bytes().to_vec());
    }
    None
}

async fn upsert_storage_entry(
    pool: &PgPool,
    alkane_id_block: i32,
    alkane_id_tx: i64,
    key: &[u8],
    value: &[u8],
    txid: &str,
    block_height: i32,
) -> Result<()> {
    sqlx::query(
        r#"
        insert into "AlkaneStorage"
        ("alkaneIdBlock", "alkaneIdTx", "key", "value", "lastTxid", "blockHeight")
        values ($1, $2, $3, $4, $5, $6)
        on conflict ("alkaneIdBlock", "alkaneIdTx", "key")
        do update set
            "value" = excluded."value",
            "lastTxid" = excluded."lastTxid",
            "blockHeight" = excluded."blockHeight",
            "updatedAt" = now()
        "#
    )
    .bind(alkane_id_block)
    .bind(alkane_id_tx)
    .bind(key)
    .bind(value)
    .bind(txid)
    .bind(block_height)
    .execute(pool)
    .await?;
    
    Ok(())
}
```

### Step 2.3: API Endpoint for Storage

**Location**: `crates/alkanes-data-api/src/handlers/storage.rs` (NEW)

```rust
pub async fn get_keys(
    State(pool): State<Arc<PgPool>>,
    Query(params): Query<GetKeysQuery>,
) -> Json<GetKeysResponse> {
    // Parse alkane ID
    let (block, tx) = parse_alkane_id(&params.alkane);
    
    // Query storage entries
    let rows = if let Some(keys) = params.keys {
        // Specific keys
        sqlx::query!(
            r#"select "key", "value", "lastTxid" from "AlkaneStorage"
               where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2 and "key" = any($3)"#,
            block, tx, &keys_to_bytea(&keys)
        )
        .fetch_all(pool.as_ref())
        .await
        .unwrap_or_default()
    } else {
        // All keys (paginated)
        let limit = params.limit.unwrap_or(100).min(1000);
        let offset = limit * (params.page.unwrap_or(1) - 1);
        
        sqlx::query!(
            r#"select "key", "value", "lastTxid" from "AlkaneStorage"
               where "alkaneIdBlock" = $1 and "alkaneIdTx" = $2
               order by "key"
               limit $3 offset $4"#,
            block, tx, limit as i64, offset as i64
        )
        .fetch_all(pool.as_ref())
        .await
        .unwrap_or_default()
    };
    
    // Format response
    // ... (similar to espo's get_keys)
}
```

---

## Phase 3: Enhanced AMM Data (Week 4-5)

### Objective
Complete AMM data indexing with candles, enhanced trade history, and pathfinding.

### Step 3.1: Enhance Trade Extraction

Current `PoolSwap` indexing is good but needs:
- Direct price calculation
- Side determination (buy/sell)
- Better sorting indexes

### Step 3.2: Candle Aggregation

**Location**: `crates/alkanes-contract-indexer/src/helpers/candle_aggregator.rs` (NEW)

```rust
// Aggregate PoolSwap data into time buckets
// Compute OHLCV per timeframe
// Store in PoolCandle table
```

### Step 3.3: Pathfinding Implementation

**Location**: `crates/alkanes-data-api/src/services/pathfinder.rs` (NEW)

```rust
// Build pool graph from database
// Implement Bellman-Ford for multi-hop routing
// Support exact_in, exact_out, implicit modes
// MEV/arbitrage detection
```

---

## Testing Strategy

### Unit Tests
- Balance calculation logic
- Storage extraction
- Price calculations
- Pathfinding algorithms

### Integration Tests
- Full block processing with balance updates
- API endpoint responses
- Database consistency

### Performance Tests
- 10k+ holders query performance
- 1M+ trade history queries
- Real-time candle updates

---

## Deployment Plan

### Stage 1: Development
- Local testing with regtest/signet
- Schema migrations
- API endpoint testing

### Stage 2: Staging
- Deploy to staging environment
- Index historical blocks
- Performance testing
- API documentation

### Stage 3: Production
- Deploy to mainnet
- Monitor indexing performance
- API rate limiting
- Caching strategies

---

## Timeline Summary

| Phase | Duration | Dependencies | Deliverables |
|-------|----------|--------------|--------------|
| Phase 1: Balance Tracking | 1-2 weeks | Unified traces ✅ | 5 API endpoints, UTXO tracking |
| Phase 2: Storage Indexing | 1 week | Phase 1 | 1 API endpoint, storage queries |
| Phase 3: Enhanced AMM | 1-2 weeks | Phases 1-2 | 5 API endpoints, pathfinding |
| Testing & Docs | 1 week | All phases | Test suite, API docs |
| **Total** | **4-6 weeks** | | **11 API endpoints** |

---

## Success Metrics

- ✅ All 11 espo API endpoints implemented
- ✅ <100ms response time for balance queries
- ✅ <500ms response time for complex queries
- ✅ 100% data consistency with traces
- ✅ API documentation complete
- ✅ 90%+ test coverage
- ✅ Production deployment successful

---

## Risk Mitigation

### Risk 1: Performance Issues
**Mitigation**: 
- Proper indexing strategy
- Caching frequently accessed data
- Pagination on all list endpoints
- Database query optimization

### Risk 2: Data Consistency
**Mitigation**:
- Transactional updates
- Reorg handling
- Data validation tests
- Audit tools

### Risk 3: API Complexity
**Mitigation**:
- Clear API documentation
- Example requests/responses
- Client library (optional)
- Postman collection

---

## Next Actions (Priority Order)

1. ✅ Review this plan with team
2. Create database migration scripts (Phase 1)
3. Implement balance_tracker.rs module
4. Integrate into pipeline.rs
5. Test with sample blocks
6. Implement API endpoints
7. Add comprehensive tests
8. Deploy to staging
9. Performance testing
10. Production deployment

---

## Conclusion

With the unified trace events in place, we have **all the data** needed to build a comprehensive API that matches or exceeds espo's functionality. The implementation is straightforward since we're primarily:

1. **Extracting** data from existing trace events
2. **Aggregating** into queryable tables
3. **Exposing** via REST API endpoints

No complex RocksDB integration, no specialized contract parsing—just clean extraction from the unified traces we're already indexing!

**Estimated Total Effort**: 4-6 weeks for one developer, or 2-3 weeks for a team of 2-3.
