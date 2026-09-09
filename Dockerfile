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
COPY lua ./lua

# Build argument for which package to build
ARG PACKAGE=alkanes-jsonrpc

# Build the specified package in release mode
RUN cargo build --release -p ${PACKAGE}

# Also build dbctl if package is alkanes-contract-indexer
RUN if [ "${PACKAGE}" = "alkanes-contract-indexer" ]; then \
      cargo build --release --bin dbctl; \
    fi

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    postgresql-client \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Build argument (must be redeclared in each stage)
ARG PACKAGE=alkanes-jsonrpc

# Copy the built binary from builder stage with its actual name
COPY --from=builder /app/target/release/${PACKAGE} /usr/local/bin/${PACKAGE}

# Create symlink for backwards compatibility with ENTRYPOINT
RUN ln -sf /usr/local/bin/${PACKAGE} /usr/local/bin/app

# For alkanes-contract-indexer: copy dbctl
RUN --mount=type=bind,from=builder,source=/app/target/release,target=/mnt/release \
    if [ "${PACKAGE}" = "alkanes-contract-indexer" ] && [ -f /mnt/release/dbctl ]; then \
      cp /mnt/release/dbctl /usr/local/bin/dbctl; \
    fi

# For alkanes-contract-indexer: copy docker-entrypoint.sh
RUN --mount=type=bind,source=crates/alkanes-contract-indexer,target=/mnt/indexer \
    if [ "${PACKAGE}" = "alkanes-contract-indexer" ] && [ -f /mnt/indexer/docker-entrypoint.sh ]; then \
      cp /mnt/indexer/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh && \
      chmod +x /usr/local/bin/docker-entrypoint.sh; \
    fi

# For alkanes-jsonrpc: create lua script directory for persistent caching
RUN if [ "${PACKAGE}" = "alkanes-jsonrpc" ]; then \
      mkdir -p /data/lua-scripts; \
    fi

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/app"]
