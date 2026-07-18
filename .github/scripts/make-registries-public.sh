#!/bin/bash
# Make Artifact Registry Repositories Public
#
# This script makes the npm and cargo repositories publicly readable
# while keeping publishing restricted to authorized service accounts.
#
# After running this script, users can install packages without authentication:
#   npm install @alkanes/ts-sdk
#   cargo build (with public registry configured)
#
# Publishing still requires Workload Identity authentication.

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Making Artifact Registry Repositories Public ===${NC}"
echo ""

# Check if authenticated
if ! gcloud auth list --filter=status:ACTIVE --format="value(account)" | grep -q .; then
    echo -e "${YELLOW}Warning: Not authenticated to GCP${NC}"
    echo "Please run: gcloud auth login"
    exit 1
fi

# Check if project is set
PROJECT=$(gcloud config get-value project 2>/dev/null)
if [ "$PROJECT" != "pkg-alkanes-build" ]; then
    echo -e "${YELLOW}Warning: Current project is not pkg-alkanes-build${NC}"
    echo "Current project: $PROJECT"
    echo "Setting project to pkg-alkanes-build..."
    gcloud config set project pkg-alkanes-build
fi

echo "Project: $(gcloud config get-value project)"
echo ""

# Function to make a repository public
make_public() {
    local REPO_NAME=$1
    local REPO_TYPE=$2

    echo -e "${BLUE}Making ${REPO_NAME} repository public...${NC}"

    # Add allUsers as Artifact Registry Reader
    if gcloud artifacts repositories add-iam-policy-binding "$REPO_NAME" \
        --location=us-central1 \
        --member="allUsers" \
        --role="roles/artifactregistry.reader" 2>&1 | tee /tmp/gcloud-output.log; then

        echo -e "${GREEN}✓ ${REPO_NAME} is now publicly readable${NC}"
    else
        if grep -q "already exists" /tmp/gcloud-output.log; then
            echo -e "${GREEN}✓ ${REPO_NAME} was already public${NC}"
        else
            echo -e "${YELLOW}⚠️  Failed to make ${REPO_NAME} public${NC}"
            cat /tmp/gcloud-output.log
            return 1
        fi
    fi

    # Verify policy
    echo "  Verifying IAM policy..."
    if gcloud artifacts repositories get-iam-policy "$REPO_NAME" \
        --location=us-central1 \
        --format=json | grep -q "allUsers"; then
        echo -e "${GREEN}  ✓ Verified: allUsers has reader access${NC}"
    else
        echo -e "${YELLOW}  ⚠️  Warning: Could not verify allUsers access${NC}"
    fi

    echo ""
}

# Make npm repository public
make_public "npm-packages" "npm"

# Make cargo repository public
make_public "cargo-packages" "cargo"

# Clean up
rm -f /tmp/gcloud-output.log

# Test public access
echo -e "${BLUE}=== Testing Public Access ===${NC}"
echo ""

echo "Testing npm repository..."
NPM_URL="https://us-central1-npm.pkg.dev/pkg-alkanes-build/npm-packages/"
if curl -s -o /dev/null -w "%{http_code}" "$NPM_URL" | grep -q "200\|403"; then
    echo -e "${GREEN}✓ npm repository is accessible${NC}"
else
    echo -e "${YELLOW}⚠️  npm repository returned unexpected status${NC}"
fi

echo ""
echo "Testing cargo repository..."
CARGO_URL="https://us-central1-cargo.pkg.dev/pkg-alkanes-build/cargo-packages/"
if curl -s -o /dev/null -w "%{http_code}" "$CARGO_URL" | grep -q "200\|403"; then
    echo -e "${GREEN}✓ cargo repository is accessible${NC}"
else
    echo -e "${YELLOW}⚠️  cargo repository returned unexpected status${NC}"
fi

echo ""
echo -e "${GREEN}=== Setup Complete ===${NC}"
echo ""
echo "Repositories are now publicly readable!"
echo ""
echo "Users can now install packages without authentication:"
echo ""
echo "  ${BLUE}npm:${NC}"
echo "    npm config set @alkanes:registry https://us-central1-npm.pkg.dev/pkg-alkanes-build/npm-packages/"
echo "    npm install @alkanes/ts-sdk"
echo ""
echo "  ${BLUE}cargo:${NC}"
echo "    # In ~/.cargo/config.toml"
echo "    [registries.alkanes]"
echo "    index = \"sparse+https://us-central1-cargo.pkg.dev/pkg-alkanes-build/cargo-packages/\""
echo ""
echo "    # In Cargo.toml"
echo "    [dependencies]"
echo "    alkanes = { version = \"10.0.0\", registry = \"alkanes\" }"
echo ""
echo "Note: Publishing still requires authentication (Workload Identity for GitHub Actions)"
echo ""
echo "To verify IAM policies:"
echo "  gcloud artifacts repositories get-iam-policy npm-packages --location=us-central1"
echo "  gcloud artifacts repositories get-iam-policy cargo-packages --location=us-central1"
echo ""
echo "To revert (make private again):"
echo "  gcloud artifacts repositories remove-iam-policy-binding npm-packages --location=us-central1 --member='allUsers' --role='roles/artifactregistry.reader'"
echo "  gcloud artifacts repositories remove-iam-policy-binding cargo-packages --location=us-central1 --member='allUsers' --role='roles/artifactregistry.reader'"
