#!/bin/bash
set -e

# Reproducible Build Script
# Builds the WASM contract using Docker for maximum reproducibility

echo "=========================================="
echo "  Building with Docker (Reproducible)    "
echo "=========================================="
echo ""

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo "Error: Docker is not installed or not in PATH"
    echo "Please install Docker to use reproducible builds"
    exit 1
fi

# Configuration
IMAGE_NAME="alkane-builder"
IMAGE_TAG="latest"
FULL_IMAGE="${IMAGE_NAME}:${IMAGE_TAG}"

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

# Check if we need to build the Docker image
if ! docker image inspect "$FULL_IMAGE" >/dev/null 2>&1; then
    echo "Docker image not found. Building $FULL_IMAGE..."
    echo ""
    docker build -f Dockerfile.builder -t "$FULL_IMAGE" .
    echo ""
    echo "✓ Docker image built successfully"
    echo ""
else
    echo "Using existing Docker image: $FULL_IMAGE"
    echo "(To rebuild the image, run: docker build -f Dockerfile.builder -t $FULL_IMAGE .)"
    echo ""
fi

# Run the build
echo "Starting reproducible build..."
echo ""

docker run --rm \
    -v "$(pwd)":/code \
    -u "$(id -u):$(id -g)" \
    "$FULL_IMAGE"

echo ""
echo "=========================================="
echo "  Reproducible Build Complete!           "
echo "=========================================="
echo ""
echo "Next steps:"
echo "  1. Share the SHA256 hash from artifacts/checksums.txt"
echo "  2. Others can verify by building with the same script"
echo "  3. Compare checksums to verify identical builds"
echo ""
