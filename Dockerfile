# Multi-stage build for SvrCtlRS server
FROM rust:bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Dioxus CLI for fullstack build
RUN cargo install dioxus-cli --version 0.7.1

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY server ./server
COPY scheduler ./scheduler
COPY database ./database
COPY plugins ./plugins

# Copy Dioxus configuration and assets
COPY Dioxus.toml ./
COPY assets ./assets

# Build with Dioxus CLI (creates server binary + WASM client assets)
RUN dx build --release --package server

# Also build svrctl CLI separately
RUN cargo build --release --bin svrctl

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN useradd -m -u 1000 -s /bin/bash svrctlrs

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/dx/server/release/web/server /app/svrctlrs-server
COPY --from=builder /app/target/release/svrctl /app/svrctl

# Copy Dioxus fullstack assets (WASM client + static files)
COPY --from=builder /app/target/dx/server/release/web/public /app/dist
COPY --from=builder /app/assets /app/assets

# Create data directory and set permissions
RUN mkdir -p /app/data && chown -R svrctlrs:svrctlrs /app

# Switch to non-root user
USER svrctlrs

# Expose port
EXPOSE 8080

# Set default environment variables
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:/app/data/svrctlrs.db

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/app/svrctl", "health"]

# Run server
CMD ["/app/svrctlrs-server"]
