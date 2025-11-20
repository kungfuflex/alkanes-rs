# Multi-stage build for alkanes-data-api
FROM rust:1.75 as builder

WORKDIR /app

# Install protobuf compiler (required by metashrew-support)
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy all crate directories
COPY crates ./crates
COPY alkanes ./alkanes
COPY src ./src

# Build the release binary
RUN cargo build --release -p alkanes-data-api

# Runtime stage
FROM debian:bookworm-slim

# Install required runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/alkanes-data-api /usr/local/bin/alkanes-data-api

# Create non-root user
RUN useradd -m -u 1000 alkanes && \
    chown alkanes:alkanes /usr/local/bin/alkanes-data-api

USER alkanes

# Expose the default port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/v1/health || exit 1

# Run the application
CMD ["alkanes-data-api"]
