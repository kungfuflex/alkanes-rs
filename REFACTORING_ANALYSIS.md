# Metashrew Stack Refactoring Analysis

## Current Architecture Overview

The Metashrew stack follows a layered architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                    rockshrew-mono                           │
│  (Production Binary - Should be lightweight wrapper)       │
├─────────────────────────────────────────────────────────────┤
│              rockshrew-sync + rockshrew-runtime             │
│         (RocksDB-specific implementations)                  │
├─────────────────────────────────────────────────────────────┤
│              metashrew-sync + metashrew-runtime             │
│            (Generic framework + core logic)                │
├─────────────────────────────────────────────────────────────┤
│                    memshrew-runtime                         │
│              (In-memory test implementations)               │
└─────────────────────────────────────────────────────────────┘
```

## Issues Identified

### 1. **Code Duplication in Adapters**

**Problem**: Similar adapter logic exists in multiple places:
- `crates/rockshrew-mono/src/adapters.rs` (655 lines)
- `crates/rockshrew-runtime/src/adapter.rs` (254 lines)
- `crates/memshrew/src/adapter.rs` (263 lines)

**Specific Duplications**:
- Height querying logic (`query_height` functions)
- Key-value store implementations
- Batch operations
- State root management

### 2. **Heavy rockshrew-mono Implementation**

**Problem**: `rockshrew-mono/src/main.rs` is 992 lines and contains:
- JSON-RPC server implementation (400+ lines)
- Adapter creation logic
- Configuration parsing
- Database setup
- Snapshot management

**Should be**: Lightweight wrapper that composes existing components.

### 3. **Inconsistent Test Infrastructure**

**Problem**: Tests use different patterns:
- Some tests directly use `MemStoreRuntime`
- Others create custom adapters
- Inconsistent mocking approaches
- No unified test framework

### 4. **Missing Generic Implementations**

**Problem**: Common functionality is reimplemented:
- JSON-RPC handling
- Height tracking
- State root calculation
- Block hash storage

## Refactoring Plan

### Phase 1: Extract Common Adapter Logic

#### 1.1 Create Generic Adapter Traits in `metashrew-runtime`

```rust
// crates/metashrew-runtime/src/adapters/mod.rs
pub mod traits;
pub mod height_tracker;
pub mod state_root_manager;
pub mod batch_processor;

// Common functionality that all adapters need
pub trait HeightTracker {
    async fn get_current_height(&self) -> Result<u32>;
    async fn set_current_height(&self, height: u32) -> Result<()>;
}

pub trait StateRootManager {
    async fn store_state_root(&self, height: u32, root: &[u8]) -> Result<()>;
    async fn get_state_root(&self, height: u32) -> Result<Option<Vec<u8>>>;
}
```

#### 1.2 Move Common Logic to `metashrew-runtime`

**Files to create**:
- `crates/metashrew-runtime/src/adapters/height_tracker.rs`
- `crates/metashrew-runtime/src/adapters/state_root_manager.rs`
- `crates/metashrew-runtime/src/adapters/batch_processor.rs`

### Phase 2: Create Generic JSON-RPC Framework

#### 2.1 Extract JSON-RPC Logic to `metashrew-sync`

```rust
// crates/metashrew-sync/src/jsonrpc/mod.rs
pub mod server;
pub mod handlers;
pub mod types;

// Generic JSON-RPC server that works with any storage/runtime
pub struct MetashrewJsonRpcServer<S, R> 
where 
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    storage: Arc<RwLock<S>>,
    runtime: Arc<RwLock<R>>,
    current_height: Arc<AtomicU32>,
}
```

#### 2.2 Standardize RPC Methods

**Common methods that should be generic**:
- `metashrew_view`
- `metashrew_preview`
- `metashrew_height`
- `metashrew_getblockhash`
- `metashrew_stateroot`
- `metashrew_snapshot`

### Phase 3: Unify Test Infrastructure

#### 3.1 Create Unified Test Framework

```rust
// crates/metashrew-sync/src/testing/mod.rs
pub mod framework;
pub mod builders;
pub mod assertions;

pub struct TestFramework<S, R> 
where 
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    storage: S,
    runtime: R,
    sync_engine: SyncEngine,
}

impl TestFramework<MockStorage, MockRuntime> {
    pub fn new_mock() -> Self { /* ... */ }
}

impl TestFramework<MemStoreAdapter, MemStoreRuntime> {
    pub fn new_memstore() -> Self { /* ... */ }
}
```

#### 3.2 Standardize Test Patterns

**Create common test utilities**:
- Block builders
- Chain simulators
- Assertion helpers
- Mock data generators

### Phase 4: Simplify rockshrew-mono

#### 4.1 Extract Configuration Management

```rust
// crates/rockshrew-sync/src/config/mod.rs
pub struct MetashrewConfig {
    pub database: DatabaseConfig,
    pub rpc: RpcConfig,
    pub sync: SyncConfig,
    pub snapshot: Option<SnapshotConfig>,
}

impl MetashrewConfig {
    pub fn from_args(args: Args) -> Self { /* ... */ }
    pub fn create_adapters(&self) -> (BitcoinAdapter, StorageAdapter, RuntimeAdapter) { /* ... */ }
}
```

#### 4.2 Create Generic Server Builder

```rust
// crates/metashrew-sync/src/server/mod.rs
pub struct MetashrewServer<B, S, R> 
where 
    B: BitcoinNodeAdapter,
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    bitcoin: B,
    storage: Arc<RwLock<S>>,
    runtime: Arc<RwLock<R>>,
    config: ServerConfig,
}

impl<B, S, R> MetashrewServer<B, S, R> {
    pub async fn start(&mut self) -> Result<()> { /* ... */ }
}
```

#### 4.3 Reduce rockshrew-mono to ~200 lines

**New structure**:
```rust
// crates/rockshrew-mono/src/main.rs (target: ~200 lines)
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = MetashrewConfig::from_args(args);
    let (bitcoin, storage, runtime) = config.create_adapters().await?;
    
    let mut server = MetashrewServer::new(bitcoin, storage, runtime, config.server);
    server.start().await
}
```

## Implementation Strategy

### Step 1: Create Foundation (Week 1)
1. Add adapter traits to `metashrew-runtime`
2. Extract common height tracking logic
3. Create generic state root management

### Step 2: JSON-RPC Framework (Week 1)
1. Extract JSON-RPC types to `metashrew-sync`
2. Create generic server implementation
3. Move RPC handlers to generic framework

### Step 3: Test Infrastructure (Week 1)
1. Create unified test framework
2. Standardize mock implementations
3. Update existing tests to use new framework

### Step 4: Simplify rockshrew-mono (Week 1)
1. Extract configuration management
2. Create server builder
3. Reduce main.rs to minimal wrapper

### Step 5: Update Dependent Crates (Week 1)
1. Update `rockshrew-runtime` to use new traits
2. Update `memshrew` to use new framework
3. Ensure all tests pass

## Benefits

### 1. **Reduced Code Duplication**
- Eliminate ~400 lines of duplicated adapter logic
- Single source of truth for common operations
- Easier maintenance and bug fixes

### 2. **Improved Test Coverage**
- Unified test framework ensures consistency
- Better mock implementations
- Easier to write comprehensive tests

### 3. **Better Separation of Concerns**
- `rockshrew-mono` becomes a thin wrapper
- Core logic lives in appropriate crates
- Clear dependency hierarchy

### 4. **Enhanced Reusability**
- Generic components can be used in other projects
- Easier to create new storage backends
- Simplified integration testing

## File Structure After Refactoring

```
crates/
├── metashrew-runtime/
│   ├── src/adapters/           # NEW: Common adapter logic
│   ├── src/jsonrpc/           # NEW: Generic JSON-RPC types
│   └── src/testing/           # NEW: Test utilities
├── metashrew-sync/
│   ├── src/server/            # NEW: Generic server framework
│   ├── src/config/            # NEW: Configuration management
│   └── src/testing/           # NEW: Test framework
├── rockshrew-mono/
│   └── src/main.rs            # SIMPLIFIED: ~200 lines
├── rockshrew-runtime/
│   └── src/adapter.rs         # SIMPLIFIED: RocksDB-specific only
└── memshrew/
    └── src/adapter.rs         # SIMPLIFIED: Memory-specific only
```

## Migration Path

### Backward Compatibility
- All existing APIs remain unchanged
- Tests continue to work during migration
- Gradual migration of functionality

### Risk Mitigation
- Implement new framework alongside existing code
- Migrate one component at a time
- Comprehensive testing at each step

This refactoring will result in a cleaner, more maintainable codebase that better reflects the intended architecture while eliminating significant code duplication.