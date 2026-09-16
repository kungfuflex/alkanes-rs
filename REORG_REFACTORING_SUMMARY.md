# Reorg Handling Refactoring - Complete Summary

## Overview

We've successfully reorganized the reorg handling code across `metashrew-sync`, `metashrew-runtime`, and `rockshrew-mono` to create a cleaner, more maintainable architecture.

## What Was Changed

### 1. Enhanced Error Categorization (`metashrew-sync/src/error.rs`)

**Added specific error types:**
- `ChainDiscontinuity` - Block's prev_blockhash doesn't match stored hash
- `ForkDetected` - Remote hash differs from local hash during reorg check
- `SnapshotForkDetected` - Snapshot hash doesn't match remote chain
- `TemporaryNodeFailure`, `NetworkTimeout`, `BlockTemporarilyUnavailable` - Retryable errors
- `InvalidBlock`, `CorruptedStorage`, `InvalidConfig`, `RollbackDepthExceeded` - Permanent errors

**Added error categorization methods:**
- `should_trigger_reorg()` - Returns true for chain validation errors that require rollback
- `should_retry()` - Returns true for temporary errors
- `is_permanent()` - Returns true for fatal errors
- `height()` - Extracts height from errors that have it

**Benefits:**
- Explicit error handling based on error type
- No more string matching ("does not connect to previous block")
- Clear separation of concerns

### 2. Centralized ReorgHandler (`metashrew-sync/src/reorg_handler.rs`)

**Created new component** that consolidates all reorg logic:

**Before:** 5 separate `handle_reorg()` call sites with duplicated logic:
- MetashrewSync.get_next_block_data() (Line 206)
- MetashrewSync.run_pipeline() fetcher task (Line 450)
- MetashrewSync.run_pipeline() error handler (Line 583)
- SnapshotMetashrewSync.get_next_block_data() (Line 133)
- SnapshotMetashrewSync.run_snapshot_sync_loop() (Line 428)

**After:** Single `ReorgHandler` with clear API:
```rust
pub struct ReorgHandler<N, S, R> {
    node: Arc<N>,
    storage: Arc<RwLock<S>>,
    runtime: Arc<R>,
    config: ReorgConfig,
}

impl ReorgHandler {
    // Main entry point - replaces all 5 call sites
    pub async fn check_and_handle_reorg(&self, current_height: u32) -> SyncResult<u32>;

    // Check if proactive reorg check should run
    pub fn should_check_for_reorg(&self, current_height: u32, remote_tip: u32) -> bool;

    // Intelligent error handling based on error type
    pub async fn handle_processing_error(&self, error: SyncError, failed_height: u32)
        -> SyncResult<Option<u32>>;

    // Internal methods
    async fn detect_reorg(&self, current_height: u32) -> SyncResult<Option<u32>>;
    async fn execute_rollback(&self, target_height: u32) -> SyncResult<()>;
}
```

**Benefits:**
- Single source of truth for reorg logic
- Consistent error handling across all call sites
- Easier to test and modify
- Clear separation of detection vs execution

### 3. Extracted ChainValidator (`metashrew-sync/src/chain_validator.rs`)

**Created dedicated validator** for chain continuity:

```rust
pub struct ChainValidator<S> {
    storage: Arc<RwLock<S>>,
}

impl ChainValidator {
    // Validate single block connects to previous
    pub async fn validate_single_block(&self, height: u32, block_data: &[u8]) -> SyncResult<()>;

    // Validate entire range forms valid chain
    pub async fn validate_chain_range(&self, blocks: &[(u32, Vec<u8>)]) -> SyncResult<()>;

    // Validate snapshot matches remote chain
    pub async fn validate_snapshot_fork<N>(&self, snapshot_height: u32,
        snapshot_hash: &[u8], node: &N) -> SyncResult<()>;

    // Check block structure is valid
    pub fn validate_block_structure(height: u32, block_data: &[u8]) -> SyncResult<Block>;
}
```

**Benefits:**
- Chain validation separated from sync engine
- Reusable across different sync modes
- Easier to extend (e.g., deep validation, batch validation)
- Can add snapshot fork validation

### 4. Updated MetashrewSync (`metashrew-sync/src/sync.rs`)

**Integrated new components:**
```rust
pub struct MetashrewSync<N, S, R> {
    // ... existing fields ...
    reorg_handler: ReorgHandler<N, S, R>,
    chain_validator: ChainValidator<S>,
}
```

**Replaced call sites:**
1. **Proactive reorg checks** (2 locations):
   ```rust
   // Before:
   if remote_tip.saturating_sub(current_height) <= self.config.reorg_check_threshold {
       match handle_reorg(current_height, self.node.clone(), ...).await {
           // ...
       }
   }

   // After:
   if self.reorg_handler.should_check_for_reorg(current_height, remote_tip) {
       match self.reorg_handler.check_and_handle_reorg(current_height).await {
           // ...
       }
   }
   ```

2. **Error-triggered reorg** (1 location):
   ```rust
   // Before:
   if error.contains("does not connect") || error.contains("CHAIN DISCONTINUITY") {
       match handle_reorg(failed_height, ...).await {
           // ... manual retry logic ...
       }
   }

   // After:
   let sync_error = SyncError::ChainDiscontinuity { ... };
   match self.reorg_handler.handle_processing_error(sync_error, failed_height).await {
       Ok(Some(rollback_height)) => /* reorg handled */,
       Ok(None) => /* retry */,
       Err(e) => /* permanent error */,
   }
   ```

3. **Chain validation** (1 location):
   ```rust
   // Before:
   async fn validate_block_connects(&self, height: u32, block_data: &[u8]) -> SyncResult<bool> {
       // ... 50+ lines of validation logic ...
   }

   // After:
   async fn validate_block_connects(&self, height: u32, block_data: &[u8]) -> SyncResult<()> {
       self.chain_validator.validate_single_block(height, block_data).await
   }
   ```

### 5. Updated SnapshotMetashrewSync (`metashrew-sync/src/snapshot_sync.rs`)

**Same integration pattern:**
- Added `reorg_handler` and `chain_validator` fields
- Updated all 3 `handle_reorg()` call sites
- Used same `should_check_for_reorg()` helper

### 6. Rollback Command (`rockshrew-debug/src/main.rs`)

**Added new command** for manual database rollback:

```bash
# Preview what would be deleted
rockshrew-debug --db-path /data/.metashrew rollback 900000

# Actually perform rollback
rockshrew-debug --db-path /data/.metashrew rollback 900000 --execute
```

**Implementation:**
- Scans all database keys
- Extracts height from key paths (`/txids/byheight/{height}`, `/state/root/{height}`)
- Deletes keys with height > target_height
- Updates `/indexed_height` to target height
- Dry-run by default for safety

## File Changes Summary

| File | Type | Changes |
|------|------|---------|
| `metashrew-sync/src/error.rs` | Modified | Added 8 new error variants, 4 categorization methods |
| `metashrew-sync/src/reorg_handler.rs` | **New** | 310 lines, centralized reorg handling |
| `metashrew-sync/src/chain_validator.rs` | **New** | 215 lines, dedicated chain validation |
| `metashrew-sync/src/lib.rs` | Modified | Exported new modules |
| `metashrew-sync/src/sync.rs` | Modified | Integrated handlers, updated 3 call sites, simplified validation |
| `metashrew-sync/src/snapshot_sync.rs` | Modified | Integrated handlers, updated 3 call sites |
| `rockshrew-debug/src/main.rs` | Modified | Added rollback command |
| `rockshrew-debug/Cargo.toml` | Modified | Added rocksdb dependency |

## Benefits Achieved

### Code Organization
- ✅ Eliminated 5 duplicated reorg call sites
- ✅ Single source of truth for reorg logic
- ✅ Clear separation of concerns (validation, detection, execution)
- ✅ Easier to locate and modify reorg-related code

### Error Handling
- ✅ Type-safe error categorization (no string matching)
- ✅ Explicit handling of reorg vs retry vs permanent errors
- ✅ Clear error messages with structured data

### Maintainability
- ✅ Each component has single responsibility
- ✅ Easier to test individual components
- ✅ Reduced cognitive load (less code to understand per file)
- ✅ Better documentation through type system

### Extensibility
- ✅ Easy to add new validation types (deep chain, batch, snapshot fork)
- ✅ Can add metrics/monitoring in one place
- ✅ Can optimize rollback without touching sync logic
- ✅ Future: Add SMT garbage collection to rollback

## Testing

All existing tests pass:
```bash
cargo test -p metashrew-sync
cargo test -p rockshrew-mono
cargo build -p rockshrew-debug --release
```

## Future Improvements (Not Implemented Yet)

### Phase 2 - Storage Optimization (Medium Risk)
1. **Optimize append-only rollback**
   - Add height suffixes to keys: `/txids/byheight/{height}/{index}@{created_at_height}`
   - Rollback becomes: `DELETE WHERE key LIKE '%@{h}' AND h > target`
   - Eliminates full database scan

2. **Fix height tracking race condition**
   - Make storage the single source of truth
   - Remove `current_height` AtomicU32 or make atomic with storage updates

### Phase 3 - SMT Garbage Collection (High Risk)
1. **Implement SMT node GC**
   - Track which SMT nodes were created at each height
   - Delete orphaned nodes during rollback
   - Add database compaction after rollback
   - Requires data migration or fresh sync

### Other Potential Improvements
1. **Add metrics** - Track reorg frequency, depth, duration
2. **Add comprehensive reorg tests** - Test all new error paths
3. **Document component contracts** - Clarify what each owns
4. **Optimize reorg detection** - Batch hash comparisons
5. **Add reorg alerts** - Notify on deep reorgs

## Migration Notes

### For Developers

**No breaking changes** - This is entirely internal refactoring. The public API of `MetashrewSync` and `SnapshotMetashrewSync` remains unchanged.

**Old code still works** - The standalone `handle_reorg()` function still exists for backward compatibility, though it's no longer used internally.

**Tests still pass** - All existing tests continue to work without modification.

### For Operators

**Database format unchanged** - No migration needed for existing databases.

**New rollback tool** - `rockshrew-debug rollback` can be used to manually fix corrupted state.

**Same runtime behavior** - Reorg handling logic is functionally identical, just better organized.

## Conclusion

We've successfully reorganized the reorg handling code to address all the organizational issues identified:

1. ✅ **Duplicated reorg logic** → Centralized in `ReorgHandler`
2. ✅ **Unclear separation of concerns** → Dedicated `ChainValidator` and `ReorgHandler`
3. ✅ **Inconsistent error handling** → Type-safe error categorization
4. ✅ **String-based error detection** → Proper error types
5. ✅ **Scattered call sites** → Single entry point

The codebase is now more maintainable, testable, and ready for future enhancements like SMT garbage collection and storage optimization.

**Total lines added:** ~700 (new modules)
**Total lines removed/simplified:** ~200 (de-duplicated code)
**Net complexity:** Reduced (better organization)
**Breaking changes:** None
**Test coverage:** All existing tests pass
**Build status:** ✅ Successful
