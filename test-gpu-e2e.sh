#!/bin/bash

# End-to-end test script for alkanes-rs GPU system
# This script tests the complete GPU pipeline from SPIR-V compilation to indexer integration

set -e

echo "🧪 Testing alkanes-rs GPU system end-to-end..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

# Test counter
TESTS_RUN=0
TESTS_PASSED=0

run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TESTS_RUN=$((TESTS_RUN + 1))
    print_status "Running: $test_name"
    
    if eval "$test_command"; then
        print_success "$test_name"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        print_error "$test_name"
        return 1
    fi
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "crates/alkanes-gpu" ]; then
    print_error "This script must be run from the alkanes-rs root directory"
    exit 1
fi

echo ""
echo "🔧 Test Suite: alkanes-rs GPU System"
echo "===================================="

# Test 1: SPIR-V Compilation
run_test "SPIR-V shader compilation" \
    "ALKANES_BUILD_SPIRV=1 cargo build -p alkanes-gpu --release"

# Test 2: SPIR-V Binary Verification
run_test "SPIR-V binary verification" \
    "[ -f 'target/spirv-builder/spirv-unknown-spv1.3/release/deps/alkanes_gpu_shader.spv' ]"

# Test 3: SPIR-V Tests
run_test "SPIR-V integration tests" \
    "ALKANES_BUILD_SPIRV=1 cargo test -p alkanes-gpu --release -- --nocapture spirv"

# Test 4: GPU Pointer Tests
run_test "GPU pointer and ejection tests" \
    "cargo test -p alkanes-gpu --release -- --nocapture gpu_pointer"

# Test 5: Host Functions Tests
run_test "GPU host functions tests" \
    "cargo test -p alkanes-gpu --release -- --nocapture host_functions"

# Test 6: Rockshrew-mono Build
run_test "Rockshrew-mono indexer build" \
    "cd submodules/metashrew && cargo build -p rockshrew-mono --release && cd ../.."

# Test 7: Rockshrew-mono Help (verify new flags)
run_test "Rockshrew-mono GPU flags verification" \
    "submodules/metashrew/target/release/rockshrew-mono --help | grep -q 'use-shader'"

# Test 8: Alkanes System Build
run_test "Alkanes system build with GPU features" \
    "cargo build --release --features gpu"

# Test 9: SPIR-V Binary Size Check
SPIRV_PATH="target/spirv-builder/spirv-unknown-spv1.3/release/deps/alkanes_gpu_shader.spv"
if [ -f "$SPIRV_PATH" ]; then
    SPIRV_SIZE=$(stat -c%s "$SPIRV_PATH" 2>/dev/null || stat -f%z "$SPIRV_PATH" 2>/dev/null || echo "0")
    if [ "$SPIRV_SIZE" -gt 1000 ]; then
        run_test "SPIR-V binary size validation (>1KB)" "true"
        print_status "SPIR-V binary size: $SPIRV_SIZE bytes"
    else
        run_test "SPIR-V binary size validation (>1KB)" "false"
    fi
else
    run_test "SPIR-V binary size validation (>1KB)" "false"
fi

# Test 10: GPU Data Structure Compatibility
run_test "GPU data structure compatibility" \
    "cargo test -p alkanes-gpu --release -- --nocapture data_structure"

echo ""
echo "📊 Test Results Summary"
echo "======================"
echo "Tests run: $TESTS_RUN"
echo "Tests passed: $TESTS_PASSED"
echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"

if [ $TESTS_PASSED -eq $TESTS_RUN ]; then
    print_success "🎉 All tests passed! GPU system is ready for production use."
    echo ""
    echo "✅ System Status: READY"
    echo "✅ SPIR-V Compilation: WORKING"
    echo "✅ GPU Pipeline: FUNCTIONAL"
    echo "✅ Indexer Integration: READY"
    echo ""
    echo "🚀 Next Steps:"
    echo "1. Set up Bitcoin node (testnet recommended)"
    echo "2. Create alkanes indexer WASM"
    echo "3. Run: ./submodules/metashrew/target/release/rockshrew-mono \\"
    echo "     --daemon-rpc-url http://localhost:18332 \\"
    echo "     --indexer ./indexer.wasm \\"
    echo "     --db-path ./testdb \\"
    echo "     --use-shader $SPIRV_PATH \\"
    echo "     --compute-size 64"
    
    exit 0
else
    print_error "❌ Some tests failed. System may not be fully functional."
    echo ""
    echo "❌ System Status: NEEDS ATTENTION"
    echo "⚠️  Failed tests: $((TESTS_RUN - TESTS_PASSED))/$TESTS_RUN"
    echo ""
    echo "🔧 Troubleshooting:"
    echo "1. Check build dependencies (Rust, Vulkan SDK)"
    echo "2. Verify GPU/Vulkan support: vulkaninfo"
    echo "3. Re-run build: ./build-gpu-system.sh"
    echo "4. Check individual test failures above"
    
    exit 1
fi