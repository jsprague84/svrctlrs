# 🎉 SvrCtlRS - Complete Database-Driven Deployment Guide

## Overview

SvrCtlRS is now a **fully database-driven** server monitoring and management system with:
- ✅ **5 Plugins**: Docker, Updates, Health, Weather, Speedtest
- ✅ **Database Configuration**: All settings managed via UI
- ✅ **Minimal Environment Variables**: Only 3 infrastructure settings
- ✅ **HTMX + Askama UI**: Fast, responsive, mobile-friendly
- ✅ **Multi-arch Docker Images**: AMD64 and ARM64 support

---

## Quick Start (5 Minutes)

### 1. Create Directory Structure

```bash
mkdir -p ~/docker-compose/svrctlrs
cd ~/docker-compose/svrctlrs
```

### 2. Download Configuration Files

```bash
# Download docker-compose.yml
curl -O https://raw.githubusercontent.com/jsprague84/svrctlrs/develop/docker-compose.yml

# Download env.example
curl -O https://raw.githubusercontent.com/jsprague84/svrctlrs/develop/env.example

# Copy to .env
cp env.example .env
```

### 3. Configure Environment (Optional)

Edit `.env` to customize:

```bash
# Docker Image
TAG=develop  # or 'latest' for stable

# Server
HTTP_PORT=8080

# Logging
RUST_LOG=info  # or 'debug' for troubleshooting
```

### 4. Configure Docker Plugin (Optional)

If you want Docker monitoring:

```bash
# Find your docker group GID
stat -c '%g' /var/run/docker.sock

# Edit docker-compose.yml and update the user line:
# user: "1000:999"  # Replace 999 with your docker GID
```

### 5. Start the Container

```bash
docker compose up -d
```

### 6. Access the UI

Open http://localhost:8080 (or your server IP)

---

## Configuration via UI

### 1. Add Servers

Navigate to **Servers** → **Add Server**

- Name: `my-server`
- Host: `server.example.com` or IP address
- Port: `22`
- Username: `ubuntu`
- Description: Optional

**Note**: SSH keys are automatically loaded from `~/.ssh/` (mounted volume)

### 2. Configure Plugins

Navigate to **Plugins** → Click **Configure** on any plugin

#### Docker Plugin
- **Schedule**: `0 */5 * * * *` (every 5 minutes)
- **Requirements**: Docker socket mounted (see Quick Start #4)

#### Health Plugin
- **Schedule**: `0 */5 * * * *` (every 5 minutes)
- **Monitors**: CPU, memory, disk usage

#### Updates Plugin
- **Schedule**: `0 6 * * *` (6 AM daily)
- **Checks**: Available OS updates

#### Weather Plugin
- **Schedule**: `0 6 * * *` (6 AM daily)
- **API Key**: Get from [OpenWeatherMap](https://openweathermap.org/api)
- **Location**: ZIP code (e.g., `52726`) or city (e.g., `Davenport,IA`)
- **Units**: `imperial` or `metric`

#### Speedtest Plugin
- **Schedule**: `0 */6 * * * *` (every 6 hours)
- **Min Download**: `1000` Mbps (threshold for alerts)
- **Min Upload**: `40` Mbps (threshold for alerts)

### 3. Configure Notifications

Navigate to **Settings** → **Notifications** → **Add Backend**

#### Gotify
- **Name**: `My Gotify`
- **URL**: `https://gotify.example.com`
- **Token**: Your Gotify app token

#### ntfy.sh
- **Name**: `My ntfy`
- **URL**: `https://ntfy.sh` (or self-hosted)
- **Topic**: `my-server-alerts`
- **Token**: Optional (for protected topics)

### 4. Create Tasks

Navigate to **Tasks** → **Add Task**

- **Name**: `Check Docker Health`
- **Plugin**: `docker`
- **Command**: `docker_health`
- **Schedule**: `0 */5 * * * *`
- **Server**: Select target server (or leave empty for local)

---

## Database Fixes (If Needed)

If you upgraded from an older version, you may need to fix task IDs:

```bash
# Fix Updates plugin task ID
docker compose exec svrctlrs /app/scripts/fix-plugin-task-ids.sh

# Restart
docker compose restart svrctlrs
```

---

## Troubleshooting

### Weather Plugin: "OWM_API_KEY not configured"

**Cause**: Plugin config not loaded from database

**Fix**: 
1. Wait for GitHub Actions build to complete (~4 min)
2. Pull new image: `docker compose pull`
3. Restart: `docker compose restart svrctlrs`

### Speedtest Plugin: "speedtest CLI not found"

**Cause**: Old Docker image without Ookla CLI

**Fix**: Same as above (pull new image)

### Docker Plugin: "Failed to list containers"

**Cause**: Docker socket not mounted or wrong GID

**Fix**:
1. Verify socket is mounted in `docker-compose.yml`
2. Find docker GID: `stat -c '%g' /var/run/docker.sock`
3. Update `user:` in `docker-compose.yml` to `"1000:YOUR_GID"`
4. Restart: `docker compose restart svrctlrs`

### Updates Plugin: "Unknown task: execute"

**Cause**: Old database with wrong task ID

**Fix**: Run the fix script (see Database Fixes above)

### Check Logs

```bash
# View all logs
docker compose logs svrctlrs

# Follow logs
docker compose logs -f svrctlrs

# Last 100 lines
docker compose logs --tail=100 svrctlrs
```

---

## Architecture

### Database Schema

```
servers          - Server configurations
tasks            - Scheduled tasks
task_history     - Task execution history
plugins          - Plugin configurations and enable/disable
notification_backends - Gotify and ntfy configurations
```

### Plugin System

Each plugin:
- Registers with the system on startup
- Loads configuration from database
- Provides one or more executable tasks
- Can send notifications via configured backends

### Task Execution

1. Scheduler loads enabled tasks from database
2. Tasks run on schedule (cron expressions)
3. Results stored in `task_history`
4. Notifications sent on failures or thresholds

---

## Environment Variables (Minimal)

Only 3 settings remain as environment variables:

```bash
RUST_LOG=info                              # Logging level
DATABASE_URL=sqlite:/app/data/svrctlrs.db  # Database path
SSH_KEY_PATH=/home/svrctlrs/.ssh           # SSH keys directory
```

**Everything else** is configured via the database and UI!

---

## Security Notes

### Docker Socket Access

Mounting `/var/run/docker.sock` gives the container **full Docker daemon access**. This is required for the Docker plugin but is a security consideration.

**Recommendations**:
- Only mount if you need Docker monitoring
- Use read-only mount (`:ro`)
- Run container as non-root user (already configured)
- Monitor container logs for suspicious activity

### SSH Keys

SSH keys are mounted read-only from your host. The container:
- Runs as user `1000` (svrctlrs)
- Cannot modify your SSH keys
- Uses keys for remote server management only

---

## Updating

### Update to Latest Develop Build

```bash
cd ~/docker-compose/svrctlrs
docker compose pull
docker compose up -d
```

### Update to Stable Release

```bash
# Edit .env
TAG=latest

# Pull and restart
docker compose pull
docker compose up -d
```

---

## Backup and Restore

### Backup Database

```bash
# Stop container
docker compose stop

# Backup database
docker cp svrctlrs:/app/data/svrctlrs.db ./backup-$(date +%Y%m%d).db

# Start container
docker compose start
```

### Restore Database

```bash
# Stop container
docker compose stop

# Restore database
docker cp ./backup-20231125.db svrctlrs:/app/data/svrctlrs.db

# Fix permissions
docker compose exec svrctlrs chown svrctlrs:svrctlrs /app/data/svrctlrs.db

# Start container
docker compose start
```

---

## Support

- **GitHub**: https://github.com/jsprague84/svrctlrs
- **Issues**: https://github.com/jsprague84/svrctlrs/issues
- **Documentation**: https://github.com/jsprague84/svrctlrs/tree/develop/docs

---

## License

MIT License - See LICENSE file for details

