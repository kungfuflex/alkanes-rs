#!/bin/sh
set -e

# Deterministic WASM Build Script
# Runs inside the Docker container to produce reproducible builds
# Based on CosmWasm's optimizer approach

echo "=========================================="
echo "  Deterministic WASM Build for Alkanes   "
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="free-mint"
ARTIFACTS_DIR="/code/artifacts"
TARGET_DIR="/code/target"

echo -e "${BLUE}Project:${NC} $PROJECT_NAME"
echo -e "${BLUE}Rust version:${NC} $(rustc --version)"
echo -e "${BLUE}Cargo version:${NC} $(cargo --version)"
echo ""

# Clean previous artifacts
echo -e "${YELLOW}Cleaning previous artifacts...${NC}"
rm -rf "$ARTIFACTS_DIR"
mkdir -p "$ARTIFACTS_DIR"

# Clean cargo cache to ensure fresh build
echo -e "${YELLOW}Cleaning cargo cache...${NC}"
cargo clean

# Build the contract
echo ""
echo -e "${BLUE}Building WASM contract...${NC}"
cargo build --release --target wasm32-unknown-unknown --locked

# Get the output WASM file
WASM_FILE="${TARGET_DIR}/wasm32-unknown-unknown/release/${PROJECT_NAME}.wasm"

if [ ! -f "$WASM_FILE" ]; then
    echo -e "${RED}Error: WASM file not found at $WASM_FILE${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Build successful${NC}"
echo ""

# Get original size
ORIGINAL_SIZE=$(wc -c < "$WASM_FILE")
echo -e "${BLUE}Original size:${NC} $ORIGINAL_SIZE bytes"

# Optimize with wasm-opt
echo -e "${YELLOW}Running wasm-opt for size optimization...${NC}"
wasm-opt -Os "$WASM_FILE" -o "${ARTIFACTS_DIR}/${PROJECT_NAME}.wasm"

# Get optimized size
OPTIMIZED_SIZE=$(wc -c < "${ARTIFACTS_DIR}/${PROJECT_NAME}.wasm")
echo -e "${BLUE}Optimized size:${NC} $OPTIMIZED_SIZE bytes"
REDUCTION=$((100 - (OPTIMIZED_SIZE * 100 / ORIGINAL_SIZE)))
echo -e "${GREEN}Size reduction:${NC} ${REDUCTION}%"
echo ""

# Generate checksums
echo -e "${YELLOW}Generating checksums...${NC}"
cd "$ARTIFACTS_DIR"
sha256sum "${PROJECT_NAME}.wasm" > checksums.txt
SHA256=$(cat checksums.txt | cut -d ' ' -f 1)

echo -e "${GREEN}✓ Checksums generated${NC}"
echo ""
echo -e "${BLUE}SHA256:${NC} $SHA256"
echo ""

# Create build metadata
cat > build-info.json << EOF
{
  "project": "$PROJECT_NAME",
  "rust_version": "$(rustc --version)",
  "build_timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "original_size": $ORIGINAL_SIZE,
  "optimized_size": $OPTIMIZED_SIZE,
  "sha256": "$SHA256",
  "rustflags": "$RUSTFLAGS"
}
EOF

echo -e "${GREEN}=========================================="
echo -e "  Build Complete!                         "
echo -e "==========================================${NC}"
echo ""
echo -e "Artifacts saved to: ${BLUE}./artifacts/${NC}"
echo -e "  - ${PROJECT_NAME}.wasm (optimized binary)"
echo -e "  - checksums.txt (SHA256 hash)"
echo -e "  - build-info.json (build metadata)"
echo ""
echo -e "${YELLOW}To verify this build, run:${NC}"
echo -e "  ./scripts/verify-build.sh $SHA256"
echo ""
