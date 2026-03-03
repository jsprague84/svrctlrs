# CI/CD Optimization Guide: Applying SvrCtlRS Patterns to rustRoast

This guide documents the CI/CD optimizations implemented in SvrCtlRS and provides a step-by-step migration path for applying the same patterns to the rustRoast project.

---

## 1. Overview

### What SvrCtlRS Has

SvrCtlRS uses a **4-workflow CI/CD architecture**:

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `test.yml` | PRs, push to main/develop/master | Rust tests, clippy, fmt, build, frontend checks |
| `docker-publish-main.yml` | Push to master, `v*` tags | Multi-arch Docker build + GitHub Release |
| `docker-publish-develop.yml` | Push to develop | Fast AMD64-only Docker build |
| `docker-publish-dev-branch.yml` | Push to `ralph/**` | Fast AMD64-only Docker build for dev branches |

**Key features:**
- **Concurrency groups** prevent duplicate builds; PRs cancel superseded runs
- **Reusable test workflow** (`workflow_call`) — Docker workflows call `test.yml` before building
- **cargo-chef + sccache Dockerfile** — dependency caching for fast rebuilds
- **Auto GitHub Releases** on `v*` tags with generated release notes
- **Frontend CI** — SvelteKit typecheck + build runs alongside Rust checks

### What rustRoast Currently Has

rustRoast has a **2-workflow setup**:

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | PRs + push to main | Rust + frontend checks (single workflow) |
| `docker-publish.yml` | Push to main | Single-platform Docker build |

**Current Dockerfile:** Basic 3-stage build (frontend → backend → runtime) without cargo-chef or sccache.

### What to Apply

1. **Dockerfile:** Add cargo-chef + sccache for dependency caching
2. **Concurrency groups:** Add to both workflows
3. **Reusable test workflow:** Make `ci.yml` callable via `workflow_call`
4. **Release job:** Auto GitHub Release on `v*` tags
5. **Develop workflow:** Add a fast-iteration workflow for develop branch
6. **Multi-arch builds:** AMD64 + ARM64 for production

---

## 2. Dockerfile: cargo-chef + sccache

Replace the current 3-stage Dockerfile with a 5-stage optimized build.

### Current rustRoast Dockerfile (3-stage)

```dockerfile
# Stage 1: Frontend
FROM node:20-alpine AS frontend
WORKDIR /app/apps/dashboard
COPY apps/dashboard/package.json apps/dashboard/package-lock.json ./
RUN npm ci
COPY apps/dashboard/ ./
RUN npm run build

# Stage 2: Backend (no dependency caching)
FROM rust:1.85-slim-bookworm AS backend
RUN apt-get update && apt-get install -y pkg-config libssl-dev curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p rustroast-server

# Stage 3: Runtime
FROM debian:bookworm-slim
# ...
```

### Optimized rustRoast Dockerfile (5-stage)

```dockerfile
# syntax=docker/dockerfile:1
# Multi-stage optimized build for rustRoast with cargo-chef + sccache

# ============================================
# Frontend: Build SvelteKit SPA
# ============================================
FROM node:22-slim AS frontend

WORKDIR /app/apps/dashboard
COPY apps/dashboard/package.json apps/dashboard/package-lock.json ./
RUN npm ci
COPY apps/dashboard/ ./
RUN npm run build

# ============================================
# Base: Install Rust build tools
# ============================================
FROM rust:bookworm AS base

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

# Copy workspace manifest and all crates
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

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
    curl \
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
COPY crates/ crates/

# Build the server binary
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo build --release -p rustroast-server

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
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN groupadd -r rustroast && useradd -r -g rustroast rustroast

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/rustroast-server /usr/local/bin/rustroast-server

# Copy SvelteKit SPA build from frontend stage
COPY --from=frontend /app/apps/dashboard/build/ /app/static/

# Create data directory
RUN mkdir -p /app/data && chown -R rustroast:rustroast /app

# Switch to non-root user
USER rustroast

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/healthz || exit 1

EXPOSE 8080

ENV RUSTROAST_APP_DIR=/app/static
ENV RUSTROAST_HTTP_ADDR=0.0.0.0:8080
ENV MQTT_BROKER_HOST=mosquitto
ENV MQTT_BROKER_PORT=1883
ENV RUSTROAST_DB_PATH=/app/data/rustroast.db
ENV RUST_LOG=info

CMD ["rustroast-server"]
```

### Key Differences from SvrCtlRS

| Aspect | SvrCtlRS | rustRoast |
|--------|----------|-----------|
| Workspace layout | `core/`, `server/`, `database/` | `crates/` directory |
| Binary name | `server`, `svrctl` | `rustroast-server` |
| Frontend path | `ui/` | `apps/dashboard/` |
| Frontend output | `ui/build` → `/app/ui/build` | `apps/dashboard/build` → `/app/static/` |
| Build command | `cargo build --release --package server --bin server --features server` | `cargo build --release -p rustroast-server` |
| Runtime deps | `openssh-client`, `sqlite3` | `curl` (for healthcheck) |

### Why This Is Faster

1. **cargo-chef** separates dependency compilation from source compilation. When only source files change, dependencies are fully cached.
2. **sccache** provides an additional compilation cache layer that persists across Docker builds.
3. **BuildKit cache mounts** keep cargo registry, git checkouts, and sccache data between builds without bloating image layers.

---

## 3. Workflows: Restructure

### 3a. Make CI Workflow Reusable

Update `ci.yml` to support `workflow_call` so Docker workflows can gate on it:

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]
  workflow_dispatch:
  workflow_call:  # Allow Docker workflows to call this

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

env:
  CARGO_TERM_COLOR: always

jobs:
  rust:
    name: Rust (build, test, lint)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Build
        run: cargo build --workspace

      - name: Run tests
        run: cargo test --workspace

      - name: Clippy
        run: cargo clippy --workspace -- -D warnings -A dead_code

  frontend:
    name: Frontend (check, build)
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: apps/dashboard
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: apps/dashboard/package-lock.json

      - name: Install dependencies
        run: npm ci

      - name: Svelte check
        run: npm run check

      - name: Build
        run: npm run build

  ci-success:
    name: CI Success
    needs: [rust, frontend]
    runs-on: ubuntu-latest
    if: always()
    steps:
      - name: Check all jobs succeeded
        run: |
          if [ "${{ contains(needs.*.result, 'failure') }}" = "true" ]; then
            echo "One or more CI jobs failed"
            exit 1
          elif [ "${{ contains(needs.*.result, 'cancelled') }}" = "true" ]; then
            echo "One or more CI jobs were cancelled"
            exit 1
          else
            echo "All CI jobs passed"
          fi
```

### 3b. Docker Publish — Main (Multi-arch + Release)

```yaml
# .github/workflows/docker-publish-main.yml
name: Docker Publish - Main (Multi-arch)

on:
  push:
    branches: [main]
    tags:
      - 'v*.*.*'
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  test:
    uses: ./.github/workflows/ci.yml

  build-and-push:
    needs: test
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
      id-token: write
      attestations: write

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=ref,event=branch
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            type=sha
            type=raw,value=latest,enable={{is_default_branch}}

      - name: Build and push (AMD64 + ARM64)
        id: build
        uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
          build-args: |
            BUILDKIT_INLINE_CACHE=1

      - name: Generate artifact attestation
        uses: actions/attest-build-provenance@v1
        with:
          subject-name: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          subject-digest: ${{ steps.build.outputs.digest }}
          push-to-registry: true

  release:
    needs: build-and-push
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-latest
    permissions:
      contents: write

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
```

### 3c. Docker Publish — Develop (Fast)

```yaml
# .github/workflows/docker-publish-develop.yml
name: Docker Publish - Develop (Fast AMD64)

on:
  push:
    branches: [develop]
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  test:
    uses: ./.github/workflows/ci.yml

  build-and-push:
    needs: test
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=raw,value=develop
            type=sha,prefix=develop-

      - name: Build and push (AMD64 only)
        uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha,scope=develop
          cache-to: type=gha,mode=max,scope=develop
          build-args: |
            BUILDKIT_INLINE_CACHE=1
```

---

## 4. Step-by-Step Checklist

### Phase 1: Dockerfile Optimization

- [ ] Back up current `Dockerfile`
- [ ] Replace with 5-stage cargo-chef + sccache version from Section 2
- [ ] Test locally: `docker build -t rustroast-test .`
- [ ] Verify the image runs: `docker run --rm rustroast-test --help`
- [ ] Build again to confirm cache hits (second build should be significantly faster)
- [ ] Upgrade Node base image from `node:20-alpine` to `node:22-slim` (consistency)

### Phase 2: CI Workflow Updates

- [ ] Add `workflow_call` trigger to `ci.yml`
- [ ] Add `concurrency` block to `ci.yml`
- [ ] Add `develop` branch to `ci.yml` triggers
- [ ] Add `ci-success` summary job
- [ ] Upgrade Node from 20 to 22 in CI
- [ ] Test: push to a PR and verify concurrency cancellation works

### Phase 3: Docker Workflows

- [ ] Rename `docker-publish.yml` to `docker-publish-main.yml`
- [ ] Add `v*` tag trigger for release builds
- [ ] Add `test` job that calls `ci.yml` via `workflow_call`
- [ ] Gate `build-and-push` on `test` with `needs: test`
- [ ] Add concurrency block
- [ ] Add QEMU + multi-arch (`linux/amd64,linux/arm64`)
- [ ] Add artifact attestation step
- [ ] Add `release` job with `softprops/action-gh-release@v2`
- [ ] Verify `build-push-action` is already at `@v6` (rustRoast already uses v6)

### Phase 4: Develop Workflow (New)

- [ ] Create `.github/workflows/docker-publish-develop.yml`
- [ ] AMD64-only for fast iteration
- [ ] Tags: `develop`, `develop-{sha}`
- [ ] Scoped GHA cache: `scope=develop`
- [ ] Gate on CI tests passing

### Phase 5: Release Flow

- [ ] Create a test tag: `git tag v0.1.0-rc1 && git push --tags`
- [ ] Verify GitHub Release is created with auto-generated notes
- [ ] Verify Docker images are tagged with `0.1.0-rc1`, `0.1`, `0`
- [ ] Clean up test tag if needed

### Phase 6: Branch Protection (Optional)

- [ ] Set branch protection on `main` to require `ci-success` check
- [ ] Set branch protection on `develop` to require `ci-success` check

---

## Summary of Differences

| Feature | SvrCtlRS | rustRoast (after migration) |
|---------|----------|-----------------------------|
| Workspace `COPY` | `core/`, `server/`, `database/` | `crates/` |
| Binary target | `--package server --bin server --features server` | `-p rustroast-server` |
| Frontend dir | `ui/` | `apps/dashboard/` |
| CI workflow | `test.yml` | `ci.yml` |
| Extra binary | `svrctl` CLI | None |
| Runtime deps | `openssh-client`, `sqlite3` | `curl` |
| Docker v6 | Upgraded from v5 | Already on v6 |
