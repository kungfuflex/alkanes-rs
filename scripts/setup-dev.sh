#!/bin/bash
set -e

# Development Setup Script
# Prepares your local environment for deterministic builds

echo "=========================================="
echo "  Deterministic Build Setup              "
echo "=========================================="
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check Docker
echo -e "${BLUE}Checking Docker...${NC}"
if command -v docker &> /dev/null; then
    echo -e "${GREEN}✓ Docker found: $(docker --version)${NC}"
else
    echo -e "${RED}✗ Docker not found${NC}"
    echo "Please install Docker: https://docs.docker.com/get-docker/"
    exit 1
fi
echo ""

# Check Rust (optional for local dev)
echo -e "${BLUE}Checking Rust...${NC}"
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    echo -e "${GREEN}✓ Rust found: $RUST_VERSION${NC}"

    # Check if correct version
    EXPECTED_VERSION="1.90.0"
    if [[ $RUST_VERSION == *"$EXPECTED_VERSION"* ]]; then
        echo -e "${GREEN}✓ Correct Rust version (pinned by rust-toolchain.toml)${NC}"
    else
        echo -e "${YELLOW}⚠ Different Rust version detected${NC}"
        echo "  rust-toolchain.toml will auto-install 1.90.0 when building"
    fi

    # Check wasm target
    if rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
        echo -e "${GREEN}✓ wasm32-unknown-unknown target installed${NC}"
    else
        echo -e "${YELLOW}Installing wasm32-unknown-unknown target...${NC}"
        rustup target add wasm32-unknown-unknown
        echo -e "${GREEN}✓ Target installed${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Rust not found${NC}"
    echo "For local development builds, install Rust: https://rustup.rs/"
    echo "Docker builds will still work without local Rust."
fi
echo ""

# Build Docker image
echo -e "${BLUE}Building Docker image for reproducible builds...${NC}"
docker build -f Dockerfile.builder -t alkane-builder:latest .
echo -e "${GREEN}✓ Docker image built${NC}"
echo ""

# Test build
echo -e "${YELLOW}Running test build to verify setup...${NC}"
./scripts/build-reproducible.sh

if [ -f "artifacts/checksums.txt" ]; then
    echo ""
    echo -e "${GREEN}=========================================="
    echo -e "  Setup Complete! ✓                       "
    echo -e "==========================================${NC}"
    echo ""
    echo "Your checksum for this build:"
    cat artifacts/checksums.txt
    echo ""
    echo -e "${BLUE}Next steps:${NC}"
    echo "  1. For development: cargo build --release"
    echo "  2. For verification: ./scripts/build-reproducible.sh"
    echo "  3. Read VERIFICATION_QUICK_START.md for more info"
    echo ""
else
    echo -e "${RED}✗ Build failed${NC}"
    echo "Check the output above for errors"
    exit 1
fi
