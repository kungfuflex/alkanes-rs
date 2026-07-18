# Alkanes Data API - Full Implementation Plan

## Status: In Progress

### Completed ✅
1. **Removed API Key Middleware** - No authentication required
2. **Created AlkanesRpcClient** - Handles all Sandshrew JSON-RPC calls
3. **Updated Configuration** - Uses SANDSHREW_URL instead of separate Bitcoin RPC
4. **Created AlkanesService** - Business logic layer for alkanes operations
5. **Updated get_alkanes handler** - Example of full implementation pattern

### Architecture Overview

```
┌─────────────────┐
│  HTTP Request   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Handler       │  (actix-web route handlers)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Service       │  (Business logic layer)
└────────┬────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌────────┐ ┌──────────┐
│  RPC   │ │ Database │  (Data access layer)
└────────┘ └──────────┘
    │         │
    ▼         ▼
┌────────┐ ┌──────────┐
│Sandshrew│ │PostgreSQL│
└────────┘ └──────────┘
```

## Implementation Layers

### 1. RPC Layer (`services/alkanes_rpc.rs`)

**Purpose**: Abstract all Sandshrew JSON-RPC calls

**Key Methods**:
- `get_alkanes_by_address(address)` - Get alkanes for address
- `simulate(request)` - Call alkane contracts (for metadata)
- `get_block_count()` - Get current block height
- `get_blockchain_info()` - Get chain info
- `get_address_utxos(address)` - Get UTXOs via esplora
- `get_transaction(txid)` - Get tx details
- `get_address_txs(address)` - Get address transaction history

**Implementation Pattern**:
```rust
async fn call(&self, method: &str, params: Value) -> Result<Value> {
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params,
        id: self.next_id(),
    };
    
    let response = self.client
        .post(&self.url)
        .json(&request)
        .send()
        .await?;
        
    let rpc_response: RpcResponse = response.json().await?;
    
    if let Some(error) = rpc_response.error {
        return Err(anyhow::anyhow!("RPC error: {:?}", error));
    }
    
    rpc_response.result
        .ok_or_else(|| anyhow::anyhow!("No result"))
}
```

### 2. Service Layer

#### A. Alkanes Service (`services/alkanes.rs`)

**Methods to Implement**:
- ✅ `get_alkanes_utxos(address)` - Get alkanes UTXOs
- ✅ `get_alkanes_by_address(address, filter_lp)` - Get alkanes with balances
- ✅ `get_static_alkane_data(id)` - Get metadata via simulation
- ⏳ `get_alkanes(limit, offset, sort, order, search)` - List all alkanes
- ⏳ `global_search(query)` - Search alkanes
- ✅ `get_alkane_details(id)` - Get full alkane details

**Reference**: `reference/oyl-api/src.ts/services/alkanes/alkanes.ts`

**Key Patterns**:
1. **Caching**: Check Redis before RPC calls
2. **Aggregation**: Sum balances across multiple UTXOs
3. **Simulation**: Call alkane contracts for metadata
4. **Price Enrichment**: Add price data from pools

#### B. Pool Service (`services/pools.rs` - TO CREATE)

**Methods to Implement**:
- `get_pools_by_factory(factory_id)` - List all pools
- `get_pool_by_id(pool_id)` - Get specific pool
- `get_pool_history(pool_id)` - Get historical states
- `get_address_positions(address)` - Get liquidity positions
- `calculate_pool_metrics(pool)` - Calculate TVL, APR, etc.

**Database Schema** (from `alkanes-contract-indexer`):
```sql
-- Pools table
CREATE TABLE pool (
  id TEXT PRIMARY KEY,
  factory_block_id TEXT,
  factory_tx_id TEXT,
  pool_block_id TEXT,
  pool_tx_id TEXT,
  token0_block_id TEXT,
  token0_tx_id TEXT,
  token1_block_id TEXT,
  token1_tx_id TEXT,
  pool_name TEXT,
  created_at TIMESTAMP,
  updated_at TIMESTAMP
);

-- Pool states table
CREATE TABLE pool_state (
  id SERIAL PRIMARY KEY,
  pool_id TEXT REFERENCES pool(id),
  block_height INTEGER,
  token0_amount TEXT,
  token1_amount TEXT,
  token_supply TEXT,
  timestamp TIMESTAMP
);

-- Pool creation events
CREATE TABLE pool_creation (
  id SERIAL PRIMARY KEY,
  pool_id TEXT REFERENCES pool(id),
  creator_address TEXT,
  token0_amount TEXT,
  token1_amount TEXT,
  block_height INTEGER,
  txid TEXT,
  timestamp TIMESTAMP
);
```

**Implementation Pattern**:
```rust
pub async fn get_pool_by_id(&self, pool_id: &AlkaneId) -> Result<Pool> {
    // Check cache
    let cache_key = format!("pool:{}:{}:latest", pool_id.block, pool_id.tx);
    if let Some(cached) = self.redis.get(&cache_key).await? {
        return Ok(cached);
    }
    
    // Query database
    let pool = sqlx::query_as!(
        Pool,
        r#"
        SELECT p.*, ps.token0_amount, ps.token1_amount, ps.token_supply
        FROM pool p
        JOIN LATERAL (
            SELECT * FROM pool_state
            WHERE pool_id = p.id
            ORDER BY block_height DESC
            LIMIT 1
        ) ps ON true
        WHERE p.pool_block_id = $1 AND p.pool_tx_id = $2
        "#,
        pool_id.block,
        pool_id.tx
    )
    .fetch_one(&self.db)
    .await?;
    
    // Cache result
    self.redis.set(&cache_key, &pool, 24 * 3600).await?;
    
    Ok(pool)
}
```

#### C. History/Volume Service (`services/history.rs` - TO CREATE)

**Methods to Implement**:
- `get_pool_swap_history(pool_id, limit, offset)` - Get swaps for pool
- `get_token_swap_history(token_id, limit, offset)` - Get swaps for token
- `get_pool_mint_history(pool_id, limit, offset)` - Get liquidity adds
- `get_pool_burn_history(pool_id, limit, offset)` - Get liquidity removes
- `get_address_swap_history(address, pool_id)` - Get user swaps
- `get_wrap_history(address, limit, offset)` - Get wrap transactions
- `get_unwrap_history(address, limit, offset)` - Get unwrap transactions
- `get_all_amm_tx_history(filters)` - Get combined AMM history

**Database Schema**:
```sql
-- Swaps table
CREATE TABLE swap (
  id SERIAL PRIMARY KEY,
  pool_id TEXT REFERENCES pool(id),
  from_address TEXT,
  token_in_block_id TEXT,
  token_in_tx_id TEXT,
  token_out_block_id TEXT,
  token_out_tx_id TEXT,
  amount_in TEXT,
  amount_out TEXT,
  block_height INTEGER,
  txid TEXT,
  successful BOOLEAN,
  timestamp TIMESTAMP
);

-- Mints table (liquidity adds)
CREATE TABLE mint (
  id SERIAL PRIMARY KEY,
  pool_id TEXT REFERENCES pool(id),
  from_address TEXT,
  token0_amount TEXT,
  token1_amount TEXT,
  liquidity_amount TEXT,
  block_height INTEGER,
  txid TEXT,
  successful BOOLEAN,
  timestamp TIMESTAMP
);

-- Burns table (liquidity removes)
CREATE TABLE burn (
  id SERIAL PRIMARY KEY,
  pool_id TEXT REFERENCES pool(id),
  from_address TEXT,
  token0_amount TEXT,
  token1_amount TEXT,
  liquidity_amount TEXT,
  block_height INTEGER,
  txid TEXT,
  successful BOOLEAN,
  timestamp TIMESTAMP
);

-- Wraps/unwraps
CREATE TABLE wrap (
  id SERIAL PRIMARY KEY,
  from_address TEXT,
  amount TEXT,
  block_height INTEGER,
  txid TEXT,
  successful BOOLEAN,
  timestamp TIMESTAMP
);
```

**Implementation Pattern**:
```rust
pub async fn get_pool_swap_history(
    &self,
    pool_id: &AlkaneId,
    limit: i32,
    offset: i32,
    successful_only: bool,
) -> Result<(Vec<Swap>, usize)> {
    let mut where_clauses = vec![
        "pool_block_id = $1",
        "pool_tx_id = $2",
    ];
    
    if successful_only {
        where_clauses.push("successful = true");
    }
    
    let where_str = where_clauses.join(" AND ");
    
    let query = format!(
        r#"
        SELECT * FROM swap
        WHERE {}
        ORDER BY block_height DESC, id DESC
        LIMIT $3 OFFSET $4
        "#,
        where_str
    );
    
    let swaps = sqlx::query_as::<_, Swap>(&query)
        .bind(&pool_id.block)
        .bind(&pool_id.tx)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;
    
    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) FROM swap WHERE {}",
        where_str
    );
    
    let total: (i64,) = sqlx::query_as(&count_query)
        .bind(&pool_id.block)
        .bind(&pool_id.tx)
        .fetch_one(&self.db)
        .await?;
    
    Ok((swaps, total.0 as usize))
}
```

#### D. Bitcoin/UTXO Service (`services/bitcoin.rs` - TO CREATE)

**Methods to Implement**:
- `get_address_utxos(address)` - Get all UTXOs
- `get_amm_utxos(address)` - Get spendable UTXOs (no runes/inscriptions/alkanes)
- `get_address_balance(address)` - Get BTC balance
- `get_address_tx_history(address, limit)` - Get transaction history

**Implementation Pattern**:
```rust
pub async fn get_address_utxos(&self, address: &str) -> Result<Vec<FormattedUtxo>> {
    // Get UTXOs from Sandshrew/Esplora
    let utxos = self.rpc.get_address_utxos(address).await?;
    
    // Get alkanes UTXOs
    let alkanes_utxos = self.alkanes_service
        .get_alkanes_utxos(address)
        .await?;
    
    let alkane_outpoints: HashSet<String> = alkanes_utxos
        .iter()
        .map(|u| format!("{}:{}", u.tx_id, u.output_index))
        .collect();
    
    // Filter out UTXOs with runes, inscriptions, or alkanes
    let spendable_utxos = utxos
        .filter(|u| {
            let outpoint = format!("{}:{}", u.txid, u.vout);
            u.runes.is_empty() 
                && u.inscriptions.is_empty() 
                && !alkane_outpoints.contains(&outpoint)
        })
        .map(|u| format_utxo(u, address))
        .collect();
    
    Ok(spendable_utxos)
}
```

### 3. Handler Layer

**Pattern**: All handlers follow the same structure:
1. Extract request parameters
2. Validate required fields
3. Create service instance
4. Call service method
5. Return formatted response or error

**Example** (from `handlers/alkanes.rs`):
```rust
pub async fn get_alkanes_by_address(
    state: web::Data<AppState>,
    req: web::Json<AddressRequest>,
) -> impl Responder {
    if req.address.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse::new(
            400,
            "address is required".to_string(),
        ));
    }

    let alkanes_service = AlkanesService::new(
        state.alkanes_rpc.clone(),
        state.redis_client.clone(),
    );

    match alkanes_service
        .get_alkanes_by_address(&req.address, true)
        .await
    {
        Ok(alkanes) => {
            let response = ApiResponse::ok(alkanes);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let error = ErrorResponse::with_stack(
                500,
                "Failed to get alkanes by address".to_string(),
                e.to_string(),
            );
            HttpResponse::InternalServerError().json(error)
        }
    }
}
```

### 4. Redis Caching Strategy

**Cache Keys**:
- `ALKANE-{block}-{tx}` - Static alkane metadata
- `ALKANES_RESULTS` - Full alkanes list (refreshed on block change)
- `pool:{factory}:{block}:{tx}:block-{height}` - Pool state at height
- `pools:{factory}:{block}:{tx}:block-{height}` - All pools for factory

**Cache Invalidation**:
- Block height change triggers background cache update
- TTL: 24 hours for most data
- No TTL for static metadata

**Implementation**:
```rust
pub async fn with_cache<T, F>(
    redis: &redis::Client,
    key: &str,
    ttl: Option<usize>,
    fetcher: F,
) -> Result<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
    F: Future<Output = Result<T>>,
{
    // Try cache
    let mut conn = redis.get_async_connection().await?;
    if let Ok(Some(cached)) = conn.get::<_, Option<String>>(key).await {
        if let Ok(value) = serde_json::from_str(&cached) {
            return Ok(value);
        }
    }
    
    // Fetch fresh data
    let value = fetcher.await?;
    
    // Store in cache
    let serialized = serde_json::to_string(&value)?;
    if let Some(ttl) = ttl {
        conn.set_ex(key, serialized, ttl).await?;
    } else {
        conn.set(key, serialized).await?;
    }
    
    Ok(value)
}
```

## Implementation Order

### Phase 1: Core Alkanes Functionality (Current)
1. ✅ AlkanesRpcClient
2. ✅ AlkanesService basic methods
3. ⏳ Complete alkanes handlers
4. ⏳ Redis caching integration
5. ⏳ Static metadata simulation

### Phase 2: Pool Queries
1. ⏳ Create PoolService
2. ⏳ Implement pool database queries
3. ⏳ Add pool metrics calculation
4. ⏳ Implement pool handlers
5. ⏳ Add token pair routing

### Phase 3: Transaction History
1. ⏳ Create HistoryService
2. ⏳ Implement swap history queries
3. ⏳ Implement mint/burn history
4. ⏳ Implement wrap/unwrap history
5. ⏳ Add combined history endpoint

### Phase 4: Bitcoin Integration
1. ⏳ Create BitcoinService
2. ⏳ Implement UTXO queries
3. ⏳ Implement transaction history
4. ⏳ Add balance queries
5. ⏳ Add intent history (for wallet)

### Phase 5: Testing & Optimization
1. ⏳ Integration tests
2. ⏳ Performance optimization
3. ⏳ Load testing
4. ⏳ Documentation
5. ⏳ Docker integration

## Testing Strategy

### Unit Tests
- Test each service method independently
- Mock RPC client responses
- Mock database queries
- Test error handling

### Integration Tests
- Test full request/response cycle
- Use test database
- Use mock Sandshrew server
- Test caching behavior

### Example Test:
```rust
#[actix_web::test]
async fn test_get_alkanes_by_address() {
    let mock_rpc = MockAlkanesRpcClient::new();
    mock_rpc.expect_get_alkanes_by_address()
        .returning(|_| Ok(vec![/* mock data */]));
    
    let service = AlkanesService::new(mock_rpc, mock_redis());
    
    let result = service
        .get_alkanes_by_address("bc1p...", true)
        .await
        .unwrap();
    
    assert!(!result.is_empty());
    assert_eq!(result[0].name, Some("Test Alkane".to_string()));
}
```

## Docker Integration

### docker-compose.yaml additions:
```yaml
  alkanes-data-api:
    build:
      context: .
      dockerfile: crates/alkanes-data-api/Dockerfile
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/alkanes
      - REDIS_URL=redis://redis:6379
      - SANDSHREW_URL=http://sandshrew:8080
      - NETWORK_ENV=regtest
      - INFURA_ENDPOINT=https://mainnet.infura.io/v3/099fc58e0de9451d80b18d7c74caa7c1
      - ALKANE_FACTORY_ID=2:123
      - RUST_LOG=info
    ports:
      - "3000:3000"
    depends_on:
      - postgres
      - redis
      - sandshrew
    restart: unless-stopped
```

## Next Steps

1. **Complete AlkanesService** - Finish all methods
2. **Create PoolService** - Implement pool queries
3. **Create HistoryService** - Implement transaction history
4. **Update all handlers** - Connect to services
5. **Add comprehensive tests** - Unit and integration
6. **Docker integration** - Add to compose files
7. **Documentation** - API docs and examples

## Reference Files

- TypeScript Reference: `/reference/oyl-api/src.ts/`
- Database Schema: `alkanes-contract-indexer` PostgreSQL schema
- RPC Methods: `alkanes-jsonrpc/src/sandshrew.rs`
- SDK Types: `alkanes-cli-common/src/alkanes/`
