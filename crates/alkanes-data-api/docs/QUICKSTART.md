# Alkanes Data API - Quick Start Guide

## Installation

```bash
cd /data/alkanes-rs
cargo build --release -p alkanes-data-api
```

## Configuration

1. Copy the example environment file:
```bash
cp crates/alkanes-data-api/.env.example crates/alkanes-data-api/.env
```

2. Edit `.env` with your settings:
```env
API_KEY=your_secret_api_key_here
DATABASE_URL=postgresql://user:password@localhost:5432/alkanes
REDIS_URL=redis://localhost:6379
BITCOIN_RPC_URL=http://localhost:8332
BITCOIN_RPC_USER=your_rpc_user
BITCOIN_RPC_PASSWORD=your_rpc_password
```

## Running

```bash
# From project root
cargo run --release -p alkanes-data-api

# Or run the binary directly
./target/release/alkanes-data-api
```

The API will start on `http://0.0.0.0:3000`

## Testing Endpoints

### Health Check (no auth required)
```bash
curl http://localhost:3000/api/v1/health
```

### Get Bitcoin Price
```bash
curl -X POST http://localhost:3000/api/v1/get-bitcoin-price \
  -H "x-oyl-api-key: your_secret_api_key_here" \
  -H "Content-Type: application/json"
```

Expected response:
```json
{
  "statusCode": 200,
  "data": {
    "bitcoin": {
      "usd": 98765.43
    }
  }
}
```

### Get Alkanes by Address
```bash
curl -X POST http://localhost:3000/api/v1/get-alkanes-by-address \
  -H "x-oyl-api-key: your_secret_api_key_here" \
  -H "Content-Type: application/json" \
  -d '{
    "address": "bc1pexampleaddress..."
  }'
```

### Get Pool Details
```bash
curl -X POST http://localhost:3000/api/v1/get-pool-details \
  -H "x-oyl-api-key: your_secret_api_key_here" \
  -H "Content-Type: application/json" \
  -d '{
    "poolId": {
      "block": "2",
      "tx": "123"
    }
  }'
```

### Get All Pools
```bash
curl -X POST http://localhost:3000/api/v1/get-pools \
  -H "x-oyl-api-key: your_secret_api_key_here" \
  -H "Content-Type: application/json" \
  -d '{}'
```

## Authentication

All endpoints (except `/health`) require the `x-oyl-api-key` header:

```bash
-H "x-oyl-api-key: your_secret_api_key_here"
```

Unauthorized requests will receive:
```json
{
  "statusCode": 403,
  "error": "Unauthorized: Invalid apiKey"
}
```

## Current Status

⚠️ **Note**: All endpoints are currently returning stub/empty responses. Database implementation is the next phase.

✅ **Working Now**:
- Bitcoin price feed (via Uniswap V3)
- API authentication
- Request routing
- Error handling

🔨 **Coming Next**:
- Database queries for alkanes data
- Pool state queries
- Transaction history
- UTXO queries

## Logging

Set the `RUST_LOG` environment variable for detailed logs:

```bash
# Info level (default)
RUST_LOG=info cargo run --release -p alkanes-data-api

# Debug level
RUST_LOG=debug cargo run --release -p alkanes-data-api

# Specific module
RUST_LOG=alkanes_data_api=debug cargo run --release -p alkanes-data-api
```

## Common Issues

### Port Already in Use
```
Error: Os { code: 48, kind: AddrInUse, message: "Address already in use" }
```

Solution: Change the port in `.env`:
```env
PORT=3001
```

### Database Connection Failed
```
Error: error connecting to database: Connection refused
```

Solution: Ensure PostgreSQL is running and `DATABASE_URL` is correct:
```bash
# Check if PostgreSQL is running
pg_isready

# Test connection
psql postgresql://user:password@localhost:5432/alkanes
```

### Invalid API Key
```json
{
  "statusCode": 403,
  "error": "Unauthorized: Invalid apiKey"
}
```

Solution: Ensure you're using the same `API_KEY` from your `.env` file in the request header.

### Bitcoin RPC Connection Failed
```
Error: failed to connect to bitcoin rpc
```

Solution: Check Bitcoin Core is running and RPC credentials are correct:
```bash
bitcoin-cli -rpcuser=your_user -rpcpassword=your_pass getblockchaininfo
```

## Development

### Check Code
```bash
cargo check -p alkanes-data-api
```

### Format Code
```bash
cargo fmt -p alkanes-data-api
```

### Lint Code
```bash
cargo clippy -p alkanes-data-api
```

### Watch for Changes
```bash
cargo watch -x 'run --release -p alkanes-data-api'
```

## Integration with Docker Compose

Add to your `docker-compose.yaml`:

```yaml
services:
  alkanes-data-api:
    build:
      context: .
      dockerfile: crates/alkanes-data-api/Dockerfile
    environment:
      HOST: 0.0.0.0
      PORT: 3000
      API_KEY: ${API_KEY}
      DATABASE_URL: postgresql://postgres:password@postgres:5432/alkanes
      REDIS_URL: redis://redis:6379
      BITCOIN_RPC_URL: http://bitcoin:8332
      BITCOIN_RPC_USER: bitcoin
      BITCOIN_RPC_PASSWORD: password
      INFURA_ENDPOINT: https://mainnet.infura.io/v3/099fc58e0de9451d80b18d7c74caa7c1
      RUST_LOG: info
    ports:
      - "3000:3000"
    depends_on:
      - postgres
      - redis
      - bitcoin
    restart: unless-stopped
```

## API Documentation

Full API documentation is available at:
- [README.md](./README.md) - Overview and features
- [IMPLEMENTATION_STATUS.md](./IMPLEMENTATION_STATUS.md) - Implementation status
- [SUMMARY.md](./SUMMARY.md) - Technical summary

## Next Steps

1. **Implement Database Queries**
   - Start with alkanes endpoints
   - Add pool queries
   - Implement transaction history
   - Add Bitcoin UTXO queries

2. **Add Redis Caching**
   - Cache frequently accessed data
   - Implement cache invalidation
   - Add TTL for different data types

3. **Optimize Performance**
   - Add database indexes
   - Implement connection pooling tuning
   - Add request rate limiting
   - Implement response compression

4. **Add Tests**
   - Unit tests for handlers
   - Integration tests for database
   - API endpoint tests
   - Load testing

## Support

For issues or questions:
1. Check the logs: `RUST_LOG=debug cargo run`
2. Review the documentation files
3. Check database connectivity
4. Verify environment variables

## License

MIT
