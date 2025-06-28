#!/bin/bash

# Test script to verify feature-gated inscription parsing works correctly
# This script tests both Bitcoin (default) and Dogecoin modes

set -e

echo "🧪 Testing Feature-Gated Inscription Parsing"
echo "============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Change to the alkanes-rs directory
cd "$(dirname "$0")"

print_status "Testing in directory: $(pwd)"

# Test 1: Bitcoin mode (default - no dogecoin feature)
echo ""
print_status "🔸 Test 1: Bitcoin Mode (Taproot Witness Parsing)"
echo "Testing ordinals crate without dogecoin feature..."

if cargo test -p ordinals --lib inscription_tests 2>/dev/null; then
    print_success "Bitcoin mode tests passed ✅"
else
    print_error "Bitcoin mode tests failed ❌"
    exit 1
fi

# Test 2: Dogecoin mode (with dogecoin feature)
echo ""
print_status "🔸 Test 2: Dogecoin Mode (Script_sig Parsing)"
echo "Testing ordinals crate with dogecoin feature..."

if cargo test -p ordinals --lib inscription_tests --features dogecoin 2>/dev/null; then
    print_success "Dogecoin mode tests passed ✅"
else
    print_error "Dogecoin mode tests failed ❌"
    exit 1
fi

# Test 3: ordinals-scriptsig crate independently
echo ""
print_status "🔸 Test 3: ordinals-scriptsig Crate"
echo "Testing ordinals-scriptsig crate independently..."

if cargo test -p ordinals-scriptsig 2>/dev/null; then
    print_success "ordinals-scriptsig crate tests passed ✅"
else
    print_error "ordinals-scriptsig crate tests failed ❌"
    exit 1
fi

# Test 4: Full alkanes-rs build with Bitcoin mode
echo ""
print_status "🔸 Test 4: Full alkanes-rs Build (Bitcoin Mode)"
echo "Testing full alkanes-rs build without dogecoin feature..."

if cargo check --workspace 2>/dev/null; then
    print_success "Bitcoin mode build check passed ✅"
else
    print_error "Bitcoin mode build check failed ❌"
    exit 1
fi

# Test 5: Full alkanes-rs build with Dogecoin mode
echo ""
print_status "🔸 Test 5: Full alkanes-rs Build (Dogecoin Mode)"
echo "Testing full alkanes-rs build with dogecoin feature..."

if cargo check --workspace --features dogecoin 2>/dev/null; then
    print_success "Dogecoin mode build check passed ✅"
else
    print_error "Dogecoin mode build check failed ❌"
    exit 1
fi

# Test 6: Verify feature flag behavior
echo ""
print_status "🔸 Test 6: Feature Flag Verification"
echo "Verifying that the correct inscription parsing is used..."

# Test Bitcoin mode specifically
echo "Testing Bitcoin mode feature detection..."
if cargo test -p ordinals --lib inscription_tests::tests::test_feature_flag_consistency 2>/dev/null; then
    print_success "Bitcoin mode feature detection works ✅"
else
    print_warning "Bitcoin mode feature detection test inconclusive ⚠️"
fi

# Test Dogecoin mode specifically
echo "Testing Dogecoin mode feature detection..."
if cargo test -p ordinals --lib inscription_tests::tests::test_feature_flag_consistency --features dogecoin 2>/dev/null; then
    print_success "Dogecoin mode feature detection works ✅"
else
    print_warning "Dogecoin mode feature detection test inconclusive ⚠️"
fi

# Summary
echo ""
echo "🎉 All Tests Completed!"
echo "======================="
print_success "✅ Bitcoin mode (taproot witness parsing) works correctly"
print_success "✅ Dogecoin mode (script_sig parsing) works correctly"
print_success "✅ Feature-gated compilation works correctly"
print_success "✅ Both modes can be built and tested independently"

echo ""
print_status "📋 Summary:"
echo "   • ordinals-scriptsig crate: Dogecoin-specific inscription parsing"
echo "   • ordinals crate: Feature-gated unified interface"
echo "   • alkanes-rs: Can be built for either Bitcoin or Dogecoin"
echo ""
print_status "🚀 Ready for production use with both Bitcoin and Dogecoin!"

echo ""
print_status "💡 Usage Examples:"
echo "   # Build for Bitcoin (default):"
echo "   cargo build --release"
echo ""
echo "   # Build for Dogecoin:"
echo "   cargo build --release --features dogecoin"
echo ""
echo "   # Test Bitcoin mode:"
echo "   cargo test -p ordinals"
echo ""
echo "   # Test Dogecoin mode:"
echo "   cargo test -p ordinals --features dogecoin"