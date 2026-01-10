# Deterministic WASM Builder for Alkane Contracts
# Based on CosmWasm's rust-optimizer approach
# This container provides a reproducible build environment

FROM rust:1.90.0-alpine

# Install dependencies for WASM compilation
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    git \
    cmake \
    make \
    g++ \
    clang \
    lld

# Install wasm32-unknown-unknown target
RUN rustup target add wasm32-unknown-unknown

# Install wasm-opt for size optimization and further determinism
# wasm-opt normalizes the output and makes builds more reproducible
RUN apk add --no-cache binaryen

# Set cargo to use sparse protocol for faster, more reliable downloads
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Set workspace directory
WORKDIR /code

# Environment variables for reproducible builds
ENV RUSTFLAGS="-C link-arg=-s -C codegen-units=1 -C opt-level=3 -C lto=fat -C panic=abort -C embed-bitcode=no"

# Default command runs the build script
CMD ["/bin/sh", "/code/scripts/docker-build.sh"]

# Build instructions:
# 1. Build the image: docker build -f Dockerfile.builder -t alkane-builder:latest .
# 2. Run the builder: docker run --rm -v "$(pwd)":/code alkane-builder:latest
# 3. Find output in ./artifacts/ directory with checksums
