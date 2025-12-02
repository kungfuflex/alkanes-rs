# Data API Test Results

## ✅ FULLY WORKING Routes

### 1. get-swap-history
```bash
alkanes-cli dataapi get-swap-history --pool-id 2:3
```
**Status**: ✅ Perfect - Shows pool 2:3 (correct pool ID from factory-aware tracking!)

### 2. get-pool-history  
```bash
alkanes-cli dataapi get-pool-history 2:3
```
**Status**: ✅ Perfect - Alias for get-swap-history

### 3. get-pools
```bash
alkanes-cli dataapi get-pools
```
**Status**: ✅ Perfect - Lists all pools with reserves and LP supply

### 4. get-bitcoin-price
```bash
alkanes-cli dataapi get-bitcoin-price
```
**Status**: ✅ Perfect - Returns current BTC price

### 5. get-market-chart
```bash
alkanes-cli dataapi get-market-chart 7
```
**Status**: ✅ Perfect - Returns price chart data

### 6. health
```bash
alkanes-cli dataapi health
```
**Status**: ✅ Perfect - Returns "OK"

## ⚠️ ISSUES / EMPTY RESPONSES

### 7. get-alkanes
```bash
alkanes-cli dataapi get-alkanes
```
**Status**: ⚠️ Returns empty - "No tokens found"
**Issue**: Likely AlkaneBalance table is empty or query is incorrect

### 8. get-alkane-details
```bash
alkanes-cli dataapi get-alkane-details 2:0
```
**Status**: ❌ Parse error
**Issue**: API expects `{"alkaneId": {...}}` but CLI sends `{"id": "2:0"}`

### 9. get-pool-by-id
```bash
alkanes-cli dataapi get-pool-by-id 2:3
```
**Status**: ❌ Empty response
**Issue**: API returns empty, might be looking in wrong table

### 10. get-alkanes-by-address
```bash
alkanes-cli dataapi get-alkanes-by-address bcrt1qae0476c3a7fmla5nj09ee5g74wdup52adkwx2x
```
**Status**: ❌ 500 error - "Failed to get alkanes by address"
**Issue**: Backend error in service layer

## 🎯 KEY SUCCESS: Factory-Aware Pool Tracking

The most important achievement:
- **Pool ID in swaps**: Shows `2:3` (actual pool) NOT `4:65522` (factory)
- **Pool registry working**: Loads pools from database at startup
- **Dynamic discovery**: Tracks new pools created by factory

### Example Swap Output
```
💱 Swap History
════════════════════════════════════════════════════════════════════════════════

1. ✅ Swap #97ca98de-66e5-4a94-991b-136c25809ee1
   Pool: 2:3                              ← CORRECT!
   Trade: 2:0 → 32:0
   Amount: 300000000.0000 → 99900000.0000
   Price: 0.333000
   Block: Block #480
```

## Database Verification

```sql
SELECT pool_block, pool_tx FROM "TraceTrade" WHERE pool_block != 0;
```
**Result**: `pool_block=2, pool_tx=3` ✅

## Summary

**Working (6/10 routes):**
- All swap/pool history routes ✅
- Price/market data routes ✅
- Health check ✅

**Needs Fixes (4/10 routes):**
- get-alkanes (empty data)
- get-alkane-details (request format mismatch)
- get-pool-by-id (empty response)
- get-alkanes-by-address (500 error)

**Core Achievement:**
✅ **Factory-aware pool tracking fully operational!**
- Trades correctly attributed to pools (2:3)
- Not capturing factory/router (4:65522)
- Pool registry loads from database
- Dynamic pool discovery implemented
