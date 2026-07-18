# Alkanes Data API - Setup Guide

## Quick Start

### Prerequisites

1. **Database**: PostgreSQL with alkanes-contract-indexer schema (must be running and indexed)
2. **Redis**: For caching
3. **Sandshrew RPC**: Unified Bitcoin + Metashrew endpoint  
4. **Infura API Key** (optional): For BTC price endpoints only

---

## Environment Variables

All environment variables are configured in the docker-compose files. **No .env file needed.**

### Required Variables (Already Set)

```yaml
DATABASE_URL: postgres://alkanes_user:alkanes_pass@postgres:5432/alkanes_indexer
REDIS_URL: redis://redis:6379
SANDSHREW_URL: http://jsonrpc:18888
NETWORK_ENV: regtest  # or signet, mainnet
ALKANE_FACTORY_ID: "4:65522"  # Network-specific
HOST: 0.0.0.0
PORT: 3000
RUST_LOG: info,alkanes_data_api=debug
```

### Optional Variable (Needs Configuration)

```yaml
INFURA_ENDPOINT: https://mainnet.infura.io/v3/YOUR_INFURA_KEY_HERE
```

**What it's for**: Only needed for 4 BTC price endpoints that query Uniswap V3 on Ethereum mainnet.

**How to get a key**:
1. Go to https://infura.io/ (free tier available)
2. Create account and project
3. Copy your Project ID
4. Update the `INFURA_ENDPOINT` in docker-compose.yaml:
   ```yaml
   INFURA_ENDPOINT: https://mainnet.infura.io/v3/YOUR_PROJECT_ID
   ```

---

## Deployment

### Step 1: Ensure Database is Ready

The alkanes-data-api requires the database to be populated by alkanes-contract-indexer first.

**Check if indexer is running:**
```bash
docker-compose ps alkanes-contract-indexer
```

**Check database tables exist:**
```bash
docker-compose exec postgres psql -U alkanes_user -d alkanes_indexer -c "\dt"
```

You should see tables like: `pool`, `swap`, `mint`, `burn`, `pool_creation`, `wrap`, etc.

**If tables don't exist**, start the indexer first:
```bash
docker-compose up -d alkanes-contract-indexer
# Wait for it to index blocks
docker-compose logs -f alkanes-contract-indexer
```

### Step 2: Configure Infura (Optional)

If you want BTC price endpoints:
1. Get your Infura API key (see above)
2. Edit `docker-compose.yaml` and update `INFURA_ENDPOINT`

### Step 3: Start the API

```bash
docker-compose up -d alkanes-data-api
```

### Step 4: Verify

```bash
# Health check
curl http://localhost:3000/api/v1/health
# Should return: OK

# Test an alkanes endpoint
curl -X POST http://localhost:3000/api/v1/get-alkanes \
  -H "Content-Type: application/json" \
  -d '{}'

# Test BTC price (requires Infura)
curl -X POST http://localhost:3000/api/v1/get-bitcoin-price \
  -H "Content-Type: application/json"
```

---

## Endpoint Status

### ✅ Working Without Infura (39 endpoints)

- **Health**: 1 endpoint
- **Alkanes**: 6 endpoints  
- **Pools**: 7 endpoints
- **History**: 18 endpoints
- **Bitcoin/UTXOs**: 7 endpoints

### ⚠️ Requires Infura (4 endpoints)

- `POST /api/v1/get-bitcoin-price`
- `POST /api/v1/get-bitcoin-market-chart`
- `POST /api/v1/get-bitcoin-market-weekly`
- `POST /api/v1/get-bitcoin-markets`

---

## Troubleshooting

### Error: "relation does not exist"

**Problem**: Database tables are missing.

**Solution**: Ensure alkanes-contract-indexer has run and created the schema:
```bash
docker-compose up -d alkanes-contract-indexer
docker-compose logs -f alkanes-contract-indexer
```

Wait for it to index at least a few blocks.

### Error: "Failed to call slot0"

**Problem**: Invalid or missing Infura API key.

**Solution**:
1. Get valid Infura API key from https://infura.io/
2. Update `INFURA_ENDPOINT` in docker-compose.yaml
3. Restart: `docker-compose restart alkanes-data-api`

### Port Already in Use

**Problem**: Port 3000 is already bound.

**Solution**: Change the port in docker-compose.yaml:
```yaml
ports:
  - "3001:3000"  # Map to different host port
```

### Can't Connect to Database

**Problem**: PostgreSQL not ready or wrong credentials.

**Solution**: Check PostgreSQL is running and credentials match:
```bash
docker-compose ps postgres
docker-compose logs postgres
```

---

## Configuration for Different Networks

### Regtest (docker-compose.yaml)
```yaml
NETWORK_ENV: regtest
ALKANE_FACTORY_ID: "4:65522"
SANDSHREW_URL: http://jsonrpc:18888
```

### Signet (docker-compose.signet.yaml)
```yaml
NETWORK_ENV: signet
ALKANE_FACTORY_ID: "0:0"
SANDSHREW_URL: http://jsonrpc:18888
```

### Mainnet (docker-compose.mainnet.yaml)
```yaml
NETWORK_ENV: mainnet
ALKANE_FACTORY_ID: "840000:1"
SANDSHREW_URL: http://jsonrpc:18888
```

---

## Summary

**To deploy alkanes-data-api:**

1. ✅ **Start indexer first** - Ensure alkanes-contract-indexer is running and has indexed data
2. ✅ **All env vars set** - No .env file needed, everything in docker-compose.yaml
3. ⚠️ **Optional: Set Infura key** - Only needed for 4 BTC price endpoints
4. ✅ **Start API** - `docker-compose up -d alkanes-data-api`
5. ✅ **Test** - `curl http://localhost:3000/api/v1/health`

**39 out of 43 endpoints work immediately without any additional configuration!**
