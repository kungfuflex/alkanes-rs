# Alkanes-RS GPU System Setup Guide

This guide covers setting up and using the GPU-accelerated alkanes indexing system with SPIR-V compute shaders.

## 🎯 Overview

The alkanes-rs GPU system provides parallel message processing using GPU compute shaders compiled to SPIR-V. This enables significant performance improvements for alkanes indexing workloads.

### Key Components

- **alkanes-gpu**: Host-side GPU pipeline with ejection capabilities
- **alkanes-gpu-shader**: SPIR-V compute shader for actual GPU execution
- **rockshrew-mono**: Indexer with GPU integration (`--use-shader`, `--compute-size`)

## 🔧 Prerequisites

### System Requirements

1. **Rust Toolchain**: Latest stable Rust with `spirv` target support
2. **Vulkan SDK**: For GPU compute support
3. **GPU Hardware**: Vulkan-compatible GPU (NVIDIA, AMD, or Intel)

### Installation

```bash
# Install Vulkan SDK (Ubuntu/Debian)
sudo apt update
sudo apt install vulkan-tools libvulkan-dev vulkan-validationlayers-dev

# Install Vulkan SDK (macOS)
brew install vulkan-headers vulkan-loader vulkan-tools

# Verify Vulkan installation
vulkaninfo --summary
```

## 🚀 Quick Start

### 1. Build the Complete System

```bash
# Build everything with GPU support
./build-gpu-system.sh
```

This script will:
- Compile the SPIR-V shader (`alkanes_gpu_shader.spv`)
- Build the alkanes-rs system with GPU features
- Build rockshrew-mono with GPU flags
- Run verification tests

### 2. Run End-to-End Tests

```bash
# Verify the complete system
./test-gpu-e2e.sh
```

### 3. Start GPU-Accelerated Indexing

```bash
# Run with GPU compute acceleration
./submodules/metashrew/target/release/rockshrew-mono \
  --daemon-rpc-url http://localhost:18332 \
  --indexer ./path/to/indexer.wasm \
  --db-path ./testdb \
  --use-shader ./target/spirv-builder/spirv-unknown-spv1.3/release/deps/alkanes_gpu_shader.spv \
  --compute-size 64
```

## 📋 Command Line Options

### New GPU Flags

- `--use-shader <PATH>`: Path to SPIR-V shader binary for GPU compute
- `--compute-size <SIZE>`: Number of parallel compute shards (default: 64)

### Existing Vulkan Flags

- `--pipeline <PATH>`: Path to Vulkan GPU pipeline binary

### Example Configurations

```bash
# GPU compute only
rockshrew-mono \
  --daemon-rpc-url http://localhost:8332 \
  --indexer ./indexer.wasm \
  --db-path ./db \
  --use-shader ./alkanes_gpu_shader.spv \
  --compute-size 128

# Both Vulkan pipeline and GPU compute
rockshrew-mono \
  --daemon-rpc-url http://localhost:8332 \
  --indexer ./indexer.wasm \
  --db-path ./db \
  --pipeline ./vulkan_pipeline.bin \
  --use-shader ./alkanes_gpu_shader.spv \
  --compute-size 64

# CPU fallback (no GPU)
rockshrew-mono \
  --daemon-rpc-url http://localhost:8332 \
  --indexer ./indexer.wasm \
  --db-path ./db
```

## 🏗️ Architecture

### GPU Pipeline Flow

```
Bitcoin Blocks → Alkanes Messages → GPU Shards → SPIR-V Compute → Results
                                        ↓
                                   Ejection Detection
                                        ↓
                                   CPU Fallback (if needed)
```

### Key Features

1. **Shard Processing**: Messages grouped into shards for parallel GPU execution
2. **Ejection System**: Automatic fallback to CPU when GPU constraints exceeded
3. **Message Ordering**: Preserved during ejection to maintain consistency
4. **Constraint Checking**: GPU memory and processing limits enforced

### Data Structures

- **GpuExecutionShard**: Input data for GPU compute (messages + context)
- **GpuExecutionResult**: Output from GPU with results and ejection info
- **GpuAtomicPointer**: GPU-aware storage with ejection detection

## 🧪 Testing

### Unit Tests

```bash
# Test SPIR-V compilation and integration
ALKANES_BUILD_SPIRV=1 cargo test -p alkanes-gpu --release -- --nocapture spirv

# Test GPU pointers and ejection
cargo test -p alkanes-gpu --release -- --nocapture gpu_pointer

# Test host functions
cargo test -p alkanes-gpu --release -- --nocapture host_functions
```

### Integration Tests

```bash
# Complete end-to-end test suite
./test-gpu-e2e.sh

# Manual SPIR-V compilation test
ALKANES_BUILD_SPIRV=1 cargo build -p alkanes-gpu --release
```

### Performance Testing

```bash
# Monitor GPU utilization during indexing
nvidia-smi -l 1  # NVIDIA GPUs
radeontop        # AMD GPUs

# Check SPIR-V binary size and validity
ls -la target/spirv-builder/spirv-unknown-spv1.3/release/deps/alkanes_gpu_shader.spv
```

## 🔍 Troubleshooting

### Common Issues

#### 1. SPIR-V Compilation Fails

```bash
# Check Rust GPU toolchain
rustup target list | grep spirv

# Rebuild with verbose output
ALKANES_BUILD_SPIRV=1 cargo build -p alkanes-gpu --release -v
```

#### 2. Vulkan Not Found

```bash
# Check Vulkan installation
vulkaninfo
ldd $(which vulkaninfo)

# Install Vulkan SDK
# Ubuntu: sudo apt install vulkan-tools libvulkan-dev
# macOS: brew install vulkan-headers vulkan-loader
```

#### 3. GPU Not Detected

```bash
# Check GPU support
vulkaninfo --summary | grep GPU
lspci | grep VGA

# Check driver installation
nvidia-smi     # NVIDIA
rocm-smi       # AMD
```

#### 4. Build Errors

```bash
# Clean and rebuild
cargo clean
./build-gpu-system.sh

# Check dependencies
cargo tree -p alkanes-gpu
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug ./submodules/metashrew/target/release/rockshrew-mono \
  --use-shader ./alkanes_gpu_shader.spv \
  --compute-size 32 \
  [other args...]
```

## 📊 Performance Tuning

### Compute Size Optimization

- **Small shards (16-32)**: Lower latency, higher CPU overhead
- **Medium shards (64-128)**: Balanced performance (recommended)
- **Large shards (256+)**: Higher throughput, higher memory usage

### GPU Memory Considerations

- Monitor GPU memory usage: `nvidia-smi` or `radeontop`
- Reduce `--compute-size` if running out of GPU memory
- Consider message complexity when sizing shards

### Ejection Monitoring

Watch for ejection messages in logs:
```
[INFO] GPU constraint violation - ejecting shard to CPU
[INFO] Ejection reason: storage_overflow
```

High ejection rates may indicate:
- Compute size too large
- Messages too complex for GPU constraints
- Need for CPU-only processing

## 🔧 Development

### Building Custom Shaders

```bash
# Modify alkanes-gpu-shader/src/lib.rs
# Rebuild with SPIR-V
ALKANES_BUILD_SPIRV=1 cargo build -p alkanes-gpu --release

# Test new shader
./test-gpu-e2e.sh
```

### Adding GPU Host Functions

1. Implement in `alkanes-gpu/src/gpu_host_functions.rs`
2. Add to `GpuHostFunctions` trait
3. Update SPIR-V shader if needed
4. Add tests

### Debugging GPU Execution

```bash
# Enable GPU debug logging
RUST_LOG=alkanes_gpu=debug,metashrew_runtime=debug \
  rockshrew-mono [args...]

# Use CPU fallback for comparison
rockshrew-mono [args without --use-shader]
```

## 📚 References

- [SPIR-V Specification](https://www.khronos.org/registry/spir-v/)
- [Vulkan Documentation](https://vulkan.lunarg.com/)
- [Rust GPU Project](https://github.com/EmbarkStudios/rust-gpu)
- [Alkanes Protocol](https://github.com/kungfuflex/alkanes-rs)

## 🤝 Contributing

1. Test changes with `./test-gpu-e2e.sh`
2. Ensure SPIR-V compilation works
3. Add tests for new GPU functionality
4. Update documentation

## 📄 License

MIT License - see LICENSE file for details.