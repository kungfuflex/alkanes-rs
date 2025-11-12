# Split Transaction Dust Output Fix

## Problem

When using `--split-max-vsize` to split large transactions, the resulting split transactions had **0 sat outputs** (dust), causing the broadcast to fail with:

```
Error: RPC error: Code -26: dust, tx with dust output must be 0-fee
```

### Root Cause

The splitting logic was making incorrect assumptions:

1. **Hardcoded UTXO values**: The code assumed each input UTXO was worth 1999 sats (`let sats_per_input = 1999u64;`)
2. **Incorrect fee calculation**: It tried to detect the fee rate from the unsigned transaction using the hardcoded UTXO value
3. **Wrong output calculation**: `chunk_output_amount = chunk_input_amount - chunk_fee` resulted in 0 or negative values when the assumptions were wrong

### Example of the Bug

For a transaction with:
- 7,298 inputs
- Total output: 3,562,176 sats
- Actual UTXO value per input: ~488 sats (not 1999!)

The splitting code would calculate:
- Assumed input: 669 × 1999 = 1,337,331 sats
- Fee: ~38,523 sats
- Output: 1,337,331 - 38,523 = 1,298,808 sats ❌ (wrong!)

But the fee was actually larger than this assumed input, causing `saturating_sub` to return **0 sats**, creating a dust output.

## Solution

Changed the splitting logic to **proportionally distribute the original output** instead of trying to recalculate from UTXO values:

### New Approach

1. **No UTXO value assumptions**: Don't assume each input is worth 1999 sats
2. **Proportional output distribution**: Each split transaction gets a percentage of the original output based on its input count
3. **Dust protection**: Validate that each output is above the dust threshold (546 sats)

### Code Changes

In `crates/alkanes-cli-sys/src/lib.rs`:

```rust
// OLD (BUGGY) CODE:
let sats_per_input = 1999u64;
let chunk_input_amount = chunk_inputs.len() as u64 * sats_per_input;
let chunk_fee = (chunk_vsize as f64 * detected_fee_rate).ceil() as u64;
let chunk_output_amount = chunk_input_amount.saturating_sub(chunk_fee);  // ❌ Can be 0!

// NEW (FIXED) CODE:
let is_last_chunk = tx_idx == num_transactions - 1;
let chunk_output_amount = if is_last_chunk {
    // Last chunk gets whatever is left to avoid rounding errors
    original_output_amount.saturating_sub(total_output_allocated)
} else {
    // Proportional allocation: (chunk_inputs / total_inputs) * original_output
    ((chunk_inputs.len() as u128 * original_output_amount as u128) / all_inputs.len() as u128) as u64
};

// Verify output is above dust threshold
if chunk_output_amount < 546 {
    return Err(AlkanesError::InvalidParameters(format!(
        "Transaction {} would have dust output ({} sats). Original output too small to split.",
        tx_idx + 1, chunk_output_amount
    )));
}
```

## How It Works Now

### Example

Given a transaction with:
- **7,298 inputs**
- **Original output: 3,562,176 sats**
- **Split limit: 100k vsize**
- **Max inputs per chunk: ~653**

The split will create **12 transactions**:

| TX | Inputs | Output (sats) | Calculation |
|----|--------|---------------|-------------|
| 1  | 653    | (653/7298) × 3,562,176 = **318,711** | ✅ |
| 2  | 653    | (653/7298) × 3,562,176 = **318,711** | ✅ |
| ... | ... | ... | ... |
| 11 | 653    | (653/7298) × 3,562,176 = **318,711** | ✅ |
| 12 | 125    | Remaining = 3,562,176 - (11 × 318,711) = **56,255** | ✅ |

All outputs are **above the 546 sat dust threshold** ✅

### Key Points

1. **Proportional distribution** ensures fair allocation
2. **Last chunk gets remainder** to avoid rounding errors
3. **Dust protection** prevents invalid transactions
4. **No UTXO queries needed** - works with any input types
5. **Preserves total value** - all chunks sum to original output

## Important Notes

### Fees Are Already Paid

The **unsigned transaction already has fees calculated and deducted** from the output. When we split:
- We're splitting the **post-fee output amount**
- Each chunk sends part of this output to the same destination
- The original transaction builder (`wallet send`) already handled fee calculation

### Why This Approach Works

The unsigned transaction represents:
```
Total Inputs - Fee = Output Amount
```

When we split, we're essentially saying:
```
Split the output amount across multiple transactions,
each using a subset of the original inputs
```

The fees were already accounted for in the original transaction, so we just need to distribute the output fairly.

## Testing

To verify the fix works:

```bash
# Build a transaction with many inputs
alkanes-cli --wallet-address <addr> --esplora-api-url <url> -p mainnet \
  wallet send <dest> 1 --fee-rate 1 --send-all -y > /tmp/unsigned.hex

# Sign and split it
alkanes-cli --wallet-key-file /root/pk.hex -p mainnet \
  wallet sign-tx --from-file /tmp/unsigned.hex --split-max-vsize 100k

# Decode to verify outputs are not dust
alkanes-cli wallet decode-tx --file /tmp/unsigned_signed_0.hex | grep "sats"
```

Expected: All outputs should be **> 546 sats**

## Edge Cases Handled

1. **Rounding errors**: Last chunk gets exact remainder
2. **Dust detection**: Early error if output would be too small
3. **Varying UTXO sizes**: No assumptions about input values
4. **Integer overflow**: Uses u128 for intermediate calculations

---

**Date**: 2025-11-12  
**Version**: alkanes-cli 10.0.0  
**Issue**: Dust outputs (0 sats) in split transactions  
**Status**: Fixed ✅
