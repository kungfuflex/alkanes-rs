# Alkanes Data API - TODO List

## Critical: Database Query Column Name Fixes

All SQL queries need to use PascalCase table names and camelCase column names with quotes to match the PostgreSQL schema.

### Files to Fix:
1. `src/services/pools.rs` (~10 queries)
2. `src/services/history.rs` (~50+ queries)

### Column Name Mapping:
```
snake_case (current) → "camelCase" (schema)
----------------------------------------
pool_id              → "poolId"
block_height         → "blockHeight"
token0_amount        → "token0Amount"
token1_amount        → "token1Amount"
token_supply         → "tokenSupply"
pool_block_id        → "poolBlockId"
pool_tx_id           → "poolTxId"
token0_block_id      → "token0BlockId"
token0_tx_id         → "token0TxId"
token1_block_id      → "token1BlockId"
token1_tx_id         → "token1TxId"
factory_block_id     → "factoryBlockId"
factory_tx_id        → "factoryTxId"
creator_address      → "creatorAddress"
transaction_id       → "transactionId"
transaction_index    → "transactionIndex"
sold_token_block_id  → "soldTokenBlockId"
sold_token_tx_id     → "soldTokenTxId"
bought_token_block_id → "boughtTokenBlockId"
bought_token_tx_id   → "boughtTokenTxId"
sold_amount          → "soldAmount"
bought_amount        → "boughtAmount"
seller_address       → "sellerAddress"
from_address         → "fromAddress"
to_address           → "toAddress"
```

### Table Names to Fix:
```
pool             → "Pool"
pool_state       → "PoolState"
pool_creation    → "PoolCreation"
pool_swap        → "PoolSwap"
pool_mint        → "PoolMint"
pool_burn        → "PoolBurn"
processed_blocks → "ProcessedBlocks"
```

## Implemented ✅

### Core Services
- ✅ `alkanes_rpc.rs` - RPC client for Sandshrew
- ✅ `price.rs` - Bitcoin price from Uniswap V3 via Infura with custom headers
- ✅ `bitcoin.rs` - Bitcoin blockchain queries (7 endpoints)
- ✅ `alkanes.rs` - Alkanes token operations (fixed static_opcodes to use "99", "100", "102", "104", "1000")
- ✅ `pools.rs` - AMM pool operations (needs column name fixes)
- ✅ `history.rs` - Transaction history (needs column name fixes)

### Handlers
- ✅ `handlers/alkanes.rs` - 6 endpoints
- ✅ `handlers/pools.rs` - 7 endpoints  
- ✅ `handlers/history.rs` - 18 endpoints
- ✅ `handlers/bitcoin.rs` - 7 endpoints
- ✅ `handlers/price.rs` - 4 endpoints
- ✅ `handlers/health.rs` - 1 endpoint

### Infrastructure
- ✅ Docker image built and tested
- ✅ Database schema auto-initialization
- ✅ Redis caching setup
- ✅ Environment variables configured
- ✅ All dependencies resolved

## Testing Status

### Working Endpoints ✅
- `/api/v1/health` - Returns OK
- `/api/v1/get-bitcoin-price` - Real Infura data ($86,965.73)
- `/api/v1/get-bitcoin-market-chart` - Price/market cap/volume data
- `/api/v1/get-alkanes` - Returns empty (expected - no indexed data yet)

### Blocked by Column Names ⚠️
- All `/api/v1/get-pool*` endpoints
- All `/api/v1/get-*-history` endpoints
- Pool-related queries

## Future Enhancements

### Alkanes Service
- [ ] Implement full `get_alkanes()` with proper pagination and caching
- [ ] Implement `global_search()` for alkanes by name/symbol/ID
- [ ] Add price fetching from pools for alkane tokens
- [ ] Filter LP tokens in `get_alkanes_by_address()`

### Pools Service
- [ ] Multi-hop swap routing (currently only direct pairs)
- [ ] Pool volume calculations
- [ ] Pool APR/APY calculations
- [ ] Liquidity depth analysis

### History Service  
- [ ] Enhanced filtering options
- [ ] Time-based aggregations
- [ ] CSV export functionality

### Performance
- [ ] Connection pooling optimization
- [ ] Query batching where possible
- [ ] Cache warming strategies
- [ ] Database query optimization with EXPLAIN ANALYZE

### Monitoring
- [ ] Prometheus metrics export
- [ ] Request/response logging
- [ ] Error tracking integration
- [ ] Performance monitoring

## Next Steps

1. **Immediate Priority**: Fix all SQL column names in `pools.rs` and `history.rs`
2. **Test**: Verify all 43 endpoints work with real database data
3. **Deploy**: Update docker-compose and restart services
4. **Monitor**: Check logs for errors and performance issues
5. **Optimize**: Profile slow queries and add indexes as needed

## Reference

Original TypeScript implementation: `/data/alkanes-rs/reference/oyl-api/src.ts/services/alkanes/`

Key files:
- `alkanes.ts` - Main alkanes service logic
- `constants.ts` - staticOpcodes and other constants
- `helpers.ts` - Utility functions
- `market/poolService.ts` - Pool operations
- `market/volumeService.ts` - Volume calculations
