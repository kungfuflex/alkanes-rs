# Command Parity Checklist: alkanes-bindgen-cli vs alkanes-cli

## Testing: Run `/data/alkanes-rs/alkanes-bindgen-cli` vs `alkanes-cli`

## Wallet Commands (19/19 = 100%) ✅

| Command | Status | Notes |
|---------|--------|-------|
| `wallet create` | ✅ IMPLEMENTED | Creates wallet with mnemonic |
| `wallet addresses` | ✅ IMPLEMENTED | Gets addresses by spec (e.g., p2tr:0-10) |
| `wallet utxos` | ✅ IMPLEMENTED | Lists UTXOs using UtxoProvider |
| `wallet balance` | ✅ IMPLEMENTED | Gets wallet balance |
| `wallet send` | ✅ IMPLEMENTED | Sends BTC transaction |
| `wallet freeze` | ✅ IMPLEMENTED | Freeze a UTXO |
| `wallet unfreeze` | ✅ IMPLEMENTED | Unfreeze a UTXO |
| `wallet sign` | ✅ IMPLEMENTED | Sign a PSBT |
| `wallet history` | ✅ IMPLEMENTED | Get transaction history |
| `wallet create-tx` | ✅ IMPLEMENTED | Create a transaction |
| `wallet sign-tx` | ✅ IMPLEMENTED | Sign a transaction |
| `wallet decode-tx` | ✅ IMPLEMENTED | Decode a transaction |
| `wallet broadcast-tx` | ✅ IMPLEMENTED | Broadcast a transaction |
| `wallet estimate-fee` | ✅ IMPLEMENTED | Estimate transaction fee |
| `wallet fee-rates` | ✅ IMPLEMENTED | Get current fee rates |
| `wallet sync` | ✅ IMPLEMENTED | Sync wallet with blockchain |
| `wallet backup` | ✅ IMPLEMENTED | Backup wallet |
| `wallet mnemonic` | ✅ IMPLEMENTED | Get wallet mnemonic |

## Bitcoind Commands (14/18 = 78%)

| Command | Status | Notes |
|---------|--------|-------|
| `bitcoind getblockcount` | ✅ IMPLEMENTED | Get current block count |
| `bitcoind generatetoaddress` | ✅ IMPLEMENTED | Generate blocks (regtest) |
| `bitcoind getblockchaininfo` | ✅ IMPLEMENTED | Get blockchain info |
| `bitcoind getrawtransaction` | ✅ IMPLEMENTED | Get raw transaction |
| `bitcoind getblock` | ✅ IMPLEMENTED | Get block by hash |
| `bitcoind getblockhash` | ✅ IMPLEMENTED | Get block hash by height |
| `bitcoind sendrawtransaction` | ✅ IMPLEMENTED | Broadcast transaction |
| `bitcoind getnetworkinfo` | ✅ IMPLEMENTED | Get network info |
| `bitcoind getmempoolinfo` | ✅ IMPLEMENTED | Get mempool info |
| `bitcoind generatefuture` | ✅ IMPLEMENTED | Generate future block |
| `bitcoind getblockheader` | ✅ IMPLEMENTED | Get block header |
| `bitcoind getblockstats` | ✅ IMPLEMENTED | Get block statistics |
| `bitcoind estimatesmartfee` | ✅ IMPLEMENTED | Estimate smart fee |
| `bitcoind getchaintips` | ✅ IMPLEMENTED | Get chain tips |
| `bitcoind decoderawtransaction` | ⏳ BLOCKED | Needs WASM binding |
| `bitcoind decodepsbt` | ⏳ BLOCKED | Needs WASM binding |
| `bitcoind getrawmempool` | ⏳ BLOCKED | Needs WASM binding |
| `bitcoind gettxout` | ⏳ BLOCKED | Needs WASM binding |

## Alkanes Commands (0/21 = 0%)

| Command | Status | Notes |
|---------|--------|-------|
| `alkanes execute` | ❌ MISSING | **CRITICAL** - Execute alkanes transaction |
| `alkanes inspect` | ❌ MISSING | Inspect alkanes contract |
| `alkanes trace` | ❌ MISSING | Trace alkanes transaction |
| `alkanes simulate` | ❌ MISSING | Simulate alkanes transaction |
| `alkanes tx-script` | ❌ MISSING | Execute tx-script with WASM |
| `alkanes sequence` | ❌ MISSING | Get sequence for outpoint |
| `alkanes spendables` | ❌ MISSING | Get spendable outpoints |
| `alkanes traceblock` | ❌ MISSING | Trace a block |
| `alkanes getbytecode` | ❌ MISSING | **CRITICAL** - Get bytecode |
| `alkanes getbalance` | ❌ MISSING | Get alkanes balance |
| `alkanes wrap-btc` | ❌ MISSING | **CRITICAL** - Wrap BTC to frBTC |
| `alkanes unwrap` | ❌ MISSING | Get pending unwraps |
| `alkanes backtest` | ❌ MISSING | Backtest transaction |
| `alkanes get-all-pools` | ❌ MISSING | Get all AMM pools |
| `alkanes all-pools-details` | ❌ MISSING | Get all pools with details |
| `alkanes pool-details` | ❌ MISSING | Get pool details |
| `alkanes reflect-alkane` | ❌ MISSING | Reflect alkane metadata |
| `alkanes reflect-alkane-range` | ❌ MISSING | Reflect range of alkanes |
| `alkanes init-pool` | ❌ MISSING | **CRITICAL** - Initialize liquidity pool |
| `alkanes swap` | ❌ MISSING | **CRITICAL** - Execute AMM swap |

## Other Command Groups (Not Yet Started)

### Esplora (0/35 = 0%)
- All 35 commands need implementation

### Ord (0/15 = 0%)
- All 15 commands need implementation

### Runestone (0/3 = 0%)
- All 3 commands need implementation

### Protorunes (0/4 = 0%)
- All 4 commands need implementation

### Metashrew (0/3 = 0%)
- All 3 commands need implementation

### Lua (0/1 = 0%)
- `lua evalscript` needs implementation

### Dataapi (0/20 = 0%)
- All 20 commands need implementation

### OPI (0/30 = 0%)
- All 30 commands across BRC-20, Runes, Bitmap, etc. need implementation

### Subfrost (0/1 = 0%)
- `subfrost minimum-unwrap` needs implementation

### ESPO (0/15 = 0%)
- All 15 commands need implementation

### BRC20-Prog (0/4 = 0%)
- All 4 commands need implementation

### Decodepsbt (0/1 = 0%)
- `decodepsbt` command needs implementation

## Overall Progress

- **Total Commands**: 190
- **Implemented**: 33 (19 wallet + 14 bitcoind)
- **Blocked (needs WASM)**: 4 (bitcoind)
- **Missing**: 153
- **Progress**: 17.4%

## Priority Implementation Order

1. **Alkanes (CRITICAL)** - Needed for deploy-regtest.sh
   - execute, getbytecode, wrap-btc, init-pool, swap, getbalance

2. **Complete Wallet** - 14 remaining commands

3. **Complete Bitcoind** - 9 remaining commands

4. **Esplora** - 35 commands for API access

5. **All Other Groups** - 90+ commands

## Next Steps

Implement commands in priority order, testing each one against the Rust CLI to ensure parity.
