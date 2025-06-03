# OYL Protocol Reference Material

This document provides comprehensive reference information gathered from analyzing the OYL protocol contracts and alkanes-rs storage patterns.

## OYL Contract Analysis

### 1. OYL Token Contract (`oyl-token`)

**Opcodes Available:**
- `0`: Initialize - `Initialize { total_supply: u128, name: String, symbol: String }`
- `99`: GetName - Returns token name as bytes
- `100`: GetSymbol - Returns token symbol as bytes  
- `101`: GetTotalSupply - Returns total supply as u128 (16 bytes)
- `1000`: GetData - Returns arbitrary token data as bytes

**Storage Pattern:**
- Uses `MintableToken` trait for name/symbol storage
- Implements `AuthenticatedResponder` for ownership control
- Token metadata stored in alkane's internal storage

**Key Implementation Details:**
```rust
// Token initialization creates total supply and sets metadata
fn initialize(&self, total_supply: u128, name: String, symbol: String) -> Result<CallResponse> {
    self.observe_initialization()?;
    <Self as MintableToken>::set_name_and_symbol_str(self, name, symbol);
    response.alkanes.0.push(self.mint(&context, total_supply)?);
    Ok(response)
}
```

### 2. AMM Factory Contract (`alkanes-runtime-factory`)

**Key Storage Locations:**
- `/pool_factory_id` - Stores the pool implementation alkane ID
- `/pools/{alkane_a_bytes}/{alkane_b_bytes}` - Maps token pairs to pool IDs
- `/all_pools/{index}` - Array of all pool IDs by index
- `/all_pools_length` - Total number of pools created

**Opcodes Available:**
- `0`: InitFactory - `{ pool_factory_id: u128, auth_token_units: u128 }`
- `1`: CreateNewPool - Creates new pool for incoming token pair
- `2`: FindExistingPoolId - `{ alkane_a: AlkaneId, alkane_b: AlkaneId }`
- `3`: GetAllPools - Returns list of all pool IDs
- `4`: GetNumPools - Returns total number of pools
- `7`: SetPoolFactoryId - `{ pool_factory_id: u128 }`
- `10`: CollectFees - `{ pool_id: AlkaneId }`
- `20`: SwapExactTokensForTokensAlongPath - Multi-hop swapping

**Pool Creation Process:**
```rust
// Pools are created with sorted token pairs to ensure uniqueness
let (a, b) = sort_alkanes((alkane_a.id.clone(), alkane_b.id.clone()));
let pool_id = AlkaneId::new(2, self.sequence()); // New alkane at [2, sequence]

// Pool registry is maintained
StoragePointer::from_keyword("/all_pools/")
    .select(&length.to_le_bytes().to_vec())
    .set(Arc::new(pool_id.into()));
```

### 3. AMM Pool Contract (`alkanes-runtime-pool`)

**Key Storage Locations:**
- `/factory_id` - Factory that created this pool
- `/alkane/0` - First token in the pair
- `/alkane/1` - Second token in the pair  
- `/claimablefees` - Accumulated protocol fees
- `/klast` - Last K value for fee calculation

**Opcodes Available:**
- `0`: InitPool - `{ alkane_a: AlkaneId, alkane_b: AlkaneId, factory: AlkaneId }`
- `1`: AddLiquidity - Add liquidity to pool
- `2`: Burn - Remove liquidity from pool
- `3`: SwapExactTokensForTokens - `{ amount_out_predicate: u128, deadline: u128 }`
- `4`: SwapTokensForExactTokens - `{ amount_out: u128, amount_in_max: u128, deadline: u128 }`
- `10`: CollectFees - Collect protocol fees
- `20`: Swap - Low-level swap function
- `50`: ForwardIncoming - Forward incoming tokens
- `99`: GetName - Pool name (e.g., "TOKEN_A / TOKEN_B LP")
- `999`: PoolDetails - Returns comprehensive pool information

**Pool Details Response:**
```rust
pub struct PoolInfo {
    pub token_a: AlkaneId,
    pub token_b: AlkaneId,
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_supply: u128,
    pub pool_name: String,
}
```

## Alkanes-rs Storage Patterns

### 1. Global Storage Layout

**Alkane Storage Keyspace:**
```
/alkanes/{alkane_id}/                    # Root for alkane-specific data
/alkanes/{alkane_id}/balances/{holder}   # Token balances by holder
/alkanes/{alkane_id}/inventory/          # List of tokens held by alkane
/alkanes/{alkane_id}/storage/{key}       # Alkane's internal storage
```

**Global Index Tables:**
```
/alkanes_id_to_outpoint/{alkane_id}      # Maps alkane ID to creation outpoint
/alkanes/{alkane_id}/                    # Alkane bytecode storage (compressed)
```

### 2. Balance Management

**Balance Pointer Pattern:**
```rust
pub fn balance_pointer(
    atomic: &mut AtomicPointer,
    who: &AlkaneId,      // Token holder
    what: &AlkaneId,     // Token being held
) -> AtomicPointer {
    atomic
        .derive(&IndexPointer::default())
        .keyword("/alkanes/")
        .select(&what_bytes)           // Token ID
        .keyword("/balances/")
        .select(&who_bytes)            // Holder ID
}
```

**Inventory Tracking:**
```rust
pub fn alkane_inventory_pointer(who: &AlkaneId) -> IndexPointer {
    IndexPointer::from_keyword("/alkanes")
        .select(&who_bytes)
        .keyword("/inventory/")
}
```

### 3. Storage Access from View Functions

**Direct Storage Access:**
```rust
// Access alkane's internal storage
let storage_value = IndexPointer::from_keyword("/alkanes/")
    .select(&alkane_id.into())
    .keyword("/storage/")
    .select(&storage_key)
    .get();
```

**Balance Queries:**
```rust
// Get balance of token `what` held by `who`
let balance = IndexPointer::from_keyword("/alkanes/")
    .select(&what.into())
    .keyword("/balances/")
    .select(&who.into())
    .get_value::<u128>();
```

## OYL Data Access Patterns

### 1. Token Information Access

**Basic Token Data:**
```rust
// Get token name (opcode 99)
let name_data = call_view(&token_id, &vec![99], STATIC_FUEL)?;
let name = String::from_utf8(name_data)?;

// Get token symbol (opcode 100)  
let symbol_data = call_view(&token_id, &vec![100], STATIC_FUEL)?;
let symbol = String::from_utf8(symbol_data)?;

// Get total supply (opcode 101)
let supply_data = call_view(&token_id, &vec![101], STATIC_FUEL)?;
let total_supply = u128::from_le_bytes(supply_data.try_into()?);
```

**Token Holders:**
```rust
// Access all holders of a token via balance storage
let balance_prefix = IndexPointer::from_keyword("/alkanes/")
    .select(&token_id.into())
    .keyword("/balances/");
    
// Iterate through all balance entries to find holders
for holder_key in balance_prefix.get_list() {
    let balance = balance_prefix.select(&holder_key).get_value::<u128>();
    if balance > 0 {
        // holder_key is the AlkaneId of the holder
        holders.push((AlkaneId::from(holder_key), balance));
    }
}
```

### 2. Pool Information Access

**Pool Discovery:**
```rust
// Get all pools from factory (opcode 3)
let pools_data = call_view(&factory_id, &vec![3], STATIC_FUEL)?;
let mut cursor = Cursor::new(pools_data);
let pool_count = consume_u128(&mut cursor)?;

let mut pools = Vec::new();
for _ in 0..pool_count {
    let block = consume_u128(&mut cursor)?;
    let tx = consume_u128(&mut cursor)?;
    pools.push(AlkaneId::new(block, tx));
}
```

**Pool Details:**
```rust
// Get comprehensive pool information (opcode 999)
let pool_details = call_view(&pool_id, &vec![999], STATIC_FUEL)?;
let pool_info = PoolInfo::from_bytes(&pool_details)?;

// Pool reserves are stored as balances
let reserve_a = IndexPointer::from_keyword("/alkanes/")
    .select(&pool_info.token_a.into())
    .keyword("/balances/")
    .select(&pool_id.into())
    .get_value::<u128>();
```

### 3. Activity Tracking

**Transaction Traces:**
```rust
// Access transaction traces for activity events
let trace_data = IndexPointer::from_keyword("/traces/")
    .select(&outpoint_bytes)
    .get();
    
if !trace_data.is_empty() {
    let trace = AlkanesTrace::parse_from_bytes(&trace_data)?;
    // Extract swap, transfer, mint, burn events from trace
}
```

**Block-level Activity:**
```rust
// Get all activity for a specific block height
let block_traces = IndexPointer::from_keyword("/traces_by_height/")
    .select(&height.to_le_bytes())
    .get_list();
    
for outpoint_bytes in block_traces {
    let outpoint = OutPoint::consensus_decode(&outpoint_bytes)?;
    // Process each transaction's activity
}
```

## Price Calculation Strategies

### 1. Pool-based Pricing

**Constant Product Formula:**
```rust
// Calculate price from pool reserves
fn calculate_price(reserve_a: u128, reserve_b: u128) -> f64 {
    if reserve_a == 0 { return 0.0; }
    reserve_b as f64 / reserve_a as f64
}

// Get price from pool
let pool_details = call_view(&pool_id, &vec![999], STATIC_FUEL)?;
let pool_info = PoolInfo::from_bytes(&pool_details)?;
let price = calculate_price(pool_info.reserve_a, pool_info.reserve_b);
```

### 2. Multi-pool Aggregation

**Weighted Average Pricing:**
```rust
fn calculate_weighted_price(pools: &[PoolInfo]) -> f64 {
    let mut total_liquidity = 0u128;
    let mut weighted_sum = 0.0;
    
    for pool in pools {
        let liquidity = pool.reserve_a + pool.reserve_b;
        let price = pool.reserve_b as f64 / pool.reserve_a as f64;
        
        weighted_sum += price * liquidity as f64;
        total_liquidity += liquidity;
    }
    
    if total_liquidity == 0 { 0.0 } else { weighted_sum / total_liquidity as f64 }
}
```

## Storage Optimization Strategies

### 1. Indexing Tables

**Efficient Lookups:**
```rust
// Create lookup tables for common queries
pub static TOKEN_HOLDERS: LazyLock<KeyValuePointer> = 
    LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/token_holders/"));

pub static POOLS_BY_TOKEN: LazyLock<KeyValuePointer> = 
    LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/pools_by_token/"));

pub static ACTIVITIES_BY_TOKEN: LazyLock<KeyValuePointer> = 
    LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/activities_by_token/"));
```

### 2. Caching Strategy

**Price Caching:**
```rust
// Cache frequently accessed price data
pub static PRICE_CACHE: LazyLock<KeyValuePointer> = 
    LazyLock::new(|| KeyValuePointer::from_keyword("/oyl/price_cache/"));

// Store price with timestamp
let price_entry = PriceEntry {
    price: calculated_price,
    timestamp: current_timestamp,
    block_height: current_height,
};
PRICE_CACHE.select(&token_id.into()).set(Arc::new(price_entry.to_bytes()));
```

## Implementation Recommendations

### 1. View Function Pattern

```rust
pub fn oyl_token_info(input: &Vec<u8>) -> Result<TokenInfoResponse> {
    let request = TokenInfoRequest::parse_from_bytes(input)?;
    let mut response = TokenInfoResponse::new();
    
    // Get basic token data via opcodes
    let name = call_view(&request.token_id, &vec![99], STATIC_FUEL)
        .and_then(|v| String::from_utf8(v).map_err(|e| anyhow!(e)))
        .unwrap_or_default();
    
    // Get holders from balance storage
    let holders = get_token_holders(&request.token_id)?;
    
    // Get pools containing this token
    let pools = get_pools_for_token(&request.token_id)?;
    
    // Calculate derived data
    let price = calculate_token_price(&request.token_id, &pools)?;
    let market_cap = price * total_supply as f64;
    
    response.name = name;
    response.holders = holders.len() as u64;
    response.pools = pools.len() as u64;
    response.price = price;
    response.market_cap = market_cap;
    
    Ok(response)
}
```

### 2. Efficient Data Aggregation

```rust
// Batch multiple opcode calls for efficiency
pub fn get_multiple_token_data(token_ids: &[AlkaneId]) -> Result<Vec<TokenData>> {
    let mut calls = Vec::new();
    
    for token_id in token_ids {
        calls.push((*token_id, vec![99])); // name
        calls.push((*token_id, vec![100])); // symbol  
        calls.push((*token_id, vec![101])); // supply
    }
    
    let results = call_multiview(&token_ids, &calls.iter().map(|(_, inputs)| inputs.clone()).collect(), STATIC_FUEL)?;
    
    // Process batched results
    process_batched_token_data(results)
}
```

This reference material provides the foundation needed to implement comprehensive OYL protocol integration with efficient data access patterns and proper storage utilization.