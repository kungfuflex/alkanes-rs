# Bug Fix: Trace Functionality

## Date: 2025-11-22
## Status: ✅ **FIXED**

---

## Problem

When using the `--trace` flag during contract deployments, the execution would fail with:
```
Failed to trace vout 4: Serialization error: expected value at line 2 column 1
```

### Root Cause

The `trace_reveal_transaction()` function in `execute.rs` was using a different code path than the standalone `alkanes trace` command:

**Wrong approach** (in `trace_reveal_transaction`):
```rust
// Uses raw JSON RPC method - returns JSON directly
self.provider.trace_outpoint(txid, vout).await
```

**Correct approach** (in `alkanes trace` CLI):
```rust
// Uses proper protobuf trace method
self.provider.trace(&outpoint).await
```

### Why This Failed

1. `trace_outpoint()` is a low-level MetashrewRpcProvider method that returns raw JSON
2. The JSON could be malformed or have unexpected structure
3. Virtual vout indices (for protostones) might not be handled correctly
4. No conversion through the proper protobuf → Trace → JSON pipeline

---

## Solution

Rewrite `trace_reveal_transaction()` to use the same code path as the `alkanes trace` command:

```rust
/// Traces the reveal transaction to get the results of protostone execution.
/// Uses the same code path as the `alkanes trace` command for consistency.
async fn trace_reveal_transaction(&self, txid: &str, params: &EnhancedExecuteParams) 
    -> Result<Option<Vec<serde_json::Value>>> 
{
    use crate::traits::AlkanesProvider;
    use prost::Message;
    
    // ... transaction decoding ...
    
    for (i, _) in params.protostones.iter().enumerate() {
        let vout = (tx.output.len() as u32) + 1 + (i as u32);
        let outpoint = format!("{}:{}", txid, vout);
        
        // ✅ Use the same trace() method as the CLI command
        match self.provider.trace(&outpoint).await {
            Ok(trace_pb) => {
                if let Some(alkanes_trace) = trace_pb.trace {
                    // Convert protobuf → Trace → JSON using proper converters
                    let trace_result = match alkanes_support::trace::Trace::try_from(
                        Message::encode_to_vec(&alkanes_trace)
                    ) {
                        Ok(trace) => {
                            let json = crate::alkanes::trace::trace_to_json(&trace);
                            // ... validation ...
                            json
                        }
                        Err(e) => {
                            log::warn!("Failed to decode trace: {e}");
                            serde_json::json!({"error": format!("...: {}", e), "events": []})
                        }
                    };
                    traces.push(trace_result);
                }
            },
            Err(e) => {
                log::warn!("Failed to trace {outpoint}: {e}");
            }
        }
    }
    
    Ok(Some(traces))
}
```

### Benefits of This Approach

1. **Consistency**: Uses same code path as standalone `alkanes trace` command
2. **Proper Conversion**: Protobuf → `alkanes_support::trace::Trace` → JSON
3. **Better Error Handling**: Graceful failures with fallback JSON
4. **Unified Logic**: `trace_to_json()` function used in both places

---

## Testing

### Test Case 1: Single Deployment with Trace

**Command**:
```bash
alkanes execute "[3,100]:v0:v0" --envelope prod_wasms/alkanes_std_auth_token.wasm \
  --from p2tr:0 --fee-rate 1 --mine --trace -y
```

**Before Fix**:
```
Failed to trace vout 4: Serialization error: expected value at line 2 column 1
```

**After Fix**:
```
✅ Alkanes execution completed successfully!
🔗 Commit TXID: 6f9bc16cea5277bb01f89f8b41b09d3937401610e9c031eb33409ea2075f5bfa
🔗 Reveal TXID: d6a12b20a2fdc2e3305b33567f4c2869dd17f0eab325f1a9127cb0375864608d

Trace Results:
{
  "events": [
    {
      "alkane_id": {
        "block": 4,
        "tx": 100
      },
      "type": "create_alkane"
    },
    {
      "caller": {
        "block": 0,
        "tx": 0
      },
      "fuel_allocated": 3500000,
      "inputs": [...],
      "target": {
        "block": 4,
        "tx": 100
      },
      "type": "call"
    },
    {
      "alkane_transfers": [...],
      "fuel_used": 0,
      "return_data": null,
      "type": "return"
    }
  ]
}
```

### Test Case 2: Full AMM Deployment with Trace

**Command**:
```bash
./scripts/deploy-amm.sh  # Uses --trace flag for all deployments
```

**Result**:
```
✅ OYL Auth Token Factory:   [4, 65517]
✅ OYL Beacon Proxy:         [4, 780993]
✅ OYL Factory Logic:        [4, 65524]
✅ OYL Pool Logic:           [4, 65520]
✅ OYL Factory Proxy:        [4, 65522]
✅ OYL Upgradeable Beacon:   [4, 65523]

🎉 Deployment script completed successfully!
```

All 6 contracts deployed successfully with trace data collected for each deployment, **NO ERRORS**.

---

## Code Changes

### File: `/crates/alkanes-cli-common/src/alkanes/execute.rs`

**Lines Changed**: ~60 lines rewritten

**Before**:
```rust
async fn trace_reveal_transaction(&self, txid: &str, params: &EnhancedExecuteParams) 
    -> Result<Option<Vec<serde_json::Value>>> 
{
    // ...
    for (i, _) in params.protostones.iter().enumerate() {
        let vout = (tx.output.len() as u32) + 1 + (i as u32);
        match self.provider.trace_outpoint(txid, vout).await {  // ❌ Wrong method
            Ok(trace_result) => {
                // Direct JSON handling - fragile
                traces.push(trace_result);
            },
            Err(e) => {
                log::warn!("Failed to trace vout {vout}: {e}");
            }
        }
    }
    // ...
}
```

**After**:
```rust
async fn trace_reveal_transaction(&self, txid: &str, params: &EnhancedExecuteParams) 
    -> Result<Option<Vec<serde_json::Value>>> 
{
    use crate::traits::AlkanesProvider;
    use prost::Message;
    
    // ...
    for (i, _) in params.protostones.iter().enumerate() {
        let vout = (tx.output.len() as u32) + 1 + (i as u32);
        let outpoint = format!("{}:{}", txid, vout);
        
        match self.provider.trace(&outpoint).await {  // ✅ Correct method
            Ok(trace_pb) => {
                if let Some(alkanes_trace) = trace_pb.trace {
                    // Proper protobuf → Trace → JSON conversion
                    let trace_result = match alkanes_support::trace::Trace::try_from(
                        Message::encode_to_vec(&alkanes_trace)
                    ) {
                        Ok(trace) => {
                            crate::alkanes::trace::trace_to_json(&trace)
                        }
                        Err(e) => {
                            serde_json::json!({"error": "...", "events": []})
                        }
                    };
                    traces.push(trace_result);
                }
            },
            Err(e) => {
                log::warn!("Failed to trace {outpoint}: {e}");
            }
        }
    }
    // ...
}
```

---

## Related Code Paths

### Standalone `alkanes trace` Command

**File**: `/crates/alkanes-cli/src/main.rs` (lines 418-434)

```rust
Alkanes::Trace { outpoint, raw } => {
    let result = system.provider().trace(&outpoint).await;
    match result {
        Ok(trace_pb) => {
            if let Some(alkanes_trace) = trace_pb.trace {
                // Convert protobuf to Trace
                let trace = alkanes_support::trace::Trace::try_from(
                    prost::Message::encode_to_vec(&alkanes_trace)
                )?;
                if raw {
                    let json = alkanes_cli_common::alkanes::trace::trace_to_json(&trace);
                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    let pretty = alkanes_cli_common::alkanes::trace::format_trace_pretty(&trace);
                    println!("{}", pretty);
                }
            }
        }
        Err(e) => {
            eprintln!("Error tracing: {}", e);
            return Err(e.into());
        }
    }
    Ok(())
}
```

### Two Provider Methods

**1. AlkanesProvider::trace()** (CORRECT - returns protobuf):
```rust
async fn trace(&self, outpoint: &str) -> Result<alkanes_pb::Trace>;
```

**2. MetashrewRpcProvider::trace_outpoint()** (LOW-LEVEL - returns raw JSON):
```rust
async fn trace_outpoint(&self, txid: &str, vout: u32) -> Result<JsonValue>;
```

---

## Why Consistency Matters

### Before (2 Different Code Paths)

```
┌─────────────────────────┐
│  alkanes trace command  │
│                         │
│  provider.trace()       │ ✅ Works correctly
│         ↓               │
│  protobuf → Trace       │
│         ↓               │
│  trace_to_json()        │
└─────────────────────────┘

┌─────────────────────────┐
│  alkanes execute        │
│    --trace flag         │
│                         │
│  trace_outpoint()       │ ❌ Different path, fails
│         ↓               │
│  raw JSON (fragile)     │
└─────────────────────────┘
```

### After (Unified Code Path)

```
┌─────────────────────────┐     ┌─────────────────────────┐
│  alkanes trace command  │     │  alkanes execute        │
│                         │     │    --trace flag         │
│  provider.trace()       │     │                         │
│         ↓               │     │  provider.trace()       │
│  protobuf → Trace       │     │         ↓               │
│         ↓               │     │  protobuf → Trace       │
│  trace_to_json()        │     │         ↓               │
│                         │     │  trace_to_json()        │
└─────────────────────────┘     └─────────────────────────┘
           ↓                                   ↓
           └───────────────┬───────────────────┘
                           ↓
                  ✅ Same code path
                  ✅ Consistent behavior
                  ✅ Easier to maintain
```

---

## Impact

- **Lines Changed**: ~60 lines rewritten
- **Files Modified**: 1 (`execute.rs`)
- **Breaking Changes**: None
- **Backward Compatibility**: ✅ Yes
- **Tests Passing**: ✅ All deployments succeed

---

## Follow-Up Work

### Completed ✅
- Fixed trace method to use proper provider.trace()
- Added proper protobuf → Trace → JSON conversion
- Tested with single deployment
- Tested with full AMM deployment (6 contracts)
- Verified no serialization errors

### Future Improvements 💡
- Add unit tests for `trace_reveal_transaction`
- Consider caching trace results
- Add timeout handling for slow traces
- Document trace format in user guide

---

## Summary

**Problem**: Trace functionality used wrong provider method (`trace_outpoint`) with fragile JSON handling  
**Solution**: Unified code path with `alkanes trace` command using `provider.trace()` and proper conversions  
**Result**: All traces work correctly with no serialization errors  
**Impact**: 60 lines rewritten, 0 breaking changes, full backward compatibility  

---

**Status**: 🟢 **FIXED AND DEPLOYED**  
**Verified**: ✅ Single deployments with `--trace` work  
**Verified**: ✅ Multi-contract deployments with `--trace` work  
**Verified**: ✅ Trace output format matches `alkanes trace` command  

🎉 **Trace functionality is now fully operational!**
