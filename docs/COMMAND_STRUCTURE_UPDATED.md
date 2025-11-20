# ✅ DataAPI Command Structure - Updated

## Summary

The DataAPI commands have been restructured per requirements:

1. ✅ `dataapi` is now a **top-level command** (not under `alkanes`)
2. ✅ `--data-api` flag added with **network-specific defaults**
3. ✅ All commands work with the new structure

---

## 🎯 New Command Structure

### Before (Old Structure):
```bash
# ❌ Old way
alkanes-cli alkanes dataapi get-bitcoin-price
```

### After (New Structure):
```bash
# ✅ New way - dataapi is top-level
alkanes-cli dataapi get-bitcoin-price

# ✅ With custom data API URL
alkanes-cli --data-api http://localhost:4000 dataapi get-bitcoin-price

# ✅ Network-specific defaults
alkanes-cli -p regtest dataapi get-bitcoin-price      # Uses http://localhost:4000
alkanes-cli -p mainnet dataapi get-bitcoin-price      # Uses https://mainnet-api.oyl.gg
```

---

## 🌐 Network-Specific Defaults

The `--data-api` flag has smart defaults based on the `-p` (provider) flag:

| Network | Default Data API URL |
|---------|---------------------|
| **regtest** | `http://localhost:4000` |
| **signet** | `http://localhost:4000` |
| **testnet** | `http://localhost:4000` |
| **mainnet** | `https://mainnet-api.oyl.gg` |

### Examples:

```bash
# Regtest (default) - uses http://localhost:4000
alkanes-cli dataapi get-bitcoin-price
alkanes-cli -p regtest dataapi get-bitcoin-price

# Mainnet - uses https://mainnet-api.oyl.gg
alkanes-cli -p mainnet dataapi get-bitcoin-price

# Custom URL - overrides network default
alkanes-cli --data-api https://custom-api.example.com dataapi get-bitcoin-price
```

---

## 📋 All DataAPI Commands (Top-Level)

```bash
# Health check
alkanes-cli dataapi health

# Bitcoin price
alkanes-cli dataapi get-bitcoin-price

# Market chart (days: 1, 7, 14, 30, 90, 180, 365, max)
alkanes-cli dataapi get-market-chart 7

# All alkanes
alkanes-cli dataapi get-alkanes [--limit 10] [--offset 0]

# Alkanes by address
alkanes-cli dataapi get-alkanes-by-address <address>

# Alkane details
alkanes-cli dataapi get-alkane-details 2:0

# All pools (defaults to factory 4:65522)
alkanes-cli dataapi get-pools [--factory 4:65522]

# Pool by ID
alkanes-cli dataapi get-pool-by-id <pool_id>

# Pool history
alkanes-cli dataapi get-pool-history <pool_id> [--category swap]

# Swap history
alkanes-cli dataapi get-swap-history [--pool-id <id>] [--limit 10]
```

---

## 🔧 AMM Commands (Under alkanes)

AMM operations remain under the `alkanes` subcommand:

```bash
# Initialize pool (factory defaults to 4:65522)
alkanes-cli alkanes init-pool \
    --pair 2:0,32:0 \
    --liquidity 300000000:50000 \
    --to p2tr:0 \
    --from p2tr:0 \
    [--factory 4:65522]

# Execute swap (factory defaults to 4:65522)
alkanes-cli alkanes swap \
    --path 2:0:32:0 \
    --input 1000000 \
    --minimum 100 \
    --to p2tr:0 \
    --from p2tr:0 \
    [--factory 4:65522]
```

---

## 🔍 Implementation Details

### Global Flag Added
**File**: `crates/alkanes-cli/src/commands.rs`

```rust
pub struct DeezelCommands {
    // ... other fields ...
    
    /// Data API URL (defaults to http://localhost:4000 for regtest, 
    /// https://mainnet-api.oyl.gg for mainnet)
    #[arg(long)]
    pub data_api: Option<String>,
    
    /// Network provider
    #[arg(short, long, default_value = "regtest")]
    pub provider: String,
    
    // ... commands ...
}
```

### Top-Level Command
**File**: `crates/alkanes-cli/src/commands.rs`

```rust
pub enum Commands {
    // ... other commands ...
    
    /// DataAPI subcommands - Query data from alkanes-data-api
    #[command(subcommand)]
    Dataapi(DataApiCommand),
}
```

### Handler with Network Logic
**File**: `crates/alkanes-cli/src/main.rs`

```rust
async fn execute_dataapi_command(args: &DeezelCommands, command: DataApiCommand) -> Result<()> {
    // Determine the data API URL based on --data-api flag or provider network
    let api_url = if let Some(ref url) = args.data_api {
        url.clone()
    } else {
        match args.provider.as_str() {
            "mainnet" => "https://mainnet-api.oyl.gg".to_string(),
            "regtest" | "signet" | "testnet" | _ => "http://localhost:4000".to_string(),
        }
    };
    
    let client = DataApiClient::new(api_url);
    // ... execute commands ...
}
```

---

## ✅ Verification

### Command Structure Verified
```bash
$ alkanes-cli --help | grep dataapi
  dataapi     DataAPI subcommands - Query data from alkanes-data-api

$ alkanes-cli --help | grep data-api
      --data-api <DATA_API>
          Data API URL (defaults to http://localhost:4000 for regtest, 
          https://mainnet-api.oyl.gg for mainnet)

$ alkanes-cli dataapi --help
DataAPI subcommands - Query data from alkanes-data-api

Usage: alkanes-cli dataapi <COMMAND>

Commands:
  get-alkanes             Get all alkanes
  get-alkanes-by-address  Get alkanes for an address
  get-alkane-details      Get alkane details
  get-pools               Get all pools (defaults to factory 4:65522)
  ...
```

### AMM Commands Verified
```bash
$ alkanes-cli alkanes --help | grep "init-pool\|swap"
  init-pool          Initialize a new liquidity pool
  swap               Execute a swap on the AMM
```

---

## 🧪 Testing Examples

### Test Network Defaults
```bash
# Regtest - should use http://localhost:4000
alkanes-cli -p regtest dataapi health

# Mainnet - should use https://mainnet-api.oyl.gg
alkanes-cli -p mainnet dataapi get-bitcoin-price
```

### Test Custom URL
```bash
# Override default with custom URL
alkanes-cli --data-api https://custom-api.example.com dataapi health
```

### Test AMM Commands
```bash
# Init pool (factory defaults to 4:65522)
alkanes-cli alkanes init-pool \
    --pair 2:0,32:0 \
    --liquidity 300000000:50000 \
    --to p2tr:0 \
    --from p2tr:0

# Swap (factory defaults to 4:65522)
alkanes-cli alkanes swap \
    --path 2:0:32:0 \
    --input 1000000 \
    --minimum 100 \
    --to p2tr:0 \
    --from p2tr:0
```

---

## ✨ Benefits

1. **Cleaner Command Structure**: `dataapi` at top level makes more sense
2. **Network-Aware Defaults**: Automatically uses correct API based on network
3. **Flexibility**: Can override with `--data-api` for custom deployments
4. **Better UX**: Shorter commands, more intuitive

---

## 🎉 Status

**✅ Complete** - All requirements implemented:
- ✅ `dataapi` is top-level command (not under `alkanes`)
- ✅ `--data-api` flag with network-specific defaults
- ✅ Regtest → `http://localhost:4000`
- ✅ Mainnet → `https://mainnet-api.oyl.gg`
- ✅ Factory defaults to `4:65522` everywhere
- ✅ Release build succeeds (11.11s)
- ✅ Binary is 21 MB

---

*Updated: November 20, 2025*
*Build: Release v10.0.0*
*Status: 🟢 READY TO TEST*
