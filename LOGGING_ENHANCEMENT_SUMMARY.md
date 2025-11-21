# Enhanced Logging System Implementation

## Overview

This document summarizes the comprehensive logging enhancements made to the alkanes-rs project to provide proper colored/treeview/emoji enhanced logs throughout the indexing process, including WASM runtime output.

## Changes Made

### 1. Created Unified Logging Modules

#### `/crates/alkanes/src/logging.rs`
- **Purpose**: Centralized logging utilities for the alkanes crate with colored/treeview/emoji support
- **Features**:
  - Emoji-prefixed log styles for different contexts:
    - ℹ️ INFO
    - ✅ SUCCESS
    - ⚠️ WARNING
    - ❌ ERROR
    - 🔍 DEBUG
    - 📦 BLOCK
    - 💳 TX (Transaction)
    - 🧪 ALKANE
    - 🪙 RUNE
    - ⚙️ VM (Virtual Machine)
    - ⛽ FUEL
    - 💾 CACHE
    - 🌐 NETWORK
  - Convenient macros for each log type:
    - `log_info!()`, `log_success!()`, `log_warning!()`, `log_error!()`
    - `log_debug!()` (only in debug builds with `debug-log` feature)
    - `log_block!()`, `log_tx!()`, `log_alkane!()`, `log_rune!()`
    - `log_vm!()`, `log_fuel!()`, `log_cache!()`, `log_network!()`
  - Tree-style hierarchical logging with `LogTree` struct
  - Uses `metashrew_core::println!` for WASM compatibility

#### `/crates/protorune/src/logging.rs`
- **Purpose**: Centralized logging utilities for the protorune crate
- **Features**:
  - Similar emoji-prefixed log styles tailored to protorune operations:
    - ℹ️ INFO
    - ✅ SUCCESS
    - ⚠️ WARNING
    - ❌ ERROR
    - 🔍 DEBUG
    - 📦 BLOCK
    - 💳 TX
    - 🔮 PROTORUNE
    - 🪙 RUNE
    - 💎 PROTOSTONE
    - ⚖️ BALANCE
    - 🔥 BURN
  - Corresponding macros for each log type
  - Tree-style hierarchical logging support
  - WASM-compatible via `metashrew_core::println!`

### 2. WASM Logging Interceptor

#### `/crates/rockshrew-mono/src/lib.rs`
- **Enhancement**: Added logging interceptor to `MetashrewRuntime::load()` calls
- **Implementation**:
  ```rust
  // Setup WASM logging interceptor with enhanced formatting
  let logging_interceptor: Box<dyn FnMut(String) + Send> = Box::new(|msg: String| {
      info!("🔮 [WASM] {}", msg);
  });
  
  let runtime = MetashrewRuntime::load(
      args.indexer.clone(), 
      adapter, 
      engine, 
      Some(logging_interceptor)
  ).await?;
  ```
- **Result**: All WASM runtime logs are now prefixed with 🔮 [WASM] for easy identification
- **Applied to**: Both fork and non-fork runtime initialization paths

### 3. Updated Logging in Core Indexers

#### `/crates/alkanes/src/indexer.rs`
- **Enhanced block indexing logging**:
  - Added `log_block!()` at start of block indexing
  - Added `log_network!()` for genesis block processing
  - Added `log_success!()` after successful block indexing
  - Added `log_cache!()` to report cache statistics
  - Changed error logging from `println!()` to `log_error!()`

#### `/crates/protorune/src/lib.rs`
- **Enhanced protorune processing logging**:
  - Changed duplicate rune name warnings from `println!()` to `log_warning!()`
  - Changed rune unlock errors from `println!()` to `log_error!()`
  - Changed transaction processing errors from `println!()` to `log_error!()`
  - Changed blacklisted transaction notices from `println!()` to `log_warning!()`
  - Changed critical protostone decode errors from `println!()` to `log_error!()`
  - Added `log_debug!()` for verbose debugging information

### 4. Module Registration

Both crates now include the logging module in their module tree:
- `/crates/alkanes/src/lib.rs`: Added `pub mod logging;`
- `/crates/protorune/src/lib.rs`: Added `pub mod logging;`

## Benefits

1. **Consistency**: Uniform logging style across all indexing operations
2. **Visual Clarity**: Emoji prefixes make log scanning much easier
3. **Context Awareness**: Different log types for different subsystems
4. **WASM Integration**: Clear distinction between native and WASM logs with 🔮 [WASM] prefix
5. **Debug Control**: Debug logs can be toggled with feature flags
6. **Tree View Support**: Hierarchical logging for complex operations
7. **WASM Compatibility**: Uses `metashrew_core::println!` which works in both WASM and native contexts

## Example Output

With these changes, log output will look like:

```
📦 [BLOCK] Indexing block at height 840000
🌐 [NETWORK] Processing genesis block
✅ [SUCCESS] Block 840000 indexed successfully
💾 [CACHE] Cached wallet responses for 42 addresses
🔮 [WASM] Indexing transaction 1/150
🔮 [WASM] Processing alkane message for contract 123:456
⚠️ [WARNING] Duplicate rune name EXAMPLE with rune id 789:10 - skipping etching
```

## Compilation Status

✅ All changes compile successfully:
- `cargo check -p alkanes --all-features` ✅
- `cargo check -p protorune --all-features` ✅
- `cargo check -p rockshrew-mono` ✅
- `cargo build --release -p alkanes --target wasm32-unknown-unknown` ✅
- `cargo build --release -p protorune --target wasm32-unknown-unknown` ✅

## Important Implementation Details

### Macro Export Pattern

The logging macros use `#[macro_export]` which exports them to the crate root automatically. This means:

1. **In the defining crate** (alkanes, protorune): Macros are available throughout the crate without imports
2. **From external crates**: Would need to import like `use alkanes::log_block;`
3. **Within modules**: Can be used directly after the logging module is declared

Each macro uses this pattern to ensure proper scoping:
```rust
#[macro_export]
macro_rules! log_block {
    ($($arg:tt)*) => {{
        use $crate::logging::{log_message, LogStyle};
        log_message(LogStyle::BLOCK, format!($($arg)*));
    }};
}
```

The `use $crate::logging::...` inside the macro ensures the correct paths are resolved regardless of where the macro is called from.

## Future Enhancements

Potential areas for further logging improvements:
1. Add colored output for terminal display (when running outside WASM)
2. Expand tree-view logging for complex VM execution traces
3. Add performance metrics logging (timing, gas usage, etc.)
4. Create structured logging for machine-readable output
5. Add log aggregation and filtering utilities

## Usage

To use the new logging system in code:

```rust
// In alkanes crate
use crate::{log_block, log_success, log_error, log_cache};

log_block!("Processing block at height {}", height);
log_success!("Operation completed successfully");
log_error!("Failed to process transaction: {}", error);

// In protorune crate  
use crate::{log_protorune, log_warning, log_rune};

log_protorune!("Processing protorune message");
log_warning!("Invalid rune configuration detected");
log_rune!("Etching rune {} at block {}", name, block);
```

## Notes

- The logging system is designed to work seamlessly in both WASM and native contexts
- Debug logs are only compiled in when the `debug-log` feature is enabled for alkanes, or in debug builds for protorune
- The logging interceptor in rockshrew-mono ensures all WASM output is properly tagged and routed through the native logging system
- All macros use standard Rust formatting syntax (same as `println!` and `format!`)
