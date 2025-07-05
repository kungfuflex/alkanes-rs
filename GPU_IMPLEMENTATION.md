# Alkanes GPU Acceleration Implementation

This document describes the complete GPU acceleration implementation for alkanes-rs using Vulkan compute shaders for parallel message processing.

## Overview

The GPU acceleration system enables parallel processing of alkanes protocol messages that access disjoint storage slots, significantly improving indexing performance for blocks with many independent transactions.

## Architecture

### Dual Compilation System

The implementation uses a dual compilation approach:

1. **WASM Target** (`alkanes.wasm`): Standard metashrew indexer
2. **Vulkan Target** (`alkanes.vulkan`): GPU-accelerated version with `__pipeline` function

### Key Components

#### 1. Dependency Tracking (`protorune/src/gpu_tracking.rs`)
- Tracks storage slot access patterns for each transaction
- Identifies conflicts between transactions
- Groups non-conflicting transactions for parallel execution
- Provides detailed statistics on parallelization opportunities

#### 2. GPU Pipeline (`protorune/src/gpu_pipeline.rs`)
- Connects dependency analysis to GPU execution
- Serializes message batches for GPU processing
- Handles fallback to CPU when GPU is unavailable
- Integrates with the main indexer workflow

#### 3. Vulkan Runtime (`metashrew-runtime/src/vulkan_runtime.rs`)
- Real Vulkan device management and initialization
- Memory allocation and buffer management
- Compute shader execution with SPIR-V binaries
- Thread-safe global context management

#### 4. GPU Compute Shaders (`alkanes-gpu/`)
- Rust-GPU based compute shaders compiled to SPIR-V
- GPU-compatible data structures for message processing
- Host-side interface for CPU fallback and testing

#### 5. Host Function Integration (`protorune/src/gpu_abi.rs`)
- Defines the interface between WASM and Vulkan execution
- Handles data serialization/deserialization
- Provides the bridge for metashrew runtime calls

## Building

### Prerequisites

```bash
# Install Rust with required targets
rustup target add wasm32-unknown-unknown
rustup target add x86_64-unknown-linux-gnu

# Install Vulkan development libraries (Ubuntu/Debian)
sudo apt-get install vulkan-tools libvulkan-dev vulkan-validationlayers-dev spirv-tools

# Or on macOS
brew install vulkan-headers vulkan-loader vulkan-tools spirv-tools
```

### Dual Compilation

Use the provided build script to compile both targets:

```bash
cd alkanes-rs
./build-dual.sh
```

This creates:
- `target/wasm32-unknown-unknown/release/alkanes.wasm` - Standard WASM indexer
- `target/vulkan/release/alkanes.vulkan` - GPU-accelerated version

### Manual Building

#### WASM Target (Standard)
```bash
cargo build --target wasm32-unknown-unknown --release
```

#### Vulkan Target (GPU)
```bash
# Use Vulkan-specific configuration
cp Cargo-vulkan.toml Cargo.toml
cp src/lib-vulkan.rs src/lib.rs
cargo build --target x86_64-unknown-linux-gnu --release --features vulkan
```

## Usage

### With rockshrew-mono

#### Standard CPU-only indexing:
```bash
rockshrew-mono --wasm target/wasm32-unknown-unknown/release/alkanes.wasm
```

#### GPU-accelerated indexing:
```bash
rockshrew-mono \
  --wasm target/wasm32-unknown-unknown/release/alkanes.wasm \
  --use-vulkan target/vulkan/release/alkanes.vulkan
```

### Configuration

The GPU acceleration system automatically:
- Detects available Vulkan-compatible GPUs
- Analyzes transaction dependencies
- Decides when GPU acceleration is beneficial
- Falls back to CPU processing when needed

#### Minimum Requirements for GPU Acceleration
- At least 4 parallelizable transactions per block
- Vulkan-compatible GPU with compute shader support
- Sufficient GPU memory for message batches

## Performance

### Dependency Analysis Statistics

The system provides detailed logging of parallelization opportunities:

```
Block 12345 dependency analysis (GPU): 156 total, 89 parallelizable, 12 conflicts, 8 groups, largest group: 23, 57.05% parallel ratio, 15 storage profiles
```

### GPU vs CPU Performance

Expected performance improvements:
- **High parallelism blocks**: 2-5x speedup
- **Medium parallelism blocks**: 1.5-2x speedup  
- **Low parallelism blocks**: CPU fallback (no overhead)

## Implementation Details

### Storage Conflict Detection

The dependency tracker identifies conflicts by:
1. Monitoring all storage slot accesses per transaction
2. Building conflict graphs between transactions
3. Using graph coloring to find independent groups
4. Optimizing group sizes for GPU batch processing

### GPU Memory Management

- **Input Buffers**: Serialized message batches with metadata
- **Output Buffers**: Processing results and storage updates
- **Storage Buffers**: Pre-loaded storage context for messages
- **Compute Shaders**: SPIR-V binaries compiled from Rust-GPU

### Error Handling

The system gracefully handles:
- GPU device initialization failures
- Vulkan driver issues
- Memory allocation failures
- Compute shader compilation errors
- Runtime execution errors

All errors result in automatic fallback to CPU processing.

## Testing

### Unit Tests
```bash
# Test dependency tracking
cargo test --target x86_64-unknown-linux-gnu --features gpu gpu_tracking

# Test GPU pipeline
cargo test --target x86_64-unknown-linux-gnu --features gpu gpu_pipeline

# Test Vulkan runtime
cargo test --target x86_64-unknown-linux-gnu --features gpu vulkan_runtime
```

### Integration Tests
```bash
# Test dual compilation
./build-dual.sh

# Test GPU acceleration with sample data
cargo test --target x86_64-unknown-linux-gnu --features gpu --test integration
```

## Debugging

### Enable GPU Logging
```bash
export RUST_LOG=debug
export VK_LAYER_PATH=/usr/share/vulkan/explicit_layer.d
```

### Vulkan Validation Layers
```bash
export VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation
```

### Force CPU Fallback
```bash
export ALKANES_DISABLE_GPU=1
```

## Troubleshooting

### Common Issues

#### "No Vulkan-compatible GPU found"
- Install proper GPU drivers
- Verify Vulkan support: `vulkaninfo`
- Check GPU compute capability

#### "SPIR-V compilation failed"
- Ensure spirv-tools are installed
- Check Rust-GPU toolchain setup
- Verify target architecture support

#### "GPU execution timeout"
- Reduce batch sizes in configuration
- Check GPU memory availability
- Monitor GPU utilization

### Performance Tuning

#### Batch Size Optimization
```rust
// In gpu_pipeline.rs
const MIN_GPU_BATCH_SIZE: usize = 4;    // Increase for higher GPU utilization
const MAX_GPU_BATCH_SIZE: usize = 1024; // Decrease if memory limited
```

#### Memory Usage
```rust
// In gpu_abi.rs  
const MAX_KV_PAIRS: usize = 1024;       // Adjust based on GPU memory
const MAX_SHARD_SIZE: usize = 64;       // Optimize for GPU cores
```

## Future Enhancements

### Planned Features
- [ ] Multi-GPU support for larger blocks
- [ ] Dynamic batch size optimization
- [ ] GPU memory pool management
- [ ] Advanced conflict resolution algorithms
- [ ] Real-time performance monitoring
- [ ] Automatic GPU/CPU hybrid scheduling

### Optimization Opportunities
- [ ] Custom SPIR-V optimizations
- [ ] Persistent GPU context caching
- [ ] Asynchronous GPU execution
- [ ] Memory-mapped GPU buffers
- [ ] Compute shader specialization

## Contributing

When contributing to GPU acceleration:

1. **Test both targets**: Ensure changes work for WASM and Vulkan
2. **Maintain fallback**: CPU processing must always work
3. **Add comprehensive tests**: Cover error conditions and edge cases
4. **Document performance impact**: Measure before/after performance
5. **Follow safety guidelines**: GPU code requires extra safety considerations

## License

This GPU acceleration implementation is part of alkanes-rs and follows the same MIT license.