# Multi-stage Dockerfile for debshrew-mono
# Optimized for production deployment with minimal image size

# Build stage
FROM rust:1.75-slim as builder

# Install system dependencies required for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libclang-dev \
    cmake \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./

# Copy all crate directories (needed for workspace build)
COPY crates/ ./crates/
COPY src/ ./src/

# Build the application in release mode
RUN cargo build --release --bin debshrew-mono

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN useradd -r -s /bin/false -m -d /app debshrew

# Set working directory
WORKDIR /app

# Copy the built binary from builder stage
COPY --from=builder /app/target/release/debshrew-mono /usr/local/bin/debshrew-mono

# Create data directory
RUN mkdir -p /app/data && chown debshrew:debshrew /app/data

# Copy example configuration
COPY crates/debshrew-mono/examples/cdc-config.yaml /app/cdc-config.yaml.example

# Switch to app user
USER debshrew

# Expose default ports
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Default command
CMD ["debshrew-mono", "--help"]