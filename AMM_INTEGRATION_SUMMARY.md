# AMM Integration Summary

## Overview
Successfully integrated AMM (Automated Market Maker) simulation functionality from deezel-sprimage into alkanes-cli-common, following the proper architecture pattern of using `metashrew_view` with encoded `MessageContextParcel` messages.

## Changes Made

### 1. Enhanced alkanes-cli-common/src/alkanes/amm.rs

Added three new methods to `AmmManager`:

#### `get_all_pools(factory_id, provider)`
- Simulates factory contract with opcode 3 (GET_ALL_POOLS)
- Returns `GetAllPoolsResult` with list of pool IDs
- Uses proper MessageContextParcel encoding with LEB128

#### `get_all_pools_details(factory_id, provider)`
- First calls `get_all_pools()` to get pool list
- Then simulates each pool with opcode 999 (POOL_DETAILS)
- Returns `AllPoolsDetailsResult` with full pool information including:
  - token0 and token1 AlkaneIds
  - token amounts and reserves
  - LP token supply
  - pool name

#### `get_pool_details(pool_id, provider)`
- Simulates a single pool with opcode 999 (POOL_DETAILS)
- Returns `PoolDetailsResult` with pool information

### 2. Added Result Types

New public types exported from amm.rs:
- `GetAllPoolsResult` - contains count and vector of pool AlkaneIds
- `AllPoolsDetailsResult` - contains count and vector of PoolDetailsWithId
- `PoolDetailsResult` - pool information without ID
- `PoolDetailsWithId` - pool information with pool ID

### 3. Added Pool/Factory Operation Codes

Constants for AMM contract interaction:
```rust
const POOL_OPCODE_POOL_DETAILS: u64 = 999;
const FACTORY_OPCODE_GET_ALL_POOLS: u64 = 3;
```

### 4. Added Decoding Functions

- `decode_get_all_pools()` - Decodes hex response to pool list
- `decode_pool_details()` - Decodes hex response to pool details
- `parse_alkane_id_from_hex()` - Parses 32-byte AlkaneId from hex
- Helper functions: `strip_0x()`, `hex_to_bytes()`, `read_u128_le()`

### 5. Updated alkanes-cli-common/src/alkanes/mod.rs

- Added `pub mod amm;`
- Exported AMM result types for CLI usage

### 6. Added CLI Commands to alkanes-cli-common/src/commands.rs

Three new AlkanesCommands variants:
- `GetAllPools { factory_id, raw }` - List all pools from factory
- `AllPoolsDetails { factory_id, raw }` - List all pools with full details
- `PoolDetails { pool_id, raw }` - Get details for specific pool

## Key Implementation Details

### Proper metashrew_view Usage

Unlike deezel-sprimage which used raw JSON simulate requests, this implementation correctly uses:

1. **MessageContextParcel** - The protobuf message structure
2. **LEB128 encoding** - For calldata (block, tx, opcode)
3. **AlkanesProvider trait** - Using the `simulate()` method which:
   - Encodes the MessageContextParcel  
   - Calls metashrew_view with the encoded data
   - Returns the simulation result

Example calldata encoding:
```rust
let mut calldata = Vec::new();
leb128::write::unsigned(&mut calldata, factory_id.block).unwrap();
leb128::write::unsigned(&mut calldata, factory_id.tx).unwrap();
leb128::write::unsigned(&mut calldata, FACTORY_OPCODE_GET_ALL_POOLS).unwrap();
```

### Response Format

The simulate response has this structure:
```json
{
  "data": "0x..." // hex-encoded return data
}
```

Not the nested `execution.data` structure that deezel-sprimage expected.

## Next Steps

### For alkanes-cli Integration

Still needed in `./crates/alkanes-cli/src/main.rs`:

1. Handle the new AMM commands in the match statement
2. Call the AmmManager methods
3. Pretty-print the results (similar to other commands)

Example handler:
```rust
AlkanesCommands::GetAllPools { factory_id, raw } => {
    let (block, tx) = parse_alkane_id(&factory_id)?;
    let factory_id = alkanes::types::AlkaneId { block, tx };
    
    let amm_manager = alkanes::amm::AmmManager::new(executor);
    let result = amm_manager.get_all_pools(&factory_id, provider).await?;
    
    if raw {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("Found {} pools:", result.count);
        for pool in &result.pools {
            println!("  {}:{}", pool.block, pool.tx);
        }
    }
}
```

### For alkanes-contract-indexer

The indexer can now use `alkanes-cli-sys` instead of the modified deezel crates because:

1. All AMM functionality is now in alkanes-cli-common
2. Proper metashrew_view integration through AlkanesProvider trait  
3. No need for custom deezel modifications

## Benefits

1. **Cleaner Architecture** - Uses existing alkanes-cli patterns
2. **Proper Abstraction** - Works through AlkanesProvider trait
3. **Reusable** - Can be used in CLI, indexer, and web builds
4. **Maintainable** - Follows alkanes-rs conventions
5. **Type-Safe** - Proper protobuf message encoding

## Testing

To test the new commands:

```bash
# Get all pools from factory
alkanes-cli alkanes get-all-pools 32:0

# Get all pools with details
alkanes-cli alkanes all-pools-details 32:0

# Get single pool details  
alkanes-cli alkanes pool-details 100:5
```

## Files Modified

1. `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/amm.rs` - Added AMM query methods
2. `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/mod.rs` - Export AMM module
3. `/data/alkanes-rs/crates/alkanes-cli-common/src/commands.rs` - Added CLI commands

## Files To Be Modified

1. `/data/alkanes-rs/crates/alkanes-cli/src/main.rs` - Add command handlers
2. `/data/alkanes-rs/crates/alkanes-contract-indexer/*` - Update to use alkanes-cli-sys
