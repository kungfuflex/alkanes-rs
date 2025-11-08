# Build stage
FROM rust:1-slim-bookworm as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    pkg-config \
    libclang-dev \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Switch to a nightly toolchain that supports edition 2024
RUN rustup default nightly

# Clone espo repository
WORKDIR /build
RUN git clone https://github.com/bitapeslabs/espo.git .

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /build/target/release/espo /usr/local/bin/

# Create non-root user and required directories
RUN useradd -r -s /bin/false espo \
    && mkdir -p /data /db /tmp_db \
    && chown -R espo:espo /data /db /tmp_db

USER root
VOLUME /data

WORKDIR /root
EXPOSE 9069
ENTRYPOINT ["/usr/local/bin/espo"]
