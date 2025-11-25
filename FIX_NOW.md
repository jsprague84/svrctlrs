# Fix Your Current Database Right Now

## Quick Fix (Current Container)

Since your current container doesn't have sqlite3 installed yet, use this command to fix the database from the host:

```bash
# Navigate to your docker-compose directory
cd ~/docker-compose/svrctlrs

# Fix the database (assuming data is mounted to ./data)
sqlite3 data/svrctlrs.db <<EOF
UPDATE tasks SET command = 'system_metrics' WHERE plugin_id = 'health';
UPDATE tasks SET command = 'docker_health' WHERE plugin_id = 'docker';
UPDATE tasks SET command = 'speedtest_run' WHERE plugin_id = 'speedtest';
UPDATE tasks SET command = 'weather_check' WHERE plugin_id = 'weather';
SELECT 'Fixed tasks:' as '';
SELECT id, name, plugin_id, command, enabled FROM tasks;
EOF
```

## Enable Weather and Speedtest Plugins

Add to your `docker-compose.yml` environment section:

```yaml
services:
  svrctlrs:
    environment:
      - ENABLE_WEATHER_PLUGIN=true
      - ENABLE_SPEEDTEST_PLUGIN=true
```

Or create/update `.env` file:

```bash
echo "ENABLE_WEATHER_PLUGIN=true" >> .env
echo "ENABLE_SPEEDTEST_PLUGIN=true" >> .env
```

## Restart with New Image

```bash
# Pull latest image with all fixes
docker-compose pull

# Restart
docker-compose up -d

# Watch logs
docker-compose logs -f svrctlrs
```

## Expected Output

You should see:
- ✅ All 5 plugins registered (docker, updates, health, weather, speedtest)
- ✅ Tasks executing without "Unknown task" errors
- ✅ Task execution results in logs

## After Pulling New Image

Once you have the new image, you can use the built-in helper script:

```bash
docker-compose exec svrctlrs /app/scripts/fix-task-commands.sh
```

## Verify Everything Works

```bash
# Check logs for successful task execution
docker-compose logs -f svrctlrs | grep -i "success\|completed"

# View tasks in database
docker-compose exec svrctlrs sqlite3 /app/data/svrctlrs.db "SELECT id, name, plugin_id, command, enabled FROM tasks;"

# Check plugin registration
docker-compose logs svrctlrs | grep "Registering.*plugin"
```

