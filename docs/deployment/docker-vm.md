# Docker VM Setup Guide

Quick reference for deploying SvrCtlRS on your docker-vm test server.

## Initial Setup

### 1. Create Directory

```bash
ssh ubuntu@docker-vm
mkdir -p ~/docker-compose/svrctlrs
cd ~/docker-compose/svrctlrs
```

### 2. Create docker-compose.yml

```yaml
services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:develop
    container_name: svrctlrs
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=info
      - DATABASE_URL=sqlite:/app/data/svrctlrs.db
    volumes:
      - svrctlrs-data:/app/data
      # Optional: Mount SSH keys for remote execution
      # - ~/.ssh:/home/svrctlrs/.ssh:ro
    healthcheck:
      test: ["CMD", "/app/svrctl", "health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 5s

volumes:
  svrctlrs-data:
    driver: local
```

### 3. Login to GitHub Container Registry (if private)

```bash
# Create a GitHub Personal Access Token with read:packages scope
# Then login:
echo $GITHUB_PAT | docker login ghcr.io -u jsprague84 --password-stdin
```

Or make the package public:
- Go to: https://github.com/users/jsprague84/packages/container/svrctlrs/settings
- Change visibility to "Public"

## Daily Usage

### Pull Latest Develop Image

```bash
cd ~/docker-compose/svrctlrs
docker-compose pull
docker-compose up -d
```

### View Logs

```bash
docker-compose logs -f
```

### Check Status

```bash
docker-compose ps
docker-compose exec svrctlrs /app/svrctl health
```

### Restart

```bash
docker-compose restart
```

### Stop

```bash
docker-compose down
```

### Clean Restart (Remove Data)

```bash
docker-compose down -v
docker-compose up -d
```

## Testing After Push

After pushing to develop branch:

```bash
# 1. Wait for GitHub Actions to complete (~5-8 min)
#    Check: https://github.com/jsprague84/svrctlrs/actions

# 2. Pull new image
cd ~/docker-compose/svrctlrs
docker-compose pull

# 3. Restart with new image
docker-compose up -d

# 4. Watch logs
docker-compose logs -f

# 5. Test UI
curl http://localhost:8080/
# Or open in browser: http://docker-vm:8080
```

## Accessing the UI

- **URL**: http://docker-vm:8080 (or http://localhost:8080 if on the VM)
- **Pages**:
  - Dashboard: http://docker-vm:8080/
  - Servers: http://docker-vm:8080/servers
  - Tasks: http://docker-vm:8080/tasks
  - Plugins: http://docker-vm:8080/plugins
  - Settings: http://docker-vm:8080/settings

## Troubleshooting

### Image Not Updating

```bash
# Force pull and recreate
docker-compose pull
docker-compose up -d --force-recreate
```

### Check Image Version

```bash
docker images ghcr.io/jsprague84/svrctlrs
```

### View Container Logs

```bash
docker-compose logs --tail=100 -f
```

### Enter Container

```bash
docker-compose exec svrctlrs /bin/bash
```

### Check Health

```bash
docker-compose exec svrctlrs /app/svrctl health
```

### Clean Up Old Images

```bash
docker image prune -a
```

## Environment Variables

You can customize the deployment by setting environment variables:

```bash
# Create .env file
cat > .env <<EOF
RUST_LOG=debug
TAG=develop
EOF

# Then start
docker-compose up -d
```

## Quick Commands Cheat Sheet

```bash
# Pull and restart
dcp && dcu -d

# View logs
dcl -f

# Stop
dcd

# Clean restart
dcd -v && dcu -d
```

Where:
- `dcp` = `docker-compose pull`
- `dcu` = `docker-compose up`
- `dcl` = `docker-compose logs`
- `dcd` = `docker-compose down`

