# Build stage
FROM rust:1-slim-bookworm as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    pkg-config \
    libclang-dev \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY src ./src

# Build memshrew-p2p binary
RUN cargo build --release -p memshrew-p2p

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary and entrypoint
COPY --from=builder /build/target/release/memshrew-p2p /usr/local/bin/
COPY docker/memshrew/docker-entrypoint.sh /docker-entrypoint.sh

# Create non-root user
RUN useradd -r -s /bin/false metashrew \
    && mkdir -p /data \
    && mkdir -p /metashrew \
    && chown metashrew:metashrew /data /metashrew

USER metashrew
VOLUME /data

CMD ["bash", "/docker-entrypoint.sh"]
