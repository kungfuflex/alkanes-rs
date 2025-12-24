# Subfrost-CLI Integration with Alkanes-CLI-Common

## Summary

Successfully integrated subfrost-cli with alkanes-cli-common's RpcConfig structure, aligning command-line interfaces and providing consistent flags across both tools.

## Changes Made

### 1. Updated `subfrost-common/src/commands.rs`

**Before:**
```rust
pub struct Args {
    pub sandshrew_rpc_url: Option<String>,
    pub bitcoin_rpc_url: Option<String>,
    pub metashrew_rpc_url: Option<String>,
    pub esplora_url: Option<String>,
    pub ord_url: Option<String>,
    pub provider: String,
    // ... FROST-specific fields
}
```

**After:**
```rust
pub struct Args {
    #[clap(flatten)]
    pub rpc_config: RpcConfig,  // From alkanes-cli-common

    // FROST-specific fields (preserved)
    pub frost_keystore: Option<String>,
    pub frost_files: Option<String>,
    pub circuit: Option<String>,
    // ... other FROST-specific fields
}
```

### 2. Updated Field References

Updated all code that referenced the old Args fields to use the new structure:
- `args.provider` → `args.rpc_config.provider`
- `args.bitcoin_rpc_url` → `args.rpc_config.bitcoin_rpc_url`
- `args.metashrew_rpc_url` → `args.rpc_config.metashrew_rpc_url`
- `args.sandshrew_rpc_url` → `args.rpc_config.jsonrpc_url` (renamed)
- `args.esplora_url` → `args.rpc_config.esplora_url`

Files updated:
- `/data/subfrost-master/crates/subfrost-cli/src/lib.rs`
- `/data/subfrost-master/crates/subfrost-cli/src/frost.rs`
- `/data/subfrost-master/crates/subfrost-cli/src/unwrap.rs`

### 3. New CLI Flags Available

subfrost-cli now has access to all RpcConfig flags from alkanes-cli-common:

**Network Configuration:**
- `-p, --provider <PROVIDER>` - Network provider (mainnet, testnet, signet, regtest)
- `--magic <MAGIC>` - Custom network magic bytes

**RPC URLs:**
- `--bitcoin-rpc-url <URL>` - Bitcoin Core RPC
- `--jsonrpc-url <URL>` - Subfrost JSON-RPC (replaces --sandshrew-rpc-url)
- `--titan-api-url <URL>` - Titan API (alternative to jsonrpc)
- `--metashrew-rpc-url <URL>` - Metashrew RPC
- `--esplora-url <URL>` - Esplora REST API
- `--ord-url <URL>` - Ord indexer API
- `--brc20-prog-rpc-url <URL>` - BRC20-Prog RPC
- `--data-api-url <URL>` - Analytics/indexing data API
- `--espo-rpc-url <URL>` - ESPO alkanes balance indexer

**Additional Options:**
- `--subfrost-api-key <KEY>` - Subfrost API authentication
- `--timeout-seconds <SECONDS>` - RPC timeout (default: 600)
- `--jsonrpc-header <HEADER>` - Custom HTTP headers (repeatable)

**FROST-Specific Flags (Preserved):**
- `--frost-keystore <PATH>` - FROST keystore file
- `--frost-files <PATH>` - FROST key shares directory
- `--circuit <URL>` - P2P circuit relay URL
- `--passphrase <PASS>` - Wallet/keystore passphrase
- `--idle-connection-timeout <SECONDS>` - libp2p timeout
- `--without-ord` - Skip ord indexer endpoints

## Compilation Results

✅ **All checks passed:**
- `cargo check -p subfrost-cli` - Success
- `cargo build -p subfrost-cli --release` - Success
- Binary location: `/data/subfrost-master/target/release/subfrost-cli`
- Binary size: 39 MB

## Test Script

Created `/data/alkanes-rs/test-frost-multisig.sh` to test the complete FROST multisig workflow:

1. Creates FROST multisig (6-of-9 threshold)
2. Gets multisig address
3. Mines blocks to fund the address
4. Creates a regular wallet
5. Sends Bitcoin from multisig to regular wallet
6. Confirms the transaction

Usage:
```bash
cd /data/alkanes-rs
./test-frost-multisig.sh
```

## Breaking Changes

### CLI Flag Rename

- `--sandshrew-rpc-url` → `--jsonrpc-url`

Users need to update their scripts/commands to use the new flag name.

### ConcreteProvider Usage

Code that creates `ConcreteProvider` instances should now source URLs from `args.rpc_config.*` instead of directly from `args.*`.

## Benefits

1. **Consistency** - Both alkanes-cli and subfrost-cli now use the same RPC configuration structure
2. **More Options** - subfrost-cli inherits all RPC configuration options from alkanes-cli-common
3. **Maintainability** - Shared code reduces duplication and makes updates easier
4. **Extensibility** - Future RPC options added to RpcConfig automatically available in both tools

## Next Steps

1. Run the test script to verify FROST multisig functionality
2. Update documentation to reflect new flag names
3. Consider deprecating subfrost-cli-sys in favor of alkanes-cli-sys for even more code sharing
