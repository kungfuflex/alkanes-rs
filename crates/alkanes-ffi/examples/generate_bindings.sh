#!/bin/bash
# Script to generate language bindings for alkanes-ffi

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
CRATE_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(dirname "$(dirname "$CRATE_DIR")")"

echo "Building alkanes-ffi..."
cd "$PROJECT_ROOT"
cargo build --package alkanes-ffi --release

echo ""
echo "Generating language bindings..."

# Detect platform
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    LIB_EXT="so"
    LIB_PATH="$PROJECT_ROOT/target/release/libalkanes_ffi.so"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    LIB_EXT="dylib"
    LIB_PATH="$PROJECT_ROOT/target/release/libalkanes_ffi.dylib"
else
    echo "Unsupported platform: $OSTYPE"
    exit 1
fi

if [ ! -f "$LIB_PATH" ]; then
    echo "Error: Library not found at $LIB_PATH"
    exit 1
fi

# Check if uniffi-bindgen is installed
if ! command -v uniffi-bindgen &> /dev/null; then
    echo "uniffi-bindgen not found. Installing..."
    cargo install uniffi-bindgen --version 0.28
fi

# Generate Kotlin bindings
echo ""
echo "Generating Kotlin bindings..."
uniffi-bindgen generate \
    --library "$LIB_PATH" \
    --language kotlin \
    --out-dir "$CRATE_DIR/out/kotlin"

echo "✓ Kotlin bindings generated at: $CRATE_DIR/out/kotlin/"

# Generate Swift bindings
echo ""
echo "Generating Swift bindings..."
uniffi-bindgen generate \
    --library "$LIB_PATH" \
    --language swift \
    --out-dir "$CRATE_DIR/out/swift"

echo "✓ Swift bindings generated at: $CRATE_DIR/out/swift/"

# Generate Python bindings
echo ""
echo "Generating Python bindings..."
uniffi-bindgen generate \
    --library "$LIB_PATH" \
    --language python \
    --out-dir "$CRATE_DIR/out/python"

echo "✓ Python bindings generated at: $CRATE_DIR/out/python/"

echo ""
echo "========================================" 
echo "✓ All bindings generated successfully!"
echo "========================================"
echo ""
echo "Next steps:"
echo "  - Kotlin: Copy files from out/kotlin/ to your Android/JVM project"
echo "  - Swift: Copy files from out/swift/ to your iOS/macOS project"
echo "  - Python: Use the files in out/python/ directly"
echo ""
echo "See the README.md for integration instructions."
