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

## Bitcoind Commands (18/18 = 100%) ✅

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
| `bitcoind decoderawtransaction` | ✅ IMPLEMENTED | Decode raw transaction |
| `bitcoind decodepsbt` | ✅ IMPLEMENTED | Decode PSBT |
| `bitcoind getrawmempool` | ✅ IMPLEMENTED | Get raw mempool |
| `bitcoind gettxout` | ✅ IMPLEMENTED | Get transaction output |

## Alkanes Commands (11/21 = 52%)

| Command | Status | Notes |
|---------|--------|-------|
| `alkanes execute` | ❌ MISSING | **CRITICAL** - Execute alkanes transaction (needs tx building) |
| `alkanes inspect` | ✅ IMPLEMENTED | Inspect alkanes contract |
| `alkanes trace` | ✅ IMPLEMENTED | Trace alkanes transaction |
| `alkanes simulate` | ✅ IMPLEMENTED | Simulate alkanes transaction |
| `alkanes tx-script` | ❌ MISSING | Execute tx-script with WASM |
| `alkanes sequence` | ❌ MISSING | Get sequence for outpoint |
| `alkanes spendables` | ❌ MISSING | Get spendable outpoints |
| `alkanes traceblock` | ❌ MISSING | Trace a block |
| `alkanes getbytecode` | ✅ IMPLEMENTED | Get bytecode |
| `alkanes getbalance` | ✅ IMPLEMENTED | Get alkanes balance (alias: balance) |
| `alkanes wrap-btc` | ❌ MISSING | **CRITICAL** - Wrap BTC to frBTC (needs tx building) |
| `alkanes unwrap` | ✅ IMPLEMENTED | Get pending unwraps |
| `alkanes backtest` | ❌ MISSING | Backtest transaction |
| `alkanes get-all-pools` | ✅ IMPLEMENTED | Get all AMM pools |
| `alkanes all-pools-details` | ✅ IMPLEMENTED | Get all pools with details |
| `alkanes pool-details` | ❌ MISSING | Get pool details |
| `alkanes reflect` | ✅ IMPLEMENTED | Reflect alkane metadata |
| `alkanes reflect-alkane-range` | ❌ MISSING | Reflect range of alkanes |
| `alkanes init-pool` | ❌ MISSING | **CRITICAL** - Initialize liquidity pool (needs tx building) |
| `alkanes swap` | ❌ MISSING | **CRITICAL** - Execute AMM swap (needs tx building) |
| `alkanes by-address` | ✅ IMPLEMENTED | Get alkanes by address |
| `alkanes by-outpoint` | ✅ IMPLEMENTED | Get alkanes by outpoint |

## Esplora Commands (11/35 = 31%)

| Command | Status | Notes |
|---------|--------|-------|
| `esplora tx` | ✅ IMPLEMENTED | Get transaction by hash |
| `esplora tx-status` | ✅ IMPLEMENTED | Get transaction confirmation status |
| `esplora address` | ✅ IMPLEMENTED | Get address information |
| `esplora address-utxos` | ✅ IMPLEMENTED | Get address UTXOs |
| `esplora address-txs` | ✅ IMPLEMENTED | Get address transactions |
| `esplora address-txs-chain` | ✅ IMPLEMENTED | Get confirmed transactions (paginated) |
| `esplora blocks-tip-height` | ✅ IMPLEMENTED | Get current tip height |
| `esplora blocks-tip-hash` | ✅ IMPLEMENTED | Get current tip hash |
| `esplora fee-estimates` | ✅ IMPLEMENTED | Get fee estimates |
| `esplora broadcast-tx` | ✅ IMPLEMENTED | Broadcast transaction |
| `esplora tx-hex` | ✅ IMPLEMENTED | Get transaction as hex |
| (24+ more commands) | ❌ MISSING | Additional Esplora endpoints |

## Ord Commands (6/15 = 40%)

| Command | Status | Notes |
|---------|--------|-------|
| `ord inscription` | ✅ IMPLEMENTED | Get inscription by ID |
| `ord inscriptions` | ✅ IMPLEMENTED | Get inscriptions in block range |
| `ord outputs` | ✅ IMPLEMENTED | Get outputs for inscription |
| `ord rune` | ✅ IMPLEMENTED | Get rune information |
| `ord list` | ✅ IMPLEMENTED | List inscriptions |
| `ord find` | ✅ IMPLEMENTED | Find inscription by satpoint |
| (9+ more commands) | ❌ MISSING | Additional Ord endpoints |

## Runestone Commands (2/3 = 67%)

| Command | Status | Notes |
|---------|--------|-------|
| `runestone decode` | ✅ IMPLEMENTED | Decode runestone from transaction |
| `runestone analyze` | ✅ IMPLEMENTED | Analyze runestone structure |
| (1 more command) | ❌ MISSING | Additional Runestone functionality |

## Protorunes Commands (2/4 = 50%)

| Command | Status | Notes |
|---------|--------|-------|
| `protorunes decode` | ✅ IMPLEMENTED | Decode protorune from transaction |
| `protorunes analyze` | ✅ IMPLEMENTED | Analyze protorune structure |
| (2 more commands) | ❌ MISSING | Additional Protorunes functionality |

## Metashrew Commands (4/3 = 133%) ✅

| Command | Status | Notes |
|---------|--------|-------|
| `metashrew height` | ✅ IMPLEMENTED | Get current Metashrew height |
| `metashrew state-root` | ✅ IMPLEMENTED | Get state root at height |
| `metashrew getblockhash` | ✅ IMPLEMENTED | Get block hash at height |
| `metashrew view` | ✅ IMPLEMENTED | View state data |

## Lua Commands (2/1 = 200%) ✅

| Command | Status | Notes |
|---------|--------|-------|
| `lua evalscript` | ✅ IMPLEMENTED | Execute Lua script |
| `lua eval` | ✅ IMPLEMENTED | Execute Lua expression |

## Dataapi Commands (11/20 = 55%)

| Command | Status | Notes |
|---------|--------|-------|
| `dataapi pools` | ✅ IMPLEMENTED | Get pools for factory |
| `dataapi pool-history` | ✅ IMPLEMENTED | Get pool history |
| `dataapi trades` | ✅ IMPLEMENTED | Get trade history |
| `dataapi candles` | ✅ IMPLEMENTED | Get candle data |
| `dataapi reserves` | ✅ IMPLEMENTED | Get pool reserves |
| `dataapi holders` | ✅ IMPLEMENTED | Get alkane holders |
| `dataapi holders-count` | ✅ IMPLEMENTED | Get holder count |
| `dataapi bitcoin-price` | ✅ IMPLEMENTED | Get Bitcoin price |
| `dataapi bitcoin-market-chart` | ✅ IMPLEMENTED | Get market chart |
| `dataapi address-balances` | ✅ IMPLEMENTED | Get address balances |
| `dataapi alkanes-by-address` | ✅ IMPLEMENTED | Get alkanes by address |
| (9+ more commands) | ❌ MISSING | Additional Dataapi endpoints |

## ESPO Commands (9/15 = 60%)

| Command | Status | Notes |
|---------|--------|-------|
| `espo height` | ✅ IMPLEMENTED | Get ESPO height |
| `espo ping` | ✅ IMPLEMENTED | Ping ESPO service |
| `espo address-balances` | ✅ IMPLEMENTED | Get balances for address |
| `espo address-outpoints` | ✅ IMPLEMENTED | Get outpoints for address |
| `espo outpoint-balances` | ✅ IMPLEMENTED | Get balances for outpoint |
| `espo holders` | ✅ IMPLEMENTED | Get holders for alkane |
| `espo holders-count` | ✅ IMPLEMENTED | Get holder count |
| `espo keys` | ✅ IMPLEMENTED | Get storage keys |
| `espo ammdata-ping` | ✅ IMPLEMENTED | Ping AMM data service |
| (6+ more commands) | ❌ MISSING | Additional ESPO endpoints |

## BRC20-Prog Commands (9/4 = 225%) ✅

| Command | Status | Notes |
|---------|--------|-------|
| `brc20-prog balance` | ✅ IMPLEMENTED | Get balance for address |
| `brc20-prog code` | ✅ IMPLEMENTED | Get contract code |
| `brc20-prog block-number` | ✅ IMPLEMENTED | Get current block number |
| `brc20-prog chain-id` | ✅ IMPLEMENTED | Get chain ID |
| `brc20-prog tx-receipt` | ✅ IMPLEMENTED | Get transaction receipt |
| `brc20-prog tx` | ✅ IMPLEMENTED | Get transaction by hash |
| `brc20-prog block` | ✅ IMPLEMENTED | Get block by number |
| `brc20-prog call` | ✅ IMPLEMENTED | Call contract function |
| `brc20-prog estimate-gas` | ✅ IMPLEMENTED | Estimate gas |

## OPI Commands (5/30 = 17%)

| Command | Status | Notes |
|---------|--------|-------|
| `opi block-height` | ⚠️ PLACEHOLDER | Requires HTTP endpoint access |
| `opi extras-block-height` | ⚠️ PLACEHOLDER | Requires HTTP endpoint access |
| `opi db-version` | ⚠️ PLACEHOLDER | Requires HTTP endpoint access |
| `opi current-balance` | ⚠️ PLACEHOLDER | Requires HTTP endpoint access |
| `opi holders` | ⚠️ PLACEHOLDER | Requires HTTP endpoint access |
| (25+ more commands) | ❌ MISSING | Require direct HTTP endpoint access |

**Note**: OPI commands require direct HTTP requests to OPI endpoints and cannot be implemented through current WASM bindings.

## Subfrost Commands (0/1 = 0%)

| Command | Status | Notes |
|---------|--------|-------|
| `subfrost minimum-unwrap` | ❌ MISSING | Get minimum unwrap amount |

## Overall Progress

- **Total Commands**: ~190
- **Implemented**: 87+ commands across all groups
- **Progress**: ~46%

### Breakdown by Group:
- ✅ Wallet: 19/19 (100%)
- ✅ Bitcoind: 18/18 (100%)
- 🟡 Alkanes: 11/21 (52%)
- 🟡 Esplora: 11/35 (31%)
- 🟡 Ord: 6/15 (40%)
- 🟡 Runestone: 2/3 (67%)
- 🟡 Protorunes: 2/4 (50%)
- ✅ Metashrew: 4/3 (133%)
- ✅ Lua: 2/1 (200%)
- 🟡 Dataapi: 11/20 (55%)
- 🟡 ESPO: 9/15 (60%)
- ✅ BRC20-Prog: 9/4 (225%)
- ⚠️ OPI: 5/30 (17% - placeholder only)
- ❌ Subfrost: 0/1 (0%)

## Critical Missing Commands

### Transaction Building Commands (High Priority)
These require additional WASM bindings for transaction construction:
- `alkanes execute` - Execute alkanes transaction
- `alkanes wrap-btc` - Wrap BTC to frBTC
- `alkanes init-pool` - Initialize liquidity pool
- `alkanes swap` - Execute AMM swap

### OPI Commands (Low Priority)
OPI commands require direct HTTP endpoint access and cannot be implemented through current WASM bindings. Users should use the Rust `alkanes-cli` for OPI operations.

## Next Steps

1. **Test Implemented Commands** - Verify all 87+ commands work correctly
2. **Add Remaining Query Commands** - Complete Esplora, Ord, Dataapi, ESPO
3. **Transaction Building** - Implement execute, wrap-btc, init-pool, swap (requires WASM enhancements)
4. **Subfrost** - Add minimum-unwrap command
5. **Documentation** - Add examples and usage guide
