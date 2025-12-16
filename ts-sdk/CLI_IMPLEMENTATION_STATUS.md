# alkanes-bindgen-cli Implementation Status

## 🎯 Goal
Create a Node.js CLI (`alkanes-cli`) in `./ts-sdk` with **full feature parity** to the Rust `alkanes-cli` by leveraging shared code from `alkanes-cli-common` through `alkanes-web-sys` WASM bindings.

## ✅ Completed Core Infrastructure

### Phase 1: Foundation (100% Complete)
- ✅ **UtxoProvider Trait** - Implemented in `alkanes-web-sys` (CRITICAL GAP FIXED)
  - Added `get_utxos_by_spec` method
  - Parses address specs (e.g., `p2tr:0-5`)
  - Fetches UTXOs via Esplora API
  - Location: `/data/alkanes-rs/crates/alkanes-web-sys/src/provider.rs:3611-3664`

- ✅ **WASM Bindings** - Rebuilt with new functionality
  - 6.3MB WASM binary with UtxoProvider support

- ✅ **Package Configuration**
  - `bin` field: `alkanes-cli` → `dist/cli.js`
  - Dependencies: commander, chalk, ora, inquirer, cli-table3
  - Build scripts for CLI

- ✅ **CLI Infrastructure**
  - Main entry: `src/cli/index.ts`
  - Utilities: provider.ts, config.ts, formatting.ts, wallet.ts, prompts.ts
  - Dynamic WASM loading (resolves path issues)
  - Global options: `-p`, `--wallet-file`, `--passphrase`, `--jsonrpc-url`, `--raw`, `-y`

## 📊 Command Implementation Progress

### Legend
- ✅ Implemented and working
- 🔨 Partially implemented
- ❌ Not yet implemented

### Command Group Overview (15 groups total)

| Group | Commands Implemented | Total Commands | Progress |
|-------|---------------------|----------------|----------|
| **wallet** | 5 / 19 | 19 | 26% 🔨 |
| **bitcoind** | 9 / 18 | 18 | 50% 🔨 |
| **alkanes** | 0 / 21 | 21 | 0% ❌ |
| **esplora** | 0 / 35 | 35 | 0% ❌ |
| **ord** | 0 / 15 | 15 | 0% ❌ |
| **runestone** | 0 / 3 | 3 | 0% ❌ |
| **protorunes** | 0 / 4 | 4 | 0% ❌ |
| **metashrew** | 0 / 3 | 3 | 0% ❌ |
| **lua** | 0 / 1 | 1 | 0% ❌ |
| **dataapi** | 0 / 20 | 20 | 0% ❌ |
| **opi** | 0 / 30 | 30 | 0% ❌ |
| **subfrost** | 0 / 1 | 1 | 0% ❌ |
| **espo** | 0 / 15 | 15 | 0% ❌ |
| **brc20-prog** | 0 / 4 | 4 | 0% ❌ |
| **decodepsbt** | 0 / 1 | 1 | 0% ❌ |
| **TOTAL** | **14 / 190** | **190** | **7%** |

## ✅ Implemented Commands

### Wallet Commands (5/19)
- ✅ `wallet create` - Create/restore wallet with mnemonic
- ✅ `wallet addresses <spec>` - Get addresses (e.g., `p2tr:0-10`)
- ✅ `wallet utxos <spec>` - Get UTXOs using UtxoProvider
- ✅ `wallet balance` - Check wallet balance
- ✅ `wallet send <address> <amount>` - Send BTC transactions

#### Remaining Wallet Commands
- ❌ `wallet freeze <outpoint>` - Freeze UTXO
- ❌ `wallet unfreeze <outpoint>` - Unfreeze UTXO
- ❌ `wallet sign <psbt>` - Sign PSBT
- ❌ `wallet history` - Transaction history
- ❌ `wallet create-tx` - Create transaction
- ❌ `wallet sign-tx` - Sign transaction
- ❌ `wallet decode-tx` - Decode transaction
- ❌ `wallet broadcast-tx` - Broadcast transaction
- ❌ `wallet estimate-fee` - Estimate fee
- ❌ `wallet fee-rates` - Get fee rates
- ❌ `wallet sync` - Sync wallet
- ❌ `wallet backup` - Backup wallet
- ❌ `wallet mnemonic` - Get mnemonic

### Bitcoind Commands (9/18)
- ✅ `bitcoind getblockcount` - Get block height
- ✅ `bitcoind generatetoaddress <n> <addr>` - Mine blocks (regtest)
- ✅ `bitcoind getblockchaininfo` - Blockchain info
- ✅ `bitcoind getrawtransaction <txid>` - Get raw tx
- ✅ `bitcoind getblock <hash>` - Get block
- ✅ `bitcoind getblockhash <height>` - Get block hash
- ✅ `bitcoind sendrawtransaction <hex>` - Broadcast tx
- ✅ `bitcoind getnetworkinfo` - Network info
- ✅ `bitcoind getmempoolinfo` - Mempool info

#### Remaining Bitcoind Commands
- ❌ `bitcoind generatefuture` - Generate future block
- ❌ `bitcoind getblockheader` - Get block header
- ❌ `bitcoind getblockstats` - Get block stats
- ❌ `bitcoind decoderawtransaction` - Decode raw tx
- ❌ `bitcoind decodepsbt` - Decode PSBT
- ❌ `bitcoind getchaintips` - Get chain tips
- ❌ `bitcoind getrawmempool` - Get mempool txs
- ❌ `bitcoind gettxout` - Get tx output
- ❌ `bitcoind help` - Bitcoin RPC help

## 🚀 Testing the CLI

### Installation
```bash
# Build the CLI
cd /data/alkanes-rs/ts-sdk
npm run build:cli

# Test locally
node dist/index.js --help

# Or install globally
npm link
alkanes-cli --help
```

### Example Commands
```bash
# Wallet operations
alkanes-cli wallet create
alkanes-cli wallet addresses p2tr:0-10
alkanes-cli wallet utxos p2tr:0
alkanes-cli wallet balance

# Bitcoind operations (requires RPC access)
alkanes-cli -p regtest --jsonrpc-url http://localhost:18443 bitcoind getblockcount
alkanes-cli -p regtest bitcoind generatetoaddress 1 p2tr:0

# With global options
alkanes-cli -p mainnet --raw bitcoind getblockchaininfo
```

## 📋 Next Steps: Command-by-Command Comparison

To achieve full parity, we need to implement:

### Priority 1: Core Alkanes Commands (Critical for deploy-regtest.sh)
- `alkanes execute` - Deploy/call contracts
- `alkanes getbytecode` - Verify deployment
- `alkanes wrap-btc` - Wrap BTC to frBTC
- `alkanes init-pool` - Initialize AMM pool
- `alkanes swap` - Execute swap
- `alkanes getbalance` - Get alkanes balance

### Priority 2: Remaining Wallet Commands
- Complete all 14 remaining wallet commands for full wallet functionality

### Priority 3: Esplora Commands
- 35 commands for Esplora API access

### Priority 4: All Other Command Groups
- ord, runestone, protorunes, metashrew, lua, dataapi, opi, subfrost, espo, brc20-prog, decodepsbt

## 🏗️ Implementation Pattern

Most commands follow this pattern:

```typescript
command
  .command('name <arg>')
  .description('Description')
  .action(async (arg, options, command) => {
    const globalOpts = command.parent?.parent?.opts() || {};
    const spinner = ora('Loading...').start();

    const provider = await createProvider({
      network: globalOpts.provider,
      jsonrpcUrl: globalOpts.jsonrpcUrl,
    });

    const result = await provider.method_name_js(arg);
    const data = JSON.parse(result);

    spinner.succeed();
    console.log(formatOutput(data, globalOpts));
  });
```

## 🎯 Architecture Success

The three-layer architecture is working perfectly:

```
alkanes-cli (Node.js/TypeScript)
    ↓
ts-sdk/src/cli/ (Command implementations)
    ↓
@alkanes/ts-sdk/wasm (WASM bindings)
    ↓
alkanes-cli-common (Shared Rust business logic)
```

This mirrors the Rust CLI architecture:

```
alkanes-cli (Rust)
    ↓
alkanes-cli-sys (System integration)
    ↓
alkanes-cli-common (Shared business logic)
```

## 💡 Key Achievements

1. **UtxoProvider Gap Fixed** - The critical missing piece is now implemented
2. **Dynamic WASM Loading** - Solves path resolution issues in bundled CLI
3. **Feature Parity Framework** - Infrastructure ready for rapid command addition
4. **Proven Pattern** - Wallet and bitcoind commands demonstrate the approach works

## 📈 Velocity Potential

With the infrastructure in place, adding new commands is now straightforward:
- **Simple commands** (RPC wrappers): ~5-10 minutes each
- **Complex commands** (multi-step logic): ~30-60 minutes each
- **Estimated time to complete**: 20-40 hours of focused implementation

The hard architectural work is done! 🎉
