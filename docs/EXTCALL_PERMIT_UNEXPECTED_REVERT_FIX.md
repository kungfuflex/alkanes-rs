# Extcall Graceful Error Handling Fix

## Summary

Fixed a critical bug in the alkanes runtime where external calls (`__call`, `__staticcall`, `__delegatecall`) to contracts that revert would propagate the revert to the parent execution context, instead of returning a negative error code that could be handled gracefully by the caller.

## Problem

When a contract made an external call (staticcall, call, or delegatecall) to another contract that reverted (e.g., by calling an unimplemented opcode or explicitly reverting), the entire parent execution would fail with "ALKANES: revert", even though the external call API is designed to return negative values on failure.

### Expected Behavior (as per API design)

According to the host function signatures:
```typescript
// Returns length of return data if >= 0, error code if < 0
__staticcall(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64): i32
__call(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64): i32
__delegatecall(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64): i32
```

The caller should be able to check:
```typescript
const result = __staticcall(...);
if (result < 0) {
    // Handle the error gracefully
    // returndata is empty
} else {
    // Success - result is the size of return data
    // Use __returndatacopy to retrieve it
}
```

### Actual Behavior (before fix)

Any revert in the child contract would cause the entire parent execution to abort with "ALKANES: revert", making it impossible to:
- Query opcodes that might not be implemented
- Handle errors gracefully
- Build resilient contracts that can recover from call failures

### Root Cause

In `/data/alkanes-rs/crates/alkanes/src/vm/host_functions.rs`, the `extcall()` function at line 805 used the `?` operator on `run_after_special()`:

```rust
// OLD CODE (buggy)
let (response, gas_used) = run_after_special(
    Arc::new(Mutex::new(subcontext.clone())),
    binary_rc,
    start_fuel,
)?;  // ← This ? propagates errors upward instead of returning -1
```

When the child contract reverted, `run_after_special()` would return `Err(...)`, and the `?` operator would propagate this error up the call stack:

1. `run_after_special()` returns `Err` → 
2. `extcall()` returns `Err` (due to `?`) → 
3. `handle_extcall()` catches it and calls `_handle_extcall_abort()` → 
4. Sets `had_failure = true` → 
5. `instance.execute()` checks `had_failure` and returns `Err(anyhow!("ALKANES: revert"))`

This violated the design intent where extcalls should return negative values for graceful error handling.

## Solution

Modified `extcall()` to catch errors from `run_after_special()` using a `match` statement instead of the `?` operator. When the child contract fails, we now:

1. Set `returndata` to empty (`vec![]`)
2. Add a trace event recording the failure
3. Return `-1` to indicate failure to the caller

### Code Changes

File: `/data/alkanes-rs/crates/alkanes/src/vm/host_functions.rs`

**Before (lines 805-810):**
```rust
// Run the call in a new context
let (response, gas_used) = run_after_special(
    Arc::new(Mutex::new(subcontext.clone())),
    binary_rc,
    start_fuel,
)?;
```

**After (lines 805-837):**
```rust
// Run the call in a new context
// Handle both success and controlled failures (child contract reverts)
let (response, gas_used) = match run_after_special(
    Arc::new(Mutex::new(subcontext.clone())),
    binary_rc,
    start_fuel,
) {
    Ok((resp, fuel)) => (resp, fuel),
    Err(e) => {
        // Child contract reverted or encountered an error
        // This is a controlled failure - return empty data and set returndata
        // The caller can check the negative return value to detect failure
        #[cfg(feature = "debug-log")]
        {
            println!("extcall: child contract failed: {:?}", e);
        }
        
        {
            let mut context_guard = caller.data_mut().context.lock().unwrap();
            // Set returndata to empty to indicate failure
            context_guard.returndata = vec![];
            
            // Add trace event for the failure
            let mut return_context = TraceResponse::default();
            return_context.fuel_used = 0;
            context_guard
                .trace
                .clock(TraceEvent::ReturnContext(return_context));
        }
        
        // Return -1 to indicate failure (negative value signals error to caller)
        return Ok(-1);
    }
};
```

## Testing

### Test Case: Calling Unimplemented Opcode

Created test program: `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-test-unimplemented-staticcall/`

**Test Scenario:**
```typescript
// Call alkane 2:0, opcode 102 (GetCap) which is NOT implemented
const result = __staticcall(cellpack, emptyParcel, emptyStorage, 10000);

if (result < 0) {
    // Error handling now works!
    return error_response;
} else {
    // Success case
    const data = new ArrayBuffer(result);
    __returndatacopy(data);
    return data;
}
```

**Before Fix:**
```
Error: Alkanes error: Failed to parse tx_script response
(Server returned: "ALKANES: revert")
```

**After Fix:**
The staticcall returns `-1`, allowing the caller to handle the error gracefully and continue execution.

### Verified Patterns

1. ✅ **Direct staticcall to unimplemented opcode** - Returns `-1` instead of reverting
2. ✅ **Recursive calls with errors** - Parent can catch child failures
3. ✅ **CloneFuture (5:n) sandboxing** - Errors in cloned contracts are catchable
4. ✅ **Multiple staticcalls in sequence** - One failure doesn't abort subsequent calls

## Impact

### Benefits

1. **Resilient Contracts**: Contracts can now gracefully handle call failures without aborting
2. **Metadata Enrichment**: Can safely query view opcodes that might not be implemented
3. **Error Recovery**: Contracts can implement fallback logic when calls fail
4. **Compatibility**: Aligns runtime behavior with the documented API contract

### Use Cases Enabled

1. **Alkane Reflection**: Query standard view opcodes (GetName, GetSymbol, etc.) on any alkane, handling missing opcodes gracefully
2. **Safe Probing**: Test if a contract implements certain opcodes without risking revert
3. **Graceful Degradation**: Provide default values when optional features aren't available
4. **Sandboxed Execution**: Execute untrusted code and handle failures without crashing

### Example: Alkane Metadata Enrichment

```typescript
export function enrichAlkane(targetBlock: u64, targetTx: u64): AlkaneReflection {
    const reflection = new AlkaneReflection();
    
    // Try to get name (opcode 99)
    const nameResult = __staticcall(buildCellpack(targetBlock, targetTx, 99), ...);
    if (nameResult >= 0) {
        const nameData = new ArrayBuffer(nameResult);
        __returndatacopy(nameData);
        reflection.name = nameData;
    } // else: name not implemented, leave empty
    
    // Try to get symbol (opcode 100)
    const symbolResult = __staticcall(buildCellpack(targetBlock, targetTx, 100), ...);
    if (symbolResult >= 0) {
        const symbolData = new ArrayBuffer(symbolResult);
        __returndatacopy(symbolData);
        reflection.symbol = symbolData;
    } // else: symbol not implemented, leave empty
    
    // Continue with other opcodes...
    return reflection;
}
```

## Backwards Compatibility

**This fix is backwards compatible:**

- Contracts that don't make external calls are unaffected
- Contracts that only call implemented opcodes see no behavioral change (still return positive values)
- Contracts making calls that previously would have reverted will now receive `-1` instead
  - If they don't check the return value, they'll get empty returndata (safe)
  - If they do check, they can now handle errors gracefully (improvement)

## Related Files

### Modified
- `/data/alkanes-rs/crates/alkanes/src/vm/host_functions.rs` - Core fix in `extcall()` function

### Test Files
- `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-test-unimplemented-staticcall/` - Test program demonstrating the fix
- `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/asc/reflect-alkane/` - Real-world use case: alkane metadata reflection

### Related
- `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly/alkanes/runtime.ts` - Host function type definitions
- `/data/alkanes-rs/crates/alkanes/src/vm/instance.rs` - Execution flow and `had_failure` handling

## Build Instructions

```bash
cd /data/alkanes-rs
cargo build --release --bin alkanes-cli
```

The fix is in the `alkanes` crate which is a dependency of `alkanes-cli`, so rebuilding the CLI will include the fix.

## Deployment

After building, deploy the updated binary to:
- Regtest indexer at `https://regtest.subfrost.io/v4/subfrost`
- Any other alkanes indexers/nodes that execute tx-scripts

## Author

- Issue discovered during implementation of alkane metadata reflection feature
- Fix implemented: 2025-12-01
- Tested and verified on regtest

## References

- Original design intent: External calls should return negative values on failure
- API documentation: `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/asc/alkanes-asm-common/assembly/alkanes/runtime.ts`
- Similar pattern in EVM: `CALL`, `STATICCALL`, `DELEGATECALL` return 0 on failure, 1 on success
