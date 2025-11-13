# Multi-stage build for alkanes Rust binaries
FROM rust:1.83-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY src ./src

# Build argument for which package to build
ARG PACKAGE=alkanes-jsonrpc

# Build the specified package in release mode
RUN cargo build --release -p ${PACKAGE}

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Build argument (must be redeclared in each stage)
ARG PACKAGE=alkanes-jsonrpc

# Copy the built binary from builder stage
COPY --from=builder /app/target/release/${PACKAGE} /usr/local/bin/app

# Set the binary as the entrypoint
ENTRYPOINT ["/usr/local/bin/app"]
