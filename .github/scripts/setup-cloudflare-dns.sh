#!/bin/bash
# Cloudflare DNS Setup Script for Alkanes Build Package Registry
#
# This script automatically configures DNS records for the pkg.alkanes.build endpoints
# that point to Google Cloud Artifact Registry.
#
# Requirements:
# - CLOUDFLARE_API_TOKEN environment variable set
# - jq installed (for JSON parsing)
# - curl installed
#
# Usage:
#   export CLOUDFLARE_API_TOKEN="your-token-here"
#   ./setup-cloudflare-dns.sh

set -e

# Configuration
ZONE_NAME="alkanes.build"
CLOUDFLARE_API_BASE="https://api.cloudflare.com/client/v4"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check for required tools
if ! command -v jq &> /dev/null; then
    echo -e "${RED}Error: jq is not installed${NC}"
    echo "Install it with: sudo apt-get install jq (Ubuntu/Debian) or brew install jq (macOS)"
    exit 1
fi

if ! command -v curl &> /dev/null; then
    echo -e "${RED}Error: curl is not installed${NC}"
    exit 1
fi

# Check for API token
if [ -z "${CLOUDFLARE_API_TOKEN}" ]; then
    echo -e "${RED}Error: CLOUDFLARE_API_TOKEN environment variable is not set${NC}"
    echo "Get your token at: https://dash.cloudflare.com/profile/api-tokens"
    echo "Required permissions: Zone.DNS (Edit)"
    exit 1
fi

echo -e "${GREEN}=== Cloudflare DNS Setup for Alkanes Build ===${NC}"
echo ""

# Get Zone ID
echo "Fetching zone ID for ${ZONE_NAME}..."
ZONE_RESPONSE=$(curl -s -X GET "${CLOUDFLARE_API_BASE}/zones?name=${ZONE_NAME}" \
  -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
  -H "Content-Type: application/json")

ZONE_ID=$(echo "${ZONE_RESPONSE}" | jq -r '.result[0].id // empty')

if [ -z "${ZONE_ID}" ] || [ "${ZONE_ID}" == "null" ]; then
    echo -e "${RED}Error: Could not find zone ${ZONE_NAME}${NC}"
    echo "Response: ${ZONE_RESPONSE}"
    echo ""
    echo "Make sure:"
    echo "  1. The domain alkanes.build is in your Cloudflare account"
    echo "  2. Your API token has access to this zone"
    exit 1
fi

echo -e "${GREEN}✓ Found zone ID: ${ZONE_ID}${NC}"
echo ""

# Function to create or update CNAME record
update_cname() {
  local NAME=$1
  local TARGET=$2
  local FULL_NAME="${NAME}.${ZONE_NAME}"

  echo "Configuring CNAME: ${FULL_NAME} → ${TARGET}"

  # Check if record exists
  RECORD_RESPONSE=$(curl -s -X GET \
    "${CLOUDFLARE_API_BASE}/zones/${ZONE_ID}/dns_records?type=CNAME&name=${FULL_NAME}" \
    -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
    -H "Content-Type: application/json")

  RECORD_ID=$(echo "${RECORD_RESPONSE}" | jq -r '.result[0].id // empty')

  if [ -z "${RECORD_ID}" ] || [ "${RECORD_ID}" == "null" ]; then
    # Create new record
    echo "  → Creating new CNAME record..."
    CREATE_RESPONSE=$(curl -s -X POST "${CLOUDFLARE_API_BASE}/zones/${ZONE_ID}/dns_records" \
      -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{
        \"type\": \"CNAME\",
        \"name\": \"${NAME}\",
        \"content\": \"${TARGET}\",
        \"ttl\": 1,
        \"proxied\": false,
        \"comment\": \"Alkanes package registry (auto-managed)\"
      }")

    SUCCESS=$(echo "${CREATE_RESPONSE}" | jq -r '.success')
    if [ "${SUCCESS}" == "true" ]; then
      echo -e "${GREEN}  ✓ Created: ${FULL_NAME} → ${TARGET}${NC}"
    else
      echo -e "${RED}  ✗ Failed to create record${NC}"
      echo "  Response: ${CREATE_RESPONSE}"
      return 1
    fi
  else
    # Update existing record
    echo "  → Updating existing CNAME record (ID: ${RECORD_ID})..."
    UPDATE_RESPONSE=$(curl -s -X PUT \
      "${CLOUDFLARE_API_BASE}/zones/${ZONE_ID}/dns_records/${RECORD_ID}" \
      -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{
        \"type\": \"CNAME\",
        \"name\": \"${NAME}\",
        \"content\": \"${TARGET}\",
        \"ttl\": 1,
        \"proxied\": false,
        \"comment\": \"Alkanes package registry (auto-managed)\"
      }")

    SUCCESS=$(echo "${UPDATE_RESPONSE}" | jq -r '.success')
    if [ "${SUCCESS}" == "true" ]; then
      echo -e "${GREEN}  ✓ Updated: ${FULL_NAME} → ${TARGET}${NC}"
    else
      echo -e "${RED}  ✗ Failed to update record${NC}"
      echo "  Response: ${UPDATE_RESPONSE}"
      return 1
    fi
  fi

  echo ""
}

# Create CNAME records for npm and cargo registries
echo "Setting up DNS records for Google Cloud Artifact Registry..."
echo ""

update_cname "npm.pkg" "us-central1-npm.pkg.dev"
update_cname "cargo.pkg" "us-central1-cargo.pkg.dev"

# Verify DNS records
echo -e "${GREEN}=== DNS Configuration Complete ===${NC}"
echo ""
echo "Configured endpoints:"
echo "  • npm.pkg.alkanes.build → us-central1-npm.pkg.dev"
echo "  • cargo.pkg.alkanes.build → us-central1-cargo.pkg.dev"
echo ""
echo -e "${YELLOW}Note: DNS propagation may take a few minutes.${NC}"
echo ""
echo "Verify with:"
echo "  dig npm.pkg.alkanes.build CNAME +short"
echo "  dig cargo.pkg.alkanes.build CNAME +short"
echo ""
echo "View records at: https://dash.cloudflare.com/"
