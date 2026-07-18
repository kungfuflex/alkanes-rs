# Alkanes Data API - Quick Reference

Base URL: `http://localhost:3000/api/v1`

## Balance Endpoints

### GET Address Balances
```bash
curl -X POST http://localhost:3000/api/v1/get-address-balances \
  -H "Content-Type: application/json" \
  -d '{"address":"bc1p...", "include_outpoints":true}'
```

### GET Outpoint Balances
```bash
curl -X POST http://localhost:3000/api/v1/get-outpoint-balances \
  -H "Content-Type: application/json" \
  -d '{"outpoint":"txid:0"}'
```

### GET Holders
```bash
curl -X POST http://localhost:3000/api/v1/get-holders \
  -H "Content-Type: application/json" \
  -d '{"alkane":"840000:123", "page":1, "limit":100}'
```

### GET Holders Count
```bash
curl -X POST http://localhost:3000/api/v1/get-holders-count \
  -H "Content-Type: application/json" \
  -d '{"alkane":"840000:123"}'
```

### GET Address Outpoints
```bash
curl -X POST http://localhost:3000/api/v1/get-address-outpoints \
  -H "Content-Type: application/json" \
  -d '{"address":"bc1p..."}'
```

## Storage Endpoints

### GET Keys
```bash
curl -X POST http://localhost:3000/api/v1/get-keys \
  -H "Content-Type: application/json" \
  -d '{"alkane":"840000:123", "prefix":"reserve", "limit":100}'
```

## AMM Endpoints

### GET Trades
```bash
curl -X POST http://localhost:3000/api/v1/get-trades \
  -H "Content-Type: application/json" \
  -d '{"pool":"840000:456", "start_time":1704067200, "limit":100}'
```

### GET Candles
```bash
curl -X POST http://localhost:3000/api/v1/get-candles \
  -H "Content-Type: application/json" \
  -d '{"pool":"840000:456", "interval":"1h", "limit":500}'
```

### GET Reserves
```bash
curl -X POST http://localhost:3000/api/v1/get-reserves \
  -H "Content-Type: application/json" \
  -d '{"pool":"840000:456"}'
```

### Pathfind
```bash
curl -X POST http://localhost:3000/api/v1/pathfind \
  -H "Content-Type: application/json" \
  -d '{"token_in":"840000:100", "token_out":"840000:200", "amount_in":"1000", "max_hops":3}'
```

## Response Examples

### Address Balances Response
```json
{
  "ok": true,
  "address": "bc1p...",
  "balances": {
    "840000:123": "1000000",
    "840000:124": "500000"
  },
  "outpoints": [
    {
      "outpoint": "txid:0",
      "entries": [
        {"alkane": "840000:123", "amount": "500000"}
      ]
    }
  ]
}
```

### Holders Response
```json
{
  "ok": true,
  "alkane": "840000:123",
  "page": 1,
  "limit": 100,
  "total": 1250,
  "has_more": true,
  "items": [
    {"address": "bc1p...", "amount": "10000000"},
    {"address": "bc1q...", "amount": "5000000"}
  ]
}
```

### Trades Response
```json
{
  "ok": true,
  "pool": "840000:456",
  "trades": [
    {
      "txid": "abc123...",
      "vout": 0,
      "token0": "840000:100",
      "token1": "840000:200",
      "amount0_in": "1000",
      "amount1_in": "0",
      "amount0_out": "0",
      "amount1_out": "2000",
      "reserve0_after": "100000",
      "reserve1_after": "200000",
      "timestamp": "2024-01-01T00:00:00Z",
      "block_height": 840000
    }
  ]
}
```

### Candles Response
```json
{
  "ok": true,
  "pool": "840000:456",
  "interval": "1h",
  "candles": [
    {
      "open_time": "2024-01-01T00:00:00Z",
      "close_time": "2024-01-01T01:00:00Z",
      "open": "2.0",
      "high": "2.5",
      "low": "1.8",
      "close": "2.3",
      "volume0": "10000",
      "volume1": "20000",
      "trade_count": 15
    }
  ]
}
```

### Storage Keys Response
```json
{
  "ok": true,
  "alkane": "840000:123",
  "keys": {
    "reserve0": {
      "key": "reserve0",
      "value": "1000000",
      "last_txid": "abc123...",
      "last_vout": 0,
      "block_height": 840050,
      "updated_at": "2024-01-01T12:00:00Z"
    },
    "reserve1": {
      "key": "reserve1",
      "value": "2000000",
      "last_txid": "abc123...",
      "last_vout": 0,
      "block_height": 840050,
      "updated_at": "2024-01-01T12:00:00Z"
    }
  }
}
```

## Error Responses

```json
{
  "ok": false,
  "error": "invalid_alkane_format",
  "hint": "expected \"<block>:<tx>\""
}
```

```json
{
  "ok": false,
  "error": "internal_error"
}
```

## Rate Limiting

- Default: 100 requests/minute per IP
- Burst: 20 requests
- Headers: `X-RateLimit-Remaining`, `X-RateLimit-Reset`

## Pagination

For paginated endpoints (e.g., `get-holders`):
- `page`: Page number (1-indexed)
- `limit`: Items per page (max 1000)
- Response includes `has_more` boolean

## Timestamps

- All timestamps are Unix seconds (since epoch)
- Derived from Bitcoin block headers
- Response timestamps are ISO 8601 strings

## Intervals (for candles)

Supported: `1m`, `5m`, `15m`, `30m`, `1h`, `4h`, `1d`, `1w`

## Best Practices

1. **Use pagination** for large result sets
2. **Cache responses** when appropriate
3. **Specify time ranges** for trade/candle queries
4. **Use prefix search** for storage key queries
5. **Check `ok` field** in responses
