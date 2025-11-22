# Trace Events: Complete Data Extraction Guide

## Overview

With the newly implemented **unified trace system**, ALL data needed for comprehensive blockchain indexing is now available directly from trace events. This document explains what data is available and how to extract it.

---

## Trace Event Types

### 1. ReceiveIntent
**When**: Emitted at the START of protostone processing  
**Purpose**: Shows incoming alkane balances available to the protostone

**Data Structure**:
```rust
TraceEvent::ReceiveIntent {
    incoming_alkanes: AlkaneTransferParcel([
        AlkaneTransfer {
            id: AlkaneId { block: 2, tx: 68441 },
            value: 1000000
        }
    ])
}
```

**Use Cases**:
- Balance tracking (initial state)
- Transaction flow analysis
- Contract input validation

### 2. ValueTransfer
**When**: Emitted AFTER edict processing  
**Purpose**: Shows outgoing alkane transfers to specific vouts

**Data Structure**:
```rust
TraceEvent::ValueTransfer {
    transfers: vec![
        AlkaneTransfer {
            id: AlkaneId { block: 2, tx: 68441 },
            value: 500000
        }
    ],
    redirect_to: 0  // Target vout
}
```

**Use Cases**:
- Balance tracking (final state)
- UTXO indexing
- Payment tracking
- Swap output tracking

### 3. EnterCall / EnterDelegatecall / EnterStaticcall
**When**: Emitted when entering a contract call  
**Purpose**: Shows call context, inputs, and initial state

**Data Structure**:
```rust
TraceEvent::EnterCall(TraceContext {
    inner: Context {
        myself: AlkaneId { block: 2, tx: 1 },
        caller: AlkaneId { block: 0, tx: 0 },
        vout: 3,
        incoming_alkanes: AlkaneTransferParcel([...]),
        inputs: vec![30, 2, 1, 100, 0, ...]
    },
    target: AlkaneId { block: 2, tx: 1 },
    fuel: 3500000
})
```

**Use Cases**:
- Call graph construction
- Gas tracking
- Contract interaction analysis
- Cross-contract calls

### 4. ReturnContext
**When**: Emitted when a contract call succeeds  
**Purpose**: Shows return data, storage changes, and outgoing alkanes

**Data Structure**:
```rust
TraceEvent::ReturnContext(TraceResponse {
    inner: ExtendedCallResponse {
        alkanes: AlkaneTransferParcel([...]),
        storage: StorageMap(BTreeMap {
            b"key1" => b"value1",
            b"counter" => b"\x0a\x00\x00\x00",  // u32::10
        }),
        data: vec![...]  // Return data
    },
    fuel_used: 125000
})
```

**Use Cases**:
- **KV Storage tracking** (storage field)
- Contract state changes
- Return value analysis
- Gas consumption tracking

### 5. RevertContext
**When**: Emitted when a contract call fails  
**Purpose**: Shows error state and partial results

**Data Structure**:
```rust
TraceEvent::RevertContext(TraceResponse {
    inner: ExtendedCallResponse {
        alkanes: AlkaneTransferParcel([]),
        storage: StorageMap::default(),
        data: vec![...]  // Error data
    },
    fuel_used: u64::MAX  // Indicates failure
})
```

**Use Cases**:
- Error tracking
- Failed transaction analysis
- Debug information

### 6. CreateAlkane
**When**: Emitted when a new alkane/contract is deployed  
**Purpose**: Records contract creation

**Data Structure**:
```rust
TraceEvent::CreateAlkane(AlkaneId {
    block: 2,
    tx: 12345
})
```

**Use Cases**:
- Contract registry
- Deployment tracking

---

## Data Extraction Strategies

### Balance Tracking

**Source**: `ReceiveIntent` + `ValueTransfer` events

**Algorithm**:
```rust
for event in trace.events {
    match event {
        ReceiveIntent { incoming_alkanes } => {
            // Initial balances available to protostone
            for transfer in incoming_alkanes.0 {
                track_incoming(transfer.id, transfer.value, vout);
            }
        }
        ValueTransfer { transfers, redirect_to } => {
            // Final balances sent to specific vout
            let address = get_address_for_vout(tx, redirect_to);
            for transfer in transfers {
                update_balance(address, transfer.id, transfer.value);
                create_utxo_balance(address, outpoint, transfer.id, transfer.value);
            }
        }
    }
}
```

**Output Tables**:
- `AlkaneBalance` - Aggregate per address
- `AlkaneBalanceUtxo` - Per UTXO tracking
- `AlkaneHolder` - Unique holder enumeration

### KV Storage Tracking

**Source**: `ReturnContext.inner.storage` field

**Algorithm**:
```rust
for event in trace.events {
    if let ReturnContext(response) = event {
        let contract_id = extract_contract_from_context(event);
        for (key, value) in response.inner.storage.0 {
            upsert_storage(
                contract_id,
                key,
                value,
                tx.txid,
                block_height
            );
        }
    }
}
```

**Output Tables**:
- `AlkaneStorage` - Current key-value pairs per contract

**Key Features**:
- Track last modifying transaction
- Support historical queries by block height
- Enable UTF-8 decoding for human-readable keys

### AMM/DEX Trade Tracking

**Source**: `ReceiveIntent` + `ValueTransfer` + context from `EnterCall`

**Algorithm**:
```rust
// Identify swap by looking for pool contract calls
for i in 0..trace.events.len() {
    if let EnterCall(ctx) = &trace.events[i] {
        if is_pool_contract(ctx.target) {
            // Find corresponding ReceiveIntent (before call)
            let receive = find_receive_intent_before(trace, i);
            // Find corresponding ValueTransfer (after call)
            let transfer = find_value_transfer_after(trace, i);
            
            if let (Some(recv), Some(xfer)) = (receive, transfer) {
                let swap = SwapEvent {
                    pool_id: ctx.target,
                    token_in: recv.incoming_alkanes[0].id,
                    amount_in: recv.incoming_alkanes[0].value,
                    token_out: xfer.transfers[0].id,
                    amount_out: xfer.transfers[0].value,
                    timestamp: block.time,
                    side: determine_side(token_in, token_out, pool),
                };
                index_swap(swap);
            }
        }
    }
}
```

**Output Tables**:
- `PoolSwap` - Individual trade records
- `PoolCandle` - OHLCV aggregated data
- `Pool` - Reserve state updates

**Derived Data**:
- Price = amount_out / amount_in (or inverse)
- Volume aggregation by timeframe
- Liquidity changes from reserve updates

### Gas/Fuel Tracking

**Source**: `EnterCall.fuel` + `ReturnContext.fuel_used`

**Algorithm**:
```rust
let mut gas_tracker = GasTracker::new();
for event in trace.events {
    match event {
        EnterCall(ctx) => {
            gas_tracker.start_call(ctx.target, ctx.fuel);
        }
        ReturnContext(resp) => {
            gas_tracker.finish_call(resp.fuel_used);
        }
        RevertContext(resp) => {
            gas_tracker.revert_call(resp.fuel_used);
        }
    }
}
```

**Use Cases**:
- Gas optimization analysis
- Contract efficiency metrics
- Fee estimation

### Call Graph Construction

**Source**: `EnterCall` + `EnterDelegatecall` + `EnterStaticcall`

**Algorithm**:
```rust
let mut call_stack = Vec::new();
for event in trace.events {
    match event {
        EnterCall(ctx) | EnterDelegatecall(ctx) | EnterStaticcall(ctx) => {
            let parent = call_stack.last();
            call_stack.push(CallNode {
                contract: ctx.target,
                caller: ctx.inner.caller,
                depth: call_stack.len(),
                parent: parent.map(|p| p.contract),
            });
        }
        ReturnContext(_) | RevertContext(_) => {
            call_stack.pop();
        }
    }
}
```

**Use Cases**:
- Contract interaction visualization
- Dependency analysis
- Security auditing

---

## Protobuf Format

All trace events are serialized using Protocol Buffers for efficient storage:

```protobuf
message AlkanesTraceEvent {
  oneof event {
    AlkanesReceiveIntent receive_intent = 4;
    AlkanesValueTransfer value_transfer = 5;
    AlkanesEnterContext enter_context = 1;
    AlkanesExitContext exit_context = 2;
    AlkanesCreate create_alkane = 3;
  }
}

message AlkanesReceiveIntent {
  repeated AlkaneTransfer incoming_alkanes = 1;
}

message AlkanesValueTransfer {
  repeated AlkaneTransfer transfers = 1;
  uint32 redirect_to = 2;
}

message ExtendedCallResponse {
  repeated AlkaneTransfer alkanes = 1;
  repeated KeyValuePair storage = 2;  // <-- Storage changes!
  bytes data = 3;
}

message KeyValuePair {
  bytes key = 1;
  bytes value = 2;
}
```

---

## Database Access

Traces are stored in the `TraceEvent` table:

```sql
SELECT 
  "transactionId",
  "vout",
  "eventType",
  "data",
  "alkaneAddressBlock",
  "alkaneAddressTx"
FROM "TraceEvent"
WHERE "transactionId" = $1
ORDER BY "vout" ASC;
```

**Event Types**:
- `receive_intent` - ReceiveIntent events
- `value_transfer` - ValueTransfer events
- `invoke` - EnterCall events (has contract address)
- `return` - ReturnContext events (has storage!)
- `revert` - RevertContext events

---

## Key Insights

### 1. Storage is Already Indexed
The `storage` field in `ExtendedCallResponse` contains ALL key-value changes from contract execution. This data is preserved in the protobuf encoding and stored in the database.

### 2. Complete Balance History
The combination of `ReceiveIntent` and `ValueTransfer` provides complete tracking of alkane movements without needing RocksDB access.

### 3. AMM Data from Traces
Unlike espo which requires specialized indexing, we can extract ALL AMM data (swaps, reserves, prices) directly from trace events.

### 4. One Source of Truth
Traces contain:
- ✅ Balance transfers
- ✅ Storage changes
- ✅ Call context
- ✅ Gas usage
- ✅ Return values
- ✅ Error states

### 5. Historical Queries
Because traces are indexed by block height, we can:
- Query balance at any point in time
- Track storage evolution
- Analyze historical prices
- Replay transaction execution

---

## Performance Considerations

### Indexing Strategy
1. **Real-time indexing** during block processing
2. **Batch updates** for aggregate tables (balances, holders)
3. **Materialized views** for expensive queries (OHLCV candles)
4. **Proper indexes** on frequently queried fields

### Query Optimization
```sql
-- Good: Use covering index
SELECT "alkaneIdBlock", "alkaneIdTx", "amount"
FROM "AlkaneBalance"
WHERE "address" = $1;

-- Good: Paginated with index
SELECT * FROM "AlkaneHolder"
WHERE "alkaneIdBlock" = $1 AND "alkaneIdTx" = $2
ORDER BY "totalAmount" DESC
LIMIT 100 OFFSET 0;

-- Avoid: Full table scan
SELECT * FROM "TraceEvent"
WHERE "data"::text LIKE '%swap%';  -- BAD!
```

### Storage Efficiency
- Store amounts as `text` to avoid numeric overflow
- Use `bytea` for binary data (keys, values)
- Set storage mode to `EXTERNAL` for large JSONB columns
- Use BRIN indexes for time-series data

---

## Next Steps

1. ✅ Unified trace events implemented
2. ✅ Data extraction strategies documented
3. **TODO**: Implement balance tracking indexer
4. **TODO**: Implement storage tracking indexer
5. **TODO**: Implement AMM data indexer
6. **TODO**: Add API endpoints
7. **TODO**: Performance testing & optimization

---

## Conclusion

The unified trace system provides **complete observability** of the alkanes blockchain. Every balance transfer, storage change, contract call, and state transition is captured in a single, unified format. This enables building comprehensive APIs that rival or exceed specialized indexers like espo, all from a single data source.
