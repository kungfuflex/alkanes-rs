# Espo API Implementation Plan for alkanes-data-api

## Overview
Espo provides two main modules with distinct functionality:
1. **essentials** - Core balance and key-value storage tracking
2. **ammdata** - AMM/DEX specific features (candles, trades, pathfinding)

With our new unified trace events (`ReceiveIntent` and `ValueTransfer`), we can now implement comprehensive balance tracking similar to espo's essentials module.

---

## Module 1: Essentials (Balance & KV Storage)

### Current Status in alkanes-data-api
- ✅ Basic transaction indexing
- ✅ Trace event storage  
- ✅ Pool tracking (limited)
- ❌ Per-address balance tracking
- ❌ Per-outpoint balance tracking
- ❌ Holders enumeration
- ❌ Key-value storage tracking

### API Endpoints to Implement

#### 1. `get_address_balances`
**Purpose**: Get all alkane balances for a Bitcoin address

**Request**:
```json
{
  "address": "bc1q...",
  "include_outpoints": false  // optional, default false
}
```

**Response**:
```json
{
  "ok": true,
  "address": "bc1q...",
  "balances": {
    "2:68441": "1000000",
    "2:1": "5000000"
  },
  "outpoints": [  // only if include_outpoints=true
    {
      "outpoint": "txid:vout",
      "entries": [
        {"alkane": "2:68441", "amount": "500000"}
      ]
    }
  ]
}
```

**Implementation Strategy**:
- Create `AlkaneBalance` table with columns:
  - `address` (text, indexed)
  - `alkane_id_block` (int)
  - `alkane_id_tx` (int)
  - `amount` (numeric/bigint as string)
  - `updated_at` (timestamp)
- Create `AlkaneBalanceUtxo` table for UTXO-level tracking:
  - `address` (text)
  - `outpoint_txid` (text)
  - `outpoint_vout` (int)
  - `alkane_id_block` (int)
  - `alkane_id_tx` (int)
  - `amount` (numeric/bigint as string)
- Extract from `TraceEvent` table where `eventType IN ('receive_intent', 'value_transfer')`
- Build balances by processing value transfers and computing net changes

#### 2. `get_outpoint_balances`
**Purpose**: Get alkane balances held by a specific UTXO

**Request**:
```json
{
  "outpoint": "txid:vout"
}
```

**Response**:
```json
{
  "ok": true,
  "outpoint": "txid:vout",
  "items": [{
    "outpoint": "txid:vout",
    "address": "bc1q...",  // optional, if known
    "entries": [
      {"alkane": "2:68441", "amount": "500000"},
      {"alkane": "2:1", "amount": "1000000"}
    ]
  }]
}
```

**Implementation Strategy**:
- Query `AlkaneBalanceUtxo` table by outpoint
- Join with address lookup if available
- Return all alkane balances for that UTXO

#### 3. `get_holders`
**Purpose**: Get paginated list of holders for a specific alkane

**Request**:
```json
{
  "alkane": "2:68441",
  "page": 1,
  "limit": 100
}
```

**Response**:
```json
{
  "ok": true,
  "alkane": "2:68441",
  "page": 1,
  "limit": 100,
  "total": 1523,
  "has_more": true,
  "items": [
    {"address": "bc1q...", "amount": "1000000"},
    {"address": "bc1q...", "amount": "500000"}
  ]
}
```

**Implementation Strategy**:
- Create `AlkaneHolder` table (materialized view or table):
  - `alkane_id_block` (int)
  - `alkane_id_tx` (int)
  - `address` (text)
  - `total_amount` (numeric)
  - `holder_rank` (optional, for sorting)
- Update via trigger or periodic aggregation from `AlkaneBalance`
- Support pagination with offset/limit

#### 4. `get_holders_count`
**Purpose**: Get total number of unique holders for an alkane

**Request**:
```json
{
  "alkane": "2:68441"
}
```

**Response**:
```json
{
  "ok": true,
  "count": 1523
}
```

**Implementation Strategy**:
- Simple COUNT(DISTINCT address) on `AlkaneBalance` for the alkane
- Can cache in separate table for performance

#### 5. `get_address_outpoints`
**Purpose**: Get all UTXOs with alkane balances for an address

**Request**:
```json
{
  "address": "bc1q..."
}
```

**Response**:
```json
{
  "ok": true,
  "address": "bc1q...",
  "outpoints": [
    {
      "outpoint": "txid:vout",
      "entries": [
        {"alkane": "2:68441", "amount": "500000"}
      ]
    }
  ]
}
```

**Implementation Strategy**:
- Query `AlkaneBalanceUtxo` filtered by address
- Group by outpoint
- Return all outpoints with their alkane entries

#### 6. `get_keys` (KV Storage)
**Purpose**: Get contract storage key-value pairs for an alkane

**Request**:
```json
{
  "alkane": "2:68441",
  "keys": ["0x1234...", "mykey"],  // optional, if omitted returns all
  "page": 1,
  "limit": 100,
  "try_decode_utf8": true  // optional, default true
}
```

**Response**:
```json
{
  "ok": true,
  "alkane": "2:68441",
  "page": 1,
  "limit": 100,
  "total": 50,
  "has_more": false,
  "items": {
    "mykey": {
      "key_hex": "0x6d796b6579",
      "key_str": "mykey",
      "value_hex": "0x0a00000000000000",
      "value_str": null,
      "value_u128": "10",
      "last_txid": "deadbeef..."
    }
  }
}
```

**Implementation Strategy**:
- This requires RocksDB access (not available via Postgres)
- **Option 1**: Implement a separate microservice that queries the indexer's RocksDB
- **Option 2**: Index storage changes into Postgres during indexing
- **Option 3**: Proxy to existing alkanes RPC endpoint that has RocksDB access
- **Recommendation**: Option 3 for now, Option 2 for future enhancement

---

## Module 2: AMM Data (Currently Out of Scope)

These endpoints are specific to AMM/DEX contracts and require specialized indexing:

### Endpoints (Future Work)
1. `get_candles` - OHLCV candle data for trading pairs
2. `get_trades` - Historical trade data with sorting/filtering
3. `get_pools` - Live pool reserves and metadata
4. `find_best_swap_path` - Multi-hop swap routing
5. `get_best_mev_swap` - Arbitrage cycle detection

**Note**: These require:
- Contract-specific event parsing (swap, mint, burn events)
- Time-series data aggregation
- Graph pathfinding algorithms
- Real-time reserve tracking

Currently, `alkanes-contract-indexer` has some pool tracking but would need significant enhancement to match espo's AMM features.

---

## Implementation Priority

### Phase 1: Balance Tracking (High Priority) ✅ READY
**Prerequisites**: ✅ Unified trace with ReceiveIntent/ValueTransfer events

**Tasks**:
1. Create database schema for balance tables
2. Implement balance extraction from trace events
3. Build balance indexer that processes each block
4. Add API endpoints for address/outpoint/holder queries
5. Add comprehensive tests

**Estimated Effort**: 2-3 days

### Phase 2: KV Storage Tracking (Medium Priority) ✅ DATA AVAILABLE
**Prerequisites**: ✅ Storage changes ARE included in trace events!

**Discovery**: The `ReturnContext` trace event includes an `ExtendedCallResponse` which has a `storage: StorageMap` field containing all key-value changes made during contract execution. This data is ALREADY being indexed in the `TraceEvent` table!

**Tasks**:
1. Extract storage changes from TraceEvent.data (ReturnContext events)
2. Create AlkaneStorage table with schema:
   - `alkane_id_block`, `alkane_id_tx` (contract identifier)
   - `key` (bytea, the storage key)
   - `value` (bytea, the storage value)
   - `last_txid` (text, transaction that last modified this key)
   - `block_height` (int, for historical queries)
3. Build storage indexer that parses ReturnContext events
4. Implement get_keys endpoint with pagination
5. Add UTF-8 decoding option for human-readable keys

**Estimated Effort**: 1-2 days (implementation only, data already available!)

### Phase 3: AMM Data Module (Medium Priority) ✅ PARTIAL DATA AVAILABLE
**Prerequisites**: ✅ Many events already indexed, need aggregation logic

**Discovery**: With unified traces, we have access to:
- **ReceiveIntent** - Shows incoming alkanes to each swap
- **ValueTransfer** - Shows outgoing alkanes (swap results)
- **ReturnContext.data** - Contains swap execution details
- **Existing pool tracking** - Already indexing pool creation/updates

**What We Have**:
- ✅ Trade execution data (in/out amounts per swap)
- ✅ Pool state changes
- ✅ Transaction timestamps
- ✅ Swap event identification

**What We Need to Build**:
1. **Trade History Indexing**:
   - Parse swap events from traces
   - Extract token_in, token_out, amount_in, amount_out
   - Store in PoolSwap table with proper indexing
   - Add sorting by timestamp, amount, side

2. **OHLCV Candle Aggregation**:
   - Calculate price from amount_in/amount_out ratios
   - Aggregate into time buckets (10m, 1h, 1d, 1w, 1M)
   - Compute open/high/low/close/volume per timeframe
   - Store in PoolCandle table

3. **Live Reserve Tracking**:
   - Extract reserves from ValueTransfer events
   - Update pool reserves after each swap
   - Maintain current state in PoolSnapshot table

4. **Pathfinding & Routing**:
   - Build graph of pool connections
   - Implement Bellman-Ford for multi-hop routing
   - Support exact_in, exact_out, and implicit modes
   - Add MEV/arbitrage detection

**Tasks**:
1. Enhance trade extraction from traces
2. Implement OHLCV aggregation pipeline
3. Build reserve state machine
4. Add pathfinding algorithm
5. Implement all AMM RPC endpoints
6. Add comprehensive tests

**Estimated Effort**: 1-2 weeks

**Note**: Unlike espo which requires specialized contract parsing, we can extract ALL this data from the unified trace events we're already indexing!

---

## Database Schema Design

### AlkaneBalance
```sql
CREATE TABLE "AlkaneBalance" (
  "id" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  "address" text NOT NULL,
  "alkaneIdBlock" integer NOT NULL,
  "alkaneIdTx" bigint NOT NULL,
  "amount" text NOT NULL,  -- Store as string to avoid overflow
  "updatedAt" timestamptz NOT NULL DEFAULT now(),
  "createdAt" timestamptz NOT NULL DEFAULT now(),
  UNIQUE("address", "alkaneIdBlock", "alkaneIdTx")
);

CREATE INDEX "idx_AlkaneBalance_address" ON "AlkaneBalance"("address");
CREATE INDEX "idx_AlkaneBalance_alkane" ON "AlkaneBalance"("alkaneIdBlock", "alkaneIdTx");
CREATE INDEX "idx_AlkaneBalance_amount" ON "AlkaneBalance"("alkaneIdBlock", "alkaneIdTx", "amount" DESC);
```

### AlkaneBalanceUtxo
```sql
CREATE TABLE "AlkaneBalanceUtxo" (
  "id" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  "address" text NOT NULL,
  "outpointTxid" text NOT NULL,
  "outpointVout" integer NOT NULL,
  "alkaneIdBlock" integer NOT NULL,
  "alkaneIdTx" bigint NOT NULL,
  "amount" text NOT NULL,
  "blockHeight" integer NOT NULL,
  "createdAt" timestamptz NOT NULL DEFAULT now(),
  "updatedAt" timestamptz NOT NULL DEFAULT now(),
  UNIQUE("outpointTxid", "outpointVout", "alkaneIdBlock", "alkaneIdTx")
);

CREATE INDEX "idx_AlkaneBalanceUtxo_address" ON "AlkaneBalanceUtxo"("address");
CREATE INDEX "idx_AlkaneBalanceUtxo_outpoint" ON "AlkaneBalanceUtxo"("outpointTxid", "outpointVout");
CREATE INDEX "idx_AlkaneBalanceUtxo_alkane" ON "AlkaneBalanceUtxo"("alkaneIdBlock", "alkaneIdTx");
CREATE INDEX "idx_AlkaneBalanceUtxo_block" ON "AlkaneBalanceUtxo"("blockHeight");
```

### AlkaneHolder (Materialized View)
```sql
CREATE TABLE "AlkaneHolder" (
  "alkaneIdBlock" integer NOT NULL,
  "alkaneIdTx" bigint NOT NULL,
  "address" text NOT NULL,
  "totalAmount" text NOT NULL,
  "lastUpdated" timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY("alkaneIdBlock", "alkaneIdTx", "address")
);

CREATE INDEX "idx_AlkaneHolder_alkane" ON "AlkaneHolder"("alkaneIdBlock", "alkaneIdTx");
CREATE INDEX "idx_AlkaneHolder_amount" ON "AlkaneHolder"("alkaneIdBlock", "alkaneIdTx", "totalAmount" DESC);
```

### AlkaneStorage (KV Storage)
```sql
CREATE TABLE "AlkaneStorage" (
  "id" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  "alkaneIdBlock" integer NOT NULL,
  "alkaneIdTx" bigint NOT NULL,
  "key" bytea NOT NULL,
  "value" bytea NOT NULL,
  "lastTxid" text NOT NULL,
  "blockHeight" integer NOT NULL,
  "updatedAt" timestamptz NOT NULL DEFAULT now(),
  "createdAt" timestamptz NOT NULL DEFAULT now(),
  UNIQUE("alkaneIdBlock", "alkaneIdTx", "key")
);

CREATE INDEX "idx_AlkaneStorage_alkane" ON "AlkaneStorage"("alkaneIdBlock", "alkaneIdTx");
CREATE INDEX "idx_AlkaneStorage_key" ON "AlkaneStorage"("alkaneIdBlock", "alkaneIdTx", "key");
CREATE INDEX "idx_AlkaneStorage_block" ON "AlkaneStorage"("blockHeight");
```

### PoolCandle (OHLCV Data)
```sql
CREATE TABLE "PoolCandle" (
  "id" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  "poolIdBlock" integer NOT NULL,
  "poolIdTx" bigint NOT NULL,
  "timeframe" text NOT NULL,  -- '10m', '1h', '1d', '1w', '1M'
  "side" text NOT NULL,        -- 'base' or 'quote'
  "timestamp" bigint NOT NULL,
  "open" text NOT NULL,
  "high" text NOT NULL,
  "low" text NOT NULL,
  "close" text NOT NULL,
  "volume" text NOT NULL,
  "createdAt" timestamptz NOT NULL DEFAULT now(),
  UNIQUE("poolIdBlock", "poolIdTx", "timeframe", "side", "timestamp")
);

CREATE INDEX "idx_PoolCandle_pool" ON "PoolCandle"("poolIdBlock", "poolIdTx", "timeframe", "side");
CREATE INDEX "idx_PoolCandle_time" ON "PoolCandle"("poolIdBlock", "poolIdTx", "timeframe", "timestamp" DESC);
```

---

## Balance Extraction Logic

### Processing ReceiveIntent Events
```rust
// When processing a protostone with ReceiveIntent event
for trace_event in trace.events {
    if event.event_type == "receive_intent" {
        let data = event.data;
        let incoming = data["incoming_alkanes"];  // Array of transfers
        let vout = event.vout;
        
        // These are incoming balances to the protostone
        // They represent what was available BEFORE the message executed
        for transfer in incoming {
            let alkane_id = transfer["id"];
            let amount = transfer["amount"];
            // Track as initial state
        }
    }
}
```

### Processing ValueTransfer Events
```rust
// When processing ValueTransfer events
if event.event_type == "value_transfer" {
    let data = event.data;
    let transfers = data["transfers"];  // Array of outgoing transfers
    let redirect_to = data["redirect_to"];  // Target vout
    
    // Get the Bitcoin address for this vout
    let address = get_address_for_vout(tx, redirect_to);
    
    for transfer in transfers {
        let alkane_id = transfer["id"];
        let amount = transfer["amount"];
        
        // Update UTXO balance
        upsert_balance_utxo(address, outpoint, alkane_id, amount);
        
        // Update aggregate address balance
        update_address_balance(address, alkane_id, amount);
    }
}
```

### Balance Reconciliation
- Track both incoming (ReceiveIntent) and outgoing (ValueTransfer) for each protostone
- Net change = outgoing - incoming (per alkane per vout)
- Maintain running totals per address
- Support both UTXO-level and aggregate balances

---

## Testing Strategy

1. **Unit Tests**:
   - Balance calculation from trace events
   - UTXO tracking logic
   - Address aggregation

2. **Integration Tests**:
   - Full block processing with multiple protostones
   - Complex multi-hop transfers
   - Edge cases (zero balances, large amounts)

3. **API Tests**:
   - All endpoint responses
   - Pagination
   - Error handling

4. **Performance Tests**:
   - Large holder lists (10k+ holders)
   - Address with many UTXOs (100+)
   - High-frequency balance updates

---

## Next Steps

1. ✅ Review this plan
2. Create database schema migrations
3. Implement balance extraction helper
4. Integrate into pipeline
5. Build API endpoints
6. Add comprehensive tests
7. Deploy and monitor
