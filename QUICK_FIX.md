# Quick Fix for Plugin Execution Issues

## Issues Found

1. **Task command field mismatch** - Fixed in latest commit
2. **Weather and Speedtest plugins not registered** - Need to enable them
3. **Database tasks have wrong command values** - Need to update database

## Solution

### 1. Enable Weather and Speedtest Plugins

Add to your `docker-compose.yml` or `.env`:

```yaml
environment:
  - ENABLE_WEATHER_PLUGIN=true
  - ENABLE_SPEEDTEST_PLUGIN=true
```

Or in your `.env` file:
```bash
ENABLE_WEATHER_PLUGIN=true
ENABLE_SPEEDTEST_PLUGIN=true
```

### 2. Fix Database Task Commands

The database tasks have incorrect `command` values. They should match the plugin's task IDs:

```bash
# Connect to the database
sqlite3 data/svrctlrs.db

# Update task commands to match plugin task IDs
UPDATE tasks SET command = 'system_metrics' WHERE plugin_id = 'health';
UPDATE tasks SET command = 'docker_health' WHERE plugin_id = 'docker';
UPDATE tasks SET command = 'speedtest_run' WHERE plugin_id = 'speedtest';
UPDATE tasks SET command = 'weather_check' WHERE plugin_id = 'weather';

# Verify the changes
SELECT id, name, plugin_id, command, enabled FROM tasks;

# Exit
.quit
```

### 3. Restart the Container

```bash
docker-compose down
docker-compose pull  # Get latest image
docker-compose up -d
docker-compose logs -f svrctlrs
```

## Expected Plugin Task IDs

Each plugin expects specific task IDs in the `command` field:

### Docker Plugin
- `docker_health` - Check Docker container health (every 5 min)
- `docker_cleanup` - Analyze cleanup opportunities (Sundays 2 AM)
- `docker_analysis` - Advanced analysis (Sundays 3 AM)

### Health Plugin
- `system_metrics` - Collect system metrics (every 5 min)

### Updates Plugin
- `updates_check` - Check for OS updates (every 6 hours)
- `updates_apply` - Apply OS updates (Sundays 3 AM, disabled by default)
- `os_cleanup` - Clean package cache (Sundays 4 AM, disabled by default)

### Weather Plugin
- `weather_check` - Fetch weather data (configurable schedule)

### Speedtest Plugin
- `speedtest_run` - Run speed test (configurable schedule)

## Verification

After restarting, you should see:

```
INFO server::state: Registering Docker plugin
INFO server::state: Registering Updates plugin
INFO server::state: Registering Health plugin
INFO server::state: Registering Weather plugin (add-on)
INFO server::state: Registering SpeedTest plugin (add-on)
```

And tasks should execute without "Unknown task" errors.

## Alternative: Recreate Database

If you want a fresh start:

```bash
# Stop container
docker-compose down

# Backup old database (optional)
mv data/svrctlrs.db data/svrctlrs.db.backup

# Start container (will create new database with migrations)
docker-compose up -d

# Configure plugins via UI at http://localhost:8080/plugins
```

Then configure each plugin through the UI with proper schedules and settings.

