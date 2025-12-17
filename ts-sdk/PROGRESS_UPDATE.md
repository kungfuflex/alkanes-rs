# CLI Implementation Progress Update

## Summary of Recent Improvements

### Commands Fixed:
1. **Protorunes** (2/2 = 100%) ✅ - FIXED
   - Replaced incorrect decode/analyze with correct by-address/by-outpoint commands
   
2. **Runestone** (3/2 = 150%) ✅ - COMPLETE
   - Added trace command
   - Now has: decode, analyze, trace

3. **ESPO** (14/14 = 100%) ✅ - COMPLETE
   - Added candles, trades, pools, find-best-swap-path, get-best-mev-swap
   - All ESPO commands now implemented!

4. **Alkanes** (12/20 = 60%) - IMPROVED
   - Added traceblock command

5. **Subfrost** (1/1 = 100%) ⚠️ - PLACEHOLDER
   - Created command structure (implementation pending WASM bindings)

6. **Decodepsbt** (1/1 = 100%) ✅ - COMPLETE
   - Added standalone command

## New Overall Status: ~100/170 commands (59%)

### Fully Complete Groups (100%):
1. Wallet - 19/19
2. Bitcoind - 18/18  
3. Metashrew - 4/4
4. Lua - 2/2
5. BRC20-Prog - 9/9
6. Protorunes - 2/2 ✅ NEW
7. ESPO - 14/14 ✅ NEW
8. Runestone - 3/3 ✅ NEW
9. Decodepsbt - 1/1 ✅ NEW

### Partially Complete:
10. Alkanes - 12/20 (60%)
11. Esplora - 11/31 (35%)
12. Ord - 6/14 (43%)
13. Dataapi - 11/17 (65%)

### Placeholder:
14. Subfrost - 1/1 (placeholder)
15. OPI - 5/30 (placeholder)

## Next Priority: Add Missing Commands with Existing WASM Bindings

The following commands need NEW WASM bindings to be added to alkanes-web-sys:

### Esplora (~20 missing commands):
- Block operations: blocks, block-height, block, block-status, block-txids, block-header, block-raw, block-txid, block-txs
- Address: address-txs-mempool, address-prefix
- Transaction: tx-raw, tx-merkle-proof, tx-merkleblock-proof, tx-outspend, tx-outspends
- Mempool: mempool, mempool-txids, mempool-recent
- Other: post-tx

### Ord (~8 missing commands):
- address-info, block-info, block-count, blocks
- children, content, parents
- sat, tx-info

### Alkanes (~8 missing commands):
- Query commands: sequence, spendables, backtest, pool-details, reflect-alkane-range
- Transaction building: execute, wrap-btc, init-pool, swap

### Dataapi (~6 missing commands):
- get-alkanes, get-alkane-details, get-pool-by-id
- health, get-outpoint-balances
- get-block-height, get-block-hash, get-indexer-position

## Implementation Strategy

To reach ~150/170 commands (88% parity), we need to:

1. **Add WASM bindings** for ~50 simple query commands
2. **Skip for now:** Transaction building commands (execute, wrap-btc, init-pool, swap)
3. **Skip for now:** OPI commands (require HTTP client)

This will give us near-complete parity for all read-only query operations!
