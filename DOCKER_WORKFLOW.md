# Docker Workflow - Development & Production

## Quick Reference

### Development Workflow (Fast - AMD64 only)

```bash
# 1. Make changes locally
git add .
git commit -m "feat: add new feature"
git push origin develop

# 2. GitHub Actions automatically builds AMD64 image (~5-8 min)
#    Image published as: ghcr.io/YOUR_USERNAME/svrctlrs:develop

# 3. On your docker-vm test server:
docker-compose pull
docker-compose up -d

# 4. Test and verify
docker-compose logs -f
```

### Production Workflow (Multi-arch - AMD64 + ARM64)

```bash
# 1. Merge to main
git checkout main
git merge develop
git push origin main

# 2. GitHub Actions builds multi-arch image (~15-20 min)
#    Image published as: ghcr.io/YOUR_USERNAME/svrctlrs:latest

# 3. Deploy to production
docker pull ghcr.io/YOUR_USERNAME/svrctlrs:latest
docker-compose up -d
```

## GitHub Actions Workflows

### `develop` branch → Fast AMD64 Build
- **Trigger**: Push to `develop`
- **Build time**: ~5-8 minutes
- **Platforms**: AMD64 only
- **Tags**: `develop`, `develop-<sha>`
- **Use case**: Rapid testing on docker-vm

### `main` branch → Multi-arch Build
- **Trigger**: Push to `main` or version tags
- **Build time**: ~15-20 minutes
- **Platforms**: AMD64 + ARM64
- **Tags**: `latest`, `main`, `v*.*.*`, `<sha>`
- **Use case**: Production deployments

## docker-compose.yml Setup

On your docker-vm, create or use the provided `docker-compose.yml`:

```yaml
version: '3.8'

services:
  svrctlrs:
    image: ghcr.io/YOUR_USERNAME/svrctlrs:develop
    container_name: svrctlrs
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - DATABASE_URL=sqlite:/app/data/svrctlrs.db
    volumes:
      - svrctlrs-data:/app/data
      - ~/.ssh:/home/svrctlrs/.ssh:ro  # Optional: for SSH keys
    healthcheck:
      test: ["/app/svrctl", "health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  svrctlrs-data:
```

## Typical Development Cycle

```
Local Development
    ↓
git push origin develop
    ↓
GitHub Actions builds AMD64 image (5-8 min)
    ↓
docker-compose pull (on docker-vm)
    ↓
docker-compose up -d
    ↓
Test & Verify
    ↓
Repeat or merge to main
```

## Useful Commands

### On docker-vm

```bash
# Pull latest develop image
docker-compose pull

# Restart with new image
docker-compose up -d

# View logs
docker-compose logs -f

# Check status
docker-compose ps

# Stop
docker-compose down

# Clean up old images
docker image prune -a
```

### Check Build Status

Visit: `https://github.com/YOUR_USERNAME/svrctlrs/actions`

## First Time Setup

### 1. Make Repository Package Public (Optional)

If you want to skip login on docker-vm:

1. Go to: `https://github.com/users/YOUR_USERNAME/packages/container/svrctlrs/settings`
2. Change visibility to "Public"
3. No `docker login` needed!

### 2. Or Login with Personal Access Token

```bash
# Create PAT with read:packages scope at:
# https://github.com/settings/tokens

# Login on docker-vm
echo $GITHUB_PAT | docker login ghcr.io -u YOUR_USERNAME --password-stdin
```

## Build Cache

Both workflows use GitHub Actions cache for faster builds:
- First build: ~8-20 minutes
- Cached builds: ~3-10 minutes

Cache is automatically managed by GitHub Actions.

## Troubleshooting

### Image not updating on docker-vm

```bash
# Force pull and recreate
docker-compose pull
docker-compose up -d --force-recreate
```

### Check if new image is available

```bash
# Get image digest
docker manifest inspect ghcr.io/YOUR_USERNAME/svrctlrs:develop | grep digest
```

### View build logs

1. Go to: `https://github.com/YOUR_USERNAME/svrctlrs/actions`
2. Click on latest workflow run
3. View build logs

## That's It!

Your workflow is:
1. **Push to develop** → Auto-builds AMD64 image
2. **Pull on docker-vm** → `docker-compose pull && docker-compose up -d`
3. **Test** → Real-world environment
4. **Repeat** → Fast iteration!

