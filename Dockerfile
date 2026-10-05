# syntax=docker/dockerfile:1
# Multi-stage build for the SvrCtlRS server image: SvelteKit SPA + Rust server + svrctl CLI.
# Built by .forgejo/workflows/docker.yaml on the epyc-ai `build` runner; cargo-chef and sccache
# keep rebuilds short once BuildKit's cache mounts are warm.

# ============================================
# Frontend: Build Svelte SPA
# ============================================
FROM node:22-slim AS frontend

WORKDIR /app/ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY ui/ ./
RUN npm run build

# ============================================
# Base: Install Rust build tools
# ============================================
# Pinned so a toolchain change is a deliberate commit, not a surprise on the next build.
FROM rust:1.98-bookworm AS base

# Install cargo-chef and sccache for optimal caching
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    cargo install cargo-chef --locked && \
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
COPY database ./database

# Generate recipe.json containing all workspace dependencies
RUN cargo chef prepare --recipe-path recipe.json

# ============================================
# Builder: Cook dependencies + build app
# ============================================
FROM base AS builder

# TLS is rustls/ring throughout (no openssl in Cargo.lock), so no libssl-dev is needed.
RUN apt-get update && apt-get install -y \
    pkg-config \
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
COPY database ./database

# Build server binary
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo build --release --package server --bin server --features server

# Build svrctl CLI
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

# The commit this image was built from; the CI job passes it and /api/v1/health reports it,
# which is how a deploy proves the running container is the commit it meant to ship.
ARG SVRCTLRS_REVISION=unknown
ENV SVRCTLRS_REVISION=${SVRCTLRS_REVISION}
LABEL org.opencontainers.image.revision=${SVRCTLRS_REVISION} \
      org.opencontainers.image.source="https://git.js-node.cc/jsprague/svrctlrs" \
      org.opencontainers.image.title="SvrCtlRS"

# Runtime dependencies. openssh-client is kept for the "local" terminal, where a user may
# want `ssh` and `ssh-keygen` inside the container's shell; sqlite is bundled into the binary.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN useradd -m -u 1000 -s /bin/bash svrctlrs

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/server /app/svrctlrs-server
COPY --from=builder /app/target/release/svrctl /app/svrctl

# Copy Svelte SPA build from frontend stage
COPY --from=frontend /app/ui/build /app/ui/build

# Create data directory and .ssh directory for the svrctlrs user
RUN mkdir -p /app/data && \
    mkdir -p /home/svrctlrs/.ssh && \
    chown -R svrctlrs:svrctlrs /app /home/svrctlrs/.ssh && \
    chmod 700 /home/svrctlrs/.ssh

# Switch to non-root user
USER svrctlrs

# Expose port
EXPOSE 8080

# Set default environment variables
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:/app/data/svrctlrs.db
ENV SPA_DIR=/app/ui/build

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/app/svrctl", "health"]

# Run server
CMD ["/app/svrctlrs-server"]
