# Remaining Work for Full CLI Parity

## Current Status: 87+ / ~170 commands (~51%)

This document lists all remaining work to achieve full feature parity between `alkanes-bindgen-cli` (TypeScript) and `alkanes-cli` (Rust).

---

## ✅ COMPLETE Command Groups (100%)

### 1. Wallet (19/19)
All wallet commands implemented.

### 2. Bitcoind (18/18)
All Bitcoin RPC commands implemented.

### 3. Metashrew (4/4)
All commands: `height`, `state-root`, `getblockhash`, `view`

### 4. Lua (2/2)
All commands: `evalscript`, `eval`

### 5. BRC20-Prog (9/9)
All commands implemented including balance, code, block-number, chain-id, etc.

---

## 🟡 PARTIALLY COMPLETE Command Groups

### 6. Alkanes (11/20 = 55%)

**Implemented:**
- ✅ getbytecode
- ✅ balance (getbalance)
- ✅ trace
- ✅ inspect
- ✅ simulate
- ✅ unwrap
- ✅ get-all-pools
- ✅ all-pools-details
- ✅ reflect (reflect-alkane)
- ✅ by-address
- ✅ by-outpoint

**Missing (9 commands):**
- ❌ `execute` - Execute alkanes transaction (**CRITICAL** - needs tx building)
- ❌ `tx-script` - Execute tx-script with WASM bytecode
- ❌ `sequence` - Get sequence for outpoint
- ❌ `spendables` - Get spendable outpoints for address
- ❌ `traceblock` - Trace a block
- ❌ `backtest` - Backtest a transaction
- ❌ `pool-details` - Get details for specific pool
- ❌ `reflect-alkane-range` - Reflect metadata for range of alkanes
- ❌ `init-pool` - Initialize liquidity pool (**CRITICAL** - needs tx building)
- ❌ `swap` - Execute AMM swap (**CRITICAL** - needs tx building)

**Blockers:**
- Transaction building commands (execute, init-pool, swap) require WASM transaction construction capabilities
- Need to check if WASM bindings exist for: sequence, spendables, traceblock, backtest, pool-details, reflect-alkane-range

---

### 7. Esplora (11/31 = 35%)

**Implemented:**
- ✅ tx
- ✅ tx-status
- ✅ tx-hex
- ✅ address
- ✅ address-utxo (we call it address-utxos)
- ✅ address-txs
- ✅ address-txs-chain
- ✅ blocks-tip-height
- ✅ blocks-tip-hash
- ✅ fee-estimates
- ✅ broadcast (we call it broadcast-tx)

**Missing (20 commands):**
- ❌ `blocks` - Get recent blocks
- ❌ `block-height` - Get block at height
- ❌ `block` - Get block by hash
- ❌ `block-status` - Get block status
- ❌ `block-txids` - Get transaction IDs in block
- ❌ `block-header` - Get block header
- ❌ `block-raw` - Get raw block
- ❌ `block-txid` - Get transaction ID at index in block
- ❌ `block-txs` - Get transactions in block
- ❌ `address-txs-mempool` - Get mempool transactions for address
- ❌ `address-prefix` - Search addresses by prefix
- ❌ `tx-raw` - Get raw transaction
- ❌ `tx-merkle-proof` - Get merkle proof for transaction
- ❌ `tx-merkleblock-proof` - Get merkle block proof
- ❌ `tx-outspend` - Get outspend for transaction output
- ❌ `tx-outspends` - Get all outspends for transaction
- ❌ `post-tx` - Post transaction (alternative to broadcast)
- ❌ `mempool` - Get mempool info
- ❌ `mempool-txids` - Get mempool transaction IDs
- ❌ `mempool-recent` - Get recent mempool transactions

**Blockers:**
- Need to add WASM bindings for all missing Esplora endpoints

---

### 8. Ord (6/14 = 43%)

**Implemented:**
- ✅ inscription
- ✅ inscriptions (inscriptions-in-block)
- ✅ output (we call it outputs)
- ✅ rune
- ✅ list
- ✅ find

**Missing (8 commands):**
- ❌ `address-info` - Get address information
- ❌ `block-info` - Get block information
- ❌ `block-count` - Get latest block count
- ❌ `blocks` - Get latest blocks
- ❌ `children` - Get children of inscription
- ❌ `content` - Get inscription content
- ❌ `parents` - Get parents of inscription
- ❌ `sat` - Get sat information
- ❌ `tx-info` - Get transaction information

**Blockers:**
- Need to add WASM bindings for missing Ord endpoints

---

### 9. Dataapi (11/17 = 65%)

**Implemented:**
- ✅ get-pools (we call it pools)
- ✅ get-pool-history (we call it pool-history)
- ✅ get-swap-history (we call it trades)
- ✅ get-bitcoin-price (we call it bitcoin-price)
- ✅ get-market-chart (we call it bitcoin-market-chart)
- ✅ get-holders (we call it holders)
- ✅ get-holder-count (we call it holders-count)
- ✅ get-address-balances (we call it address-balances)
- ✅ get-alkanes-by-address (we call it alkanes-by-address)
- ✅ candles
- ✅ reserves

**Missing (6 commands):**
- ❌ `get-alkanes` - Get all alkanes
- ❌ `get-alkane-details` - Get alkane details
- ❌ `get-pool-by-id` - Get pool details by ID
- ❌ `health` - Health check
- ❌ `get-outpoint-balances` - Get balances for outpoint
- ❌ `get-block-height` - Get latest indexed block height
- ❌ `get-block-hash` - Get latest indexed block hash
- ❌ `get-indexer-position` - Get indexer position

**Blockers:**
- Need to add WASM bindings for missing Dataapi endpoints

---

### 10. ESPO (9/14 = 64%)

**Implemented:**
- ✅ height
- ✅ balances (we call it address-balances)
- ✅ outpoints (we call it address-outpoints)
- ✅ outpoint (we call it outpoint-balances)
- ✅ holders
- ✅ holders-count
- ✅ keys
- ✅ ping
- ✅ ammdata-ping

**Missing (5 commands):**
- ❌ `candles` - Get OHLCV candlestick data
- ❌ `trades` - Get trade history for pool
- ❌ `pools` - Get all pools with pagination
- ❌ `find-best-swap-path` - Find best swap path between tokens
- ❌ `get-best-mev-swap` - Find best MEV swap opportunity

**Blockers:**
- Need to add WASM bindings for missing ESPO endpoints

---

### 11. Runestone (1/2 = 50%)

**Implemented:**
- ✅ analyze

**Missing (1 command):**
- ❌ `trace` - Trace all protostones in runestone transaction

**Blockers:**
- We implemented `decode` which may not exist in Rust CLI
- Need to verify WASM binding exists for `trace`

---

### 12. Protorunes (0/2 = 0%)

**Implemented:**
- None (we mistakenly implemented decode/analyze which don't match Rust CLI)

**Missing (2 commands):**
- ❌ `by-address` - Get protorunes by address
- ❌ `by-outpoint` - Get protorunes by outpoint

**Blockers:**
- Need WASM bindings: `get_protorunes_by_address`, `get_protorunes_by_outpoint`
- These trait methods exist in `alkanes-cli-common` but not exposed in WASM

---

## ❌ NOT IMPLEMENTED Command Groups

### 13. Subfrost (0/1 = 0%)

**Missing (1 command):**
- ❌ `minimum-unwrap` - Calculate minimum unwrap amount

**Blockers:**
- Need WASM binding for subfrost minimum unwrap calculation

---

### 14. OPI (5/30+ = ~17%)

**Status:** Placeholder only

All OPI commands require direct HTTP requests to OPI endpoints and **cannot be implemented through current WASM bindings**.

**Recommendation:** Users should use the Rust `alkanes-cli` for OPI operations.

Commands affected:
- All BRC-20 indexer queries
- All Runes indexer queries
- All Bitmap indexer queries
- All POW20 indexer queries
- All SNS indexer queries

---

### 15. Decodepsbt (0/1 = 0%)

**Missing (1 command):**
- ❌ `decodepsbt` - Decode PSBT without bitcoind

**Note:** This is a standalone command, not part of a command group.

**Blockers:**
- Need WASM binding for PSBT decoding

---

## 🎯 Priority Implementation Plan

### Phase 1: Fix Incorrect Implementations (High Priority)

1. **Protorunes** - Replace decode/analyze with by-address/by-outpoint
   - Add WASM bindings: `protorunes_by_address_js`, `protorunes_by_outpoint_js`
   - Update commands in protorunes.ts

2. **Runestone** - Verify trace vs decode
   - Check if trace WASM binding exists
   - Remove decode if it doesn't match Rust CLI

### Phase 2: Complete High-Value Query Commands (Medium Priority)

3. **Esplora Block Commands** - Add remaining 20 commands
   - Most are simple query endpoints
   - High value for blockchain exploration

4. **Ord Commands** - Add remaining 8 commands
   - Valuable for inscriptions/ordinals work

5. **Dataapi Commands** - Add remaining 6 commands
   - Complete the analytics API

6. **ESPO Commands** - Add remaining 5 commands
   - AMM-specific queries (candles, trades, pools, swap paths)

### Phase 3: Complete Alkanes Query Commands (Medium Priority)

7. **Alkanes Non-Transaction Commands** - Add 6 commands
   - sequence, spendables, traceblock, backtest, pool-details, reflect-alkane-range
   - These don't require transaction building

### Phase 4: Transaction Building (High Complexity)

8. **Alkanes Transaction Commands** - Add 3 critical commands
   - execute, init-pool, swap
   - **Requires:** WASM transaction construction capabilities
   - **Blocker:** May need significant WASM bindings work

### Phase 5: Miscellaneous (Low Priority)

9. **Subfrost** - Add minimum-unwrap command
10. **Decodepsbt** - Add standalone command

### Phase 6: OPI (Deferred)

11. **OPI Commands** - NOT RECOMMENDED
    - Would require direct HTTP client in WASM
    - Users should use Rust CLI for OPI operations

---

## 📊 Summary

### By Implementation Effort:

**Easy (Just need WASM bindings):**
- Esplora: 20 commands
- Ord: 8 commands
- Dataapi: 6 commands
- ESPO: 5 commands
- Alkanes queries: 6 commands
- Subfrost: 1 command
- Decodepsbt: 1 command
- **Total: ~47 commands**

**Medium (Need trait methods + WASM bindings):**
- Protorunes: 2 commands
- Runestone: 1 command (trace)
- **Total: ~3 commands**

**Hard (Need transaction building):**
- Alkanes: execute, init-pool, swap
- **Total: 3 commands**

**Not Feasible:**
- OPI: ~25 commands (require HTTP client)

---

## 🔧 Next Steps

1. **Audit WASM bindings** - Check which provider trait methods are already exposed
2. **Add missing bindings** - Expose remaining methods in alkanes-web-sys
3. **Implement easy commands first** - Knock out the 47 simple query commands
4. **Fix protorunes/runestone** - Correct the mismatched implementations
5. **Defer transaction building** - Save execute/init-pool/swap for last (most complex)
6. **Skip OPI** - Document that users should use Rust CLI for OPI

**Target:** Reach 90%+ parity (~150/170 commands) without transaction building support.
