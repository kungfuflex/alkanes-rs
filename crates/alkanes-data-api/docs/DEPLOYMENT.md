# Deployment Guide for Alkanes Data API

## Quick Start

The Alkanes Data API has been successfully implemented and compiled. The binary is ready for deployment.

## Binary Information

- **Location**: `target/release/alkanes-data-api`
- **Size**: ~15MB
- **Language**: Rust
- **Framework**: actix-web 4.0

## Prerequisites

Before deploying, ensure you have:

1. **PostgreSQL Database** with alkanes-contract-indexer schema
2. **Redis Server** for caching
3. **Sandshrew RPC** endpoint (unified Bitcoin + Metashrew API)
4. **Ethereum RPC** endpoint (for BTC price feed via Uniswap V3)

## Environment Variables

Create a `.env` file with the following variables:

```env
# Required
DATABASE_URL=postgresql://postgres:password@localhost:5432/alkanes
REDIS_URL=redis://localhost:6379
SANDSHREW_URL=http://localhost:8080

# Network Configuration
NETWORK_ENV=mainnet  # Options: mainnet, testnet, signet, regtest
ALKANE_FACTORY_ID=840000:1  # Format: block:tx

# Server
HOST=0.0.0.0
PORT=3000

# Ethereum RPC for Price Feed
ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/YOUR_INFURA_PROJECT_ID

# Logging
RUST_LOG=info,alkanes_data_api=debug
```

## Deployment Options

### Option 1: Direct Binary Execution

```bash
# 1. Ensure dependencies are running
docker-compose up -d postgres redis sandshrew

# 2. Set environment variables
export DATABASE_URL="postgresql://postgres:password@localhost:5432/alkanes"
export REDIS_URL="redis://localhost:6379"
export SANDSHREW_URL="http://localhost:8080"
export NETWORK_ENV="mainnet"
export ALKANE_FACTORY_ID="840000:1"
export ETHEREUM_RPC_URL="https://mainnet.infura.io/v3/YOUR_KEY"

# 3. Run the binary
./target/release/alkanes-data-api
```

### Option 2: Docker Deployment

```bash
# Build the image
docker build -f crates/alkanes-data-api/Dockerfile -t alkanes-data-api:latest .

# Run with Docker
docker run -d \
  --name alkanes-data-api \
  -p 3000:3000 \
  --env-file .env \
  alkanes-data-api:latest
```

### Option 3: Docker Compose (Recommended)

Add to your `docker-compose.yaml`:

```yaml
services:
  alkanes-data-api:
    build:
      context: .
      dockerfile: crates/alkanes-data-api/Dockerfile
    container_name: alkanes-data-api
    environment:
      DATABASE_URL: postgresql://postgres:password@postgres:5432/alkanes
      REDIS_URL: redis://redis:6379
      SANDSHREW_URL: http://sandshrew:8080
      NETWORK_ENV: mainnet
      ALKANE_FACTORY_ID: "840000:1"
      ETHEREUM_RPC_URL: ${ETHEREUM_RPC_URL}
      HOST: 0.0.0.0
      PORT: 3000
      RUST_LOG: info,alkanes_data_api=debug
    ports:
      - "3000:3000"
    depends_on:
      - postgres
      - redis
      - sandshrew
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/api/v1/health"]
      interval: 30s
      timeout: 3s
      retries: 3
      start_period: 10s
```

Then deploy:

```bash
docker-compose up -d alkanes-data-api
```

## Verification

### Health Check

```bash
curl http://localhost:3000/api/v1/health
```

Expected response:
```json
{
  "statusCode": 200,
  "data": {
    "status": "healthy",
    "version": "1.0.0"
  }
}
```

### Test BTC Price Endpoint

```bash
curl -X POST http://localhost:3000/api/v1/get-bitcoin-price \
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

## Monitoring

### Logs

```bash
# Docker logs
docker logs -f alkanes-data-api

# Direct binary logs (if using systemd)
journalctl -u alkanes-data-api -f
```

### Metrics

The API includes:
- Health check endpoint at `/api/v1/health`
- Built-in logging with configurable levels via `RUST_LOG`

## Production Recommendations

### 1. Use systemd (for non-Docker deployments)

Create `/etc/systemd/system/alkanes-data-api.service`:

```ini
[Unit]
Description=Alkanes Data API
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=alkanes
WorkingDirectory=/opt/alkanes
EnvironmentFile=/opt/alkanes/.env
ExecStart=/opt/alkanes/target/release/alkanes-data-api
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable alkanes-data-api
sudo systemctl start alkanes-data-api
sudo systemctl status alkanes-data-api
```

### 2. Reverse Proxy with Nginx

```nginx
server {
    listen 80;
    server_name api.example.com;

    location /api/v1/ {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }
}
```

### 3. Performance Tuning

**PostgreSQL Connection Pool**:
- Default: 10 connections
- Adjust based on load: `DATABASE_MAX_CONNECTIONS=20`

**Redis Caching**:
- Block-height-based invalidation for pools
- 60-second cache for BTC price

**CORS Configuration**:
- Currently allows all origins (`*`)
- Restrict in production by editing `src/main.rs`

### 4. Security

**Database Access**:
- Use read-only database user for API
- Enable SSL/TLS for PostgreSQL connections

**Network Security**:
- Run API behind firewall
- Use TLS/SSL (HTTPS) via reverse proxy
- Rate limiting via nginx or application gateway

**Environment Variables**:
- Never commit `.env` to version control
- Use secrets management (AWS Secrets Manager, Vault, etc.)

## Troubleshooting

### Database Connection Issues

```bash
# Test PostgreSQL connection
psql $DATABASE_URL -c "SELECT 1"

# Check if required tables exist
psql $DATABASE_URL -c "\dt"
```

### Redis Connection Issues

```bash
# Test Redis connection
redis-cli -u $REDIS_URL ping
```

### Sandshrew RPC Issues

```bash
# Test Sandshrew endpoint
curl -X POST $SANDSHREW_URL \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}'
```

### High Memory Usage

- Check Redis memory usage: `redis-cli INFO memory`
- Monitor database connection pool: Check logs for connection leaks
- Consider increasing connection pool size if many concurrent requests

## Architecture Summary

### Service Layer Architecture

1. **AlkanesRpcClient** (`services/alkanes_rpc.rs`)
   - Unified JSON-RPC client for Sandshrew
   - Handles all blockchain queries

2. **AlkanesService** (`services/alkanes.rs`)
   - Business logic for alkanes operations
   - Balance aggregation, UTXO management

3. **PoolService** (`services/pools.rs`)
   - Database queries for AMM pools
   - Redis caching with block-height invalidation

4. **HistoryService** (`services/history.rs`)
   - Transaction history queries
   - Supports pagination and filtering

5. **BitcoinService** (`services/bitcoin.rs`)
   - Bitcoin/UTXO operations
   - Address balance calculations

6. **PriceService** (`services/price.rs`)
   - BTC price feed via Uniswap V3
   - Uses alloy-rs for Ethereum interaction

### Database Schema

The API expects PostgreSQL tables from alkanes-contract-indexer:
- `pool` - AMM pool records
- `pool_state` - Pool state snapshots
- `pool_creation` - Pool creation events
- `swap` - Swap transactions
- `mint` - Liquidity additions
- `burn` - Liquidity removals
- `wrap` - BTC wrapping events

## Support

For issues or questions:
1. Check logs: `docker logs alkanes-data-api` or `journalctl -u alkanes-data-api`
2. Verify all dependencies are running
3. Confirm environment variables are set correctly
4. Review `FULL_IMPLEMENTATION_PLAN.md` for architectural details
