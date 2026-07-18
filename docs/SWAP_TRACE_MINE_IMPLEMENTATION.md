# Swap --mine and --trace Implementation

## Summary

Enhanced the `alkanes swap` command with automatic block mining and runestone trace integration for seamless regtest development workflow.

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
  3. Mines 1 block to a wallet address (defaults to change address)
  4. **Syncs the provider** (`provider.sync()`) - ensures metashrew catches up to bitcoind
  5. Proceeds with trace (if `--trace` is also provided)

**Example Usage**:
```bash
# Mine a block after swap
alkanes-cli --provider regtest alkanes swap \
  --path 2:0,32:0 \
  --input 500000 \
  --mine

# Mine and trace
alkanes-cli --provider regtest alkanes swap \
  --path 2:0,32:0 \
  --input 500000 \
  --mine \
  --trace
```

### 2. Runestone Trace Integration (No Polling Needed)

**Location**: `./crates/alkanes-cli-common/src/alkanes/execute.rs`

**Function**: `trace_reveal_transaction()`

**Previous Behavior**:
- Made a single `provider.trace()` call per outpoint
- Would fail if called before transaction confirmed
- No coordination with mining/sync

**New Behavior**:
- Uses same logic as `runestone trace` command
- After `provider.sync()`, traces are immediately available
- **No polling or retries needed**
- Calculates virtual vouts correctly: `tx.output.len() + 1 + protostone_index`

**Key Insight**:
After mining a block with `--mine`, the flow is:
1. Broadcast transaction
2. Mine block (`provider.generate_to_address()`)
3. **Sync provider** (`provider.sync()`) - ensures metashrew catches up to bitcoind
4. Get traces (`provider.trace()`) - **traces are now available, no waiting**

No retries needed because sync guarantees metashrew has processed the block.

**Virtual Vout Calculation**:
```rust
// Protostones are indexed starting at tx.output.len() + 1
let base_vout = tx.output.len() as u32 + 1;

for (i, _) in params.protostones.iter().enumerate() {
    let vout = base_vout + i as u32;
    let outpoint = format!("{}:{}", txid, vout);
    
    // No retries - trace is available immediately after sync
    match self.provider.trace(&outpoint).await {
        Ok(trace_pb) => { /* process trace */ }
        Err(e) => { /* handle error */ }
    }
}
```

**Output**:
```
[INFO] Mining blocks on regtest network...
[INFO] Tracing reveal transaction: abc123...
[INFO] Tracing protostone #1 at virtual vout 3 (outpoint: abc123:3)
[DEBUG] Successfully traced protostone #1: 5 events
```

### 3. Why No Polling?

**The Problem with Old Approach**:
Previous implementations would poll traces repeatedly because they didn't coordinate with blockchain state.

**The Solution**:
The `--mine` flag ensures proper coordination:
1. Transaction is broadcast and **confirmed in a block**
2. `provider.sync()` ensures metashrew has **indexed the block**
3. Once synced, traces are **guaranteed to be available**

**For Non-Regtest Networks**:
If you don't use `--mine`, ensure the transaction is:
1. Confirmed (included in a block)
2. Metashrew has synced to that block height

Then traces will be available without any polling.

## Benefits

### 1. **Improved Developer Experience**
Before:
```bash
# Execute swap
alkanes-cli alkanes swap --path 2:0,32:0 --input 500000

# Wait for confirmation (manual)
bitcoin-cli generatetoaddress 1 bcrt1q...

# Try to trace (fails if not synced)
alkanes-cli alkanes trace txid:vout
```

After:
```bash
# One command, fully automated
alkanes-cli --provider regtest alkanes swap \
  --path 2:0,32:0 \
  --input 500000 \
  --mine \
  --trace

# Automatically: broadcasts → mines → syncs → shows trace
```

### 2. **Reliability**
- **No more "trace not found" errors** - sync guarantees traces are available
- **Automatic mining** - no manual block generation needed
- **Deterministic behavior** - no retries, no race conditions

### 3. **Better Logging**
- Shows protostone number and virtual vout
- Clear indication of successful traces
- Counts events in each trace

## Testing

### Test 1: Basic Swap with Mining
```bash
regtest-cli alkanes swap \
  --path 2:0,32:0 \
  --input 500000 \
  --mine
```

**Expected**: Transaction broadcasts, block mines automatically

### Test 2: Swap with Trace
```bash
regtest-cli alkanes swap \
  --path 2:0,32:0 \
  --input 500000 \
  --mine \
  --trace
```

**Expected**: Transaction executes, mines, syncs, displays trace data

### Test 3: Non-Regtest (Should Skip Mining)
```bash
alkanes-cli --provider mainnet alkanes swap \
  --path 2:0,32:0 \
  --input 500000 \
  --mine
```

**Expected**: `--mine` flag ignored, normal execution

## Edge Cases Handled

1. **Trace not available**: Only happens if transaction not confirmed or metashrew not synced
2. **Empty events array**: Valid response - protostone executed but had no events
3. **RPC errors**: Logged and returned in trace output with error details
4. **Multiple protostones**: Each protostone traced with correct virtual vout
5. **Non-regtest network**: Mining is safely skipped (user must wait for confirmation)

## Backward Compatibility

- ✅ **`--mine` is optional** - existing commands work unchanged
- ✅ **`--trace` behavior improved** - uses runestone trace logic
- ✅ **All other flags unchanged** - no breaking changes to swap command
- ✅ **Works with all execute-based commands** - same logic applies to `alkanes execute`, `wrap-btc`, etc.

## Related Commands with Similar Behavior

The `--mine` and trace enhancements are available for:
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

### Trace Logic (No Polling)
```rust
// After provider.sync(), traces are immediately available
let base_vout = tx.output.len() as u32 + 1;

for (i, _) in params.protostones.iter().enumerate() {
    let vout = base_vout + i as u32;
    let outpoint = format!("{}:{}", txid, vout);
    
    match self.provider.trace(&outpoint).await {
        Ok(trace_pb) => {
            if let Some(alkanes_trace) = trace_pb.trace {
                match Trace::try_from(Message::encode_to_vec(&alkanes_trace)) {
                    Ok(trace) => {
                        let json = trace_to_json(&trace);
                        traces.push(json); // Success
                    }
                    Err(e) => {
                        // Decode error - log and continue
                        traces.push(error_json);
                    }
                }
            }
        }
        Err(e) => {
            // RPC error - log and continue
            traces.push(error_json);
        }
    }
}
```

### Execution Flow with --mine and --trace
```rust
// 1. Broadcast transaction
let txid = self.provider.broadcast_transaction(tx_hex).await?;

// 2. Mine block if --mine flag
if params.mine_enabled {
    self.mine_blocks_if_regtest(params).await?;
    self.provider.sync().await?;  // ← KEY: Ensures metashrew catches up
}

// 3. Get traces if --trace flag (NO POLLING)
let traces = if params.trace_enabled {
    self.trace_reveal_transaction(&txid, params).await?
} else {
    None
};
```

## Future Enhancements

1. **Parallel trace fetching**:
   - Fetch all protostone traces concurrently instead of sequentially
   - Use `tokio::join!` or `futures::join_all`

2. **Rich trace output**:
   - Colored event logging
   - Pretty-printed JSON with syntax highlighting
   - Summary statistics (gas used, events fired, etc.)

3. **Trace filtering**:
   - `--trace-events <PATTERN>` - only show matching events
   - `--trace-errors-only` - only show failed traces

4. **Export formats**:
   - `--trace-format json|yaml|table` - different output formats
   - `--trace-output <FILE>` - save traces to file

---

**Status**: ✅ Implemented and tested
**Version**: alkanes-rs v10.0.0+
**Date**: 2025-11-24
