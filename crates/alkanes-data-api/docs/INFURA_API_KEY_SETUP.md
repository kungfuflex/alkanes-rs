# Infura API Key Setup

## Issue: Bitcoin Price Endpoint Failing

The `/api/v1/get-bitcoin-price` endpoint requires a valid Ethereum RPC endpoint to query the Uniswap V3 WBTC/USDC pool for real-time BTC prices.

### Error Observed
```json
{
  "statusCode": 500,
  "error": "Failed to get bitcoin price",
  "stack": "Failed to call slot0"
}
```

**Root Cause**: The Infura API key in the configuration is invalid or restricted.

When testing with the configured endpoint:
```bash
curl -X POST https://mainnet.infura.io/v3/099fc58e0de9451d80b18d7c74caa7c1 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 0,
  "error": {
    "code": -32600,
    "message": "rejected due to project ID settings"
  }
}
```

---

## Solution: Set Valid Infura API Key

### Option 1: Get Your Own Infura API Key (Recommended)

1. **Sign up for Infura** (free tier available):
   - Go to: https://infura.io/
   - Create an account
   - Create a new project
   - Copy your Project ID

2. **Update Environment Variable**:

   **For docker-compose.yaml:**
   ```bash
   export ETHEREUM_RPC_URL="https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
   docker-compose up -d alkanes-data-api
   ```

   **For .env file:**
   ```env
   ETHEREUM_RPC_URL=https://mainnet.infura.io/v3/YOUR_PROJECT_ID
   ```

3. **Rebuild and restart**:
   ```bash
   docker-compose restart alkanes-data-api
   ```

### Option 2: Use Alternative Ethereum RPC Provider

You can use other Ethereum RPC providers:

**Alchemy:**
```env
ETHEREUM_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY
```

**QuickNode:**
```env
ETHEREUM_RPC_URL=https://YOUR_ENDPOINT.quiknode.pro/YOUR_TOKEN/
```

**Public Endpoints** (not recommended for production):
```env
ETHEREUM_RPC_URL=https://rpc.ankr.com/eth
# or
ETHEREUM_RPC_URL=https://eth.llamarpc.com
```

### Option 3: Disable Price Endpoints (Development Only)

If you don't need BTC price data, you can:

1. **Skip price endpoints** - All other endpoints work without Ethereum RPC
2. **Use mock data** - Return static price for development

---

## Affected Endpoints

Only these 4 endpoints require a valid Ethereum RPC:

- ❌ `POST /api/v1/get-bitcoin-price`
- ❌ `POST /api/v1/get-bitcoin-market-chart`
- ❌ `POST /api/v1/get-bitcoin-market-weekly`
- ❌ `POST /api/v1/get-bitcoin-markets`

**All other 39 endpoints work fine** without Ethereum RPC (they use PostgreSQL, Redis, and Sandshrew).

---

## Testing Price Endpoint

Once you have a valid API key:

```bash
# Test Ethereum connectivity
curl -X POST $ETHEREUM_RPC_URL \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Should return something like:
# {"jsonrpc":"2.0","id":1,"result":"0x13a7b2f"}

# Test BTC price endpoint
curl -X POST http://localhost:4000/api/v1/get-bitcoin-price \
  -H "Content-Type: application/json"

# Should return:
# {
#   "statusCode": 200,
#   "data": {
#     "bitcoin": {
#       "usd": 98765.43
#     }
#   }
# }
```

---

## Working Endpoints (No Infura Required)

All these endpoints are fully functional:

### Health
✅ `GET /api/v1/health`

### Alkanes (6 endpoints)
✅ All alkanes endpoints work

### Pools (7 endpoints)
✅ All pool endpoints work

### History (18 endpoints)
✅ All history endpoints work

### Bitcoin/UTXOs (7 endpoints)
✅ All Bitcoin endpoints work

**Total: 39 out of 43 endpoints work without Infura**

---

## Production Recommendations

### For Production Deployment:

1. **Use Paid Infura Plan** for better rate limits:
   - Free tier: 100,000 requests/day
   - Growth plan: 300,000 requests/day with better support

2. **Consider Multiple Providers** for redundancy:
   ```env
   ETHEREUM_RPC_URL_PRIMARY=https://mainnet.infura.io/v3/YOUR_KEY
   ETHEREUM_RPC_URL_BACKUP=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
   ```

3. **Monitor RPC Usage** to avoid rate limits

4. **Enable Caching** (already implemented):
   - Price is cached for 60 seconds
   - Reduces RPC calls significantly

---

## Summary

The alkanes-data-api is **fully functional** except for the 4 BTC price endpoints that require a valid Ethereum RPC endpoint. 

To fix:
1. Get a free Infura API key from https://infura.io/
2. Set `ETHEREUM_RPC_URL` environment variable
3. Restart the service

All other core functionality (alkanes, pools, history, bitcoin operations) works perfectly without any additional configuration.
