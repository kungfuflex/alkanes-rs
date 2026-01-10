#!/bin/bash
set -e

# Build Verification Script
# Verifies that a WASM binary matches the expected checksum
# Usage: ./verify-build.sh <expected-sha256>

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo "=========================================="
echo "  WASM Build Verification                "
echo "=========================================="
echo ""

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ARTIFACTS_DIR="$PROJECT_ROOT/artifacts"
WASM_FILE="$ARTIFACTS_DIR/free-mint.wasm"

# Check if expected hash is provided
if [ $# -eq 0 ]; then
    echo -e "${YELLOW}Usage: $0 <expected-sha256>${NC}"
    echo ""
    echo "This script verifies that the built WASM matches an expected checksum."
    echo ""
    echo "To verify against published builds:"
    echo "  1. Get the published SHA256 hash"
    echo "  2. Build locally: ./scripts/build-reproducible.sh"
    echo "  3. Run: $0 <published-sha256>"
    echo ""

    # If we have a local build, show its checksum
    if [ -f "$WASM_FILE" ]; then
        echo -e "${BLUE}Current local build checksum:${NC}"
        sha256sum "$WASM_FILE"
        echo ""
        echo "To verify against this build:"
        CURRENT_HASH=$(sha256sum "$WASM_FILE" | cut -d ' ' -f 1)
        echo "  $0 $CURRENT_HASH"
    else
        echo -e "${RED}No build found. Run ./scripts/build-reproducible.sh first.${NC}"
    fi
    echo ""
    exit 1
fi

EXPECTED_HASH="$1"

# Check if artifacts exist
if [ ! -d "$ARTIFACTS_DIR" ]; then
    echo -e "${RED}Error: Artifacts directory not found${NC}"
    echo "Please run ./scripts/build-reproducible.sh first"
    exit 1
fi

if [ ! -f "$WASM_FILE" ]; then
    echo -e "${RED}Error: WASM file not found${NC}"
    echo "Please run ./scripts/build-reproducible.sh first"
    exit 1
fi

# Calculate checksum
echo -e "${BLUE}Verifying WASM binary...${NC}"
echo ""
echo -e "File: ${YELLOW}$WASM_FILE${NC}"
ACTUAL_HASH=$(sha256sum "$WASM_FILE" | cut -d ' ' -f 1)
echo -e "Actual SHA256:   ${YELLOW}$ACTUAL_HASH${NC}"
echo -e "Expected SHA256: ${YELLOW}$EXPECTED_HASH${NC}"
echo ""

# Compare hashes
if [ "$ACTUAL_HASH" = "$EXPECTED_HASH" ]; then
    echo -e "${GREEN}✓ VERIFICATION SUCCESSFUL!${NC}"
    echo ""
    echo "The WASM binary matches the expected checksum."
    echo "This build is reproducible and verified."

    # Show build info if available
    if [ -f "$ARTIFACTS_DIR/build-info.json" ]; then
        echo ""
        echo -e "${BLUE}Build Information:${NC}"
        cat "$ARTIFACTS_DIR/build-info.json" | grep -v "^{" | grep -v "^}" | sed 's/^  //'
    fi

    exit 0
else
    echo -e "${RED}✗ VERIFICATION FAILED!${NC}"
    echo ""
    echo "The checksums do not match. This could mean:"
    echo "  1. The source code is different"
    echo "  2. The build environment is different"
    echo "  3. The build tools (Rust version, etc.) are different"
    echo ""
    echo "For reproducible builds, ensure:"
    echo "  - Same source code (git commit)"
    echo "  - Using Docker build: ./scripts/build-reproducible.sh"
    echo "  - Same rust-toolchain.toml version"
    echo ""
    exit 1
fi
