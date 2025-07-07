# SPIR-V Dependency Analysis for Alkanes GPU Pipeline

## Goal
Create generic implementations of all dependencies needed to run wasmi and alkanes-support in SPIR-V context, with concrete implementations for different targets (wasm32-unknown-unknown vs spirv-unknown-spv1.3).

## Core Dependencies That Need Generic Implementations

### 1. Standard Library Replacements
- **std::collections** → `alkanes-collections`
  - HashMap, BTreeMap, HashSet, BTreeSet
  - Vec, VecDeque, LinkedList
- **std::sync** → `alkanes-sync` 
  - Mutex, RwLock, Arc, Rc
  - Atomic types
- **std::thread** → `alkanes-thread`
  - Thread spawning, joining
  - Thread-local storage

### 2. Allocation Strategy
- **alloc** → `alkanes-alloc`
  - Custom allocator trait
  - SPIR-V: Fixed-size arena allocator
  - WASM32: Standard heap allocator
  - Box, Vec implementations

### 3. Error Handling & Macros
- **core macros** → `alkanes-core`
  - panic!, unreachable!, assert!
  - Result, Option (re-exports)
  - Error trait implementations

### 4. WASM Interpreter Stack
- **wasmi** → `alkanes-wasmi`
  - Core WASM interpreter
  - Generic over allocation strategy
- **wasmparser** → `alkanes-wasmparser`
  - WASM bytecode parsing
  - No-std compatible
- **indexmap** → `alkanes-indexmap`
  - Ordered maps for WASM
- **smallvec** → `alkanes-smallvec`
  - Stack-allocated vectors

### 5. Cryptographic & Utility Crates
- **ahash** → Already have `alkanes-ahash`
- **hashbrown** → Already have `alkanes-hashbrown`
- **once_cell** → Already have `alkanes-once-cell`
- **arrayvec** → Already have `alkanes-arrayvec`

### 6. Serialization (if needed)
- **serde** → `alkanes-serde`
  - Generic serialization framework
  - No-std compatible

## Implementation Strategy

### Phase 1: Core Infrastructure
1. `alkanes-alloc` - Custom allocation strategy
2. `alkanes-collections` - Generic collections
3. `alkanes-sync` - Synchronization primitives
4. `alkanes-core` - Core macros and error handling

### Phase 2: WASM Infrastructure  
1. `alkanes-wasmparser` - WASM parsing (no-std)
2. `alkanes-smallvec` - Small vector optimization
3. `alkanes-indexmap` - Ordered maps

### Phase 3: WASM Interpreter
1. `alkanes-wasmi-core` - Core WASM types and validation
2. `alkanes-wasmi` - Full WASM interpreter with generic allocation

### Phase 4: Integration
1. Update `alkanes-support` to use generic wasmi
2. Update `alkanes-gpu-shader` to provide SPIR-V implementations
3. Test end-to-end WASM execution in GPU shaders

## Target-Specific Implementations

### SPIR-V Target (`spirv-unknown-spv1.3`)
- Fixed-size arena allocator (no heap)
- Spin locks instead of OS mutexes
- Stack-based collections with compile-time limits
- Simplified error handling (no unwinding)

### WASM32 Target (`wasm32-unknown-unknown`)
- Standard heap allocator
- Web-compatible synchronization
- Dynamic collections
- Standard error handling

### Native Targets
- Standard library implementations
- OS-level synchronization
- Full heap allocation
- Complete error handling with backtraces

## Existing Crates We Can Leverage
- `alkanes-ahash` ✓
- `alkanes-hashbrown` ✓  
- `alkanes-arrayvec` ✓
- `alkanes-once-cell` ✓
- `alkanes-cfg-if` ✓
- `alkanes-wasmi-core` ✓ (partially)

## New Crates Needed
- `alkanes-alloc`
- `alkanes-collections` 
- `alkanes-sync`
- `alkanes-core`
- `alkanes-wasmparser`
- `alkanes-smallvec`
- `alkanes-indexmap`
- `alkanes-wasmi` (full rewrite)
- `alkanes-serde` (if needed)