# Alkanes Data API - Implementation Status

## ✅ Completed

### Core Infrastructure
- [x] Actix-web server setup with CORS and logging
- [x] API key authentication middleware
- [x] Configuration management via environment variables
- [x] PostgreSQL connection pooling (SQLx)
- [x] Redis client integration
- [x] Bitcoin Core RPC client
- [x] Alloy-rs integration for Ethereum RPC

### Bitcoin Price Feed (Uniswap V3)
- [x] Real-time BTC price from WBTC/USDC pool
- [x] Price caching (60-second TTL)
- [x] Historical price chart generation
- [x] 52-week high/low data
- [x] Market summary data

### API Endpoints Structure
- [x] Health check endpoint
- [x] 4 Bitcoin price endpoints
- [x] 6 Alkanes query endpoints
- [x] 7 Pool/AMM endpoints
- [x] 18 Transaction history endpoints
- [x] 7 Bitcoin/UTXO endpoints

**Total: 43 endpoints defined**

## 🔨 Next Steps (Database Implementation)

All endpoints currently return stub responses. Next phase is to implement actual database queries:

### Phase 1: Alkanes Queries
- [ ] `get_alkanes` - Query all alkanes from database
- [ ] `get_alkanes_by_address` - Query alkanes by address
- [ ] `get_alkane_details` - Query specific alkane details
- [ ] `get_alkanes_utxo` - Query alkane UTXOs
- [ ] `get_amm_utxos` - Query spendable UTXOs (exclude alkane UTXOs)
- [ ] `global_alkanes_search` - Full-text search across alkanes

### Phase 2: Pool Queries
- [ ] `get_pools` - Query all pools
- [ ] `get_pool_details` - Query specific pool with metrics
- [ ] `get_all_pools_details` - Query all pools with full details
- [ ] `address_positions` - Query liquidity positions for address
- [ ] `get_all_token_pairs` - Query all token pairs for factory
- [ ] `get_token_pairs` - Query pairs containing specific token
- [ ] `get_alkane_swap_pair_details` - Calculate swap routing paths

### Phase 3: Transaction History
- [ ] Pool swap history
- [ ] Token swap history
- [ ] Liquidity mint/burn history
- [ ] Pool creation history
- [ ] Wrap/unwrap history
- [ ] Address-specific transaction queries
- [ ] Combined AMM transaction history with filtering

### Phase 4: Bitcoin Data
- [ ] Address balance queries (via Bitcoin RPC or indexer)
- [ ] UTXO queries with spend strategy support
- [ ] Transaction history queries
- [ ] Intent history for wallet integration
- [ ] Account-level (multi-address) queries

## Database Schema Requirements

The implementation assumes the PostgreSQL schema from `alkanes-contract-indexer`:

### Required Tables
- `alkanes` - Alkane token definitions
- `pools` - AMM pool states
- `pool_states` - Historical pool state snapshots
- `swaps` - Swap transactions
- `mints` - Liquidity additions
- `burns` - Liquidity removals
- `wraps` - BTC wrap transactions
- `unwraps` - BTC unwrap transactions

### Required Indexes
- Address indexes for all transaction types
- Pool ID indexes
- Alkane ID indexes
- Timestamp indexes for historical queries
- Composite indexes for common query patterns

## API Usage Examples

### Get Bitcoin Price
```bash
curl -X POST http://localhost:3000/api/v1/get-bitcoin-price \
  -H "x-oyl-api-key: your_key" \
  -H "Content-Type: application/json"
```

### Get Alkanes by Address
```bash
curl -X POST http://localhost:3000/api/v1/get-alkanes-by-address \
  -H "x-oyl-api-key: your_key" \
  -H "Content-Type: application/json" \
  -d '{"address": "bc1p..."}'
```

### Get Pool Details
```bash
curl -X POST http://localhost:3000/api/v1/get-pool-details \
  -H "x-oyl-api-key: your_key" \
  -H "Content-Type: application/json" \
  -d '{
    "poolId": {
      "block": "2",
      "tx": "123"
    }
  }'
```

## Dependencies

- **actix-web 4.8** - Web framework
- **alloy 0.6** - Ethereum/Uniswap integration
- **sqlx 0.7** - PostgreSQL async driver
- **redis 0.25** - Caching layer
- **bitcoincore-rpc 0.19** - Bitcoin Core RPC client
- **tokio** - Async runtime

## Configuration

Required environment variables:
- `API_KEY` - API authentication key
- `DATABASE_URL` - PostgreSQL connection string
- `REDIS_URL` - Redis connection string
- `BITCOIN_RPC_URL` - Bitcoin Core RPC endpoint
- `BITCOIN_RPC_USER` - Bitcoin RPC username
- `BITCOIN_RPC_PASSWORD` - Bitcoin RPC password
- `INFURA_ENDPOINT` - Ethereum RPC endpoint (default provided)

## Docker Compose Integration

The API can be added to `docker-compose.yaml`, `docker-compose.signet.yaml`, and `docker-compose.mainnet.yaml`:

```yaml
alkanes-data-api:
  build:
    context: .
    dockerfile: Dockerfile.data-api
  environment:
    - DATABASE_URL=postgresql://user:pass@postgres:5432/alkanes
    - REDIS_URL=redis://redis:6379
    - BITCOIN_RPC_URL=http://bitcoin:8332
    - API_KEY=${API_KEY}
  ports:
    - "3000:3000"
  depends_on:
    - postgres
    - redis
    - bitcoin
```

## Performance Considerations

1. **Price Caching**: BTC price cached for 60s to minimize Ethereum RPC calls
2. **Database Connection Pool**: Max 10 concurrent connections
3. **Redis Caching**: Should cache frequently accessed data (pools, token info)
4. **Pagination**: All list endpoints support limit/offset pagination
5. **Indexing**: Requires proper database indexes for query performance

## Next Milestone

**Milestone 1**: Implement Phase 1 (Alkanes Queries) with real database queries
- Connect to PostgreSQL database from alkanes-contract-indexer
- Implement actual SQL queries for alkanes endpoints
- Add Redis caching for alkane metadata
- Test against regtest environment
