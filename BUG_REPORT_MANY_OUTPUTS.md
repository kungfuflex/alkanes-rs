# Runes Allocation Bug: Remainder Goes to Wrong Output

## Bug Summary

When using the Runes "allocate to each output" pattern (`edict.output == num_outputs`) with OP_RETURN at vout 0, the remainder is allocated to the wrong output due to an off-by-one error.

## Real-World Example

Transaction: `bb910271835329f56a76100d31a65663be5b8b76e60410bb8ef6988401028b8b`

**Setup:**
- Incoming: 8,356.95545699 ALKAMIST
- Edict: `amount=500, output=82` (82 total outputs)
- vout 0: OP_RETURN
- vouts 1-81: Regular outputs

**Expected Behavior:**
- vouts 1-16: Each gets 500 ALKAMIST (16 × 500 = 8,000)
- vout 17: Gets remainder (8,356 - 8,000 = 356.95545699)

**Actual Behavior (BUG):**
- vout 16: Gets 356.95545699 (remainder) ❌
- vout 17: Gets 500 (full amount) ❌

The allocations for vout 16 and 17 are **swapped**!

## Root Cause

Location: `/data/alkanes-rs-v2.1.6/crates/protorune/src/lib.rs:116-123`

```rust
for i in 0..tx.output.len() as u32 {
    let amount_outpoint = std::cmp::min(remaining, amount);
    remaining -= amount_outpoint;  // ❌ BUG: Decrements BEFORE checking OP_RETURN
    if tx.output[i as usize].script_pubkey.is_op_return() {
        continue;  // ❌ Skips OP_RETURN but `remaining` already decremented!
    }
    output.insert(i, amount_outpoint);
}
```

### What's Wrong

1. Loop iterates ALL outputs (including OP_RETURN at vout 0)
2. For vout 0 (OP_RETURN):
   - Calculates `amount_outpoint = 500`
   - **Decrements `remaining`** (remaining = 8356 - 500 = 7856)
   - Then checks `is_op_return()` and skips
   - No runes allocated to vout 0 (correct)
3. For vouts 1-16: Each gets 500, remaining decreases normally
4. For vout 17: `remaining` is now only 356 instead of 856

The bug: We decrement `remaining` for the OP_RETURN output even though we don't allocate anything to it.

## The Fix

Move the `remaining -= amount_outpoint` line AFTER the OP_RETURN check:

```rust
for i in 0..tx.output.len() as u32 {
    // Skip OP_RETURN outputs FIRST
    if tx.output[i as usize].script_pubkey.is_op_return() {
        continue;
    }

    // Now calculate and decrement for non-OP_RETURN outputs only
    let amount_outpoint = std::cmp::min(remaining, amount);
    remaining -= amount_outpoint;
    output.insert(i, amount_outpoint);
}
```

## Test Case

Created test at: `/data/alkanes-rs-v2.1.6/crates/protorune/src/tests/test_many_outputs_bug.rs`

Test name: `test_many_outputs_with_op_return_at_index_0`

This test recreates the bug scenario with:
- 18 total outputs (vout 0 = OP_RETURN, vouts 1-17 regular)
- 8356 runes available
- Edict with `amount=500, output=18` (allocate to all)
- Verifies vout 16 and 17 get correct amounts

### Running the Test

```bash
cd /data/alkanes-rs-v2.1.6
cargo test --package protorune --lib test_many_outputs_with_op_return_at_index_0
```

**Expected Result (with bug):** Test FAILS showing swapped values
**Expected Result (after fix):** Test PASSES

## Impact

This bug affects ANY transaction using the "allocate to all outputs" pattern when:
1. OP_RETURN is at vout 0
2. There are many outputs
3. The last outputs receive the remainder

The remainder gets allocated to the wrong output, causing balance mismatches.

## Files Modified

1. **Test created:** `crates/protorune/src/tests/test_many_outputs_bug.rs`
2. **Test registered:** `crates/protorune/src/tests/mod.rs`
3. **Fix needed:** `crates/protorune/src/lib.rs:116-123`
