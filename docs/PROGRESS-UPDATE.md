# Alkanes Change Implementation Progress

## Date: 2025-11-22
## Session: Continued Implementation

---

## What We Completed in This Session ✅

### 1. Alkanes Balance Tracking Infrastructure

**Status**: ✅ Complete and Working

**Changes Made**:

1. **Created `UtxoSelectionResult` struct** in `execute.rs`:
   ```rust
   struct UtxoSelectionResult {
       outpoints: Vec<OutPoint>,
       alkanes_found: alloc::collections::BTreeMap<AlkaneId, u64>,
   }
   ```
   
2. **Added `AlkanesBalance` type** in `types.rs`:
   ```rust
   pub struct AlkanesBalance {
       pub alkane_id: AlkaneId,
       pub amount: u64,
   }
   ```

3. **Enhanced `AlkaneId` with required traits**:
   - Added `PartialOrd`, `Ord` derives for use as BTreeMap key
   - Added `Hash` for efficient lookups

4. **Modified `select_utxos()` method**:
   - Now tracks ALL alkanes found in selected UTXOs
   - Returns `UtxoSelectionResult` instead of just `Vec<OutPoint>`
   - Logs alkanes found for debugging

**Implementation Details**:

The `select_utxos()` method now:
- Queries `protorunesbyoutpoint` for each UTXO
- Tracks what we need (from requirements)
- Tracks what we actually find (for change calculation)
- Returns both outpoints and full alkanes inventory

**Example Output**:
```
Selected 3 UTXOs meeting all requirements (Bitcoin: 100000000/50000000, Alkanes: 1 types)
Alkanes found in selected UTXOs:
  2:1 = 10000 units
```

5. **Updated All Call Sites**:
   - `build_single_transaction()`: Uses `utxo_selection.outpoints`
   - `build_commit_reveal_pattern()`: Uses `utxo_selection.outpoints`
   - `build_reveal_psbt()`: Uses `utxo_selection.outpoints`

6. **Added Warning for Unimplemented Feature**:
   ```rust
   if !utxo_selection.alkanes_found.is_empty() {
       log::warn!("Alkanes found in UTXOs - automatic change handling not yet implemented!");
       log::warn!("Excess alkanes may be burned! Use with caution.");
   }
   ```

---

## What's Left to Implement 🚧

### Phase 1: Automatic Protostone Generation (CRITICAL)

**Priority**: HIGH - This is the main feature we set out to implement

**Steps Remaining**:

1. **Calculate Excess Alkanes**
   ```rust
   // In build_single_transaction(), after UTXO selection:
   let alkanes_needed = calculate_alkanes_needed(&params.input_requirements);
   let alkanes_excess = calculate_excess(&utxo_selection.alkanes_found, &alkanes_needed);
   ```

2. **Generate Automatic Protostone**
   ```rust
   if !alkanes_excess.is_empty() {
       let auto_protostone = generate_alkanes_change_protostone(
           &alkanes_excess,
           params.alkanes_change_address.as_ref()
               .or(params.change_address.as_ref())
               .unwrap_or(&"p2tr:0".to_string())
       );
       // Insert at index 0
   }
   ```

3. **Adjust Protostone References**
   ```rust
   // Shift all user protostone references: p0→p1, p1→p2, etc.
   let adjusted_protostones = adjust_protostone_references(&params.protostones);
   ```

4. **Create Alkanes Change Output**
   ```rust
   // Add output if not already present
   if alkanes_change_needed {
       outputs.push(create_alkanes_change_output(...));
   }
   ```

5. **Validate Final Transaction**
   - Ensure all alkanes are accounted for
   - No alkanes burned
   - Proper output mapping

**Estimated Effort**: 2-3 hours

---

### Phase 2: Mock Testing Infrastructure

**Priority**: HIGH - Needed for reliable testing

**What We Need**:

1. **MockProvider Implementation**
   ```rust
   pub struct MockProvider {
       utxos: Vec<(OutPoint, UtxoInfo)>,
       alkanes_balances: BTreeMap<OutPoint, Vec<AlkanesBalance>>,
       network: Network,
   }
   ```

2. **Test Helper Functions**
   ```rust
   fn create_mock_utxo(...) -> (OutPoint, UtxoInfo);
   fn add_alkanes_to_utxo(...);
   fn build_test_transaction(...) -> Result<Transaction>;
   ```

3. **Test Scenarios**
   - Simple deployment (no alkanes)
   - Deployment with excess alkanes
   - Multiple alkanes types
   - Exact alkanes match (no change needed)
   - Complex multi-protostone patterns

**Estimated Effort**: 3-4 hours

---

### Phase 3: Integration Testing

**Priority**: MEDIUM - Once logic is complete

**Tests Needed**:
- Real regtest deployment with deploy-amm.sh
- Manual verification of no burned alkanes
- Edge case testing

**Estimated Effort**: 1-2 hours

---

## Current Code State

### Files Modified Today

1. **`/crates/alkanes-cli-common/src/alkanes/types.rs`**
   - Added `AlkanesBalance` struct
   - Enhanced `AlkaneId` with `Ord`, `PartialOrd`, `Hash`

2. **`/crates/alkanes-cli-common/src/alkanes/execute.rs`**
   - Added `UtxoSelectionResult` struct
   - Modified `select_utxos()` to track alkanes
   - Updated all call sites
   - Added warning for unimplemented feature

### Build Status
- ✅ Compilation: Success
- ✅ Build time: 35.25 seconds
- ⚠️  Warnings: 8 (non-critical, mostly unused imports)
- ✅ Errors: 0

### Code Statistics
- **Lines added today**: ~40 lines
- **Lines modified today**: ~15 lines
- **Total implementation size**: ~300 lines (across both sessions)

---

## Architecture Decisions Made

### 1. BTreeMap for Alkanes Storage
**Why**: Deterministic ordering, efficient lookups, required for Rust's collections

### 2. Separate Tracking of "Needed" vs "Found"
**Why**: Need to calculate excess = found - needed

### 3. Return Structure from select_utxos
**Why**: Need both outpoints (for transaction building) and alkanes inventory (for change calculation)

### 4. Warning Before Implementation
**Why**: Safe development practice - warn users before feature is complete

---

## Next Session Plan

### Immediate Tasks (in order):

1. **Implement `calculate_alkanes_needed()`**
   - Extract alkanes requirements from `InputRequirement::Alkanes`
   - Return `BTreeMap<AlkaneId, u64>`

2. **Implement `calculate_excess()`**
   - Compare found vs needed
   - Return `BTreeMap<AlkaneId, u64>` of excess amounts

3. **Implement `generate_alkanes_change_protostone()`**
   - Create ProtostoneSpec with edicts for excess alkanes
   - Point to alkanes change output
   - Set refund pointer correctly

4. **Implement `adjust_protostone_references()`**
   - Scan all protostones for p0, p1, p2, etc.
   - Increment each by 1
   - Return adjusted list

5. **Implement `create_alkanes_change_output()`**
   - Resolve alkanes-change address
   - Create TxOut with dust value
   - Insert in correct position

6. **Integration**
   - Wire all pieces together in `build_single_transaction()`
   - Test with simple case
   - Add comprehensive tests

7. **Mock Provider**
   - Implement trait methods
   - Add test data builders
   - Create test suite

---

## Risk Assessment

### High Risk (Needs Careful Implementation)
- ✅ UTXO selection tracking (DONE)
- 🚧 Protostone reference adjustment
- 🚧 Output index management

### Medium Risk
- 🚧 Alkanes excess calculation
- 🚧 Automatic protostone generation

### Low Risk (Straightforward)
- 🚧 Output creation
- 🚧 Address resolution
- 🚧 Logging and debugging

---

## Performance Considerations

### Current Overhead
- Per-UTXO RPC call to `protorunesbyoutpoint`: ~10-50ms each
- BTreeMap operations: O(log n)
- Additional memory: ~100 bytes per alkane type

### Optimization Opportunities
- Batch RPC calls (if API supports it)
- Cache alkanes balances
- Lazy evaluation of excess calculation

---

## Testing Strategy

### Unit Tests (MockProvider)
1. ✅ Type checking and compilation
2. ⏳ Alkanes tracking accuracy
3. ⏳ Excess calculation correctness
4. ⏳ Protostone generation
5. ⏳ Reference adjustment

### Integration Tests (Regtest)
1. ⏳ Simple deployment
2. ⏳ Alkanes transfer with change
3. ⏳ Multi-protostone patterns
4. ⏳ Edge cases

### Manual Tests
1. ⏳ deploy-amm.sh script
2. ⏳ Real-world scenarios
3. ⏳ Error handling

---

## Documentation Status

### Completed
- ✅ alkanes-execute-scheme.md
- ✅ alkanes-execute-implementation-status.md
- ✅ ALKANES-EXECUTE-QUICK-START.md
- ✅ IMPLEMENTATION-COMPLETE-SUMMARY.md
- ✅ PROGRESS-UPDATE.md (this document)

### Needs Update
- ⏳ Quick start guide (add alkanes-change examples)
- ⏳ Implementation status (update with Phase 2 progress)
- ⏳ API documentation

---

## Summary

We've successfully implemented the **infrastructure** for alkanes change handling:
- ✅ Tracking what alkanes are in UTXOs
- ✅ Returning full inventory from UTXO selection
- ✅ Warning users about incomplete feature

**Next Steps**: Implement the **logic** for:
- 🚧 Calculating excess
- 🚧 Generating automatic protostones
- 🚧 Adjusting references
- 🚧 Creating change outputs

**Estimated Time to Completion**: 4-6 hours of focused work

The foundation is solid and well-structured. The remaining implementation is straightforward algorithmic work with clear specifications.

---

## Questions for Next Session

1. Should we batch implement all helper functions first, then integrate? Or implement incrementally with tests?

2. Do we want comprehensive unit tests before integration testing?

3. Should MockProvider be in a separate file or inline in tests?

4. Any specific edge cases we should prioritize?

---

**Session End**: Ready to continue with Phase 1 implementation.
