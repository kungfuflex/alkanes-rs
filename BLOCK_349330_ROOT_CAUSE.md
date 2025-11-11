# Block 349330 Memory Fault - Root Cause Analysis

## Summary

Block 349330 is a **small, normal block** (1624 bytes, 1 coinbase transaction) that parses successfully but causes a memory fault during WASM execution.

## Block Details

```
Height: 349330
Hash: 0000000005b7e1000ce3fa26a0f453068863a84fd6ea4441092edbcee9a0cd69
Size: 1624 bytes
Transactions: 1 (coinbase only)
Outputs: 2 (one P2PKH, one P2SH)
```

This is NOT a large or unusual block. The error is NOT caused by block size.

## The Error

```
[ERROR] WASM _start function failed: error while executing at wasm backtrace:
        0: 0x1b319f - alkanes.wasm!metashrew_core::input
        1: 0x4487f - alkanes.wasm!_start
    Caused by:
        memory fault at wasm address 0xff897608 in linear memory of size 0xe10000
```

Translation:
- Trying to access: `0xff897608` = 4,287,198,728 bytes ≈ 4.2 GB
- Available memory: `0xe10000` = 14,745,600 bytes ≈ 14 MB
- **The WASM is trying to access memory 290x larger than available!**

## Root Cause

The error occurs in `metashrew_core::input()` at this line:

```rust
// crates/metashrew-core/src/lib.rs:311
__load_input(to_ptr(&mut buffer) + 4);
```

Where `to_ptr` is defined as:

```rust
// crates/metashrew-support/src/compat.rs:67-68
pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    return v.as_mut_ptr() as usize as i32;
}
```

### The Bug

The cast chain `as usize as i32` is **dangerous** on 64-bit systems:

1. Rust allocates a `Vec<u8>` in WASM linear memory
2. Gets the pointer as `usize` (could be any 64-bit address)
3. Casts to `i32` (32-bit signed integer)
4. **If the address is > 2^31-1, this wraps to a negative number**
5. When interpreted as unsigned memory address, it points to invalid high memory

### Why Block 349330?

The issue is **non-deterministic** - it depends on where WASM allocates memory:

- If `Vec` is allocated at a low address (< 2^31), it works fine
- If `Vec` is allocated at a high address (≥ 2^31), the cast wraps and breaks
- Block 349330 happens to trigger an allocation at a high address

### Specific Address Analysis

```
0xff897608 = 4,287,198,728
0xe10000   = 14,745,600

0xff897608 interpreted as i32 = -7,768,568 (negative!)
```

This is a **negative pointer** that got reinterpreted as a large positive address.

## The Fix

### Option 1: Safe Pointer Handling (Recommended)

Change `to_ptr` to check bounds:

```rust
pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    let ptr = v.as_mut_ptr() as usize;
    if ptr > i32::MAX as usize {
        panic!("Vector allocated beyond i32 address space: 0x{:x}", ptr);
    }
    ptr as i32
}
```

But this will still fail on high addresses. **Better solution:**

### Option 2: Use u32 Instead of i32 (Better)

WASM memory is always positive, so pointers should be `u32`:

```rust
pub fn to_ptr(v: &mut Vec<u8>) -> u32 {
    let ptr = v.as_mut_ptr() as usize;
    assert!(ptr <= u32::MAX as usize, "Pointer out of WASM address space");
    ptr as u32
}
```

Then update all WASM imports to expect `u32` instead of `i32`.

### Option 3: Increase WASM Memory Allocation Strategy

Configure WASM to allocate from low addresses first, avoiding the wraparound:

```rust
// In metashrew-runtime configuration
const INITIAL_MEMORY_PAGES: u32 = 256;  // 16 MB
const MAX_MEMORY_PAGES: u32 = 16384;    // 1 GB
```

This doesn't fix the root cause but makes it less likely to trigger.

### Option 4: Use WASM-Managed Memory Allocator

Use `wasm-bindgen` or similar to ensure allocations stay in valid range:

```rust
use wasm_bindgen::memory;

pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    // Ensure allocation is in WASM linear memory
    let ptr = v.as_mut_ptr() as usize;
    assert!(ptr < memory().data_size(), "Pointer outside WASM memory");
    ptr as i32
}
```

## Why This Wasn't Caught Earlier

1. **Intermittent**: Only triggers when WASM allocates at high addresses
2. **Block-dependent**: Different blocks cause different allocation patterns
3. **Testing gaps**: Most test blocks (0, 250, 286639, 407) didn't hit this case
4. **Silent casting**: Rust allows `as` casts that truncate without warning

## Verification

To confirm this is the issue:

1. Add logging to `to_ptr`:
   ```rust
   pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
       let ptr = v.as_mut_ptr() as usize;
       log::debug!("to_ptr: vec at 0x{:x}, casting to i32: {}", ptr, ptr as i32);
       ptr as i32
   }
   ```

2. Run block 349330 again and check if the logged address matches `0xff897608`

## Recommended Action

**Implement Option 2** (use `u32` for pointers):

1. Change `to_ptr` return type to `u32`
2. Update all call sites
3. Update WASM host functions to accept `u32` for memory addresses
4. Add assertions to catch out-of-bounds pointers early
5. Test with all existing blocks (0, 250, 286639, 407, 349330)

This is the correct fix because:
- WASM linear memory addresses are always unsigned
- `i32` was the wrong type from the start
- This prevents the wraparound bug permanently
- Aligns with WASM spec (memory is 0 to 4GB unsigned)

## Impact

**Current**: Intermittent failures on random blocks when memory allocation is unlucky

**After fix**: Reliable processing of all blocks, with early detection of any true memory issues

## Files to Modify

1. `crates/metashrew-support/src/compat.rs` - Change `to_ptr` signature
2. `crates/metashrew-core/src/lib.rs` - Update call sites
3. `crates/metashrew-runtime/src/runtime.rs` - Update host function signatures if needed
4. All WASM import definitions
