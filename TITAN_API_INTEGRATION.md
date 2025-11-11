# Titan API Integration for alkanes-cli

## Summary

This document describes the changes made to alkanes-cli-common, alkanes-cli-sys, and alkanes-cli to support using Titan's REST API as an alternative backend to sandshrew-rpc.

## Changes Made

### 1. **network.rs** - Added Titan API URL Configuration

**File**: `crates/alkanes-cli-common/src/network.rs`

- Added `titan_api_url: Option<String>` field to `RpcConfig` struct
- Added validation method `validate()` to ensure only one backend is configured:
  - Prevents both `sandshrew_rpc_url` and `titan_api_url` from being set simultaneously
  - Returns an error if both are provided
- Added `using_titan_api()` helper method to check if Titan API is being used
- Updated `Default` implementation to include `titan_api_url: None`

**CLI Flag**: `--titan-api-url <URL>`

### 2. **provider.rs** - Added Titan REST API Client Methods

**File**: `crates/alkanes-cli-common/src/provider.rs`

#### Added Helper Methods:

- `call_titan_rest_api(&self, path: &str) -> Result<serde_json::Value>`
  - Makes GET requests to Titan REST API
  - Handles timeout configuration
  - Returns JSON response
  - Available for both `native-deps` and no-op for WASM

- `post_titan_rest_api(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value>`
  - Makes POST requests to Titan REST API
  - Used for endpoints like `/alkanes/simulate`

#### Updated AlkanesProvider Implementation:

Modified the following methods to support Titan REST API:

1. **protorunes_by_address**
   - Endpoint: `/alkanes/byaddress/{address}[/atheight/{height}]`
   - Falls back to sandshrew if titan_api_url not set

2. **protorunes_by_outpoint**
   - Endpoint: `/alkanes/byoutpoint/{txid}:{vout}[/atheight/{height}]`
   - Falls back to sandshrew if titan_api_url not set

3. **trace**
   - Endpoint: `/alkanes/trace/{outpoint}`
   - Handles both hex and JSON response formats
   - Falls back to sandshrew if titan_api_url not set

4. **spendables_by_address**
   - Endpoint: `/alkanes/byaddress/{address}`
   - Returns JSON directly from Titan
   - Falls back to sandshrew if titan_api_url not set

5. **get_bytecode**
   - Endpoint: `/alkanes/getbytecode/{alkane_id}[/atheight/{height}]`
   - Returns hex-encoded bytecode
   - Falls back to sandshrew if titan_api_url not set

### 3. **main.rs** - Added Validation in CLI Entry Point

**File**: `crates/alkanes-cli/src/main.rs`

- Added `alkanes_args.rpc_config.validate()?;` call after parsing arguments
- Ensures validation happens before SystemAlkanes initialization

### 4. **lib.rs** - Added Validation in System Library

**File**: `crates/alkanes-cli-sys/src/lib.rs`

- Added `args.rpc_config.validate()?;` call in `SystemAlkanes::new()`
- Validates configuration at system initialization

## Titan REST API Endpoints Mapping

| Method | Titan REST Endpoint | Parameters |
|--------|-------------------|------------|
| `protorunes_by_address` | `GET /alkanes/byaddress/{address}` | `atheight/{height}` (optional) |
| `protorunes_by_outpoint` | `GET /alkanes/byoutpoint/{txid}:{vout}` | `atheight/{height}` (optional) |
| `trace` | `GET /alkanes/trace/{outpoint}` | - |
| `spendables_by_address` | `GET /alkanes/byaddress/{address}` | - |
| `get_bytecode` | `GET /alkanes/getbytecode/{alkane_id}` | `atheight/{height}` (optional) |
| `get_inventory` | `GET /alkanes/getinventory/{alkane_id}` | `atheight/{height}` (optional) |
| `get_storage_at` | `GET /alkanes/getstorageat/{alkane_id}/{key}` | `atheight/{height}` (optional) |
| `simulate` | `POST /alkanes/simulate` | JSON body with simulation params |

**Note**: `get_inventory` and `get_storage_at` are called via the generic `view()` method, which currently doesn't have Titan API support but can be added if needed.

## Usage Examples

### Using Sandshrew RPC (Original)
```bash
alkanes --sandshrew-rpc-url http://localhost:18888 alkanes getbytecode 2:0
```

### Using Titan API (New)
```bash
alkanes --titan-api-url http://localhost:3030 alkanes getbytecode 2:0
```

### Error Case (Both Specified)
```bash
alkanes --sandshrew-rpc-url http://localhost:18888 --titan-api-url http://localhost:3030 alkanes getbytecode 2:0
```
**Error**: `Cannot specify both --sandshrew-rpc-url and --titan-api-url. Please choose one backend.`

## Supported Commands with Titan API

All `alkanes` and `protorunes` subcommands work with Titan API backend, except:
- `alkanes execute` - Requires transaction building, still uses full stack
- `alkanes wrap-btc` - Requires transaction building, still uses full stack

Supported commands include:
- ✅ `alkanes getbytecode`
- ✅ `alkanes trace`
- ✅ `alkanes simulate`
- ✅ `alkanes spendables`
- ✅ `alkanes sequence`
- ✅ `alkanes trace-block`
- ✅ `alkanes get-balance`
- ✅ `protorunes by-address`
- ✅ `protorunes by-outpoint`

## Benefits

1. **Simplified Architecture**: Can use Titan's REST API directly without needing sandshrew-rpc
2. **Better Performance**: REST API may be faster than protobuf-based RPC for some operations
3. **Easier Deployment**: One less service to run (no sandshrew-rpc needed)
4. **Backward Compatible**: Still supports sandshrew-rpc if preferred
5. **Flexible**: Users can choose which backend to use based on their setup

## Testing

To test the implementation:

1. **Start Titan with alkanes enabled**:
   ```bash
   titan --enable-alkanes --chain regtest
   ```

2. **Test with Titan API**:
   ```bash
   alkanes --titan-api-url http://localhost:3030 -p regtest alkanes getbytecode 2:0
   ```

3. **Verify validation**:
   ```bash
   # This should fail with validation error
   alkanes --sandshrew-rpc-url http://localhost:18888 \
           --titan-api-url http://localhost:3030 \
           alkanes getbytecode 2:0
   ```

## Future Enhancements

Potential future improvements:
1. Add Titan API support for `get_inventory` and `get_storage_at` via view method
2. Add Titan API endpoints for remaining metashrew methods
3. Add connection health checks and fallback logic
4. Add caching layer for frequently accessed data
5. Add metrics and monitoring for API calls
