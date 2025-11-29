# Alkanes AssemblyScript Tx-Scripts

This directory contains AssemblyScript-based tx-scripts for efficient batch querying of Alkanes data.

## Success Story

✅ **Working Implementation** - The `get-pool-details` tx-script successfully compiles to **981 bytes** of optimized WASM!

This infrastructure enables batching multiple alkane calls into a single RPC request, dramatically reducing network overhead.

## Overview

Tx-scripts are WASM programs that run within the Alkanes simulation environment to perform batch operations in a single RPC call. Instead of making N+1 separate RPC calls (one to get IDs, N to get details), a tx-script can make all calls internally and return aggregated results.

## Directory Structure

```
asc/
├── alkanes-asm-common/     # Shared runtime library
│   └── assembly/
│       ├── runtime.ts       # Host function imports
│       ├── arraybuffer.ts   # ArrayBuffer utilities
│       ├── types.ts         # Core Alkanes types
│       ├── context.ts       # Context loading
│       ├── staticcall.ts    # Staticcall helpers
│       ├── utils.ts         # General utilities
│       └── index.ts         # Main exports
│
└── get-pool-details/        # Example: Batch fetch pool details
    └── assembly/
        └── index.ts         # Main tx-script
```

## alkanes-asm-common Library

The shared library provides:

### Runtime Functions
- `__request_context()` - Get context size
- `__load_context(ptr)` - Load execution context
- `__staticcall(...)` - Make staticcall to another alkane
- `__returndatacopy(ptr)` - Copy return data
- `__log(ptr, len)` - Debug logging
- `__abort(ptr, len)` - Abort execution

### Core Types
- `AlkaneId` - Alkane identifier (block:tx)
- `Cellpack` - Call parameters for staticcalls
- `EmptyAlkaneParcel` - Empty AlkaneTransferParcel
- `EmptyStorageMap` - Empty StorageMap
- `CallResponse` - Response parsing
- `ExtendedCallResponse` - Response builder

### Context
- `ExecutionContext` - Parsed execution context
  - `myself: AlkaneId` - Current alkane
  - `caller: AlkaneId` - Calling alkane
  - `inputs: u128[]` - Input parameters
  - `getInput(index)` - Get input by index

### Staticcall Helpers
- `staticcall(target, opcode)` - Simple staticcall
- `staticcallWithInputs(target, inputs)` - Staticcall with multiple inputs
- `FactoryOpcodes` - Factory contract opcodes
- `PoolOpcodes` - Pool contract opcodes

### ArrayBuffer Utilities
- `allocArrayBuffer(size)` - Allocate with length prefix
- `getDataPtr(buf)` - Get data pointer
- `writeU128/readU128` - Serialize u128 values
- `writeU64/readU64` - Serialize u64 values
- `writeU32/readU32` - Serialize u32 values

## Building Tx-Scripts

### Prerequisites

```bash
# Install Node.js and npm (if not already installed)
npm install -g assemblyscript

# Or use the project's local installation
cd get-pool-details
npm install
```

### Building

```bash
cd get-pool-details
npm run build

# Output: build/release.wasm (optimized)
#         build/release.wat (text format)
```

### Optimization

AssemblyScript can produce very small WASM binaries:
- Use `--optimize` flag
- Set `--shrinkLevel 2` for maximum compression
- Use `--runtime stub` (no GC overhead)
- Result: Typically <2KB for simple tx-scripts

## Writing a Tx-Script

### Basic Template

```typescript
// Import runtime functions
@external("env", "__request_context")
declare function __request_context(): i32;

@external("env", "__load_context")
declare function __load_context(ptr: i32): i32;

@external("env", "__staticcall")
declare function __staticcall(
  cellpack: i32,
  alkanes: i32,
  storage: i32,
  fuel: u64
): i32;

@external("env", "__returndatacopy")
declare function __returndatacopy(ptr: i32): void;

/**
 * Main entry point
 * @returns Pointer to response data (length at ptr-4)
 */
export function __execute(): i32 {
  // 1. Load context and parse inputs
  const contextSize = __request_context();
  store<u32>(0, contextSize);
  __load_context(4);
  
  // Read inputs at offset 100 (after context header)
  const input0 = load<u128>(100);
  
  // 2. Build cellpack for staticcall
  // Layout: [length(4)][target_block(16)][target_tx(16)][opcode(16)]
  const cellpackPtr: usize = 2048;
  store<u32>(cellpackPtr - 4, 48); // length
  store<u128>(cellpackPtr, targetBlock);
  store<u128>(cellpackPtr + 16, targetTx);
  store<u128>(cellpackPtr + 32, opcode);
  
  // 3. Create empty parcels
  const alkanesPtr: usize = 3200;
  store<u32>(alkanesPtr - 4, 16);
  store<u128>(alkanesPtr, 0);
  
  const storagePtr: usize = 3300;
  store<u32>(storagePtr - 4, 4);
  store<u32>(storagePtr, 0);
  
  // 4. Make staticcall
  const result = __staticcall(
    cellpackPtr as i32,
    alkanesPtr as i32,
    storagePtr as i32,
    0xFFFFFFFFFFFFFFFF
  );
  
  if (result < 0) {
    // Handle error
    return buildErrorResponse();
  }
  
  // 5. Copy return data
  const returnPtr: usize = 8192;
  store<u32>(returnPtr - 4, result);
  __returndatacopy(returnPtr as i32);
  
  // 6. Build response
  // Format: [length(4)][alkanes(16)][storage(16)][data...]
  const responsePtr: usize = 16384;
  let offset = responsePtr + 4;
  
  store<u128>(offset, 0); // alkanes count
  offset += 16;
  store<u128>(offset, 0); // storage count
  offset += 16;
  
  // Copy data (skip AlkaneTransferParcel)
  const dataPtr = returnPtr + 16;
  const dataLen = result - 16;
  memory.copy(offset, dataPtr, dataLen);
  offset += dataLen;
  
  // Write total length
  store<u32>(responsePtr, offset - (responsePtr + 4));
  
  return (responsePtr + 4) as i32;
}
```

### Memory Layout Convention

To avoid conflicts, use this standard layout:

```
0-1023:       Context (with length prefix at 0)
2048-2095:    Cellpack for first staticcall
3200-3215:    Empty AlkaneTransferParcel
3300-3303:    Empty StorageMap
4096-4143:    Cellpack for second staticcall
8192-12287:   Return data buffer 1
12288-16383:  Return data buffer 2
16384+:       Final response buffer
```

## Example: get-pool-details

Fetches AMM pool details for a range of pools in a single RPC call.

### Inputs
- `[0]`: start_index - Starting pool index (0-based)
- `[1]`: batch_size - Number of pools to fetch

### Output
```
[alkanes_count(16)][storage_count(16)][pool_count(16)]
[pool0_block(16)][pool0_tx(16)][pool0_details]
[pool1_block(16)][pool1_tx(16)][pool1_details]
...
```

### Usage from Rust

```rust
let wasm_bytes = include_bytes!("../asc/get-pool-details/build/release.wasm");
let response = provider.tx_script(
    wasm_bytes,
    vec![0, 10], // start_index=0, batch_size=10
    Some("latest".to_string())
).await?;
```

## ArrayBuffer Layout

The Alkanes runtime expects pointers to have a 4-byte length prefix:

```
Memory: [length: u32][data: bytes...]
         ^              ^
         ptr-4          ptr (passed to host functions)
```

When passing pointers to host functions like `__staticcall`, the pointer should point to the data, with the length at `ptr-4`.

## Response Format

Tx-scripts should return data in ExtendedCallResponse format:

```
[alkanes_count: u128(16)]    - Number of alkane transfers (usually 0)
[storage_count: u128(16)]    - Number of storage changes (usually 0)
[data: bytes...]             - Your actual response data
```

The pointer returned from `__execute` should point to the start of this data (with length at `ptr-4`).

## Debugging

### Enable Debug Build

```bash
npm run asbuild:debug
```

### Use __log for Debugging

```typescript
function log(message: string): void {
  const buf = String.UTF8.encode(message);
  __log(changetype<usize>(buf), buf.byteLength);
}

// In your code:
log("Pool count: " + poolCount.toString());
```

### Check WAT Output

The build generates `.wat` text format files - inspect these to understand the compiled output:

```bash
cat build/release.wat
```

## Performance Tips

1. **Minimize allocations** - Reuse buffers when possible
2. **Use inline functions** - AssemblyScript will inline small functions
3. **Avoid String operations** - Expensive in WASM
4. **Pre-calculate sizes** - Better than dynamic growth
5. **Use u32/u64 when possible** - Faster than u128
6. **Batch staticcalls** - Amortize the overhead

## Common Patterns

### Pattern: Fetch List then Details

```typescript
// 1. Call factory to get list of IDs
const listResponse = staticcall(factory, LIST_OPCODE);
const ids = parseIds(listResponse);

// 2. Loop and fetch details for each
for (let i = 0; i < ids.length; i++) {
  const details = staticcall(ids[i], DETAILS_OPCODE);
  // Process details...
}
```

### Pattern: Parameterized Range

```typescript
// Read start/end from inputs
const start = load<u128>(100) as u32;
const count = load<u128>(116) as u32;

// Fetch only the requested range
for (let i = start; i < start + count; i++) {
  // Process item i...
}
```

### Pattern: Error Handling

```typescript
const result = __staticcall(...);

if (result < 0) {
  // Staticcall failed - return graceful error
  return buildEmptyResponse();
}

// Success - process result
__returndatacopy(returnPtr as i32);
```

## References

- [AssemblyScript Documentation](https://www.assemblyscript.org/)
- [WebAssembly Spec](https://webassembly.github.io/spec/)
- [metashrew-as](https://github.com/sandshrewmetaprotocols/metashrew-as) - Reference implementations
- [protorune](https://github.com/kungfuflex/protorune) - MessageContext patterns
