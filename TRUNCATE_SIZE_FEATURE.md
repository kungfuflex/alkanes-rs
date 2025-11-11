# Transaction Size Truncation Feature

## Summary

Enhanced the `--truncate-excess-vsize` flag to accept size arguments with units, allowing precise control over transaction size limits.

## Changes Made

### 1. Updated Command Definition
**File:** `/crates/alkanes-cli-common/src/commands.rs`

Changed from boolean flag to optional string argument:
```rust
// Before
truncate_excess_vsize: bool,

// After  
truncate_excess_vsize: Option<String>,
```

**Documentation:**
- Format: number followed by unit (b/B, k/K, m/M)
- Examples: `100k`, `1m`, `500K`, `1000000b`
- If specified without value, defaults to Bitcoin consensus limit (1m)

### 2. Added Size Parsing Function
**File:** `/crates/alkanes-cli-sys/src/lib.rs`

Added `parse_size_string()` helper function:
```rust
fn parse_size_string(size_str: &str) -> Result<usize>
```

**Supported Units:**
- `b` or `B`: bytes (multiplier: 1)
- `k` or `K`: kilobytes (multiplier: 1024)
- `m` or `M`: megabytes (multiplier: 1024 * 1024)
- No unit: bytes (multiplier: 1)

**Examples:**
- `"100k"` → 102,400 bytes
- `"1m"` → 1,048,576 bytes
- `"500K"` → 512,000 bytes
- `"1000000"` → 1,000,000 bytes

### 3. Updated Transaction Signing Logic
**File:** `/crates/alkanes-cli-sys/src/lib.rs` (SignTx handler)

**Key Changes:**
1. Parse the size limit from the `--truncate-excess-vsize` argument
2. If no truncation specified (`max_tx_size == 0`), check Bitcoin consensus limit (1 MB)
3. If truncation specified, use the custom size limit
4. Enhanced output messages to show size in both bytes and KB
5. Better warning messages when size limits are exceeded

**Logic Flow:**
```rust
let max_tx_size = if let Some(ref size_str) = truncate_excess_vsize {
    parse_size_string(size_str)?
} else {
    0 // No truncation
};

if max_tx_size > 0 {
    // Perform truncation if estimated size > max_tx_size
    // Recalculate fees and output amounts
    // Show detailed stats about truncation
}
```

### 4. Updated Script
**File:** `/home/ubuntu/cmd-build-tx-only.sh`

Changed from:
```bash
--truncate-excess-vsize
```

To:
```bash
--truncate-excess-vsize 100k
```

## Usage Examples

### Truncate to 100 KB
```bash
alkanes-cli wallet sign-tx \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize 100k
```

### Truncate to 1 MB (Bitcoin consensus limit)
```bash
alkanes-cli wallet sign-tx \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize 1m
```

### Truncate to specific byte size
```bash
alkanes-cli wallet sign-tx \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize 500000
```

### No truncation (original behavior)
```bash
alkanes-cli wallet sign-tx \
  --from-file unsigned_tx.hex
```

## Output Format

With truncation enabled, the output now shows:

```
📊 Size limit: 102400 bytes (100.00 KB)
📊 Estimated signed size: 780943 bytes (762.64 KB)
⚠️  Transaction will exceed size limit (102400 bytes / 100.00 KB)
⚠️  Truncating inputs: 9894 → 945 inputs
⚠️  Removed 8949 inputs

📊 Adjusted transaction:
   Inputs: 945 UTXOs
   Input amount: 1889055 sats
   Transaction vSize: 54810 vbytes
   Fee: 57222 sats
   Output amount: 1831833 sats
   Fee rate: 1.0440 sat/vB

✅ Transaction signed successfully!
⚠️  Transaction was truncated to fit size limit
   Original inputs: 9894
   Final inputs: 945
   Removed inputs: 8949
📏 Signed transaction size: 98234 bytes (95.93 KB)
✅ Transaction size is within specified limit
```

## Benefits

1. **Flexible Size Control**: Users can specify any size limit, not just the consensus limit
2. **Better for Testing**: Can create smaller transactions for testing purposes
3. **Network Constraints**: Can accommodate specific network or relay policy size limits
4. **Clear Feedback**: Detailed output shows exactly what was truncated and why
5. **Backwards Compatible**: Not specifying the flag maintains original behavior

## Testing

To test with the script:
```bash
# Ensure private key is in place
sudo bash /home/ubuntu/cmd-build-tx-only.sh
```

The script will:
1. Build an unsigned transaction with all available UTXOs
2. Truncate inputs to fit within 100k if needed
3. Sign the truncated transaction
4. Output detailed statistics about the truncation

## Future Enhancements

Potential improvements:
1. Add a `--target-fee-rate` flag to maintain specific fee rates after truncation
2. Support for `g/G` (gigabytes) unit for very large transactions
3. Add `--truncate-strategy` flag (e.g., "smallest-first", "largest-first", "random")
4. Dry-run mode to preview truncation without signing

## Related Files

- `/crates/alkanes-cli-common/src/commands.rs` - Command definitions
- `/crates/alkanes-cli-sys/src/lib.rs` - Implementation
- `/home/ubuntu/cmd-build-tx-only.sh` - Example usage script
- `/data/alkanes-rs/WITNESS_FIX_SUMMARY.md` - Related witness construction fixes
