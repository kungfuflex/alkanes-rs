#!/bin/bash
# Configure Cloudflare DNS for pkg.alkanes.build
#
# This script sets up DNS to point to the Cloud Run reverse proxy

set -e

CLOUDFLARE_API_TOKEN="${CLOUDFLARE_API_TOKEN}"
ZONE_NAME="alkanes.build"
# Cloud Run custom domain endpoint (for SSL certificate)
CLOUD_RUN_URL="ghs.googlehosted.com"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Cloudflare DNS Setup for pkg.alkanes.build ===${NC}"

# Check for API token
if [ -z "${CLOUDFLARE_API_TOKEN}" ]; then
    echo -e "${YELLOW}Error: CLOUDFLARE_API_TOKEN not set${NC}"
    echo "Get token from GitHub secrets or create at: https://dash.cloudflare.com/profile/api-tokens"
    exit 1
fi

# Get Zone ID
echo "Fetching zone ID..."
ZONE_ID=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones?name=${ZONE_NAME}" \
  -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
  -H "Content-Type: application/json" | jq -r '.result[0].id // empty')

if [ -z "${ZONE_ID}" ]; then
    echo -e "${YELLOW}Error: Could not find zone ${ZONE_NAME}${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Found zone ID: ${ZONE_ID}${NC}"

# Function to create or update CNAME record
update_cname() {
  local NAME=$1
  local TARGET=$2
  local PROXIED=$3
  local FULL_NAME="${NAME}.${ZONE_NAME}"

  echo "Configuring CNAME: ${FULL_NAME} → ${TARGET} (Proxied: ${PROXIED})"

  # Check if record exists
  RECORD_RESPONSE=$(curl -s -X GET \
    "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?type=CNAME&name=${FULL_NAME}" \
    -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
    -H "Content-Type: application/json")

  RECORD_ID=$(echo "${RECORD_RESPONSE}" | jq -r '.result[0].id // empty')

  if [ -z "${RECORD_ID}" ]; then
    # Create new record
    echo "  → Creating new CNAME record..."
    CREATE_RESPONSE=$(curl -s -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records" \
      -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{
        \"type\": \"CNAME\",
        \"name\": \"${NAME}\",
        \"content\": \"${TARGET}\",
        \"ttl\": 1,
        \"proxied\": ${PROXIED},
        \"comment\": \"Alkanes package registry (auto-managed)\"
      }")

    SUCCESS=$(echo "${CREATE_RESPONSE}" | jq -r '.success')
    if [ "${SUCCESS}" == "true" ]; then
      echo -e "${GREEN}  ✓ Created: ${FULL_NAME} → ${TARGET}${NC}"
    else
      echo -e "${YELLOW}  ✗ Failed to create record${NC}"
      echo "  Response: ${CREATE_RESPONSE}"
    fi
  else
    # Update existing record
    echo "  → Updating existing CNAME record..."
    UPDATE_RESPONSE=$(curl -s -X PUT \
      "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records/${RECORD_ID}" \
      -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{
        \"type\": \"CNAME\",
        \"name\": \"${NAME}\",
        \"content\": \"${TARGET}\",
        \"ttl\": 1,
        \"proxied\": ${PROXIED},
        \"comment\": \"Alkanes package registry (auto-managed)\"
      }")

    SUCCESS=$(echo "${UPDATE_RESPONSE}" | jq -r '.success')
    if [ "${SUCCESS}" == "true" ]; then
      echo -e "${GREEN}  ✓ Updated: ${FULL_NAME} → ${TARGET}${NC}"
    else
      echo -e "${YELLOW}  ✗ Failed to update record${NC}"
      echo "  Response: ${UPDATE_RESPONSE}"
    fi
  fi
}

# Create CNAME record for pkg.alkanes.build pointing to Cloud Run
# Proxied=false to allow Cloud Run to handle requests directly (Cloudflare proxy breaks it)
update_cname "pkg" "${CLOUD_RUN_URL}" "false"

echo ""
echo -e "${GREEN}=== DNS Configuration Complete ===${NC}"
echo ""
echo "Configured endpoint:"
echo "  • pkg.alkanes.build → ${CLOUD_RUN_URL}"
echo ""
echo -e "${YELLOW}Note: DNS propagation may take a few minutes.${NC}"
echo ""
echo "Verify with:"
echo "  dig pkg.alkanes.build CNAME +short"
echo "  curl https://pkg.alkanes.build/health"
echo ""
echo "Users can now install with:"
echo "  npm config set @alkanes:registry https://pkg.alkanes.build/"
echo "  npm install @alkanes/ts-sdk"
