# Test Suite Overhaul Plan

## Overview
Redesign the test suite to leverage our new refactored architecture with comprehensive coverage, clean organization, and effective use of in-memory implementations for fast, reliable testing.

## New Test Structure

### 1. Core Framework Tests (`tests/core/`)
- **Generic Adapter Tests** - Test all generic adapter traits
- **Runtime Integration Tests** - Test MetashrewRuntime with different backends
- **JSON-RPC Framework Tests** - Test the new generic JSON-RPC server

### 2. Storage Backend Tests (`tests/backends/`)
- **In-Memory Backend Tests** - Comprehensive memshrew testing
- **RocksDB Backend Tests** - Integration tests with RocksDB
- **Cross-Backend Compatibility** - Ensure consistent behavior

### 3. End-to-End Tests (`tests/e2e/`)
- **Complete Indexing Workflow** - Full block processing pipeline
- **Historical Query Tests** - BST and view function testing
- **Chain Reorganization Tests** - Reorg handling

### 4. Integration Tests (`tests/integration/`)
- **Rockshrew-Mono Integration** - Test the production binary with mocks
- **JSON-RPC Server Integration** - Full server testing
- **Snapshot System Integration** - Comprehensive snapshot testing

### 5. Performance Tests (`tests/performance/`)
- **Benchmark Tests** - Performance regression detection
- **Memory Usage Tests** - Memory leak detection
- **Concurrent Access Tests** - Thread safety validation

## Tests to Remove

### Debug Tests (Remove Completely)
- `smt_key_debug_test.rs` - Debugging code, not actual tests
- `state_root_debug_test.rs` - Debugging code, not actual tests

### Legacy Snapshot Tests (Replace)
- `production_snapshot_test.rs` - Incomplete, replace with proper integration tests
- `snapshot_directory_creation_test.rs` - Basic functionality, merge into comprehensive tests
- `snapshot_initialization_fix_test.rs` - Specific bug fix, merge into integration tests
- `snapshot_rocksdb_lock_fix_test.rs` - Specific bug fix, merge into integration tests

### Redundant Tests (Consolidate)
- `atomic_block_processing_test.rs` - Merge into core workflow tests
- `atomic_processing_test.rs` - Merge into core workflow tests
- `state_root_gap_test.rs` - Merge into comprehensive state root tests
- `state_root_production_test.rs` - Merge into integration tests

## Tests to Keep & Enhance

### Core Tests (Enhance)
- `comprehensive_e2e_test.rs` - Enhance with new architecture
- `integration_tests.rs` - Enhance with generic adapter testing
- `runtime_tests.rs` - Enhance with cross-backend testing
- `historical_view_test.rs` - Keep as specialized test
- `reorg_focused_test.rs` - Keep as specialized test

### JSON-RPC Tests (Enhance)
- `jsonrpc_height_test.rs` - Enhance with new server framework
- `jsonrpc_preview_test.rs` - Enhance with new server framework
- `stateroot_jsonrpc_test.rs` - Enhance with new server framework

### Utility Tests (Keep)
- `block_builder.rs` - Essential utility, enhance
- `bst_verification_test.rs` - Keep for BST validation
- `surface_api_test.rs` - Keep for API validation

## New Tests to Create

### 1. Generic Adapter Framework Tests
```rust
// tests/core/generic_adapters_test.rs
- Test HeightTracker trait implementations
- Test StateRootManager trait implementations  
- Test BatchProcessor trait implementations
- Test BlockHashManager trait implementations
- Test StorageAdapterCore trait implementations
```

### 2. Cross-Backend Compatibility Tests
```rust
// tests/backends/compatibility_test.rs
- Test same operations on memshrew vs rockshrew
- Verify identical results across backends
- Test data migration between backends
```

### 3. Rockshrew-Mono Integration Tests
```rust
// tests/integration/rockshrew_mono_test.rs
- Test complete rockshrew-mono workflow with mocks
- Test JSON-RPC server integration
- Test snapshot system integration
- Test configuration handling
```

### 4. Performance Regression Tests
```rust
// tests/performance/regression_test.rs
- Benchmark block processing speed
- Benchmark view function performance
- Benchmark memory usage patterns
- Detect performance regressions
```

## Implementation Strategy

### Phase 1: Core Framework Tests
1. Create comprehensive tests for all generic adapter traits
2. Test MetashrewRuntime with both memshrew and rockshrew backends
3. Validate JSON-RPC framework functionality

### Phase 2: Integration Tests
1. Create rockshrew-mono integration tests using in-memory mocks
2. Test complete indexing workflows
3. Validate snapshot system integration

### Phase 3: Cleanup & Organization
1. Remove debug and legacy tests
2. Consolidate redundant tests
3. Organize tests into logical modules

### Phase 4: Performance & Validation
1. Add performance regression tests
2. Add comprehensive validation tests
3. Ensure 100% coverage of critical paths

## Benefits

1. **Faster Test Execution** - In-memory backends for most tests
2. **Better Coverage** - Comprehensive testing of new architecture
3. **Easier Maintenance** - Well-organized, focused tests
4. **Regression Detection** - Performance and functionality regression tests
5. **Documentation** - Tests serve as usage examples for new architecture