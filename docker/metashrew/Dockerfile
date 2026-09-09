# Build stage
FROM rust:1-slim-bookworm as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    pkg-config \
    clang \
    libclang-dev \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Add wasm32 target
RUN rustup target add wasm32-unknown-unknown

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY src ./src

# Build alkanes.wasm
RUN cargo build --release -p alkanes --target wasm32-unknown-unknown

# Build rockshrew-mono binary
RUN cargo build --release -p rockshrew-mono

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary, wasm, and entrypoint
COPY --from=builder /build/target/release/rockshrew-mono /usr/local/bin/
COPY --from=builder /build/target/wasm32-unknown-unknown/release/alkanes.wasm /metashrew/indexer.wasm
COPY docker/metashrew/docker-entrypoint.sh /docker-entrypoint.sh

# Create non-root user
RUN useradd -r -s /bin/false metashrew \
    && mkdir -p /data \
    && chown metashrew:metashrew /data /metashrew

USER metashrew
VOLUME /data

CMD ["bash", "/docker-entrypoint.sh"]
