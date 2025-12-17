# alkanes-bindgen-cli Implementation Status

## Current Achievement: 100/170 commands (59%)

### 🎉 Fully Complete Command Groups (9 groups at 100%):

1. ✅ **Wallet** (19/19) - 100%
2. ✅ **Bitcoind** (18/18) - 100%
3. ✅ **Metashrew** (4/4) - 100%
4. ✅ **Lua** (2/2) - 100%
5. ✅ **BRC20-Prog** (9/9) - 100%
6. ✅ **Protorunes** (2/2) - 100% - JUST FIXED
7. ✅ **ESPO** (14/14) - 100% - JUST COMPLETED
8. ✅ **Runestone** (3/3) - 100% - JUST COMPLETED
9. ✅ **Decodepsbt** (1/1) - 100% - JUST ADDED

### 🟡 Partially Complete (4 groups):

10. **Alkanes** (12/20 = 60%) - Added traceblock
   - ✅ getbytecode, balance, trace, inspect, simulate, unwrap
   - ✅ get-all-pools, all-pools-details, reflect
   - ✅ by-address, by-outpoint
   - ✅ traceblock (JUST ADDED)
   - ❌ Missing: execute, wrap-btc, init-pool, swap (tx building)
   - ❌ Missing: sequence, spendables, backtest, pool-details, reflect-alkane-range

11. **Esplora** (11/31 = 35%)
   - ✅ tx, tx-status, tx-hex, address, address-utxos, address-txs, address-txs-chain
   - ✅ blocks-tip-height, blocks-tip-hash, fee-estimates, broadcast
   - ❌ Missing 20: See "Missing WASM Bindings" section below

12. **Ord** (6/14 = 43%)
   - ✅ inscription, inscriptions, outputs, rune, list, find
   - ❌ Missing 8: See "Missing WASM Bindings" section below

13. **Dataapi** (11/17 = 65%)
   - ✅ pools, pool-history, trades, candles, reserves
   - ✅ holders, holders-count, bitcoin-price, bitcoin-market-chart
   - ✅ address-balances, alkanes-by-address
   - ❌ Missing 6: See "Missing WASM Bindings" section below

### ⚠️ Placeholder Only (2 groups):

14. **Subfrost** (1/1) - Placeholder created, needs WASM binding for minimum-unwrap
15. **OPI** (5/30) - Placeholder only (requires HTTP client, not feasible via WASM)

---

## 📋 Missing WASM Bindings Needed

To reach **~150/170 commands (88% parity)**, we need to add WASM bindings for these commands:

### Esplora (20 commands):

**Block Operations (9 commands):**
- `blocks` - Get blocks starting from height
- `block-height` - Get block by height
- `block` - Get block by hash
- `block-status` - Get block status
- `block-txids` - Get transaction IDs in block
- `block-header` - Get block header
- `block-raw` - Get raw block data
- `block-txid` - Get transaction ID by index
- `block-txs` - Get block transactions

**Address Operations (2 commands):**
- `address-txs-mempool` - Get mempool transactions for address
- `address-prefix` - Search addresses by prefix

**Transaction Operations (5 commands):**
- `tx-raw` - Get raw transaction
- `tx-merkle-proof` - Get merkle proof
- `tx-merkleblock-proof` - Get merkle block proof
- `tx-outspend` - Get outspend for output
- `tx-outspends` - Get all outspends

**Mempool Operations (3 commands):**
- `mempool` - Get mempool info
- `mempool-txids` - Get mempool transaction IDs
- `mempool-recent` - Get recent mempool transactions

**Other (1 command):**
- `post-tx` - Post transaction (alternative to broadcast)

### Ord (8 commands):

- `address-info` - Get address information
- `block-info` - Get block information
- `block-count` - Get latest block count
- `blocks` - Get latest blocks
- `children` - Get children of inscription
- `content` - Get inscription content
- `parents` - Get parents of inscription
- `sat` - Get sat information
- `tx-info` - Get transaction information

### Alkanes (5 query commands):

- `sequence` - Get sequence for outpoint
- `spendables` - Get spendable outpoints for address
- `backtest` - Backtest a transaction
- `pool-details` - Get details for specific pool
- `reflect-alkane-range` - Reflect metadata for range of alkanes

### Dataapi (6 commands):

- `get-alkanes` - Get all alkanes
- `get-alkane-details` - Get alkane details
- `get-pool-by-id` - Get pool details by ID
- `health` - Health check
- `get-outpoint-balances` - Get balances for outpoint
- `get-block-height` - Get latest indexed block height
- `get-block-hash` - Get latest indexed block hash
- `get-indexer-position` - Get indexer position

**Total: ~39 commands need WASM bindings**

---

## 🚧 Deferred Commands (Will NOT Implement)

### Transaction Building Commands (4 commands):
These require complex transaction construction logic in WASM:
- `alkanes execute` - Execute alkanes transaction
- `alkanes wrap-btc` - Wrap BTC to frBTC
- `alkanes init-pool` - Initialize liquidity pool
- `alkanes swap` - Execute AMM swap

**Status:** Deferred - Requires significant WASM transaction building infrastructure

### OPI Commands (~25 commands):
These require direct HTTP requests to OPI endpoints:
- All BRC-20, Runes, Bitmap, POW20, SNS indexer queries

**Status:** Not feasible via WASM - Users should use Rust alkanes-cli

**Total Deferred: ~29 commands**

---

## 📊 Path to 88% Parity (150/170 commands)

### Phase 1: Add Esplora WASM Bindings (20 commands)
File: `/data/alkanes-rs/crates/alkanes-web-sys/src/provider.rs`

Add methods like:
```rust
#[wasm_bindgen(js_name = esploraGetBlocks)]
pub fn esplora_get_blocks_js(&self, start_height: Option<u64>) -> js_sys::Promise {
    // Implementation
}
```

Then implement CLI commands in:
File: `/data/alkanes-rs/ts-sdk/src/cli/commands/esplora.ts`

### Phase 2: Add Ord WASM Bindings (8 commands)
File: `/data/alkanes-rs/crates/alkanes-web-sys/src/provider.rs`

Add methods for address-info, block-info, children, content, etc.

Then implement CLI commands in:
File: `/data/alkanes-rs/ts-sdk/src/cli/commands/ord.ts`

### Phase 3: Add Remaining WASM Bindings (11 commands)
- Alkanes: 5 commands
- Dataapi: 6 commands

---

## 🎯 Summary

### Current Status:
- **100/170 commands implemented (59%)**
- **9 command groups at 100%**
- **Significant progress on ESPO, Protorunes, Runestone**

### To Reach 88% Parity:
- Add **~39 WASM bindings** for simple query commands
- Implement corresponding CLI commands
- Skip transaction building and OPI commands

### Estimated Work:
- **Esplora bindings:** ~4-6 hours (20 commands, similar patterns)
- **Ord bindings:** ~2-3 hours (8 commands)
- **Alkanes/Dataapi bindings:** ~2-3 hours (11 commands)
- **CLI implementation:** ~2-3 hours (all 39 commands follow same pattern)

**Total:** ~10-15 hours to reach 88% parity

---

## ✅ Immediate Next Steps

1. **Start with Esplora** - Largest group, most impactful
2. **Add provider methods** to `alkanes-web-sys/src/provider.rs`
3. **Rebuild WASM** with `wasm-pack build`
4. **Add CLI commands** to respective files
5. **Test each group** before moving to next
6. **Update progress** after each phase

---

## 🔄 Build & Test Commands

```bash
# Rebuild WASM
cd crates/alkanes-web-sys
wasm-pack build --target bundler

# Copy to ts-sdk
cd ../../ts-sdk
npm run build:wasm

# Build CLI
pnpm build:cli

# Test commands
node dist/index.js esplora --help
node dist/index.js ord --help
```
