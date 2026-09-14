FROM rust:1.83.0-bookworm as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    cargo \
    clang \
    cmake \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Clone and build electrs
WORKDIR /app
RUN git clone https://github.com/kungfuflex/flextrs.git . \
    && git checkout new-index \
    && git checkout 504bd533cf5fb6442f3962ed0d8f0c296e45ef37 \
    && cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/flextrs /usr/local/bin/flextrs

# Create required directories
RUN mkdir -p /data/bitcoin /data/electrs

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Set default data directories
ENV ELECTRS_DB_DIR=/data/electrs
ENV ELECTRS_DAEMON_DIR=/data/bitcoin

ENTRYPOINT ["docker-entrypoint.sh"]
