# Experimental AssemblyScript WASM - Ready for Testing ✅

## Summary

Successfully implemented and compiled the `--experimental-asm` flag for `alkanes get-all-pools` that uses our fully tested AssemblyScript serialization infrastructure.

## What Was Built

### 1. Complete Serialization Infrastructure ✅
- **StorageMap** - 4/4 tests passing, verified byte-for-byte with Rust
- **AlkaneTransferParcel** - 6/6 tests passing, verified byte-for-byte with Rust  
- **ExtendedCallResponse** - 6/6 tests passing, uses proven building blocks
- **Total:** 16/16 tests passing

### 2. get-all-pools WASM ✅
**File:** `crates/alkanes-cli-common/src/alkanes/asc/get-all-pools/assembly/index.ts`

```typescript
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  const response = new ExtendedCallResponse();
  
  // Call factory to get all pools (opcode 3 = GET_ALL_POOLS)
  const factoryResult = responder.staticcall(FACTORY, GET_ALL_POOLS_OPCODE);
  
  // Check if call succeeded
  if (factoryResult != null) {
    response.setData(factoryResult.data);
  }
  
  // Finalize and return
  const result = response.finalize();
  return changetype<i32>(changetype<usize>(result));
}
```

**Compiled:** `get-all-pools/build/release.wasm` (9,895 bytes)

### 3. CLI Integration ✅
**Command:**
```bash
alkanes-cli alkanes get-all-pools --experimental-asm
```

**Implementation:**
- Loads pre-compiled WASM from `get-all-pools/build/release.wasm`
- Calls `provider.tx_script(wasm_bytes, inputs, None)`
- Parses ExtendedCallResponse format from returned data
- Displays pool list

## How It Works

### Execution Flow

1. **CLI invokes tx_script:**
   ```rust
   let wasm_bytes = include_bytes!("...get-all-pools/build/release.wasm");
   let response_data = system.provider().tx_script(wasm_bytes, vec![], None).await?;
   ```

2. **WASM executes __execute():**
   - Creates `AlkaneResponder` to access alkanes runtime
   - Makes staticcall to factory (4:65522) with opcode 3 (GET_ALL_POOLS)
   - Wraps result in `ExtendedCallResponse`
   - Calls `finalize()` to serialize

3. **finalize() serialization:**
   ```typescript
   const alkanesBytes = this.alkanes.serialize();  // Empty: 16 bytes
   const storageBytes = this.storage.serialize();  // Empty: 4 bytes
   // Total: alkanes + storage + data
   ```

4. **Runtime returns data:**
   - ExtendedCallResponse format: `[alkanes(16)][storage(4)][pool_data]`
   - Pool data from factory: `[count(16)][pool0(32)][pool1(32)]...`

5. **CLI parses response:**
   ```rust
   // Skip ExtendedCallResponse overhead (20 bytes)
   let pool_count = u128::from_le_bytes(response_data[32..48])?;
   // Parse pools...
   ```

## Response Format

### ExtendedCallResponse Structure
```
Offset  | Size | Field
--------|------|-------
0       | 16   | alkanes.count (u128) = 0
16      | 4    | storage.count (u32) = 0
20      | 16   | pool_count (u128)
36      | 32   | pool[0] (block:16 + tx:16)
68      | 32   | pool[1]
...     | ...  | ...
```

### Example Response (Empty pools)
```
Hex: 00000000000000000000000000000000  // alkanes count = 0
     00000000                          // storage count = 0
     00000000000000000000000000000000  // pool count = 0
Total: 20 bytes
```

### Example Response (2 pools)
```
Hex: 00000000000000000000000000000000  // alkanes count = 0
     00000000                          // storage count = 0
     02000000000000000000000000000000  // pool count = 2
     0500000000000000000000000000000000 // pool[0].block = 5
     0a00000000000000000000000000000000 // pool[0].tx = 10
     0600000000000000000000000000000000 // pool[1].block = 6
     0c00000000000000000000000000000000 // pool[1].tx = 12
Total: 116 bytes
```

## Testing Status

### Unit Tests ✅
All 16 serialization tests pass:
```bash
cd crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common
node test/run-serialization-tests.js        # StorageMap + Parcel
node test/run-extended-response-tests.js    # ExtendedCallResponse
```

### Compilation ✅
```bash
cd crates/alkanes-cli-common/src/alkanes/asc/get-all-pools
npm run build
# Output: build/release.wasm (9,895 bytes)
```

### CLI Build ✅
```bash
cargo build --bin alkanes-cli
# Successfully builds with embedded WASM
```

### Runtime Test ⏳
Requires metashrew server running:
```bash
alkanes-cli alkanes get-all-pools --experimental-asm
# Currently fails: metashrew server not running
```

## Next Steps

### To Test End-to-End

1. **Start metashrew server:**
   ```bash
   # Start your local metashrew instance
   ```

2. **Run command:**
   ```bash
   alkanes-cli alkanes get-all-pools --experimental-asm
   ```

3. **Expected output:**
   ```
   🚀 Using experimental AssemblyScript WASM...
      Loaded WASM (9895 bytes)
      Calling factory 4:65522...
      ✅ Got response: X bytes
      Response (hex): 00000000...
      📊 Pool count: N
      🎯 Parsed N pools:
         1. block:tx
         2. block:tx
         ...
   ```

### To Debug

If issues occur, check:
1. **WASM size** - Should be ~10KB
2. **Response format** - First 20 bytes should be ExtendedCallResponse header
3. **Pool count** - At offset 32 (after 16-byte alkanes + 4-byte storage)
4. **Pool data** - Each pool is 32 bytes (16 block + 16 tx)

## Architecture Benefits

### Why This Approach Works

1. **Proven building blocks** - StorageMap and AlkaneTransferParcel verified separately
2. **Simple composition** - ExtendedCallResponse just concatenates serialized sections
3. **No memory tricks** - Uses Arrays during construction, ArrayBuffer only in final step
4. **Rust-compatible** - Exact byte-for-byte match with Rust implementations
5. **Single RPC call** - Gets all pools in one tx_script execution

### Performance

- **Current (without flag):** N+1 RPC calls (1 for count, N for pools)
- **With --experimental-asm:** 1 RPC call (all data in tx_script)
- **WASM size:** 9.9 KB (small enough for efficient transmission)
- **Serialization:** O(n) where n = number of pools

## Files Modified

### Implementation
- `crates/alkanes-cli-common/src/alkanes/asc/get-all-pools/assembly/index.ts` - Main logic
- `crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly/alkanes/types.ts` - ExtendedCallResponse
- `crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly/alkanes/responder.ts` - Use new serialization
- `crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly/index.ts` - Export types

### CLI
- `crates/alkanes-cli/src/commands.rs` - Add --experimental-asm flag
- `crates/alkanes-cli/src/main.rs` - Handle flag, call tx_script

### Build Output
- `crates/alkanes-cli-common/src/alkanes/asc/get-all-pools/build/release.wasm` - Compiled WASM

## Status

✅ **Infrastructure Complete** - All serialization components tested and verified
✅ **WASM Compiled** - get-all-pools.wasm built successfully
✅ **CLI Integrated** - Flag added, code paths implemented
⏳ **Runtime Test Pending** - Requires metashrew server

**Ready for integration testing once metashrew server is available!**
