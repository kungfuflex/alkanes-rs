# Block 349330 - New Error After Memory Fix

## Status: Memory Fault Fixed ✓, New Parser Error Found

### What Changed

**BEFORE (Memory Fault):**
```
[ERROR] memory fault at wasm address 0xff897608 in linear memory of size 0xe10000
```
❌ Couldn't even load block data into WASM

**AFTER (Parser Error):**
```
[DEBUG] [__load_input] Successfully wrote data to memory
[WASM]: height is 349330
[ERROR] wasm trap: wasm `unreachable` instruction executed
        at _start (unwrap_failed)
```
✓ Block data loads successfully  
❌ ZcashBlock::parse() is returning an error

### Progress Made

1. ✅ **Fixed pointer wraparound bug** - Changed `i32` → `u32` for all WASM pointers
2. ✅ **Block data loads into WASM** - No more memory faults
3. ✅ **Indexer starts execution** - Gets to parsing stage
4. ❌ **New issue**: `ZcashBlock::parse()` fails with an error

### The New Error

Location: `crates/alkanes/src/lib.rs:351-352`

```rust
let block: Block = crate::zcash::ZcashBlock::parse(&mut Cursor::<Vec<u8>>::new(reader.to_vec()))
    .unwrap()  // ← This unwrap is panicking
    .block;
```

### Analysis

The block parses successfully in our standalone test:
```rust
// This works:
let block_bytes = hex::decode(...).expect(...);
let mut cursor = Cursor::new(block_bytes.clone());
let zblock = ZcashBlock::parse(&mut cursor).expect(...);  // ✓ Success
```

But fails in `_start()`:
```rust
// This fails:
let data = input();  // 1628 bytes (4 height + 1624 block)
let reader = &data[4..];  // 1624 bytes of block data
let block = ZcashBlock::parse(&mut Cursor::new(reader.to_vec()))
    .unwrap();  // ❌ Panic!
```

### Possible Causes

1. **Data corruption**: The block data from `input()` might be different from the hex file
2. **Cursor position**: Maybe the cursor isn't starting at position 0
3. **Network configuration**: The Zcash network config might not be set correctly
4. **WASM vs native parsing**: Some difference in how parsing works in WASM
5. **Extra bytes**: Maybe there's a newline or extra data at the end

### Next Steps

To debug this, we need to:

1. **Add detailed error logging** to `_start()`:
   ```rust
   let block_result = ZcashBlock::parse(&mut Cursor::new(reader.to_vec()));
   match block_result {
       Ok(zblock) => {
           println!("[WASM] ZcashBlock parsed successfully");
           zblock.block
       }
       Err(e) => {
           println!("[WASM] ZcashBlock parse failed: {:?}", e);
           println!("[WASM] Block data length: {}", reader.len());
           println!("[WASM] First 100 bytes: {}", hex::encode(&reader[..100.min(reader.len())]));
           panic!("ZcashBlock parse failed");
       }
   }
   ```

2. **Compare data** between the test and runtime:
   - Check if `reader` in WASM matches the hex file
   - Verify block data isn't corrupted
   - Check for trailing bytes

3. **Test with other Zcash blocks**:
   - Try blocks 0, 250, 286639 to see if they work
   - This will tell us if it's block-specific or a general issue

4. **Check network configuration**:
   - Ensure Zcash network is configured before parsing
   - Verify `configure_network()` is called (it is, line 345 removed this)

### Recommendation

Replace the `unwrap()` with proper error handling to see the actual error message:

```rust
#[cfg(feature = "zcash")]
let block: Block = match crate::zcash::ZcashBlock::parse(&mut Cursor::<Vec<u8>>::new(reader.to_vec())) {
    Ok(zblock) => {
        println!("[WASM] Successfully parsed Zcash block at height {}", height);
        zblock.block
    }
    Err(e) => {
        println!("[WASM] ERROR: Failed to parse Zcash block: {:?}", e);
        println!("[WASM] Block data length: {} bytes", reader.len());
        println!("[WASM] First 200 bytes of block: {}", 
                 hex::encode(&reader[..200.min(reader.len())]));
        panic!("ZcashBlock parsing failed at height {}: {:?}", height, e);
    }
};
```

This will show us the actual parsing error and help identify the root cause.

### Summary

- ✅ Memory fault bug: **FIXED**
- ✅ Pointer wraparound: **FIXED**  
- ✅ Block loading: **WORKS**
- ❌ Block parsing: **NEW ISSUE**

This is progress! We've fixed the original bug and uncovered a second issue in the Zcash parser logic.
