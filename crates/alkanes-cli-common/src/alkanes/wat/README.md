# WebAssembly Transaction Scripts (WAT Templates)

This directory contains WebAssembly Text (WAT) templates that can be compiled to WASM and executed on-chain to perform complex operations in a single transaction.

## Overview

Instead of making multiple RPC calls to gather information and compute optimal paths, we can embed WASM bytecode directly in a transaction using the `--envelope` flag. This WASM code executes on-chain and can make `__staticcall` invocations to query pools, calculate paths, and return optimized results.

## Architecture

### Context Inputs

WASM scripts receive inputs through the `Context` object:

```rust
pub struct Context {
    pub myself: AlkaneId,      // The alkane being executed (simulation target)
    pub caller: AlkaneId,      // The caller
    pub vout: u32,             // Output index
    pub incoming_alkanes: AlkaneTransferParcel,  // Alkanes sent with the call
    pub inputs: Vec<u128>,     // Input parameters
}
```

The `inputs` array is where we pass parameters to our WASM scripts.

### Runtime Functions

WASM scripts have access to these runtime functions:

- `__request_context()` - Get size of context data
- `__load_context(output: i32)` - Load context into memory
- `__staticcall(cellpack, alkanes, checkpoint, fuel)` - Make read-only calls to other contracts
- `__returndatacopy(output: i32)` - Copy return data from last call
- `__log(value: i32)` - Log data for debugging

### Memory Layout

Scripts use a structured memory layout:
```
0-1023:     Context buffer
1024-2047:  Working buffer for cellpacks
2048-4095:  Pool list and details
4096-8191:  Path search working memory  
8192+:      Response buffer
```

## Available Scripts

### `optimize_swap_path.wat`

Finds the optimal swap path between two tokens using on-chain liquidity data.

**Context Inputs:**
- `inputs[0]` - factory_block
- `inputs[1]` - factory_tx
- `inputs[2]` - input_token_block
- `inputs[3]` - input_token_tx
- `inputs[4]` - output_token_block
- `inputs[5]` - output_token_tx
- `inputs[6]` - input_amount
- `inputs[7]` - max_hops (1-4)
- `inputs[8]` - min_liquidity_threshold

**Returns:**
```
[path_length(16 bytes)]
[token0_block(16)]
[token0_tx(16)]
[token1_block(16)]
[token1_tx(16)]
...
```

**Usage:**
```bash
# Compile WAT to WASM
alkanes-cli tx-script compile optimize_swap_path.wat -o path_finder.wasm

# Use in simulation
alkanes-cli alkanes simulate 1:0:0 \\
  --inputs 4,65522,2,0,32,0,500000,2,1000 \\
  --envelope path_finder.wasm
```

## Usage with `alkanes swap`

The swap command has an experimental flag to use WASM-based path optimization:

```bash
alkanes-cli alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --experimental-optimize-path-finding
```

This will:
1. Compile `optimize_swap_path.wat` to WASM
2. Execute it with `--envelope` in a single simulate call
3. Parse the returned optimal path
4. Use that path for the swap

## Compilation

WAT files are compiled using the `wat` crate, which is no_std compatible:

```rust
use alkanes_cli_common::alkanes::wat;

// Compile built-in template
let wasm_bytes = wat::compile_wat_to_wasm(wat::OPTIMIZE_SWAP_PATH_WAT)?;

// Compile custom WAT file
let custom_wat = std::fs::read_to_string("my_script.wat")?;
let wasm_bytes = wat::compile_wat_to_wasm(&custom_wat)?;
```

## Development Tips

### Testing WAT Scripts

1. **Use logging:** Call `__log` with pointers to memory regions to inspect values
2. **Start simple:** Begin with a script that just returns the input tokens
3. **Incremental complexity:** Add one feature at a time (pool fetching, then 1-hop, then 2-hop, etc.)
4. **Memory alignment:** Ensure all u128 values are 16-byte aligned

### Common Patterns

**Loading a u128 from context inputs:**
```wat
(func $load_input (param $index i32) (result i64)
  ;; Calculate offset: context_base + 80 + (index * 16)
  (i32.add
    (i32.add (global.get $context_ptr) (i32.const 80))
    (i32.mul (local.get $index) (i32.const 16)))
  (i64.load))
```

**Building a cellpack:**
```wat
;; Cellpack format: [target_block][target_tx][inputs_count][input0]...
(call $store_u128 (i32.const 1024) (global.get $target_block))
(call $store_u128 (i32.const 1040) (global.get $target_tx))
(call $store_u128 (i32.const 1056) (i64.const 1))  ;; inputs_count
(call $store_u128 (i32.const 1072) (i64.const 3))  ;; opcode
```

**Making a staticcall:**
```wat
(call $__staticcall 
  (i32.const 1024)                ;; cellpack pointer
  (i32.const 0)                   ;; no incoming alkanes
  (i32.const 0)                   ;; no checkpoint
  (i64.const 0xFFFFFFFFFFFFFFFF)) ;; max fuel
```

## Future Enhancements

- [ ] Full multi-hop path finding (1-4 hops)
- [ ] Liquidity filtering in WASM
- [ ] Price impact calculations
- [ ] MEV protection (slippage + deadline)
- [ ] Path caching across calls
- [ ] Gas optimization profiling

## References

- [WebAssembly Text Format Spec](https://webassembly.github.io/spec/core/text/index.html)
- [wat crate documentation](https://docs.rs/wat/)
- [alkanes-runtime source](../../../alkanes-runtime/)
