# Docker Plugin Setup Guide

This guide shows how to enable the Docker plugin with automatic GID detection.

## 🎯 The Problem

The Docker plugin needs access to `/var/run/docker.sock` inside the container. This requires the container user to be in the Docker group, which has different GIDs on different systems (typically 999, 998, or 997).

## ✅ The Solution

We've added automatic Docker GID detection that configures everything for you!

## 📋 Setup Steps (On docker-vm)

### Option 1: Automatic (Recommended)

```bash
cd ~/docker-compose/svrctlrs

# Pull the latest image
docker compose pull

# Run the setup script (auto-detects Docker GID)
docker compose run --rm svrctlrs /app/scripts/setup-docker-gid.sh

# Or if you have the repo cloned locally:
./scripts/setup-docker-gid.sh

# Restart with new configuration
docker compose up -d
```

### Option 2: Manual

```bash
cd ~/docker-compose/svrctlrs

# 1. Find your Docker GID
stat -c '%g' /var/run/docker.sock
# Example output: 999

# 2. Add to .env file
echo "DOCKER_GID=999" >> .env

# 3. Restart
docker compose up -d
```

## 🧪 Testing

After setup, test the Docker plugin:

1. Go to http://192.168.5.172:8080/tasks
2. Find the "docker Task"
3. Click "Run Now"
4. Check logs: `docker compose logs -f svrctlrs`

You should see:
```
INFO server::executor: Task 2 completed successfully
```

Instead of:
```
ERROR server::executor: Task 2 failed: Plugin docker execution failed
```

## 🔍 Troubleshooting

### Check Current Configuration

```bash
# View .env file
cat .env | grep DOCKER_GID

# Check container user
docker compose exec svrctlrs id

# Check Docker socket permissions
ls -la /var/run/docker.sock

# Check if container can access Docker
docker compose exec svrctlrs docker ps
```

### Common Issues

**Issue**: "Permission denied" when accessing Docker socket

**Solution**: Make sure DOCKER_GID matches your host's Docker group:
```bash
# On host
stat -c '%g' /var/run/docker.sock

# Should match DOCKER_GID in .env
grep DOCKER_GID .env
```

**Issue**: Setup script not found

**Solution**: Pull the latest image first:
```bash
docker compose pull
docker compose up -d
```

## 📊 What Gets Monitored

Once working, the Docker plugin monitors:

- ✅ Container health status
- ✅ Container resource usage (CPU, memory)
- ✅ Image updates available
- ✅ Container restart counts
- ✅ Network connectivity

## 🔒 Security Note

Mounting the Docker socket gives the container access to the Docker daemon. This is required for the Docker plugin to work, but it does give the container significant permissions.

If you don't need Docker monitoring:
1. Comment out the Docker socket mount in `docker-compose.yml`
2. Disable the Docker plugin in the UI at `/plugins`

## 🎉 Success!

Once configured, the Docker plugin will:
- Run every 5 minutes (configurable)
- Send notifications for issues
- Track historical data
- Show real-time status in the UI

