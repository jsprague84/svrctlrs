# GitHub Actions Workflows

## Overview

This directory contains GitHub Actions workflows for building and publishing Docker images for SvrCtlRS.

## Workflows

### 1. `docker-publish-main.yml` - Production Multi-arch Build

**Triggers:**
- Push to `main` branch
- Git tags matching `v*.*.*` (e.g., `v1.0.0`)
- Manual workflow dispatch

**Features:**
- ✅ Builds for **AMD64 + ARM64** architectures
- ✅ Publishes to GitHub Container Registry (ghcr.io)
- ✅ Tags: `latest`, `main`, version tags, commit SHA
- ✅ Build attestation for security
- ✅ GitHub Actions cache for faster builds

**Build time:** ~15-20 minutes (multi-arch)

**Usage:**
```bash
# Pull latest production image
docker pull ghcr.io/YOUR_USERNAME/svrctlrs:latest

# Pull specific version
docker pull ghcr.io/YOUR_USERNAME/svrctlrs:v1.0.0
```

### 2. `docker-publish-develop.yml` - Fast Development Build

**Triggers:**
- Push to `develop` branch
- Manual workflow dispatch

**Features:**
- ✅ Builds for **AMD64 only** (fast!)
- ✅ Publishes to GitHub Container Registry (ghcr.io)
- ✅ Tags: `develop`, `develop-<commit-sha>`
- ✅ Optimized cache for rapid iteration
- ✅ Perfect for testing on docker-vm

**Build time:** ~5-8 minutes (single arch)

**Usage:**
```bash
# Pull latest develop image
docker pull ghcr.io/YOUR_USERNAME/svrctlrs:develop

# On your docker-vm test server
docker pull ghcr.io/YOUR_USERNAME/svrctlrs:develop
docker-compose up -d
```

### 3. `docker-test-pull.yml` - Automated Testing

**Triggers:**
- After successful `docker-publish-develop.yml` completion
- Manual workflow dispatch

**Features:**
- ✅ Pulls the newly built image
- ✅ Verifies image integrity
- ✅ Tests container startup
- ✅ Generates pull command for easy testing

**Usage:**
Check the workflow summary for the pull command after each develop build.

## Workflow Strategy

### Development Workflow (Fast)

```
1. Make changes
2. Commit to develop branch
3. Push to GitHub
   └─> Triggers: docker-publish-develop.yml
       └─> Builds AMD64 image (~5-8 min)
           └─> Triggers: docker-test-pull.yml
               └─> Verifies image
4. Pull on docker-vm:
   docker pull ghcr.io/YOUR_USERNAME/svrctlrs:develop
5. Test on docker-vm:
   docker-compose up -d
```

### Production Workflow (Multi-arch)

```
1. Merge develop → main
2. Push to GitHub
   └─> Triggers: docker-publish-main.yml
       └─> Builds AMD64 + ARM64 images (~15-20 min)
           └─> Publishes with tags: latest, main, sha
3. Deploy to production:
   docker pull ghcr.io/YOUR_USERNAME/svrctlrs:latest
```

### Release Workflow (Versioned)

```
1. Create git tag:
   git tag v1.0.0
   git push origin v1.0.0
2. Triggers: docker-publish-main.yml
   └─> Builds multi-arch images
       └─> Publishes with tags: v1.0.0, v1.0, v1, latest
3. Users can pull specific versions:
   docker pull ghcr.io/YOUR_USERNAME/svrctlrs:v1.0.0
```

## Configuration

### Environment Variables

Set these in your GitHub repository settings (Settings → Secrets and variables → Actions):

- `GITHUB_TOKEN` - Automatically provided by GitHub Actions
- No additional secrets needed!

### Registry Permissions

The workflows use GitHub Container Registry (ghcr.io) which is automatically configured with your GitHub account.

**To pull images:**
1. Create a GitHub Personal Access Token (PAT) with `read:packages` scope
2. Login to ghcr.io:
   ```bash
   echo $GITHUB_PAT | docker login ghcr.io -u YOUR_USERNAME --password-stdin
   ```

**Or make images public:**
1. Go to https://github.com/users/YOUR_USERNAME/packages/container/svrctlrs/settings
2. Change visibility to "Public"
3. No login required for pulling!

## Testing on docker-vm

### Quick Start

```bash
# On your docker-vm server

# 1. Login to ghcr.io (if private)
echo $GITHUB_PAT | docker login ghcr.io -u YOUR_USERNAME --password-stdin

# 2. Create docker-compose.yml
cat > docker-compose.yml <<EOF
version: '3.8'
services:
  svrctlrs:
    image: ghcr.io/YOUR_USERNAME/svrctlrs:develop
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=debug
    volumes:
      - ./data:/app/data
EOF

# 3. Pull and run
docker-compose pull
docker-compose up -d

# 4. Check logs
docker-compose logs -f

# 5. Test
curl http://localhost:8080/
```

### Update to Latest Develop

```bash
docker-compose pull
docker-compose up -d
docker-compose logs -f
```

## Build Optimization

### Cache Strategy

Both workflows use GitHub Actions cache to speed up builds:

- **Dependencies cache:** Cached until `Cargo.lock` changes
- **sccache:** Caches compiled Rust artifacts
- **Docker layer cache:** Caches Docker build layers

**Result:** Subsequent builds are **3-5x faster**!

### Build Times

| Workflow | Architecture | First Build | Cached Build |
|----------|-------------|-------------|--------------|
| Main | AMD64 + ARM64 | ~20 min | ~10 min |
| Develop | AMD64 only | ~8 min | ~3 min |

## Troubleshooting

### Image not found

```bash
# Check if image exists
docker manifest inspect ghcr.io/YOUR_USERNAME/svrctlrs:develop

# Check workflow status
# Go to: https://github.com/YOUR_USERNAME/svrctlrs/actions
```

### Permission denied

```bash
# Make sure you're logged in
docker login ghcr.io -u YOUR_USERNAME

# Or make the package public (see Registry Permissions above)
```

### Build failed

Check the workflow logs:
1. Go to https://github.com/YOUR_USERNAME/svrctlrs/actions
2. Click on the failed workflow
3. Check the build logs for errors

Common issues:
- Compilation errors → Fix Rust code
- Dependency errors → Update `Cargo.lock`
- Docker build errors → Check `Dockerfile`

## Manual Workflow Dispatch

You can manually trigger workflows from the GitHub UI:

1. Go to https://github.com/YOUR_USERNAME/svrctlrs/actions
2. Select the workflow (e.g., "Docker Publish - Develop")
3. Click "Run workflow"
4. Select branch and click "Run workflow"

## Best Practices

1. **Always test on develop first**
   - Push to `develop` branch
   - Test on docker-vm
   - Merge to `main` when stable

2. **Use semantic versioning for releases**
   - `v1.0.0` for major releases
   - `v1.1.0` for minor updates
   - `v1.0.1` for patches

3. **Monitor build times**
   - Check workflow duration
   - Optimize if builds take too long
   - Use cache effectively

4. **Keep images clean**
   - Delete old develop images periodically
   - Keep only recent versions

## Resources

- [GitHub Container Registry Docs](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
- [Docker Buildx Multi-platform](https://docs.docker.com/build/building/multi-platform/)
- [GitHub Actions Cache](https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows)

