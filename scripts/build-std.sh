#!/bin/bash
set -e

# Script to build all alkanes in ./alkanes and generate WASM files

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ALKANES_DIR="$ROOT_DIR/alkanes"
OUTPUT_DIR="$ROOT_DIR/crates/alkanes/src/tests/std/wasm"
TARGET_DIR="$ROOT_DIR/target/build-std"

echo "Building all alkanes to WASM..."
echo "Root directory: $ROOT_DIR"
echo "Alkanes directory: $ALKANES_DIR"
echo "Output directory: $OUTPUT_DIR"

# Create output directory if it doesn't exist
mkdir -p "$OUTPUT_DIR"

# List of alkanes that need network-specific builds
NETWORK_SPECIFIC_ALKANES=(
    "alkanes-std-genesis-alkane"
    "alkanes-std-genesis-alkane-upgraded"
    "alkanes-std-genesis-alkane-upgraded-eoa"
    "alkanes-std-merkle-distributor"
)

# Network configurations: network_name:feature_name
NETWORKS=(
    "bellscoin:bellscoin"
    "luckycoin:luckycoin"
    "mainnet:mainnet"
    "fractal:fractal"
    "regtest:regtest"
    "testnet:regtest"
)

# Function to build a package
build_package() {
    local package_name=$1
    local features=$2
    local output_name=$3
    
    echo "Building $package_name with features: $features"
    
    if [ -z "$features" ]; then
        cargo build --release \
            --target wasm32-unknown-unknown \
            --target-dir "$TARGET_DIR" \
            -p "$package_name"
    else
        cargo build --release \
            --target wasm32-unknown-unknown \
            --target-dir "$TARGET_DIR" \
            -p "$package_name" \
            --features "$features"
    fi
    
    # Copy the built WASM to output directory
    local wasm_name="${package_name//-/_}"
    cp "$TARGET_DIR/wasm32-unknown-unknown/release/${wasm_name}.wasm" \
       "$OUTPUT_DIR/${output_name}.wasm"
    
    echo "Created $OUTPUT_DIR/${output_name}.wasm"
}

# Check if network-specific alkane
is_network_specific() {
    local alkane=$1
    for ns_alkane in "${NETWORK_SPECIFIC_ALKANES[@]}"; do
        if [ "$alkane" = "$ns_alkane" ]; then
            return 0
        fi
    done
    return 1
}

# Find all alkanes-std-* directories
cd "$ALKANES_DIR"
for alkane_dir in alkanes-std-*; do
    if [ ! -d "$alkane_dir" ]; then
        continue
    fi
    
    alkane_name="$alkane_dir"
    
    if is_network_specific "$alkane_name"; then
        # Build for each network
        for network_config in "${NETWORKS[@]}"; do
            network_name="${network_config%%:*}"
            feature_name="${network_config##*:}"
            output_name="${alkane_name//-/_}_${network_name}"
            
            build_package "$alkane_name" "$feature_name" "$output_name"
        done
        
        # Also build a default version with regtest features for tests
        output_name="${alkane_name//-/_}"
        build_package "$alkane_name" "regtest" "$output_name"
    else
        # Build without features
        output_name="${alkane_name//-/_}"
        build_package "$alkane_name" "" "$output_name"
    fi
done

echo ""
echo "Generating mod.rs file..."

# Generate the mod.rs file
MOD_FILE="$ROOT_DIR/crates/alkanes/src/tests/std/mod.rs"
> "$MOD_FILE"

# Find all WASM files and generate module declarations
cd "$OUTPUT_DIR"
for wasm_file in *.wasm; do
    if [ ! -f "$wasm_file" ]; then
        continue
    fi
    
    module_name="${wasm_file%.wasm}"
    
    cat >> "$MOD_FILE" << EOF
#[allow(dead_code)]
pub mod ${module_name}_build {
    pub fn get_bytes() -> Vec<u8> {
        include_bytes!("./wasm/${wasm_file}").to_vec()
    }
}

EOF
done

echo "Generated $MOD_FILE"
echo ""
echo "Build complete! All WASM files are in $OUTPUT_DIR"
echo "The mod.rs file has been generated with include_bytes! macros."
