# Alkanes Data API - Endpoint Reference

Quick reference for all planned API endpoints matching espo functionality.

---

## Balance & Holder Endpoints

### 1. GET `/balance/address`
Get all alkane balances for a Bitcoin address.

**Query Parameters**:
- `address` (required): Bitcoin address
- `include_outpoints` (optional): Include UTXO-level breakdown

**Example**:
```bash
GET /balance/address?address=bc1q...&include_outpoints=true
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
  "outpoints": [
    {
      "outpoint": "txid:0",
      "entries": [
        {"alkane": "2:68441", "amount": "500000"}
      ]
    }
  ]
}
```

### 2. GET `/balance/outpoint`
Get alkane balances held by a specific UTXO.

**Query Parameters**:
- `outpoint` (required): `txid:vout` format

**Example**:
```bash
GET /balance/outpoint?outpoint=abc123...:0
```

**Response**:
```json
{
  "ok": true,
  "outpoint": "abc123...:0",
  "items": [{
    "outpoint": "abc123...:0",
    "address": "bc1q...",
    "entries": [
      {"alkane": "2:68441", "amount": "500000"}
    ]
  }]
}
```

### 3. GET `/balance/holders`
Get paginated list of holders for a specific alkane.

**Query Parameters**:
- `alkane` (required): `block:tx` format
- `page` (optional, default 1)
- `limit` (optional, default 100)

**Example**:
```bash
GET /balance/holders?alkane=2:68441&page=1&limit=50
```

**Response**:
```json
{
  "ok": true,
  "alkane": "2:68441",
  "page": 1,
  "limit": 50,
  "total": 1523,
  "has_more": true,
  "items": [
    {"address": "bc1q...", "amount": "1000000"},
    {"address": "bc1q...", "amount": "500000"}
  ]
}
```

### 4. GET `/balance/holders/count`
Get total number of unique holders for an alkane.

**Query Parameters**:
- `alkane` (required): `block:tx` format

**Example**:
```bash
GET /balance/holders/count?alkane=2:68441
```

**Response**:
```json
{
  "ok": true,
  "alkane": "2:68441",
  "count": 1523
}
```

### 5. GET `/balance/address/outpoints`
Get all UTXOs with alkane balances for an address.

**Query Parameters**:
- `address` (required): Bitcoin address

**Example**:
```bash
GET /balance/address/outpoints?address=bc1q...
```

**Response**:
```json
{
  "ok": true,
  "address": "bc1q...",
  "outpoints": [
    {
      "outpoint": "txid:0",
      "entries": [
        {"alkane": "2:68441", "amount": "500000"}
      ]
    }
  ]
}
```

---

## Storage Endpoints

### 6. GET `/storage/keys`
Get contract storage key-value pairs for an alkane.

**Query Parameters**:
- `alkane` (required): `block:tx` format
- `keys` (optional): Specific keys to fetch (comma-separated)
- `page` (optional, default 1)
- `limit` (optional, default 100)
- `try_decode_utf8` (optional, default true)

**Example**:
```bash
GET /storage/keys?alkane=2:68441&keys=0x1234,mykey&limit=50
```

**Response**:
```json
{
  "ok": true,
  "alkane": "2:68441",
  "page": 1,
  "limit": 50,
  "total": 25,
  "has_more": false,
  "items": {
    "mykey": {
      "key_hex": "0x6d796b6579",
      "key_str": "mykey",
      "value_hex": "0x0a00000000000000",
      "value_str": null,
      "value_u128": "10",
      "last_txid": "abc123..."
    }
  }
}
```

---

## AMM/DEX Endpoints

### 7. GET `/amm/candles`
Get OHLCV candle data for a trading pair.

**Query Parameters**:
- `pool` (required): Pool alkane ID `block:tx`
- `timeframe` (optional, default "1h"): 10m, 1h, 1d, 1w, 1M
- `side` (optional, default "base"): base or quote
- `page` (optional, default 1)
- `limit` (optional, default 120)
- `now` (optional): Timestamp for relative queries

**Example**:
```bash
GET /amm/candles?pool=2:68441&timeframe=1h&side=base&limit=100
```

**Response**:
```json
{
  "ok": true,
  "pool": "2:68441",
  "timeframe": "1h",
  "side": "base",
  "page": 1,
  "limit": 100,
  "total": 500,
  "has_more": true,
  "candles": [
    {
      "ts": 1700000000,
      "open": 1.234,
      "high": 1.456,
      "low": 1.123,
      "close": 1.345,
      "volume": 100000.0
    }
  ]
}
```

### 8. GET `/amm/trades`
Get historical trade data with sorting/filtering.

**Query Parameters**:
- `pool` (required): Pool alkane ID
- `side` (optional, default "base"): Price side
- `filter_side` (optional, default "all"): all, buy, sell
- `sort` (optional, default "ts"): ts, amount, side
- `dir` (optional, default "desc"): asc, desc
- `page` (optional, default 1)
- `limit` (optional, default 50)

**Example**:
```bash
GET /amm/trades?pool=2:68441&filter_side=buy&sort=amount&dir=desc
```

**Response**:
```json
{
  "ok": true,
  "pool": "2:68441",
  "side": "base",
  "filter_side": "buy",
  "sort": "amount",
  "dir": "desc",
  "page": 1,
  "limit": 50,
  "total": 5000,
  "has_more": true,
  "trades": [
    {
      "ts": 1700000000,
      "side": "buy",
      "amount_base": "1000000",
      "amount_quote": "2000000",
      "price": 2.0
    }
  ]
}
```

### 9. GET `/amm/pools`
Get all pools with live reserves and metadata.

**Query Parameters**:
- `page` (optional, default 1)
- `limit` (optional, default all)

**Example**:
```bash
GET /amm/pools?limit=50
```

**Response**:
```json
{
  "ok": true,
  "page": 1,
  "limit": 50,
  "total": 125,
  "has_more": true,
  "pools": {
    "2:68441": {
      "base": "2:1",
      "quote": "2:2",
      "base_reserve": "1000000000",
      "quote_reserve": "2000000000",
      "source": "live"
    }
  }
}
```

### 10. POST `/amm/find_best_swap_path`
Find best multi-hop swap route.

**Request Body**:
```json
{
  "mode": "exact_in",
  "token_in": "2:1",
  "token_out": "2:2",
  "amount_in": "1000000",
  "amount_out_min": "1900000",
  "fee_bps": 30,
  "max_hops": 3
}
```

**Response**:
```json
{
  "ok": true,
  "mode": "exact_in",
  "token_in": "2:1",
  "token_out": "2:2",
  "fee_bps": 30,
  "max_hops": 3,
  "amount_in": "1000000",
  "amount_out": "1950000",
  "hops": [
    {
      "pool": "2:68441",
      "token_in": "2:1",
      "token_out": "2:3",
      "amount_in": "1000000",
      "amount_out": "1500000"
    },
    {
      "pool": "2:68442",
      "token_in": "2:3",
      "token_out": "2:2",
      "amount_in": "1500000",
      "amount_out": "1950000"
    }
  ]
}
```

**Modes**:
- `exact_in`: Specify input amount, get max output
- `exact_out`: Specify output amount, get min input needed
- `implicit`: Available amount, route optimally

### 11. POST `/amm/get_best_mev_swap`
Find best arbitrage cycle (MEV opportunity).

**Request Body**:
```json
{
  "token": "2:1",
  "fee_bps": 30,
  "max_hops": 4
}
```

**Response**:
```json
{
  "ok": true,
  "token": "2:1",
  "fee_bps": 30,
  "max_hops": 4,
  "amount_in": "1000000",
  "amount_out": "1050000",
  "profit": "50000",
  "hops": [
    {
      "pool": "2:68441",
      "token_in": "2:1",
      "token_out": "2:2",
      "amount_in": "1000000",
      "amount_out": "950000"
    },
    {
      "pool": "2:68442",
      "token_in": "2:2",
      "token_out": "2:1",
      "amount_in": "950000",
      "amount_out": "1050000"
    }
  ]
}
```

---

## Health & Utility Endpoints

### GET `/health`
Server health check.

**Response**:
```json
{
  "status": "ok",
  "version": "1.0.0",
  "uptime": 3600,
  "database": "connected"
}
```

### GET `/ping`
Simple ping endpoint.

**Response**:
```json
"pong"
```

---

## Error Responses

All endpoints return errors in this format:

```json
{
  "ok": false,
  "error": "error_code",
  "hint": "Human-readable description"
}
```

**Common Error Codes**:
- `missing_or_invalid_address`
- `missing_or_invalid_alkane`
- `missing_or_invalid_outpoint`
- `invalid_format`
- `not_found`
- `internal_error`
- `no_path_found`
- `no_liquidity`

---

## Rate Limiting

- Default: 100 requests/minute per IP
- Authenticated: 1000 requests/minute
- Burst allowance: 20 requests

**Headers**:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1700000060
```

---

## Pagination

All list endpoints support pagination:

**Parameters**:
- `page`: Page number (1-indexed)
- `limit`: Items per page (default varies, max 1000)

**Response includes**:
- `page`: Current page
- `limit`: Items per page
- `total`: Total items
- `has_more`: Boolean indicating more pages

---

## Authentication (Optional)

If API keys are required:

**Header**:
```
Authorization: Bearer YOUR_API_KEY
```

**Response** (unauthorized):
```json
{
  "ok": false,
  "error": "unauthorized",
  "hint": "API key required"
}
```

---

## Base URL

Development: `http://localhost:3000`  
Staging: `https://staging-api.alkanes.io`  
Production: `https://api.alkanes.io`

---

## Client Libraries

### Curl Examples

```bash
# Get address balances
curl "https://api.alkanes.io/balance/address?address=bc1q..."

# Get holders with pagination
curl "https://api.alkanes.io/balance/holders?alkane=2:68441&page=1&limit=100"

# Find swap path
curl -X POST "https://api.alkanes.io/amm/find_best_swap_path" \
  -H "Content-Type: application/json" \
  -d '{
    "mode": "exact_in",
    "token_in": "2:1",
    "token_out": "2:2",
    "amount_in": "1000000"
  }'
```

### JavaScript/TypeScript

```typescript
// Using fetch
const response = await fetch('https://api.alkanes.io/balance/address?address=bc1q...');
const data = await response.json();

// Using axios
const { data } = await axios.get('https://api.alkanes.io/balance/address', {
  params: { address: 'bc1q...' }
});
```

### Python

```python
import requests

# Get address balances
response = requests.get('https://api.alkanes.io/balance/address', 
    params={'address': 'bc1q...'})
data = response.json()

# Find swap path
response = requests.post('https://api.alkanes.io/amm/find_best_swap_path',
    json={
        'mode': 'exact_in',
        'token_in': '2:1',
        'token_out': '2:2',
        'amount_in': '1000000'
    })
path = response.json()
```

---

## WebSocket API (Future)

Real-time updates via WebSocket:

```javascript
const ws = new WebSocket('wss://api.alkanes.io/ws');

// Subscribe to balance updates
ws.send(JSON.stringify({
  action: 'subscribe',
  channel: 'balance',
  address: 'bc1q...'
}));

// Subscribe to trades
ws.send(JSON.stringify({
  action: 'subscribe',
  channel: 'trades',
  pool: '2:68441'
}));
```

---

## Versioning

API uses URL versioning (future):

- `/v1/balance/address` - Version 1
- `/v2/balance/address` - Version 2

Current endpoints are implicitly v1.

---

## Support

- Documentation: https://docs.alkanes.io
- GitHub Issues: https://github.com/kungfuflex/alkanes-rs/issues
- Discord: https://discord.gg/alkanes
