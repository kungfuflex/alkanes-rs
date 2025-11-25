# Lua Script Execution Refactoring

## Overview

This document describes the refactoring of Lua script execution to provide a unified, hash-based caching system that works across both native (alkanes-cli) and WASM (alkanes-web-sys) environments.

## Goals

1. **Unified API**: Single abstraction for executing Lua scripts across platforms
2. **Automatic Caching**: Hash-based script caching with automatic fallback
3. **WASM Compatible**: Works with both embedded scripts and runtime strings
4. **RPC Method Rename**: Migrate from `sandshrew_*` to `lua_*` naming
5. **Replace Custom Methods**: Use Lua scripts instead of hardcoded RPC methods

## Architecture

### Core Components

#### 1. `LuaScript` Struct (`lua_script.rs`)

```rust
pub struct LuaScript {
    content: String,  // The Lua script content
    hash: String,     // SHA-256 hash for caching
}
```

**Features:**
- Computes SHA-256 hash of script content automatically
- Can be created from static strings (`include_str!`) or runtime strings
- Provides access to both content and hash

#### 2. `LuaScriptExecutor` Trait

```rust
#[async_trait(?Send)]
pub trait LuaScriptExecutor {
    async fn execute_lua_script(&self, script: &LuaScript, args: Vec<JsonValue>) -> Result<JsonValue>;
    async fn lua_evalsaved(&self, script_hash: &str, args: Vec<JsonValue>) -> Result<JsonValue>;
    async fn lua_evalscript(&self, script_content: &str, args: Vec<JsonValue>) -> Result<JsonValue>;
}
```

**Behavior:**
- `execute_lua_script()`: High-level method with automatic caching
  1. Tries `lua_evalsaved` with script hash first (cached execution)
  2. Falls back to `lua_evalscript` with full content if not cached
- `lua_evalsaved()`: Low-level RPC call with hash
- `lua_evalscript()`: Low-level RPC call with full script

#### 3. Embedded Scripts

Pre-compiled scripts available as static constants:

```rust
pub mod scripts {
    pub static BATCH_UTXO_BALANCES: Lazy<LuaScript>;
    pub static BALANCES: Lazy<LuaScript>;
    pub static MULTICALL: Lazy<LuaScript>;
}
```

### RPC Method Migration

| Old Method | New Method | Replacement |
|------------|------------|-------------|
| `sandshrew_evalscript` | `lua_evalscript` | Direct rename |
| `sandshrew_evalsaved` | `lua_evalsaved` | Direct rename |
| `sandshrew_balances` | *(removed)* | `lua/balances.lua` script |
| `sandshrew_multicall` | *(removed)* | `lua/multicall.lua` script |

### Lua Scripts

#### `lua/balances.lua`

Replacement for `sandshrew_balances` RPC method.

**Functionality:**
- Fetches UTXOs for an address
- Queries protorunes/alkanes balances
- Queries ord outputs (inscriptions and runes)
- Categorizes UTXOs into spendable/assets/pending
- Returns comprehensive balance information

**Usage:**
```rust
use crate::lua_script::scripts;
let result = provider.execute_lua_script(
    &scripts::BALANCES,
    vec![JsonValue::String(address.clone())]
).await?;
```

#### `lua/multicall.lua`

Replacement for `sandshrew_multicall` RPC method.

**Functionality:**
- Executes multiple RPC calls in a single batch
- Returns array of results with error handling per call

**Usage:**
```rust
use crate::lua_script::scripts;
let calls = vec![
    json!(["btc_getblockcount", []]),
    json!(["btc_getblockhash", [100]]),
];
let results = provider.execute_lua_script(
    &scripts::MULTICALL,
    calls
).await?;
```

#### `lua/batch_utxo_balances.lua`

Optimized UTXO balance fetching (90+ RPC calls → 1 call per address).

**Functionality:**
- Fetches UTXOs for an address
- Queries alkane balances for each UTXO
- Returns consolidated results

**Usage:**
```rust
let result = provider.batch_fetch_utxo_balances(
    address,
    Some(1),  // protocol_tag
    None,     // block_tag
).await?;
```

## Implementation Details

### Hash-Based Caching Flow

```
User Code
    ↓
execute_lua_script(script, args)
    ↓
Try lua_evalsaved(script.hash(), args)
    ↓
Success? → Return result
    ↓
Failure (not cached)
    ↓
Fallback: lua_evalscript(script.content(), args)
    ↓
Return result
```

### Platform Compatibility

**Native (alkanes-cli):**
- Can load scripts from filesystem: `--from-file ~/my_script.lua`
- Uses embedded scripts via `include_str!()`
- Full async/await support

**WASM (alkanes-web-sys):**
- Can pass scripts as JS strings
- Uses embedded scripts via `include_str!()`
- Compatible with ?Send async trait

### Integration with DeezelProvider

The `LuaScriptExecutor` trait is now part of the `DeezelProvider` composite trait:

```rust
pub trait DeezelProvider:
    JsonRpcProvider +
    // ... other traits ...
    LuaScriptExecutor +
    // ... more traits ...
{
    // batch_fetch_utxo_balances has a default implementation
    async fn batch_fetch_utxo_balances(...) -> Result<JsonValue> { ... }
}
```

## Benefits

### 1. **Performance**
- Hash-based caching reduces bandwidth
- Server-side script caching via `lua_evalsaved`
- Single RPC call for cached scripts

### 2. **Flexibility**
- Same API for embedded and runtime scripts
- Works in both native and WASM contexts
- Easy to add new Lua scripts

### 3. **Maintainability**
- Lua scripts are separate files, easy to modify
- No need to recompile for script changes (in development)
- Clear separation of concerns

### 4. **Backwards Compatibility**
- Automatic fallback to full script execution
- Gradual migration path from old RPC methods

## Migration Guide

### For Providers

**Old Code:**
```rust
let params = json!([{ "address": address }]);
let result = self.call(&url, "sandshrew_balances", params, 1).await?;
```

**New Code:**
```rust
use crate::lua_script::scripts;
let result = self.execute_lua_script(
    &scripts::BALANCES,
    vec![JsonValue::String(address.clone())]
).await?;
```

### For CLI Tools

**Old Code:**
```rust
let request = json!({
    "jsonrpc": "2.0",
    "method": "sandshrew_evalscript",
    "params": [script_content, arg1, arg2],
    "id": 1
});
```

**New Code:**
```rust
let request = json!({
    "jsonrpc": "2.0",
    "method": "lua_evalscript",
    "params": [script_content, arg1, arg2],
    "id": 1
});
```

### For alkanes-web-sys

```javascript
// Create a LuaScript from JS string
const script = `
return {
    result = args[1] + args[2]
}
`;

// Execute via provider
const result = await provider.execute_lua_script(script, [10, 20]);
```

## Testing

### Compilation
```bash
cargo check --package alkanes-cli-common
cargo check --package alkanes-cli
cargo check --package alkanes-web-sys
```

### Script Execution
```bash
# Test balances script
alkanes-cli -p regtest \
  --sandshrew-rpc-url https://regtest.subfrost.io/v4/jsonrpc \
  wallet balance

# Test multicall (if exposed via CLI)
# Should use lua_multicall internally
```

## Future Enhancements

1. **Script Registry**: Dynamic script loading and management
2. **CLI `--from-file`**: Load Lua scripts from filesystem
3. **Script Versioning**: Version-specific script caching
4. **Performance Metrics**: Track cache hit rates
5. **More Scripts**: Additional Lua scripts for common operations

## Files Modified

### Core Implementation
- `crates/alkanes-cli-common/src/lua_script.rs` (new)
- `crates/alkanes-cli-common/src/lib.rs`
- `crates/alkanes-cli-common/src/provider.rs`
- `crates/alkanes-cli-common/src/traits.rs`
- `crates/alkanes-cli-common/Cargo.toml`

### Lua Scripts
- `lua/balances.lua` (enhanced)
- `lua/multicall.lua` (new)
- `lua/batch_utxo_balances.lua` (new)

### CLI Updates
- `crates/alkanes-cli/src/main.rs`

### Documentation
- `LUA_SCRIPT_REFACTORING.md` (this file)
- `OPTIMIZATION_SUMMARY.md`

## Notes

- All Lua scripts are embedded at compile time using `include_str!()`
- No runtime filesystem dependencies
- Works identically in native and WASM contexts
- Automatic caching reduces server load and improves performance
- Scripts are validated at compile time (syntax errors caught early)

## See Also

- [OPTIMIZATION_SUMMARY.md](./OPTIMIZATION_SUMMARY.md) - RPC call batching optimization
- [lua/](./lua/) - Lua script directory
