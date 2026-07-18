# Bug Fix: Reveal Transaction Validation

## Date: 2025-11-22
## Status: ✅ **FIXED**

---

## Problem

Contract deployments with envelope (commit/reveal pattern) were failing with:
```
Error: Validation error: Protostone 0 has pointer to output v0 but only 0 outputs will exist
```

### Root Cause

In `build_reveal_psbt()` (line 1408), the validation was happening **BEFORE** `create_outputs()` was called:

```rust
// ❌ WRONG: Validates before outputs are created
self.validate_protostones(&params.protostones, params.to_addresses.len())?;

let outputs = self.create_outputs(...).await?;
```

For contract deployments:
- No `--to` addresses are specified (just `--envelope`)
- `params.to_addresses.len()` = **0**
- Validation failed because protostone references `v0` but 0 outputs exist

### Why This Worked for Single Transactions

In `build_single_transaction()` (line 388), validation happened AFTER outputs were created:

```rust
let outputs = self.create_outputs(...).await?;
// ✅ CORRECT: Validates against actual outputs
self.validate_protostones(&params.protostones, outputs.len())?;
```

---

## Solution

Move validation to AFTER `create_outputs()` is called and validate against the **actual** number of outputs created:

```rust
let outputs = self.create_outputs(&params.to_addresses, &params.change_address, &params.input_requirements, &params.protostones).await?;

// ✅ Validate protostones against the ACTUAL number of outputs created
self.validate_protostones(&params.protostones, outputs.len())?;

let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
```

### How `create_outputs()` Works

1. Scans all protostones to find the maximum identifier referenced (e.g., v0, v1, v2)
2. If `max_identifier = Some(0)` (protostone has `v0`), creates `num_identifier_outputs = 1`
3. Creates output v0 with:
   - Address: `--change` if specified, else `p2tr:0` (default)
   - Amount: `DUST_LIMIT` (546 sats)
4. Adds BTC change output

### Result

For deployment `alkanes execute "[3,100]:v0:v0" --envelope contract.wasm`:
- Output 0 (v0): 546 sats to p2tr:0 - **Identifier output**
- Output 1: BTC change to p2wsh:0 - **Change output**
- Output 2: 0 sats OP_RETURN - **Runestone** (added by build_psbt_and_fee)

---

## Testing

### Test Case 1: Simple Deployment

**Command**:
```bash
alkanes execute "[3,100]:v0:v0" --envelope prod_wasms/alkanes_std_auth_token.wasm --from p2tr:0 --fee-rate 1 --mine -y
```

**Result**:
```
✅ Commit TXID: d28ea5654a6cd80542a802ca4ce84044bde3174185d2a12a0e8ecef3ff53a509
✅ Reveal TXID: e5506dac8da38309d75ee3c0baa20b7d02cff4d6b9ec3f8cf225d2fcd8735941
```

**Reveal Transaction Outputs**:
```json
[
  {
    "n": 0,
    "value": 0.00000546,
    "type": "witness_v1_taproot"  // v0 - identifier output
  },
  {
    "n": 1,
    "value": 0.0003336,
    "type": "witness_v0_scripthash"  // BTC change
  },
  {
    "n": 2,
    "value": 0.0,
    "type": "nulldata"  // OP_RETURN - runestone
  }
]
```

### Test Case 2: Full AMM Deployment

**Command**:
```bash
./scripts/deploy-amm.sh
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

All 6 contracts deployed successfully with proper output structure.

---

## Code Changes

### File: `/crates/alkanes-cli-common/src/alkanes/execute.rs`

**Line 1408** (before fix):
```rust
async fn build_reveal_psbt(
    &mut self,
    params: &EnhancedExecuteParams,
    envelope: &AlkanesEnvelope,
    commit_outpoint: OutPoint,
    commit_output_value: u64,
    commit_internal_key: XOnlyPublicKey,
    commit_internal_key_fingerprint: bitcoin::bip32::Fingerprint,
    commit_internal_key_path: &bitcoin::bip32::DerivationPath,
) -> Result<(bitcoin::psbt::Psbt, u64)> {
    self.validate_protostones(&params.protostones, params.to_addresses.len())?;  // ❌ WRONG
    
    let mut selected_utxos = vec![commit_outpoint];
    // ...
    
    let outputs = self.create_outputs(...).await?;
    let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
    // ...
}
```

**After fix**:
```rust
async fn build_reveal_psbt(
    &mut self,
    params: &EnhancedExecuteParams,
    envelope: &AlkanesEnvelope,
    commit_outpoint: OutPoint,
    commit_output_value: u64,
    commit_internal_key: XOnlyPublicKey,
    commit_internal_key_fingerprint: bitcoin::bip32::Fingerprint,
    commit_internal_key_path: &bitcoin::bip32::DerivationPath,
) -> Result<(bitcoin::psbt::Psbt, u64)> {
    let mut selected_utxos = vec![commit_outpoint];
    // ...
    
    let outputs = self.create_outputs(...).await?;
    
    // ✅ Validate protostones against the ACTUAL number of outputs created
    self.validate_protostones(&params.protostones, outputs.len())?;
    
    let runestone_script = self.construct_runestone_script(&params.protostones, outputs.len())?;
    // ...
}
```

### Impact

- **Lines Changed**: 3 (moved validation, added comment)
- **Files Modified**: 1
- **Breaking Changes**: None
- **Backward Compatibility**: ✅ Yes

---

## Why This Bug Existed

The validation was originally placed early to fail fast if protostones were invalid. However, this didn't account for the fact that **outputs are dynamically created** based on what the protostones reference.

For deployments without explicit `--to` addresses:
- `params.to_addresses` is empty
- But `create_outputs()` scans protostones and creates outputs for v0, v1, etc.
- The early validation used `params.to_addresses.len()` instead of the actual outputs that would be created

---

## Related Features

This fix works in conjunction with:

1. **Automatic Identifier Output Creation** (Phase 1 of alkanes change implementation)
   - Scans protostones for max identifier (v0, v1, etc.)
   - Automatically creates outputs even without `--to` addresses
   - Defaults to `p2tr:0` or uses `--change` address

2. **BTC Change Handling** (Phase 2)
   - Always creates BTC change output
   - Defaults to `p2wsh:0` if not specified

3. **OP_RETURN Addition** (existing)
   - `build_psbt_and_fee()` adds OP_RETURN with runestone
   - Happens after outputs are created

---

## Lessons Learned

1. **Validation Timing**: Validate against actual state, not expected state
2. **Dynamic Output Creation**: Outputs aren't just from `--to` addresses
3. **Consistency**: Single transaction and reveal transaction should follow same pattern
4. **Testing**: Need integration tests for envelope deployments

---

## Follow-Up Work

### Completed ✅
- Fixed validation in `build_reveal_psbt`
- Tested with single deployment
- Tested with full AMM deployment (6 contracts)
- Verified output structure

### Future Improvements 💡
- Add unit tests for `build_reveal_psbt`
- Add integration tests for envelope deployments
- Document automatic identifier output creation
- Consider refactoring validation into a single shared function

---

## Summary

**Problem**: Deployments failed because validation happened before outputs were created  
**Solution**: Move validation to after `create_outputs()` and validate against actual outputs  
**Result**: All deployments now succeed with correct output structure  
**Impact**: 3 lines changed, 0 breaking changes, full backward compatibility  

---

**Status**: 🟢 **FIXED AND DEPLOYED**  
**Verified**: ✅ Single deployments work  
**Verified**: ✅ Multi-contract deployments work  
**Verified**: ✅ Output structure is correct (v0, change, OP_RETURN)  

🎉 **Contract deployments are now fully functional!**
