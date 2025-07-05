#!/bin/bash

# Dual compilation script for alkanes-rs
# Builds both WASM and Vulkan targets

set -e

echo "🔧 Building alkanes-rs for dual targets (WASM + Vulkan)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Must be run from alkanes-rs root directory"
    exit 1
fi

# Create target directories
mkdir -p target/wasm32-unknown-unknown/release
mkdir -p target/vulkan/release

print_status "Building WASM target..."

# Build WASM target (normal alkanes.wasm) without GPU dependencies
export RUSTUP_TOOLCHAIN=1.86.0
cargo build --target wasm32-unknown-unknown --release --no-default-features

if [ $? -eq 0 ]; then
    print_success "WASM build completed"
    
    # Copy WASM binary to expected location
    if [ -f "target/wasm32-unknown-unknown/release/alkanes.wasm" ]; then
        print_status "WASM binary: target/wasm32-unknown-unknown/release/alkanes.wasm"
    else
        print_warning "WASM binary not found at expected location"
        ls -la target/wasm32-unknown-unknown/release/
    fi
else
    print_error "WASM build failed"
    exit 1
fi

print_status "Building Vulkan target..."

# Build Vulkan target (alkanes.vulkan with __pipeline function)
# Use the separate vulkan-pipeline directory

cd vulkan-pipeline

# Build for native target with Vulkan features
cargo build --target x86_64-unknown-linux-gnu --release --features vulkan

if [ $? -eq 0 ]; then
    print_success "Vulkan build completed"
    
    # Copy Vulkan binary to expected location
    mkdir -p ../target/vulkan/release
    cp target/x86_64-unknown-linux-gnu/release/libalkanes_vulkan_pipeline.so ../target/vulkan/release/alkanes.vulkan
    
    print_status "Vulkan binary: target/vulkan/release/alkanes.vulkan"
else
    print_error "Vulkan build failed"
    cd ..
    exit 1
fi

cd ..

print_success "Dual compilation completed successfully!"
print_status "Built targets:"
print_status "  - WASM: target/wasm32-unknown-unknown/release/alkanes.wasm"
print_status "  - Vulkan: target/vulkan/release/alkanes.vulkan"

echo ""
print_status "Usage with rockshrew-mono:"
print_status "  Normal: rockshrew-mono --wasm target/wasm32-unknown-unknown/release/alkanes.wasm"
print_status "  GPU:    rockshrew-mono --wasm target/wasm32-unknown-unknown/release/alkanes.wasm --use-vulkan target/vulkan/release/alkanes.vulkan"

echo ""
print_success "🚀 Ready for GPU-accelerated alkanes indexing!"