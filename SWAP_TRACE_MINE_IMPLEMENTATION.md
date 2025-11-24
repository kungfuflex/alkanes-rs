# Swap --mine and --trace Enhancements

## Summary

Enhanced the `alkanes swap` command with automatic block mining and trace polling capabilities for better regtest development workflow.

## Changes Implemented

### 1. Added `--mine` Flag to Swap Command

**Location**: `./crates/alkanes-cli/src/commands.rs` and `./crates/alkanes-cli/src/main.rs`

```rust
/// Mine a block after broadcasting (regtest only)
#[arg(long)]
mine: bool,
```

**Behavior**:
- When `--mine` is provided and provider is regtest:
  1. Broadcasts the swap transaction
  2. Waits 2 seconds
  3. Mines 1 block to a wallet address (defaults to change address or p2wpkh:0)
  4. Syncs the provider
  5. Proceeds with trace (if `--trace` is also provided)

**Example Usage**:
```bash
# Mine a block after swap
alkanes-cli --provider regtest alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --mine

# Mine and trace
alkanes-cli --provider regtest alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --mine \\
  --trace
```

### 2. Enhanced Trace with Polling and Retries

**Location**: `./crates/alkanes-cli-common/src/alkanes/execute.rs`

**Function**: `trace_reveal_transaction()`

**Previous Behavior**:
- Made a single `provider.trace()` call
- Immediately failed if trace wasn't available
- Common in regtest where trace generation takes time

**New Behavior**:
- Polls for traces up to 10 times with 1-second delays
- Retries on:
  - Empty events array
  - Missing alkanes_trace data
  - RPC errors
- Logs retry attempts with progress
- Only fails after all retries exhausted

**Retry Logic**:
```rust
const MAX_RETRIES: u32 = 10;
const RETRY_DELAY_SECS: u64 = 1;

for attempt in 1..=MAX_RETRIES {
    match self.provider.trace(&outpoint).await {
        Ok(trace_pb) => {
            if events.is_empty() && attempt < MAX_RETRIES {
                log::info!("Retrying in {RETRY_DELAY_SECS}s...");
                sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                continue;
            }
            // Process trace
            break;
        }
        Err(e) => {
            // Retry on error
        }
    }
}
```

**Output**:
```
[INFO] Tracing protostone #0 at virtual outpoint 1a2b3c...
[WARN] Trace came back with empty events array (attempt 1/10).
[INFO] Retrying in 1s...
[INFO] Trace came back with empty events array (attempt 2/10).
[INFO] Retrying in 1s...
[DEBUG] Trace result for 1a2b3c...: {...}
```

### 3. Using Runestone Trace (Full Transaction)

The implementation uses `provider.trace(&outpoint)` which internally calls `runestone trace`, retrieving traces for all protostones in the transaction, not just individual outputs.

**Virtual Outpoint Calculation**:
```rust
// For a transaction with N real outputs and M protostones:
// Virtual vout = N + 1 + protostone_index
let vout = (tx.output.len() as u32) + 1 + (i as u32);
```

This matches the protocol's trace indexing scheme where protostones create "virtual outputs" after the real transaction outputs.

## Benefits

### 1. **Improved Developer Experience**
Before:
```bash
# Execute swap
alkanes-cli alkanes swap --path 2:0,32:0 --input 500000

# Wait for confirmation (manual)
bitcoin-cli generatetoaddress 1 bcrt1q...

# Try to trace (often fails)
alkanes-cli alkanes trace txid:vout
# Error: trace not available

# Wait more
sleep 5

# Try again
alkanes-cli alkanes trace txid:vout
# Still fails sometimes
```

After:
```bash
# One command, fully automated
alkanes-cli --provider regtest alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --mine \\
  --trace

# Automatically: broadcasts → mines → waits → retries → shows trace
```

### 2. **Reliability**
- **No more "trace not found" errors** - polling handles temporary unavailability
- **Automatic mining** - no manual block generation needed
- **Consistent behavior** - works across different RPC response times

### 3. **Better Logging**
- Shows retry attempts with progress
- Clear indication when traces are being fetched
- Warnings for empty traces with retry count

## Testing

### Test 1: Basic Swap with Mining
```bash
regtest-cli alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --mine
```

**Expected**: Transaction broadcasts, block mines automatically

### Test 2: Swap with Trace Polling
```bash
regtest-cli alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --mine \\
  --trace
```

**Expected**: Transaction executes, mines, polls for trace, displays trace data

### Test 3: Non-Regtest (Should Skip Mining)
```bash
alkanes-cli --provider mainnet alkanes swap \\
  --path 2:0,32:0 \\
  --input 500000 \\
  --mine
```

**Expected**: `--mine` flag ignored, normal execution

## Edge Cases Handled

1. **Trace not immediately available**: Retries up to 10 times
2. **Empty events array**: Treats as not-ready and retries
3. **RPC errors**: Retries on transient failures
4. **Multiple protostones**: Polls for each protostone's trace independently
5. **Non-regtest network**: Mining is safely skipped

## Backward Compatibility

- ✅ **`--mine` is optional** - existing commands work unchanged
- ✅ **`--trace` behavior improved** - adds retries but doesn't break existing usage
- ✅ **All other flags unchanged** - no breaking changes to swap command
- ✅ **Works with all execute-based commands** - same logic applies to `alkanes execute`, `wrap-btc`, etc.

## Related Commands with Similar Behavior

The `--mine` and trace polling enhancements are available for:
- `alkanes execute` - already had `--mine` flag
- `alkanes wrap-btc` - already had `--mine` flag  
- `alkanes swap` - **NEW: now has `--mine` flag**
- All commands using `EnhancedExecuteParams`

## Implementation Details

### Mining Logic (Inherited from Execute)
```rust
async fn mine_blocks_if_regtest(&self, params: &EnhancedExecuteParams) -> Result<()> {
    if self.provider.get_network() == bitcoin::Network::Regtest {
        log::info!("Mining blocks on regtest network...");
        sleep(Duration::from_secs(2)).await;
        let address = if let Some(change_address) = &params.change_address {
            change_address.clone()
        } else {
            WalletProvider::get_address(self.provider).await?
        };
        self.provider.generate_to_address(1, &address).await?;
    }
    Ok(())
}
```

### Trace Polling Logic
```rust
for attempt in 1..=MAX_RETRIES {
    match self.provider.trace(&outpoint).await {
        Ok(trace_pb) => {
            if let Some(alkanes_trace) = trace_pb.trace {
                match alkanes_support::trace::Trace::try_from(...) {
                    Ok(trace) => {
                        let json = trace_to_json(&trace);
                        if json.events.is_empty() && attempt < MAX_RETRIES {
                            continue; // Retry
                        }
                        break; // Success
                    }
                    Err(e) if attempt < MAX_RETRIES => {
                        continue; // Retry
                    }
                }
            }
        }
        Err(e) if attempt < MAX_RETRIES => {
            continue; // Retry
        }
    }
}
```

## Future Enhancements

1. **Configurable retry parameters**:
   - `--trace-retries <N>` - override default 10 retries
   - `--trace-delay <SECS>` - override default 1s delay

2. **Progressive backoff**:
   - First retry: 1s
   - Second retry: 2s
   - Third retry: 4s
   - etc.

3. **Parallel trace fetching**:
   - Poll all protostone traces concurrently instead of sequentially

4. **Live trace streaming**:
   - Stream trace events as they become available
   - Show progress bar during polling

---

**Status**: ✅ Implemented and tested
**Version**: alkanes-rs v10.0.0+
**Date**: 2025-11-24
