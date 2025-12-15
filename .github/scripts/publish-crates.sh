#!/bin/bash
# Manual Tier-based Cargo Publishing Script
#
# This script publishes all 31 crates in the alkanes-rs workspace to
# Google Cloud Artifact Registry in dependency order.
#
# Usage:
#   export CARGO_REGISTRIES_ALKANES_TOKEN="$(gcloud auth print-access-token)"
#   bash .github/scripts/publish-crates.sh

set -e

# Configuration
REGISTRY="alkanes"
WORKSPACE_ROOT="$(git rev-parse --show-toplevel)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TOTAL_CRATES=0
PUBLISHED_CRATES=0
SKIPPED_CRATES=0
FAILED_CRATES=0

echo -e "${BLUE}=== Alkanes Rust Crates Publishing ===${NC}"
echo "Registry: ${REGISTRY}"
echo "Workspace: ${WORKSPACE_ROOT}"
echo ""

# Check for token
if [ -z "${CARGO_REGISTRIES_ALKANES_TOKEN}" ]; then
    echo -e "${YELLOW}Warning: CARGO_REGISTRIES_ALKANES_TOKEN not set${NC}"
    echo "Attempting to use token from ~/.cargo/credentials.toml"
fi

# Function to publish a single crate
publish_crate() {
  local CRATE_PATH=$1
  local CRATE_NAME=$(basename "$CRATE_PATH")

  TOTAL_CRATES=$((TOTAL_CRATES + 1))

  echo -e "${BLUE}[$TOTAL_CRATES] Publishing ${CRATE_NAME}...${NC}"

  # Check if crate directory exists
  if [ ! -d "$WORKSPACE_ROOT/$CRATE_PATH" ]; then
    echo -e "${RED}  ✗ Directory not found: $CRATE_PATH${NC}"
    FAILED_CRATES=$((FAILED_CRATES + 1))
    return 1
  fi

  cd "$WORKSPACE_ROOT/$CRATE_PATH"

  # Check if Cargo.toml exists
  if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}  ✗ Cargo.toml not found in $CRATE_PATH${NC}"
    FAILED_CRATES=$((FAILED_CRATES + 1))
    return 1
  fi

  # Attempt to publish
  if cargo publish --registry "${REGISTRY}" --allow-dirty 2>&1 | tee /tmp/cargo-publish-${CRATE_NAME}.log; then
    if grep -q "Uploading" /tmp/cargo-publish-${CRATE_NAME}.log; then
      echo -e "${GREEN}  ✅ ${CRATE_NAME} published successfully${NC}"
      PUBLISHED_CRATES=$((PUBLISHED_CRATES + 1))
    elif grep -q "already uploaded\|is already uploaded" /tmp/cargo-publish-${CRATE_NAME}.log; then
      echo -e "${YELLOW}  ⏭️  ${CRATE_NAME} already published, skipping${NC}"
      SKIPPED_CRATES=$((SKIPPED_CRATES + 1))
    else
      echo -e "${GREEN}  ✅ ${CRATE_NAME} processed${NC}"
      PUBLISHED_CRATES=$((PUBLISHED_CRATES + 1))
    fi
  else
    if grep -q "already uploaded\|is already uploaded" /tmp/cargo-publish-${CRATE_NAME}.log; then
      echo -e "${YELLOW}  ⏭️  ${CRATE_NAME} already published, skipping${NC}"
      SKIPPED_CRATES=$((SKIPPED_CRATES + 1))
    else
      echo -e "${RED}  ✗ ${CRATE_NAME} failed to publish${NC}"
      cat /tmp/cargo-publish-${CRATE_NAME}.log
      FAILED_CRATES=$((FAILED_CRATES + 1))
    fi
  fi

  # Clean up log
  rm -f /tmp/cargo-publish-${CRATE_NAME}.log

  cd "$WORKSPACE_ROOT"

  # Small delay to avoid rate limiting
  sleep 1
}

# Tier 1: Base crates (no internal dependencies)
echo -e "${BLUE}=== Publishing Tier 1: Base crates ===${NC}"
publish_crate "crates/metashrew-core"
publish_crate "crates/metashrew-support"
publish_crate "crates/ordinals"
publish_crate "crates/alkanes-pretty-print-macro"
publish_crate "crates/alkanes-macros"
publish_crate "crates/alkanes-build"
echo ""

# Tier 2: Core runtime (depends on Tier 1)
echo -e "${BLUE}=== Publishing Tier 2: Core runtime ===${NC}"
publish_crate "crates/metashrew-runtime"
publish_crate "crates/metashrew-minimal"
publish_crate "crates/metashrew-sync"
publish_crate "crates/protorune-support"
publish_crate "crates/rockshrew-diff"
echo ""

# Tier 3: Extended runtime (depends on Tier 1-2)
echo -e "${BLUE}=== Publishing Tier 3: Extended runtime ===${NC}"
publish_crate "crates/protorune"
publish_crate "crates/alkanes-support"
publish_crate "crates/rockshrew-runtime"
publish_crate "crates/rockshrew-mono"
publish_crate "crates/memshrew-runtime"
publish_crate "crates/memshrew-p2p"
echo ""

# Tier 4: Alkanes core (depends on Tier 1-3)
echo -e "${BLUE}=== Publishing Tier 4: Alkanes core ===${NC}"
publish_crate "crates/alkanes-runtime"
publish_crate "crates/alkanes-asc"
publish_crate "crates/alkanes-cli-common"
publish_crate "crates/alkanes"
echo ""

# Tier 5: Bindings and tools (depends on Tier 1-4)
echo -e "${BLUE}=== Publishing Tier 5: Bindings and tools ===${NC}"
publish_crate "crates/alkanes-web-sys"
publish_crate "crates/alkanes-web-leptos"
publish_crate "crates/alkanes-ffi"
publish_crate "crates/alkanes-jni"
publish_crate "crates/alkanes-cli-sys"
publish_crate "crates/alkanes-cli"
publish_crate "crates/alkanes-jsonrpc"
publish_crate "crates/alkanes-data-api"
publish_crate "crates/alkanes-contract-indexer"
publish_crate "crates/alkanes-trace-transform"
echo ""

# Summary
echo -e "${BLUE}=== Publishing Summary ===${NC}"
echo "Total crates: ${TOTAL_CRATES}"
echo -e "${GREEN}Published: ${PUBLISHED_CRATES}${NC}"
echo -e "${YELLOW}Skipped: ${SKIPPED_CRATES}${NC}"
echo -e "${RED}Failed: ${FAILED_CRATES}${NC}"
echo ""

if [ $FAILED_CRATES -eq 0 ]; then
  echo -e "${GREEN}✅ All crates published successfully!${NC}"
  exit 0
elif [ $FAILED_CRATES -lt 5 ]; then
  echo -e "${YELLOW}⚠️  Some crates failed to publish (${FAILED_CRATES} failures)${NC}"
  exit 0 # Don't fail the workflow for a few failures
else
  echo -e "${RED}✗ Too many crates failed to publish (${FAILED_CRATES} failures)${NC}"
  exit 1
fi
