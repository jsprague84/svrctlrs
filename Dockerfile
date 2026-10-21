# syntax=docker/dockerfile:1
# Multi-stage build for the SvrCtlRS server image: SvelteKit SPA + Rust server + svrctl CLI.
# Built by .forgejo/workflows/docker.yaml on the epyc-ai `build` runner. Each run gets a fresh
# builder, so the layers are cached in the registry (`:buildcache`): the tools, the npm install
# and cargo-chef's dependency build are reused until their inputs change.
#
# linux/amd64 and linux/arm64 (Raspberry Pi 4/5 and other 64-bit ARM hosts). Every stage that
# runs a command runs on the build machine's own platform ($BUILDPLATFORM): the SPA is
# platform-independent, Rust cross-compiles for $TARGETARCH, and the runtime stage only copies
# files in. So an arm64 image builds on an amd64 runner without QEMU.

# ============================================
# Frontend: Build Svelte SPA
# ============================================
FROM --platform=$BUILDPLATFORM node:22-slim AS frontend

WORKDIR /app/ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY ui/ ./
RUN npm run build

# ============================================
# Base: Install Rust build tools
# ============================================
# Pinned so a toolchain change is a deliberate commit, not a surprise on the next build.
FROM --platform=$BUILDPLATFORM rust:1.98-bookworm AS base

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
COPY api ./api
COPY core ./core
COPY server ./server
COPY database ./database

# Generate recipe.json containing all workspace dependencies
RUN cargo chef prepare --recipe-path recipe.json

# ============================================
# Builder: Cook dependencies + build app
# ============================================
FROM base AS builder
ARG TARGETARCH

# TLS is rustls/ring throughout (no openssl in Cargo.lock), so no libssl-dev is needed. An arm64
# target needs the cross C toolchain for the crates that compile C (ring, aws-lc, sqlite).
RUN apt-get update && apt-get install -y pkg-config \
    && if [ "$TARGETARCH" = "arm64" ]; then \
         apt-get install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu libc6-dev-arm64-cross; \
       fi \
    && rm -rf /var/lib/apt/lists/*
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-gnu ;; \
      arm64) echo aarch64-unknown-linux-gnu ;; \
      *) echo "unsupported TARGETARCH $TARGETARCH" >&2; exit 1 ;; \
    esac > /rust-target \
    && rustup target add "$(cat /rust-target)" \
    && case "$TARGETARCH" in arm64) echo 4227 ;; *) echo 4226 ;; esac > /sccache-port
# BuildKit runs the amd64 and arm64 builds side by side on one host network. sccache's client
# talks to its server on localhost, so each architecture gets its own port; otherwise the arm64
# compiles run in the amd64 build's server, whose toolchain has no aarch64 standard library.
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar

# Copy dependency recipe from planner
COPY --from=planner /app/recipe.json recipe.json

# Cook dependencies with cache mounts, one set per target architecture so the amd64 and arm64
# builds run side by side instead of queueing on a locked mount
# This layer is cached until Cargo.lock changes
RUN --mount=type=cache,id=cargo-registry-$TARGETARCH,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git-$TARGETARCH,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=sccache-$TARGETARCH,target=/sccache,sharing=locked \
    SCCACHE_SERVER_PORT="$(cat /sccache-port)" \
    cargo chef cook --release --target "$(cat /rust-target)" --recipe-path recipe.json

# Copy source code (invalidates cache only when source changes)
COPY Cargo.toml Cargo.lock ./
COPY api ./api
COPY core ./core
COPY server ./server
COPY database ./database

# Build the server and the svrctl CLI in one invocation so shared crates compile once
RUN --mount=type=cache,id=cargo-registry-$TARGETARCH,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git-$TARGETARCH,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=sccache-$TARGETARCH,target=/sccache,sharing=locked \
    SCCACHE_SERVER_PORT="$(cat /sccache-port)" \
    cargo build --release --target "$(cat /rust-target)" --package server --bins --features server \
    && mkdir -p /out \
    && cp "target/$(cat /rust-target)/release/server" /out/svrctlrs-server \
    && cp "target/$(cat /rust-target)/release/svrctl" /out/svrctl \
    && rm -rf target
# `rm -rf target` in the same step keeps this layer to the two binaries: the CI exports every
# layer to the registry cache, and the dependency layer above (cached until Cargo.lock
# changes) must not be re-uploaded with a fresh copy of target/ on every push.

# Show sccache statistics for debugging
RUN SCCACHE_SERVER_PORT="$(cat /sccache-port)" sccache --show-stats || true

# ============================================
# Runtime files: the app user and its directories, prepared on the build platform so the
# runtime stage needs no RUN (which on another architecture would need an emulator)
# ============================================
FROM --platform=$BUILDPLATFORM debian:bookworm-slim AS rootfs
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 -s /bin/bash svrctlrs \
    && mkdir -p /rootfs/etc /rootfs/app/data /rootfs/home/svrctlrs/.ssh \
    && cp /etc/passwd /etc/group /etc/shadow /etc/gshadow /rootfs/etc/ \
    && cp -a /etc/ssl /rootfs/etc/ssl \
    && cp -a /usr/share/ca-certificates /rootfs/ca-certificates \
    && chmod 700 /rootfs/home/svrctlrs/.ssh

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

# The binary needs no shared libraries beyond glibc and opens no subprocesses; TLS roots are
# compiled in (webpki-roots), and the system bundle is copied for anything else.
COPY --from=rootfs /rootfs/etc/passwd /rootfs/etc/group /etc/
COPY --from=rootfs --chmod=640 /rootfs/etc/shadow /rootfs/etc/gshadow /etc/
COPY --from=rootfs /rootfs/etc/ssl /etc/ssl
COPY --from=rootfs /rootfs/ca-certificates /usr/share/ca-certificates
COPY --from=rootfs --chown=1000:1000 /rootfs/home/svrctlrs /home/svrctlrs
COPY --from=rootfs --chown=1000:1000 /rootfs/app /app

WORKDIR /app

# Binaries for the target architecture, and the SPA
COPY --from=builder --chown=1000:1000 /out/svrctlrs-server /out/svrctl /app/
COPY --from=frontend --chown=1000:1000 /app/ui/build /app/ui/build

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
