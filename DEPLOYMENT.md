# SvrCtlRS Deployment Guide

This guide covers deploying SvrCtlRS using Docker and Docker Compose.

## Prerequisites

- Docker 20.10+ installed
- Docker Compose 2.0+ installed
- SSH access to the server where you want to deploy
- SSH key for remote server monitoring (if monitoring remote servers)

## Quick Start

### 1. Clone the Repository (or pull the image)

```bash
# Option A: Use pre-built image from GitHub Container Registry (recommended)
# No need to clone, just create a directory for your deployment
mkdir ~/svrctlrs && cd ~/svrctlrs

# Option B: Build from source
git clone https://github.com/jsprague84/svrctlrs.git
cd svrctlrs
```

### 2. Configure Environment

```bash
# Copy the example environment file
cp .env.example .env

# Edit the configuration
nano .env
```

**Minimum required configuration:**
```env
# Servers to monitor
SERVERS=localhost

# SSH key path (for remote servers)
SSH_KEY_PATH=~/.ssh/id_rsa

# Notification backend (choose one or both)
GOTIFY_URL=https://your-gotify-server.com
GOTIFY_KEY=your-application-key

# Or use ntfy.sh
NTFY_URL=https://ntfy.sh
NTFY_TOPIC=your-unique-topic
```

### 3. Deploy with Docker Compose

```bash
# Pull and start the container
docker-compose pull
docker-compose up -d

# View logs
docker-compose logs -f

# Check status
docker-compose ps
```

### 4. Access the Dashboard

Open your browser to: `http://your-server-ip:8080`

The web dashboard provides:
- System overview
- Server list and status
- Plugin configuration
- Scheduled tasks
- Settings and statistics

## Deployment Options

### Option 1: Using Pre-built Image (Recommended)

Your `docker-compose.yml`:

```yaml
version: '3.8'

services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:latest
    container_name: svrctlrs-server
    ports:
      - "8080:8080"
    env_file:
      - .env
    volumes:
      - svrctlrs-data:/app/data
      - ~/.ssh/id_rsa:/app/ssh_key:ro
    restart: unless-stopped

volumes:
  svrctlrs-data:
```

### Option 2: Building from Source

Uncomment the build section in `docker-compose.yml`:

```yaml
services:
  svrctlrs:
    build:
      context: .
      dockerfile: Dockerfile
    # ... rest of config
```

Then build and start:

```bash
docker-compose build
docker-compose up -d
```

## Configuration Details

### Server Configuration

Monitor multiple servers by listing them in the `SERVERS` environment variable:

```env
# Format: name:ssh_host or just name for local
SERVERS=webserver:192.168.1.10,dbserver:192.168.1.20,localhost
```

### SSH Key Setup

For remote server monitoring:

1. Generate an SSH key (if you don't have one):
   ```bash
   ssh-keygen -t ed25519 -C "svrctlrs@$(hostname)"
   ```

2. Copy the public key to remote servers:
   ```bash
   ssh-copy-id user@remote-server
   ```

3. Update `.env` with the key path:
   ```env
   SSH_KEY_PATH=~/.ssh/id_ed25519
   ```

4. **Important**: Ensure the key has proper permissions:
   ```bash
   chmod 600 ~/.ssh/id_ed25519
   ```

### Plugin Configuration

Enable/disable plugins as needed:

```env
# Core plugins
ENABLE_DOCKER_PLUGIN=true    # Docker monitoring
ENABLE_UPDATES_PLUGIN=true   # System update monitoring
ENABLE_HEALTH_PLUGIN=true    # Health checks

# Add-on plugins
ENABLE_WEATHER_PLUGIN=false
ENABLE_SPEEDTEST_PLUGIN=false
```

## Updating

### Update to Latest Version

```bash
# Pull the latest image
docker-compose pull

# Recreate the container
docker-compose up -d

# Clean up old images
docker image prune -f
```

### Update to Specific Version

```bash
# Edit docker-compose.yml to use specific version
nano docker-compose.yml

# Change:
# image: ghcr.io/jsprague84/svrctlrs:latest
# To:
# image: ghcr.io/jsprague84/svrctlrs:1.0.0

# Pull and update
docker-compose pull
docker-compose up -d
```

## Backup and Restore

### Backup Database

```bash
# Create backup directory
mkdir -p ~/svrctlrs-backups

# Copy database
docker cp svrctlrs-server:/app/data/svrctlrs.db ~/svrctlrs-backups/svrctlrs-$(date +%Y%m%d).db

# Or use Docker volume backup
docker run --rm \
  -v svrctlrs_svrctlrs-data:/data \
  -v ~/svrctlrs-backups:/backup \
  alpine tar czf /backup/svrctlrs-data-$(date +%Y%m%d).tar.gz /data
```

### Restore Database

```bash
# Stop the container
docker-compose down

# Restore from backup
docker run --rm \
  -v svrctlrs_svrctlrs-data:/data \
  -v ~/svrctlrs-backups:/backup \
  alpine sh -c "cd /data && tar xzf /backup/svrctlrs-data-20240101.tar.gz --strip 1"

# Start the container
docker-compose up -d
```

## Monitoring and Logs

### View Logs

```bash
# Follow all logs
docker-compose logs -f

# Last 100 lines
docker-compose logs --tail=100

# Specific service
docker-compose logs -f svrctlrs
```

### Check Health

```bash
# Using Docker
docker ps

# Using the CLI tool
docker exec svrctlrs-server /app/svrctl health

# Check API endpoint
curl http://localhost:8080/api/v1/health
```

### Resource Usage

```bash
# Container stats
docker stats svrctlrs-server

# Disk usage
docker exec svrctlrs-server du -sh /app/data
```

## Troubleshooting

### Container Won't Start

```bash
# Check logs for errors
docker-compose logs

# Verify configuration
docker-compose config

# Check file permissions
ls -la ~/.ssh/id_rsa
# Should be: -rw------- (600)
```

### Can't Connect to Remote Servers

```bash
# Test SSH connection manually
docker exec -it svrctlrs-server ssh -i /app/ssh_key user@remote-server

# Check SSH key is mounted correctly
docker exec svrctlrs-server ls -la /app/ssh_key
```

### Database Locked Errors

```bash
# Stop the container
docker-compose down

# Start with fresh database
docker volume rm svrctlrs_svrctlrs-data
docker-compose up -d
```

### High Memory Usage

```bash
# Set memory limits in docker-compose.yml
services:
  svrctlrs:
    # ... other config
    deploy:
      resources:
        limits:
          memory: 512M
```

## Security Considerations

1. **SSH Keys**: Mount SSH keys as read-only (`:ro`)
2. **Secrets**: Never commit `.env` file to version control
3. **Firewall**: Restrict port 8080 access if needed
4. **Updates**: Regularly update to latest version
5. **Backups**: Automate database backups

## Advanced Configuration

### Custom Port

```yaml
ports:
  - "3000:8080"  # Access on port 3000 instead
```

### Behind Reverse Proxy (Nginx)

```nginx
server {
    listen 80;
    server_name svrctlrs.example.com;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### Persistent Logs

```yaml
volumes:
  - svrctlrs-data:/app/data
  - ./logs:/app/logs  # Add log persistence
```

## Production Deployment Checklist

- [ ] Configure all required environment variables
- [ ] Set up SSH keys for remote servers
- [ ] Configure notification backends (Gotify/ntfy)
- [ ] Enable only needed plugins
- [ ] Set up automated backups
- [ ] Configure firewall rules
- [ ] Set up reverse proxy with SSL (optional)
- [ ] Test all monitored servers are accessible
- [ ] Verify webhooks are working (if used)
- [ ] Document your deployment configuration

## Support

For issues or questions:
- GitHub Issues: https://github.com/jsprague84/svrctlrs/issues
- Documentation: https://github.com/jsprague84/svrctlrs/tree/master/docs

## License

See LICENSE file in the repository.
