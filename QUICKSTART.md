# SvrCtlRS Quick Start Guide

Get SvrCtlRS up and running in 5 minutes with Docker Compose!

## 📋 Prerequisites

- Docker Engine 20.10+
- Docker Compose V2
- SSH keys (for remote server management)

## 🚀 Quick Start

### 1. Create Configuration Files

```bash
# Create .env file from example
cp .env.example .env

# Create config.toml from example
cp config/example.toml config.toml
```

### 2. Edit Configuration

**Edit `.env`** (optional - defaults work for basic setup):
```bash
# Change the image tag if needed
TAG=latest

# Change the port if 8080 is in use
HTTP_PORT=8080

# Set your SSH key path if not using ~/.ssh
SSH_KEY_PATH=~/.ssh
```

**Edit `config.toml`** (required for full functionality):
```toml
[server]
addr = "0.0.0.0:8080"
database_url = "sqlite:/app/data/svrctlrs.db"

# Add your servers
[[servers]]
name = "server1"
host = "user@hostname"
ssh_key = "/home/svrctlrs/.ssh/id_rsa"

[[servers]]
name = "server2"
host = "user@another-host"
ssh_key = "/home/svrctlrs/.ssh/id_ed25519"

# Optional: Configure notifications
[notifications]
gotify_url = "http://gotify:8080/message"
gotify_key = "your-gotify-token"
ntfy_url = "https://ntfy.sh"
ntfy_topic = "svrctlrs-alerts"

# Optional: Configure plugins
[plugins]
docker_enabled = true
updates_enabled = true
health_enabled = true
weather_enabled = false
speedtest_enabled = false
```

### 3. Start the Application

```bash
# Pull the latest image
docker-compose pull

# Start in detached mode
docker-compose up -d

# Check logs
docker-compose logs -f
```

### 4. Access the Web UI

Open your browser to: **http://localhost:8080**

Default credentials (if authentication is enabled):
- Username: `admin`
- Password: `admin` (change immediately!)

## 📁 File Structure

After setup, your directory should look like:

```
your-deployment/
├── docker-compose.yml    # Docker Compose configuration
├── .env                  # Environment variables (from .env.example)
├── config.toml          # Application configuration (from config/example.toml)
└── (optional) docker-compose.override.yml  # Custom overrides
```

## 🔧 Common Tasks

### View Logs
```bash
docker-compose logs -f
```

### Restart Application
```bash
docker-compose restart
```

### Stop Application
```bash
docker-compose down
```

### Update to Latest Version
```bash
docker-compose pull
docker-compose up -d
```

### Backup Database
```bash
docker-compose exec svrctlrs /app/svrctl backup
# Or manually copy the volume
docker cp svrctlrs:/app/data/svrctlrs.db ./backup-$(date +%Y%m%d).db
```

### Access CLI
```bash
docker-compose exec svrctlrs /app/svrctl --help
```

## 🐛 Troubleshooting

### Container Won't Start

Check logs:
```bash
docker-compose logs
```

Common issues:
- Port 8080 already in use → Change `HTTP_PORT` in `.env`
- Config file not found → Ensure `config.toml` exists
- SSH keys not accessible → Check `SSH_KEY_PATH` in `.env`

### Can't Connect to Servers

1. Verify SSH keys are mounted:
   ```bash
   docker-compose exec svrctlrs ls -la /home/svrctlrs/.ssh
   ```

2. Test SSH connection:
   ```bash
   docker-compose exec svrctlrs ssh -i /home/svrctlrs/.ssh/id_rsa user@host
   ```

3. Check config.toml has correct server definitions

### Database Issues

Reset database (⚠️ deletes all data):
```bash
docker-compose down
docker volume rm svrctlrs_svrctlrs-data
docker-compose up -d
```

### CSS Not Loading

This should be fixed in the latest version. If you still see issues:
```bash
# Pull latest image
docker-compose pull

# Force recreate
docker-compose up -d --force-recreate
```

## 🔒 Security Recommendations

### 1. Change Default Credentials
Edit `config.toml` and set a strong password.

### 2. Use HTTPS
Add a reverse proxy (Nginx, Traefik, Caddy) in front of SvrCtlRS:

**Example with Caddy:**
```yaml
# docker-compose.override.yml
services:
  caddy:
    image: caddy:latest
    ports:
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy-data:/data
    depends_on:
      - svrctlrs

volumes:
  caddy-data:
```

**Caddyfile:**
```
svrctlrs.yourdomain.com {
    reverse_proxy svrctlrs:8080
}
```

### 3. Restrict SSH Key Access
Only mount the specific keys needed:
```yaml
# In docker-compose.override.yml
services:
  svrctlrs:
    volumes:
      - ./ssh-keys:/home/svrctlrs/.ssh:ro
```

### 4. Set Resource Limits
Uncomment the `deploy.resources` section in `docker-compose.yml`.

## 📊 Monitoring

### Health Check
```bash
# Via Docker
docker-compose ps

# Via CLI
docker-compose exec svrctlrs /app/svrctl health

# Via HTTP
curl http://localhost:8080/api/health
```

### Metrics
View metrics in the web UI:
- Dashboard: http://localhost:8080/
- Tasks: http://localhost:8080/tasks
- Plugins: http://localhost:8080/plugins

## 🔄 Updating

### Stable Releases (Recommended)
```bash
# Edit .env
TAG=latest

# Pull and restart
docker-compose pull
docker-compose up -d
```

### Development Builds
```bash
# Edit .env
TAG=develop

# Pull and restart
docker-compose pull
docker-compose up -d
```

### Specific Version
```bash
# Edit .env
TAG=v1.0.0

# Pull and restart
docker-compose pull
docker-compose up -d
```

## 📚 Next Steps

- **[README.md](./README.md)** - Full project documentation
- **[config/example.toml](./config/example.toml)** - All configuration options
- **[docs/](./docs/)** - Detailed documentation
- **[CLAUDE.md](./CLAUDE.md)** - Development guide

## 💡 Tips

1. **Use `.env` for secrets** - Keep sensitive data out of `config.toml`
2. **Backup regularly** - Database is in the `svrctlrs-data` volume
3. **Check logs** - Most issues are visible in logs
4. **Use docker-compose.override.yml** - For custom configurations
5. **Keep updated** - Pull latest images regularly for bug fixes

## 🆘 Getting Help

- **Issues**: https://github.com/jsprague84/svrctlrs/issues
- **Discussions**: https://github.com/jsprague84/svrctlrs/discussions
- **Documentation**: https://github.com/jsprague84/svrctlrs/tree/main/docs

---

**That's it! You should now have SvrCtlRS running.** 🎉

Access the web UI at http://localhost:8080 and start managing your servers!

