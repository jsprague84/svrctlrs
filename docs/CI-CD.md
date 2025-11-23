# CI/CD Workflows

This document describes the CI/CD strategy for SvrCtlRS, including workflows, deployment patterns, and best practices.

## Overview

SvrCtlRS uses a **two-workflow strategy** optimized for fast development iteration and stable production releases:

1. **CI Workflow** - Fast development builds (amd64 only)
2. **Release Workflow** - Production releases (multi-arch: amd64 + arm64)

---

## Workflow Comparison

| Feature | CI Workflow | Release Workflow |
|---------|-------------|------------------|
| **File** | `.github/workflows/ci.yml` | `.github/workflows/release.yml` |
| **Triggers** | Push to `develop`, PRs, Manual | Tags `v*.*.*`, Manual |
| **Platforms** | `linux/amd64` only | `linux/amd64` + `linux/arm64` |
| **Build Time*** | ~5-7 minutes | ~10-15 minutes |
| **Image Tags** | `:develop`, `:pr-123` | `:latest`, `:v2.2.0`, `:2.2`, `:2` |
| **Purpose** | Development & testing | Production releases |
| **Runs Tests** | Yes (fmt, clippy, test) | No |
| **GitHub Release** | No | Yes (with notes) |
| **SBOM/Provenance** | Yes | Yes |

\* After initial cache population. First build ~10-15 minutes.

---

## Development Workflow

### 1. Daily Development

Work on the `develop` branch for all feature development:

```bash
# Start feature development
git checkout develop
git pull origin develop

# Make changes
vim src/...

# Commit and push
git add .
git commit -m "feat: add new monitoring plugin"
git push origin develop
```

**What happens automatically:**
1. CI workflow triggers (~5-7 minutes)
2. Runs tests (format, clippy, unit tests)
3. Builds Docker image (amd64 only)
4. Pushes to `ghcr.io/jsprague84/svrctlrs:develop`

### 2. Test on Test Server

Your test server uses the `:develop` tag:

**docker-compose.yml (test server):**
```yaml
version: '3.8'

services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:develop
    container_name: svrctlrs
    ports:
      - "8080:8080"
    volumes:
      - ./data:/app/data
      - ./config.toml:/app/config.toml:ro
    environment:
      - RUST_LOG=info
    restart: unless-stopped
```

**Deploy to test server:**
```bash
# SSH to test server
ssh test-server

# Pull latest develop image
docker-compose pull

# Restart with new image
docker-compose up -d

# Watch logs
docker-compose logs -f svrctlrs
```

**Alternative (one-liner):**
```bash
ssh test-server "cd /path/to/svrctlrs && docker-compose pull && docker-compose up -d"
```

### 3. Iterate Quickly

The development loop is fast:

```bash
# Make change → push
git commit -am "fix: correct notification timing"
git push origin develop
# Wait ~7 minutes for CI

# Test immediately
ssh test-server "cd /path/to/svrctlrs && docker-compose pull && docker-compose up -d"
# New image deployed!
```

**Time from commit to testing**: ~7-10 minutes

### 4. Create Release

When `develop` is stable and ready for production:

```bash
# Merge to master
git checkout master
git pull origin master
git merge develop
git push origin master  # No workflow triggered yet

# Create release tag
git tag -a v2.3.0 -m "Release v2.3.0: Add monitoring features

- Add new Docker health plugin
- Fix WASM asset loading
- Optimize build times (5x faster)

Breaking changes: None"

# Push tag to trigger release
git push origin v2.3.0
```

**What happens automatically:**
1. Release workflow triggers (~10-15 minutes)
2. Builds multi-arch images (amd64 + arm64)
3. Pushes to multiple tags:
   - `ghcr.io/jsprague84/svrctlrs:latest`
   - `ghcr.io/jsprague84/svrctlrs:v2.3.0`
   - `ghcr.io/jsprague84/svrctlrs:2.3`
   - `ghcr.io/jsprague84/svrctlrs:2`
4. Creates GitHub release with notes

### 5. Deploy to Production

Your production server uses the `:latest` tag:

**docker-compose.yml (production):**
```yaml
version: '3.8'

services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:latest
    container_name: svrctlrs
    ports:
      - "8080:8080"
    volumes:
      - ./data:/app/data
      - ./config.toml:/app/config.toml:ro
    environment:
      - RUST_LOG=info
    restart: unless-stopped
```

**Deploy to production:**
```bash
# SSH to production server
ssh production-server

# Pull latest release
docker-compose pull

# Restart with new image
docker-compose up -d

# Verify
docker-compose logs -f svrctlrs
```

---

## CI Workflow Details

### Triggers

**Automatic:**
- Push to `develop` branch
- Pull requests to `master` or `develop`

**Manual:**
```bash
gh workflow run "CI - Build and Push (Development)"
```

### Steps

1. **Test Job**
   - Install Rust toolchain
   - Cache dependencies (Swatinem/rust-cache)
   - Run `cargo fmt --check`
   - Run `cargo clippy`
   - Run `cargo test --workspace`

2. **Build Job** (only if tests pass)
   - Set up Docker Buildx
   - Log in to GHCR
   - Build image with cargo-chef + sccache
   - Single platform: `linux/amd64`
   - Push to GHCR (except for PRs)

### Image Tags

```bash
# Develop branch
ghcr.io/jsprague84/svrctlrs:develop
ghcr.io/jsprague84/svrctlrs:develop-sha-abc123

# Pull requests (built but not pushed to registry)
# Available in PR comment for testing
```

### Performance

**First build** (cache population):
- Duration: ~10-15 minutes
- Downloads all dependencies
- Compiles everything from scratch
- Populates GitHub Actions cache

**Subsequent builds** (cache hits):
- Duration: ~5-7 minutes
- Reuses cached dependencies (cargo-chef)
- Reuses cached artifacts (sccache)
- Only rebuilds changed code

**Time savings**: **50-60% faster** than multi-arch builds

---

## Release Workflow Details

### Triggers

**Automatic:**
- Push tag matching `v*.*.*` (e.g., `v2.3.0`)

**Manual:**
```bash
gh workflow run "Release - Build and Publish (Production)"
```

### Steps

1. **Build and Push Job**
   - Set up Docker Buildx
   - Log in to GHCR
   - Build multi-arch images (amd64 + arm64)
   - Use cargo-chef + sccache optimizations
   - Push to GHCR with multiple tags
   - Generate provenance and SBOM

2. **Create Release Job**
   - Create GitHub release
   - Add auto-generated release notes
   - Include installation instructions

### Image Tags

For tag `v2.3.0`:
```bash
ghcr.io/jsprague84/svrctlrs:latest       # Latest release
ghcr.io/jsprague84/svrctlrs:v2.3.0      # Specific version
ghcr.io/jsprague84/svrctlrs:2.3         # Minor version
ghcr.io/jsprague84/svrctlrs:2           # Major version
```

### Performance

**First build** (cache population):
- Duration: ~15-20 minutes
- Builds for two platforms
- Roughly 2x single-platform time

**Subsequent builds** (cache hits):
- Duration: ~10-12 minutes
- Shares cache with CI workflow
- Only rebuilds changed code for both platforms

---

## Environment Setup

### Test Server Configuration

```bash
# Directory structure
/opt/svrctlrs/
├── docker-compose.yml    # Uses :develop tag
├── config.toml
└── data/
    └── svrctlrs.db

# docker-compose.yml
version: '3.8'
services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:develop  # ← develop tag
    container_name: svrctlrs-test
    ports:
      - "8080:8080"
    volumes:
      - ./data:/app/data
      - ./config.toml:/app/config.toml:ro
    environment:
      - RUST_LOG=debug  # More verbose for testing
    restart: unless-stopped
```

### Production Server Configuration

```bash
# Directory structure
/opt/svrctlrs/
├── docker-compose.yml    # Uses :latest tag
├── config.toml
└── data/
    └── svrctlrs.db

# docker-compose.yml
version: '3.8'
services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:latest  # ← latest tag
    container_name: svrctlrs
    ports:
      - "8080:8080"
    volumes:
      - ./data:/app/data
      - ./config.toml:/app/config.toml:ro
    environment:
      - RUST_LOG=info  # Production logging
    restart: unless-stopped
```

---

## Monitoring Workflows

### Check CI Status

```bash
# List recent CI runs
gh run list --workflow="CI - Build and Push (Development)" --limit 5

# Watch current CI run
gh run watch

# View specific run
gh run view <run-id>

# View logs
gh run view <run-id> --log
```

### Check Release Status

```bash
# List recent releases
gh run list --workflow="Release - Build and Publish (Production)" --limit 5

# View release
gh release view v2.3.0

# List all releases
gh release list
```

### Check Image Tags

```bash
# List all tags for the image
gh api /orgs/jsprague84/packages/container/svrctlrs/versions | jq -r '.[].metadata.container.tags[]' | sort -u

# Or using Docker CLI
docker pull ghcr.io/jsprague84/svrctlrs:develop
docker images ghcr.io/jsprague84/svrctlrs
```

---

## Build Optimizations

Both workflows use the same optimizations for maximum performance:

### 1. cargo-chef Pattern

**What it does**: Caches compiled dependencies as a Docker layer

**How it works**:
```dockerfile
# Planner stage: Generate dependency recipe
FROM rust:bookworm AS planner
COPY Cargo.toml Cargo.lock ./
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage: Cook dependencies (cached layer)
FROM rust:bookworm AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# ↑ This layer is cached until Cargo.lock changes

# Copy source and build (only this runs on code changes)
COPY . .
RUN dx build --release
```

**Performance gain**: 5-10x faster builds after first run

### 2. sccache

**What it does**: Caches individual compilation artifacts

**How it works**:
```dockerfile
ENV RUSTC_WRAPPER=sccache
ENV SCCACHE_DIR=/sccache
RUN --mount=type=cache,target=/sccache cargo build --release
```

**Performance gain**: 2-3x faster compilation

### 3. BuildKit Cache Mounts

**What it does**: Persists cargo registry and git caches across builds

**How it works**:
```dockerfile
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/sccache,sharing=locked \
    cargo build --release
```

**Performance gain**: Eliminates dependency re-downloads

### 4. GitHub Actions Cache

**What it does**: Stores Docker build cache between workflow runs

**Configuration**:
```yaml
- uses: docker/build-push-action@v6
  with:
    cache-from: type=gha
    cache-to: type=gha,mode=max  # Cache all layers
```

**Performance gain**: Fast subsequent builds

### 5. cargo-binstall

**What it does**: Installs pre-built binaries instead of compiling

**Example**:
```dockerfile
# Slow: cargo install dioxus-cli (compiles from source, ~10 min)
# Fast: cargo binstall dioxus-cli (downloads binary, ~30 sec)
RUN cargo install cargo-binstall --locked && \
    cargo binstall dioxus-cli --version 0.7.1 --no-confirm
```

**Performance gain**: 10-20x faster CLI tool installation

### 6. LLD Linker for WASM

**What it does**: Uses faster LLVM linker for WASM builds

**Configuration** (`.cargo/config.toml`):
```toml
[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

**Performance gain**: 19x faster WASM linking (48s → 2.5s)

---

## Performance Benchmarks

### Build Time Comparison

| Scenario | Without Optimizations | With Optimizations | Improvement |
|----------|----------------------|-------------------|-------------|
| **First build** | 25-30 min | 10-15 min | 2x faster |
| **Code change** | 25-30 min | 1-2 min | **15x faster** |
| **Dependency change** | 25-30 min | 3-5 min | 6x faster |
| **No changes** | 25-30 min | 30-60 sec | **30x faster** |

### Single vs Multi-Platform

| Build Type | Duration | Use Case |
|------------|----------|----------|
| **Single (amd64)** | ~5-7 min | Development, testing |
| **Multi (amd64 + arm64)** | ~10-15 min | Production releases |
| **Savings** | ~50% | Development iteration |

### Real-World Example

**10 commits during development:**

| Strategy | Time per Build | Total Time | Time Saved |
|----------|---------------|------------|------------|
| Multi-arch every commit | 15 min | **150 min** | - |
| Single-arch CI + multi-arch release | 7 min × 10 + 15 min × 1 | **85 min** | **65 min (43%)** |

---

## Troubleshooting

### CI Workflow Not Triggering

**Symptom**: Push to `develop` doesn't trigger workflow

**Causes & Solutions**:

1. **Wrong branch**: Make sure you're on `develop`
   ```bash
   git branch  # Should show * develop
   git checkout develop
   ```

2. **Skip CI commit message**: Check for `[skip ci]` in commit
   ```bash
   git log -1  # View last commit message
   ```

3. **Workflow disabled**: Check GitHub settings
   - Go to Actions tab → Enable workflows

### Build Failing

**Symptom**: CI workflow fails during build

**Common causes**:

1. **Compilation errors**:
   ```bash
   # Test locally first
   cargo check --workspace
   cargo clippy --workspace
   cargo test --workspace
   ```

2. **Docker build issues**:
   ```bash
   # Test Docker build locally
   docker build -t svrctlrs:test .
   ```

3. **Cache corruption**:
   - Go to Actions → Caches → Delete cache
   - Re-run workflow

### Image Not Pulling

**Symptom**: `docker-compose pull` doesn't get new image

**Solutions**:

1. **Check if workflow succeeded**:
   ```bash
   gh run list --workflow="CI - Build and Push (Development)" --limit 1
   ```

2. **Check image timestamp**:
   ```bash
   docker pull ghcr.io/jsprague84/svrctlrs:develop
   docker images ghcr.io/jsprague84/svrctlrs:develop
   # Check CREATED column
   ```

3. **Force pull**:
   ```bash
   docker-compose pull --ignore-pull-failures
   docker-compose up -d --force-recreate
   ```

4. **Clear local cache**:
   ```bash
   docker rmi ghcr.io/jsprague84/svrctlrs:develop
   docker-compose pull
   ```

### Release Workflow Issues

**Symptom**: Tag pushed but release workflow doesn't trigger

**Causes & Solutions**:

1. **Tag format wrong**: Must match `v*.*.*`
   ```bash
   # Wrong: 2.3.0, release-2.3.0
   # Correct: v2.3.0
   git tag -d wrong-tag  # Delete wrong tag
   git tag v2.3.0        # Create correct tag
   git push origin v2.3.0
   ```

2. **Tag not pushed**:
   ```bash
   git push origin --tags  # Push all tags
   # or
   git push origin v2.3.0  # Push specific tag
   ```

---

## Best Practices

### Branch Strategy

```
master (stable)
  ↑
  └── develop (active development)
        ↑
        └── feature/new-plugin (feature branches)
```

**Rules**:
- All development happens on `develop`
- Feature branches merge to `develop`
- Only merge `develop` → `master` when ready for release
- Tag releases on `master` only

### Commit Messages

Follow conventional commits for clear history:

```bash
feat: add new monitoring plugin
fix: resolve WASM loading issue
docs: update CI/CD documentation
perf: optimize Docker build with cargo-chef
chore: update dependencies
```

### Version Numbering

Follow semantic versioning (semver):

- **Major** (v3.0.0): Breaking changes
- **Minor** (v2.3.0): New features, backward compatible
- **Patch** (v2.2.1): Bug fixes only

### Testing Before Release

Always test on test server before creating release:

```bash
# 1. Push to develop
git checkout develop
git push origin develop

# 2. Wait for CI (~7 min)
gh run watch

# 3. Test on test server
ssh test-server "cd /opt/svrctlrs && docker-compose pull && docker-compose up -d"

# 4. Verify functionality
curl http://test-server:8080/api/health

# 5. If all good, release
git checkout master
git merge develop
git tag v2.3.0
git push origin v2.3.0
```

### Cache Management

**Keep cache healthy**:

```bash
# View cache usage
gh api /repos/jsprague84/svrctlrs/actions/caches | jq '.total_count'

# Delete old caches (when > 10 GB)
gh cache delete <cache-id>

# Or delete all (force rebuild)
gh cache list | cut -f1 | xargs -I {} gh cache delete {}
```

---

## Quick Reference

### Commands

```bash
# Development
git checkout develop
git push origin develop                    # Trigger CI
ssh test-server "docker-compose pull"      # Deploy to test

# Release
git tag -a v2.3.0 -m "Release message"
git push origin v2.3.0                     # Trigger release
ssh prod-server "docker-compose pull"      # Deploy to prod

# Monitoring
gh run list                                # Recent runs
gh run watch                               # Watch current
gh release list                            # List releases

# Manual triggers
gh workflow run "CI - Build and Push (Development)"
gh workflow run "Release - Build and Publish (Production)"
```

### Image Tags

```bash
# Development
ghcr.io/jsprague84/svrctlrs:develop       # Latest develop
ghcr.io/jsprague84/svrctlrs:develop-sha-abc123  # Specific commit

# Production
ghcr.io/jsprague84/svrctlrs:latest        # Latest release
ghcr.io/jsprague84/svrctlrs:v2.3.0        # Specific version
ghcr.io/jsprague84/svrctlrs:2.3           # Minor version
ghcr.io/jsprague84/svrctlrs:2             # Major version
```

### Files

- `.github/workflows/ci.yml` - CI workflow
- `.github/workflows/release.yml` - Release workflow
- `Dockerfile` - Multi-stage optimized build
- `.dockerignore` - Exclude from build context
- `.cargo/config.toml` - WASM optimization
- `Dioxus.toml` - Dioxus build config

---

## Summary

✅ **CI Workflow**: Fast iteration on `develop` branch
- Single platform (amd64)
- ~5-7 minutes
- Push to `:develop` tag
- For test server

✅ **Release Workflow**: Stable production releases
- Multi-platform (amd64 + arm64)
- ~10-15 minutes
- Push to `:latest` tag
- For production server

✅ **Optimizations**: 5-10x faster builds
- cargo-chef (dependency caching)
- sccache (artifact caching)
- BuildKit (layer caching)
- cargo-binstall (binary installation)
- LLD linker (fast WASM builds)

✅ **Best Practices**:
- Develop on `develop` branch
- Test before releasing
- Use semantic versioning
- Keep clear commit history

**Result**: Fast development iteration + stable production releases! 🚀
