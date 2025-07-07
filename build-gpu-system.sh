#!/bin/bash

# Build script for alkanes-rs GPU system with SPIR-V shader compilation
# This script builds the complete system ready for GPU-accelerated indexing

set -e

echo "🚀 Building alkanes-rs GPU system..."

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
if [ ! -f "Cargo.toml" ] || [ ! -d "crates/alkanes-gpu" ]; then
    print_error "This script must be run from the alkanes-rs root directory"
    exit 1
fi

# Step 1: Build the SPIR-V shader
print_status "Step 1: Building SPIR-V shader for GPU compute..."
export ALKANES_BUILD_SPIRV=1
cargo build -p alkanes-gpu --release

if [ $? -eq 0 ]; then
    print_success "SPIR-V shader compiled successfully"
    
    # Check if the SPIR-V binary exists
    SPIRV_PATH="target/spirv-builder/spirv-unknown-spv1.3/release/deps/alkanes_gpu_shader.spv"
    if [ -f "$SPIRV_PATH" ]; then
        SPIRV_SIZE=$(stat -c%s "$SPIRV_PATH" 2>/dev/null || stat -f%z "$SPIRV_PATH" 2>/dev/null || echo "unknown")
        print_success "SPIR-V binary available: $SPIRV_PATH ($SPIRV_SIZE bytes)"
    else
        print_warning "SPIR-V binary not found at expected location: $SPIRV_PATH"
    fi
else
    print_error "Failed to build SPIR-V shader"
    exit 1
fi

# Step 2: Build the main alkanes system
print_status "Step 2: Building alkanes-rs main system..."
cargo build --release --features gpu

if [ $? -eq 0 ]; then
    print_success "Alkanes-rs system built successfully"
else
    print_error "Failed to build alkanes-rs system"
    exit 1
fi

# Step 3: Build rockshrew-mono with GPU support
print_status "Step 3: Building rockshrew-mono indexer..."
cd submodules/metashrew
cargo build -p rockshrew-mono --release

if [ $? -eq 0 ]; then
    print_success "Rockshrew-mono indexer built successfully"
    cd ../..
else
    print_error "Failed to build rockshrew-mono indexer"
    cd ../..
    exit 1
fi

# Step 4: Run tests to verify the system
print_status "Step 4: Running GPU system tests..."
export ALKANES_BUILD_SPIRV=1
cargo test -p alkanes-gpu --release -- --nocapture spirv

if [ $? -eq 0 ]; then
    print_success "GPU system tests passed"
else
    print_warning "Some GPU tests failed, but system may still be functional"
fi

# Step 5: Display system information
print_status "Step 5: System build complete!"
echo ""
echo "📋 Build Summary:"
echo "=================="

# Check binary locations
ALKANES_BINARY="target/release/alkanes"
ROCKSHREW_BINARY="submodules/metashrew/target/release/rockshrew-mono"
SPIRV_BINARY="target/spirv-builder/spirv-unknown-spv1.3/release/deps/alkanes_gpu_shader.spv"

if [ -f "$ALKANES_BINARY" ]; then
    ALKANES_SIZE=$(stat -c%s "$ALKANES_BINARY" 2>/dev/null || stat -f%z "$ALKANES_BINARY" 2>/dev/null || echo "unknown")
    print_success "✓ Alkanes binary: $ALKANES_BINARY ($ALKANES_SIZE bytes)"
else
    print_warning "✗ Alkanes binary not found: $ALKANES_BINARY"
fi

if [ -f "$ROCKSHREW_BINARY" ]; then
    ROCKSHREW_SIZE=$(stat -c%s "$ROCKSHREW_BINARY" 2>/dev/null || stat -f%z "$ROCKSHREW_BINARY" 2>/dev/null || echo "unknown")
    print_success "✓ Rockshrew-mono binary: $ROCKSHREW_BINARY ($ROCKSHREW_SIZE bytes)"
else
    print_warning "✗ Rockshrew-mono binary not found: $ROCKSHREW_BINARY"
fi

if [ -f "$SPIRV_BINARY" ]; then
    SPIRV_SIZE=$(stat -c%s "$SPIRV_BINARY" 2>/dev/null || stat -f%z "$SPIRV_BINARY" 2>/dev/null || echo "unknown")
    print_success "✓ SPIR-V shader: $SPIRV_BINARY ($SPIRV_SIZE bytes)"
else
    print_warning "✗ SPIR-V shader not found: $SPIRV_BINARY"
fi

echo ""
echo "🎯 Usage Examples:"
echo "=================="
echo ""
echo "1. Run rockshrew-mono with GPU shader:"
echo "   $ROCKSHREW_BINARY \\"
echo "     --daemon-rpc-url http://localhost:8332 \\"
echo "     --indexer ./path/to/indexer.wasm \\"
echo "     --db-path ./db \\"
echo "     --use-shader $SPIRV_BINARY \\"
echo "     --compute-size 64"
echo ""
echo "2. Run with both Vulkan pipeline and SPIR-V shader:"
echo "   $ROCKSHREW_BINARY \\"
echo "     --daemon-rpc-url http://localhost:8332 \\"
echo "     --indexer ./path/to/indexer.wasm \\"
echo "     --db-path ./db \\"
echo "     --pipeline ./path/to/vulkan_pipeline.bin \\"
echo "     --use-shader $SPIRV_BINARY \\"
echo "     --compute-size 128"
echo ""
echo "3. Test SPIR-V shader compilation:"
echo "   ALKANES_BUILD_SPIRV=1 cargo test -p alkanes-gpu --release -- --nocapture spirv"
echo ""

# Check for GPU/Vulkan support
print_status "Checking GPU/Vulkan support..."
if command -v vulkaninfo >/dev/null 2>&1; then
    print_success "✓ Vulkan tools available (vulkaninfo found)"
    VULKAN_DEVICES=$(vulkaninfo --summary 2>/dev/null | grep -c "GPU" || echo "0")
    if [ "$VULKAN_DEVICES" -gt 0 ]; then
        print_success "✓ $VULKAN_DEVICES Vulkan-capable GPU(s) detected"
    else
        print_warning "⚠ No Vulkan-capable GPUs detected"
    fi
else
    print_warning "⚠ Vulkan tools not found (install vulkan-tools for GPU support)"
fi

echo ""
print_success "🎉 GPU system build complete! Ready for end-to-end testing."
echo ""
echo "Next steps:"
echo "1. Set up a Bitcoin node (testnet recommended for testing)"
echo "2. Create or obtain an alkanes indexer WASM file"
echo "3. Run rockshrew-mono with the --use-shader flag"
echo "4. Monitor GPU utilization and performance"