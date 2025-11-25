# syntax=docker/dockerfile:1
# Multi-stage optimized build for SvrCtlRS with cargo-chef + sccache

# ============================================
# Base: Install build tools
# ============================================
FROM rust:bookworm AS base

# Install cargo-chef and sccache for optimal caching
RUN cargo install cargo-chef --locked && \
    cargo install sccache --version ^0.8 --locked

# Configure sccache
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache

WORKDIR /app

# ============================================
# Planner: Generate dependency recipe
# ============================================
FROM base AS planner

# Copy entire workspace to analyze dependencies
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY server ./server
COPY scheduler ./scheduler
COPY database ./database
COPY plugins ./plugins

# Generate recipe.json containing all workspace dependencies
RUN cargo chef prepare --recipe-path recipe.json

# ============================================
# Builder: Cook dependencies + build app
# ============================================
FROM base AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency recipe from planner
COPY --from=planner /app/recipe.json recipe.json

# Cook dependencies with cache mounts
# This layer is cached until Cargo.lock changes
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# Copy source code (invalidates cache only when source changes)
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY server ./server
COPY scheduler ./scheduler
COPY database ./database
COPY plugins ./plugins
COPY assets ./assets

# Build server binary with SSR support (no WASM client for now)
# Note: dx build doesn't work in Docker due to target triple detection issues
# Building server binary directly with cargo + server feature flag
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo build --release --package server --bin server --features server

# Build svrctl CLI (also needs server feature for dependencies)
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo build --release --package server --bin svrctl --features server

# Show sccache statistics for debugging
RUN sccache --show-stats || true

# ============================================
# Runtime: Minimal production image
# ============================================
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
COPY --from=builder /app/target/release/server /app/svrctlrs-server
COPY --from=builder /app/target/release/svrctl /app/svrctl

# Copy source assets (for SSR templates if needed)
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
