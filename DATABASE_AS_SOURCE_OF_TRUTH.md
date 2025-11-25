# Database as Source of Truth - Migration Guide

## What Changed?

The system has been refactored to make the **database the single source of truth** for all configuration. This eliminates confusion between environment variables and database settings.

### Before (Old Design)
- ❌ Plugins enabled via environment variables (`ENABLE_WEATHER_PLUGIN=true`)
- ❌ Required container restart to enable/disable plugins
- ❌ Configuration split between `.env` and database
- ❌ Confusing: UI showed plugins but couldn't enable them

### After (New Design)
- ✅ **All plugins controlled via database**
- ✅ Enable/disable plugins through UI at `/plugins`
- ✅ No container restart needed
- ✅ Single source of truth: the database
- ✅ Simpler deployment: fewer environment variables

## How It Works Now

### Plugin Management

**1. Default Plugin State (from database migration):**
```sql
-- Core plugins (enabled by default)
docker    - ENABLED
updates   - ENABLED  
health    - ENABLED

-- Add-on plugins (disabled by default)
weather   - DISABLED
speedtest - DISABLED
```

**2. Enable Plugins via UI:**
- Navigate to http://your-server:8080/plugins
- Click toggle switch to enable/disable any plugin
- Changes take effect immediately (no restart needed)
- Configure plugin settings (API keys, schedules, etc.)

**3. Plugin Registration:**
- Server reads database on startup
- Only enabled plugins are registered
- Plugins can be toggled on/off through UI
- Next restart will respect database settings

## Migration Steps

### For Existing Deployments

**1. Pull the latest image:**
```bash
cd ~/docker-compose/svrctlrs
docker compose pull
```

**2. Enable plugins via database:**
```bash
# Enable weather and speedtest plugins
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db <<EOF
UPDATE plugins SET enabled = 1 WHERE id = 'weather';
UPDATE plugins SET enabled = 1 WHERE id = 'speedtest';
SELECT id, name, enabled FROM plugins;
EOF
```

**3. Remove old environment variables:**
```bash
# Edit your .env file and remove these lines:
# ENABLE_WEATHER_PLUGIN=true
# ENABLE_SPEEDTEST_PLUGIN=true
# ENABLE_DOCKER_PLUGIN=true
# ENABLE_UPDATES_PLUGIN=true
# ENABLE_HEALTH_PLUGIN=true
```

**4. Restart:**
```bash
docker compose restart svrctlrs
docker compose logs -f svrctlrs
```

**Expected output:**
```
INFO server: Loading 5 enabled plugins from database
INFO server::state: Registering Docker plugin (enabled in database)
INFO server::state: Registering Updates plugin (enabled in database)
INFO server::state: Registering Health plugin (enabled in database)
INFO server::state: Registering Weather plugin (enabled in database)
INFO server::state: Registering SpeedTest plugin (enabled in database)
```

### For Fresh Deployments

**1. Deploy with docker-compose:**
```bash
docker compose up -d
```

**2. Access UI:**
```
http://your-server:8080
```

**3. Enable plugins:**
- Go to `/plugins`
- Toggle on Weather and Speedtest if needed
- Configure plugin settings
- Done! No restart needed.

## Configuration Reference

### What's Still in Environment Variables?

Only **infrastructure settings** remain in environment variables:

```yaml
environment:
  # Server
  - RUST_LOG=info
  - DATABASE_URL=sqlite:/app/data/svrctlrs.db
  
  # SSH (for remote server management)
  - SSH_KEY_PATH=~/.ssh
  
  # Optional: Notification backends (can also be configured via UI)
  - GOTIFY_URL=
  - GOTIFY_TOKEN=
  - NTFY_URL=
  - NTFY_TOPIC=
  
  # Optional: Weather plugin API key
  - WEATHER_API_KEY=
  - WEATHER_LOCATION=
```

### What's in the Database?

**Everything else:**
- Plugin enabled/disabled state
- Plugin configuration (schedules, thresholds, etc.)
- Server configurations
- Task definitions
- Notification backend configurations
- Task history

## UI Management

### Enable/Disable Plugins

1. Navigate to `/plugins`
2. See list of all available plugins
3. Toggle switch to enable/disable
4. Plugin registers/unregisters immediately

### Configure Plugins

1. Click "Configure" on any enabled plugin
2. Set schedule (cron expression)
3. Set plugin-specific settings:
   - **Weather**: API key, location, units
   - **Speedtest**: Min download/upload thresholds
   - **Docker**: Monitoring intervals
   - **Health**: Metric collection frequency
   - **Updates**: Auto-apply settings
4. Save changes
5. Changes take effect on next scheduled run

### View Plugin Status

- **Green badge**: Plugin enabled and running
- **Gray badge**: Plugin disabled
- **Last run**: Timestamp of last execution
- **Run count**: Total executions
- **Success rate**: Percentage of successful runs

## Database Schema

### Plugins Table

```sql
CREATE TABLE plugins (
    id TEXT PRIMARY KEY,           -- 'docker', 'weather', etc.
    name TEXT NOT NULL,            -- Display name
    description TEXT,              -- Description
    enabled BOOLEAN DEFAULT 1,     -- Enable/disable
    config TEXT,                   -- JSON configuration
    created_at DATETIME,
    updated_at DATETIME
);
```

### Example Query

```sql
-- Enable weather plugin
UPDATE plugins SET enabled = 1 WHERE id = 'weather';

-- Configure weather plugin
UPDATE plugins 
SET config = '{"api_key": "your-key", "location": "New York", "units": "imperial"}'
WHERE id = 'weather';

-- View all plugins
SELECT id, name, enabled FROM plugins;
```

## Benefits

### For Users
- ✅ **Simpler**: Everything in one place (the UI)
- ✅ **Faster**: No container restarts to change settings
- ✅ **Intuitive**: Toggle switches and forms, not environment variables
- ✅ **Persistent**: All settings survive container recreation

### For Developers
- ✅ **Cleaner**: Single source of truth
- ✅ **Maintainable**: Less code, fewer edge cases
- ✅ **Testable**: Database state is easy to inspect
- ✅ **Extensible**: Easy to add new plugins

## Troubleshooting

### Plugin Not Showing Up?

Check database:
```bash
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db "SELECT * FROM plugins;"
```

### Plugin Enabled But Not Running?

Check logs:
```bash
docker compose logs svrctlrs | grep -i "plugin"
```

### Want to Reset to Defaults?

```bash
# Backup database
docker compose exec svrctlrs cp /app/data/svrctlrs.db /app/data/svrctlrs.db.backup

# Reset plugins to defaults
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db <<EOF
DELETE FROM plugins;
INSERT INTO plugins (id, name, description, enabled, config) VALUES
('docker', 'Docker Monitor', 'Monitor Docker containers', 1, '{}'),
('updates', 'Updates Manager', 'Manage OS updates', 1, '{}'),
('health', 'System Health', 'Monitor system resources', 1, '{}'),
('weather', 'Weather Monitor', 'OpenWeatherMap integration', 0, '{}'),
('speedtest', 'Speed Test', 'Internet speed monitoring', 0, '{}');
EOF

# Restart
docker compose restart svrctlrs
```

## Summary

**The database is now the source of truth for:**
- ✅ Plugin enable/disable state
- ✅ Plugin configuration
- ✅ Server configurations  
- ✅ Task definitions
- ✅ Notification backends
- ✅ All application settings

**Environment variables are only for:**
- ⚙️ Infrastructure (database path, ports, logging)
- 🔐 Optional secrets (API keys, tokens)

This makes the system **simpler, more intuitive, and easier to manage**!

