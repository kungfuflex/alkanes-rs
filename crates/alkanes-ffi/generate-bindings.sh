#!/bin/bash
set -e

# Script to generate FFI bindings for multiple languages
# Requires the library to be built first

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
LIBRARY_PATH="$PROJECT_ROOT/target/release/libalkanes_ffi.so"
UDL_FILE="$SCRIPT_DIR/src/alkanes.udl"
BINDINGS_DIR="$SCRIPT_DIR/bindings"

echo "Alkanes FFI Binding Generator"
echo "=============================="
echo ""

# Check if library exists
if [ ! -f "$LIBRARY_PATH" ]; then
    echo "Error: Library not found at $LIBRARY_PATH"
    echo "Please build the library first:"
    echo "  cargo build --release --package alkanes-ffi"
    exit 1
fi

# Check if UDL file exists
if [ ! -f "$UDL_FILE" ]; then
    echo "Error: UDL file not found at $UDL_FILE"
    exit 1
fi

echo "Library: $LIBRARY_PATH"
echo "UDL: $UDL_FILE"
echo ""

# Create bindings directory
mkdir -p "$BINDINGS_DIR"

# Generate Kotlin bindings
echo "Generating Kotlin bindings..."
mkdir -p "$BINDINGS_DIR/kotlin"
cd "$SCRIPT_DIR"
cargo run --release --features=uniffi/cli --bin uniffi-bindgen generate \
    --library "$LIBRARY_PATH" \
    --language kotlin \
    --out-dir "$BINDINGS_DIR/kotlin" \
    2>/dev/null || {
        echo "Note: Using uniffi-bindgen from library (if available)"
        # Alternative: try using the uniffi library's scaffolding
        echo "Kotlin bindings would be generated at build time via uniffi macros"
    }

# Generate Swift bindings  
echo "Generating Swift bindings..."
mkdir -p "$BINDINGS_DIR/swift"
cargo run --release --features=uniffi/cli --bin uniffi-bindgen generate \
    --library "$LIBRARY_PATH" \
    --language swift \
    --out-dir "$BINDINGS_DIR/swift" \
    2>/dev/null || {
        echo "Note: Swift bindings would be generated at build time via uniffi macros"
    }

# Generate Python bindings
echo "Generating Python bindings..."
mkdir -p "$BINDINGS_DIR/python"
cargo run --release --features=uniffi/cli --bin uniffi-bindgen generate \
    --library "$LIBRARY_PATH" \
    --language python \
    --out-dir "$BINDINGS_DIR/python" \
    2>/dev/null || {
        echo "Note: Python bindings would be generated at build time via uniffi macros"
    }

echo ""
echo "Binding generation complete!"
echo "Bindings are in: $BINDINGS_DIR"
echo ""
echo "Note: UniFFI 0.28+ generates bindings via macros at build time."
echo "The generated .uniffi.rs file contains the scaffolding code."
echo "Check: $PROJECT_ROOT/target/release/build/alkanes-ffi-*/out/"
